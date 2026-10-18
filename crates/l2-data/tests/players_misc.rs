#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Loads the real Java datapack for experience, initial equipment and initial shortcuts. Skipped
//! when the folder does not exist (CI).
//!
//! The folder defaults to `C:\l2-rust\l2jmobius\dist\game`; set `L2JMOBIUS_GAME_DIR` to use another.
//! Expected numbers are the Java server's: experience up to level 86, max level 85, 11 classes with
//! starting equipment (73 items), 5 global shortcuts, 4 class shortcut lists, no enabled macro.

use std::path::PathBuf;

use l2_data::{ClassId, Datapack, ShortcutType};

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

#[test]
fn experience_matches_the_xml() {
    let Some(dir) = game_dir() else { return };
    let pack = Datapack::load(&dir).unwrap();
    let exp = &pack.experience;
    assert_eq!(exp.max_level(), Some(85));
    assert_eq!(exp.max_pet_level(), Some(85));
    assert_eq!(exp.exp_for_level(0), Some(0));
    assert_eq!(exp.exp_for_level(1), Some(0));
    assert_eq!(exp.exp_for_level(2), Some(68));
    assert_eq!(exp.exp_for_level(85), Some(13_180_481_103));
    assert_eq!(exp.exp_for_level(86), Some(16_890_558_728));
    // Above the cap (MaximumPlayerLevel + 1) Java gives the exp of the cap.
    assert_eq!(exp.exp_for_level(87), Some(16_890_558_728));
    assert_eq!(exp.exp_percent(34, 1), Some(0.5));
}

#[test]
fn max_level_feeds_the_player_templates() {
    let Some(dir) = game_dir() else { return };
    let pack = Datapack::load(&dir).unwrap();
    let t = pack.player_templates.get(ClassId(31)).unwrap();
    assert!(t.base_hp_max(85).is_some());
    assert_eq!(t.base_hp_max(86), None);
}

#[test]
fn initial_equipment_counts_and_lookups() {
    let Some(dir) = game_dir() else { return };
    let pack = Datapack::load(&dir).unwrap();
    let equipment = &pack.initial_equipment;
    assert_eq!(equipment.len(), 11);

    let human_fighter = equipment.get(ClassId(0)).unwrap();
    assert_eq!(human_fighter.len(), 7);
    assert_eq!(human_fighter[0].item_id, 2369);
    assert_eq!(human_fighter[0].count, 1);
    assert!(human_fighter[0].equipped);
    assert!(!human_fighter[1].equipped); // Dagger goes to the inventory only
    assert_eq!(human_fighter[6].item_id, 12753);
    assert_eq!(human_fighter[6].count, 10);

    // A class without an entry gets nothing (Java returns null).
    assert!(equipment.get(ClassId(1)).is_none());
    // Kamael classes are in the data too.
    assert!(equipment.get(ClassId(123)).is_some());
}

#[test]
fn initial_shortcuts_counts_and_lookups() {
    let Some(dir) = game_dir() else { return };
    let pack = Datapack::load(&dir).unwrap();
    let shortcuts = &pack.initial_shortcuts;
    assert_eq!(shortcuts.global().len(), 5);
    assert_eq!(shortcuts.class_count(), 4);
    // The only macro in the file is disabled, so Java loads none.
    assert_eq!(shortcuts.macro_count(), 0);
    assert!(shortcuts.macro_preset(10000).is_none());

    let global = shortcuts.global();
    assert_eq!(global[0].kind, ShortcutType::Action);
    assert_eq!((global[0].slot, global[0].page, global[0].id), (0, 0, 2));
    assert_eq!(global[3].kind, ShortcutType::Item);
    assert_eq!(global[3].id, 5588);
    assert_eq!(global[4].kind, ShortcutType::Macro);
    assert_eq!((global[4].page, global[4].id), (1, 10000));

    let mystic = shortcuts.class_shortcuts(ClassId(10)).unwrap();
    assert_eq!(mystic.len(), 2);
    assert_eq!((mystic[0].slot, mystic[0].id, mystic[0].level), (1, 1177, 1));
    assert_eq!(mystic[1].id, 1216);

    assert_eq!(shortcuts.class_shortcuts(ClassId(49)).unwrap()[0].id, 1001);
    assert!(shortcuts.class_shortcuts(ClassId(0)).is_none());
}
