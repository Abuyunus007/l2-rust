//! Gracia SoD Soldiers Mounted Troop instance (template 124).
// Java: dist/game/data/scripts/ai/areas/Gracia/instances/SeedOfDestruction/MountedTroops/SoldiersMountedTroop.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use super::common::{self, Spec};

const DEFEATED_TROOPS_WHITE_DRAGON_LEADER: i32 = 18785;
const DEFEATED_TROOPS_INFANTRY: i32 = 18786;
const DEFEATED_TROOPS_MAGIC_SOLDIER: i32 = 18789;

const SPEC: Spec = Spec {
    name: "SoldiersMountedTroop",
    template_id: 124,
    doorman: DEFEATED_TROOPS_INFANTRY,
    hall_1_spawns: &[
        (DEFEATED_TROOPS_WHITE_DRAGON_LEADER, 4),
        (DEFEATED_TROOPS_INFANTRY, 3),
        (DEFEATED_TROOPS_MAGIC_SOLDIER, 3),
    ],
    hall_2_spawns: &[(DEFEATED_TROOPS_INFANTRY, 4), (DEFEATED_TROOPS_MAGIC_SOLDIER, 3)],
    difficulty: 2,
};

/// Java `ai.areas.Gracia.instances.SeedOfDestruction.MountedTroops.SoldiersMountedTroop`.
pub struct SoldiersMountedTroop;

impl Script for SoldiersMountedTroop {
    fn name(&self) -> &'static str {
        SPEC.name
    }

    fn path(&self) -> &'static str {
        common::PATH
    }

    fn setup(&self, s: &mut ScriptSetup) {
        common::setup(&SPEC, s);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        common::on_event(&SPEC, ctx, event, npc, player)
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        common::on_kill(&SPEC, ctx, npc, killer);
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::super::common::tests::{DragonkinRemnants, run};
    use super::*;

    #[test]
    fn the_party_enters_and_the_machine_ends_it() {
        run(
            || {
                let mut registry = ScriptRegistry::new();
                registry.add(SoldiersMountedTroop);
                registry.add(DragonkinRemnants);
                registry
            },
            SPEC.name,
            &SPEC,
        );
    }
}
