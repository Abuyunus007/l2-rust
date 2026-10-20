//! The `TeleportToTarget` effect: the caster jumps to 25 units behind the target, as far as the
//! geodata lets it.
// Java: dist/game/data/scripts/handlers/skill/effects/TeleportToTarget.java

use l2_data::skill::EffectTemplate;
use l2_geo::Location;

use super::fly::{set_position_validated, valid_location};
use crate::effects::EffectType;
use crate::effects::registry::{EffectCx, EffectHandler};
use crate::movement::heading_to_degree;
use crate::world::World;

/// Java `TeleportToTarget`. It reads no parameters.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct TeleportToTarget;

impl TeleportToTarget {
    /// Java `TeleportToTarget(attachCond, applyCond, set, params)`.
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(TeleportToTarget))
    }
}

impl EffectHandler for TeleportToTarget {
    fn effect_type(&self) -> EffectType {
        EffectType::TeleportToTarget
    }

    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        let Some((px, py, pz)) = world.position_of(cx.effected) else {
            return;
        };
        // The point 25 units behind the target, from its heading.
        let mut ph = heading_to_degree(world.heading_of(cx.effected));
        ph += 180.0;
        if ph > 360.0 {
            ph -= 360.0;
        }
        let ph = (std::f64::consts::PI * ph) / 180.0;
        let x = (f64::from(px) + 25.0 * ph.cos()) as i32;
        let y = (f64::from(py) + 25.0 * ph.sin()) as i32;
        let z = pz;
        let Some((ex, ey, ez)) = world.position_of(cx.effector) else {
            return;
        };
        let destination = valid_location(
            world,
            cx.effector,
            Location::new(ex, ey, ez),
            Location::new(x, y, z),
        );
        world.set_intention_idle(cx.effector);
        // NOTE: Java broadcasts `FlyToLocation` here (see `fly.rs`).
        world.abort_attack(cx.effector);
        // NOTE: Java also `abortCast()` (see `blink.rs`).
        set_position_validated(world, cx.effector, destination);
    }
}
