#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Integration test of `GroundItemRepository` (`itemsonground`) against a real MySQL/MariaDB
//! server. Same rules as `characters_repo.rs`: runs only when `L2_TEST_DATABASE_URL` is set, on a
//! throwaway `l2_test_*` database.

use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use l2_db::{GroundItemRecord, GroundItemRepository, Pool, connect_with};
use sqlx::mysql::{MySqlConnectOptions, MySqlConnection};
use sqlx::{AssertSqlSafe, Connection};

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

/// A throwaway database with the table.
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
            "l2_test_ground_{}_{}_{}",
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
        for statement in statements(l2_db::game_sql!("itemsonground")) {
            sqlx::query(AssertSqlSafe(statement))
                .execute(&pool)
                .await
                .unwrap();
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
}

fn record(object_id: i32, item_id: i32, drop_time: i64, equipable: bool) -> GroundItemRecord {
    GroundItemRecord {
        object_id,
        item_id,
        count: 3_000_000_000, // above `i32`: the column is `BIGINT UNSIGNED`
        enchant_level: 2,
        x: -71_338,
        y: 258_271,
        z: -3_104,
        drop_time,
        equipable,
    }
}

#[tokio::test]
async fn the_table_is_replaced_and_read_back() {
    let Some(db) = TestDb::create().await else { return };
    let repo = GroundItemRepository::new(db.pool.clone());
    assert!(repo.load_all().await.unwrap().is_empty());

    let first = vec![
        record(0x6000_0001, 57, 1_700_000_000_000, false),
        record(0x6000_0002, 1, -1, true),
    ];
    repo.replace_all(&first).await.unwrap();
    assert_eq!(repo.load_all().await.unwrap(), first);

    // `run()` empties the table first: the old rows are gone.
    let second = vec![record(0x6000_0003, 1060, 1_700_000_000_001, false)];
    repo.replace_all(&second).await.unwrap();
    assert_eq!(repo.load_all().await.unwrap(), second);

    repo.empty_table().await.unwrap();
    assert!(repo.load_all().await.unwrap().is_empty());
    db.drop_database().await;
}

/// `DestroyPlayerDroppedItem` recycles the protected rows (`drop_time = -1`); the equipable ones
/// only with `DestroyEquipableItem`.
#[tokio::test]
async fn protected_rows_are_recycled_with_the_drop_time_given() {
    let Some(db) = TestDb::create().await else { return };
    let repo = GroundItemRepository::new(db.pool.clone());
    repo.replace_all(&[
        record(0x6000_0001, 57, -1, false),
        record(0x6000_0002, 1, -1, true),
        record(0x6000_0003, 1060, 123, false),
    ])
    .await
    .unwrap();

    repo.recycle_protected(555, false).await.unwrap();
    let times: Vec<i64> = repo
        .load_all()
        .await
        .unwrap()
        .iter()
        .map(|r| r.drop_time)
        .collect();
    assert_eq!(times, [555, -1, 123]);

    repo.recycle_protected(777, true).await.unwrap();
    let times: Vec<i64> = repo
        .load_all()
        .await
        .unwrap()
        .iter()
        .map(|r| r.drop_time)
        .collect();
    assert_eq!(times, [555, 777, 123]);
    db.drop_database().await;
}
