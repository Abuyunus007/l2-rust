//! Naia Lock AI: removes the minions after the master's death.
// Java: dist/game/data/scripts/ai/areas/Hellbound/AI/NaiaLock.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const LOCK: i32 = 18491;

/// Java `ai.areas.Hellbound.AI.NaiaLock`.
pub struct NaiaLock;

impl Script for NaiaLock {
    fn name(&self) -> &'static str {
        "NaiaLock"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Hellbound/AI"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_kill_id(&[LOCK]);
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _killer: ObjectId, _is_summon: bool) {
        // `getMinionList().onMasterDie(true)`: the minions are deleted.
        let minions: Vec<ObjectId> = ctx.world().minions_of(npc.object_id).to_vec();
        for minion in minions {
            ctx.world_mut().delete_npc(minion);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"<npc id="18491" type="Monster" level="80" name="Naia Lock"/>
        <npc id="18490" type="Monster" level="80" name="Lock Minion"/>"#;

    #[test]
    fn the_minions_go_with_the_lock() {
        let mut registry = ScriptRegistry::new();
        registry.add(NaiaLock);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.enter(1, 80, vec![]);
        let lock = t.spawn(LOCK, 20);
        let mut minion = None;
        t.world.run_as_script("NaiaLock", |ctx| {
            let master = ctx.script_npc(lock).unwrap();
            minion = ctx.spawn_minion(master, 18490);
        });
        let minion = minion.unwrap().object_id;
        assert!(t.in_world(minion));
        t.kill(1, lock);
        assert!(!t.in_world(minion));
    }
}
