#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Integration test of `ItemAuctionRepository` against a real MySQL/MariaDB server: Java's item
//! auction statements (`ItemAuction.storeMe`, `updatePlayerBidInternal`,
//! `ItemAuctionInstance.loadAuction`, `ItemAuctionManager.deleteAuction`) on the unchanged schema.
//!
//! Runs only when `L2_TEST_DATABASE_URL` is set (CI sets it for a throwaway MariaDB service on
//! Linux). Never point it at a server with real data: the test only creates and drops a database
//! named `l2_test_*`, but it needs the rights to do so.

use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use l2_db::{ItemAuctionRepository, ItemAuctionRow, Pool, connect_with};
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
            "l2_test_item_auctions_{}_{}_{}",
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
        for file in l2_db::GAME_SCHEMA {
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

#[tokio::test]
async fn the_item_auction_statements_work_on_javas_schema() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = ItemAuctionRepository::new(db.pool.clone());
    assert_eq!(repo.highest_auction_id().await.unwrap(), None);

    // `storeMe`: a created auction, then its start (same statement, the state changes).
    let mut row = ItemAuctionRow {
        auction_id: 7,
        instance_id: 32320,
        auction_item_id: 3,
        starting_time: 1_800_000_000_000,
        ending_time: 1_800_018_000_000,
        auction_state_id: 0,
    };
    repo.store_auction(&row).await.unwrap();
    let other = ItemAuctionRow {
        auction_id: 8,
        instance_id: 32321,
        ..row.clone()
    };
    repo.store_auction(&other).await.unwrap();
    row.auction_state_id = 1;
    repo.store_auction(&row).await.unwrap();
    assert_eq!(repo.highest_auction_id().await.unwrap(), Some(8));
    assert_eq!(repo.auction_ids(32320).await.unwrap(), [7]);
    assert_eq!(repo.auction(7, 32320).await.unwrap(), Some(row.clone()));
    assert_eq!(repo.auction(99, 32320).await.unwrap(), None);

    // `updatePlayerBidInternal`: two bidders, the first raises, the second cancels (written as -1).
    repo.store_bid(7, 1001, 100_000_000).await.unwrap();
    repo.store_bid(7, 1002, 120_000_000).await.unwrap();
    repo.store_bid(7, 1001, 150_000_000).await.unwrap();
    repo.store_bid(7, 1002, -1).await.unwrap();
    let mut bids = repo.bids(7).await.unwrap();
    bids.sort_by_key(|b| b.player_obj_id);
    assert_eq!(
        bids.iter()
            .map(|b| (b.player_obj_id, b.player_bid))
            .collect::<Vec<_>>(),
        [(1001, 150_000_000), (1002, -1)]
    );
    // The cancel of a finished auction deletes the row.
    repo.delete_bid(7, 1002).await.unwrap();
    assert_eq!(repo.bids(7).await.unwrap().len(), 1);

    // `deleteAuction`: the row and its bids.
    repo.delete_auction(7).await.unwrap();
    assert_eq!(repo.auction(7, 32320).await.unwrap(), None);
    assert!(repo.bids(7).await.unwrap().is_empty());
    assert_eq!(repo.auction_ids(32321).await.unwrap(), [8]);

    db.drop_database().await;
}
