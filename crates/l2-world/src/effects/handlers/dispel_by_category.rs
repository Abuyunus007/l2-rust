//! The `DispelByCategory` effect: takes buffs (`slot` "buff") or debuffs (`slot` "debuff") off
//! the creature, each with a chance that depends on `rate`, at most `max` of them.
// Java: dist/game/data/scripts/handlers/skill/effects/DispelByCategory.java

use l2_data::skill::EffectTemplate;

use super::cancel::calc_cancel_steal_effects;
use super::params;
use crate::effects::SkillFinishType;
use crate::effects::registry::{EffectCx, EffectHandler, EffectType};
use crate::world::World;

/// Java `DispelByCategory`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DispelByCategory {
    slot: Option<String>,
    rate: i32,
    max: i32,
}

impl DispelByCategory {
    /// Java `DispelByCategory(attachCond, applyCond, set, params)`: `slot` (default none), `rate`
    /// and `max` (both default 0).
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self {
            slot: params::string(template, "slot")?,
            rate: params::int(template, "rate", 0)?,
            max: params::int(template, "max", 0)?,
        }))
    }
}

impl EffectHandler for DispelByCategory {
    fn effect_type(&self) -> EffectType {
        EffectType::Dispel
    }

    fn is_instant(&self) -> bool {
        true
    }

    // NOTE: the `CancelReturn` branch (the taken effects come back later) is not ported, see
    // `DispelAll`; the default branch is the one below.
    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if world.is_dead(cx.effected) || world.is_raid(cx.effected) {
            return;
        }
        let canceled = calc_cancel_steal_effects(
            world,
            cx.effector,
            cx.effected,
            cx.skill,
            self.slot.as_deref(),
            self.rate,
            self.max,
        );
        for info in canceled {
            world.stop_skill_effects(cx.effected, SkillFinishType::Removed, info.skill.id);
        }
    }
}
