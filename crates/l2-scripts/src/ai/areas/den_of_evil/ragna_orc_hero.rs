//! The Ragna Orc Hero calls its private minions when it spawns.
// Java: dist/game/data/scripts/ai/areas/DenOfEvil/RagnaOrcHero.java

use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

const RAGNA_ORC_HERO: i32 = 22693;

/// Java `ai.areas.DenOfEvil.RagnaOrcHero`.
pub struct RagnaOrcHero;

impl Script for RagnaOrcHero {
    fn name(&self) -> &'static str {
        "RagnaOrcHero"
    }

    fn path(&self) -> &'static str {
        "ai/areas/DenOfEvil"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_spawn_id(&[RAGNA_ORC_HERO]);
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        let list = if ctx.get_random(100) < 70 {
            "Privates1"
        } else {
            "Privates2"
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

    const NPCS: &str = r#"<npc id="22693" type="Monster" level="70" name="Ragna Orc Hero">
          <parameters>
            <minions name="Privates1"><npc id="22701" count="1" respawnTime="60"/></minions>
            <minions name="Privates2"><npc id="22702" count="1" respawnTime="60"/></minions>
          </parameters>
          <stats><vitals hp="1000" mp="100"/></stats></npc>
        <npc id="22701" type="Monster" level="70" name="A"><stats><vitals hp="100" mp="100"/></stats></npc>
        <npc id="22702" type="Monster" level="70" name="B"><stats><vitals hp="100" mp="100"/></stats></npc>"#;

    #[test]
    fn the_hero_spawns_the_minions_of_one_list() {
        let mut registry = ScriptRegistry::new();
        registry.add(RagnaOrcHero);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        let hero = t.spawn(RAGNA_ORC_HERO, 100);
        t.ticks(2);
        assert_eq!(t.world.spawned_minion_count(hero), 1);
        assert_eq!(t.npcs_of(22701).len() + t.npcs_of(22702).len(), 1);
    }
}
