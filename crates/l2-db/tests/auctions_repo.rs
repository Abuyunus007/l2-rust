#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Integration test of `AuctionRepository` against a real MySQL/MariaDB server: Java's clan hall
//! auction statements (`ClanHallAuction.load`, `loadBid`, `updateInDB`, `removeBids`,
//! `confirmAuction`, `ClanHallAuctionManager.initNPC`, `Clan.setAuctionBiddedAt`) on the unchanged
//! schema.
//!
//! Runs only when `L2_TEST_DATABASE_URL` is set (CI sets it for a throwaway MariaDB service on
//! Linux). Never point it at a server with real data: the test only creates and drops a database
//! named `l2_test_*`, but it needs the rights to do so.

use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use l2_db::{AuctionRepository, NewAuction, Pool, connect_with};
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
            "l2_test_auctions_{}_{}_{}",
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

    async fn exec(&self, sql: &str) {
        sqlx::query(AssertSqlSafe(sql.to_owned()))
            .execute(&self.pool)
            .await
            .unwrap();
    }

    async fn scalar(&self, sql: &str) -> i64 {
        sqlx::query_scalar(AssertSqlSafe(format!("SELECT CAST(({sql}) AS SIGNED)")))
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
async fn the_auction_statements_work_on_javas_schema() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = AuctionRepository::new(db.pool.clone());

    // `confirmAuction`: a clan puts hall 36 up for sale.
    let auction = NewAuction {
        id: 36,
        seller_id: 4001,
        seller_name: "Seller".to_owned(),
        seller_clan_name: "Wolves".to_owned(),
        item_type: "ClanHall".to_owned(),
        item_id: 36,
        item_object_id: 0,
        item_name: "The Golden Chamber".to_owned(),
        item_quantity: 0,
        starting_bid: 5_000_000,
        current_bid: 0,
        end_date: 1_700_000_000_000,
    };
    repo.insert_auction(&auction).await.unwrap();
    // `initNPC`: the NPC auction of hall 22 (Java's `ITEM_INIT_DATA` line).
    repo.insert_npc_auction(22, "Moonstone Hall", 20_000_000, 1_073_037_600_000)
        .await
        .unwrap();
    assert_eq!(repo.auction_ids().await.unwrap(), [22, 36]);
    let npc = repo.auction(22).await.unwrap().unwrap();
    assert_eq!(
        (
            npc.seller_id,
            npc.seller_name.as_str(),
            npc.seller_clan_name.as_str()
        ),
        (0, "NPC", "NPC Clan")
    );
    assert_eq!(
        (
            npc.item_type.as_str(),
            npc.item_id,
            npc.item_object_id,
            npc.item_name.as_str()
        ),
        ("ClanHall", 22, 0, "Moonstone Hall")
    );
    assert_eq!(
        (npc.starting_bid, npc.current_bid, npc.end_date),
        (20_000_000, 0, 1_073_037_600_000)
    );
    // The itemQuantity column keeps the 1 of the init data; Java never reads it.
    assert_eq!(
        db.scalar("SELECT itemQuantity FROM auction WHERE id = 22").await,
        1
    );
    let sale = repo.auction(36).await.unwrap().unwrap();
    assert_eq!(
        (sale.seller_id, sale.item_name.as_str()),
        (4001, "The Golden Chamber")
    );
    assert!(repo.auction(99).await.unwrap().is_none());

    // `saveAuctionDate`.
    repo.update_end_date(22, 1_800_000_000_000).await.unwrap();
    assert_eq!(
        repo.auction(22).await.unwrap().unwrap().end_date,
        1_800_000_000_000
    );

    // `updateInDB`: two clans bid, the first raises; `loadBid` reads the highest first.
    repo.insert_bid(1, 36, 500, "Lions leader", 6_000_000, "Lions", 1_700_000_001_000)
        .await
        .unwrap();
    repo.insert_bid(2, 36, 600, "Bears leader", 7_000_000, "Bears", 1_700_000_002_000)
        .await
        .unwrap();
    repo.update_bid(36, 500, "Lions leader", 8_000_000, 1_700_000_003_000)
        .await
        .unwrap();
    let bids = repo.bids(36).await.unwrap();
    assert_eq!(bids.len(), 2);
    assert_eq!(
        (
            bids[0].bidder_id,
            bids[0].max_bid,
            bids[0].clan_name.as_str(),
            bids[0].time_bid
        ),
        (500, 8_000_000, "Lions", 1_700_000_003_000)
    );
    assert_eq!(
        (bids[1].bidder_id, bids[1].bidder_name.as_str()),
        (600, "Bears leader")
    );
    assert!(repo.bids(22).await.unwrap().is_empty());

    // `Clan.setAuctionBiddedAt`.
    db.exec("INSERT INTO clan_data (clan_id, clan_name) VALUES (500, 'Lions')")
        .await;
    repo.update_clan_auction_bid_at(500, 36).await.unwrap();
    assert_eq!(
        db.scalar("SELECT auction_bid_at FROM clan_data WHERE clan_id = 500")
            .await,
        36
    );

    // `cancelBid`, `removeBids`, `deleteAuctionFromDB`.
    repo.delete_bid(36, 600).await.unwrap();
    assert_eq!(repo.bids(36).await.unwrap().len(), 1);
    repo.delete_bids(36).await.unwrap();
    assert!(repo.bids(36).await.unwrap().is_empty());
    repo.delete_auction(36).await.unwrap();
    assert_eq!(repo.auction_ids().await.unwrap(), [22]);

    db.drop_database().await;
}
