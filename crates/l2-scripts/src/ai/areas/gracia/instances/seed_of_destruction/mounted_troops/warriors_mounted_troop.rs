//! Gracia SoD Warriors Mounted Troop instance (template 125).
// Java: dist/game/data/scripts/ai/areas/Gracia/instances/SeedOfDestruction/MountedTroops/WarriorsMountedTroop.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use super::common::{self, Spec};

const DEFEATED_TROOPS_WHITE_DRAGON_LEADER: i32 = 18785;
const DEFEATED_TROOPS_INFANTRY: i32 = 18786;
const DEFEATED_TROOPS_HEALER: i32 = 18790;
const DEFEATED_TROOPS_BATALION_COMMANDER: i32 = 18784;
const DEFEATED_TROOPS_COMMANDER: i32 = 18783;
const DEFEATED_TROOPS_MAGICIAN: i32 = 18788;

const SPEC: Spec = Spec {
    name: "WarriorsMountedTroop",
    template_id: 125,
    doorman: DEFEATED_TROOPS_WHITE_DRAGON_LEADER,
    hall_1_spawns: &[
        (DEFEATED_TROOPS_WHITE_DRAGON_LEADER, 1),
        (DEFEATED_TROOPS_INFANTRY, 3),
        (DEFEATED_TROOPS_HEALER, 2),
        (DEFEATED_TROOPS_BATALION_COMMANDER, 3),
        (DEFEATED_TROOPS_COMMANDER, 1),
    ],
    hall_2_spawns: &[
        (DEFEATED_TROOPS_BATALION_COMMANDER, 3),
        (DEFEATED_TROOPS_MAGICIAN, 3),
        (DEFEATED_TROOPS_INFANTRY, 1),
    ],
    difficulty: 3,
};

/// Java `ai.areas.Gracia.instances.SeedOfDestruction.MountedTroops.WarriorsMountedTroop`.
pub struct WarriorsMountedTroop;

impl Script for WarriorsMountedTroop {
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
                registry.add(WarriorsMountedTroop);
                registry.add(DragonkinRemnants);
                registry
            },
            SPEC.name,
            &SPEC,
        );
    }
}
