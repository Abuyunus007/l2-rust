//! Quest 372: Legacy of Insolence (Walderal in Giran, the Tower of Insolence papyri and books).
// Java: dist/game/data/scripts/quests/Q00372_LegacyOfInsolence/Q00372_LegacyOfInsolence.java
//
// Pattern: a kill quest whose items are traded for a random reward; the rolls are tables of
// `(upper limit, reward)` read from the lowest limit up (`if chance < 10 ... else if chance < 20`).

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const TRADER_HOLLY: i32 = 30839;
const WAREHOUSE_KEEPER_WALDERAL: i32 = 30844;
const MAGISTER_DESMOND: i32 = 30855;
const ANTIQUE_DEALER_PATRIN: i32 = 30929;
const CLAUDIA_ATHEBALDT: i32 = 31001;

// Items
const ANCIENT_RED_PAPYRUS: i32 = 5966;
const ANCIENT_BLUE_PAPYRUS: i32 = 5967;
const ANCIENT_BLACK_PAPYRUS: i32 = 5968;
const ANCIENT_WHITE_PAPYRUS: i32 = 5969;
const REVELATIONS: [i32; 7] = [5972, 5973, 5974, 5975, 5976, 5977, 5978];
const ANCIENT_EPIC_CHAPTERS: [i32; 5] = [5979, 5980, 5981, 5982, 5983];
const IMPERIAL_GENEALOGIES: [i32; 5] = [5984, 5985, 5986, 5987, 5988];
const BLUEPRINTS: [i32; 13] = [
    5989, 5990, 5991, 5992, 5993, 5994, 5995, 5996, 5997, 5998, 5999, 6000, 6001,
];

// Rewards
const RECIPE_SEALED_DARK_CRYSTAL_BOOTS_60: i32 = 5368;
const RECIPE_SEALED_TALLUM_BOOTS_60: i32 = 5370;
const RECIPE_SEALED_BOOTS_OF_NIGHTMARE_60: i32 = 5380;
const RECIPE_SEALED_MAJESTIC_BOOTS_60: i32 = 5382;
const RECIPE_SEALED_DARK_CRYSTAL_GLOVES_60: i32 = 5392;
const RECIPE_SEALED_TALLUM_GLOVES_60: i32 = 5394;
const RECIPE_SEALED_GAUNTLETS_OF_NIGHTMARE_60: i32 = 5404;
const RECIPE_SEALED_MAJESTIC_GAUNTLETS_60: i32 = 5406;
const RECIPE_SEALED_DARK_CRYSTAL_HELMET_60: i32 = 5426;
const RECIPE_SEALED_TALLUM_HELMET_60: i32 = 5428;
const RECIPE_SEALED_HELM_OF_NIGHTMARE_60: i32 = 5430;
const RECIPE_SEALED_MAJESTIC_CIRCLET_60: i32 = 5432;
const SEALED_DARK_CRYSTAL_BOOTS_LINING: i32 = 5496;
const SEALED_TALLUM_BOOTS_LINING: i32 = 5497;
const SEALED_BOOTS_OF_NIGHTMARE_LINING: i32 = 5502;
const SEALED_MAJESTIC_BOOTS_LINING: i32 = 5503;
const SEALED_DARK_CRYSTAL_GLOVES_DESIGN: i32 = 5508;
const SEALED_TALLUM_GLOVES_DESIGN: i32 = 5509;
const SEALED_GAUNTLETS_OF_NIGHTMARE_DESIGN: i32 = 5514;
const SEALED_MAJESTIC_GAUNTLETS_DESIGN: i32 = 5515;
const SEALED_DARK_CRYSTAL_HELMET_DESIGN: i32 = 5525;
const SEALED_TALLUM_HELM_DESIGN: i32 = 5526;
const SEALED_HELM_OF_NIGHTMARE_DESIGN: i32 = 5527;
const SEALED_MAJESTIC_CIRCLET_DESIGN: i32 = 5528;

// Monsters
const HALLATES_INSPECTOR: i32 = 20825;
/// Java `MONSTER_REWARDS`: monster, papyrus, chance (of 1000), count.
const MONSTER_REWARDS: [(i32, i32, i32, i64); 6] = [
    (20817, ANCIENT_RED_PAPYRUS, 302, 1),
    (20821, ANCIENT_RED_PAPYRUS, 410, 1),
    (HALLATES_INSPECTOR, ANCIENT_RED_PAPYRUS, 447, 1),
    (20829, ANCIENT_BLUE_PAPYRUS, 451, 1),
    (21062, ANCIENT_WHITE_PAPYRUS, 290, 1),
    (21069, ANCIENT_BLACK_PAPYRUS, 280, 1),
];

