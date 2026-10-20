//! The `ApplySkillEffects` effect: the listed skills (`skills`: `id,level;id,level`) are applied
//! from the caster to the target, as if they were cast on it.
// Java: dist/game/data/scripts/handlers/skill/effects/ApplySkillEffects.java

use l2_data::skill::EffectTemplate;

use super::dispel_by_slot::java_split;
use super::params;
use crate::effects::SkillKey;
use crate::effects::registry::{EffectCx, EffectHandler};
use crate::world::World;

/// Java `ApplySkillEffects`: the skills, in the order of the parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ApplySkillEffects {
    skills: Vec<SkillKey>,
}

impl ApplySkillEffects {
    /// Java `ApplySkillEffects(attachCond, applyCond, set, params)`: `skills` is optional; each
    /// entry is `id,level` (both must parse, as `Integer.parseInt` does).
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        let mut skills = Vec::new();
        if let Some(text) = params::string(template, "skills")? {
            for entry in java_split(&text, ';') {
                let pair = java_split(entry, ',');
                let id = pair
                    .first()
                    .copied()
                    .unwrap_or_default()
                    .parse::<i32>()
                    .map_err(|_| format!("Integer value required, but found: {entry}"))?;
                let level = pair
                    .get(1)
                    .ok_or_else(|| format!("skill entry without a level: {entry}"))?
                    .parse::<i32>()
                    .map_err(|_| format!("Integer value required, but found: {entry}"))?;
                skills.push(SkillKey::new(id, level));
            }
        }
        Ok(Box::new(Self { skills }))
    }
}

impl EffectHandler for ApplySkillEffects {
    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        for &skill in &self.skills {
            // Java `holder.getSkill()` is `null` for an unknown skill, which is skipped there and
            // by `apply_effects` here.
            world.apply_effects(cx.effector, cx.effected, skill);
        }
    }
}
