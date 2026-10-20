//! The `DispelBySlotProbability` effect: each listed abnormal type's buff goes with the chance
//! `rate` (rolled once per type), whatever its level.
// Java: dist/game/data/scripts/handlers/skill/effects/DispelBySlotProbability.java

use std::collections::BTreeSet;

use l2_data::skill::EffectTemplate;
use l2_data::stats::AbnormalType;

use super::dispel_by_slot::java_split;
use super::params;
use crate::effects::SkillFinishType;
use crate::effects::cancel_return::TakenBuff;
use crate::effects::registry::{EffectCx, EffectHandler, EffectType};
use crate::formulas::Rnd;
use crate::world::World;

/// Java `DispelBySlotProbability`. Java keeps a level per type too (`Short.MAX_VALUE`, so every
/// buff of the type matches); the level is not read anywhere, so only the types are kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DispelBySlotProbability {
    dispel: BTreeSet<AbnormalType>,
    rate: i32,
}

impl DispelBySlotProbability {
    /// Java `DispelBySlotProbability(attachCond, applyCond, set, params)`: `dispel` and `rate`
    /// (default 0).
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        let mut dispel = BTreeSet::new();
        if let Some(text) = params::string(template, "dispel")?.filter(|t| !t.is_empty()) {
            for stack in java_split(&text, ';') {
                let name = java_split(stack, ',').first().copied().unwrap_or_default();
                dispel.insert(AbnormalType::from_name(name).unwrap_or(AbnormalType::None));
            }
        }
        Ok(Box::new(Self {
            dispel,
            rate: params::int(template, "rate", 0)?,
        }))
    }
}

impl EffectHandler for DispelBySlotProbability {
    fn effect_type(&self) -> EffectType {
        EffectType::Dispel
    }

    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if self.dispel.is_empty() || world.is_raid(cx.effected) {
            return;
        }
        // `CancelReturn`: the taken buffs come back later.
        let returns = world.cancel_returns(cx.effector, cx.effected);
        let mut taken = Vec::new();
        for &abnormal in &self.dispel {
            if world.rng().get_int(100) >= self.rate {
                continue;
            }
            // NOTE: Java stops a transformation here (`stopTransformation(true)`) before the buff
            // of the type is taken. Transformations are M6; only the buff is taken.
            let present = world
                .effect_list(cx.effected)
                .and_then(|list| list.info_by_abnormal_type(abnormal))
                .map(TakenBuff::of);
            if let Some(buff) = present {
                if returns {
                    taken.push(buff);
                }
                world.stop_skill_effects_by_abnormal(cx.effected, SkillFinishType::Removed, abnormal);
            }
        }
        if returns && !taken.is_empty() {
            world.schedule_cancel_return(cx.effected, false, taken);
        }
    }
}
