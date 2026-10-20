//! The `Blink` effect: the creature jumps `FlyRadius` away from where it faces, on the `FlyCourse`
//! of the skill, as far as the geodata lets it.
// Java: dist/game/data/scripts/handlers/skill/effects/Blink.java

use l2_data::skill::EffectTemplate;
use l2_geo::Location;

use super::fly::{set_position_validated, valid_location};
use crate::effects::registry::{EffectCx, EffectHandler};
use crate::movement::heading_to_degree;
use crate::world::World;

/// Java `LocationUtil`'s `Math.toRadians` factor (`Math.toRadians` multiplies by it).
const DEGREES_TO_RADIANS: f64 = 0.017_453_292_519_943_295;

/// Java `Blink`. It reads no parameters.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Blink;

impl Blink {
    /// Java `Blink(attachCond, applyCond, set, params)`.
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Blink))
    }
}

impl EffectHandler for Blink {
    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        let Some((x, y, z)) = world.position_of(cx.effected) else {
            return;
        };
        let radius = cx.skill.fly_radius;
        let angle = heading_to_degree(world.heading_of(cx.effected));
        let radian = angle * DEGREES_TO_RADIANS;
        let course = f64::from(cx.skill.fly_course) * DEGREES_TO_RADIANS;
        let x1 = ((std::f64::consts::PI + radian + course).cos() * f64::from(radius)) as i32;
        let y1 = ((std::f64::consts::PI + radian + course).sin() * f64::from(radius)) as i32;
        let destination = valid_location(
            world,
            cx.effected,
            Location::new(x, y, z),
            Location::new(x + x1, y + y1, z),
        );
        world.set_intention_idle(cx.effected);
        // NOTE: Java broadcasts `FlyToLocation` here (see `fly.rs`).
        world.abort_attack(cx.effected);
        world.abort_cast(cx.effected);
        set_position_validated(world, cx.effected, destination);
    }
}
