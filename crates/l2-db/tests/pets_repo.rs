#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Integration test of `PetRepository` against a real MySQL/MariaDB server.
//!
//! It runs only when `L2_TEST_DATABASE_URL` is set (CI sets it for a throwaway MariaDB service
//! on Linux), e.g. `mysql://root:pw@127.0.0.1:3306/`. Never point it at a server with real
//! data: the test only creates and drops a database named `l2_test_*`, but it needs the rights
//! to do so. The database is built from `sql/game/pets.sql`, `sql/game/items.sql` and
//! `sql/game/character_pet_skills_save.sql`.

use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use l2_db::{NewPetSkillSave, PetRepository, PetSkillSaveRow, PetValues, Pool, connect_with};
use sqlx::mysql::{MySqlConnectOptions, MySqlConnection};
use sqlx::{AssertSqlSafe, Connection};

const SCHEMA: [&str; 3] = [
    include_str!("../sql/game/pets.sql"),
    include_str!("../sql/game/items.sql"),
    include_str!("../sql/game/character_pet_skills_save.sql"),
];

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// The statements of a schema file; whole-line `#` comments are dropped.
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

/// A throwaway database with the pet tables.
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
            "l2_test_pets_{}_{}_{}",
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

    async fn drop_database(mut self) {
        self.pool.close().await;
        sqlx::query(AssertSqlSafe(format!("DROP DATABASE IF EXISTS `{}`", self.name)))
            .execute(&mut self.admin)
            .await
            .unwrap();
    }

    /// An `items` row of the owner (a collar) in `loc`.
    async fn add_item(&self, owner_id: i32, object_id: i32, item_id: i32, loc: &str) {
        sqlx::query(
            "INSERT INTO items (owner_id,item_id,count,loc,loc_data,enchant_level,object_id,custom_type1,custom_type2,mana_left,time) VALUES (?,?,1,?,0,1,?,0,0,-1,0)",
        )
        .bind(owner_id)
        .bind(item_id)
        .bind(loc)
        .bind(object_id)
        .execute(&self.pool)
        .await
        .unwrap();
    }
}

const OWNER: i32 = 268_600_001;
const COLLAR: i32 = 268_700_001;
const OTHER_COLLAR: i32 = 268_700_002;
const BUFFALO_COLLAR_ITEM: i32 = 6648;

fn buffalo(item_obj_id: i32, name: &str, restore: bool) -> PetValues {
    PetValues {
        name: name.to_owned(),
        level: 5,
        cur_hp: 101,
        cur_mp: 50,
        exp: 625_005,
        sp: 12,
        fed: 9,
        owner_id: OWNER,
        restore,
        item_obj_id,
    }
}

/// `Pet.storeMe`: an `INSERT` first, then `UPDATE`s of the same row; `Pet.restore` reads it back;
/// `Pet.destroyControlItem` deletes it.
#[tokio::test]
async fn pet_row_is_inserted_updated_read_and_deleted_like_java() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = PetRepository::new(db.pool.clone());
    assert!(repo.pet_of_control_item(COLLAR).await.unwrap().is_none());

    repo.insert_pet(&buffalo(COLLAR, "Buffy", false)).await.unwrap();
    let row = repo.pet_of_control_item(COLLAR).await.unwrap().unwrap();
    assert_eq!(row.item_obj_id, COLLAR as u32);
    assert_eq!(row.name.as_deref(), Some("Buffy"));
    assert_eq!(
        (row.level, row.exp, row.sp, row.fed),
        (5, Some(625_005), Some(12), Some(9))
    );
    assert_eq!((row.cur_hp, row.cur_mp), (Some(101), Some(50)));
    assert_eq!((row.owner_id, row.restore.as_str()), (OWNER, "false"));

    let mut changed = buffalo(COLLAR, "Buffy", true);
    changed.level = 6;
    changed.fed = 3;
    repo.update_pet(&changed).await.unwrap();
    let row = repo.pet_of_control_item(COLLAR).await.unwrap().unwrap();
    assert_eq!((row.level, row.fed, row.restore.as_str()), (6, Some(3), "true"));

    // `CharSummonTable.init`: the pets that come back.
    repo.insert_pet(&buffalo(OTHER_COLLAR, "Other", false))
        .await
        .unwrap();
    assert_eq!(repo.restoring_pets().await.unwrap(), [(OWNER, COLLAR as u32)]);

    repo.delete_pet(COLLAR).await.unwrap();
    assert!(repo.pet_of_control_item(COLLAR).await.unwrap().is_none());
    assert!(repo.pet_of_control_item(OTHER_COLLAR).await.unwrap().is_some());
    db.drop_database().await;
}

