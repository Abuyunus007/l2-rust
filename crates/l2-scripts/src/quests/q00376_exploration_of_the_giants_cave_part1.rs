//! Quest 376: Exploration of the Giants' Cave Part 1 (Expedition Leader Sobling, Upper Giant's Cave).
// Java: dist/game/data/scripts/quests/Q00376_ExplorationOfTheGiantsCavePart1/Q00376_ExplorationOfTheGiantsCavePart1.java
//
// Pattern: a repeatable kill-count quest (`docs/design/scripts.md`, "Quest patterns"): the kill
// goes to a random started party member (`random_party_member_state` with state -1), a parchment
// drop with no limit, the player quits or goes on. The reward is the NPC's multisell list
// (`-h npc_%objectId%_multisell`), which the NPC bypass serves, not the script.

use l2_util::ObjectId;
use l2_world::script::STARTED;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const SOBLING: i32 = 31147;

// Items
const ANCIENT_PARCHMENT: i32 = 14841;
const BOOK1: i32 = 14836;
const BOOK2: i32 = 14837;
const BOOK3: i32 = 14838;
const BOOK4: i32 = 14839;
const BOOK5: i32 = 14840;

// Mobs (drop chance)
const MOBS: [(i32, f64); 8] = [
    (22670, 0.314), // const_lord
    (22671, 0.302), // const_gaurdian
    (22672, 0.300), // const_seer
    (22673, 0.258), // hirokai
    (22674, 0.248), // imagro
    (22675, 0.264), // palite
    (22676, 0.258), // hamrit
    (22677, 0.266), // kranout
];

/// Java `Q00376_ExplorationOfTheGiantsCavePart1`.
pub struct Q00376ExplorationOfTheGiantsCavePart1;

impl Script for Q00376ExplorationOfTheGiantsCavePart1 {
    fn name(&self) -> &'static str {
        "Q00376_ExplorationOfTheGiantsCavePart1"
    }

    fn path(&self) -> &'static str {
        "quests/Q00376_ExplorationOfTheGiantsCavePart1"
    }

    fn id(&self) -> i32 {
        376
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[SOBLING]);
        s.add_talk_id(&[SOBLING]);
        let mobs: Vec<i32> = MOBS.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&mobs);
        s.register_quest_items(&[ANCIENT_PARCHMENT]);
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
            let chance = MOBS.iter().find(|(id, _)| *id == npc.id).map_or(0.0, |(_, c)| *c);
            ctx.give_item_randomly(qs.player(), Some(npc), ANCIENT_PARCHMENT, 1, 1, 0, chance, true);
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

    const NAME: &str = "Q00376_ExplorationOfTheGiantsCavePart1";

    const NPCS: &str = r#"
        <npc id="31147" type="Folk" level="70" name="Expedition Leader Sobling"/>
        <npc id="22670" type="Monster" level="75" name="Cursed Lord"/>
        <npc id="22671" type="Monster" level="75" name="Cursed Guardian"/>
        <npc id="22672" type="Monster" level="75" name="Cursed Seer"/>
        <npc id="22673" type="Monster" level="75" name="Hirokai"/>
        <npc id="22674" type="Monster" level="75" name="Imagro"/>
        <npc id="22675" type="Monster" level="75" name="Palite"/>
        <npc id="22676" type="Monster" level="75" name="Hamrit"/>
        <npc id="22677" type="Monster" level="75" name="Kranout"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="14841" type="EtcItem" name="Ancient Parchment"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="14836" type="EtcItem" name="Legacy of Ancient Chapter 1"><set name="is_questitem" val="true"/></item>
        <item id="14837" type="EtcItem" name="Legacy of Ancient Chapter 2"><set name="is_questitem" val="true"/></item>
        <item id="14838" type="EtcItem" name="Legacy of Ancient Chapter 3"><set name="is_questitem" val="true"/></item>
        <item id="14839" type="EtcItem" name="Legacy of Ancient Chapter 4"><set name="is_questitem" val="true"/></item>
        <item id="14840" type="EtcItem" name="Legacy of Ancient Chapter 5"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00376ExplorationOfTheGiantsCavePart1);
        let pages = Pages::of(&Q00376ExplorationOfTheGiantsCavePart1)
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

    fn books() -> Vec<l2_world::ItemData> {
        // The five chapters of the legacy, in the player's inventory.
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
    fn parchments_drop_with_no_limit_and_no_quest_write() {
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
        // No book yet: the NPC asks for them.
        assert!(t.quest_page(1, sobling).contains("31147-02a.html"));

        // Parchments drop with no limit; no quest write for them.
        for kills in 0..30 {
            let mob = t.spawn(MOBS[kills % 8].0, 60);
            t.kill(1, mob);
        }
        assert!(t.count(1, ANCIENT_PARCHMENT) > 0);
        assert!(t.take_writes().is_empty());
        assert!(t.quest_page(1, sobling).contains("31147-02a.html"));
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
