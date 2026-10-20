//! The `Teleport` effect: the creature is moved to the fixed point `x`, `y`, `z` of the skill (a
//! random offset of `MaxOffsetOnTeleport` is added, as `teleToLocation(loc, true)` does).
// Java: dist/game/data/scripts/handlers/skill/effects/Teleport.java

use l2_data::skill::EffectTemplate;

use super::params;
use crate::effects::EffectType;
use crate::effects::registry::{EffectCx, EffectHandler};
use crate::teleport::TeleportTarget;
use crate::world::World;

/// Java `Teleport`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Teleport {
    x: i32,
    y: i32,
    z: i32,
}

impl Teleport {
    /// Java `Teleport(attachCond, applyCond, set, params)`: `x`, `y` and `z` (default 0).
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self {
            x: params::int(template, "x", 0)?,
            y: params::int(template, "y", 0)?,
            z: params::int(template, "z", 0)?,
        }))
    }
}

impl EffectHandler for Teleport {
    fn effect_type(&self) -> EffectType {
        EffectType::Teleport
    }

    fn is_instant(&self) -> bool {
        true
    }

    // NOTE: Java first takes a player out of its Krateis arena or its UC state (`UC_STATE_NONE`
    // check) and gives up when it is in one. Neither exists in the world yet, so nothing is checked.
    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        let offset = world.settings.teleport.max_offset_on_teleport;
        world.tele_to_location(
            cx.effected,
            TeleportTarget::at(self.x, self.y, self.z).with_random_offset(offset),
        );
    }
}
