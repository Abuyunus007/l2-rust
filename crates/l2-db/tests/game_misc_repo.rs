#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Integration tests of the item, skill, shortcut and subclass repositories against a real
//! MySQL/MariaDB server.
//!
//! They run only when `L2_TEST_DATABASE_URL` is set (CI sets it for a throwaway MariaDB service
//! on Linux), e.g. `mysql://root:pw@127.0.0.1:3306/`. Never point it at a server with real
//! data: the tests only create and drop databases named `l2_test_*`, but they need the rights
//! to do so. Each test gets a fresh database built from `sql/game/*.sql`.

use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use l2_db::{
    ItemLocation, ItemRepository, ItemUpdate, NewItem, NewShortcut, NewSkillSave, Pool, ShortcutRepository,
    ShortcutRow, SkillLevel, SkillRepository, SkillSaveRow, SubclassRepository, SubclassRow, SubclassValues,
    connect_with,
};
use sqlx::mysql::{MySqlConnectOptions, MySqlConnection};
use sqlx::{AssertSqlSafe, Connection};

/// The tables of the item, skill, shortcut and subclass repositories.
const SCHEMA: [&str; 6] = [
    include_str!("../sql/game/items.sql"),
    include_str!("../sql/game/item_attributes.sql"),
    include_str!("../sql/game/character_skills.sql"),
    include_str!("../sql/game/character_skills_save.sql"),
    include_str!("../sql/game/character_shortcuts.sql"),
    include_str!("../sql/game/character_subclasses.sql"),
];

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// The statements of a schema file; whole-line `#` comments are dropped (a file may end with
/// one, which would otherwise be sent as an empty statement).
fn statements(file: &str) -> Vec<String> {
    let text: String = file
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    text.split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

/// A throwaway database with the game schema.
struct TestDb {
    admin: MySqlConnection,
    name: String,
    pool: Pool,
}

impl TestDb {
    /// `None` (with a printed note) when `L2_TEST_DATABASE_URL` is not set.
    async fn create() -> Option<Self> {
        let url = match std::env::var("L2_TEST_DATABASE_URL") {
            Ok(url) if !url.trim().is_empty() => url,
            _ => {
                eprintln!("skipped: L2_TEST_DATABASE_URL is not set");
                return None;
            }
        };
        let admin_options = MySqlConnectOptions::from_str(&url).unwrap();
        let mut admin = MySqlConnection::connect_with(&admin_options).await.unwrap();

        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let name = format!(
            "l2_test_misc_{}_{}_{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed),
            nanos % 1_000_000_000
        );
        assert!(name.starts_with("l2_test_"));
        // The name is generated above from the pid, a counter and the clock.
        sqlx::query(AssertSqlSafe(format!(
            "CREATE DATABASE `{name}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"
        )))
        .execute(&mut admin)
        .await
        .unwrap();

        let pool = connect_with(admin_options.database(&name), 10).await.unwrap();
        for file in SCHEMA {
            for statement in statements(file) {
                sqlx::query(AssertSqlSafe(statement))
                    .execute(&pool)
                    .await
                    .unwrap();
            }
        }
        Some(Self { admin, name, pool })
    }

    async fn count(&self, table: &str, column: &str, id: i32) -> i64 {
        sqlx::query_scalar(AssertSqlSafe(format!(
            "SELECT COUNT(*) FROM {table} WHERE {column} = {id}"
        )))
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    async fn drop_database(mut self) {
        self.pool.close().await;
        sqlx::query(AssertSqlSafe(format!("DROP DATABASE IF EXISTS `{}`", self.name)))
            .execute(&mut self.admin)
            .await
            .unwrap();
    }
}

const NOW: i64 = 1_700_000_000_000;
const ALICE: i32 = 268_500_001;
const BOB: i32 = 268_500_002;
const ADENA: i32 = 57;
const OBJ_ADENA: i32 = 268_600_001;
const OBJ_SWORD: i32 = 268_600_002;
const OBJ_CHEST: i32 = 268_600_003;
const OBJ_BOB: i32 = 268_600_004;

fn new_item(object_id: i32, owner_id: i32, loc: ItemLocation, loc_data: i32) -> NewItem {
    NewItem {
        owner_id,
        item_id: ADENA,
        count: 1,
        loc,
        loc_data,
        enchant_level: 0,
        object_id,
        custom_type1: 0,
        custom_type2: 0,
        mana_left: -1,
        time: 0,
    }
}

#[tokio::test]
async fn items_round_trip_by_location_and_owner() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = ItemRepository::new(db.pool.clone());

    let mut adena = new_item(OBJ_ADENA, ALICE, ItemLocation::Inventory, 0);
    adena.count = 5_000;
    repo.insert_item(&adena).await.unwrap();
    repo.insert_item(&new_item(OBJ_SWORD, ALICE, ItemLocation::Paperdoll, 6))
        .await
        .unwrap();
    repo.insert_item(&new_item(OBJ_CHEST, ALICE, ItemLocation::Warehouse, 0))
        .await
        .unwrap();
    repo.insert_item(&new_item(OBJ_BOB, BOB, ItemLocation::Inventory, 0))
        .await
        .unwrap();
    // The primary key rejects a second insert of the same object id.
    assert!(repo.insert_item(&adena).await.is_err());

    // The inventory view: INVENTORY and PAPERDOLL of Alice, ordered by loc_data.
    let inv = repo
        .inventory_items(ALICE, ItemLocation::Inventory, ItemLocation::Paperdoll)
        .await
        .unwrap();
    let ids: Vec<i32> = inv.iter().map(|r| r.object_id).collect();
    assert_eq!(ids, [OBJ_ADENA, OBJ_SWORD]);
    assert_eq!(inv[0].count, 5_000);
    assert_eq!(inv[0].location(), Some(ItemLocation::Inventory));
    assert_eq!(
        (inv[0].item_id, inv[0].loc_data, inv[0].enchant_level),
        (Some(ADENA), Some(0), Some(0))
    );
    // DECIMAL columns: mana_left -1 (the default) and time 0.
    assert_eq!((inv[0].mana_left, inv[0].time), (-1, 0));
    assert_eq!(inv[1].location(), Some(ItemLocation::Paperdoll));

    // One location only.
    let warehouse = repo
        .items_of_location(ALICE, ItemLocation::Warehouse)
        .await
        .unwrap();
    assert_eq!(warehouse.len(), 1);
    assert_eq!(warehouse[0].object_id, OBJ_CHEST);
    assert!(
        repo.items_of_location(ALICE, ItemLocation::Pet)
            .await
            .unwrap()
            .is_empty()
    );

    // The equipped items shown on the character selection screen.
    let paperdoll = repo.paperdoll_items(ALICE).await.unwrap();
    assert_eq!(paperdoll.len(), 1);
    assert_eq!(
        (paperdoll[0].object_id, paperdoll[0].loc_data),
        (OBJ_SWORD, Some(6))
    );

    // Update writes every column; the DECIMAL time is 13 digits.
    repo.update_item(
        OBJ_ADENA,
        &ItemUpdate {
            owner_id: ALICE,
            count: 4_999,
            loc: ItemLocation::Inventory,
            loc_data: 1,
            enchant_level: 2,
            custom_type1: 3,
            custom_type2: 4,
            mana_left: 120,
            time: NOW,
        },
    )
    .await
    .unwrap();
    let inv = repo
        .inventory_items(ALICE, ItemLocation::Inventory, ItemLocation::Paperdoll)
        .await
        .unwrap();
    let row = &inv[0];
    assert_eq!(row.object_id, OBJ_ADENA);
    assert_eq!(row.count, 4_999);
    assert_eq!(
        (
            row.loc_data,
            row.enchant_level,
            row.custom_type1,
            row.custom_type2
        ),
        (Some(1), Some(2), Some(3), Some(4))
    );
    assert_eq!((row.mana_left, row.time), (120, NOW));

    // Augmentation rows: REPLACE on the same item, then removed.
    assert_eq!(repo.augmentation_of(OBJ_SWORD).await.unwrap(), None);
    repo.set_augmentation(OBJ_SWORD, 0x0001_0002).await.unwrap();
    assert_eq!(repo.augmentation_of(OBJ_SWORD).await.unwrap(), Some(0x0001_0002));
    repo.set_augmentation(OBJ_SWORD, 0x0003_0004).await.unwrap();
    assert_eq!(repo.augmentation_of(OBJ_SWORD).await.unwrap(), Some(0x0003_0004));
    repo.delete_augmentation(OBJ_SWORD).await.unwrap();
    assert_eq!(repo.augmentation_of(OBJ_SWORD).await.unwrap(), None);

    // Deleting an item removes its augmentation row too.
    repo.set_augmentation(OBJ_CHEST, 5).await.unwrap();
    repo.delete_item(OBJ_CHEST).await.unwrap();
    assert_eq!(db.count("items", "object_id", OBJ_CHEST).await, 0);
    assert_eq!(db.count("item_attributes", "itemId", OBJ_CHEST).await, 0);
    // Bob's item is untouched.
    assert_eq!(db.count("items", "owner_id", BOB).await, 1);

    db.drop_database().await;
}

#[tokio::test]
async fn skills_and_skill_effects_are_kept_per_class_index() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = SkillRepository::new(db.pool.clone());
    let skill = |skill_id, skill_level| SkillLevel {
        skill_id,
        skill_level,
    };

    repo.replace_skills(ALICE, 0, &[skill(1001, 1), skill(1002, 2)])
        .await
        .unwrap();
    repo.replace_skills(ALICE, 1, &[skill(1001, 9)]).await.unwrap();
    repo.replace_skills(ALICE, 0, &[]).await.unwrap();

    let mut base = repo.skills_of(ALICE, 0).await.unwrap();
    base.sort_by_key(|s| s.skill_id);
    assert_eq!(base, [skill(1001, 1), skill(1002, 2)]);
    assert_eq!(repo.skills_of(ALICE, 1).await.unwrap(), [skill(1001, 9)]);

    // A level change, then a REPLACE of an existing row.
    repo.update_skill_level(ALICE, 0, 1001, 3).await.unwrap();
    repo.replace_skills(ALICE, 0, &[skill(1002, 2)]).await.unwrap();
    let mut base = repo.skills_of(ALICE, 0).await.unwrap();
    base.sort_by_key(|s| s.skill_id);
    assert_eq!(base, [skill(1001, 3), skill(1002, 2)]);

    repo.delete_skill(ALICE, 0, 1002).await.unwrap();
    assert_eq!(repo.skills_of(ALICE, 0).await.unwrap(), [skill(1001, 3)]);

    // Removing a subclass's skills leaves the other class index alone.
    repo.delete_class_skills(ALICE, 0).await.unwrap();
    assert!(repo.skills_of(ALICE, 0).await.unwrap().is_empty());
    assert_eq!(repo.skills_of(ALICE, 1).await.unwrap(), [skill(1001, 9)]);

    // Saved effects come back in buff order; systime is unsigned.
    repo.add_skill_effects(
        ALICE,
        0,
        &[
            NewSkillSave {
                skill_id: 1204,
                skill_level: 2,
                remaining_time: 30,
                reuse_delay: 5_000,
                systime: NOW,
                restore_type: 0,
                buff_index: 2,
            },
            NewSkillSave {
                skill_id: 1085,
                skill_level: 1,
                remaining_time: -1,
                reuse_delay: 0,
                systime: 0,
                restore_type: 1,
                buff_index: 1,
            },
        ],
    )
    .await
    .unwrap();
    let effects = repo.skill_effects_of(ALICE, 0).await.unwrap();
    assert_eq!(effects.len(), 2);
    assert_eq!(
        effects[0],
        SkillSaveRow {
            skill_id: 1085,
            skill_level: 1,
            remaining_time: -1,
            reuse_delay: 0,
            systime: 0,
            restore_type: 1,
        }
    );
    assert_eq!(
        effects[1],
        SkillSaveRow {
            skill_id: 1204,
            skill_level: 2,
            remaining_time: 30,
            reuse_delay: 5_000,
            systime: NOW as u64,
            restore_type: 0,
        }
    );
    assert!(repo.skill_effects_of(ALICE, 1).await.unwrap().is_empty());
    repo.delete_skill_effects(ALICE, 0).await.unwrap();
    assert!(repo.skill_effects_of(ALICE, 0).await.unwrap().is_empty());

    db.drop_database().await;
}

