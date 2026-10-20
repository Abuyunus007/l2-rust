//! The `SetSkill` effect: the player learns the skill `skillId` at `skillLevel` (the skill list is
//! sent again).
// Java: dist/game/data/scripts/handlers/skill/effects/SetSkill.java

use l2_data::skill::EffectTemplate;

use super::params;
use crate::effects::registry::{EffectCx, EffectHandler};
use crate::world::World;

/// Java `SetSkill`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SetSkill {
    skill_id: i32,
    skill_level: i32,
}

impl SetSkill {
    /// Java `SetSkill(attachCond, applyCond, set, params)`: `skillId` (default 0), `skillLevel`
    /// (default 1).
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self {
            skill_id: params::int(template, "skillId", 0)?,
            skill_level: params::int(template, "skillLevel", 1)?,
        }))
    }
}

impl EffectHandler for SetSkill {
    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if !world.is_player(cx.effected) {
            return;
        }
        // Java `SkillData.getSkill(skillId, skillLevel)`: an unknown skill gives nothing.
        let known = world
            .skill_table()
            .is_some_and(|t| t.get(self.skill_id, self.skill_level).is_some());
        if !known {
            return;
        }
        world.add_skill(cx.effected, self.skill_id, self.skill_level);
        world.send_skill_list(cx.effected);
    }
}
