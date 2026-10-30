//! Shadai: moves with the day and the night.
// Java: dist/game/data/scripts/ai/areas/Hellbound/AI/NPC/Shadai/Shadai.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, World};

// NPCs
const SHADAI: i32 = 32347;

// Locations
const DAY_COORDS: (i32, i32, i32) = (16882, 238952, 9776);
const NIGHT_COORDS: (i32, i32, i32) = (9064, 253037, -1928);

/// Where Shadai must be when he stands at `x`, if he has to move: the night place at night, the
/// day place by day.
fn coords_to_revalidate(x: i32, is_night: bool) -> Option<(i32, i32, i32)> {
    if x != NIGHT_COORDS.0 && is_night {
        Some(NIGHT_COORDS)
    } else if x != DAY_COORDS.0 && !is_night {
        Some(DAY_COORDS)
    } else {
        None
    }
}

/// Java `ai.areas.Hellbound.AI.NPC.Shadai.Shadai`.
pub struct Shadai;

impl Script for Shadai {
    fn name(&self) -> &'static str {
        "Shadai"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Hellbound/AI/NPC/Shadai"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_spawn_id(&[SHADAI]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        if event == "VALIDATE_POS"
            && let Some(npc) = npc
        {
            let x = ctx.world().position_of(npc.object_id).map(|p| p.0)?;
            if let Some((cx, cy, cz)) = coords_to_revalidate(x, World::is_night_now()) {
                ctx.world_mut().set_npc_spawn_point(npc.object_id, cx, cy, cz);
                ctx.tele_npc_to(npc, cx, cy, cz);
            }
        }

        None
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.start_quest_timer("VALIDATE_POS", 60000, Some(npc), None, true);
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::ai::areas::hellbound::testing::spawn;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"<npc id="32347" type="Folk" level="70" name="Shadai"/>"#;

    #[test]
    fn he_moves_to_the_place_of_the_time_of_day() {
        assert_eq!(coords_to_revalidate(0, true), Some(NIGHT_COORDS));
        assert_eq!(coords_to_revalidate(0, false), Some(DAY_COORDS));
        assert_eq!(coords_to_revalidate(NIGHT_COORDS.0, true), None);
        assert_eq!(coords_to_revalidate(DAY_COORDS.0, false), None);
        // At the night place by day: the day place.
        assert_eq!(coords_to_revalidate(NIGHT_COORDS.0, false), Some(DAY_COORDS));
    }

    #[test]
    fn the_validation_repeats_every_minute_and_moves_him() {
        let mut registry = ScriptRegistry::new();
        registry.add(Shadai);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.enter(1, 80, vec![]);
        let shadai = spawn(&mut t, SHADAI, 20);
        t.world.run_as_script("Shadai", |ctx| {
            assert!(ctx.has_quest_timer("VALIDATE_POS", ctx.script_npc(shadai), None));
        });
        t.wait_ms(60_200);
        let (x, y, _) = t.world.position_of(shadai).unwrap();
        // Wherever the world clock is, one of the two places.
        assert!(
            (x, y) == (DAY_COORDS.0, DAY_COORDS.1) || (x, y) == (NIGHT_COORDS.0, NIGHT_COORDS.1),
            "{x} {y}"
        );
        t.world.run_as_script("Shadai", |ctx| {
            assert!(ctx.has_quest_timer("VALIDATE_POS", ctx.script_npc(shadai), None));
        });
    }
}
