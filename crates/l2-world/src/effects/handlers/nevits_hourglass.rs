//! The `NevitsHourglass` effect: the Nevit hourglass of the player stands still while it lasts.
// Java: dist/game/data/scripts/handlers/skill/effects/NevitsHourglass.java

use l2_data::skill::EffectTemplate;

use crate::effects::registry::{EffectCx, EffectHandler, EffectType};
use crate::world::World;
use crate::zones::ZoneId;

/// Java `NevitsHourglass`. It reads no parameters.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct NevitsHourglass;

impl NevitsHourglass {
    /// Java `NevitsHourglass(attachCond, applyCond, set, params)`.
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(NevitsHourglass))
    }
}

impl EffectHandler for NevitsHourglass {
    fn effect_type(&self) -> EffectType {
        EffectType::NevitsHourglass
    }

    /// `effected != null && effected.isPlayer()`.
    fn can_start(&self, world: &World, cx: &EffectCx<'_>) -> bool {
        world.is_player(cx.effected)
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        world.store_recommendations(cx.effected, true);
        world.set_nevit_hourglass_paused(cx.effected, true);
    }

    fn on_exit(&self, world: &mut World, cx: &EffectCx<'_>) {
        if !world.is_inside_zone(cx.effected, ZoneId::Peace) {
            world.set_nevit_hourglass_paused(cx.effected, false);
        }
    }
}
