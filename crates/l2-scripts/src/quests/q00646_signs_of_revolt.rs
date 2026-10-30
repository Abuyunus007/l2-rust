//! Quest 646: Signs of Revolt (Torrant in the Beacon Tower; no longer available since Gracia
//! Epilogue).
// Java: dist/game/data/scripts/quests/Q00646_SignsOfRevolt/Q00646_SignsOfRevolt.java
//
// Pattern: like Q00639, the start NPC ends the quest; the page depends on the level only.

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const TORRANT: i32 = 32016;

// Misc
const MIN_LEVEL: i32 = 80;

/// Java `Q00646_SignsOfRevolt`.
pub struct Q00646SignsOfRevolt;

impl Script for Q00646SignsOfRevolt {
    fn name(&self) -> &'static str {
        "Q00646_SignsOfRevolt"
    }

    fn path(&self) -> &'static str {
        "quests/Q00646_SignsOfRevolt"
    }

    fn id(&self) -> i32 {
        646
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[TORRANT]);
        s.add_talk_id(&[TORRANT]);
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        // Java: `getQuestState(player, true)` then `qs.exitQuest(true)` (no sound).
        let qs = ctx.get_quest_state(player, true)?;
        qs.exit_quest(ctx, true, false);
        let page = if ctx.player_level(player) >= MIN_LEVEL {
            "32016-01.html"
        } else {
            "32016-02.html"
        };
        Some(page.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest};

    const NPCS: &str = r#"<npc id="32016" type="Folk" level="70" name="Torrant"/>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00646SignsOfRevolt);
        let pages = Pages::of(&Q00646SignsOfRevolt)
            .plain(&["32016-01.html", "32016-02.html"])
            .npcs(&[TORRANT]);
        let mut t = ScriptTest::with_pages(registry, NPCS, "", pages);
        let torrant = t.spawn(TORRANT, 20);
        (t, torrant)
    }

    #[test]
    fn a_character_below_level_80_is_told_it_came_too_late() {
        let (mut t, torrant) = setup();
        t.enter(1, 79, vec![]);
        assert!(t.quest_page(1, torrant).contains("32016-02.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_character_of_level_80_gets_the_other_page() {
        let (mut t, torrant) = setup();
        t.enter(1, 80, vec![]);
        assert!(t.quest_page(1, torrant).contains("32016-01.html"));
        assert!(t.take_writes().is_empty());
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }
}
