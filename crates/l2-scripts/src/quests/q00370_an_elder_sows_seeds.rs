//! Quest 370: An Elder Sows Seeds (Casian in the Town of Gludio, the ant monsters, spellbook pages).
// Java: dist/game/data/scripts/quests/Q00370_AnElderSowsSeeds/Q00370_AnElderSowsSeeds.java
//
// Pattern: a repeatable party quest with drops (`docs/design/scripts.md`, "Quest patterns"): the
// spellbook pages drop from the ants (a random started member is picked for the ones with a
// per-kill roll, the killer's party for the others), and the four chapters are exchanged for adena
// (`REWARD`), the rest is paid on quitting. The Java constructor registers no quest items.

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const CASIAN: i32 = 30612;

// Items
const SPELLBOOK_PAGE: i32 = 5916;
const CHAPTER_OF_FIRE: i32 = 5917;
const CHAPTER_OF_WATER: i32 = 5918;
const CHAPTER_OF_WIND: i32 = 5919;
const CHAPTER_OF_EARTH: i32 = 5920;

// Misc
const MIN_LEVEL: i32 = 28;

// Mobs (chance in percent, a roll for a random started party member)
const MOBS1: [(i32, i32); 3] = [
    (20082, 9),  // ant_recruit
    (20086, 9),  // ant_guard
    (20090, 22), // noble_ant_leader
];

// Mobs (drop chance)
const MOBS2: [(i32, f64); 2] = [
    (20084, 0.101), // ant_patrol
    (20089, 0.100), // noble_ant
];

/// Java `exchangeChapters(player, takeAllItems)`: the smallest count of the four chapters is paid
/// (3600 each), that many of each chapter goes (all of them when `take_all_items`). `true` when
/// something was paid.
fn exchange_chapters(ctx: &mut ScriptCtx<'_>, player: ObjectId, take_all_items: bool) -> bool {
    let water_chapters = ctx.get_quest_items_count(player, CHAPTER_OF_WATER);
    let earth_chapters = ctx.get_quest_items_count(player, CHAPTER_OF_EARTH);
    let wind_chapters = ctx.get_quest_items_count(player, CHAPTER_OF_WIND);
    let fire_chapters = ctx.get_quest_items_count(player, CHAPTER_OF_FIRE);
    let min_count = water_chapters
        .min(earth_chapters)
        .min(wind_chapters)
        .min(fire_chapters);
    if min_count > 0 {
        ctx.give_adena(player, min_count * 3600, true);
    }
    let count_to_take = if take_all_items { -1 } else { min_count };
    ctx.take_items_many(
        player,
        count_to_take,
        &[
            CHAPTER_OF_WATER,
            CHAPTER_OF_EARTH,
            CHAPTER_OF_WIND,
            CHAPTER_OF_FIRE,
        ],
    );
    min_count > 0
}

/// Java `Q00370_AnElderSowsSeeds`.
pub struct Q00370AnElderSowsSeeds;

