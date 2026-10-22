#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Recipe data: the real `Recipes.xml` of the Java datapack (counts and spot checks).
//!
//! Skipped with a printed message when the folder does not exist (CI). The folder defaults to
//! `C:\l2-rust\l2jmobius\dist\game`; set `L2JMOBIUS_GAME_DIR` to use another.

use std::path::PathBuf;

use l2_data::recipe::{RecipeData, StatType};

fn game_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("L2JMOBIUS_GAME_DIR")
        .map_or_else(|| PathBuf::from(r"C:\l2-rust\l2jmobius\dist\game"), PathBuf::from);
    if dir.join("data").join("Recipes.xml").is_file() {
        Some(dir)
    } else {
        println!("skipped: {} has no data/Recipes.xml", dir.display());
        None
    }
}

#[test]
fn recipes_xml_counts_and_spot_checks() {
    let Some(root) = game_dir() else { return };
    let data = RecipeData::load(&root);
    // 1000 `<item>` elements, all ids distinct, none skipped.
    assert_eq!(data.len(), 1000);

    // Recipe 1: mk_wooden_arrow, dwarven, level 1, 100 %, makes 500 arrows from two materials
    // for 30 MP.
    let arrow = data.get(1).unwrap();
    assert_eq!(arrow.recipe_id, 1666);
    assert_eq!(arrow.recipe_name, "mk_wooden_arrow");
    assert!(arrow.is_dwarven);
    assert_eq!((arrow.level, arrow.success_rate), (1, 100));
    assert_eq!((arrow.item_id, arrow.count), (17, 500));
    assert_eq!(arrow.recipes.len(), 2);
    assert_eq!(
        (arrow.stat_use[0].kind, arrow.stat_use[0].value),
        (StatType::Mp, 30)
    );
    assert_eq!(arrow.rare_item_id, 0);

    // Craft levels in the file go from 1 to 10, and both recipe types exist.
    assert!(data.iter().all(|r| (1..=10).contains(&r.level)));
    assert!(data.iter().any(|r| !r.is_dwarven));
    // The 582 `productionRare` lines are recipes with a rare item.
    assert_eq!(data.iter().filter(|r| r.rare_item_id != 0).count(), 582);
    // Only MP is used as `statUse`.
    assert!(
        data.iter()
            .flat_map(|r| &r.stat_use)
            .all(|s| s.kind == StatType::Mp)
    );

    // The recipe item 5008 is on two recipes; the lower recipe id wins.
    let lowest = data.iter().find(|r| r.recipe_id == 5008).unwrap().id;
    assert_eq!(data.by_item_id(5008).unwrap().id, lowest);
}
