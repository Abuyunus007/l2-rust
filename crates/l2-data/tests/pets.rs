#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The shipped pet files (`data/stats/pets/*.xml`) against the numbers Java reads from them.
//! Skips when the Java reference folder is missing (CI has none).

use std::path::Path;

use l2_data::PetDataTable;

const DATAPACK: &str = r"C:\l2-rust\l2jmobius\dist\game";

fn table() -> Option<PetDataTable> {
    let root = Path::new(DATAPACK);
    if !root.join("data/stats/pets").is_dir() {
        eprintln!("skipped: {DATAPACK} is not available");
        return None;
    }
    Some(PetDataTable::load(root))
}

/// All 49 files load, each with the levels the file lists.
#[test]
fn every_shipped_pet_loads() {
    let Some(table) = table() else { return };
    assert_eq!(table.len(), 49);
    for npc in [
        12077, 12311, 12526, 12564, 12621, 12780, 16025, 16041, 16050, 16071, 16072,
    ] {
        let pet = table.pet_data(npc).unwrap_or_else(|| panic!("pet {npc}"));
        assert!(pet.min_level() <= pet.max_level(), "{npc}");
        assert!(pet.level_data(pet.min_level()).is_some(), "{npc}");
    }
}

/// Baby Buffalo (`12780_Baby_Buffalo.xml`): collar 6648, one food, 55 % hunger limit, level 1 has
/// 12 meals, 70 % owner exp taken and `org_hp` 13.1.
#[test]
fn baby_buffalo_matches_its_file() {
    let Some(table) = table() else { return };
    let pet = table.pet_data(12780).unwrap();
    assert_eq!((pet.item_id, pet.hungry_limit, pet.load), (6648, 55, 54510));
    assert_eq!(pet.food(), vec![7582]);
    assert_eq!(table.pet_data_by_item_id(6648).map(|p| p.npc_id), Some(12780));
    let first = pet.level_data(1).unwrap();
    assert_eq!(
        (first.max_feed, first.owner_exp_taken, first.max_exp),
        (12, 70, 1)
    );
    assert_eq!(first.max_hp, 13.1_f32);
    // The second level starts at 125001 exp.
    assert_eq!(pet.level_data(2).unwrap().max_exp, 125_001);
}

/// Wind Strider (`12526_Wind_Strider.xml`): three foods in the order Java's `HashSet` walks.
#[test]
fn wind_strider_food_order() {
    let Some(table) = table() else { return };
    let pet = table.pet_data(12526).unwrap();
    assert_eq!(pet.food_in_file_order(), &[5169, 5168, 6316]);
    assert_eq!(pet.food(), vec![5168, 5169, 6316]);
    assert!(l2_data::pet::is_mountable(12526));
    let first = pet.level_data(1).unwrap();
    assert_eq!(first.walk_speed_on_ride, 130.0);
    assert_eq!(first.slow_swim_speed_on_ride, 70.0);
}
