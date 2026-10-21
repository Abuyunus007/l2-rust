#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Loads the real Java datapack's skill trees. Skipped when the folder does not exist (CI).
//!
//! The folder defaults to `C:\l2-rust\l2jmobius\dist\game`; set `L2JMOBIUS_GAME_DIR` to use another.
//! The expected starting skills are read from the Java XML files and Java's `CharacterCreate` rule,
//! not from a character created on a server.

use std::path::PathBuf;

use l2_data::{ClassId, Datapack};

fn game_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("L2JMOBIUS_GAME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\l2-rust\l2jmobius\dist\game"));
    if dir.join("data").is_dir() {
        Some(dir)
    } else {
        eprintln!("skipped: {} does not exist", dir.display());
        None
    }
}

/// `(skill id, skill level)` of every skill a new character of `class` at level 1 gets, sorted.
fn starting_skills(pack: &Datapack, class: u16) -> Vec<(i32, i32)> {
    let mut skills: Vec<(i32, i32)> = pack
        .skill_trees
        .auto_get_skills(ClassId(class), 1, false)
        .iter()
        .map(|s| (s.skill_id, s.skill_level))
        .collect();
    skills.sort_unstable();
    skills
}

#[test]
fn every_class_has_a_skill_tree() {
    let Some(dir) = game_dir() else { return };
    let pack = Datapack::load(&dir).unwrap();
    assert_eq!(pack.skill_trees.len(), 103);
    for class in pack.class_list.iter() {
        assert!(
            pack.skill_trees
                .complete_class_tree(class.class_id)
                .next()
                .is_some(),
            "no skill tree for {}",
            class.name
        );
    }
}

#[test]
fn human_fighter_starting_skills_match_java() {
    let Some(dir) = game_dir() else { return };
    let pack = Datapack::load(&dir).unwrap();
    // HumanFighter.xml (classId 0): Lucky 194 and Common Craft 1322, both autoGet, getLevel 1.
    assert_eq!(starting_skills(&pack, 0), vec![(194, 1), (1322, 1)]);
}

#[test]
fn human_mystic_starting_skills_match_java() {
    let Some(dir) = game_dir() else { return };
    let pack = Datapack::load(&dir).unwrap();
    // HumanMystic.xml (classId 10): seven autoGet skills of getLevel 1.
    assert_eq!(
        starting_skills(&pack, 10),
        vec![
            (118, 1),
            (163, 1),
            (194, 1),
            (214, 1),
            (1177, 1),
            (1216, 1),
            (1322, 1),
        ]
    );
}

#[test]
fn warrior_inherits_the_human_fighter_starting_skills() {
    let Some(dir) = game_dir() else { return };
    let pack = Datapack::load(&dir).unwrap();
    // Warrior (1) has parentClassId 0 in its skill tree file.
    let skills = starting_skills(&pack, 1);
    assert!(skills.contains(&(194, 1)), "{skills:?}");
    assert!(skills.contains(&(1322, 1)), "{skills:?}");
}

/// `data/SkillLearn.xml` lists 256 trainer NPCs (counted with grep on the Java file); Auron
/// (30010) teaches the ten human fighter classes.
#[test]
fn skill_learn_data_matches_the_java_file() {
    let Some(dir) = game_dir() else { return };
    let pack = Datapack::load(&dir).unwrap();
    assert_eq!(pack.skill_learn.len(), 256);
    let auron: Vec<u16> = pack
        .skill_learn
        .classes_of(30010)
        .unwrap()
        .iter()
        .map(|c| c.0)
        .collect();
    assert_eq!(auron, (0..=9).collect::<Vec<u16>>());
    assert!(pack.skill_learn.classes_of(1).is_none());
}

/// A skill of the real tree with an item cost and an SP cost: Rogue's Divine Inspiration level 1
/// (`Rogue.xml`: getLevel 52, learnedByNpc, item 8618).
#[test]
fn skill_learn_lines_carry_sp_and_items() {
    let Some(dir) = game_dir() else { return };
    let pack = Datapack::load(&dir).unwrap();
    let line = pack.skill_trees.class_skill(ClassId(7), 1405, 1).unwrap();
    assert_eq!((line.get_level, line.learned_by_npc), (52, true));
    assert_eq!(line.required_items.len(), 1);
    assert_eq!(
        (line.required_items[0].item_id, line.required_items[0].count),
        (8618, 1)
    );
    // Bow Mastery 1: levelUpSp 1100, getLevel 20 (Rogue.xml).
    let bow = pack.skill_trees.class_skill(ClassId(7), 208, 1).unwrap();
    assert_eq!((bow.level_up_sp, bow.get_level), (1100, 20));
    // A third class inherits it.
    assert!(pack.skill_trees.class_skill(ClassId(93), 208, 1).is_some());
}

