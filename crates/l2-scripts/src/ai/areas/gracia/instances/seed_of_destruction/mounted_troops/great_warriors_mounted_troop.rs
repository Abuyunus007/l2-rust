//! Gracia SoD Great Warriors Mounted Troop instance (template 126).
// Java: dist/game/data/scripts/ai/areas/Gracia/instances/SeedOfDestruction/MountedTroops/GreatWarriorsMountedTroop.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use super::common::{self, Spec};

const DEFEATED_TROOPS_WHITE_DRAGON_LEADER: i32 = 18785;
const DEFEATED_TROOPS_HEALER: i32 = 18790;
const DEFEATED_TROOPS_BATALION_COMMANDER: i32 = 18784;
const DEFEATED_TROOPS_COMMANDER: i32 = 18783;
const DEFEATED_TROOPS_MAGIC_LEADER: i32 = 18787;

const SPEC: Spec = Spec {
    name: "GreatWarriorsMountedTroop",
    template_id: 126,
    doorman: DEFEATED_TROOPS_COMMANDER,
    hall_1_spawns: &[
        (DEFEATED_TROOPS_HEALER, 2),
        (DEFEATED_TROOPS_MAGIC_LEADER, 2),
        (DEFEATED_TROOPS_BATALION_COMMANDER, 2),
        (DEFEATED_TROOPS_COMMANDER, 3),
    ],
    hall_2_spawns: &[
        (DEFEATED_TROOPS_MAGIC_LEADER, 2),
        (DEFEATED_TROOPS_WHITE_DRAGON_LEADER, 5),
        (DEFEATED_TROOPS_BATALION_COMMANDER, 2),
    ],
    difficulty: 4,
};

/// Java `ai.areas.Gracia.instances.SeedOfDestruction.MountedTroops.GreatWarriorsMountedTroop`.
pub struct GreatWarriorsMountedTroop;

impl Script for GreatWarriorsMountedTroop {
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
                registry.add(GreatWarriorsMountedTroop);
                registry.add(DragonkinRemnants);
                registry
            },
            SPEC.name,
            &SPEC,
        );
    }
}
