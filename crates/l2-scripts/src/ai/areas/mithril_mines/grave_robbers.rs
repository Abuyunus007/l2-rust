//! The Grave Robber summoners and magicians call one of their two private lists when they spawn.
// Java: dist/game/data/scripts/ai/areas/MithrilMines/GraveRobbers.java

use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

const GRAVE_ROBBER_SUMMONER: i32 = 22678;
const GRAVE_ROBBER_MEGICIAN: i32 = 22679;

/// Java `ai.areas.MithrilMines.GraveRobbers`.
pub struct GraveRobbers;

impl Script for GraveRobbers {
    fn name(&self) -> &'static str {
        "GraveRobbers"
    }

    fn path(&self) -> &'static str {
        "ai/areas/MithrilMines"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_spawn_id(&[GRAVE_ROBBER_SUMMONER, GRAVE_ROBBER_MEGICIAN]);
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        let list = format!("Privates{}", ctx.get_random_range(1, 2));
        for minion_id in ctx.world().npc_template_minion_ids(npc.object_id, &list) {
            ctx.spawn_minion(npc, minion_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"<npc id="22678" type="Monster" level="70" name="Grave Robber Summoner">
          <parameters>
            <minions name="Privates1"><npc id="22701" count="1" respawnTime="60"/></minions>
            <minions name="Privates2"><npc id="22702" count="1" respawnTime="60"/></minions>
          </parameters>
          <stats><vitals hp="1000" mp="100"/></stats></npc>
        <npc id="22701" type="Monster" level="70" name="A"><stats><vitals hp="100" mp="100"/></stats></npc>
        <npc id="22702" type="Monster" level="70" name="B"><stats><vitals hp="100" mp="100"/></stats></npc>"#;

    #[test]
    fn the_robber_spawns_the_minions_of_one_list() {
        let mut registry = ScriptRegistry::new();
        registry.add(GraveRobbers);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        let robber = t.spawn(GRAVE_ROBBER_SUMMONER, 100);
        t.ticks(2);
        assert_eq!(t.world.spawned_minion_count(robber), 1);
        assert_eq!(t.npcs_of(22701).len() + t.npcs_of(22702).len(), 1);
    }
}
