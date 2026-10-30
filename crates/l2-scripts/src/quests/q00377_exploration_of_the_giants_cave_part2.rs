//! Quest 377: Exploration of the Giants' Cave Part 2 (Expedition Leader Sobling, lower Giant's Cave).
// Java: dist/game/data/scripts/quests/Q00377_ExplorationOfTheGiantsCavePart2/Q00377_ExplorationOfTheGiantsCavePart2.java
//
// Pattern: a repeatable kill-count quest like Q00376 (`docs/design/scripts.md`, "Quest patterns"):
// the kill goes to a random started party member (`random_party_member_state` with state -1),
// the Titan book drops with no limit. The lesser giants drop two or three books on a roll, the
// others with their chance. The reward is the NPC's multisell list (`-h npc_%objectId%_multisell`),
// served by the NPC bypass, not by the script.

use l2_util::ObjectId;
use l2_world::script::STARTED;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const SOBLING: i32 = 31147;

// Items
const TITAN_ANCIENT_BOOK: i32 = 14847;
const BOOK1: i32 = 14842;
const BOOK2: i32 = 14843;
const BOOK3: i32 = 14844;
const BOOK4: i32 = 14845;
const BOOK5: i32 = 14846;

// Mobs: the lesser giants roll out 2 or 3 books on a chance out of 1000
const MOBS1: [(i32, i32); 6] = [
    (22660, 366), // lesser_giant_re
    (22661, 424), // lesser_giant_soldier_re
    (22662, 304), // lesser_giant_shooter_re
    (22663, 304), // lesser_giant_scout_re
    (22664, 354), // lesser_giant_mage_re
    (22665, 324), // lesser_giant_elder_re
];

// Mobs: the others drop one with a chance
const MOBS2: [(i32, f64); 4] = [
    (22666, 0.276), // barif_re
    (22667, 0.284), // barif_pet_re
    (22668, 0.240), // gamlin_re
    (22669, 0.240), // leogul_re
];

/// Java `Q00377_ExplorationOfTheGiantsCavePart2`.
pub struct Q00377ExplorationOfTheGiantsCavePart2;