/// The clan trees of `pledgeSkillTree.xml` and `subPledgeSkillTree.xml`, in the order a JDK 25
/// `ConcurrentHashMap<Integer, _>` lists the keys `skillId * 1021 + skillLevel` put in file order
/// (what `getAvailablePledgeSkills` walks; produced with a small Java program).
#[test]
fn clan_skill_trees_list_in_java_map_order() {
    let Some(dir) = game_dir() else { return };
    let pack = Datapack::load(&dir).unwrap();
    let ids = |lines: &[l2_data::SkillLearn]| -> Vec<(i32, i32)> {
        lines.iter().map(|l| (l.skill_id, l.skill_level)).collect()
    };
    let pledge = ids(pack.skill_trees.pledge_skills());
    // The whole list, as the JDK prints it.
    assert_eq!(
        pledge,
        vec![
            (383, 2),
            (383, 1),
            (595, 1),
            (382, 1),
            (594, 1),
            (383, 3),
            (849, 1),
            (384, 1),
            (385, 3),
            (851, 1),
            (384, 3),
            (850, 1),
            (384, 2),
            (593, 1),
            (380, 1),
            (852, 1),
            (381, 3),
            (380, 3),
            (380, 2),
            (597, 1),
            (382, 3),
            (596, 1),
            (382, 2),
            (381, 2),
            (381, 1),
            (853, 1),
            (378, 3),
            (378, 2),
            (377, 2),
            (377, 1),
            (379, 2),
            (379, 1),
            (378, 1),
            (379, 3),
            (375, 2),
            (592, 1),
            (375, 1),
            (374, 1),
            (375, 3),
            (376, 1),
            (848, 1),
            (377, 3),
            (591, 1),
            (376, 3),
            (590, 1),
            (376, 2),
            (372, 1),
            (373, 3),
            (372, 3),
            (372, 2),
            (374, 3),
            (374, 2),
            (373, 2),
            (373, 1),
            (370, 3),
            (370, 2),
            (371, 2),
            (371, 1),
            (370, 1),
            (371, 3),
            (610, 1),
            (609, 1),
            (605, 1),
            (604, 1),
            (608, 1),
            (390, 1),
            (391, 1),
            (607, 1),
            (606, 1),
            (388, 2),
            (600, 1),
            (388, 3),
            (855, 1),
            (389, 3),
            (854, 1),
            (388, 1),
            (389, 1),
            (856, 1),
            (389, 2),
            (599, 1),
            (390, 2),
            (598, 1),
            (390, 3),
            (603, 1),
            (385, 2),
            (386, 2),
            (602, 1),
            (385, 1),
            (386, 3),
            (387, 3),
            (386, 1),
            (387, 1),
            (387, 2),
            (601, 1),
        ]
    );
    let sub = ids(pack.skill_trees.sub_pledge_skills());
    assert_eq!(
        sub,
        vec![
            (616, 1),
            (616, 3),
            (616, 2),
            (615, 2),
            (615, 1),
            (614, 1),
            (615, 3),
            (611, 2),
            (611, 1),
            (611, 3),
            (614, 3),
            (614, 2),
            (613, 2),
            (613, 1),
            (612, 1),
            (613, 3),
            (612, 3),
            (612, 2)
        ]
    );
    // Every line names a social class; the castle skills are the residence ones.
    assert!(
        pack.skill_trees
            .pledge_skills()
            .iter()
            .all(|l| l.social_class.is_some())
    );
    let line = pack.skill_trees.pledge_skill(370, 1).unwrap();
    assert_eq!((line.get_level, line.level_up_sp), (5, 1500));
    assert_eq!(line.required_items[0].item_id, 9816);
    assert!(pack.skill_trees.pledge_skill(590, 1).unwrap().residence_skill);
}
