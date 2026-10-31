#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Loads the real Java datapack. Skipped when the folder does not exist (CI).
//!
//! The folder defaults to `dist/game`; set `L2JMOBIUS_GAME_DIR` to use another.
//! Expected numbers are the Java server's: 103 classes, 103 templates, 85 levels x 6 values each.

use std::path::PathBuf;

use l2_data::{ClassId, Datapack, Location, Race};

fn game_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("L2JMOBIUS_GAME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game")));
    if dir.join("data").is_dir() {
        Some(dir)
    } else {
        eprintln!("skipped: {} does not exist", dir.display());
        None
    }
}

#[test]
fn loads_every_class_and_template_with_javas_counts() {
    let Some(dir) = game_dir() else { return };
    let pack = Datapack::load(&dir).unwrap();
    assert_eq!(pack.class_list.len(), 103);
    assert_eq!(pack.player_templates.len(), 103);
    // 103 files x 85 levels x (hp, mp, cp, hpRegen, mpRegen, cpRegen)
    assert_eq!(pack.player_templates.level_up_gain_count(), 103 * 85 * 6);
    // Every class has a template and the other way round.
    for class in pack.class_list.iter() {
        assert!(
            pack.player_templates.get(class.class_id).is_some(),
            "no template for {}",
            class.name
        );
    }
}

#[test]
fn dark_elf_fighter_matches_the_xml() {
    let Some(dir) = game_dir() else { return };
    let pack = Datapack::load(&dir).unwrap();
    let t = pack.player_templates.get(ClassId(31)).unwrap();
    assert_eq!(t.race, Race::DarkElf);
    assert_eq!((t.base_str, t.base_con, t.base_dex), (41, 32, 34));
    assert_eq!((t.base_int, t.base_wit, t.base_men), (25, 12, 26));
    assert_eq!((t.p_atk, t.m_atk, t.p_atk_spd, t.crit_rate), (4, 6, 300, 4));
    assert_eq!(t.p_def, 31 + 18 + 12 + 7 + 8 + 3 + 1);
    assert_eq!(t.m_def, 9 + 9 + 5 + 5 + 13);
    assert_eq!((t.walk_speed, t.run_speed), (85.0, 122.0));
    assert_eq!((t.collision_male.radius, t.collision_male.height), (7.5, 24.0));
    assert_eq!(
        (t.collision_female.radius, t.collision_female.height),
        (7.0, 23.5)
    );
    assert_eq!(t.creation_points.len(), 6);
    assert_eq!(
        t.creation_points[0],
        Location {
            x: 28295,
            y: 11063,
            z: -4224
        }
    );
    assert_eq!(t.base_hp_max(1), Some(94.0));
    assert_eq!(t.base_mp_max(1), Some(30.0));
    assert_eq!(t.base_cp_max(1), Some(37.6));
    assert_eq!(t.base_hp_max(2), Some(107.65));
    assert_eq!(t.base_hp_regen(2), Some(2.05));
    assert!(t.base_hp_max(85).unwrap() > t.base_hp_max(84).unwrap());
    assert_eq!(t.base_hp_max(86), None);
}

#[test]
fn class_list_names_and_parents() {
    let Some(dir) = game_dir() else { return };
    let pack = Datapack::load(&dir).unwrap();
    let list = &pack.class_list;
    assert_eq!(list.get_by_id(0).unwrap().name, "Human Fighter");
    assert_eq!(list.get_by_id(0).unwrap().parent, None);
    // NOTE(java-quirk): parent class 0 is dropped by Java (`parentClassId > 0`).
    assert_eq!(list.get_by_id(1).unwrap().parent, None);
    assert_eq!(list.get_by_id(2).unwrap().parent, Some(ClassId(1)));
    assert_eq!(list.get_by_id(136).unwrap().parent, Some(ClassId(135)));
}

/// `ClassId::race` must agree with Java's `PlayerClass` enum for all 103 ids
/// (`player_classes.json`, written by `codegen java-facts`).
#[test]
fn race_table_matches_java_player_class_enum() {
    let facts = l2_testkit::java_facts::player_classes();
    let mut checked = 0;
    for (id, race) in &facts.races {
        let expected = match race.as_str() {
            "HUMAN" => Race::Human,
            "ELF" => Race::Elf,
            "DARK_ELF" => Race::DarkElf,
            "ORC" => Race::Orc,
            "DWARF" => Race::Dwarf,
            "KAMAEL" => Race::Kamael,
            other => panic!("unexpected race {other}"),
        };
        assert_eq!(ClassId(*id).race(), Some(expected), "class {id}");
        checked += 1;
    }
    assert_eq!(checked, 103);
    let known = (0..=300u16).filter(|&i| ClassId(i).race().is_some()).count();
    assert_eq!(known, 103);
}
