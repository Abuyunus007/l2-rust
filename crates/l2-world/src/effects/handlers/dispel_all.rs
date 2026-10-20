//! The `DispelAll` effect: every buff, dance, toggle and debuff of the creature goes (and comes
//! back later when `CancelReturn` is on).
// Java: dist/game/data/scripts/handlers/skill/effects/DispelAll.java

use l2_data::skill::EffectTemplate;

use crate::effects::SkillFinishType;
use crate::effects::cancel_return::TakenBuff;
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

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if !world.cancel_returns(cx.effector, cx.effected) {
            world.stop_all_effects(cx.effected);
            return;
        }
        let taken: Vec<TakenBuff> = world
            .effect_list(cx.effected)
            .map(|list| list.effects().map(TakenBuff::of).collect())
            .unwrap_or_default();
        for buff in &taken {
            world.stop_skill_effects(cx.effected, SkillFinishType::Removed, buff.skill.id);
        }
        // A monster gets its buffs back too.
        world.schedule_cancel_return(cx.effected, true, taken);
    }
}
