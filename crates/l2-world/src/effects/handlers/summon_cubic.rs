//! The cubic effects: `SummonCubic` (a player gets a cubic) and `CubicMastery` (how many cubics a
//! player may have at once).
// Java: dist/game/data/scripts/handlers/skill/effects/SummonCubic.java
// Java: dist/game/data/scripts/handlers/skill/effects/CubicMastery.java

use l2_data::skill::EffectTemplate;
use tracing::warn;

use super::params;
use crate::effects::registry::{EffectCx, EffectHandler};
use crate::summon::cubic::AddCubic;
use crate::world::World;

/// Java `SummonCubic`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SummonCubic {
    cubic_id: i32,
    cubic_level: i32,
    cubic_power: i32,
    cubic_duration: i32,
    cubic_delay: i32,
    cubic_max_count: i32,
    cubic_skill_chance: i32,
}

impl SummonCubic {
    /// Java `SummonCubic(attachCond, applyCond, set, params)`: `cubicId` (-1), `cubicLvl` (else
    /// `cubicSkillLevel`, else 0), `cubicPower` (0), `cubicDuration` (0), `cubicDelay` (0),
    /// `cubicMaxCount` (-1), `cubicSkillChance` (0).
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self {
            cubic_id: params::int(template, "cubicId", -1)?,
            cubic_level: params::int(template, "cubicLvl", params::int(template, "cubicSkillLevel", 0)?)?,
            cubic_power: params::int(template, "cubicPower", 0)?,
            cubic_duration: params::int(template, "cubicDuration", 0)?,
            cubic_delay: params::int(template, "cubicDelay", 0)?,
            cubic_max_count: params::int(template, "cubicMaxCount", -1)?,
            cubic_skill_chance: params::int(template, "cubicSkillChance", 0)?,
        }))
    }
}

impl EffectHandler for SummonCubic {
    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        let (effector, effected, skill) = (cx.effector, cx.effected, cx.skill);
        if !world.is_player(effected) || world.is_alike_dead(effected) {
            return;
        }
        if self.cubic_id < 0 {
            warn!(
                "SummonCubic: Invalid cubic id: {} skillId: {}",
                self.cubic_id, skill.id
            );
            return;
        }
        // `player.inObserverMode() || player.isMounted()` (the observer mode: M8-OLY-01).
        if world.in_observer_mode(effected) || world.is_mounted(effected) {
            return;
        }
        let mut template_level = if self.cubic_level > 0 {
            self.cubic_level
        } else {
            skill.level
        };
        // The enchant routes of the skill levels (101+) map to the template levels 8 and up.
        if self.cubic_level <= 0 && template_level > 100 {
            template_level = ((skill.level - 100) / 7) + 8;
        }
        let Some(template) = world.cubic_template(self.cubic_id, template_level) else {
            warn!(
                "SummonCubic: Missing cubic template. cubicId: {} level: {} skillId: {}",
                self.cubic_id, template_level, skill.id
            );
            return;
        };

        if world.cubic(effected, self.cubic_id).is_some() {
            world.remove_cubic(effected, self.cubic_id);
        } else {
            let allowed = world.max_cubic_count(effected).max(1);
            while i32::try_from(world.cubic_count(effected)).unwrap_or(i32::MAX) >= allowed {
                if world.remove_first_cubic(effected).is_none() {
                    break;
                }
            }
        }
        world.add_cubic(
            effected,
            AddCubic {
                cubic_id: self.cubic_id,
                level: template.level,
                power: self.cubic_power,
                delay: self.cubic_delay,
                skill_chance: self.cubic_skill_chance,
                max_count: self.cubic_max_count,
                duration: self.cubic_duration,
                given_by_other: effected != effector,
            },
        );
        world.broadcast_user_info(effected);
    }
}

/// Java `CubicMastery`: the passive skill that raises the number of cubics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CubicMastery {
    cubic_count: i32,
}

impl CubicMastery {
    /// Java `CubicMastery(attachCond, applyCond, set, params)`: `cubicCount` (1).
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self {
            cubic_count: params::int(template, "cubicCount", 1)?,
        }))
    }
}

impl EffectHandler for CubicMastery {
    fn can_start(&self, world: &World, cx: &EffectCx<'_>) -> bool {
        world.is_player(cx.effected)
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        world.set_max_cubic_count(cx.effected, self.cubic_count);
    }

    fn on_action_time(&self, _world: &mut World, cx: &EffectCx<'_>) -> bool {
        cx.skill.is_passive()
    }

    fn on_exit(&self, world: &mut World, cx: &EffectCx<'_>) {
        world.set_max_cubic_count(cx.effected, 1);
    }
}
