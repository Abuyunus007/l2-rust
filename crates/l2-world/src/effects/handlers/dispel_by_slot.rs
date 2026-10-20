//! The `DispelBySlot` effect: takes the buff of each listed abnormal type off the creature when its
//! level is at most the level the `dispel` parameter gives (`type,level;type,level`; a level of -1
//! takes it whatever its level).
// Java: dist/game/data/scripts/handlers/skill/effects/DispelBySlot.java

use std::collections::BTreeMap;

use l2_data::skill::EffectTemplate;
use l2_data::stats::AbnormalType;

use super::params;
use crate::effects::SkillFinishType;
use crate::effects::registry::{EffectCx, EffectHandler, EffectType};
use crate::world::World;

/// Java `DispelBySlot`: the abnormal types to dispel with the level each one is taken at (a
/// `BTreeMap` keeps Java's `EnumMap` order, the declaration order of `AbnormalType`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DispelBySlot {
    dispel: BTreeMap<AbnormalType, i16>,
}

impl DispelBySlot {
    /// Java `DispelBySlot(attachCond, applyCond, set, params)`: `dispel` (none when missing or
    /// empty), each `type,level` pair split as Java's `String.split` does.
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        let mut dispel = BTreeMap::new();
        if let Some(text) = params::string(template, "dispel")?.filter(|t| !t.is_empty()) {
            for stack in java_split(&text, ';') {
                let pair = java_split(stack, ',');
                let name = pair.first().copied().unwrap_or_default();
                let value = pair
                    .get(1)
                    .ok_or_else(|| format!("dispel entry without a level: {stack}"))?;
                let abnormal = AbnormalType::from_name(name).unwrap_or(AbnormalType::None);
                let level = value
                    .parse::<i16>()
                    .map_err(|_| format!("Short value required, but found: {value}"))?;
                dispel.insert(abnormal, level);
            }
        }
        Ok(Box::new(Self { dispel }))
    }
}

/// Java `String.split(regex)` for a one-character regex: trailing empty pieces are dropped.
pub(crate) fn java_split(text: &str, separator: char) -> Vec<&str> {
    let mut pieces: Vec<&str> = text.split(separator).collect();
    while pieces.last().is_some_and(|p| p.is_empty()) {
        pieces.pop();
    }
    pieces
}

impl EffectHandler for DispelBySlot {
    fn effect_type(&self) -> EffectType {
        EffectType::DispelBySlot
    }

    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if self.dispel.is_empty() || world.is_raid(cx.effected) {
            return;
        }
        let Some(table) = world.skill_table() else {
            return;
        };
        for (&abnormal, &value) in &self.dispel {
            // Dispel transformations (buff and by GM): Java stops the transformation and skips
            // the entry.
            // NOTE(java-quirk): the condition is `isTransformed() || (isPlayer() || (value ==
            // asPlayer().getTransformationId()) || value < 0)`, true for every player; for a
            // creature that is no player Java's `asPlayer()` is null and the call throws, so the
            // entry is skipped here.
            if abnormal == AbnormalType::Transform {
                if world.is_player(cx.effected) {
                    world.stop_transformation(cx.effected, true);
                }
                continue;
            }
            let Some(info_skill) = world
                .effect_list(cx.effected)
                .and_then(|list| list.info_by_abnormal_type(abnormal))
                .map(|info| info.skill())
            else {
                continue;
            };
            let Some(skill) = table.get(info_skill.id, info_skill.level) else {
                continue;
            };
            if abnormal == skill.abnormal_type && (value < 0 || i32::from(value) >= skill.abnormal_level) {
                world.stop_skill_effects_by_abnormal(cx.effected, SkillFinishType::Removed, abnormal);
            }
        }
    }
}
