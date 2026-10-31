#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Integration test of `BuyListRepository` (`buylists`) against a real MySQL/MariaDB server. Same
//! rules as `characters_repo.rs`: runs only when `L2_TEST_DATABASE_URL` is set, on a throwaway
//! `l2_test_*` database.

use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use l2_db::{BuyListRepository, BuyListRow, Pool, connect_with};
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
            "l2_test_buylists_{}_{}_{}",
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
        for statement in statements(l2_db::game_sql!("buylists")) {
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

fn row(buylist_id: i32, item_id: i32, count: i64, next_restock_time: i64) -> BuyListRow {
    BuyListRow {
        buylist_id,
        item_id,
        count,
        next_restock_time,
    }
}

/// `Product.save`: the first save inserts, the next one updates `count` and `next_restock_time`
/// of the same key (`ON DUPLICATE KEY UPDATE`).
#[tokio::test]
async fn a_product_is_inserted_then_updated() {
    let Some(db) = TestDb::create().await else { return };
    let repo = BuyListRepository::new(db.pool.clone());
    assert!(repo.load_all().await.unwrap().is_empty());

    repo.save_product(&row(382, 1835, 4, 1_700_000_000_000))
        .await
        .unwrap();
    repo.save_product(&row(382, 2509, 7, 0)).await.unwrap();
    let mut rows = repo.load_all().await.unwrap();
    rows.sort_by_key(|r| r.item_id);
    assert_eq!(rows, [row(382, 1835, 4, 1_700_000_000_000), row(382, 2509, 7, 0)]);

    // Same key: one row, new values.
    repo.save_product(&row(382, 1835, 3, 1_700_000_100_000))
        .await
        .unwrap();
    let mut rows = repo.load_all().await.unwrap();
    rows.sort_by_key(|r| r.item_id);
    assert_eq!(rows, [row(382, 1835, 3, 1_700_000_100_000), row(382, 2509, 7, 0)]);
    db.drop_database().await;
}
