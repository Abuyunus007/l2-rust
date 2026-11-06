//! The heal a healer casts: the skill and level that fit its level.
//!
//! "Heal" (1011) while the companion is below level 38, "Greater Heal" (1217) from there. Each
//! step is the highest level of the skill whose magic level the companion has reached; the
//! tables are the `magicLevel` of the skills in `data/stats/skills`, and a test compares them.

/// A level of a heal skill and the character level it asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Step {
    pub skill_id: i32,
    pub level: i32,
    pub magic_level: i32,
}

/// Where Greater Heal takes over from Heal.
const GREATER_HEAL_FROM: i32 = 38;

/// `magicLevel` of each level of skill 1011.
const HEAL_MAGIC_LEVELS: &[i32] = &[
    3, 5, 7, 10, 12, 14, 16, 18, 20, 23, 24, 25, 28, 29, 30, 33, 34, 35,
];

/// `magicLevel` of each level of skill 1217.
const GREATER_HEAL_MAGIC_LEVELS: &[i32] = &[
    38, 39, 40, 42, 43, 44, 46, 47, 48, 50, 51, 52, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67,
    68, 69, 70, 71, 72, 73, 74,
];

/// The best heal for a companion of `level`: `None` below the first magic level.
pub fn heal_for(level: i32) -> Option<Step> {
    let (skill_id, table) = if level >= GREATER_HEAL_FROM {
        (1217, GREATER_HEAL_MAGIC_LEVELS)
    } else {
        (1011, HEAL_MAGIC_LEVELS)
    };
    // The levels grow with the magic level, so the last one that fits is the highest.
    let index = table.iter().rposition(|&magic_level| magic_level <= level)?;
    Some(Step {
        skill_id,
        level: i32::try_from(index).ok()? + 1,
        magic_level: *table.get(index)?,
    })
}

/// Every step of both skills, for the test against the datapack.
#[cfg(test)]
pub fn all_steps() -> impl Iterator<Item = Step> {
    [(1011, HEAL_MAGIC_LEVELS), (1217, GREATER_HEAL_MAGIC_LEVELS)]
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
    fn the_heal_follows_the_level() {
        assert_eq!(heal_for(1), None, "nothing below the first magic level");
        assert_eq!(heal_for(2), None);
        assert_eq!(
            heal_for(3),
            Some(Step {
                skill_id: 1011,
                level: 1,
                magic_level: 3
            })
        );
        assert_eq!(
            heal_for(20),
            Some(Step {
                skill_id: 1011,
                level: 9,
                magic_level: 20
            })
        );
        assert_eq!(heal_for(37).map(|s| s.skill_id), Some(1011));
        assert_eq!(
            heal_for(37).map(|s| s.level),
            Some(18),
            "Heal tops out at level 35"
        );
        assert_eq!(
            heal_for(38),
            Some(Step {
                skill_id: 1217,
                level: 1,
                magic_level: 38
            })
        );
        assert_eq!(heal_for(85).map(|s| (s.skill_id, s.level)), Some((1217, 33)));
    }

    #[test]
    fn a_higher_level_never_gets_a_weaker_step_inside_a_skill() {
        let mut last = (0, 0);
        for level in 3..=85 {
            if let Some(step) = heal_for(level) {
                if step.skill_id == last.0 {
                    assert!(step.level >= last.1, "level {level}");
                }
                assert!(step.magic_level <= level);
                last = (step.skill_id, step.level);
            }
        }
    }
}
