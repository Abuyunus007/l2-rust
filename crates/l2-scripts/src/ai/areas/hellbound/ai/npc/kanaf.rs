//! Kanaf, who tells one of three stories.
// Java: dist/game/data/scripts/ai/areas/Hellbound/AI/NPC/Kanaf/Kanaf.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const KANAF: i32 = 32346;

/// Java `ai.areas.Hellbound.AI.NPC.Kanaf.Kanaf`.
pub struct Kanaf;

impl Script for Kanaf {
    fn name(&self) -> &'static str {
        "Kanaf"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Hellbound/AI/NPC/Kanaf"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[KANAF]);
        s.add_talk_id(&[KANAF]);
        s.add_first_talk_id(&[KANAF]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        if event == "info" {
            return Some(format!("32346-0{}.htm", ctx.get_random_range(1, 3)));
        }

        None
    }

    fn on_first_talk(&self, _ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some(format!("{}.html", npc.id))
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::ai::areas::hellbound::testing::last_page;
    use crate::testing::{Pages, ScriptTest};

    const NPCS: &str = r#"<npc id="32346" type="Folk" level="70" name="Kanaf"/>"#;

    #[test]
    fn the_first_page_is_the_npc_id_and_info_tells_a_story() {
        let mut registry = ScriptRegistry::new();
        registry.add(Kanaf);
        let pages = Pages::of(&Kanaf).page("32346.html", &["info"]).plain(&[
            "32346-01.htm",
            "32346-02.htm",
            "32346-03.htm",
        ]);
        let mut t = ScriptTest::with_pages(registry, NPCS, "", pages);
        let npc = t.spawn(KANAF, 20);
        t.enter(1, 80, vec![]);
        t.talk(1, npc);
        assert!(last_page(&t.take(), 1).contains("32346.html"));
        let story = t.event(1, "Kanaf", "info");
        assert!(
            ["32346-01.htm", "32346-02.htm", "32346-03.htm"]
                .iter()
                .any(|p| story.contains(p))
        );
    }
}
