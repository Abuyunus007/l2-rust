#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Fishing data: the real `data/stats/fishing/*.xml` of the Java datapack (counts and spot checks).
//!
//! Skipped with a printed message when the folder does not exist (CI). The folder defaults to
//! `C:\l2-rust\l2jmobius\dist\game`; set `L2JMOBIUS_GAME_DIR` to use another.

use std::path::PathBuf;

use l2_data::fishing::{FishData, FishingMonstersData, FishingRodsData};

fn game_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("L2JMOBIUS_GAME_DIR")
        .map_or_else(|| PathBuf::from(r"C:\l2-rust\l2jmobius\dist\game"), PathBuf::from);
    if dir.join("data/stats/fishing/fishes.xml").is_file() {
        Some(dir)
    } else {
        println!("skipped: {} has no data/stats/fishing/fishes.xml", dir.display());
        None
    }
}

#[test]
fn fishes_xml_counts_and_spot_checks() {
    let Some(root) = game_dir() else { return };
    let data = FishData::load(&root);
    // 278 `<fish>` elements, all ids distinct.
    assert_eq!(data.len(), 278);

    // Fish 1: the small nimble green fish, a normal fish of group swift (1), level 1.
    let swift = data.get_fish(1, 1, 1);
    assert_eq!(swift.len(), 1);
    let fish = swift[0];
    assert_eq!((fish.fish_id, fish.item_id), (1, 6411));
    assert_eq!((fish.fish_hp, fish.hp_regen), (100, 4.0));
    assert_eq!((fish.start_combat_time, fish.combat_duration), (20, 24));
    assert_eq!((fish.fish_guts, fish.guts_check_time), (50.0, 5));

    // Every grade has all 27 levels of its three groups; the normal grade also has the boxes.
    for level in 1..=27 {
        for group in [4, 5, 6] {
            assert_eq!(data.get_fish(level, group, 0).len(), 1, "easy {level} {group}");
        }
        for group in [0, 1, 2, 3] {
            assert_eq!(data.get_fish(level, group, 1).len(), 1, "normal {level} {group}");
        }
        for group in [7, 8, 9] {
            assert_eq!(data.get_fish(level, group, 2).len(), 1, "hard {level} {group}");
        }
    }
}

#[test]
fn rods_and_monsters_xml_spot_checks() {
    let Some(root) = game_dir() else { return };
    let rods = FishingRodsData::load(&root);
    assert_eq!(rods.len(), 6);
    let rod = rods.get(6529).unwrap();
    assert_eq!(
        (rod.fishing_rod_id, rod.fishing_rod_level, rod.fishing_rod_damage),
        (1, 20, 20.0)
    );
    assert_eq!(rods.get(6534).unwrap().fishing_rod_damage, 39.0);

    let monsters = FishingMonstersData::load(&root);
    assert_eq!(monsters.len(), 8);
    assert!(monsters.get_fishing_monster(5).is_none());
    assert_eq!(
        monsters.get_fishing_monster(40).unwrap().fishing_monster_id,
        18322
    );
    assert_eq!(
        monsters.get_fishing_monster(85).unwrap().fishing_monster_id,
        18326
    );
    assert!(monsters.get_fishing_monster(86).is_none());
}
