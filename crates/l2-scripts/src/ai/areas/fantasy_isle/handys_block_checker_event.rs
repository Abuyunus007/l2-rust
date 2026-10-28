//! Handy's Block Checker Event: the registration at the arena managers.
// Java: dist/game/data/scripts/ai/areas/FantasyIsle/HandysBlockCheckerEvent.java
//
// The body of `onFirstTalk` (the capacity and "being prepared" answers, the team list, the ready
// question) is the world's `minigame/block_checker.rs`, which also runs the event.

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// Arena Managers
const A_MANAGER_1: i32 = 32521;
const A_MANAGER_2: i32 = 32522;
const A_MANAGER_3: i32 = 32523;
const A_MANAGER_4: i32 = 32524;

/// Java `ai.areas.FantasyIsle.HandysBlockCheckerEvent`.
pub struct HandysBlockCheckerEvent;

impl Script for HandysBlockCheckerEvent {
    fn name(&self) -> &'static str {
        "HandysBlockCheckerEvent"
    }

    fn path(&self) -> &'static str {
        "ai/areas/FantasyIsle"
    }

    /// Java's `main` creates the script only with `GeneralConfig.ENABLE_BLOCK_CHECKER_EVENT`.
    fn enabled(&self, world: &l2_world::World) -> bool {
        world.block_checker_event_enabled()
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_first_talk_id(&[A_MANAGER_1, A_MANAGER_2, A_MANAGER_3, A_MANAGER_4]);
        // `HandysBlockCheckerManager.getInstance().startUpParticipantsQueue()` is
        // `World::start_minigames` (with `EnableBlockCheckerEvent`).
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let arena = npc.id - A_MANAGER_1;
        ctx.world_mut().block_checker_first_talk(player, arena);
        None
    }
}

#[cfg(test)]
mod tests {
    use l2_world::{MinigameLoad, MinigameSettings, ScriptRegistry};

    use super::*;
    use crate::testing::{ScriptTest, names_to, pages_to};

    const NPCS_XML: &str = r#"<npc id="32521" type="Folk" level="70" name="Arena Manager"/>
        <npc id="32522" type="Folk" level="70" name="Arena Manager"/>"#;

    fn setup(enabled: bool) -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(HandysBlockCheckerEvent);
        let pages = [(
            "data/html/default/32521.htm".to_owned(),
            "<html><body>Own page</body></html>".to_owned(),
        )];
        let pages: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let mut t = ScriptTest::new(registry, NPCS_XML, "", &pages);
        t.world.set_minigame_settings(MinigameSettings {
            enable_block_checker_event: enabled,
            ..MinigameSettings::default()
        });
        t.world.start_minigames(MinigameLoad::default());
        t
    }

    #[test]
    fn the_first_talk_registers_the_player_in_the_arena_of_the_manager() {
        let mut t = setup(true);
        let manager = t.spawn(32521, 20);
        t.enter(1, 40, vec![]);
        t.take();
        t.talk(1, manager);
        let out = t.take();
        assert!(
            names_to(&out, 1).contains(&"EX_BLOCK_UP_SET_LIST".to_owned()),
            "{:?}",
            names_to(&out, 1)
        );
        // The script answers with packets, never with a page.
        assert!(pages_to(&out, 1).is_empty());
        assert_eq!(
            t.world.block_arena(0).map(|a| a.red.len() + a.blue.len()),
            Some(1)
        );
    }

    #[test]
    fn without_the_event_the_manager_shows_its_own_page() {
        let mut t = setup(false);
        let manager = t.spawn(32521, 20);
        t.enter(1, 40, vec![]);
        t.take();
        t.talk(1, manager);
        assert_eq!(pages_to(&t.take(), 1), ["<html><body>Own page</body></html>"]);
    }
}
