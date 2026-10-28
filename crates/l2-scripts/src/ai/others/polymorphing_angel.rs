//! A killed Angel turns into the next one.
// Java: dist/game/data/scripts/ai/others/PolymorphingAngel.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

/// Java `ANGELSPAWNS`: killed id -> spawned id.
const ANGELSPAWNS: [(i32, i32); 5] = [
    (20830, 20859),
    (21067, 21068),
    (21062, 21063),
    (20831, 20860),
    (21070, 21071),
];

/// Java `ai.others.PolymorphingAngel`.
pub struct PolymorphingAngel;

impl Script for PolymorphingAngel {
    fn name(&self) -> &'static str {
        "PolymorphingAngel"
    }

    fn path(&self) -> &'static str {
        "ai/others"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        let ids: Vec<i32> = ANGELSPAWNS.iter().map(|&(k, _)| k).collect();
        s.add_kill_id(&ids);
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _killer: ObjectId, _is_summon: bool) {
        let Some(&(_, spawn)) = ANGELSPAWNS.iter().find(|&&(k, _)| k == npc.id) else {
            return;
        };
        if let Some(new_npc) = ctx.add_spawn_at(spawn, npc.object_id, false, 0) {
            ctx.set_running(new_npc);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPC_DEFS: &str = r#"<npc id="20830" type="Monster" level="50" name="Angel"><stats><vitals hp="10" mp="100"/></stats></npc>
        <npc id="20859" type="Monster" level="50" name="Angel 2"><stats><vitals hp="1000" mp="100"/></stats></npc>"#;

    #[test]
    fn a_killed_angel_is_replaced() {
        let mut registry = ScriptRegistry::new();
        registry.add(PolymorphingAngel);
        let mut t = ScriptTest::new(registry, NPC_DEFS, "", &[]);
        let angel = t.spawn(20830, 20);
        t.enter(1, 60, vec![]);
        t.kill(1, angel);
        let spawned = t.npcs_of(20859);
        assert_eq!(spawned.len(), 1);
        t.world.delete_npc(spawned[0]);
    }
}
