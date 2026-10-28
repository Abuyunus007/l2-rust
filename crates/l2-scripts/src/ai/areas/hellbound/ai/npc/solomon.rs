//! Solomon, the native who speaks of the Quarry once Hellbound reached level five.
// Java: dist/game/data/scripts/ai/areas/Hellbound/AI/NPC/Solomon/Solomon.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use crate::ai::areas::hellbound::engine;

// NPCs
const SOLOMON: i32 = 32355;

/// Java `ai.areas.Hellbound.AI.NPC.Solomon.Solomon`.
pub struct Solomon;

impl Script for Solomon {
    fn name(&self) -> &'static str {
        "Solomon"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Hellbound/AI/NPC/Solomon"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_first_talk_id(&[SOLOMON]);
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        if engine::level(ctx) == 5 {
            return Some("32355-01.htm".to_owned());
        } else if engine::level(ctx) > 5 {
            return Some("32355-01a.htm".to_owned());
        }

        // `super.onFirstTalk(npc, player)` is `null`: `showResult` shows nothing.
        None
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::ai::areas::hellbound::testing::{last_page, set_level};
    use crate::testing::{Pages, ScriptTest};

    const NPCS: &str = r#"<npc id="32355" type="Folk" level="70" name="Solomon"/>"#;

    #[test]
    fn the_page_follows_the_level() {
        let mut registry = ScriptRegistry::new();
        registry.add(Solomon);
        let pages = Pages::of(&Solomon).plain(&["32355-01.htm", "32355-01a.htm"]).npcs(&[SOLOMON]);
        let mut t = ScriptTest::with_pages(registry, NPCS, "", pages);
        let npc = t.spawn(SOLOMON, 20);
        t.enter(1, 80, vec![]);
        set_level(&mut t, "Solomon", 5);
        t.talk(1, npc);
        assert!(last_page(&t.take(), 1).contains("32355-01.htm"));
        set_level(&mut t, "Solomon", 6);
        t.talk(1, npc);
        assert!(last_page(&t.take(), 1).contains("32355-01a.htm"));
        // Below level five `super.onFirstTalk` answers `null`: no page.
        set_level(&mut t, "Solomon", 2);
        t.talk(1, npc);
        assert!(last_page(&t.take(), 1).is_empty());
    }
}
