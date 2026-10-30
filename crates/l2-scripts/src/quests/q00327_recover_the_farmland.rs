//! Quest 327: Recover the Farmland (Leikan and Piotur in the Dion area, the Turek Orcs, Iris, Asha and Nestle).
// Java: dist/game/data/scripts/quests/Q00327_RecoverTheFarmland/Q00327_RecoverTheFarmland.java
//
// Pattern: a collection quest over several NPCs (`docs/design/scripts.md`, "Quest patterns"):
// dog tags and medallions from the Turek Orcs (a fragment with a chance), Piotur pays for the
// tags and medallions, Iris, Asha and Nestle turn the fragments into ancient relics and the
// relics into potions, scrolls and shots. The exp and SP of the relics go through `addExpAndSp`.

use l2_util::ObjectId;
use l2_world::script::QuestSound;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const IRIS: i32 = 30034;
const ASHA: i32 = 30313;
const NESTLE: i32 = 30314;
const LEIKAN: i32 = 30382;
const PIOTUR: i32 = 30597;

// Monsters
const TUREK_ORK_WARLORD: i32 = 20495;
const TUREK_ORK_ARCHER: i32 = 20496;
const TUREK_ORK_SKIRMISHER: i32 = 20497;
const TUREK_ORK_SUPPLIER: i32 = 20498;
const TUREK_ORK_FOOTMAN: i32 = 20499;
const TUREK_ORK_SENTINEL: i32 = 20500;
const TUREK_ORK_SHAMAN: i32 = 20501;

// Items
const TUREK_DOG_TAG: i32 = 1846;
const TUREK_MEDALLION: i32 = 1847;
const LEIKANS_LETTER: i32 = 5012;
const CLAY_URN_FRAGMENT: i32 = 1848;
const BRASS_TRINKET_PIECE: i32 = 1849;
const BRONZE_MIRROR_PIECE: i32 = 1850;
const JADE_NECKLACE_BEAD: i32 = 1851;
const ANCIENT_CLAY_URN: i32 = 1852;
const ANCIENT_BRASS_TIARA: i32 = 1853;
const ANCIENT_BRONZE_MIRROR: i32 = 1854;
const ANCIENT_JADE_NECKLACE: i32 = 1855;
const QUICK_STEP_POTION: i32 = 734;
const SWIFT_ATTACK_POTION: i32 = 735;
const SCROLL_OF_ESCAPE: i32 = 736;
const SCROLL_OF_RESURRECTION: i32 = 737;
const HEALING_POTION: i32 = 1061;
const SOULSHOT_D: i32 = 1463;
const SPIRITSHOT_D: i32 = 2510;

// Misc
const MIN_LEVEL: i32 = 25;

/// Java `FRAGMENTS_REWARD_DATA`: the page of each fragment (the exp of one fragment).
const FRAGMENTS_REWARD_DATA: [(&str, i32, i64); 4] = [
    ("30034-03.html", CLAY_URN_FRAGMENT, 307),
    ("30034-04.html", BRASS_TRINKET_PIECE, 368),
    ("30034-05.html", BRONZE_MIRROR_PIECE, 368),
    ("30034-06.html", JADE_NECKLACE_BEAD, 430),
];

/// Java `FULL_REWARD_DATA`: each relic and the exp it is worth.
const FULL_REWARD_DATA: [(i32, i64); 4] = [
    (ANCIENT_CLAY_URN, 2766),
    (ANCIENT_BRASS_TIARA, 3227),
    (ANCIENT_BRONZE_MIRROR, 3227),
    (ANCIENT_JADE_NECKLACE, 3919),
];

/// Java `FRAGMENTS_DROP_PROB`: the chance (percent) of a fragment, per Turek Orc.
const FRAGMENTS_DROP_PROB: [(i32, i32); 7] = [
    (TUREK_ORK_ARCHER, 21),
    (TUREK_ORK_FOOTMAN, 19),
    (TUREK_ORK_SENTINEL, 18),
    (TUREK_ORK_SHAMAN, 22),
    (TUREK_ORK_SKIRMISHER, 21),
    (TUREK_ORK_SUPPLIER, 20),
    (TUREK_ORK_WARLORD, 26),
];

/// Java `Q00327_RecoverTheFarmland`.
pub struct Q00327RecoverTheFarmland;