// Misc
const MIN_LEVEL: i32 = 59;

/// What one step of a reward table gives.
enum Reward {
    Items(&'static [i32]),
    Adena(i64),
}

use Reward::{Adena, Items};

/// The first row whose limit is above `chance` (Java's `if (chance < a) .. else if (chance < b)`);
/// a roll past the last limit gives nothing.
fn give_reward(ctx: &mut ScriptCtx<'_>, player: ObjectId, chance: i32, table: &[(i32, Reward)]) {
    if let Some((_, reward)) = table.iter().find(|(limit, _)| chance < *limit) {
        match reward {
            Items(items) => {
                for &item in *items {
                    ctx.give_items(player, item, 1);
                }
            }
            Adena(amount) => ctx.give_adena(player, *amount, true),
        }
    }
}

const BLUEPRINT_DARK_CRYSTAL: [(i32, Reward); 8] = [
    (10, Items(&[SEALED_DARK_CRYSTAL_BOOTS_LINING])),
    (20, Items(&[SEALED_DARK_CRYSTAL_GLOVES_DESIGN])),
    (30, Items(&[SEALED_DARK_CRYSTAL_HELMET_DESIGN])),
    (
        40,
        Items(&[
            SEALED_DARK_CRYSTAL_BOOTS_LINING,
            SEALED_DARK_CRYSTAL_GLOVES_DESIGN,
            SEALED_DARK_CRYSTAL_HELMET_DESIGN,
        ]),
    ),
    (51, Items(&[RECIPE_SEALED_DARK_CRYSTAL_BOOTS_60])),
    (62, Items(&[RECIPE_SEALED_DARK_CRYSTAL_GLOVES_60])),
    (79, Items(&[RECIPE_SEALED_DARK_CRYSTAL_HELMET_60])),
    (
        100,
        Items(&[
            RECIPE_SEALED_DARK_CRYSTAL_BOOTS_60,
            RECIPE_SEALED_DARK_CRYSTAL_GLOVES_60,
            RECIPE_SEALED_DARK_CRYSTAL_HELMET_60,
        ]),
    ),
];

const BLUEPRINT_TALLUM: [(i32, Reward); 8] = [
    (10, Items(&[SEALED_TALLUM_BOOTS_LINING])),
    (20, Items(&[SEALED_TALLUM_GLOVES_DESIGN])),
    (30, Items(&[SEALED_TALLUM_HELM_DESIGN])),
    (
        40,
        Items(&[
            SEALED_TALLUM_BOOTS_LINING,
            SEALED_TALLUM_GLOVES_DESIGN,
            SEALED_TALLUM_HELM_DESIGN,
        ]),
    ),
    (51, Items(&[RECIPE_SEALED_TALLUM_BOOTS_60])),
    (62, Items(&[RECIPE_SEALED_TALLUM_GLOVES_60])),
    (79, Items(&[RECIPE_SEALED_TALLUM_HELMET_60])),
    (
        100,
        Items(&[
            RECIPE_SEALED_TALLUM_BOOTS_60,
            RECIPE_SEALED_TALLUM_GLOVES_60,
            RECIPE_SEALED_TALLUM_HELMET_60,
        ]),
    ),
];

const BLUEPRINT_NIGHTMARE: [(i32, Reward); 8] = [
    (17, Items(&[SEALED_BOOTS_OF_NIGHTMARE_LINING])),
    (34, Items(&[SEALED_GAUNTLETS_OF_NIGHTMARE_DESIGN])),
    (49, Items(&[SEALED_HELM_OF_NIGHTMARE_DESIGN])),
    (
        58,
        Items(&[
            SEALED_BOOTS_OF_NIGHTMARE_LINING,
            SEALED_GAUNTLETS_OF_NIGHTMARE_DESIGN,
            SEALED_HELM_OF_NIGHTMARE_DESIGN,
        ]),
    ),
    (70, Items(&[RECIPE_SEALED_BOOTS_OF_NIGHTMARE_60])),
    (82, Items(&[RECIPE_SEALED_GAUNTLETS_OF_NIGHTMARE_60])),
    (92, Items(&[RECIPE_SEALED_HELM_OF_NIGHTMARE_60])),
    (
        100,
        Items(&[
            RECIPE_SEALED_BOOTS_OF_NIGHTMARE_60,
            RECIPE_SEALED_GAUNTLETS_OF_NIGHTMARE_60,
            RECIPE_SEALED_HELM_OF_NIGHTMARE_60,
        ]),
    ),
];

const BLUEPRINT_MAJESTIC: [(i32, Reward); 8] = [
    (17, Items(&[SEALED_MAJESTIC_BOOTS_LINING])),
    (34, Items(&[SEALED_MAJESTIC_GAUNTLETS_DESIGN])),
    (49, Items(&[SEALED_MAJESTIC_CIRCLET_DESIGN])),
    (
        58,
        Items(&[
            SEALED_MAJESTIC_BOOTS_LINING,
            SEALED_MAJESTIC_GAUNTLETS_DESIGN,
            SEALED_MAJESTIC_CIRCLET_DESIGN,
        ]),
    ),
    (70, Items(&[RECIPE_SEALED_MAJESTIC_BOOTS_60])),
    (82, Items(&[RECIPE_SEALED_MAJESTIC_GAUNTLETS_60])),
    (92, Items(&[RECIPE_SEALED_MAJESTIC_CIRCLET_60])),
    (
        100,
        Items(&[
            RECIPE_SEALED_MAJESTIC_BOOTS_60,
            RECIPE_SEALED_MAJESTIC_GAUNTLETS_60,
            RECIPE_SEALED_MAJESTIC_CIRCLET_60,
        ]),
    ),
];

const HOLLY_REWARD: [(i32, Reward); 5] = [
    (30, Items(&[SEALED_DARK_CRYSTAL_BOOTS_LINING])),
    (60, Items(&[SEALED_DARK_CRYSTAL_GLOVES_DESIGN])),
    (80, Items(&[SEALED_DARK_CRYSTAL_HELMET_DESIGN])),
    (
        90,
        Items(&[
            SEALED_DARK_CRYSTAL_BOOTS_LINING,
            SEALED_DARK_CRYSTAL_GLOVES_DESIGN,
            SEALED_DARK_CRYSTAL_HELMET_DESIGN,
        ]),
    ),
    (100, Adena(4000)),
];

const DESMOND_REWARD: [(i32, Reward); 5] = [
    (31, Items(&[SEALED_MAJESTIC_BOOTS_LINING])),
    (62, Items(&[SEALED_MAJESTIC_GAUNTLETS_DESIGN])),
    (75, Items(&[SEALED_MAJESTIC_CIRCLET_DESIGN])),
    (
        83,
        Items(&[
            SEALED_MAJESTIC_BOOTS_LINING,
            SEALED_MAJESTIC_GAUNTLETS_DESIGN,
            SEALED_MAJESTIC_CIRCLET_DESIGN,
        ]),
    ),
    (100, Adena(4000)),
];

const PATRIN_REWARD: [(i32, Reward); 5] = [
    (30, Items(&[SEALED_TALLUM_BOOTS_LINING])),
    (60, Items(&[SEALED_TALLUM_GLOVES_DESIGN])),
    (80, Items(&[SEALED_TALLUM_HELM_DESIGN])),
    (
        90,
        Items(&[
            SEALED_TALLUM_BOOTS_LINING,
            SEALED_TALLUM_GLOVES_DESIGN,
            SEALED_TALLUM_HELM_DESIGN,
        ]),
    ),
    (100, Adena(4000)),
];

const CLAUDIA_REWARD: [(i32, Reward); 5] = [
    (31, Items(&[SEALED_BOOTS_OF_NIGHTMARE_LINING])),
    (62, Items(&[SEALED_GAUNTLETS_OF_NIGHTMARE_DESIGN])),
    (75, Items(&[SEALED_HELM_OF_NIGHTMARE_DESIGN])),
    (
        83,
        Items(&[
            SEALED_BOOTS_OF_NIGHTMARE_LINING,
            SEALED_GAUNTLETS_OF_NIGHTMARE_DESIGN,
            SEALED_HELM_OF_NIGHTMARE_DESIGN,
        ]),
    ),
    (100, Adena(4000)),
];

/// The blueprint trade of Walderal: all 13 blueprints for a roll of `table`, else page 07e.
fn trade_blueprints(
    ctx: &mut ScriptCtx<'_>,
    player: ObjectId,
    chance: i32,
    table: &[(i32, Reward)],
    event: &str,
) -> Option<String> {
    if ctx.has_all_quest_items(player, &BLUEPRINTS) {
        ctx.take_items_many(player, 1, &BLUEPRINTS);
        give_reward(ctx, player, chance, table);
        Some(event.to_owned())
    } else {
        Some("30844-07e.html".to_owned())
    }
}

/// Java `Q00372_LegacyOfInsolence`.
pub struct Q00372LegacyOfInsolence;

impl Script for Q00372LegacyOfInsolence {
    fn name(&self) -> &'static str {
        "Q00372_LegacyOfInsolence"
    }

