//! Quest 611: Alliance with Varka Silenos (Naran Ashanuk in the Varka Silenos Village, Ketra badges).
// Java: dist/game/data/scripts/quests/Q00611_AllianceWithVarkaSilenos/Q00611_AllianceWithVarkaSilenos.java
//
// Pattern: a five-step alliance. Each step trades badges (soldier, officer, captain) of the
// Varka monsters for a Ketra mark; the badges are dropped by a random party member in the
// started state, only as many as the current step asks for (`canGetItem`).

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const NARAN_ASHANUK: i32 = 31378;

// Monsters: (id, drop chance out of 1000, minimum cond, badge item). Java's `MOBS`.
const MOBS: [(i32, i32, i32); 21] = [
    (21324, 500, 1), // Ketra Orc Footman
    (21325, 500, 1), // Ketra's War Hound
    (21327, 509, 1), // Ketra Orc Raider
    (21328, 521, 1), // Ketra Orc Scout
    (21329, 519, 1), // Ketra Orc Shaman
    (21331, 500, 2), // Ketra Orc Warrior
    (21332, 500, 2), // Ketra Orc Lieutenant
    (21334, 509, 2), // Ketra Orc Medium
    (21335, 518, 2), // Ketra Orc Elite Soldier
    (21336, 518, 2), // Ketra Orc White Captain
    (21338, 527, 2), // Ketra Orc Seer
    (21339, 500, 3), // Ketra Orc General
    (21340, 500, 3), // Ketra Orc Battalion Commander
    (21342, 508, 3), // Ketra Orc Grand Seer
    (21343, 628, 2), // Ketra Commander
    (21344, 604, 2), // Ketra Elite Guard
    (21345, 627, 3), // Ketra's Head Shaman
    (21346, 604, 3), // Ketra's Head Guard
    (21347, 649, 3), // Ketra Prophet
    (21348, 626, 3), // Prophet's Guard
    (21349, 626, 3), // Prophet's Aide
];

// Items
const KETRA_BADGE_SOLDIER: i32 = 7226;
const KETRA_BADGE_OFFICER: i32 = 7227;
const KETRA_BADGE_CAPTAIN: i32 = 7228;
const VALOR_FEATHER: i32 = 7229;
const WISDOM_FEATHER: i32 = 7230;
const KETRA_MARKS: [i32; 5] = [7211, 7212, 7213, 7214, 7215];
const VARKA_MARKS: [i32; 5] = [7221, 7222, 7223, 7224, 7225];

// Misc
const MIN_LEVEL: i32 = 74;
/// Java `SOLDIER_BADGE_COUNT`, `OFFICER_BADGE_COUNT`, `CAPTAIN_BADGE_COUNT`, indexed by cond - 1.
const SOLDIER_BADGE_COUNT: [i64; 5] = [100, 200, 300, 300, 400];
const OFFICER_BADGE_COUNT: [i64; 5] = [0, 100, 200, 300, 400];
const CAPTAIN_BADGE_COUNT: [i64; 5] = [0, 0, 100, 200, 200];

/// Java `DropInfo.getItemId()`: the badge of the monster's minimum cond.
fn badge_of(min_cond: i32) -> i32 {
    match min_cond {
        1 => KETRA_BADGE_SOLDIER,
        2 => KETRA_BADGE_OFFICER,
        _ => KETRA_BADGE_CAPTAIN,
    }
}

/// Java `canGetItem(qs, itemId)`: the player holds fewer badges of that kind than the cond asks.
fn can_get_item(ctx: &ScriptCtx<'_>, player: ObjectId, cond: i32, item_id: i32) -> bool {
    let index = (cond - 1) as usize;
    let count = match item_id {
        KETRA_BADGE_SOLDIER => SOLDIER_BADGE_COUNT[index],
        KETRA_BADGE_OFFICER => OFFICER_BADGE_COUNT[index],
        KETRA_BADGE_CAPTAIN => CAPTAIN_BADGE_COUNT[index],
        _ => 0,
    };
    ctx.get_quest_items_count(player, item_id) < count
}