#[tokio::test]
async fn shortcuts_are_replaced_per_slot_and_class_index() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = ShortcutRepository::new(db.pool.clone());

    repo.replace_shortcut(
        ALICE,
        0,
        &NewShortcut {
            slot: 0,
            page: 0,
            kind: 2,
            shortcut_id: 1001,
            level: 3,
        },
    )
    .await
    .unwrap();
    repo.replace_shortcut(
        ALICE,
        0,
        &NewShortcut {
            slot: 11,
            page: 3,
            kind: 1,
            shortcut_id: OBJ_SWORD,
            level: 0,
        },
    )
    .await
    .unwrap();
    repo.replace_shortcut(
        ALICE,
        1,
        &NewShortcut {
            slot: 0,
            page: 0,
            kind: 1,
            shortcut_id: OBJ_CHEST,
            level: 0,
        },
    )
    .await
    .unwrap();
    // Same slot and page: the row is replaced, not duplicated.
    repo.replace_shortcut(
        ALICE,
        0,
        &NewShortcut {
            slot: 0,
            page: 0,
            kind: 2,
            shortcut_id: 1002,
            level: 12,
        },
    )
    .await
    .unwrap();

    let mut bar = repo.shortcuts_of(ALICE, 0).await.unwrap();
    bar.sort_by_key(|s| (s.page, s.slot));
    assert_eq!(
        bar,
        [
            ShortcutRow {
                char_id: ALICE as u32,
                slot: 0,
                page: 0,
                kind: Some(2),
                shortcut_id: Some(1002),
                level: Some(12),
            },
            ShortcutRow {
                char_id: ALICE as u32,
                slot: 11,
                page: 3,
                kind: Some(1),
                shortcut_id: Some(i64::from(OBJ_SWORD)),
                level: Some(0),
            },
        ]
    );
    assert_eq!(repo.shortcuts_of(ALICE, 1).await.unwrap().len(), 1);

    repo.delete_shortcut(ALICE, 0, 11, 3).await.unwrap();
    assert_eq!(repo.shortcuts_of(ALICE, 0).await.unwrap().len(), 1);

    repo.delete_class_shortcuts(ALICE, 0).await.unwrap();
    assert!(repo.shortcuts_of(ALICE, 0).await.unwrap().is_empty());
    assert_eq!(repo.shortcuts_of(ALICE, 1).await.unwrap().len(), 1);

    db.drop_database().await;
}

