//! The town pets wander around their spawn point.
// Java: dist/game/data/scripts/ai/others/TownPets/TownPets.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const PETS: [i32; 12] = [
    31202, // Maximus
    31203, // Moon Dancer
    31204, // Georgio
    31205, // Katz
    31206, // Ten Ten
    31207, // Sardinia
    31208, // La Grange
    31209, // Misty Rain
    31266, // Kaiser
    31593, // Dorothy
    31758, // Rafi
    31955, // Ruby
];

// Misc
const MOVE_INTERVAL: i64 = 5000;

/// Java `ai.others.TownPets.TownPets`.
pub struct TownPets;

impl Script for TownPets {
    fn name(&self) -> &'static str {
        "TownPets"
    }

    fn path(&self) -> &'static str {
        "ai/others/TownPets"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_spawn_id(&PETS);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        if let Some(npc) = npc
            && ctx.world().is_object_spawned(npc.object_id)
            && event == "move"
        {
            if ctx.world().npc_region_is_active(npc.object_id) {
                // `npc.getSpawn().getX()`: where the spawn is.
                if let Some((sx, sy, _)) = ctx.npc_spawn_point(npc) {
                    let loc_x = sx + ctx.get_random_range(-50, 50);
                    let loc_y = sy + ctx.get_random_range(-50, 50);
                    if let Some((mx, my, mz)) = ctx.world().npc_valid_location_toward(npc.object_id, loc_x, loc_y)
                        && !ctx.is_inside_radius_3d(npc, mx, my, mz, 20)
                    {
                        ctx.npc_move_to(npc, mx, my, mz);
                    }
                }

                ctx.start_quest_timer("move", MOVE_INTERVAL, Some(npc), None, false);
            } else {
                ctx.start_quest_timer("move", MOVE_INTERVAL * 3, Some(npc), None, false);
            }
        }

        None
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.set_running(npc);
        ctx.start_quest_timer("move", MOVE_INTERVAL, Some(npc), None, false);
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPC_DEFS: &str = r#"<npc id="31202" type="Folk" level="1" name="Maximus"/>"#;

    #[test]
    fn a_pet_keeps_its_move_timer_going() {
        let mut registry = ScriptRegistry::new();
        registry.add(TownPets);
        let mut t = ScriptTest::new(registry, NPC_DEFS, "", &[]);
        let pet = t.spawn(31202, 20);
        t.enter(1, 10, vec![]);
        t.world.script_drain();
        assert_eq!(t.world.quest_timer_count(), 1);
        t.wait_ms(5_500);
        // The event ran and started the next one (5 s when the region is active, 15 s if not).
        assert_eq!(t.world.quest_timer_count(), 1);
        assert!(t.world.is_object_spawned(pet));
    }
}
