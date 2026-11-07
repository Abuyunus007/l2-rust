//! The spell a mage casts: the skill and level that fit its level.
//!
//! "Energy Bolt" (1274) while the companion is below level 38, "Prominence" (1230) from there,
//! the way [`heal`](super::heal) climbs its ladder. The tables are the `magicLevel` of the skills
//! in `data/stats/skills`, and a test compares them.

use super::heal::{Step, step_in};

/// Where Prominence takes over from Energy Bolt.
const PROMINENCE_FROM: i32 = 38;

/// `magicLevel` of each level of skill 1274.
const ENERGY_BOLT_MAGIC_LEVELS: &[i32] = &[20, 25, 30, 35];

/// `magicLevel` of each level of skill 1230.
const PROMINENCE_MAGIC_LEVELS: &[i32] = &[
    38, 40, 42, 44, 46, 48, 50, 52, 54, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72,
    73, 74,
];

/// The best spell for a companion of `level`: `None` below the first magic level.
pub fn spell_for(level: i32) -> Option<Step> {
    if level >= PROMINENCE_FROM {
        step_in(1230, PROMINENCE_MAGIC_LEVELS, level)
    } else {
        step_in(1274, ENERGY_BOLT_MAGIC_LEVELS, level)
    }
}

/// Every step of both skills, for the test against the datapack.
#[cfg(test)]
pub fn all_steps() -> impl Iterator<Item = Step> {
    [(1274, ENERGY_BOLT_MAGIC_LEVELS), (1230, PROMINENCE_MAGIC_LEVELS)]
        .into_iter()
        .flat_map(|(skill_id, table)| {
            table.iter().enumerate().map(move |(index, &magic_level)| Step {
                skill_id,
                level: i32::try_from(index).unwrap_or(0) + 1,
                magic_level,
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_spell_follows_the_level() {
        assert_eq!(spell_for(19), None, "nothing below the first magic level");
        assert_eq!(spell_for(20).map(|s| (s.skill_id, s.level)), Some((1274, 1)));
        assert_eq!(spell_for(37).map(|s| (s.skill_id, s.level)), Some((1274, 4)));
        assert_eq!(spell_for(38).map(|s| (s.skill_id, s.level)), Some((1230, 1)));
        assert_eq!(spell_for(60).map(|s| (s.skill_id, s.level)), Some((1230, 14)));
        assert_eq!(spell_for(85).map(|s| (s.skill_id, s.level)), Some((1230, 28)));
    }
}