#[tokio::test]
async fn subclasses_insert_update_and_delete() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = SubclassRepository::new(db.pool.clone());

    repo.insert_subclass(
        ALICE,
        2,
        &SubclassValues {
            class_id: 118,
            exp: 1_000,
            sp: 5,
            level: 40,
        },
    )
    .await
    .unwrap();
    repo.insert_subclass(
        ALICE,
        1,
        &SubclassValues {
            class_id: 114,
            exp: 0,
            sp: 0,
            level: 40,
        },
    )
    .await
    .unwrap();
    // One row per (charId, class_id): the same class cannot be added twice.
    assert!(
        repo.insert_subclass(
            ALICE,
            3,
            &SubclassValues {
                class_id: 114,
                exp: 0,
                sp: 0,
                level: 40,
            },
        )
        .await
        .is_err()
    );

    // Lowest class index first.
    let subs = repo.subclasses_of(ALICE).await.unwrap();
    assert_eq!(
        subs,
        [
            SubclassRow {
                class_id: 114,
                exp: 0,
                sp: 0,
                level: 40,
                class_index: 1,
            },
            SubclassRow {
                class_id: 118,
                exp: 1_000,
                sp: 5,
                level: 40,
                class_index: 2,
            },
        ]
    );

    repo.update_subclass(
        ALICE,
        2,
        &SubclassValues {
            class_id: 119,
            exp: 4_200_000_000,
            sp: 123_456,
            level: 41,
        },
    )
    .await
    .unwrap();
    let subs = repo.subclasses_of(ALICE).await.unwrap();
    assert_eq!(
        subs[1],
        SubclassRow {
            class_id: 119,
            exp: 4_200_000_000,
            sp: 123_456,
            level: 41,
            class_index: 2,
        }
    );

    repo.delete_subclass(ALICE, 1).await.unwrap();
    let subs = repo.subclasses_of(ALICE).await.unwrap();
    assert_eq!(subs.len(), 1);
    assert_eq!(subs[0].class_index, 2);
    assert!(repo.subclasses_of(BOB).await.unwrap().is_empty());

    db.drop_database().await;
}
