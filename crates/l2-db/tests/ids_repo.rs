#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Integration test of `IdRepository` (the used-object-id scan) against a real MySQL/MariaDB
//! server. Same rules as `characters_repo.rs`: runs only when `L2_TEST_DATABASE_URL` is set,
//! on a throwaway `l2_test_*` database.

use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use l2_db::{IdRepository, Pool, connect_with};
use sqlx::mysql::{MySqlConnectOptions, MySqlConnection};
use sqlx::{AssertSqlSafe, Connection};

/// The five tables Java's IdManager scans.
const SCHEMA: [&str; 5] = [
    include_str!("../sql/game/characters.sql"),
    include_str!("../sql/game/items.sql"),
    include_str!("../sql/game/clan_data.sql"),
    include_str!("../sql/game/itemsonground.sql"),
    include_str!("../sql/game/messages.sql"),
];

/// The Rust range of D-39 (`l2_world::RUST_FIRST_OBJECT_ID`..=`RUST_LAST_OBJECT_ID`).
const RUST_FIRST: i32 = 0x6000_0000;
const RUST_LAST: i32 = 0x7FFF_FFFF;

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

/// A throwaway database with the tables above.
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
            "l2_test_ids_{}_{}_{}",
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

    async fn exec(&self, sql: String) {
        sqlx::query(AssertSqlSafe(sql)).execute(&self.pool).await.unwrap();
    }

    async fn count(&self, table: &str) -> i64 {
        sqlx::query_scalar(AssertSqlSafe(format!("SELECT COUNT(*) FROM {table}")))
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

#[tokio::test]
async fn empty_database_has_no_used_ids() {
    let Some(db) = TestDb::create().await else { return };
    let used = IdRepository::new(db.pool.clone())
        .used_object_ids(RUST_FIRST, RUST_LAST)
        .await
        .unwrap();
    assert!(used.ids.is_empty());
    assert_eq!(used.outside_range, 0);
    db.drop_database().await;
}

#[tokio::test]
async fn scan_reads_all_five_tables_and_keeps_only_the_requested_range() {
    let Some(db) = TestDb::create().await else { return };
    // Java-created rows in Java's range, the way the real database looks.
    db.exec("INSERT INTO characters (charId, char_name) VALUES (268500001, 'java_char')".into())
        .await;
    db.exec("INSERT INTO items (owner_id, object_id, item_id) VALUES (268500001, 268500050, 57)".into())
        .await;
    db.exec("INSERT INTO clan_data (clan_id) VALUES (268500100)".into())
        .await;
    // Rows in the Rust range, one per table, the item id twice (items and itemsonground).
    let r = RUST_FIRST;
    db.exec(format!(
        "INSERT INTO characters (charId, char_name) VALUES ({}, 'rust_char')",
        r + 1
    ))
    .await;
    db.exec(format!(
        "INSERT INTO items (owner_id, object_id, item_id) VALUES ({}, {}, 57)",
        r + 1,
        r + 2
    ))
    .await;
    db.exec(format!("INSERT INTO clan_data (clan_id) VALUES ({})", r + 3))
        .await;
    db.exec(format!(
        "INSERT INTO itemsonground (object_id) VALUES ({})",
        r + 4
    ))
    .await;
    db.exec(format!(
        "INSERT INTO itemsonground (object_id) VALUES ({})",
        r + 2
    ))
    .await;
    db.exec(format!("INSERT INTO messages (messageId) VALUES ({RUST_LAST})"))
        .await;
    // Below Java's range, and an unsigned charId that is a negative `int` in Java.
    db.exec("INSERT INTO items (owner_id, object_id, item_id) VALUES (1, 12345, 57)".into())
        .await;
    db.exec("INSERT INTO characters (charId, char_name) VALUES (4294967295, 'huge')".into())
        .await;

    let repo = IdRepository::new(db.pool.clone());
    let rows_before = [
        db.count("characters").await,
        db.count("items").await,
        db.count("clan_data").await,
        db.count("itemsonground").await,
        db.count("messages").await,
    ];

    let rust = repo.used_object_ids(RUST_FIRST, RUST_LAST).await.unwrap();
    assert_eq!(rust.ids, vec![r + 1, r + 2, r + 3, r + 4, RUST_LAST]);
    // 268500001, 268500050, 268500100, 12345 and 4294967295; the duplicate r + 2 is in range.
    assert_eq!(rust.outside_range, 5);

    // The same scan with Java's own range (what Java's IdManager reads).
    let java = repo.used_object_ids(0x1000_0000, RUST_LAST).await.unwrap();
    assert_eq!(java.ids.len(), 3 + 5);
    assert_eq!(java.ids[0], 268_500_001);
    assert_eq!(java.outside_range, 2);

    // Read-only: nothing changed.
    let rows_after = [
        db.count("characters").await,
        db.count("items").await,
        db.count("clan_data").await,
        db.count("itemsonground").await,
        db.count("messages").await,
    ];
    assert_eq!(rows_before, rows_after);
    db.drop_database().await;
}
