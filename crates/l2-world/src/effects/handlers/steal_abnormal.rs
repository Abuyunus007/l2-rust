//! The `StealAbnormal` effect: the buffs the target of a player takes (`slot`, `rate`, `max` as in
//! `DispelByCategory`) move to the caster, with the time they have left.
// Java: dist/game/data/scripts/handlers/skill/effects/StealAbnormal.java

use l2_data::skill::{EffectScope, EffectTemplate};

use super::cancel::calc_cancel_steal_effects;
use super::params;
use crate::effects::registry::{EffectCx, EffectHandler, EffectType};
use crate::effects::{BuffInfo, SkillFinishType};
use crate::world::World;

/// Java `StealAbnormal`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StealAbnormal {
    slot: Option<String>,
    rate: i32,
    max: i32,
}

impl StealAbnormal {
    /// Java `StealAbnormal(attachCond, applyCond, set, params)`.
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self {
            slot: params::string(template, "slot")?,
            rate: params::int(template, "rate", 0)?,
            max: params::int(template, "max", 0)?,
        }))
    }
}

impl EffectHandler for StealAbnormal {
    fn effect_type(&self) -> EffectType {
        EffectType::StealAbnormal
    }

    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if !world.is_player(cx.effected) || cx.effector == cx.effected {
            return;
        }
        let to_steal = calc_cancel_steal_effects(
            world,
            cx.effector,
            cx.effected,
            cx.skill,
            self.slot.as_deref(),
            self.rate,
            self.max,
        );
        if to_steal.is_empty() {
            return;
        }
        let Some(table) = world.skill_table() else {
            return;
        };
        for info_to_steal in to_steal {
            let Some(skill) = table.get(info_to_steal.skill.id, info_to_steal.skill.level) else {
                continue;
            };
            // Invert effected and effector: the thief is the effected of the stolen buff.
            let id = world.effects.next_info_id();
            let mut stolen = BuffInfo::new(
                id,
                cx.effected,
                cx.effector,
                skill,
                info_to_steal.time,
                world.tick,
            );
            // To include all the effects, the template is applied rather than the buff info.
            world.apply_effect_scope(EffectScope::General, &mut stolen, true, true);
            world.remove_effect(cx.effected, SkillFinishType::Removed, info_to_steal.info);
            world.add_buff_info(stolen);
        }
    }
}
