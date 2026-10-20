//! `AttackTrait` and `DefenceTrait`: the effects that move a creature's trait tables.
// Java: dist/game/data/scripts/handlers/skill/effects/AttackTrait.java
// Java: dist/game/data/scripts/handlers/skill/effects/DefenceTrait.java

use l2_data::skill::{EffectTemplate, ParamValue, TraitType};
use tracing::warn;

use crate::effects::registry::{EffectCx, EffectHandler};
use crate::world::World;

/// Java `AttackTrait`: more damage with a trait (`(value + 100) / 100`, a `float`).
#[derive(Debug, Clone)]
pub(crate) struct AttackTrait {
    attack_traits: Vec<(TraitType, f32)>,
}

impl AttackTrait {
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        let mut attack_traits = Vec::new();
        if template.params.is_empty() {
            warn!("AttackTrait: this effect must have parameters!");
            return Ok(Box::new(AttackTrait { attack_traits }));
        }
        for (key, value) in &template.params {
            let ParamValue::Text(text) = value else {
                continue;
            };
            let trait_type =
                TraitType::from_name(key).ok_or_else(|| format!("No enum constant TraitType.{key}"))?;
            let value: f32 = text
                .trim()
                .parse()
                .map_err(|_| format!("For input string: \"{text}\""))?;
            attack_traits.push((trait_type, (value + 100.0) / 100.0));
        }
        // An `EnumMap` iterates in ordinal order and keeps one value per key.
        attack_traits.sort_by_key(|(t, _)| t.ordinal());
        Ok(Box::new(AttackTrait { attack_traits }))
    }
}

impl EffectHandler for AttackTrait {
    fn on_exit(&self, world: &mut World, cx: &EffectCx<'_>) {
        if let Some(control) = world.control_mut(cx.effected) {
            for &(t, value) in &self.attack_traits {
                control.traits.remove_attack(t, value);
            }
        }
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if let Some(control) = world.control_mut(cx.effected) {
            for &(t, value) in &self.attack_traits {
                control.traits.add_attack(t, value);
            }
        }
    }
}

/// Java `DefenceTrait`: less damage from a trait, or immunity from a full 100%.
#[derive(Debug, Clone)]
pub(crate) struct DefenceTrait {
    defence_traits: Vec<(TraitType, f32)>,
}

impl DefenceTrait {
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        let mut defence_traits = Vec::new();
        if template.params.is_empty() {
            warn!("DefenceTrait: must have parameters.");
            return Ok(Box::new(DefenceTrait { defence_traits }));
        }
        for (key, value) in &template.params {
            let ParamValue::Text(text) = value else {
                continue;
            };
            let Ok(value) = text.trim().parse::<f32>() else {
                warn!("DefenceTrait: value of {key} must be float value {text} found.");
                continue;
            };
            if value == 0.0 {
                continue;
            }
            let Some(trait_type) = TraitType::from_name(key) else {
                warn!("DefenceTrait: value of TraitType enum required but found: {text}");
                continue;
            };
            defence_traits.push((trait_type, (value + 100.0) / 100.0));
        }
        defence_traits.sort_by_key(|(t, _)| t.ordinal());
        Ok(Box::new(DefenceTrait { defence_traits }))
    }
}

impl EffectHandler for DefenceTrait {
    fn on_exit(&self, world: &mut World, cx: &EffectCx<'_>) {
        if let Some(control) = world.control_mut(cx.effected) {
            for &(t, value) in &self.defence_traits {
                control.traits.remove_defence(t, value);
            }
        }
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if let Some(control) = world.control_mut(cx.effected) {
            for &(t, value) in &self.defence_traits {
                control.traits.add_defence(t, value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use l2_data::skill::EffectScope;

    use super::*;

    fn template(params: &[(&str, &str)]) -> EffectTemplate {
        EffectTemplate {
            name: "AttackTrait".into(),
            scope: EffectScope::General,
            ticks: 0,
            attrs: BTreeMap::new(),
            params: params
                .iter()
                .map(|(k, v)| ((*k).to_owned(), ParamValue::Text((*v).to_owned())))
                .collect(),
            funcs: Vec::new(),
            attach_cond: None,
            apply_cond: None,
        }
    }

    #[test]
    fn attack_trait_reads_the_percentages() {
        assert!(AttackTrait::create(&template(&[("SHOCK", "25"), ("SLEEP", "-10")])).is_ok());
        // `TraitType.valueOf` throws in the constructor.
        assert!(AttackTrait::create(&template(&[("NOT_A_TRAIT", "25")])).is_err());
        assert!(AttackTrait::create(&template(&[("SHOCK", "x")])).is_err());
    }

    #[test]
    fn defence_trait_skips_what_java_skips() {
        // Bad numbers, zero and unknown names are logged and dropped, not errors.
        assert!(DefenceTrait::create(&template(&[("SHOCK", "x"), ("SLEEP", "0"), ("NOPE", "5")])).is_ok());
    }
}
