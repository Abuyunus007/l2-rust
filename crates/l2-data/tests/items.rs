#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Loads the real Java item files (197 files, 19199 items). Skipped when the folder does not
//! exist (CI).
//!
//! The folder defaults to `dist/game`; set `L2JMOBIUS_GAME_DIR` to use another.
//! The expected numbers were counted from the XML with an independent script (every `<item>`
//! element, `<set>` values with the last one winning); Java's own count line is
//! `Loaded 19199 items in total.`

use std::path::PathBuf;

use l2_data::item::{
    ActionType, ArmorType, BodyPart, CrystalType, EtcItemType, ItemTemplates, MaterialType, WeaponType,
};

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
fn loads_every_item_with_javas_counts() {
    let Some(dir) = game_dir() else { return };
    let items = ItemTemplates::load(&dir, false);
    assert_eq!(items.len(), 19199);
    assert_eq!(items.etc_item_count(), 11481);
    assert_eq!(items.armor_count(), 3825);
    assert_eq!(items.weapon_count(), 3893);
    assert_eq!(items.highest_id(), 22567);
    assert!(items.get(-1).is_none());
    assert!(items.get(22568).is_none());
    assert!(items.get(0).is_none());
}

#[test]
fn aggregates_match_the_xml() {
    let Some(dir) = game_dir() else { return };
    let items = ItemTemplates::load(&dir, false);
    let count = |f: &dyn Fn(&l2_data::ItemTemplate) -> bool| items.iter().filter(|i| f(i)).count();
    assert_eq!(count(&|i| i.stackable), 10208);
    assert_eq!(count(&|i| i.quest_item), 2691);
    assert_eq!(count(&|i| !i.tradeable), 7973);
    assert_eq!(count(&|i| i.enchantable), 4405);
    assert_eq!(items.iter().map(|i| i64::from(i.weight)).sum::<i64>(), 10_224_762);
    assert_eq!(
        items.iter().map(|i| i64::from(i.price)).sum::<i64>(),
        167_235_879_079
    );
    assert_eq!(
        items.iter().map(|i| i64::from(i.crystal_count)).sum::<i64>(),
        8_900_176
    );
    assert_eq!(count(&|i| i.crystal_type == CrystalType::None), 12607);
    assert_eq!(count(&|i| i.crystal_type == CrystalType::A), 1347);
    assert_eq!(count(&|i| i.crystal_type == CrystalType::S84), 798);
    assert_eq!(
        count(&|i| i.weapon().is_some_and(|w| w.weapon_type == WeaponType::Bow)),
        245
    );
    assert_eq!(
        count(&|i| i.armor().is_some_and(|a| a.armor_type == ArmorType::Shield)),
        169
    );
    assert_eq!(
        count(&|i| i.etc().is_some_and(|e| e.etc_type == EtcItemType::Shot)),
        56
    );
    assert_eq!(count(&|i| i.body_part == BodyPart::None), 11343);
    assert_eq!(count(&|i| i.body_part == BodyPart::BothHands), 2189);
    assert_eq!(count(&|i| i.body_part == BodyPart::RightBracelet), 12);
    assert_eq!(count(&|i| i.body_part == BodyPart::FullArmor), 172);
}

#[test]
fn adena_is_money() {
    let Some(dir) = game_dir() else { return };
    let items = ItemTemplates::load(&dir, false);
    let adena = items.get(57).unwrap();
    assert_eq!(adena.name, "Adena");
    assert_eq!(adena.icon.as_deref(), Some("icon.etc_adena_i00"));
    assert!(adena.is_etc_item() && adena.stackable);
    assert!(!adena.sellable && adena.tradeable && adena.dropable);
    assert_eq!((adena.type1, adena.type2), (4, 4));
    assert_eq!(adena.material, MaterialType::Gold);
    assert_eq!((adena.price, adena.weight), (1, 0));
    assert_eq!(adena.body_part.mask(), 0);
    assert!(!adena.is_equipable());
}

#[test]
fn short_sword_is_a_weapon() {
    let Some(dir) = game_dir() else { return };
    let items = ItemTemplates::load(&dir, false);
    let sword = items.get(1).unwrap();
    assert_eq!(sword.name, "Short Sword");
    assert_eq!((sword.type1, sword.type2), (0, 0));
    assert_eq!(sword.body_part.mask(), 0x80);
    assert_eq!((sword.weight, sword.price), (1600, 590));
    assert_eq!(sword.default_action, ActionType::Equip);
    assert_eq!(sword.material, MaterialType::Steel);
    assert!(sword.immediate_effect && sword.is_equipable());
    let w = sword.weapon().unwrap();
    assert_eq!(w.weapon_type, WeaponType::Sword);
    assert_eq!((w.soulshot_count, w.spiritshot_count), (1, 1));
    assert_eq!(
        (w.base_attack_range, w.base_attack_radius, w.base_attack_angle),
        (40, 40, 120)
    );
    assert_eq!(sword.item_mask(), 1);
}

#[test]
fn mithril_breastplate_is_an_armor() {
    let Some(dir) = game_dir() else { return };
    let items = ItemTemplates::load(&dir, false);
    let armor = items.get(58).unwrap();
    assert_eq!(armor.name, "Mithril Breastplate");
    assert_eq!((armor.type1, armor.type2), (1, 1));
    assert_eq!(armor.body_part.mask(), 0x400);
    assert_eq!(armor.armor().unwrap().armor_type, ArmorType::Heavy);
    assert_eq!(armor.crystal_type, CrystalType::D);
    assert_eq!(armor.crystal_count, 260);
    assert!(armor.enchantable && armor.is_crystallizable());
    assert_eq!((armor.weight, armor.price), (8670, 141_100));
    assert_eq!(armor.material, MaterialType::Mithril);
}

#[test]
fn healing_potion_is_a_stackable_etc_item() {
    let Some(dir) = game_dir() else { return };
    let items = ItemTemplates::load(&dir, false);
    let potion = items.get(1060).unwrap();
    assert_eq!(potion.name, "Lesser Healing Potion");
    assert!(potion.stackable && potion.oly_restricted && potion.for_npc);
    assert_eq!((potion.type1, potion.type2), (4, 5));
    assert_eq!(potion.reuse_delay, 10000);
    assert_eq!(potion.default_action, ActionType::SkillReduce);
    assert!(potion.is_potion());
    let etc = potion.etc().unwrap();
    assert_eq!(etc.etc_type, EtcItemType::Potion);
    assert_eq!(etc.handler.as_deref(), Some("ItemSkills"));
}

#[test]
fn custom_items_override_and_add() {
    let Some(dir) = game_dir() else { return };
    let items = ItemTemplates::load(&dir, true);
    // Gold Wyvern (50000) only exists in custom/.
    assert_eq!(items.len(), 19200);
    assert_eq!(items.highest_id(), 50000);
    assert_eq!(items.get(50000).unwrap().name, "Gold Wyvern");
}
