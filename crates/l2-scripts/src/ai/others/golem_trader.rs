//! The Golem Trader servitor vanishes after three minutes.
// Java: dist/game/data/scripts/ai/others/Servitors/GolemTrader.java

use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const GOLEM_TRADER: i32 = 13128;

/// Java `ai.others.Servitors.GolemTrader`.
pub struct GolemTrader;

impl Script for GolemTrader {
    fn name(&self) -> &'static str {
        "GolemTrader"
    }

    fn path(&self) -> &'static str {
        "ai/others/Servitors"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_spawn_id(&[GOLEM_TRADER]);
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.world_mut().schedule_npc_despawn(npc.object_id, 180000);
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPC_DEFS: &str = r#"<npc id="13128" type="Folk" level="1" name="Golem Trader"/>"#;

    #[test]
    fn the_trader_is_gone_after_three_minutes() {
        let mut registry = ScriptRegistry::new();
        registry.add(GolemTrader);
        let mut t = ScriptTest::new(registry, NPC_DEFS, "", &[]);
        let golem = t.spawn(GOLEM_TRADER, 20);
        t.world.script_drain();
        t.wait_ms(170_000);
        assert!(t.in_world(golem));
        t.wait_ms(11_000);
        assert!(!t.in_world(golem));
    }
}
