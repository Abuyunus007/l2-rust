//! The class skills a companion uses (`class_skills`, made from the datapack): which it knows at
//! its level and which of a kind it takes.

use super::class_skills::{CLASS_SKILLS, SUMMON_NPCS};

/// What a skill does for a companion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    /// A physical blow at one enemy (Power Strike, Backstab, Double Shot ...).
    Strike,
    /// A spell at one enemy (Wind Strike, Hydro Blast ...).
    Nuke,
    /// Makes an enemy fight the caster (Aggression ...).
    Taunt,
    /// Heals one ally or the party.
    Heal,
    /// Raises a dead ally.
    Resurrect,
    /// A buff of an ally or the party (Might, songs, dances, chants ...).
    Buff,
    /// Calls a servitor.
    Summon,
}

/// A skill a companion knows, at the level it knows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Known {
    pub id: i32,
    pub level: i32,
    pub kind: Kind,
    /// The character level the skill is first learnt at: a later skill is a stronger one.
    pub learnt_at: i32,
}

/// Every class skill of `class_id` a companion of `level` knows, each at its highest level.
pub fn known(class_id: i32, level: i32) -> Vec<Known> {
    let Some((_, skills)) = CLASS_SKILLS.iter().find(|(id, _)| *id == class_id) else {
        return Vec::new();
    };
    skills
        .iter()
        .filter_map(|(id, kind, ladder)| {
            let learnt_at = ladder.first()?.0;
            let (_, skill_level) = ladder.iter().rev().find(|(at, _)| *at <= level)?;
            Some(Known {
                id: *id,
                level: *skill_level,
                kind: *kind,
                learnt_at,
            })
        })
        .collect()
}

/// The skills of `kind` a companion knows, the strongest (the latest learnt) first.
pub fn of_kind(class_id: i32, level: i32, kind: Kind) -> Vec<Known> {
    let mut found: Vec<Known> = known(class_id, level)
        .into_iter()
        .filter(|k| k.kind == kind)
        .collect();
    found.sort_by_key(|k| (std::cmp::Reverse(k.learnt_at), k.id));
    found
}

/// The servitor a summon skill calls at `level`.
pub fn summon_npc(skill_id: i32, level: i32) -> Option<i32> {
    let (_, npcs) = SUMMON_NPCS.iter().find(|(id, _)| *id == skill_id)?;
    npcs.get(usize::try_from(level - 1).ok()?)
        .or_else(|| npcs.last())
        .copied()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use crate::custom::companions::class_skills::SKILL_NAMES;
    use crate::custom::companions::classes::{CLASSES, by_id};
    use crate::custom::companions::role::Role;
    use crate::testing::datapack;

    fn name(id: i32) -> &'static str {
        SKILL_NAMES
            .iter()
            .find(|(i, _)| *i == id)
            .map_or("?", |(_, n)| *n)
    }

    #[test]
    fn a_skill_is_known_from_its_level_on_and_grows() {
        // An Adventurer (Rogue line): Mortal Blow from the 1st class on, Backstab later.
        let at = |level| {
            known(93, level)
                .into_iter()
                .filter(|k| k.kind == Kind::Strike)
                .map(|k| (name(k.id), k.level))
                .collect::<Vec<_>>()
        };
        let low = at(20);
        let high = at(80);
        assert!(low.iter().any(|(n, _)| *n == "Mortal Blow"), "{low:?}");
        assert!(high.iter().any(|(n, _)| *n == "Backstab"), "{high:?}");
        let mortal = |list: &[(&str, i32)]| list.iter().find(|(n, _)| *n == "Mortal Blow").unwrap().1;
        assert!(mortal(&high) > mortal(&low));
        assert!(known(93, 1).is_empty(), "nothing at level 1");
        assert!(known(999, 80).is_empty(), "no such class");
    }

    #[test]
    fn every_role_has_its_kind_of_skill_at_the_top_of_its_tier() {
        // At the last level of each tier (39, 75, 85) the role's own kind is there.
        let needs = |role: Role| match role {
            Role::Tank => Some(Kind::Taunt),
            Role::Warrior | Role::Dagger | Role::Archer => Some(Kind::Strike),
            Role::Mage => Some(Kind::Nuke),
            Role::Healer => Some(Kind::Heal),
            Role::Summoner => Some(Kind::Summon),
            Role::Buffer => None,
        };
        for class in CLASSES {
            let level = [39, 75, 85][usize::from(class.tier) - 1];
            if let Some(kind) = needs(class.role) {
                assert!(
                    !of_kind(class.id, level, kind).is_empty(),
                    "{} has no {kind:?} at {level}",
                    class.name
                );
            }
        }
        // The buffers buff, or fight (Inspector, Judicator).
        for class in CLASSES.iter().filter(|c| c.role == Role::Buffer) {
            let level = [39, 75, 85][usize::from(class.tier) - 1];
            assert!(
                !of_kind(class.id, level, Kind::Buff).is_empty()
                    || !of_kind(class.id, level, Kind::Strike).is_empty(),
                "{}",
                class.name
            );
        }
    }

    #[test]
    fn the_strongest_comes_first_and_a_summon_has_its_servitor() {
        let nukes = of_kind(by_id(94).unwrap().id, 80, Kind::Nuke);
        assert!(nukes.windows(2).all(|w| w[0].learnt_at >= w[1].learnt_at));
        let summons = of_kind(14, 60, Kind::Summon);
        assert!(!summons.is_empty());
        for summon in summons {
            assert!(
                summon_npc(summon.id, summon.level).is_some(),
                "{}",
                name(summon.id)
            );
        }
        assert_eq!(summon_npc(1, 1), None);
    }

    #[test]
    fn every_skill_and_servitor_is_in_the_datapack() {
        let Some(root) = datapack() else { return };
        let skills = l2_data::SkillTemplates::load(root, &l2_data::SkillLoadOptions::default());
        for (class, list) in CLASS_SKILLS {
            for (id, _, ladder) in *list {
                for (_, level) in *ladder {
                    assert!(
                        skills.get(*id, *level).is_some(),
                        "class {class}: skill {id} level {level}"
                    );
                }
            }
        }
        let npcs = l2_data::NpcTemplates::load(root, &l2_data::NpcLoadOptions::default());
        for (skill, list) in SUMMON_NPCS {
            for npc in *list {
                assert!(npcs.get(*npc).is_some(), "summon {skill}: npc {npc}");
            }
        }
    }
}
