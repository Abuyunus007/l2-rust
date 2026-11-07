//! How strong a companion is for the level of its owner.
//!
//! The NPC template holds the HP, MP and defence of a level 85 character. A companion of level
//! `L` gets `L`'s share of them, to a power per value: HP and defence grow with the square of the
//! level, MP a bit slower. At level 85 every factor is 1.
//!
//! Its attack is the P. Atk. and M. Atk. of the weapon it holds ([`super::look`]), and the engine
//! adds the stat bonus and the level modifier as it does for a player: a companion hits like a
//! player of its level with that weapon, without soulshots and passive skills. (The templates
//! once held a level 85 character's final attack; the engine multiplied it by the level modifier
//! a second time, and companions killed monsters with one blow.)

use l2_world::NpcScale;

use super::gear_data::WEAPON_ATTACK;

/// The level the templates are written for.
pub const REFERENCE_LEVEL: i32 = 85;

/// Java's attack of an empty hand (`PlayerTemplate` `basePAtk` and `baseMAtk` of the fist).
const FIST: (i32, i32) = (4, 6);

/// The factors of `L2R.ini` on a companion's values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Multipliers {
    pub p_atk: f64,
    pub m_atk: f64,
    pub hp: f64,
}

impl Default for Multipliers {
    fn default() -> Self {
        Self {
            p_atk: 1.0,
            m_atk: 1.0,
            hp: 1.0,
        }
    }
}

/// The P. Atk. and M. Atk. of a weapon of the companion tables; an unknown item is a fist.
pub fn weapon_attack(item_id: i32) -> (i32, i32) {
    WEAPON_ATTACK
        .iter()
        .find(|(id, _, _)| *id == item_id)
        .map_or(FIST, |&(_, p_atk, m_atk)| (p_atk, m_atk))
}

/// The values of a companion of `level` (a level beyond 85 counts as 85, one below 1 as 1) that
/// holds `weapon`.
pub fn scale_for(level: i32, weapon: i32, multipliers: Multipliers) -> NpcScale {
    let level = level.clamp(1, REFERENCE_LEVEL);
    let share = f64::from(level) / f64::from(REFERENCE_LEVEL);
    let factor = |power: f64| share.powf(power) as f32;
    let (p_atk, m_atk) = weapon_attack(weapon);
    let times = |value: i32, by: f64| (f64::from(value) * by).round() as i32;
    NpcScale {
        level,
        hp: factor(2.0) * multipliers.hp as f32,
        mp: factor(1.5),
        p_atk: times(p_atk, multipliers.p_atk),
        m_atk: times(m_atk, multipliers.m_atk),
        defence: factor(2.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A sword of the table (Elven Long Sword: P. Atk. 92, M. Atk. 54).
    const SWORD: i32 = 2499;

    #[test]
    fn the_reference_level_is_the_template_and_the_attack_is_the_weapons() {
        let scale = scale_for(85, SWORD, Multipliers::default());
        assert_eq!(scale.level, 85);
        for factor in [scale.hp, scale.mp, scale.defence] {
            assert!((factor - 1.0).abs() < 1e-6);
        }
        assert_eq!((scale.p_atk, scale.m_atk), (92, 54));
        assert_eq!(weapon_attack(SWORD), (92, 54));
        assert_eq!(weapon_attack(57), FIST, "Adena is no weapon");
    }

    #[test]
    fn a_lower_level_has_less_hp_and_defence_and_the_same_weapon() {
        let mut last = scale_for(1, SWORD, Multipliers::default());
        for level in 2..=85 {
            let scale = scale_for(level, SWORD, Multipliers::default());
            assert!(scale.hp > last.hp && scale.mp > last.mp, "level {level}");
            assert!(scale.defence > last.defence, "level {level}");
            assert_eq!(scale.p_atk, last.p_atk, "the level modifier is the engine's");
            last = scale;
        }
        assert_eq!(
            scale_for(99, SWORD, Multipliers::default()),
            scale_for(85, SWORD, Multipliers::default())
        );
        assert_eq!(scale_for(0, SWORD, Multipliers::default()).level, 1);
    }

    #[test]
    fn the_multipliers_of_the_config_count() {
        let half = Multipliers {
            p_atk: 0.5,
            m_atk: 2.0,
            hp: 1.5,
        };
        let plain = scale_for(60, SWORD, Multipliers::default());
        let scale = scale_for(60, SWORD, half);
        assert_eq!((scale.p_atk, scale.m_atk), (46, 108));
        assert!((scale.hp - plain.hp * 1.5).abs() < 1e-6);
        assert_eq!((scale.mp, scale.defence), (plain.mp, plain.defence));
    }
}