/// Iris's fragment pages (`30034-03.html` .. `30034-06.html`) and her relic page (`30034-07.html`):
/// `addExpAndSp(count * exp)`, the items gone, the item sound.
fn give_fragment_exp(ctx: &mut ScriptCtx<'_>, player: ObjectId, item: i32, exp_each: i64) {
    let count = ctx.get_quest_items_count(player, item);
    ctx.add_exp_and_sp(player, count * exp_each, 0);
    ctx.take_items(player, item, -1);
    ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
}

impl Script for Q00327RecoverTheFarmland {
    fn name(&self) -> &'static str {
        "Q00327_RecoverTheFarmland"
    }

    fn path(&self) -> &'static str {
        "quests/Q00327_RecoverTheFarmland"
    }

    fn id(&self) -> i32 {
        327
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[LEIKAN, PIOTUR]);
        s.add_talk_id(&[LEIKAN, PIOTUR, IRIS, ASHA, NESTLE]);
        s.add_kill_id(&[
            TUREK_ORK_WARLORD,
            TUREK_ORK_ARCHER,
            TUREK_ORK_SKIRMISHER,
            TUREK_ORK_SUPPLIER,
            TUREK_ORK_FOOTMAN,
            TUREK_ORK_SENTINEL,
            TUREK_ORK_SHAMAN,
        ]);
        s.register_quest_items(&[TUREK_DOG_TAG, TUREK_MEDALLION, LEIKANS_LETTER]);
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

        let mut html: Option<String> = None;
        match event {
            "30034-01.html" | "30313-01.html" | "30314-02.html" | "30314-08.html" | "30314-09.html"
            | "30382-05a.html" | "30382-05b.html" | "30597-03.html" | "30597-07.html" => {
                html = Some(event.to_owned());
            }
            "30382-03.htm" => {
                qs.start_quest(ctx);
                ctx.give_items(player, LEIKANS_LETTER, 1);
                qs.set_cond(ctx, 2, false);
                html = Some(event.to_owned());
            }
            "30597-03.htm" => {
                qs.start_quest(ctx);
                html = Some(event.to_owned());
            }
            "30597-06.html" => {
                qs.exit_quest(ctx, true, true);
                html = Some(event.to_owned());
            }
            "30034-03.html" | "30034-04.html" | "30034-05.html" | "30034-06.html" => {
                let (_, item, exp_each) = FRAGMENTS_REWARD_DATA
                    .iter()
                    .copied()
                    .find(|(page, _, _)| *page == event)
                    .unwrap_or_default();
                if !ctx.has_quest_items(player, item) {
                    html = Some("30034-02.html".to_owned());
                } else {
                    give_fragment_exp(ctx, player, item, exp_each);
                    html = Some(event.to_owned());
                }
            }
            "30034-07.html" => {
                let mut rewarded = false;
                for (item, exp_each) in FULL_REWARD_DATA {
                    if ctx.has_quest_items(player, item) {
                        give_fragment_exp(ctx, player, item, exp_each);
                        rewarded = true;
                    }
                }
                html = Some(if rewarded { event } else { "30034-02.html" }.to_owned());
            }
            "30313-03.html" => {
                if ctx.get_quest_items_count(player, CLAY_URN_FRAGMENT) < 5 {
                    html = Some("30313-02.html".to_owned());
                } else {
                    ctx.take_items(player, CLAY_URN_FRAGMENT, 5);
                    if ctx.get_random(6) < 5 {
                        ctx.give_items(player, ANCIENT_CLAY_URN, 1);
                        html = Some(event.to_owned());
                    } else {
                        html = Some("30313-10.html".to_owned());
                    }
                }
            }
            "30313-05.html" => {
                if ctx.get_quest_items_count(player, BRASS_TRINKET_PIECE) < 5 {
                    html = Some("30313-04.html".to_owned());
                } else {
                    ctx.take_items(player, BRASS_TRINKET_PIECE, 5);
                    if ctx.get_random(7) < 6 {
                        ctx.give_items(player, ANCIENT_BRASS_TIARA, 1);
                        html = Some(event.to_owned());
                    } else {
                        html = Some("30313-10.html".to_owned());
                    }
                }
            }
            "30313-07.html" => {
                if ctx.get_quest_items_count(player, BRONZE_MIRROR_PIECE) < 5 {
                    html = Some("30313-06.html".to_owned());
                } else {
                    ctx.take_items(player, BRONZE_MIRROR_PIECE, 5);
                    if ctx.get_random(7) < 6 {
                        ctx.give_items(player, ANCIENT_BRONZE_MIRROR, 1);
                        html = Some(event.to_owned());
                    } else {
                        html = Some("30313-10.html".to_owned());
                    }
                }
            }
            "30313-09.html" => {
                if ctx.get_quest_items_count(player, JADE_NECKLACE_BEAD) < 5 {
                    html = Some("30313-08.html".to_owned());
                } else {
                    ctx.take_items(player, JADE_NECKLACE_BEAD, 5);
                    if ctx.get_random(8) < 7 {
                        ctx.give_items(player, ANCIENT_JADE_NECKLACE, 1);
                        html = Some(event.to_owned());
                    } else {
                        html = Some("30313-10.html".to_owned());
                    }
                }
            }
            "30314-03.html" => {
                if !ctx.has_quest_items(player, ANCIENT_CLAY_URN) {
                    html = Some("30314-07.html".to_owned());
                } else {
                    let amount = i64::from(ctx.get_random_range(70, 110));
                    ctx.reward_items(player, SOULSHOT_D, amount);
                    ctx.take_items(player, ANCIENT_CLAY_URN, 1);
                    html = Some(event.to_owned());
                }
            }
            "30314-04.html" => {
                if !ctx.has_quest_items(player, ANCIENT_BRASS_TIARA) {
                    html = Some("30314-07.html".to_owned());
                } else {
                    let rnd = ctx.get_random(100);
                    if rnd < 40 {
                        ctx.reward_items(player, HEALING_POTION, 1);
                    } else if rnd < 84 {
                        ctx.reward_items(player, QUICK_STEP_POTION, 1);
                    } else {
                        ctx.reward_items(player, SWIFT_ATTACK_POTION, 1);
                    }
                    ctx.take_items(player, ANCIENT_BRASS_TIARA, 1);
                    html = Some(event.to_owned());
                }
            }
            "30314-05.html" => {
                if !ctx.has_quest_items(player, ANCIENT_BRONZE_MIRROR) {
                    html = Some("30314-07.html".to_owned());
                } else {
                    let item = if ctx.get_random(100) < 59 {
                        SCROLL_OF_ESCAPE
                    } else {
                        SCROLL_OF_RESURRECTION
                    };
                    ctx.reward_items(player, item, 1);
                    ctx.take_items(player, ANCIENT_BRONZE_MIRROR, 1);
                    html = Some(event.to_owned());
                }
            }
            "30314-06.html" => {
                if !ctx.has_quest_items(player, ANCIENT_JADE_NECKLACE) {
                    html = Some("30314-07.html".to_owned());
                } else {
                    let amount = i64::from(ctx.get_random_range(50, 90));
                    ctx.reward_items(player, SPIRITSHOT_D, amount);
                    ctx.take_items(player, ANCIENT_JADE_NECKLACE, 1);
                    html = Some(event.to_owned());
                }
            }
            _ => {}
        }
        html
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if ctx.get_quest_state(killer, false).is_none() {
            return;
        }
        if npc.id == TUREK_ORK_SHAMAN || npc.id == TUREK_ORK_WARLORD {
            ctx.give_items(killer, TUREK_MEDALLION, 1);
        } else {
            ctx.give_items(killer, TUREK_DOG_TAG, 1);
        }

        // Java `FRAGMENTS_DROP_PROB.get(npc.getId())`: the registered Turek Orcs only.
        let chance = FRAGMENTS_DROP_PROB
            .iter()
            .find(|(id, _)| *id == npc.id)
            .map_or(0, |(_, chance)| *chance);
        if ctx.get_random(100) < chance {
            let fragment = ctx.get_random_range(CLAY_URN_FRAGMENT, JADE_NECKLACE_BEAD);
            ctx.give_items(killer, fragment, 1);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut html = ctx.get_no_quest_msg(player);

        match npc.id {
            LEIKAN => {
                if qs.is_created(ctx) {
                    html = if ctx.player_level(player) >= MIN_LEVEL {
                        "30382-02.htm"
                    } else {
                        "30382-01.htm"
                    }
                    .to_owned();
                } else if qs.is_started(ctx) {
                    if ctx.has_quest_items(player, LEIKANS_LETTER) {
                        html = "30382-04.html".to_owned();
                    } else {
                        html = "30382-05.html".to_owned();
                        qs.set_cond(ctx, 5, true);
                    }
                }
            }
            PIOTUR => {
                if qs.is_created(ctx) {
                    html = if ctx.player_level(player) >= MIN_LEVEL {
                        "30597-02.htm"
                    } else {
                        "30597-01.htm"
                    }
                    .to_owned();
                } else if qs.is_started(ctx) {
                    if ctx.has_quest_items(player, LEIKANS_LETTER) {
                        html = "30597-03a.htm".to_owned();
                        ctx.take_items(player, LEIKANS_LETTER, -1);
                        qs.set_cond(ctx, 3, true);
                    } else if !ctx.has_quest_items(player, TUREK_DOG_TAG)
                        && !ctx.has_quest_items(player, TUREK_MEDALLION)
                    {
                        html = "30597-04.html".to_owned();
                    } else {
                        html = "30597-05.html".to_owned();
                        let dog_tags = ctx.get_quest_items_count(player, TUREK_DOG_TAG);
                        let medallions = ctx.get_quest_items_count(player, TUREK_MEDALLION);
                        let bonus = if dog_tags + medallions >= 10 { 619 } else { 0 };
                        let reward_count = dog_tags * 40 + medallions * 50 + bonus;
                        ctx.give_adena(player, reward_count, true);
                        ctx.take_items(player, TUREK_DOG_TAG, -1);
                        ctx.take_items(player, TUREK_MEDALLION, -1);
                        qs.set_cond(ctx, 4, true);
                    }
                }
            }
            IRIS => {
                if qs.is_started(ctx) {
                    html = "30034-01.html".to_owned();
                }
            }
            ASHA => {
                if qs.is_started(ctx) {
                    html = "30313-01.html".to_owned();
                }
            }
            NESTLE => {
                if qs.is_started(ctx) {
                    html = "30314-01.html".to_owned();
                }
            }
            _ => {}
        }
        Some(html)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::{player_class, race};

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, names_to, pages_to, rows};

    const NAME: &str = "Q00327_RecoverTheFarmland";

    const NPCS: &str = r#"
        <npc id="30034" type="Folk" level="1" name="Iris"/>
        <npc id="30313" type="Folk" level="1" name="Asha"/>
        <npc id="30314" type="Folk" level="1" name="Nestle"/>
        <npc id="30382" type="Folk" level="1" name="Leikan"/>
        <npc id="30597" type="Folk" level="1" name="Piotur"/>
        <npc id="20496" type="Monster" level="30" name="Turek Orc Archer"/>
        <npc id="20501" type="Monster" level="30" name="Turek Orc Shaman"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1846" type="EtcItem" name="Turek Dog Tag"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1847" type="EtcItem" name="Turek Medallion"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="5012" type="EtcItem" name="Leikan's Letter"><set name="is_questitem" val="true"/></item>
        <item id="1848" type="EtcItem" name="Clay Urn Fragment"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1849" type="EtcItem" name="Brass Trinket Piece"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1852" type="EtcItem" name="Ancient Clay Urn"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1853" type="EtcItem" name="Ancient Brass Tiara"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1463" type="EtcItem" name="Soulshot D"><set name="is_stackable" val="true"/></item>
        <item id="734" type="EtcItem" name="Quick Step Potion"><set name="is_stackable" val="true"/></item>
        <item id="735" type="EtcItem" name="Swift Attack Potion"><set name="is_stackable" val="true"/></item>
        <item id="1061" type="EtcItem" name="Healing Potion"><set name="is_stackable" val="true"/></item>"#;

    struct Npcs {
        leikan: Oid,
        piotur: Oid,
        iris: Oid,
        asha: Oid,
        nestle: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00327RecoverTheFarmland);
        let pages = Pages::of(&Q00327RecoverTheFarmland)
            .page("30382-02.htm", &["30382-03.htm"])
            .plain(&[
                "30382-01.htm",
                "30382-03.htm",
                "30382-04.html",
                "30382-05a.html",
                "30382-05b.html",
            ])
            .page("30382-05.html", &["30382-05a.html"])
            .page("30597-02.htm", &["30597-03.htm"])
            .page("30597-03a.htm", &["30597-03.html"])
            .plain(&["30597-01.htm", "30597-03.htm", "30597-03.html"])
            .page("30597-05.html", &["30597-06.html", "30597-07.html"])
            .plain(&["30597-04.html", "30597-06.html", "30597-07.html"])
            .page(
                "30034-01.html",
                &[
                    "30034-03.html",
                    "30034-04.html",
                    "30034-05.html",
                    "30034-06.html",
                    "30034-07.html",
                ],
            )
            .page("30034-02.html", &["30034-01.html"])
            .page("30034-03.html", &["30034-01.html"])
            .page("30034-07.html", &["30034-01.html"])
            .page(
                "30313-01.html",
                &["30313-03.html", "30313-05.html", "30313-07.html", "30313-09.html"],
            )
            .plain(&["30313-02.html", "30313-03.html", "30313-10.html"])
            .page("30314-01.html", &["30314-02.html"])
            .page(
                "30314-02.html",
                &["30314-03.html", "30314-04.html", "30314-05.html", "30314-06.html"],
            )
            .page("30314-03.html", &["30314-08.html"])
            .page(
                "30314-08.html",
                &["30314-03.html", "30314-04.html", "30314-09.html"],
            )
            .plain(&["30314-07.html", "30314-09.html"])
            .npcs(&[LEIKAN, PIOTUR, IRIS, ASHA, NESTLE]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            leikan: t.spawn(LEIKAN, 20),
            piotur: t.spawn(PIOTUR, 40),
            iris: t.spawn(IRIS, 60),
            asha: t.spawn(ASHA, 80),
            nestle: t.spawn(NESTLE, 100),
        };
        (t, npcs)
    }

    /// Starts the quest from Piotur's side (`30597-03.htm`), which hands out no letter.
    fn start_from_piotur(t: &mut ScriptTest, npcs: &Npcs, player: i32) {
        assert!(t.quest_page(player, npcs.piotur).contains("30597-02.htm"));
        assert!(t.event(player, NAME, "30597-03.htm").contains("30597-03.htm"));
        t.take_writes();
    }

    /// Kills Turek Orc Archers until `count` dog tags are in the inventory.
    fn hunt_tags(t: &mut ScriptTest, count: i64) {
        for _ in 0..400 {
            if t.count(1, TUREK_DOG_TAG) >= count {
                return;
            }
            let archer = t.spawn(TUREK_ORK_ARCHER, 60);
            t.kill(1, archer);
        }
        panic!("no {count} tags after 400 kills");
    }

    #[test]
    fn the_start_from_leikan_hands_out_the_letter() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 24, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, npcs.leikan).contains("30382-01.htm"));
        assert!(t.take_writes().is_empty());

        t.enter_as(2, 25, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(2, npcs.leikan).contains("30382-02.htm"));
        assert!(t.event(2, NAME, "30382-03.htm").contains("30382-03.htm"));
        assert_eq!(t.count(2, LEIKANS_LETTER), 1);
        // The start, then `setCond(2)` (no sound) for the letter.
        assert_eq!(
            t.take_writes(),
            [
                rows::of(2, rows::insert(NAME, "cond", "1")),
                rows::of(2, rows::insert(NAME, "<state>", "Started")),
                rows::of(2, rows::update(NAME, "cond", "2")),
            ]
        );
    }

    #[test]
    fn the_tags_and_the_medallion_then_piotur_pays() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 25, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, npcs.leikan).contains("30382-02.htm"));
        assert!(t.event(1, NAME, "30382-03.htm").contains("30382-03.htm"));
        t.take_writes();

        // Piotur takes the letter first: cond 3, and the page with the orders.
        assert!(t.quest_page(1, npcs.piotur).contains("30597-03a.htm"));
        assert_eq!(t.count(1, LEIKANS_LETTER), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // Ten dog tags, then a medallion from a shaman: the reward with the bonus (11 items).
        hunt_tags(&mut t, 10);
        let shaman = t.spawn(TUREK_ORK_SHAMAN, 60);
        t.kill(1, shaman);
        let tags = t.count(1, TUREK_DOG_TAG);
        assert_eq!(t.count(1, TUREK_MEDALLION), 1);
        assert!(t.take_writes().is_empty());

        // The reward: 40 per tag, 50 per medallion and the bonus of 619; cond 4.
        assert!(t.quest_page(1, npcs.piotur).contains("30597-05.html"));
        assert_eq!(t.count(1, 57), tags * 40 + 50 + 619);
        assert_eq!((t.count(1, TUREK_DOG_TAG), t.count(1, TUREK_MEDALLION)), (0, 0));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);

        // The end: the quest ends repeatable with the finish sound.
        t.bypass(1, &format!("Script {NAME} 30597-06.html"));
        let out = t.take();
        assert!(pages_to(&out, 1).pop().unwrap().contains("30597-06.html"));
        assert!(names_to(&out, 1).contains(&"PLAY_SOUND".to_owned()));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }

    #[test]
    fn asha_makes_the_urn_from_five_fragments() {
        let (mut t, npcs) = setup();
        t.enter_as(
            1,
            30,
            race::HUMAN,
            player_class::FIGHTER,
            vec![item(900, 1848, 4)],
        );
        t.enter_as(
            2,
            30,
            race::HUMAN,
            player_class::FIGHTER,
            vec![item(901, 1848, 5)],
        );
        start_from_piotur(&mut t, &npcs, 1);
        start_from_piotur(&mut t, &npcs, 2);

        // Four fragments are not enough: Asha says so and keeps them.
        assert!(t.quest_page(1, npcs.asha).contains("30313-01.html"));
        assert!(t.event(1, NAME, "30313-03.html").contains("30313-02.html"));
        assert_eq!(t.count(1, CLAY_URN_FRAGMENT), 4);

        // Five: the fragments go and the roll of 6 decides (5 in 6 make the urn).
        assert!(t.quest_page(2, npcs.asha).contains("30313-01.html"));
        let page = t.event(2, NAME, "30313-03.html");
        assert!(
            page.contains("30313-03.html") || page.contains("30313-10.html"),
            "{page}"
        );
        assert_eq!(t.count(2, CLAY_URN_FRAGMENT), 0);
        assert_eq!(
            t.count(2, ANCIENT_CLAY_URN),
            i64::from(page.contains("30313-03.html"))
        );
    }

    #[test]
    fn iris_pays_for_the_fragments_and_the_relics() {
        let (mut t, npcs) = setup();
        t.enter_as(
            1,
            30,
            race::HUMAN,
            player_class::FIGHTER,
            vec![item(900, 1848, 2), item(901, 1852, 1)],
        );
        start_from_piotur(&mut t, &npcs, 1);

        // No tiara yet is no reason to pay: Iris asks for the item first (02).
        assert!(t.quest_page(1, npcs.iris).contains("30034-01.html"));
        assert!(t.event(1, NAME, "30034-04.html").contains("30034-02.html"));
        assert_eq!(t.count(1, BRASS_TRINKET_PIECE), 0);

        // Two clay fragments: the exp of them is paid and the fragments go.
        assert!(t.quest_page(1, npcs.iris).contains("30034-01.html"));
        assert!(t.event(1, NAME, "30034-03.html").contains("30034-03.html"));
        assert_eq!(t.count(1, CLAY_URN_FRAGMENT), 0);

        // The relic (the urn) is paid as a whole by 07.
        assert!(t.quest_page(1, npcs.iris).contains("30034-01.html"));
        assert!(t.event(1, NAME, "30034-07.html").contains("30034-07.html"));
        assert_eq!(t.count(1, ANCIENT_CLAY_URN), 0);
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn nestle_turns_the_urn_into_shots_and_needs_the_relic() {
        let (mut t, npcs) = setup();
        t.enter_as(
            1,
            30,
            race::HUMAN,
            player_class::FIGHTER,
            vec![item(900, 1852, 1)],
        );
        start_from_piotur(&mut t, &npcs, 1);

        // Without the tiara Nestle sends the player back (07).
        assert!(t.quest_page(1, npcs.nestle).contains("30314-01.html"));
        assert!(t.event(1, NAME, "30314-02.html").contains("30314-02.html"));
        assert!(t.event(1, NAME, "30314-04.html").contains("30314-07.html"));

        // The urn: 70 to 110 soulshots (D), the urn goes.
        assert!(t.quest_page(1, npcs.nestle).contains("30314-01.html"));
        assert!(t.event(1, NAME, "30314-02.html").contains("30314-02.html"));
        assert!(t.event(1, NAME, "30314-03.html").contains("30314-03.html"));
        let shots = t.count(1, SOULSHOT_D);
        assert!((70..=110).contains(&shots), "{shots}");
        assert_eq!(t.count(1, ANCIENT_CLAY_URN), 0);
        assert!(t.take_writes().is_empty());
    }
}
