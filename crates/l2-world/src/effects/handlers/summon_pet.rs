//! The `SummonPet` effect: the caster calls the pet of the collar it used.
// Java: dist/game/data/scripts/handlers/skill/effects/SummonPet.java

use l2_data::skill::EffectTemplate;

use crate::effects::registry::{EffectCx, EffectHandler, EffectType};
use crate::world::World;

/// Java `SummonPet`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SummonPet;

impl SummonPet {
    /// Java `SummonPet(attachCond, applyCond, set, params)`: no parameters.
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self))
    }
}

impl EffectHandler for SummonPet {
    fn effect_type(&self) -> EffectType {
        EffectType::SummonPet
    }

    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        // `effector == null || effected == null || !effector.isPlayer() || !effected.isPlayer() ||
        // effected.isAlikeDead()`.
        if !world.is_player(cx.effector) || !world.is_player(cx.effected) || world.is_alike_dead(cx.effected)
        {
            return;
        }
        world.summon_pet_effect(cx.effector);
    }
}
