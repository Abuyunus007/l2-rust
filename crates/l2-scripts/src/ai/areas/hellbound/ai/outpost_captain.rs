//! The Outpost Captain: his death opens the door of the outpost and moves Hellbound to level 9.
// Java: dist/game/data/scripts/ai/areas/Hellbound/AI/OutpostCaptain.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use crate::ai::areas::hellbound::engine;

// NPCs
const CAPTAIN: i32 = 18466;
const DEFENDERS: [i32; 2] = [
    22357, // Enceinte Defender
    22358, // Enceinte Defender
];

/// The door of the outpost.
const DOOR: i32 = 20250001;

/// Java `ai.areas.Hellbound.AI.OutpostCaptain`.
pub struct OutpostCaptain;

impl Script for OutpostCaptain {
    fn name(&self) -> &'static str {
        "OutpostCaptain"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Hellbound/AI"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_kill_id(&[CAPTAIN]);
        s.add_spawn_id(&[CAPTAIN]);
        s.add_spawn_id(&DEFENDERS);
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, _killer: ObjectId, _is_summon: bool) {
        if ctx.world().door_by_door_id(DOOR).is_some() {
            ctx.open_door(DOOR, 0);
        }

        if engine::level(ctx) == 8 {
            engine::set_level(ctx, 9);
        }
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.set_random_walking(npc, false);

        if npc.id == CAPTAIN {
            let hellbound_level = engine::level(ctx);
            if !(7..=8).contains(&hellbound_level) {
                let spawn = ctx.world().npc_spawn_index(npc.object_id);
                ctx.world_mut().delete_npc(npc.object_id);
                if let Some(spawn) = spawn {
                    ctx.world_mut().stop_spawn_respawn(spawn);
                }
            } else if ctx.world().door_by_door_id(DOOR).is_some() {
                ctx.close_door(DOOR, 0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::ai::areas::hellbound::testing::set_level;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"<npc id="18466" type="Monster" level="80" name="Outpost Captain"/>
        <npc id="22357" type="Monster" level="80" name="Enceinte Defender"/>"#;

    fn setup() -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(OutpostCaptain);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.enter(1, 80, vec![]);
        t
    }

    #[test]
    fn the_captain_is_only_there_at_levels_seven_and_eight() {
        let mut t = setup();
        let captain = crate::ai::areas::hellbound::testing::spawn(&mut t, CAPTAIN, 20);
        assert!(!t.in_world(captain));
        set_level(&mut t, "OutpostCaptain", 7);
        let captain = crate::ai::areas::hellbound::testing::spawn(&mut t, CAPTAIN, 20);
        assert!(t.in_world(captain));
    }

    #[test]
    fn the_defenders_never_walk() {
        let mut t = setup();
        let defender = crate::ai::areas::hellbound::testing::spawn(&mut t, 22357, 20);
        assert!(t.world.npc_random_walking_forbidden(defender));
    }

    #[test]
    fn the_captain_s_death_moves_level_eight_to_nine() {
        let mut t = setup();
        set_level(&mut t, "OutpostCaptain", 8);
        let captain = crate::ai::areas::hellbound::testing::spawn(&mut t, CAPTAIN, 20);
        t.kill(1, captain);
        assert_eq!(t.world.global_variables().get_int("HBLevel", 0), 9);
        // At another level nothing changes.
        set_level(&mut t, "OutpostCaptain", 7);
        let captain = crate::ai::areas::hellbound::testing::spawn(&mut t, CAPTAIN, 30);
        t.kill(1, captain);
        assert_eq!(t.world.global_variables().get_int("HBLevel", 0), 7);
    }
}