/// Java `Q00611_AllianceWithVarkaSilenos`.
pub struct Q00611AllianceWithVarkaSilenos;

impl Script for Q00611AllianceWithVarkaSilenos {
    fn name(&self) -> &'static str {
        "Q00611_AllianceWithVarkaSilenos"
    }

    fn path(&self) -> &'static str {
        "quests/Q00611_AllianceWithVarkaSilenos"
    }

    fn id(&self) -> i32 {
        611
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[NARAN_ASHANUK]);
        s.add_talk_id(&[NARAN_ASHANUK]);
        let mobs: Vec<i32> = MOBS.iter().map(|(id, _, _)| *id).collect();
        s.add_kill_id(&mobs);
        s.register_quest_items(&[KETRA_BADGE_SOLDIER, KETRA_BADGE_OFFICER, KETRA_BADGE_CAPTAIN]);
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
        let mut htmltext = Some(event.to_owned());
        match event {
            "31378-12a.html" | "31378-12b.html" | "31378-25.html" => {}
            "31378-04.htm" => {
                if ctx.has_at_least_one_quest_item(player, &KETRA_MARKS) {
                    return Some("31378-03.htm".to_owned());
                }
                qs.set_state(ctx, STARTED);
                ctx.play_sound(player, QuestSound::ItemsoundQuestAccept);
                for (i, mark) in VARKA_MARKS.iter().enumerate() {
                    if ctx.has_quest_items(player, *mark) {
                        qs.set_cond(ctx, i as i32 + 2, false);
                        return Some(format!("31378-0{}.htm", i + 5));
                    }
                }
                qs.set_cond(ctx, 1, false);
            }
            "31378-12.html" => {
                if ctx.get_quest_items_count(player, KETRA_BADGE_SOLDIER) < SOLDIER_BADGE_COUNT[0] {
                    return Some(ctx.get_no_quest_msg(player));
                }
                ctx.take_items(player, KETRA_BADGE_SOLDIER, -1);
                ctx.give_items(player, VARKA_MARKS[0], 1);
                qs.set_cond(ctx, 2, true);
            }
            "31378-15.html" => {
                if ctx.get_quest_items_count(player, KETRA_BADGE_SOLDIER) < SOLDIER_BADGE_COUNT[1]
                    || ctx.get_quest_items_count(player, KETRA_BADGE_OFFICER) < OFFICER_BADGE_COUNT[1]
                {
                    return Some(ctx.get_no_quest_msg(player));
                }
                ctx.take_items_many(
                    player,
                    -1,
                    &[KETRA_BADGE_SOLDIER, KETRA_BADGE_OFFICER, VARKA_MARKS[0]],
                );
                ctx.give_items(player, VARKA_MARKS[1], 1);
                qs.set_cond(ctx, 3, true);
            }
            "31378-18.html" => {
                if ctx.get_quest_items_count(player, KETRA_BADGE_SOLDIER) < SOLDIER_BADGE_COUNT[2]
                    || ctx.get_quest_items_count(player, KETRA_BADGE_OFFICER) < OFFICER_BADGE_COUNT[2]
                    || ctx.get_quest_items_count(player, KETRA_BADGE_CAPTAIN) < CAPTAIN_BADGE_COUNT[2]
                {
                    return Some(ctx.get_no_quest_msg(player));
                }
                ctx.take_items_many(
                    player,
                    -1,
                    &[
                        KETRA_BADGE_SOLDIER,
                        KETRA_BADGE_OFFICER,
                        KETRA_BADGE_CAPTAIN,
                        VARKA_MARKS[1],
                    ],
                );
                ctx.give_items(player, VARKA_MARKS[2], 1);
                qs.set_cond(ctx, 4, true);
            }
            "31378-21.html" => {
                if !ctx.has_quest_items(player, VALOR_FEATHER)
                    || ctx.get_quest_items_count(player, KETRA_BADGE_SOLDIER) < SOLDIER_BADGE_COUNT[3]
                    || ctx.get_quest_items_count(player, KETRA_BADGE_OFFICER) < OFFICER_BADGE_COUNT[3]
                    || ctx.get_quest_items_count(player, KETRA_BADGE_CAPTAIN) < CAPTAIN_BADGE_COUNT[3]
                {
                    return Some(ctx.get_no_quest_msg(player));
                }
                ctx.take_items_many(
                    player,
                    -1,
                    &[
                        KETRA_BADGE_SOLDIER,
                        KETRA_BADGE_OFFICER,
                        KETRA_BADGE_CAPTAIN,
                        VALOR_FEATHER,
                        VARKA_MARKS[2],
                    ],
                );
                ctx.give_items(player, VARKA_MARKS[3], 1);
                qs.set_cond(ctx, 5, true);
            }
            "31378-26.html" => {
                ctx.take_items_many(player, -1, &VARKA_MARKS);
                ctx.take_items_many(player, -1, &[VALOR_FEATHER, WISDOM_FEATHER]);
                qs.exit_quest(ctx, true, true);
            }
            _ => htmltext = None,
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(member) = ctx.get_random_party_member_state(killer, STARTED) else {
            return;
        };
        let Some(qs) = ctx.get_quest_state(member, false) else {
            return;
        };
        let Some(&(_, chance, min_cond)) = MOBS.iter().find(|(id, _, _)| *id == npc.id) else {
            return;
        };
        let item = badge_of(min_cond);
        let cond = qs.get_cond(ctx);
        if cond >= min_cond
            && cond < 6
            && can_get_item(ctx, member, cond, item)
            && ctx.get_random(1000) < chance
        {
            ctx.give_items(member, item, 1);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let count = |ctx: &ScriptCtx<'_>, id: i32| ctx.get_quest_items_count(player, id);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    "31378-01.htm"
                } else {
                    "31378-02.htm"
                }
                .to_owned();
            }
            STARTED => match qs.get_cond(ctx) {
                1 => {
                    htmltext = if count(ctx, KETRA_BADGE_SOLDIER) >= SOLDIER_BADGE_COUNT[0] {
                        "31378-11.html"
                    } else {
                        "31378-10.html"
                    }
                    .to_owned();
                }
                2 => {
                    htmltext = if ctx.has_quest_items(player, VARKA_MARKS[0])
                        && count(ctx, KETRA_BADGE_SOLDIER) >= SOLDIER_BADGE_COUNT[1]
                        && count(ctx, KETRA_BADGE_OFFICER) >= OFFICER_BADGE_COUNT[1]
                    {
                        "31378-14.html"
                    } else {
                        "31378-13.html"
                    }
                    .to_owned();
                }
                3 => {
                    htmltext = if ctx.has_quest_items(player, VARKA_MARKS[1])
                        && count(ctx, KETRA_BADGE_SOLDIER) >= SOLDIER_BADGE_COUNT[2]
                        && count(ctx, KETRA_BADGE_OFFICER) >= OFFICER_BADGE_COUNT[2]
                        && count(ctx, KETRA_BADGE_CAPTAIN) >= CAPTAIN_BADGE_COUNT[2]
                    {
                        "31378-17.html"
                    } else {
                        "31378-16.html"
                    }
                    .to_owned();
                }
                4 => {
                    htmltext = if ctx.has_all_quest_items(player, &[VARKA_MARKS[2], VALOR_FEATHER])
                        && count(ctx, KETRA_BADGE_SOLDIER) >= SOLDIER_BADGE_COUNT[3]
                        && count(ctx, KETRA_BADGE_OFFICER) >= OFFICER_BADGE_COUNT[3]
                        && count(ctx, KETRA_BADGE_CAPTAIN) >= CAPTAIN_BADGE_COUNT[3]
                    {
                        "31378-20.html"
                    } else {
                        "31378-19.html"
                    }
                    .to_owned();
                }
                5 => {
                    if !ctx.has_quest_items(player, VARKA_MARKS[3])
                        || !ctx.has_quest_items(player, WISDOM_FEATHER)
                        || count(ctx, KETRA_BADGE_SOLDIER) < SOLDIER_BADGE_COUNT[4]
                        || count(ctx, KETRA_BADGE_OFFICER) < OFFICER_BADGE_COUNT[4]
                        || count(ctx, KETRA_BADGE_CAPTAIN) < CAPTAIN_BADGE_COUNT[4]
                    {
                        return Some("31378-22.html".to_owned());
                    }
                    qs.set_cond(ctx, 6, true);
                    ctx.take_items_many(
                        player,
                        -1,
                        &[
                            KETRA_BADGE_SOLDIER,
                            KETRA_BADGE_OFFICER,
                            KETRA_BADGE_CAPTAIN,
                            WISDOM_FEATHER,
                            VARKA_MARKS[3],
                        ],
                    );
                    ctx.give_items(player, VARKA_MARKS[4], 1);
                    htmltext = "31378-23.html".to_owned();
                }
                6 => {
                    if ctx.has_quest_items(player, VARKA_MARKS[4]) {
                        htmltext = "31378-24.html".to_owned();
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

    const NAME: &str = "Q00611_AllianceWithVarkaSilenos";

    const NPCS: &str = r#"
        <npc id="31378" type="Folk" level="70" name="Wahkan"/>
        <npc id="21324" type="Monster" level="20" name="Ketra Orc Footman"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7226" type="EtcItem" name="Ketra Badge - Soldier"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="7227" type="EtcItem" name="Ketra Badge - Officer"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="7221" type="EtcItem" name="Mark of Varka's Alliance - Level 1"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00611AllianceWithVarkaSilenos);
        let pages = Pages::of(&Q00611AllianceWithVarkaSilenos)
            .page("31378-01.htm", &["31378-04.htm"])
            .plain(&[
                "31378-02.htm",
                "31378-03.htm",
                "31378-04.htm",
                "31378-10.html",
                "31378-13.html",
            ])
            .page("31378-11.html", &["31378-12.html"])
            .plain(&["31378-12.html", "31378-14.html", "31378-05.htm"])
            .npcs(&[NARAN_ASHANUK]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let wahkan = t.spawn(NARAN_ASHANUK, 20);
        (t, wahkan)
    }

    #[test]
    fn a_character_below_level_74_is_sent_away() {
        let (mut t, wahkan) = setup();
        t.enter(1, 73, vec![]);
        assert!(t.quest_page(1, wahkan).contains("31378-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_alliance_starts_at_cond_one() {
        let (mut t, wahkan) = setup();
        t.enter(1, 74, vec![]);
        assert!(t.quest_page(1, wahkan).contains("31378-01.htm"));
        assert!(t.event(1, NAME, "31378-04.htm").contains("31378-04.htm"));
        assert!(t.take_writes().contains(&rows::insert(NAME, "cond", "1")));
        assert!(t.quest_page(1, wahkan).contains("31378-10.html"));
    }

    #[test]
    fn a_hundred_soldier_badges_make_the_first_trade() {
        let (mut t, wahkan) = setup();
        t.enter(1, 74, vec![item(100, KETRA_BADGE_SOLDIER, 99)]);
        t.quest_page(1, wahkan);
        t.event(1, NAME, "31378-04.htm");
        t.take_writes();
        t.take();
        // 99 badges: the NPC still asks for more.
        assert!(t.quest_page(1, wahkan).contains("31378-10.html"));
        // The Ketra Orc Footman drops a soldier badge at cond 1 (50%).
        for _ in 0..400 {
            if t.count(1, KETRA_BADGE_SOLDIER) >= 100 {
                break;
            }
            let mob = t.spawn(21324, 60);
            t.kill(1, mob);
        }
        assert_eq!(t.count(1, KETRA_BADGE_SOLDIER), 100);
        assert!(t.quest_page(1, wahkan).contains("31378-11.html"));
        assert!(t.event(1, NAME, "31378-12.html").contains("31378-12.html"));
        assert_eq!(
            (t.count(1, KETRA_BADGE_SOLDIER), t.count(1, VARKA_MARKS[0])),
            (0, 1)
        );
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "2")));
    }
}
