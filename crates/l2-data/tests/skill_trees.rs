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
