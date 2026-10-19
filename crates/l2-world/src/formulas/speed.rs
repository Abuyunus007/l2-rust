//! Attack and cast speed formulas.
//!
//! Java: `mechanics/stats/Formulas.java` `calcPAtkSpd`, `calcAtkSpd`;
//! `entity/actor/Creature.java` `calculateTimeBetweenAttacks`, `calculateReuseTime`.

/// Java `Formulas.calcPAtkSpd(attacker, target, rate)`: the delay in milliseconds before the next
/// attack for an attack-speed `rate`. (The attacker and target arguments are unused in Java.)
pub fn calc_p_atk_spd(rate: f64) -> i32 {
    // Measured Oct 2006 by Tank6585, formula by Sami.
    // attack speed 312 equals 1500 ms delay... (or 300 + 40 ms delay?)
    if rate < 2.0 {
        return 2700;
    }

    (470_000.0 / rate) as i32
}

/// Java `Formulas.calcAtkSpd(attacker, skill, skillTime)`: the time in milliseconds a skill takes
/// to cast (magic) or perform (physical).
///
/// `m_atk_spd` is `attacker.getMAtkSpd()` (an `int`), `p_atk_spd` is `attacker.getPAtkSpd()`.
pub fn calc_atk_spd(skill_is_magic: bool, skill_time: f64, m_atk_spd: i32, p_atk_spd: f64) -> i32 {
    if skill_is_magic {
        return ((skill_time / f64::from(m_atk_spd)) * 333.0) as i32;
    }

    ((skill_time / p_atk_spd) * 300.0) as i32
}

/// Java `Creature.calculateTimeBetweenAttacks()`: milliseconds of one attack (`doAttack` hits at
/// half of it). `p_atk_spd` is the stat engine's `getPAtkSpd()`.
pub fn calculate_time_between_attacks(p_atk_spd: f64) -> i32 {
    (500_000.0 / p_atk_spd) as i32
}

/// The attack type `calculateReuseTime` distinguishes when transformed (Java `WeaponType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReuseAttackType {
    Bow,
    Crossbow,
    /// Every other `getAttackType()`.
    Other,
}

/// What `Creature.calculateReuseTime(weapon)` reads.
#[derive(Debug, Clone, Copy)]
pub struct ReuseInput {
    /// `_stat.getPAtkSpd()`.
    pub p_atk_spd: f64,
    /// `weapon.getReuseDelay()`; `None` when the creature has no weapon (`weapon == null`).
    pub weapon_reuse_delay: Option<i32>,
    /// `isTransformed()`.
    pub is_transformed: bool,
    /// `getAttackType()`.
    pub attack_type: ReuseAttackType,
    /// `_stat.getWeaponReuseModifier(null)` (only read when transformed with a bow or crossbow).
    pub weapon_reuse_modifier: f64,
}

/// Java `Creature.calculateReuseTime(weapon)`: the bow reuse delay in milliseconds, 0 for weapons
/// without one.
pub fn calculate_reuse_time(input: &ReuseInput) -> i32 {
    if input.is_transformed {
        match input.attack_type {
            ReuseAttackType::Bow => {
                return ((1500 * 333) as f64 * input.weapon_reuse_modifier / input.p_atk_spd) as i32;
            }
            ReuseAttackType::Crossbow => {
                return ((1200 * 333) as f64 * input.weapon_reuse_modifier / input.p_atk_spd) as i32;
            }
            ReuseAttackType::Other => {}
        }
    }

    match input.weapon_reuse_delay {
        None | Some(0) => 0,
        Some(delay) => (f64::from(delay.wrapping_mul(333)) / input.p_atk_spd) as i32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p_atk_spd_has_a_floor_of_2700_below_rate_2() {
        assert_eq!(calc_p_atk_spd(-5.0), 2700);
        assert_eq!(calc_p_atk_spd(1.999), 2700);
        assert_eq!(calc_p_atk_spd(2.0), 235_000);
        assert_eq!(calc_p_atk_spd(278.0), 1690);
        assert_eq!(calc_p_atk_spd(1.0e6), 0);
    }

    #[test]
    fn atk_spd_magic_and_physical() {
        // (500 / 213) * 333 = 781.7
        assert_eq!(calc_atk_spd(true, 500.0, 213, 417.0), 781);
        // (1000 / 417) * 300 = 719.4
        assert_eq!(calc_atk_spd(false, 1000.0, 213, 417.0), 719);
    }

    #[test]
    fn time_between_attacks_truncates() {
        assert_eq!(calculate_time_between_attacks(417.0), 1199);
        assert_eq!(calculate_time_between_attacks(330.0), 1515);
    }

    #[test]
    fn reuse_time() {
        let bow = ReuseInput {
            p_atk_spd: 322.0,
            weapon_reuse_delay: Some(1500),
            is_transformed: false,
            attack_type: ReuseAttackType::Bow,
            weapon_reuse_modifier: 1.0,
        };
        assert_eq!(calculate_reuse_time(&bow), 1551);
        // No weapon, or a weapon without a reuse delay: none.
        assert_eq!(
            calculate_reuse_time(&ReuseInput {
                weapon_reuse_delay: None,
                ..bow
            }),
            0
        );
        assert_eq!(
            calculate_reuse_time(&ReuseInput {
                weapon_reuse_delay: Some(0),
                ..bow
            }),
            0
        );
        // Transformed archers ignore the weapon: 1500 * 333 / 322 (bow), 1200 * 333 / 322 (crossbow).
        let transformed = ReuseInput {
            is_transformed: true,
            weapon_reuse_delay: None,
            ..bow
        };
        assert_eq!(calculate_reuse_time(&transformed), 1551);
        assert_eq!(
            calculate_reuse_time(&ReuseInput {
                attack_type: ReuseAttackType::Crossbow,
                ..transformed
            }),
            1240
        );
        assert_eq!(
            calculate_reuse_time(&ReuseInput {
                attack_type: ReuseAttackType::Other,
                ..transformed
            }),
            0
        );
    }
}
