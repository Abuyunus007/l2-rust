//! The `Ride` effect: an instant effect that mounts the player on the NPC `npcId`.
// Java: dist/game/data/scripts/handlers/skill/effects/Ride.java

use l2_data::skill::EffectTemplate;

use super::statset::get_int;
use crate::effects::registry::{EffectCx, EffectHandler};
use crate::world::World;

/// Java `Ride`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Ride {
    npc_id: i32,
}

impl Ride {
    /// Java `Ride(attachCond, applyCond, set, params)`: `npcId` (default 0).
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Ride {
            npc_id: get_int(template, "npcId", 0)?,
        }))
    }
}

impl EffectHandler for Ride {
    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        world.ride_effect(cx.effected, self.npc_id);
    }
}
