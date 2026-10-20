//! The `CallSkill` effect: the caster triggers another skill on the target.
// Java: dist/game/data/scripts/handlers/skill/effects/CallSkill.java

use l2_data::skill::EffectTemplate;

use super::params;
use crate::effects::SkillKey;
use crate::effects::registry::{EffectCx, EffectHandler};
use crate::world::World;

/// Java `CallSkill`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CallSkill {
    skill: SkillKey,
}

impl CallSkill {
    /// Java `CallSkill(attachCond, applyCond, set, params)`: `skillId` (required) and
    /// `skillLevel` (default 1).
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        if template.param("skillId").is_none() {
            return Err("Integer value required, but not specified: skillId!".to_owned());
        }
        Ok(Box::new(Self {
            skill: SkillKey::new(
                params::int(template, "skillId", 0)?,
                params::int(template, "skillLevel", 1)?,
            ),
        }))
    }
}

impl EffectHandler for CallSkill {
    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        // Prevent infinite loop.
        if cx.skill.id == self.skill.id && cx.skill.level == self.skill.level {
            return;
        }
        // `SkillHolder.getSkill()`: `SkillData.getSkill(id, Math.max(level, 1))`.
        let Some(table) = world.skill_table() else {
            return;
        };
        let Some(called) = table.get_skill(self.skill.id, self.skill.level.max(1)) else {
            return;
        };
        world.make_trigger_cast(cx.effector, SkillKey::of(called), cx.effected, true);
    }
}
