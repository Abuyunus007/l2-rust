//! The Chimeras of Hellbound: a Magic Bottle on a nearly dead one gives life force.
// Java: dist/game/data/scripts/ai/areas/Hellbound/AI/Chimeras.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

use crate::ai::areas::hellbound::engine;

// NPCs
const NPCS: [i32; 4] = [
    22349, // Chimera of Earth
    22350, // Chimera of Darkness
    22351, // Chimera of Wind
    22352, // Chimera of Fire
];
const CELTUS: i32 = 22353;

// Locations
const LOCATIONS: [(i32, i32, i32); 4] = [
    (3678, 233418, -3319),
    (2038, 237125, -3363),
    (7222, 240617, -2033),
    (9969, 235570, -1993),
];

// Skills
const BOTTLE: i32 = 2359; // Magic Bottle

// Items
const DIM_LIFE_FORCE: i32 = 9680;
const LIFE_FORCE: i32 = 9681;
const CONTAINED_LIFE_FORCE: i32 = 9682;

/// Java `ai.areas.Hellbound.AI.Chimeras`.
pub struct Chimeras;

impl Script for Chimeras {
    fn name(&self) -> &'static str {
        "Chimeras"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Hellbound/AI"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_skill_see_id(&NPCS);
        s.add_spawn_id(&[CELTUS]);
        s.add_skill_see_id(&[CELTUS]);
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        if engine::level(ctx) == 7 {
            // Have random spawn points only in 7 level
            let Some((x, y, z)) = ctx.get_random_entry(&LOCATIONS) else {
                return;
            };
            let near = ctx
                .world()
                .position_of(npc.object_id)
                .is_some_and(|(nx, ny, _)| {
                    let (dx, dy) = (i64::from(nx - x), i64::from(ny - y));
                    dx * dx + dy * dy <= 200 * 200
                });
            if !near {
                ctx.world_mut().set_npc_spawn_point(npc.object_id, x, y, z);
                // `ThreadPool.schedule(new Teleport(npc, loc), 100)`.
                ctx.start_quest_timer(&format!("teleport {x} {y} {z}"), 100, Some(npc), None, false);
            }
        }
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        if let Some(coordinates) = event.strip_prefix("teleport ") {
            let npc = npc?;
            let mut it = coordinates.split(' ').filter_map(|v| v.parse::<i32>().ok());
            if let (Some(x), Some(y), Some(z)) = (it.next(), it.next(), it.next()) {
                ctx.tele_npc_to(npc, x, y, z);
            }
        }

        None
    }

    fn on_skill_see(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        caster: ObjectId,
        skill: SkillKey,
        targets: &[ObjectId],
        _is_summon: bool,
    ) {
        if skill.id == BOTTLE
            && !ctx.is_dead(npc.object_id)
            && targets.first() == Some(&npc.object_id)
            && ctx.npc_current_hp(npc) < f64::from(ctx.npc_max_hp(npc)) * 0.1
        {
            if engine::level(ctx) == 7 {
                engine::update_trust(ctx, 3, true);
            }

            // `npc.setDead(true)` only prepares `npc.onDecay()`, which `delete_npc` is.
            if npc.id == CELTUS {
                // The retail value is 1; `DeathDropAmountMultiplier` above 1 multiplies it.
                let multiplier = ctx.script_settings().rate_death_drop_amount_multiplier;
                let amount = if multiplier > 1.0 { multiplier as i64 } else { 1 };
                ctx.drop_item(npc, Some(caster), CONTAINED_LIFE_FORCE, amount);
            } else if ctx.get_random(100) < 80 {
                ctx.drop_item(npc, Some(caster), DIM_LIFE_FORCE, 1);
            } else if ctx.get_random(100) < 80 {
                ctx.drop_item(npc, Some(caster), LIFE_FORCE, 1);
            }

            ctx.world_mut().delete_npc(npc.object_id);
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

    const NPCS: &str = r#"<npc id="22353" type="Monster" level="60" name="Celtus"/>
        <npc id="22349" type="Monster" level="60" name="Chimera of Earth"/>"#;

    fn setup() -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(HellboundEngine);
        registry.add(Chimeras);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.enter(1, 80, vec![]);
        t
    }

    #[test]
    fn celtus_moves_to_a_random_point_at_level_seven_only() {
        let mut t = setup();
        let celtus = crate::ai::areas::hellbound::testing::spawn(&mut t, 22353, 20);
        t.wait_ms(300);
        let (x, _, _) = t.world.position_of(celtus).unwrap();
        assert_eq!(x, crate::testing::X + 20);

        set_level(&mut t, "Chimeras", 7);
        let celtus = crate::ai::areas::hellbound::testing::spawn(&mut t, 22353, 30);
        t.wait_ms(300);
        let at = t.world.position_of(celtus).unwrap();
        // The height is the world's (a few units off the table's).
        assert!(LOCATIONS.iter().any(|l| (l.0, l.1) == (at.0, at.1)), "{at:?}");
    }

    #[test]
    fn a_bottle_on_a_healthy_chimera_does_nothing() {
        let mut t = setup();
        let chimera = crate::ai::areas::hellbound::testing::spawn(&mut t, 22349, 20);
        t.world.run_as_script("Chimeras", |ctx| {
            let npc = ctx.script_npc(chimera).unwrap();
            Chimeras.on_skill_see(ctx, npc, ObjectId(1), SkillKey::new(BOTTLE, 1), &[chimera], false);
        });
        assert!(t.in_world(chimera));
    }
}