/// `Player.storePetFood`: only `fed` of the one row changes.
#[tokio::test]
async fn update_fed_changes_only_the_food_of_one_row() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = PetRepository::new(db.pool.clone());
    repo.insert_pet(&buffalo(COLLAR, "Buffy", true)).await.unwrap();
    repo.insert_pet(&buffalo(OTHER_COLLAR, "Other", false))
        .await
        .unwrap();
    repo.update_fed(COLLAR, 4_321).await.unwrap();
    let row = repo.pet_of_control_item(COLLAR).await.unwrap().unwrap();
    assert_eq!(
        (row.fed, row.name.as_deref(), row.restore.as_str()),
        (Some(4_321), Some("Buffy"), "true")
    );
    let other = repo.pet_of_control_item(OTHER_COLLAR).await.unwrap().unwrap();
    assert_eq!(other.fed, Some(9));
    // A collar without a row: nothing happens (Java's UPDATE matches no row).
    repo.update_fed(1, 5).await.unwrap();
    db.drop_database().await;
}

/// The pets of a character are the rows of the collars it holds in its inventory.
#[tokio::test]
async fn pets_of_owner_are_the_rows_of_the_collars_in_the_inventory() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = PetRepository::new(db.pool.clone());
    db.add_item(OWNER, COLLAR, BUFFALO_COLLAR_ITEM, "INVENTORY").await;
    db.add_item(999, OTHER_COLLAR, BUFFALO_COLLAR_ITEM, "INVENTORY")
        .await;
    repo.insert_pet(&buffalo(COLLAR, "Mine", false)).await.unwrap();
    repo.insert_pet(&buffalo(OTHER_COLLAR, "Theirs", false))
        .await
        .unwrap();
    let rows = repo.pets_of_owner(OWNER).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name.as_deref(), Some("Mine"));
    // `PetNameTable.doesPetNameExist`: the same name on the same kind of collar; the compare is
    // case insensitive like the column's collation.
    assert!(repo.pet_name_exists("Mine", BUFFALO_COLLAR_ITEM).await.unwrap());
    assert!(repo.pet_name_exists("mine", BUFFALO_COLLAR_ITEM).await.unwrap());
    assert!(!repo.pet_name_exists("Mine", 1).await.unwrap());
    assert!(!repo.pet_name_exists("Nobody", BUFFALO_COLLAR_ITEM).await.unwrap());
    let mut names = repo.all_pet_names().await.unwrap();
    names.sort();
    assert_eq!(
        names,
        [
            ("Mine".to_owned(), BUFFALO_COLLAR_ITEM),
            ("Theirs".to_owned(), BUFFALO_COLLAR_ITEM)
        ]
    );
    db.drop_database().await;
}

/// `Pet.storeEffect`: the rows of the pet are replaced, `buff_index` counts from 1; the delete alone
/// is `restoreEffects`.
#[tokio::test]
async fn pet_buffs_are_replaced_per_pet() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = PetRepository::new(db.pool.clone());
    db.add_item(OWNER, COLLAR, BUFFALO_COLLAR_ITEM, "INVENTORY").await;
    db.add_item(OWNER, OTHER_COLLAR, BUFFALO_COLLAR_ITEM, "INVENTORY")
        .await;
    let save = |skill_id, remaining_time| NewPetSkillSave {
        skill_id,
        skill_level: 1,
        remaining_time,
    };
    let row = |pet, skill_id, remaining_time, buff_index| PetSkillSaveRow {
        pet_obj_item_id: pet,
        skill_id,
        skill_level: 1,
        remaining_time,
        buff_index,
    };
    repo.store_effects(COLLAR, &[save(1040, 100), save(1068, 50)])
        .await
        .unwrap();
    repo.store_effects(OTHER_COLLAR, &[save(1204, 10)]).await.unwrap();
    repo.store_effects(COLLAR, &[save(1068, 40)]).await.unwrap();
    assert_eq!(repo.effects_of(COLLAR).await.unwrap(), [row(COLLAR, 1068, 40, 1)]);
    assert_eq!(
        repo.effects_of_owner(OWNER).await.unwrap(),
        [row(COLLAR, 1068, 40, 1), row(OTHER_COLLAR, 1204, 10, 1)]
    );
    repo.store_effects(COLLAR, &[]).await.unwrap();
    assert!(repo.effects_of(COLLAR).await.unwrap().is_empty());
    assert_eq!(repo.effects_of(OTHER_COLLAR).await.unwrap().len(), 1);
    db.drop_database().await;
}
