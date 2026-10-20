//! The `ThrowUp` effect: the target is thrown away from the caster, past it by `FlyRadius` and
//! the height difference, and stunned while it flies.
// Java: dist/game/data/scripts/handlers/skill/effects/ThrowUp.java

use l2_data::skill::EffectTemplate;
use l2_geo::Location;
use tracing::info;

use super::fly::{set_position_validated, valid_location};
use crate::effects::effect_flag;
use crate::effects::registry::{EffectCx, EffectHandler};
use crate::world::World;

/// Java `ThrowUp`. It reads no parameters.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ThrowUp;

impl ThrowUp {
    /// Java `ThrowUp(attachCond, applyCond, set, params)`.
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(ThrowUp))
    }
}

impl EffectHandler for ThrowUp {
    fn effect_flags(&self) -> u32 {
        effect_flag::STUNNED
    }

    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        let (Some((cur_x, cur_y, cur_z)), Some((ex, ey, ez))) =
            (world.position_of(cx.effected), world.position_of(cx.effector))
        else {
            return;
        };
        let dx = f64::from(ex - cur_x);
        let dy = f64::from(ey - cur_y);
        let distance = (dx * dx + dy * dy).sqrt();
        if distance > 2000.0 {
            info!(
                "EffectThrow was going to use invalid coordinates for characters, getEffected: {cur_x},{cur_y} and getEffector: {ex},{ey}"
            );
            return;
        }
        let dz = f64::from(ez - cur_z);
        // Java: `int offset = Math.min((int) distance + skill.getFlyRadius(), 1400);`, then
        // `offset += Math.abs(dz)` (an int that takes a double: the sum is truncated).
        let mut offset = std::cmp::min((distance as i32) + cx.skill.fly_radius, 1400);
        offset = (f64::from(offset) + dz.abs()) as i32;
        if offset < 5 {
            offset = 5;
        }
        if distance < 1.0 {
            return;
        }
        let sin = dy / distance;
        let cos = dx / distance;
        let x = ex - (f64::from(offset) * cos) as i32;
        let y = ey - (f64::from(offset) * sin) as i32;
        let destination = valid_location(
            world,
            cx.effected,
            Location::new(cur_x, cur_y, cur_z),
            Location::new(x, y, cur_z),
        );
        // NOTE: Java broadcasts `FlyToLocation` (`THROW_UP`) here (see `fly.rs`).
        set_position_validated(world, cx.effected, destination);
    }
}
