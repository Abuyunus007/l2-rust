//! The `Buff` effect: a continuous effect whose only work is its stat functions (which the buff
//! info adds and removes for every effect). The reference handler: every other handler file
//! follows this layout.
// Java: dist/game/data/scripts/handlers/skill/effects/Buff.java

use l2_data::skill::EffectTemplate;

use crate::effects::registry::{EffectCx, EffectHandler, EffectType};
use crate::world::World;

/// Java `Buff`. It reads no parameters.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Buff;

impl Buff {
    /// Java `Buff(attachCond, applyCond, set, params)`.
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Buff))
    }
}

impl EffectHandler for Buff {
    fn effect_type(&self) -> EffectType {
        EffectType::Buff
    }

    /// A `Buff` with `ticks` keeps ticking only on a passive or toggle skill; a toggle whose tick
    /// says `false` is switched off (`BuffInfo.onTick`).
    fn on_action_time(&self, _world: &mut World, cx: &EffectCx<'_>) -> bool {
        cx.skill.is_passive() || cx.skill.is_toggle()
    }
}
