//! `ImmobileBuff`: a buff under which the creature cannot move.
// Java: dist/game/data/scripts/handlers/skill/effects/ImmobileBuff.java

use l2_data::skill::EffectTemplate;

use crate::effects::registry::{EffectCx, EffectHandler, EffectType};
use crate::world::World;

/// Java `ImmobileBuff extends Buff`.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ImmobileBuff;

impl ImmobileBuff {
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(ImmobileBuff))
    }
}

impl EffectHandler for ImmobileBuff {
    fn effect_type(&self) -> EffectType {
        EffectType::Buff
    }

    /// `Buff.onActionTime`, inherited.
    fn on_action_time(&self, _world: &mut World, cx: &EffectCx<'_>) -> bool {
        cx.skill.is_passive() || cx.skill.is_toggle()
    }

    fn on_exit(&self, world: &mut World, cx: &EffectCx<'_>) {
        world.set_immobilized(cx.effected, false);
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        world.set_immobilized(cx.effected, true);
    }
}

// Java: dist/game/data/scripts/handlers/skill/effects/ImmobilePetBuff.java
/// Java `ImmobilePetBuff`: the owner's skill that keeps its own summon in place.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ImmobilePetBuff;

impl ImmobilePetBuff {
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(ImmobilePetBuff))
    }
}

impl EffectHandler for ImmobilePetBuff {
    fn effect_type(&self) -> EffectType {
        EffectType::Buff
    }

    fn on_exit(&self, world: &mut World, cx: &EffectCx<'_>) {
        world.set_immobilized(cx.effected, false);
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if world.is_summon(cx.effected)
            && world.is_player(cx.effector)
            && world.summon_owner(cx.effected) == Some(cx.effector)
        {
            world.set_immobilized(cx.effected, true);
        }
    }
}