impl Script for Q00377ExplorationOfTheGiantsCavePart2 {
    fn name(&self) -> &'static str {
        "Q00377_ExplorationOfTheGiantsCavePart2"
    }

    fn path(&self) -> &'static str {
        "quests/Q00377_ExplorationOfTheGiantsCavePart2"
    }

    fn id(&self) -> i32 {
        377
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[SOBLING]);
        s.add_talk_id(&[SOBLING]);
        let mobs1: Vec<i32> = MOBS1.iter().map(|(id, _)| *id).collect();
        let mobs2: Vec<i32> = MOBS2.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&mobs1);
        s.add_kill_id(&mobs2);
        s.register_quest_items(&[TITAN_ANCIENT_BOOK]);
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
            "31147-02.htm" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "31147-04.html" | "31147-cont.html" => {
                htmltext = Some(event.to_owned());
            }
            "31147-quit.html" => {
                qs.exit_quest(ctx, true, true);
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.random_party_member_state(killer, -1, 3, Some(npc));
        if let Some(qs) = qs {
            let mobs1_chance = MOBS1.iter().find(|(id, _)| *id == npc.id).map(|(_, c)| *c);
            if let Some(mobs1_chance) = mobs1_chance {
                // Java evaluates the amount (`getRandom(1000) < chance ? 3 : 2`) before the call.
                let amount = if ctx.get_random(1000) < mobs1_chance { 3 } else { 2 };
                ctx.give_item_randomly(
                    qs.player(),
                    Some(npc),
                    TITAN_ANCIENT_BOOK,
                    amount,
                    amount,
                    0,
                    1.0,
                    true,
                );
            } else if let Some((_, chance)) = MOBS2.iter().find(|(id, _)| *id == npc.id) {
                ctx.give_item_randomly(qs.player(), Some(npc), TITAN_ANCIENT_BOOK, 1, 1, 0, *chance, true);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            htmltext = if ctx.player_level(player) >= 79 {
                "31147-01.htm"
            } else {
                "31147-00.html"
            }
            .to_owned();
        } else if qs.state(ctx) == STARTED {
            htmltext = if ctx.has_all_quest_items(player, &[BOOK1, BOOK2, BOOK3, BOOK4, BOOK5]) {
                "31147-03.html"
            } else {
                "31147-02a.html"
            }
            .to_owned();
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

    const NAME: &str = "Q00377_ExplorationOfTheGiantsCavePart2";

    const NPCS: &str = r#"
        <npc id="31147" type="Folk" level="70" name="Expedition Leader Sobling"/>
        <npc id="22660" type="Monster" level="75" name="Lesser Giant Soldier"/>
        <npc id="22666" type="Monster" level="75" name="Barif"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="14847" type="EtcItem" name="Titan Ancient Book"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="14842" type="EtcItem" name="Legacy of Ancient Chapter 1"><set name="is_questitem" val="true"/></item>
        <item id="14843" type="EtcItem" name="Legacy of Ancient Chapter 2"><set name="is_questitem" val="true"/></item>
        <item id="14844" type="EtcItem" name="Legacy of Ancient Chapter 3"><set name="is_questitem" val="true"/></item>
        <item id="14845" type="EtcItem" name="Legacy of Ancient Chapter 4"><set name="is_questitem" val="true"/></item>
        <item id="14846" type="EtcItem" name="Legacy of Ancient Chapter 5"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00377ExplorationOfTheGiantsCavePart2);
        let pages = Pages::of(&Q00377ExplorationOfTheGiantsCavePart2)
            .page("31147-01.htm", &["31147-02.htm"])
            .plain(&["31147-00.html", "31147-02.htm", "31147-02a.html"])
            .page(
                "31147-03.html",
                &["31147-04.html", "31147-cont.html", "31147-quit.html"],
            )
            .plain(&["31147-04.html", "31147-cont.html", "31147-quit.html"])
            .npcs(&[SOBLING]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let sobling = t.spawn(SOBLING, 20);
        (t, sobling)
    }

    /// The five chapters of the legacy, in the player's inventory.
    fn books() -> Vec<l2_world::ItemData> {
        [BOOK1, BOOK2, BOOK3, BOOK4, BOOK5]
            .iter()
            .enumerate()
            .map(|(i, id)| item(100 + i as i32, *id, 1))
            .collect()
    }

    #[test]
    fn a_level_78_character_is_sent_away() {
        let (mut t, sobling) = setup();
        t.enter(1, 78, vec![]);
        assert!(t.quest_page(1, sobling).contains("31147-00.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn lesser_giants_roll_two_or_three_books_and_the_others_one() {
        let (mut t, sobling) = setup();
        t.enter(1, 79, vec![]);
        assert!(t.quest_page(1, sobling).contains("31147-01.htm"));
        assert!(t.event(1, NAME, "31147-02.htm").contains("31147-02.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, sobling).contains("31147-02a.html"));

        // A lesser giant gives two or three books at once (no limit, no quest write).
        let before = t.count(1, TITAN_ANCIENT_BOOK);
        let giant = t.spawn(22660, 60);
        t.kill(1, giant);
        let got = t.count(1, TITAN_ANCIENT_BOOK) - before;
        assert!(got == 2 || got == 3, "{got}");
        assert!(t.take_writes().is_empty());

        // The others drop with their chance: a few kills give some.
        for _ in 0..30 {
            let barif = t.spawn(22666, 60);
            t.kill(1, barif);
        }
        assert!(t.count(1, TITAN_ANCIENT_BOOK) > got);
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_five_chapters_show_the_reward_page_and_quit_ends_the_quest() {
        let (mut t, sobling) = setup();
        t.enter(1, 79, books());
        assert!(t.quest_page(1, sobling).contains("31147-01.htm"));
        assert!(t.event(1, NAME, "31147-02.htm").contains("31147-02.htm"));
        t.take_writes();
        assert!(t.quest_page(1, sobling).contains("31147-03.html"));
        assert!(t.event(1, NAME, "31147-cont.html").contains("31147-cont.html"));
        assert!(t.quest_page(1, sobling).contains("31147-03.html"));
        assert!(t.event(1, NAME, "31147-04.html").contains("31147-04.html"));
        // The multisell list is the NPC's own bypass: the script takes nothing.
        assert_eq!(t.count(1, BOOK1), 1);
        assert_eq!(t.count(1, BOOK5), 1);
        assert!(t.take_writes().is_empty());

        // "Stop searching": the quest ends repeatable.
        assert!(t.quest_page(1, sobling).contains("31147-03.html"));
        assert!(t.event(1, NAME, "31147-quit.html").contains("31147-quit.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.quest_page(1, sobling).contains("31147-01.htm"));
    }
}
