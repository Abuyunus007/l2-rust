//! Quest 639: Guardians of the Holy Grail (Dominic in Town of Aden; no longer available since
//! Freya, CT 2.5).
// Java: dist/game/data/scripts/quests/Q00639_GuardiansOfTheHolyGrail/Q00639_GuardiansOfTheHolyGrail.java
//
// Pattern: the start NPC only ends the quest: the state is created, removed (repeatable) and the
// one page is shown. Nothing can be taken from it.

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const DOMINIC: i32 = 31350;

/// Java `Q00639_GuardiansOfTheHolyGrail`.
pub struct Q00639GuardiansOfTheHolyGrail;

impl Script for Q00639GuardiansOfTheHolyGrail {
    fn name(&self) -> &'static str {
        "Q00639_GuardiansOfTheHolyGrail"
    }

    fn path(&self) -> &'static str {
        "quests/Q00639_GuardiansOfTheHolyGrail"
    }

    fn id(&self) -> i32 {
        639
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[DOMINIC]);
        s.add_talk_id(&[DOMINIC]);
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        // Java: `getQuestState(player, true)` then `qs.exitQuest(true)` (no sound).
        let qs = ctx.get_quest_state(player, true)?;
        qs.exit_quest(ctx, true, false);
        Some("31350-01.html".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest};

    const NPCS: &str = r#"<npc id="31350" type="Folk" level="70" name="Dominic"/>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00639GuardiansOfTheHolyGrail);
        let pages = Pages::of(&Q00639GuardiansOfTheHolyGrail)
            .plain(&["31350-01.html"])
            .npcs(&[DOMINIC]);
        let mut t = ScriptTest::with_pages(registry, NPCS, "", pages);
        let dominic = t.spawn(DOMINIC, 20);
        (t, dominic)
    }

    #[test]
    fn talking_shows_the_page_and_writes_nothing() {
        let (mut t, dominic) = setup();
        t.enter(1, 20, vec![]);
        assert!(t.quest_page(1, dominic).contains("31350-01.html"));
        // `getQuestState(player, true)` makes a state in memory, and `exitQuest` returns at once
        // for a state that is not started: no row is written.
        assert!(t.take_writes().is_empty());
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }
}
