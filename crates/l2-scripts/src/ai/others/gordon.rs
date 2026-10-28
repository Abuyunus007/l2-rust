//! Gordon (grand boss 29095): attacks a player who carries a cursed weapon and never goes home.
// Java: dist/game/data/scripts/ai/others/Gordon.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

const GORDON: i32 = 29095;

/// Java `ai.others.Gordon`.
pub struct Gordon;

impl Script for Gordon {
    fn name(&self) -> &'static str {
        "Gordon"
    }

    fn path(&self) -> &'static str {
        "ai/others"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_spawn_id(&[GORDON]);
        s.add_creature_see_id(&[GORDON]);
    }

    // The engine does not fire `on_creature_see` yet (`CreatureSeeTaskManager`).
    fn on_creature_see(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, creature: ObjectId) {
        if ctx.world().player(creature).is_some() && ctx.is_cursed_weapon_equipped(creature) {
            ctx.add_attack_desire(npc, creature, 999);
        }
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.set_can_return_to_spawn_point(npc, false);
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"<npc id="29095" type="GrandBoss" level="80" name="Gordon"><stats><vitals hp="1000" mp="100"/></stats></npc>"#;

    fn setup() -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(Gordon);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        let boss = t.spawn(GORDON, 100);
        t.enter(1, 80, vec![]);
        t.take();
        (t, boss)
    }

    #[test]
    fn gordon_does_not_return_to_its_spawn_point() {
        let (mut t, boss) = setup();
        // The spawn hook ran when Gordon entered the world.
        t.world.script_drain();
        assert!(t.world.npc_cannot_return_to_spawn_point(boss));
    }

    #[test]
    fn a_player_without_a_cursed_weapon_is_left_alone() {
        let (mut t, boss) = setup();
        t.world.script_on_creature_see(boss, ObjectId(1));
        t.world.script_drain();
        assert!(t.world.aggro_list(boss).is_empty());
    }
}
