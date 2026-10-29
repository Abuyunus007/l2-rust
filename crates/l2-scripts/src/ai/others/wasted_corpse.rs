//! The Corpse of Deadman dies as soon as it spawns.
// Java: dist/game/data/scripts/ai/others/WastedCorpse.java

use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const CORPSE_OF_DEADMAN: i32 = 18119;

/// Java `ai.others.WastedCorpse`.
pub struct WastedCorpse;

impl Script for WastedCorpse {
    fn name(&self) -> &'static str {
        "WastedCorpse"
    }

    fn path(&self) -> &'static str {
        "ai/others"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_spawn_id(&[CORPSE_OF_DEADMAN]);
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.world_mut().do_die(npc.object_id, None);
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPC_DEFS: &str = r#"<npc id="18119" type="Monster" level="10" name="Corpse of Deadman"><stats><vitals hp="1000" mp="100"/></stats></npc>"#;

    #[test]
    fn the_corpse_is_dead_on_spawn() {
        let mut registry = ScriptRegistry::new();
        registry.add(WastedCorpse);
        let mut t = ScriptTest::new(registry, NPC_DEFS, "", &[]);
        let corpse = t.spawn(CORPSE_OF_DEADMAN, 20);
        t.world.script_drain();
        assert!(t.world.is_dead(corpse));
    }
}
