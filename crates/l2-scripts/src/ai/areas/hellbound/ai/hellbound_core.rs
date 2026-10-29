//! Naia's beam on the Hellbound Core.
// Java: dist/game/data/scripts/ai/areas/Hellbound/AI/HellboundCore.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

use crate::ai::areas::hellbound::engine;

// NPCs
const NAIA: i32 = 18484;
const HELLBOUND_CORE: i32 = 32331;

// Skills
const BEAM: (i32, i32) = (5493, 1);

/// Java `ai.areas.Hellbound.AI.HellboundCore`.
pub struct HellboundCore;

impl Script for HellboundCore {
    fn name(&self) -> &'static str {
        "HellboundCore"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Hellbound/AI"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_spawn_id(&[HELLBOUND_CORE, NAIA]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        if event.eq_ignore_ascii_case("cast") && engine::level(ctx) <= 6 {
            let npc = npc?;
            for naia in ctx.world().visible_objects_in_range(npc.object_id, 900) {
                let is_naia = ctx
                    .world()
                    .npc(naia)
                    .is_some_and(|n| n.class.is_monster() && n.npc_id == NAIA);
                if is_naia && !ctx.is_dead(naia) && !ctx.world().is_channeling(naia) {
                    ctx.world_mut().set_npc_target(naia, Some(npc.object_id));
                    ctx.world_mut()
                        .do_simultaneous_cast(naia, SkillKey::new(BEAM.0, BEAM.1));
                }
            }

            ctx.start_quest_timer("cast", 10000, Some(npc), None, false);
        }

        None
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        if npc.id == NAIA {
            ctx.set_random_walking(npc, false);
        } else {
            ctx.start_quest_timer("cast", 10000, Some(npc), None, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::ai::areas::hellbound::engine::HellboundEngine;
    use crate::ai::areas::hellbound::testing::set_level;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"<npc id="18484" type="Monster" level="80" name="Naia"/>
        <npc id="32331" type="Folk" level="80" name="Hellbound Core"/>"#;

    #[test]
    fn the_core_casts_every_ten_seconds_until_level_seven() {
        let mut registry = ScriptRegistry::new();
        registry.add(HellboundEngine);
        registry.add(HellboundCore);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.enter(1, 80, vec![]);
        let naia = crate::ai::areas::hellbound::testing::spawn(&mut t, NAIA, 20);
        assert!(t.world.npc_random_walking_forbidden(naia));
        let core = crate::ai::areas::hellbound::testing::spawn(&mut t, HELLBOUND_CORE, 30);
        t.world.run_as_script("HellboundCore", |ctx| {
            assert!(ctx.has_quest_timer("cast", ctx.script_npc(core), None));
        });
        // The timer renews itself while the level is below seven.
        t.wait_ms(10_200);
        t.world.run_as_script("HellboundCore", |ctx| {
            assert!(ctx.has_quest_timer("cast", ctx.script_npc(core), None));
        });
        set_level(&mut t, "HellboundCore", 7);
        t.wait_ms(10_200);
        t.world.run_as_script("HellboundCore", |ctx| {
            assert!(!ctx.has_quest_timer("cast", ctx.script_npc(core), None));
        });
    }
}
