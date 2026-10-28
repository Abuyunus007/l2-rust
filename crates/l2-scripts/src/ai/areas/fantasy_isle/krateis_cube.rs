//! Krateis' Cube: the monsters of the arenas give points to the player who kills them.
// Java: dist/game/data/scripts/ai/areas/FantasyIsle/KrateisCube.java
//
// The arenas, their registration cycle and the points are the world's `minigame/krateis.rs`.

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const MONSTERS: [i32; 22] = [
    18579, 18580, 18581, 18582, 18583, 18584, 18585, 18586, 18587, 18588, 18589, 18590, 18591, 18592, 18593,
    18594, 18595, 18596, 18597, 18598, 18599, 18600,
];

/// Java `ai.areas.FantasyIsle.KrateisCube`.
pub struct KrateisCube;

impl Script for KrateisCube {
    fn name(&self) -> &'static str {
        "KrateisCube"
    }

    fn path(&self) -> &'static str {
        "ai/areas/FantasyIsle"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_kill_id(&MONSTERS);
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // `player.getKrateiArena()`: the world looks the arena up and does nothing without one.
        ctx.world_mut().krateis_add_points(killer, false);
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;

    #[test]
    fn the_monsters_are_java_s() {
        assert_eq!(MONSTERS.len(), 22);
        assert_eq!(MONSTERS[0], 18579);
        assert_eq!(MONSTERS[21], 18600);
        let mut registry = ScriptRegistry::new();
        registry.add(KrateisCube);
    }
}
