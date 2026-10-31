#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The real Java buy lists and merchant price configs. Skipped with a printed message when the
//! folder does not exist (CI). The folder defaults to `dist/game`; set
//! `L2JMOBIUS_GAME_DIR` to use another.

use std::path::PathBuf;

use l2_data::{BuyListOptions, BuyLists, ItemTemplates, MerchantPriceConfigs};

fn game_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("L2JMOBIUS_GAME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game")));
    if dir.join("data/buylists").is_dir() {
        Some(dir)
    } else {
        eprintln!("skipped: {} does not exist", dir.display());
        None
    }
}

/// Every shipped file is read: the file names are `<digits>.xml`, every `<item>` names an item of
/// the datapack, and the lists that Java's example tests below rely on are what the XML says.
#[test]
fn the_shipped_buy_lists_load() {
    let Some(root) = game_dir() else { return };
    let items = ItemTemplates::load(&root, false);
    let lists = BuyLists::load(&root, &items, &BuyListOptions::default());
    let files = std::fs::read_dir(root.join("data/buylists"))
        .unwrap()
        .flatten()
        .filter(|e| e.path().is_file())
        .count();
    assert_eq!(lists.len(), files, "every file gives a list");

    // 0000382.xml: the first NPC and its first products, as written in the file.
    let list = lists.get(382).unwrap();
    assert!(list.is_npc_allowed(31380));
    assert!(!list.is_npc_allowed(31381));
    assert_eq!(list.products()[0].item_id, 1835);
    assert_eq!(list.products()[0].price, 8);
    // No price in the file: the item's reference price decides.
    let no_price = list.product(1463).unwrap();
    assert_eq!(no_price.price, -1);
    assert_eq!(
        no_price.price_or(items.get(1463).unwrap().price),
        i64::from(items.get(1463).unwrap().price)
    );

    // 0000007.xml: a limited product with a restock delay of 180 minutes and no NPC list.
    let list = lists.get(7).unwrap();
    let album = list.product(5900).unwrap();
    assert_eq!(
        (album.price, album.max_count, album.restock_delay_ms),
        (10_000, 1, 180 * 60_000)
    );
    assert!(list.npcs_allowed().is_none());
}

#[test]
fn the_shipped_price_configs_load() {
    let Some(root) = game_dir() else { return };
    let configs = MerchantPriceConfigs::load(&root);
    assert_eq!(configs.len(), 28);
    let giran = configs.get(0).unwrap();
    assert_eq!(
        (
            giran.name.as_str(),
            giran.base_tax,
            giran.castle_id,
            giran.zone_id
        ),
        ("Giran Town", 10, 3, 11020)
    );
    // `defaultPriceConfig="18"`: Neutral Territory, 50% and no castle.
    let none = std::collections::HashSet::new();
    let default = configs.for_zones(&none).unwrap();
    assert_eq!((default.id, default.base_tax, default.castle_id), (18, 50, -1));
}