    fn path(&self) -> &'static str {
        "quests/Q00372_LegacyOfInsolence"
    }

    fn id(&self) -> i32 {
        372
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[WAREHOUSE_KEEPER_WALDERAL]);
        s.add_talk_id(&[
            WAREHOUSE_KEEPER_WALDERAL,
            TRADER_HOLLY,
            MAGISTER_DESMOND,
            ANTIQUE_DEALER_PATRIN,
            CLAUDIA_ATHEBALDT,
        ]);
        let mobs: Vec<i32> = MONSTER_REWARDS.iter().map(|(id, ..)| *id).collect();
        s.add_kill_id(&mobs);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;

        // Rolled for every event, used or not.
        let chance = ctx.get_random(100);

        match event {
            "30844-04.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    return Some(event.to_owned());
                }
                None
            }
            "30844-07.html" => {
                if ctx.has_all_quest_items(player, &BLUEPRINTS) {
                    Some(event.to_owned())
                } else {
                    Some("30844-06.html".to_owned())
                }
            }
            "30844-09.html" => {
                qs.exit_quest(ctx, true, true);
                Some(event.to_owned())
            }
            "30844-07a.html" => trade_blueprints(ctx, player, chance, &BLUEPRINT_DARK_CRYSTAL, event),
            "30844-07b.html" => trade_blueprints(ctx, player, chance, &BLUEPRINT_TALLUM, event),
            "30844-07c.html" => trade_blueprints(ctx, player, chance, &BLUEPRINT_NIGHTMARE, event),
            "30844-07d.html" => trade_blueprints(ctx, player, chance, &BLUEPRINT_MAJESTIC, event),
            "30844-05b.html" => {
                qs.set_cond(ctx, 2, false);
                Some(event.to_owned())
            }
            "30844-03.htm" | "30844-05.html" | "30844-05a.html" | "30844-08.html" | "30844-10.html"
            | "30844-11.html" => Some(event.to_owned()),
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(&(_, item, item_chance, count)) = MONSTER_REWARDS.iter().find(|(id, ..)| *id == npc.id)
        else {
            return;
        };
        if npc.id == HALLATES_INSPECTOR {
            if ctx.get_random(1000) < item_chance {
                let qs = ctx.random_party_member_state(killer, -1, 3, Some(npc));
                if let Some(qs) = qs {
                    let member = qs.player();
                    ctx.give_items(member, item, count);
                    ctx.play_sound(member, QuestSound::ItemsoundQuestItemget);
                }
            }
            return;
        }

        if ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true)
            && ctx.get_random(1000) < item_chance
        {
            let mut rewarded = None;
            if !ctx.is_in_party(killer) {
                let qs = ctx.get_quest_state(killer, false);
                if qs.is_some_and(|qs| qs.is_started(ctx)) {
                    rewarded = Some(killer);
                }
            } else {
                let mut best = 0;
                for member in ctx.world().party_members_of(killer) {
                    let member_qs = ctx.get_quest_state(member, false);
                    if member_qs.is_some_and(|qs| qs.is_started(ctx)) {
                        let roll = ctx.get_random(1000);
                        if best < roll {
                            best = roll;
                            rewarded = Some(member);
                        }
                    }
                }
            }

            if let Some(rewarded) = rewarded
                && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, rewarded, true)
            {
                ctx.give_items(rewarded, item, count);
                ctx.play_sound(rewarded, QuestSound::ItemsoundQuestItemget);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        // Rolled on every talk.
        let chance = ctx.get_random(100);
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                if npc.id == WAREHOUSE_KEEPER_WALDERAL {
                    htmltext = if ctx.player_level(player) < MIN_LEVEL {
                        "30844-01.htm"
                    } else {
                        "30844-02.htm"
                    }
                    .to_owned();
                }
            }
            STARTED => match npc.id {
                WAREHOUSE_KEEPER_WALDERAL => htmltext = "30844-05.html".to_owned(),
                TRADER_HOLLY => {
                    if ctx.has_all_quest_items(player, &IMPERIAL_GENEALOGIES) {
                        ctx.take_items_many(player, 1, &IMPERIAL_GENEALOGIES);
                        give_reward(ctx, player, chance, &HOLLY_REWARD);
                        htmltext = "30839-02.html".to_owned();
                    } else {
                        htmltext = "30839-01.html".to_owned();
                    }
                }
                MAGISTER_DESMOND => {
                    if ctx.has_all_quest_items(player, &REVELATIONS) {
                        ctx.take_items_many(player, 1, &REVELATIONS);
                        give_reward(ctx, player, chance, &DESMOND_REWARD);
                        htmltext = "30855-02.html".to_owned();
                    } else {
                        htmltext = "30855-01.html".to_owned();
                    }
                }
                ANTIQUE_DEALER_PATRIN => {
                    if ctx.has_all_quest_items(player, &ANCIENT_EPIC_CHAPTERS) {
                        ctx.take_items_many(player, 1, &ANCIENT_EPIC_CHAPTERS);
                        give_reward(ctx, player, chance, &PATRIN_REWARD);
                    }
                    // NOTE(java-quirk): both branches show 30929-02, also without the books.
                    htmltext = "30929-02.html".to_owned();
                }
                CLAUDIA_ATHEBALDT => {
                    if ctx.has_all_quest_items(player, &REVELATIONS) {
                        ctx.take_items_many(player, 1, &REVELATIONS);
                        give_reward(ctx, player, chance, &CLAUDIA_REWARD);
                        htmltext = "31001-02.html".to_owned();
                    } else {
                        htmltext = "31001-01.html".to_owned();
                    }
                }
                _ => {}
            },
            _ => {}
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00372_LegacyOfInsolence";

    const NPCS: &str = r#"
        <npc id="30839" type="Folk" level="70" name="Holly"/>
        <npc id="30844" type="Folk" level="70" name="Walderal"/>
        <npc id="30855" type="Folk" level="70" name="Desmond"/>
        <npc id="30929" type="Folk" level="70" name="Patrin"/>
        <npc id="31001" type="Folk" level="70" name="Claudia"/>
        <npc id="20817" type="Monster" level="60" name="Mob 1"/>
        <npc id="20825" type="Monster" level="60" name="Hallate's Inspector"/>"#;

    /// Every item the quest touches, as a template (quest items 5966..6001, rewards).
    fn items() -> String {
        let mut out = String::from(
            r#"<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>"#,
        );
        for id in (5966..=6001).chain([
            5368, 5370, 5380, 5382, 5392, 5394, 5404, 5406, 5426, 5428, 5430, 5432, 5496, 5497, 5502, 5503,
            5508, 5509, 5514, 5515, 5525, 5526, 5527, 5528,
        ]) {
            out.push_str(&format!(
                r#"<item id="{id}" type="EtcItem" name="Item {id}"><set name="is_questitem" val="{}"/></item>"#,
                (5966..=6001).contains(&id)
            ));
        }
        out
    }

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00372LegacyOfInsolence);
        let pages = Pages::of(&Q00372LegacyOfInsolence)
            .page("30844-02.htm", &["30844-03.htm"])
            .page("30844-03.htm", &["30844-04.htm"])
            .page(
                "30844-05.html",
                &[
                    "30844-05a.html",
                    "30844-05b.html",
                    "30844-07.html",
                    "30844-08.html",
                    "30844-09.html",
                ],
            )
            .page(
                "30844-07.html",
                &[
                    "30844-07a.html",
                    "30844-07b.html",
                    "30844-07c.html",
                    "30844-07d.html",
                ],
            )
            .page(
                "30844-07a.html",
                &["30844-11.html", "30844-08.html", "30844-09.html"],
            )
            .plain(&[
                "30844-01.htm",
                "30844-04.htm",
                "30844-05b.html",
                "30844-06.html",
                "30844-07e.html",
                "30844-08.html",
                "30844-09.html",
                "30844-11.html",
                "30839-01.html",
                "30839-02.html",
                "30855-01.html",
                "30855-02.html",
                "30929-01.html",
                "30929-02.html",
                "31001-01.html",
                "31001-02.html",
            ])
            .npcs(&[
                WAREHOUSE_KEEPER_WALDERAL,
                TRADER_HOLLY,
                MAGISTER_DESMOND,
                ANTIQUE_DEALER_PATRIN,
                CLAUDIA_ATHEBALDT,
            ]);
        let mut t = ScriptTest::with_pages(registry, NPCS, &items(), pages);
        let walderal = t.spawn(WAREHOUSE_KEEPER_WALDERAL, 20);
        (t, walderal)
    }

    #[test]
    fn a_low_level_character_is_sent_away() {
        let (mut t, walderal) = setup();
        t.enter(1, 58, vec![]);
        assert!(t.quest_page(1, walderal).contains("30844-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_blueprints_are_traded_for_a_dark_crystal_piece_or_recipe() {
        let (mut t, walderal) = setup();
        let blueprints = BLUEPRINTS
            .iter()
            .enumerate()
            .map(|(i, id)| item(100 + i as i32, *id, 1))
            .collect();
        t.enter(1, 59, blueprints);
        assert!(t.quest_page(1, walderal).contains("30844-02.htm"));
        assert!(t.event(1, NAME, "30844-03.htm").contains("30844-03.htm"));
        assert!(t.event(1, NAME, "30844-04.htm").contains("30844-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, walderal).contains("30844-05.html"));
        assert!(t.event(1, NAME, "30844-05b.html").contains("30844-05b.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, walderal).contains("30844-05.html"));
        assert!(t.event(1, NAME, "30844-07.html").contains("30844-07.html"));
        assert!(t.event(1, NAME, "30844-07a.html").contains("30844-07a.html"));
        // All thirteen are gone and something from the dark crystal table came back.
        assert!(BLUEPRINTS.iter().all(|&id| t.count(1, id) == 0));
        let rewards = [
            SEALED_DARK_CRYSTAL_BOOTS_LINING,
            SEALED_DARK_CRYSTAL_GLOVES_DESIGN,
            SEALED_DARK_CRYSTAL_HELMET_DESIGN,
            RECIPE_SEALED_DARK_CRYSTAL_BOOTS_60,
            RECIPE_SEALED_DARK_CRYSTAL_GLOVES_60,
            RECIPE_SEALED_DARK_CRYSTAL_HELMET_60,
        ];
        assert!(rewards.iter().any(|&id| t.count(1, id) > 0));
        // Without the blueprints the next trade is refused.
        assert!(t.event(1, NAME, "30844-11.html").contains("30844-11.html"));
        assert!(t.quest_page(1, walderal).contains("30844-05.html"));
        assert!(t.event(1, NAME, "30844-07.html").contains("30844-06.html"));
        assert!(t.quest_page(1, walderal).contains("30844-05.html"));
        assert!(t.event(1, NAME, "30844-09.html").contains("30844-09.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }

    #[test]
    fn a_kill_drops_a_papyrus_for_a_started_character() {
        let (mut t, walderal) = setup();
        t.enter(1, 59, vec![]);
        t.quest_page(1, walderal);
        t.event(1, NAME, "30844-03.htm");
        t.event(1, NAME, "30844-04.htm");
        for _ in 0..100 {
            let mob = t.spawn(20817, 60);
            t.kill(1, mob);
            if t.count(1, ANCIENT_RED_PAPYRUS) > 0 {
                return;
            }
        }
        panic!("no papyrus after 100 kills");
    }

    #[test]
    fn an_inspector_drops_to_a_party_member() {
        let (mut t, walderal) = setup();
        t.enter(1, 59, vec![]);
        t.quest_page(1, walderal);
        t.event(1, NAME, "30844-03.htm");
        t.event(1, NAME, "30844-04.htm");
        for _ in 0..100 {
            let mob = t.spawn(HALLATES_INSPECTOR, 60);
            t.kill(1, mob);
            if t.count(1, ANCIENT_RED_PAPYRUS) > 0 {
                return;
            }
        }
        panic!("no papyrus after 100 kills");
    }
}
