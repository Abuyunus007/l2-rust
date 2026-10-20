//! Trait and attribute (element) bonuses for skills.
//!
//! Java: `mechanics/stats/Formulas.java` `calcGeneralTraitBonus`, `calcWeaponTraitBonus`,
//! `calcAttributeBonus(attacker, target, skill)`.
//! The normal-attack versions (`calc_attack_trait_bonus`, `calc_attribute_bonus`) live in
//! [`damage`](super::damage); this file adds the parts a skill reads.

use super::damage::{TraitInput, calc_attribute_bonus};

/// What `traitType` is for `calcGeneralTraitBonus`: `TraitType.NONE` itself, or one of the
/// `getType()` groups.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraitKind {
    /// `TraitType.NONE`: returns 1 before even the invulnerability check.
    None,
    /// `getType()` 0 or 1 (`UNK_8` and the weapon types): invulnerability counts, then 1.
    Other,
    /// `getType()` 2 (weaknesses): needs an attack trait on the attacker and a defence trait on
    /// the target.
    Weakness,
    /// `getType()` 3 (resistances): skipped when `ignoreResistance`.
    Resistance,
}

impl TraitKind {
    /// The kind for a `TraitType`: `is_none` is `traitType == TraitType.NONE`, `java_type` its
    /// `getType()` (`l2_data::skill::TraitType::kind`).
    pub fn of(is_none: bool, java_type: i32) -> Self {
        if is_none {
            return TraitKind::None;
        }
        match java_type {
            2 => TraitKind::Weakness,
            3 => TraitKind::Resistance,
            _ => TraitKind::Other,
        }
    }
}

/// Java `Formulas.calcGeneralTraitBonus(attacker, target, traitType, ignoreResistance)`.
///
/// 1 for no trait, 0 when the target is invulnerable to it, else `clamp(attack - defence + 1,
/// 0.05, 2)` for weaknesses (when both sides have the trait) and resistances.
pub fn calc_general_trait_bonus(kind: TraitKind, t: &TraitInput, ignore_resistance: bool) -> f64 {
    if kind == TraitKind::None {
        return 1.0;
    }

    if t.target_invul {
        return 0.0;
    }

    match kind {
        TraitKind::Weakness => {
            if !t.attacker_has_attack_trait || !t.target_has_defence_trait {
                return 1.0;
            }
        }
        TraitKind::Resistance => {
            if ignore_resistance {
                return 1.0;
            }
        }
        TraitKind::None | TraitKind::Other => return 1.0,
    }

    // float - float stays a float in Java, only the `+ 1.0` widens it.
    let result = f64::from(t.attack_trait - t.defence_trait) + 1.0;
    result.clamp(0.05, 2.0)
}

/// Java `Formulas.calcWeaponTraitBonus(attacker, target)`: `weapon_defence_trait` is
/// `target.getStat().getDefenceTraits()[attacker.getAttackType().getTraitType().ordinal()]`.
pub fn calc_weapon_trait_bonus(weapon_defence_trait: f32) -> f64 {
    let result = f64::from(weapon_defence_trait) - 1.0;
    1.0 - result
}

/// Which element `calcAttributeBonus(attacker, target, skill)` asks both creatures about: the
/// skill's element when it has one (not -1), else the attacker's strongest one
/// (`attacker.getAttackElement()`). The caller reads `getAttackElementValue(e)` and
/// `getDefenseElementValue(e)` for it and hands them to [`calc_attribute_bonus_skill`].
pub fn attribute_element(skill_element: Option<i8>, attacker_attack_element: i8) -> i8 {
    match skill_element {
        Some(element) if element != -1 => element,
        _ => attacker_attack_element,
    }
}

