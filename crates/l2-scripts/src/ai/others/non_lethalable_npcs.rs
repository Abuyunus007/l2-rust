//! Headquarters and the conquerable hall bosses cannot be hit by lethal skills.
// Java: dist/game/data/scripts/ai/others/NonLethalableNpcs.java

use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

const NPCS: [i32; 6] = [
    35062, // Headquarters
    // Conquerable Hall bosses.
    35410, // Gustav
    35375, // Nurka
    35629, // Lidia
    35630, // Alfred
    35631, // Giselle
];

/// Java `ai.others.NonLethalableNpcs`.
pub struct NonLethalableNpcs;

impl Script for NonLethalableNpcs {
    fn name(&self) -> &'static str {
        "NonLethalableNpcs"
    }

    fn path(&self) -> &'static str {
        "ai/others"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_spawn_id(&NPCS);
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.world_mut().set_lethalable(npc.object_id, false);
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPC_DEFS: &str = r#"<npc id="35062" type="Monster" level="50" name="Headquarters"><stats><vitals hp="1000" mp="100"/></stats></npc>"#;

    #[test]
    fn a_spawned_headquarters_is_not_lethalable() {
        let mut registry = ScriptRegistry::new();
        registry.add(NonLethalableNpcs);
        let mut t = ScriptTest::new(registry, NPC_DEFS, "", &[]);
        let hq = t.spawn(35062, 20);
        t.world.script_drain();
        assert!(!t.world.is_lethalable_by_script(hq));
    }
}
