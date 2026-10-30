//! The Ragna Orc Commander calls its private minions when it spawns.
// Java: dist/game/data/scripts/ai/areas/DenOfEvil/RagnaOrcCommander.java

use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

const RAGNA_ORC_COMMANDER: i32 = 22694;

/// Java `ai.areas.DenOfEvil.RagnaOrcCommander`.
pub struct RagnaOrcCommander;

impl Script for RagnaOrcCommander {
    fn name(&self) -> &'static str {
        "RagnaOrcCommander"
    }

    fn path(&self) -> &'static str {
        "ai/areas/DenOfEvil"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_spawn_id(&[RAGNA_ORC_COMMANDER]);
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        for minion_id in ctx.world().npc_template_minion_ids(npc.object_id, "Privates1") {
            ctx.spawn_minion(npc, minion_id);
        }
        let list = if ctx.get_random_boolean() {
            "Privates2"
        } else {
            "Privates3"
        };
        for minion_id in ctx.world().npc_template_minion_ids(npc.object_id, list) {
            ctx.spawn_minion(npc, minion_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"<npc id="22694" type="Monster" level="70" name="Ragna Orc Commander">
          <parameters>
            <minions name="Privates1"><npc id="22701" count="1" respawnTime="60"/></minions>
            <minions name="Privates2"><npc id="22702" count="1" respawnTime="60"/></minions>
            <minions name="Privates3"><npc id="22703" count="1" respawnTime="60"/></minions>
          </parameters>
          <stats><vitals hp="1000" mp="100"/></stats></npc>
        <npc id="22701" type="Monster" level="70" name="A"><stats><vitals hp="100" mp="100"/></stats></npc>
        <npc id="22702" type="Monster" level="70" name="B"><stats><vitals hp="100" mp="100"/></stats></npc>
        <npc id="22703" type="Monster" level="70" name="C"><stats><vitals hp="100" mp="100"/></stats></npc>"#;

    #[test]
    fn the_commander_spawns_the_first_list_and_one_of_the_others() {
        let mut registry = ScriptRegistry::new();
        registry.add(RagnaOrcCommander);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        let commander = t.spawn(RAGNA_ORC_COMMANDER, 100);
        t.ticks(2);
        assert_eq!(t.world.spawned_minion_count(commander), 2);
        assert_eq!(t.npcs_of(22701).len(), 1);
        assert_eq!(t.npcs_of(22702).len() + t.npcs_of(22703).len(), 1);
    }
}
