//! The trait tables of a creature (`AttackTrait`, `DefenceTrait` effects): how much more damage a
//! trait deals, how much less it takes, and which traits it is immune to.
// Java: entity/actor/stat/CreatureStat.java (_attackTraits, _attackTraitsCount, _defenceTraits,
//       _defenceTraitsCount, _traitsInvul and their getters)
// Java: mechanics/stats/TraitType.java
// Java: entity/item/type/WeaponType.java (getTraitType)

use l2_data::item::WeaponType;
use l2_data::skill::TraitType;

/// How many trait types there are (`TraitType.values().length`).
const TRAITS: usize = TraitType::ALL.len();

/// The tables of one creature. Every multiplier starts at 1.0, every count at 0.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TraitStats {
    attack: [f32; TRAITS],
    attack_count: [i32; TRAITS],
    defence: [f32; TRAITS],
    defence_count: [i32; TRAITS],
    invul: [i32; TRAITS],
}

impl Default for TraitStats {
    fn default() -> Self {
        Self {
            attack: [1.0; TRAITS],
            attack_count: [0; TRAITS],
            defence: [1.0; TRAITS],
            defence_count: [0; TRAITS],
            invul: [0; TRAITS],
        }
    }
}

impl TraitStats {
    /// `getAttackTrait(type)`.
    pub fn attack_trait(&self, t: TraitType) -> f32 {
        self.attack[t.ordinal()]
    }

    /// `hasAttackTrait(type)`.
    pub fn has_attack_trait(&self, t: TraitType) -> bool {
        self.attack_count[t.ordinal()] > 0
    }

    /// `getDefenceTrait(type)`.
    pub fn defence_trait(&self, t: TraitType) -> f32 {
        self.defence[t.ordinal()]
    }

    /// `hasDefenceTrait(type)`.
    pub fn has_defence_trait(&self, t: TraitType) -> bool {
        self.defence_count[t.ordinal()] > 0
    }

    /// `isTraitInvul(type)`.
    pub fn is_trait_invul(&self, t: TraitType) -> bool {
        self.invul[t.ordinal()] > 0
    }

    /// `AttackTrait.onStart` for one trait: `attackTraits[t] *= value; count++`.
    pub fn add_attack(&mut self, t: TraitType, value: f32) {
        self.attack[t.ordinal()] *= value;
        self.attack_count[t.ordinal()] += 1;
    }

    /// `AttackTrait.onExit` for one trait.
    pub fn remove_attack(&mut self, t: TraitType, value: f32) {
        let i = t.ordinal();
        if self.attack_count[i] == 0 {
            return;
        }
        self.attack[i] /= value;
        self.attack_count[i] -= 1;
    }

    /// `DefenceTrait.onStart` for one trait: a value under 2.0 scales the defence, from 2.0 on
    /// (a 100% trait) the creature becomes immune.
    pub fn add_defence(&mut self, t: TraitType, value: f32) {
        let i = t.ordinal();
        if value < 2.0 {
            self.defence[i] *= value;
            self.defence_count[i] += 1;
        } else {
            self.invul[i] += 1;
        }
    }

    /// `DefenceTrait.onExit` for one trait.
    pub fn remove_defence(&mut self, t: TraitType, value: f32) {
        let i = t.ordinal();
        if value < 2.0 {
            if self.defence_count[i] == 0 {
                return;
            }
            self.defence[i] /= value;
            self.defence_count[i] -= 1;
        } else {
            if self.invul[i] == 0 {
                return;
            }
            self.invul[i] -= 1;
        }
    }
}

/// Java `WeaponType.getTraitType()`.
pub fn weapon_trait_type(weapon: WeaponType) -> TraitType {
    match weapon {
        WeaponType::Sword => TraitType::Sword,
        WeaponType::Blunt => TraitType::Blunt,
        WeaponType::Dagger => TraitType::Dagger,
        WeaponType::Bow => TraitType::Bow,
        WeaponType::Pole => TraitType::Pole,
        WeaponType::Dual => TraitType::Dual,
        WeaponType::Etc => TraitType::Etc,
        WeaponType::Fist => TraitType::Fist,
        WeaponType::DualFist => TraitType::DualFist,
        WeaponType::Rapier => TraitType::Rapier,
        WeaponType::AncientSword => TraitType::AncientSword,
        WeaponType::CrossBow => TraitType::Crossbow,
        WeaponType::DualDagger => TraitType::DualDagger,
        // NONE, FISHINGROD, FLAG and OWNTHING.
        _ => TraitType::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_table_is_neutral() {
        let t = TraitStats::default();
        assert_eq!(t.attack_trait(TraitType::Shock), 1.0);
        assert!(!t.has_attack_trait(TraitType::Shock));
        assert!(!t.is_trait_invul(TraitType::Shock));
    }

    #[test]
    fn attack_traits_stack_and_unwind() {
        let mut t = TraitStats::default();
        t.add_attack(TraitType::Shock, 1.25);
        t.add_attack(TraitType::Shock, 1.25);
        assert_eq!(t.attack_trait(TraitType::Shock), 1.25_f32 * 1.25_f32);
        assert!(t.has_attack_trait(TraitType::Shock));
        t.remove_attack(TraitType::Shock, 1.25);
        t.remove_attack(TraitType::Shock, 1.25);
        assert!(!t.has_attack_trait(TraitType::Shock));
        // A removal without a start changes nothing.
        t.remove_attack(TraitType::Shock, 1.25);
        assert_eq!(t.attack_trait(TraitType::Shock), 1.0);
    }

    #[test]
    fn a_full_defence_trait_is_immunity() {
        let mut t = TraitStats::default();
        t.add_defence(TraitType::Sleep, 2.0);
        assert!(t.is_trait_invul(TraitType::Sleep));
        assert!(!t.has_defence_trait(TraitType::Sleep));
        t.remove_defence(TraitType::Sleep, 2.0);
        assert!(!t.is_trait_invul(TraitType::Sleep));
        t.add_defence(TraitType::Sleep, 1.5);
        assert!(t.has_defence_trait(TraitType::Sleep));
        assert_eq!(t.defence_trait(TraitType::Sleep), 1.5);
    }

    #[test]
    fn weapons_map_to_their_traits() {
        assert_eq!(weapon_trait_type(WeaponType::Sword), TraitType::Sword);
        assert_eq!(weapon_trait_type(WeaponType::CrossBow), TraitType::Crossbow);
        assert_eq!(weapon_trait_type(WeaponType::FishingRod), TraitType::None);
    }
}
