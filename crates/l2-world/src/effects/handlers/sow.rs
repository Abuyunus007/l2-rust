//! The `Sow` effect: the seed of the player's Sowing skill is planted in the monster.
// Java: dist/game/data/scripts/handlers/skill/effects/Sow.java
// The rules (and `Attackable.setSeeded`) are `manor/farming.rs`.

use l2_data::skill::EffectTemplate;

use crate::effects::registry::{EffectCx, EffectHandler};
use crate::world::World;

/// Java `Sow`. It reads no parameters.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Sow;

impl Sow {
    /// Java `Sow(attachCond, applyCond, set, params)`.
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Sow))
    }
}

impl EffectHandler for Sow {
    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        world.manor_sow(cx.effector, cx.effected);
    }
}
