//! The `Harvesting` effect: the seeder takes the crop from the dead monster.
// Java: dist/game/data/scripts/handlers/skill/effects/Harvesting.java
// The rules are `manor/farming.rs`.

use l2_data::skill::EffectTemplate;

use crate::effects::registry::{EffectCx, EffectHandler};
use crate::world::World;

/// Java `Harvesting`. It reads no parameters.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Harvesting;

impl Harvesting {
    /// Java `Harvesting(attachCond, applyCond, set, params)`.
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Harvesting))
    }
}

impl EffectHandler for Harvesting {
    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        world.manor_harvest(cx.effector, cx.effected);
    }
}
