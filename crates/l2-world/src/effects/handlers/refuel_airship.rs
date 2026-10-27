//! The `RefuelAirship` effect: the fuel of the air ship the caster rides goes up.
// Java: dist/game/data/scripts/handlers/skill/effects/RefuelAirship.java

use l2_data::skill::EffectTemplate;

use super::params;
use crate::effects::registry::{EffectCx, EffectHandler};
use crate::world::World;

/// Java `RefuelAirship`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RefuelAirship {
    value: i32,
}

impl RefuelAirship {
    /// Java `RefuelAirship(attachCond, applyCond, set, params)`: `value` (default 0).
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self {
            value: params::int(template, "value", 0)?,
        }))
    }
}

impl EffectHandler for RefuelAirship {
    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        world.refuel_air_ship(cx.effector, self.value);
    }
}
