//! Quest 122: Ominous News (Seer Moira in the village, Karuda on the western path).
// Java: dist/game/data/scripts/quests/Q00122_OminousNews/Q00122_OminousNews.java
//
// Pattern: the same as Q00121 (a start NPC, a second NPC, a reward with adena and exp). The
// `on_event` echoes every event, like Java `return event`.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const MOIRA: i32 = 31979;
const KARUDA: i32 = 32017;

// Misc
const MIN_LEVEL: i32 = 20;

/// Java `Q00122_OminousNews`.
pub struct Q00122OminousNews;

impl Script for Q00122OminousNews {
    fn name(&self) -> &'static str {
        "Q00122_OminousNews"
    }

    fn path(&self) -> &'static str {
        "quests/Q00122_OminousNews"
    }

    fn id(&self) -> i32 {
        122
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MOIRA]);
        s.add_talk_id(&[MOIRA, KARUDA]);
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
            "31979-02.htm" => qs.start_quest(ctx),
            "32017-02.html" => {
                ctx.give_adena(player, 8923, true);
                ctx.add_exp_and_sp(player, 45_151, 2310);
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
            MOIRA => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        "31979-01.htm"
                    } else {
                        "31979-00.htm"
                    }
                    .to_owned();
                }
                STARTED => htmltext = "31979-03.html".to_owned(),
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            KARUDA => {
                if qs.is_started(ctx) {
                    htmltext = "32017-01.html".to_owned();
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
        <npc id="31979" type="Folk" level="20" name="Seer Moira"/>
        <npc id="32017" type="Folk" level="20" name="Karuda"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>"#;

    const NAME: &str = "Q00122_OminousNews";

    struct Npcs {
        moira: Oid,
        karuda: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00122OminousNews);
        let pages = Pages::of(&Q00122OminousNews)
            .page("31979-00.htm", &[])
            .page("31979-01.htm", &["31979-02.htm"])
            .plain(&["31979-02.htm", "31979-03.html"])
            .page("32017-01.html", &["32017-02.html"])
            .plain(&["32017-02.html"])
            .npcs(&[MOIRA, KARUDA])
            .file("data/html/noquest.htm", "<html><body>noquest</body></html>");
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            moira: t.spawn(MOIRA, 20),
            karuda: t.spawn(KARUDA, 40),
        };
        (t, npcs)
    }

    #[test]
    fn a_character_below_level_20_is_sent_away() {
        let (mut t, npcs) = setup();
        t.enter(1, 19, vec![]);
        assert!(t.quest_page(1, npcs.moira).contains("31979-00.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn karuda_has_nothing_to_say_before_the_quest() {
        let (mut t, npcs) = setup();
        t.enter(1, 20, vec![]);
        assert!(t.quest_page(1, npcs.karuda).contains("noquest"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_whole_quest_from_moira_to_the_reward() {
        let (mut t, npcs) = setup();
        t.enter(1, 20, vec![]);
        assert!(t.quest_page(1, npcs.moira).contains("31979-01.htm"));

        // Accept: `startQuest()`.
        assert!(t.event(1, NAME, "31979-02.htm").contains("31979-02.htm"));
        assert_eq!(
            t.take_writes(),
            [insert(NAME, "cond", "1"), insert(NAME, "<state>", "Started")]
        );

        // Moira repeats her line while the quest runs; Karuda is the next step.
        assert!(t.quest_page(1, npcs.moira).contains("31979-03.html"));
        assert!(t.quest_page(1, npcs.karuda).contains("32017-01.html"));

        // Karuda's reward: adena, exp and sp, then `exitQuest(false, true)`.
        assert!(t.event(1, NAME, "32017-02.html").contains("32017-02.html"));
        assert_eq!(t.count(1, 57), 8923);
        assert_eq!(
            t.take_writes(),
            [delete(NAME, false), update(NAME, "<state>", "Completed")]
        );
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());

        // Done: the "already completed" page.
        assert!(t.quest_page(1, npcs.moira).contains("This quest has already been completed."));
    }
}
