//! The `MpConsumePerLevel` effect: every tick takes `power` times the ticks multiplier times the
//! level factor of MP from the target; when there is not enough, the skill is switched off.
// Java: dist/game/data/scripts/handlers/skill/effects/MpConsumePerLevel.java

use l2_data::skill::EffectTemplate;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;

use super::params;
use crate::effects::registry::{EffectCx, EffectHandler};
use crate::world::World;

/// Java `MpConsumePerLevel`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MpConsumePerLevel {
    power: f64,
}

impl MpConsumePerLevel {
    /// Java `MpConsumePerLevel(attachCond, applyCond, set, params)`: `power` (default 0).
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self {
            power: params::double(template, "power", 0.0)?,
        }))
    }
}

impl EffectHandler for MpConsumePerLevel {
    fn on_action_time(&self, world: &mut World, cx: &EffectCx<'_>) -> bool {
        if world.is_dead(cx.effected) {
            return false;
        }
        let level = world.stats(cx.effected).map_or(0, |v| v.level());
        let consume = self.power * cx.ticks_multiplier(world) * (f64::from(level - 1) / 7.5);
        let current_mp = world.core_of(cx.effected).map_or(0.0, |c| c.cur_mp);
        if consume > current_mp {
            world.send(
                cx.effected,
                &SystemMessage::new(SystemMessageId::YourSkillWasDeactivatedDueToLackOfMp),
            );
            return false;
        }
        world.reduce_mp(cx.effected, consume);
        cx.skill.is_toggle()
    }
}
