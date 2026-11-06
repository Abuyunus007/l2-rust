//! How strong a companion is for the level of its owner.
//!
//! The NPC template holds the values of a level 85 character. A companion of level `L` gets
//! `L`'s share of them, to a power per value: HP and defence grow with the square of the level,
//! attack a bit faster (a low level character hits for far less than a high level one, but is
//! not far more fragile) and MP in between. At level 85 every factor is 1.

use l2_world::NpcScale;

/// The level the templates are written for.
pub const REFERENCE_LEVEL: i32 = 85;

/// The factors for a companion of `level` (a level beyond 85 counts as 85, one below 1 as 1).
pub fn scale_for(level: i32) -> NpcScale {
    let level = level.clamp(1, REFERENCE_LEVEL);
    let share = f64::from(level) / f64::from(REFERENCE_LEVEL);
    let factor = |power: f64| share.powf(power) as f32;
    NpcScale {
        level,
        hp: factor(2.0),
        mp: factor(1.5),
        attack: factor(2.5),
        defence: factor(2.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reference_level_is_the_template() {
        let scale = scale_for(85);
        assert_eq!(scale.level, 85);
        for factor in [scale.hp, scale.mp, scale.attack, scale.defence] {
            assert!((factor - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn a_lower_level_is_weaker_in_every_value_and_a_higher_one_never_stronger_than_the_template() {
        let mut last = scale_for(1);
        for level in 2..=85 {
            let scale = scale_for(level);
            assert!(scale.hp > last.hp && scale.mp > last.mp, "level {level}");
            assert!(
                scale.attack > last.attack && scale.defence > last.defence,
                "level {level}"
            );
            last = scale;
        }
        assert_eq!(scale_for(99), scale_for(85), "above 85 it stays 85");
        assert_eq!(scale_for(99).level, 85);
        assert_eq!(scale_for(0).level, 1);
        assert_eq!(scale_for(-5).level, 1);
    }

    #[test]
    fn the_curve_has_the_shape_it_is_documented_to_have() {
        let half = scale_for(42); // about half of 85
        let share = 42.0_f32 / 85.0;
        assert!((half.hp - share.powi(2)).abs() < 1e-4);
        assert!((half.attack - share.powf(2.5)).abs() < 1e-4);
        assert!(half.attack < half.hp, "attack grows faster than HP");
        assert!(half.mp > half.hp, "MP falls off slower than HP");
    }
}