impl Script for Q00370AnElderSowsSeeds {
    fn name(&self) -> &'static str {
        "Q00370_AnElderSowsSeeds"
    }

    fn path(&self) -> &'static str {
        "quests/Q00370_AnElderSowsSeeds"
    }

    fn id(&self) -> i32 {
        370
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[CASIAN]);
        s.add_talk_id(&[CASIAN]);
        let mobs1: Vec<i32> = MOBS1.iter().map(|(id, _)| *id).collect();
        let mobs2: Vec<i32> = MOBS2.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&mobs1);
        s.add_kill_id(&mobs2);
    }

    fn check_party_member_state(
        &self,
        ctx: &mut ScriptCtx<'_>,
        qs: l2_world::script::Qs,
        _npc: Option<ScriptNpc>,
    ) -> bool {
        qs.is_started(ctx)
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
        let mut htmltext = None;
        match event {
            "30612-02.htm" | "30612-03.htm" | "30612-06.html" | "30612-07.html" | "30612-09.html" => {
                htmltext = Some(event.to_owned());
            }
            "30612-04.htm" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "REWARD" => {
                if qs.is_started(ctx) {
                    htmltext = Some(if exchange_chapters(ctx, player, false) {
                        "30612-08.html".to_owned()
                    } else {
                        "30612-11.html".to_owned()
                    });
                }
            }
            "30612-10.html" => {
                if qs.is_started(ctx) {
                    exchange_chapters(ctx, player, true);
                    qs.exit_quest(ctx, true, true);
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if let Some(qs) = ctx.random_party_member_state(killer, -1, 3, Some(npc)) {
            if let Some((_, chance)) = MOBS1.iter().find(|(id, _)| *id == npc.id) {
                if ctx.get_random(100) < *chance
                    && let Some(lucky) = ctx.random_party_member_for_npc(killer, Some(npc))
                {
                    ctx.give_item_randomly(lucky, Some(npc), SPELLBOOK_PAGE, 1, 1, 0, 1.0, true);
                }
            } else if let Some((_, chance)) = MOBS2.iter().find(|(id, _)| *id == npc.id) {
                ctx.give_item_randomly(qs.player(), Some(npc), SPELLBOOK_PAGE, 1, 1, 0, *chance, true);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                "30612-01.htm"
            } else {
                "30612-05.html"
            }
            .to_owned();
        } else if qs.is_started(ctx) {
            htmltext = "30612-06.html".to_owned();
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

    const NAME: &str = "Q00370_AnElderSowsSeeds";

    const NPCS: &str = r#"
        <npc id="30612" type="Folk" level="70" name="Casian"/>
        <npc id="20082" type="Monster" level="20" name="Ant Recruit"/>
        <npc id="20084" type="Monster" level="20" name="Ant Patrol"/>
        <npc id="20086" type="Monster" level="20" name="Ant Guard"/>
        <npc id="20089" type="Monster" level="20" name="Noble Ant"/>
        <npc id="20090" type="Monster" level="20" name="Noble Ant Leader"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="5916" type="EtcItem" name="Spellbook Page"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="5917" type="EtcItem" name="Chapter of Fire"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="5918" type="EtcItem" name="Chapter of Water"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="5919" type="EtcItem" name="Chapter of Wind"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="5920" type="EtcItem" name="Chapter of Earth"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00370AnElderSowsSeeds);
        let pages = Pages::of(&Q00370AnElderSowsSeeds)
            .page("30612-01.htm", &["30612-02.htm"])
            .page("30612-02.htm", &["30612-03.htm"])
            .page("30612-03.htm", &["30612-04.htm"])
            .plain(&[
                "30612-04.htm",
                "30612-05.html",
                "30612-09.html",
                "30612-10.html",
                "30612-11.html",
            ])
            .page(
                "30612-06.html",
                &["30612-07.html", "REWARD", "30612-09.html", "30612-10.html"],
            )
            .page("30612-07.html", &["30612-06.html"])
            .page("30612-08.html", &["30612-09.html", "30612-10.html"])
            .npcs(&[CASIAN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let casian = t.spawn(CASIAN, 20);
        (t, casian)
    }

    /// Accepts the quest: 01 -> 02 -> 03 -> 04 (cond 1).
    fn accept(t: &mut ScriptTest, casian: Oid) {
        assert!(t.quest_page(1, casian).contains("30612-01.htm"));
        assert!(t.event(1, NAME, "30612-02.htm").contains("30612-02.htm"));
        assert!(t.event(1, NAME, "30612-03.htm").contains("30612-03.htm"));
        assert!(t.event(1, NAME, "30612-04.htm").contains("30612-04.htm"));
        t.take_writes();
    }

    #[test]
    fn a_level_27_character_is_sent_away() {
        let (mut t, casian) = setup();
        t.enter(1, 27, vec![]);
        assert!(t.quest_page(1, casian).contains("30612-05.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_chapters_are_exchanged_and_the_quit_takes_the_rest() {
        let (mut t, casian) = setup();
        t.enter(
            1,
            28,
            vec![
                item(100, CHAPTER_OF_WATER, 2),
                item(101, CHAPTER_OF_EARTH, 3),
                item(102, CHAPTER_OF_WIND, 1),
                item(103, CHAPTER_OF_FIRE, 4),
            ],
        );
        assert!(t.quest_page(1, casian).contains("30612-01.htm"));
        assert!(t.event(1, NAME, "30612-02.htm").contains("30612-02.htm"));
        assert!(t.event(1, NAME, "30612-03.htm").contains("30612-03.htm"));
        assert!(t.event(1, NAME, "30612-04.htm").contains("30612-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // One of each chapter: 3600 adena, one of each goes.
        assert!(t.quest_page(1, casian).contains("30612-06.html"));
        assert!(t.event(1, NAME, "REWARD").contains("30612-08.html"));
        assert_eq!(t.count(1, 57), 3600);
        assert_eq!(
            (
                t.count(1, CHAPTER_OF_WATER),
                t.count(1, CHAPTER_OF_EARTH),
                t.count(1, CHAPTER_OF_WIND),
                t.count(1, CHAPTER_OF_FIRE)
            ),
            (1, 2, 0, 3)
        );
        assert!(t.take_writes().is_empty());

        // No wind chapter left: nothing is paid or taken.
        assert!(t.quest_page(1, casian).contains("30612-06.html"));
        assert!(t.event(1, NAME, "REWARD").contains("30612-11.html"));
        assert_eq!(t.count(1, 57), 3600);
        assert_eq!(t.count(1, CHAPTER_OF_WATER), 1);

        // "I want to quit" (10): the rest of the chapters go, nothing is paid, the quest ends.
        assert!(t.quest_page(1, casian).contains("30612-06.html"));
        assert!(t.event(1, NAME, "30612-10.html").contains("30612-10.html"));
        assert_eq!(t.count(1, CHAPTER_OF_WATER), 0);
        assert_eq!(t.count(1, CHAPTER_OF_FIRE), 0);
        assert_eq!(t.count(1, 57), 3600);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }

    #[test]
    fn the_ants_drop_spellbook_pages() {
        let (mut t, casian) = setup();
        t.enter(1, 28, vec![]);
        accept(&mut t, casian);
        for kills in 0..400 {
            if t.count(1, SPELLBOOK_PAGE) >= 2 {
                break;
            }
            let ant = match kills % 3 {
                0 => 20084,
                1 => 20082,
                _ => 20089,
            };
            let mob = t.spawn(ant, 60);
            t.kill(1, mob);
        }
        assert!(t.count(1, SPELLBOOK_PAGE) >= 2);
        // The pages are not quest items of this script: no quest row is written.
        assert!(t.take_writes().is_empty());
    }
}
