//! The `DispelAll` effect: every buff, dance, toggle and debuff of the creature goes.
// Java: dist/game/data/scripts/handlers/skill/effects/DispelAll.java

use l2_data::skill::EffectTemplate;

use crate::effects::registry::{EffectCx, EffectHandler, EffectType};
use crate::world::World;

/// Java `DispelAll`. It reads no parameters.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct DispelAll;

impl DispelAll {
    /// Java `DispelAll(attachCond, applyCond, set, params)`.
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(DispelAll))
    }
}

impl EffectHandler for DispelAll {
    fn effect_type(&self) -> EffectType {
        EffectType::Dispel
    }

    fn is_instant(&self) -> bool {
        true
    }

    // NOTE: Java gives the taken effects back after `CancelReturnConfig.TIME_TO_RETURN` when
    // `CancelReturn` is on. That config is not in the effect settings, so the world always takes
    // the effects for good, which is Java's answer with the default (`CancelReturn=false`).
    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        world.stop_all_effects(cx.effected);
    }
}
