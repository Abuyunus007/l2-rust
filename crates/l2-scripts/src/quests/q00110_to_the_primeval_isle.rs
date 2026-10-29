//! Quest 110: To the Primeval Isle (Anton in Rune Town hands out the book, Marquez on the island).
// Java: dist/game/data/scripts/quests/Q00110_ToThePrimevalIsle/Q00110_ToThePrimevalIsle.java
//
// Pattern: a delivery (the book is a quest item, given at the start and kept until the reward),
// `on_event` echoes every event like Java `return event`; the no-quest message comes back from
// `on_event` when the player has no state (Java `getNoQuestMsg`).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const ANTON: i32 = 31338;
const MARQUEZ: i32 = 32113;

// Item
const ANCIENT_BOOK: i32 = 8777;

/// Java `Q00110_ToThePrimevalIsle`.
pub struct Q00110ToThePrimevalIsle;

impl Script for Q00110ToThePrimevalIsle {
    fn name(&self) -> &'static str {
        "Q00110_ToThePrimevalIsle"
    }

    fn path(&self) -> &'static str {
        "quests/Q00110_ToThePrimevalIsle"
    }

    fn id(&self) -> i32 {
        110
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ANTON]);
        s.add_talk_id(&[ANTON, MARQUEZ]);
        s.register_quest_items(&[ANCIENT_BOOK]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return Some(ctx.get_no_quest_msg(player));
        };

        match event {
            "31338-1.html" => {
                ctx.give_items(player, ANCIENT_BOOK, 1);
                qs.start_quest(ctx);
            }
            "32113-2.html" | "32113-2a.html" => {
                ctx.give_adena(player, 191_678, true);
                ctx.add_exp_and_sp(player, 251_602, 25245);
                qs.exit_quest(ctx, false, true);
            }
            _ => {}
        }
        Some(event.to_owned())
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            ANTON => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) < 75 {
                        "31338-0a.htm"
                    } else {
                        "31338-0b.htm"
                    }
                    .to_owned();
                }
                STARTED => htmltext = "31338-1a.html".to_owned(),
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            MARQUEZ => {
                if qs.is_cond(ctx, 1) {
                    htmltext = "32113-1.html".to_owned();
                }
            }
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
    use crate::testing::rows::{delete, insert, update};
    use crate::testing::{Pages, ScriptTest};

    const NPCS: &str = r#"
        <npc id="31338" type="Folk" level="75" name="Anton"/>
        <npc id="32113" type="Folk" level="75" name="Marquez"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="8777" type="EtcItem" name="Ancient Book"><set name="is_questitem" val="true"/></item>"#;

    const NAME: &str = "Q00110_ToThePrimevalIsle";

    struct Npcs {
        anton: Oid,
        marquez: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00110ToThePrimevalIsle);
        let pages = Pages::of(&Q00110ToThePrimevalIsle)
            .page("31338-0b.htm", &["31338-0c.htm"])
            .page("31338-0c.htm", &["31338-0d.htm"])
            .page("31338-0d.htm", &["31338-1.html"])
            .plain(&["31338-0a.htm", "31338-1a.html"])
            .page("32113-1.html", &["32113-1a.html"])
            .page("32113-1a.html", &["32113-1b.html"])
            .page("32113-1b.html", &["32113-2.html", "32113-2a.html"])
            .plain(&["31338-1.html", "32113-2.html", "32113-2a.html"])
            .npcs(&[ANTON, MARQUEZ])
            .file("data/html/noquest.htm", "<html><body>noquest</body></html>");
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            anton: t.spawn(ANTON, 20),
            marquez: t.spawn(MARQUEZ, 40),
        };
        (t, npcs)
    }

    #[test]
    fn a_character_below_level_75_is_sent_away() {
        let (mut t, npcs) = setup();
        t.enter(1, 74, vec![]);
        assert!(t.quest_page(1, npcs.anton).contains("31338-0a.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn marquez_has_nothing_to_say_before_the_book_is_delivered() {
        let (mut t, npcs) = setup();
        t.enter(1, 75, vec![]);
        assert!(t.quest_page(1, npcs.marquez).contains("noquest"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_whole_quest_from_anton_to_the_reward() {
        let (mut t, npcs) = setup();
        t.enter(1, 75, vec![]);
        assert!(t.quest_page(1, npcs.anton).contains("31338-0b.htm"));
        assert!(t.event(1, NAME, "31338-0c.htm").contains("31338-0c.htm"));
        assert!(t.event(1, NAME, "31338-0d.htm").contains("31338-0d.htm"));

        // Accept: the book is given, then `startQuest()`.
        assert!(t.event(1, NAME, "31338-1.html").contains("31338-1.html"));
        assert_eq!(t.count(1, ANCIENT_BOOK), 1);
        assert_eq!(
            t.take_writes(),
            [insert(NAME, "cond", "1"), insert(NAME, "<state>", "Started")]
        );

        // Anton repeats his line while the quest runs; Marquez is the next step.
        assert!(t.quest_page(1, npcs.anton).contains("31338-1a.html"));
        assert!(t.quest_page(1, npcs.marquez).contains("32113-1.html"));
        assert!(t.event(1, NAME, "32113-1a.html").contains("32113-1a.html"));
        assert!(t.event(1, NAME, "32113-1b.html").contains("32113-1b.html"));

        // Marquez's reward: adena, exp and sp, then `exitQuest(false, true)`.
        assert!(t.event(1, NAME, "32113-2.html").contains("32113-2.html"));
        assert_eq!(t.count(1, 57), 191_678);
        assert_eq!(t.count(1, ANCIENT_BOOK), 0);
        assert_eq!(
            t.take_writes(),
            [delete(NAME, false), update(NAME, "<state>", "Completed")]
        );
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());

        // Done: the "already completed" page.
        assert!(
            t.quest_page(1, npcs.anton)
                .contains("This quest has already been completed.")
        );
    }
}
