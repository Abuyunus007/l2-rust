//! Quest 121: Pavel the Giant (Newyear at the Blacksmith's, Yumi at the Pavel Ruins barracks).
// Java: dist/game/data/scripts/quests/Q00121_PavelTheGiant/Q00121_PavelTheGiant.java
//
// Pattern: a start NPC that sends the player to a second NPC, a reward with exp and sp, and an
// `on_event` that echoes every event (Java `return event`).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const NEWYEAR: i32 = 31961;
const YUMI: i32 = 32041;

// Misc
const MIN_LEVEL: i32 = 70;

/// Java `Q00121_PavelTheGiant`.
pub struct Q00121PavelTheGiant;

impl Script for Q00121PavelTheGiant {
    fn name(&self) -> &'static str {
        "Q00121_PavelTheGiant"
    }

    fn path(&self) -> &'static str {
        "quests/Q00121_PavelTheGiant"
    }

    fn id(&self) -> i32 {
        121
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[NEWYEAR]);
        s.add_talk_id(&[NEWYEAR, YUMI]);
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
            "31961-02.htm" => qs.start_quest(ctx),
            "32041-02.html" => {
                ctx.add_exp_and_sp(player, 346_320, 26_069);
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
            NEWYEAR => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        "31961-01.htm"
                    } else {
                        "31961-00.htm"
                    }
                    .to_owned();
                }
                STARTED => htmltext = "31961-03.html".to_owned(),
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            YUMI => {
                if qs.is_started(ctx) {
                    htmltext = "32041-01.html".to_owned();
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
        <npc id="31961" type="Folk" level="70" name="Newyear"/>
        <npc id="32041" type="Folk" level="70" name="Yumi"/>"#;

    const NAME: &str = "Q00121_PavelTheGiant";

    struct Npcs {
        newyear: Oid,
        yumi: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00121PavelTheGiant);
        let pages = Pages::of(&Q00121PavelTheGiant)
            .page("31961-00.htm", &[])
            .page("31961-01.htm", &["31961-02.htm"])
            .plain(&["31961-02.htm", "31961-03.html"])
            .page("32041-01.html", &["32041-02.html"])
            .plain(&["32041-02.html"])
            .npcs(&[NEWYEAR, YUMI])
            .file("data/html/noquest.htm", "<html><body>noquest</body></html>");
        let mut t = ScriptTest::with_pages(registry, NPCS, "", pages);
        let npcs = Npcs {
            newyear: t.spawn(NEWYEAR, 20),
            yumi: t.spawn(YUMI, 40),
        };
        (t, npcs)
    }

    #[test]
    fn a_character_below_level_70_is_sent_away() {
        let (mut t, npcs) = setup();
        t.enter(1, 69, vec![]);
        assert!(t.quest_page(1, npcs.newyear).contains("31961-00.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn yumi_has_nothing_to_say_before_the_quest() {
        let (mut t, npcs) = setup();
        t.enter(1, 70, vec![]);
        assert!(t.quest_page(1, npcs.yumi).contains("noquest"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_whole_quest_from_newyear_to_the_reward() {
        let (mut t, npcs) = setup();
        t.enter(1, 70, vec![]);
        assert!(t.quest_page(1, npcs.newyear).contains("31961-01.htm"));

        // Accept: `startQuest()`; the event is echoed as the quest window page.
        assert!(t.event(1, NAME, "31961-02.htm").contains("31961-02.htm"));
        assert_eq!(
            t.take_writes(),
            [insert(NAME, "cond", "1"), insert(NAME, "<state>", "Started")]
        );
        assert_eq!(t.world.quest_list(Oid(1)).quests.len(), 1);

        // Newyear repeats his line while the quest runs; Yumi starts the job.
        assert!(t.quest_page(1, npcs.newyear).contains("31961-03.html"));
        assert!(t.quest_page(1, npcs.yumi).contains("32041-01.html"));

        // Yumi's reward: exp and sp, then `exitQuest(false, true)`.
        assert!(t.event(1, NAME, "32041-02.html").contains("32041-02.html"));
        // The exp itself is not checked: the test world has no exp tables (`can_gain_exp`).
        assert_eq!(
            t.take_writes(),
            [delete(NAME, false), update(NAME, "<state>", "Completed")]
        );
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());

        // Done: the "already completed" page.
        assert!(t.quest_page(1, npcs.newyear).contains("This quest has already been completed."));
    }
}
