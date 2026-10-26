#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Integration test of `OfflineTradeRepository` against a real MySQL/MariaDB server: the statements
//! of `OfflineTraderTable` (the inserts and deletes of `character_offline_trade` and
//! `character_offline_trade_items`, the two loads) on the unchanged schema.
//!
//! Runs only when `L2_TEST_DATABASE_URL` is set (CI sets it for a throwaway MariaDB service on
//! Linux). Never point it at a server with real data: the test only creates and drops a database
//! named `l2_test_*`, but it needs the rights to do so.

use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use l2_db::{OfflineTradeItemRow, OfflineTradeRepository, OfflineTradeStatusRow, Pool, connect_with};
use sqlx::mysql::{MySqlConnectOptions, MySqlConnection};
use sqlx::{AssertSqlSafe, Connection};

const SCHEMA: [&str; 2] = [
    include_str!("../sql/game/character_offline_trade.sql"),
    include_str!("../sql/game/character_offline_trade_items.sql"),
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
            "l2_test_offline_{}_{}_{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed),
            nanos % 1_000_000_000
        );
        assert!(name.starts_with("l2_test_"));
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
}

const TRADER: i32 = 0x6000_0001;
const OTHER: i32 = 0x6000_0002;
const TIME: i64 = 1_700_000_000_000;

#[tokio::test]
async fn a_shop_is_written_read_and_removed() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = OfflineTradeRepository::new(db.pool.clone());
    assert!(repo.load_status().await.unwrap().is_empty());

    // `SAVE_ITEMS` then `SAVE_OFFLINE_STATUS`, as `storeOffliners` does for each trader.
    repo.insert_item(TRADER, 0x1000_0064, 1, 500).await.unwrap();
    repo.insert_item(TRADER, 0x1000_0069, 4, 30).await.unwrap();
    repo.insert_status(TRADER, TIME, 1, Some("Cheap")).await.unwrap();
    repo.insert_item(OTHER, 5, 0, 800).await.unwrap();
    repo.insert_status(OTHER, TIME + 1, 5, None).await.unwrap();

    let mut status = repo.load_status().await.unwrap();
    status.sort_by_key(|r| r.char_id);
    assert_eq!(
        status,
        vec![
            OfflineTradeStatusRow {
                char_id: TRADER as u32,
                time: TIME as u64,
                store_type: 1,
                title: Some("Cheap".into()),
            },
            OfflineTradeStatusRow {
                char_id: OTHER as u32,
                time: TIME as u64 + 1,
                store_type: 5,
                title: None,
            },
        ]
    );
    let items = repo.load_items(TRADER).await.unwrap();
    assert_eq!(
        items,
        vec![
            OfflineTradeItemRow {
                char_id: TRADER as u32,
                item: 0x1000_0064,
                count: 1,
                price: 500
            },
            OfflineTradeItemRow {
                char_id: TRADER as u32,
                item: 0x1000_0069,
                count: 4,
                price: 30
            },
        ]
    );

    // `onTransaction`: the lines of one trader go and are written again; the row stays.
    repo.clear_player_items(TRADER).await.unwrap();
    assert!(repo.load_items(TRADER).await.unwrap().is_empty());
    assert_eq!(repo.load_items(OTHER).await.unwrap().len(), 1);
    assert_eq!(repo.load_status().await.unwrap().len(), 2);

    // `removeTrader`: lines and row.
    repo.clear_player_items(OTHER).await.unwrap();
    repo.clear_player_status(OTHER).await.unwrap();
    let status = repo.load_status().await.unwrap();
    assert_eq!(status.len(), 1);
    assert_eq!(status[0].char_id, TRADER as u32);

    // `storeOffliners` starts with both tables emptied.
    repo.insert_item(TRADER, 7, 1, 1).await.unwrap();
    repo.clear_all().await.unwrap();
    assert!(repo.load_status().await.unwrap().is_empty());
    assert!(repo.load_items(TRADER).await.unwrap().is_empty());
    db.drop_database().await;
}