/// Java `Formulas.calcAttributeBonus(attacker, target, skill)`.
///
/// `skill_element_power` is `Some(skill.getElementPower())` when the skill has an element
/// (`skill != null && skill.getElement() != -1`), which is added to the attacker's value;
/// `attack_element_value` and `defence_element_value` are for the element
/// [`attribute_element`] names.
pub fn calc_attribute_bonus_skill(
    skill_element_power: Option<i32>,
    attack_element_value: i32,
    defence_element_value: i32,
) -> f64 {
    let attack_attribute = match skill_element_power {
        Some(power) => attack_element_value.wrapping_add(power),
        None => attack_element_value,
    };
    calc_attribute_bonus(attack_attribute, defence_element_value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn weak() -> TraitInput {
        TraitInput {
            target_invul: false,
            attacker_has_attack_trait: true,
            target_has_defence_trait: true,
            attack_trait: 1.5,
            defence_trait: 1.0,
        }
    }

    #[test]
    fn none_wins_over_invulnerability() {
        let invul = TraitInput {
            target_invul: true,
            ..weak()
        };
        assert_eq!(calc_general_trait_bonus(TraitKind::None, &invul, false), 1.0);
        assert_eq!(calc_general_trait_bonus(TraitKind::Other, &invul, false), 0.0);
        assert_eq!(calc_general_trait_bonus(TraitKind::Weakness, &invul, true), 0.0);
        assert_eq!(calc_general_trait_bonus(TraitKind::Resistance, &invul, true), 0.0);
    }

    #[test]
    fn other_traits_have_no_bonus() {
        assert_eq!(calc_general_trait_bonus(TraitKind::Other, &weak(), false), 1.0);
    }

    #[test]
    fn weakness_needs_both_traits() {
        let one_sided = TraitInput {
            target_has_defence_trait: false,
            ..weak()
        };
        assert_eq!(
            calc_general_trait_bonus(TraitKind::Weakness, &one_sided, false),
            1.0
        );
        assert_eq!(
            calc_general_trait_bonus(TraitKind::Weakness, &weak(), false),
            f64::from(0.5_f32) + 1.0
        );
    }

    #[test]
    fn resistance_is_skipped_when_ignored_and_clamped_otherwise() {
        let t = TraitInput {
            attacker_has_attack_trait: false,
            target_has_defence_trait: false,
            attack_trait: 1.0,
            defence_trait: 1.5,
            ..weak()
        };
        assert_eq!(calc_general_trait_bonus(TraitKind::Resistance, &t, true), 1.0);
        // 1 - 1.5 + 1 = 0.5; no trait counts needed for a resistance.
        assert_eq!(calc_general_trait_bonus(TraitKind::Resistance, &t, false), 0.5);
        let huge = TraitInput {
            defence_trait: -9.0,
            ..t
        };
        assert_eq!(calc_general_trait_bonus(TraitKind::Resistance, &huge, false), 2.0);
        let tiny = TraitInput {
            defence_trait: 9.0,
            ..t
        };
        assert_eq!(
            calc_general_trait_bonus(TraitKind::Resistance, &tiny, false),
            0.05
        );
    }

    #[test]
    fn kind_of_the_java_types() {
        assert_eq!(TraitKind::of(true, 0), TraitKind::None);
        assert_eq!(TraitKind::of(false, 0), TraitKind::Other);
        assert_eq!(TraitKind::of(false, 1), TraitKind::Other);
        assert_eq!(TraitKind::of(false, 2), TraitKind::Weakness);
        assert_eq!(TraitKind::of(false, 3), TraitKind::Resistance);
    }

    #[test]
    fn weapon_trait_bonus() {
        assert_eq!(calc_weapon_trait_bonus(1.0), 1.0);
        assert_eq!(calc_weapon_trait_bonus(1.25), 0.75);
        assert_eq!(calc_weapon_trait_bonus(2.0), 0.0);
    }

    #[test]
    fn attribute_bonus_with_a_skill_element() {
        assert_eq!(attribute_element(Some(2), 4), 2);
        assert_eq!(attribute_element(Some(-1), 4), 4);
        assert_eq!(attribute_element(None, 4), 4);
        // Element power is added to the attacker's value.
        assert_eq!(
            calc_attribute_bonus_skill(Some(20), 0, -5),
            calc_attribute_bonus(20, -5)
        );
        assert_eq!(calc_attribute_bonus_skill(None, 7, 7), 1.0);
    }
}
