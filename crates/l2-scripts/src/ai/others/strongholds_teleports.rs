//! The Stronghold teleporters: the page depends on the player's level.
// Java: dist/game/data/scripts/ai/others/StrongholdsTeleports/StrongholdsTeleports.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const NPCS: [i32; 4] = [32163, 32181, 32184, 32186];

/// Java `ai.others.StrongholdsTeleports.StrongholdsTeleports`.
pub struct StrongholdsTeleports;

impl Script for StrongholdsTeleports {
    fn name(&self) -> &'static str {
        "StrongholdsTeleports"
    }

    fn path(&self) -> &'static str {
        "ai/others/StrongholdsTeleports"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_first_talk_id(&NPCS);
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        if ctx.player_level(player) < 20 {
            return Some(format!("{}.htm", npc.id));
        }

        Some(format!("{}-no.htm", npc.id))
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, pages_to};

    const NPCS_XML: &str = r#"<npc id="32163" type="Folk" level="70" name="Teleporter"/>"#;
    const DIR: &str = "data/scripts/ai/others/StrongholdsTeleports";

    #[test]
    fn the_page_depends_on_the_level() {
        let mut registry = ScriptRegistry::new();
        registry.add(StrongholdsTeleports);
        let pages = [
            (format!("{DIR}/32163.htm"), "<html><body>low</body></html>"),
            (format!("{DIR}/32163-no.htm"), "<html><body>high</body></html>"),
        ];
        let pages: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), *b)).collect();
        let mut t = ScriptTest::new(registry, NPCS_XML, "", &pages);
        let npc = t.spawn(32163, 20);
        t.enter(1, 19, vec![]);
        t.enter(2, 20, vec![]);
        t.take();
        t.talk(1, npc);
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("low"));
        t.talk(2, npc);
        assert!(pages_to(&t.take(), 2).pop().unwrap().contains("high"));
    }
}
