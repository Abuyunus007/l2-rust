//! The landing rolls the instant status effects use (`Confuse`, `Bluff`, `SkillTurning`,
//! `TargetCancel`): `Formulas.calcProbability`, with the attribute and trait bonuses it reads,
//! and the trait inputs of a normal hit.
// Java: mechanics/stats/Formulas.java (calcProbability, calcAttributeBonus, calcGeneralTraitBonus,
//       calcWeaponTraitBonus, calcAttackTraitBonus)
//
// NOTE: M5-FORM-02 ports the skill formulas as pure functions over plain inputs. These three
// world-side functions only fill the inputs of the status effects from the stat engine and the
// trait tables; when FORM-02 is merged they may call its functions instead of repeating the
// arithmetic.

use l2_data::SkillTemplate;
use l2_data::skill::TraitType;
use l2_util::ObjectId;

use super::traits::weapon_trait_type;
use crate::formulas::{Rnd, TraitInput, calc_attribute_bonus};
use crate::world::World;

impl World {
    /// Java `Formulas.calcAttributeBonus(attacker, target, skill)`.
    pub(crate) fn calc_attribute_bonus_for_effect(
        &self,
        attacker: ObjectId,
        target: ObjectId,
        skill: &SkillTemplate,
    ) -> f64 {
        let (Some(a), Some(t)) = (self.stats(attacker), self.stats(target)) else {
            return 1.0;
        };
        let (attack_attribute, defence_attribute) = if skill.element != -1 {
            (
                a.attack_element_value(skill.element).wrapping_add(skill.element_power),
                t.defense_element_value(skill.element),
            )
        } else {
            let element = a.attack_element();
            (a.attack_element_value(element), t.defense_element_value(element))
        };
        calc_attribute_bonus(attack_attribute, defence_attribute)
    }

    /// Java `Formulas.calcGeneralTraitBonus(attacker, target, traitType, ignoreResistance)`.
    pub(crate) fn calc_general_trait_bonus(
        &self,
        attacker: ObjectId,
        target: ObjectId,
        trait_type: TraitType,
        ignore_resistance: bool,
    ) -> f64 {
        if trait_type == TraitType::None {
            return 1.0;
        }
        let target_traits = self.control(target).map(|c| &c.traits);
        let attacker_traits = self.control(attacker).map(|c| &c.traits);
        if target_traits.is_some_and(|t| t.is_trait_invul(trait_type)) {
            return 0.0;
        }
        match trait_type.kind() {
            2 => {
                let has_attack = attacker_traits.is_some_and(|t| t.has_attack_trait(trait_type));
                let has_defence = target_traits.is_some_and(|t| t.has_defence_trait(trait_type));
                if !has_attack || !has_defence {
                    return 1.0;
                }
            }
            3 => {
                if ignore_resistance {
                    return 1.0;
                }
            }
            _ => return 1.0,
        }
        let attack = attacker_traits.map_or(1.0, |t| t.attack_trait(trait_type));
        let defence = target_traits.map_or(1.0, |t| t.defence_trait(trait_type));
        // float - float stays a float, only the `+ 1.0` widens it.
        let result = f64::from(attack - defence) + 1.0;
        result.clamp(0.05, 2.0)
    }

    /// Java `Formulas.calcProbability(baseChance, attacker, target, skill)`.
    pub(crate) fn calc_probability_for_effect(
        &mut self,
        base_chance: f64,
        attacker: ObjectId,
        target: ObjectId,
        skill: &SkillTemplate,
    ) -> bool {
        // `Rnd.get(100)` is drawn first.
        let roll = self.rng().get_int(100);
        let Some(t) = self.stats(target) else {
            return false;
        };
        let chance = ((((f64::from(skill.magic_level) + base_chance) - f64::from(t.level())) + 30.0)
            - f64::from(t.int()))
            * self.calc_attribute_bonus_for_effect(attacker, target, skill)
            * self.calc_general_trait_bonus(attacker, target, skill.trait_type, false);
        f64::from(roll) < chance
    }

    /// The trait inputs `Formulas.calcAttackTraitBonus(attacker, target)` reads: the target's
    /// defence trait against the attacker's weapon type, and every weakness trait (type 2).
    pub(crate) fn attack_trait_inputs(&self, attacker: ObjectId, target: ObjectId) -> (f32, Vec<TraitInput>) {
        let target_traits = self.control(target).map(|c| &c.traits);
        let attacker_traits = self.control(attacker).map(|c| &c.traits);
        let weapon = self
            .stats(attacker)
            .map_or(TraitType::None, |a| weapon_trait_type(a.attack_type()));
        let weapon_defence = target_traits.map_or(1.0, |t| t.defence_trait(weapon));
        let weaknesses = TraitType::ALL
            .iter()
            .copied()
            .filter(|t| t.kind() == 2)
            .map(|t| TraitInput {
                target_invul: target_traits.is_some_and(|x| x.is_trait_invul(t)),
                attacker_has_attack_trait: attacker_traits.is_some_and(|x| x.has_attack_trait(t)),
                target_has_defence_trait: target_traits.is_some_and(|x| x.has_defence_trait(t)),
                attack_trait: attacker_traits.map_or(1.0, |x| x.attack_trait(t)),
                defence_trait: target_traits.map_or(1.0, |x| x.defence_trait(t)),
            })
            .collect();
        (weapon_defence, weaknesses)
    }
}
