//! The item auction tables: `item_auction` and `item_auction_bid` (L-05).
// Java: gameserver/managers/ItemAuctionManager.java (the highest auction id, deleteAuction)
// Java: gameserver/entity/item/auction/ItemAuctionInstance.java (the SELECT_* and DELETE_*
//       statements, loadAuction)
// Java: gameserver/entity/item/auction/ItemAuction.java (storeMe, updatePlayerBidInternal)
//
// Same layout as auctions.rs: verbatim SQL, one method per statement. D-39: the Java game server
// reads and writes the same rows.

use sqlx::FromRow;

use crate::error::Result;
use crate::pool::Pool;

// Java: ItemAuctionManager (constructor)
const SELECT_HIGHEST_AUCTION_ID: &str =
    "SELECT auctionId FROM item_auction ORDER BY auctionId DESC LIMIT 0, 1";
// Java: ItemAuctionInstance.SELECT_AUCTION_ID_BY_INSTANCE_ID
const SELECT_AUCTION_ID_BY_INSTANCE_ID: &str = "SELECT auctionId FROM item_auction WHERE instanceId = ?";
// Java: ItemAuctionInstance.SELECT_AUCTION_INFO
const SELECT_AUCTION_INFO: &str =
    "SELECT auctionItemId, startingTime, endingTime, auctionStateId FROM item_auction WHERE auctionId = ? ";
// Java: ItemAuctionInstance.SELECT_PLAYERS_ID_BY_AUCTION_ID
const SELECT_PLAYERS_ID_BY_AUCTION_ID: &str =
    "SELECT playerObjId, playerBid FROM item_auction_bid WHERE auctionId = ?";
// Java: ItemAuction.storeMe
const STORE_AUCTION: &str = "INSERT INTO item_auction (auctionId,instanceId,auctionItemId,startingTime,endingTime,auctionStateId) VALUES (?,?,?,?,?,?) ON DUPLICATE KEY UPDATE auctionStateId=?";
// Java: ItemAuction.INSERT_ITEM_AUCTION_BID
const INSERT_ITEM_AUCTION_BID: &str = "INSERT INTO item_auction_bid (auctionId, playerObjId, playerBid) VALUES (?, ?, ?) ON DUPLICATE KEY UPDATE playerBid = ?";
// Java: ItemAuction.DELETE_ITEM_AUCTION_BID
const DELETE_ITEM_AUCTION_BID: &str = "DELETE FROM item_auction_bid WHERE auctionId = ? AND playerObjId = ?";
// Java: ItemAuctionManager.deleteAuction
const DELETE_AUCTION: &str = "DELETE FROM item_auction WHERE auctionId=?";
const DELETE_AUCTION_BIDS: &str = "DELETE FROM item_auction_bid WHERE auctionId=?";

/// An `item_auction` row as `loadAuction` reads it (the instance id is the one asked for).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemAuctionRow {
    pub auction_id: i32,
    pub instance_id: i32,
    pub auction_item_id: i32,
    pub starting_time: i64,
    pub ending_time: i64,
    /// `auctionStateId`: 0 created, 1 started, 2 finished (anything else is an invalid row).
    pub auction_state_id: i8,
}

#[derive(Debug, Clone, FromRow)]
struct ItemAuctionDbRow {
    #[sqlx(rename = "auctionItemId")]
    auction_item_id: i32,
    #[sqlx(rename = "startingTime")]
    starting_time: u64,
    #[sqlx(rename = "endingTime")]
    ending_time: u64,
    #[sqlx(rename = "auctionStateId")]
    auction_state_id: i8,
}

/// An `item_auction_bid` row.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct ItemAuctionBidRow {
    #[sqlx(rename = "playerObjId")]
    pub player_obj_id: i32,
    /// `playerBid`; -1 is a canceled bid (Java writes the canceled bid back with -1).
    #[sqlx(rename = "playerBid")]
    pub player_bid: i64,
}

/// `getLong` of an unsigned `BIGINT`.
fn long(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

/// Every query on the item auction tables.
#[derive(Debug, Clone)]
pub struct ItemAuctionRepository {
    pool: Pool,
}

impl ItemAuctionRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// The highest `auctionId`, `None` for an empty table.
    // Java: ItemAuctionManager (constructor)
    pub async fn highest_auction_id(&self) -> Result<Option<i32>> {
        let id = sqlx::query_scalar::<_, i32>(SELECT_HIGHEST_AUCTION_ID)
            .fetch_optional(&self.pool)
            .await?;
        Ok(id)
    }

    /// The auction ids of an instance, in the order the database gives them.
    // Java: ItemAuctionInstance (constructor)
    pub async fn auction_ids(&self, instance_id: i32) -> Result<Vec<i32>> {
        let ids = sqlx::query_scalar::<_, i32>(SELECT_AUCTION_ID_BY_INSTANCE_ID)
            .bind(instance_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(ids)
    }

    /// One auction row; `None` when the row is gone (Java warns "Auction data not found").
    // Java: ItemAuctionInstance.loadAuction
    pub async fn auction(&self, auction_id: i32, instance_id: i32) -> Result<Option<ItemAuctionRow>> {
        let rows = sqlx::query_as::<_, ItemAuctionDbRow>(SELECT_AUCTION_INFO)
            .bind(auction_id)
            .fetch_all(&self.pool)
            .await?;
        // Java reads the first row (`rset.next()`).
        Ok(rows.into_iter().next().map(|r| ItemAuctionRow {
            auction_id,
            instance_id,
            auction_item_id: r.auction_item_id,
            starting_time: long(r.starting_time),
            ending_time: long(r.ending_time),
            auction_state_id: r.auction_state_id,
        }))
    }

    /// The bids of an auction, in the order the database gives them.
    // Java: ItemAuctionInstance.loadAuction
    pub async fn bids(&self, auction_id: i32) -> Result<Vec<ItemAuctionBidRow>> {
        let rows = sqlx::query_as::<_, ItemAuctionBidRow>(SELECT_PLAYERS_ID_BY_AUCTION_ID)
            .bind(auction_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    // Java: ItemAuction.storeMe
    pub async fn store_auction(&self, row: &ItemAuctionRow) -> Result<()> {
        sqlx::query(STORE_AUCTION)
            .bind(row.auction_id)
            .bind(row.instance_id)
            .bind(row.auction_item_id)
            .bind(row.starting_time)
            .bind(row.ending_time)
            .bind(row.auction_state_id)
            .bind(row.auction_state_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // Java: ItemAuction.updatePlayerBidInternal (insert)
    pub async fn store_bid(&self, auction_id: i32, player_obj_id: i32, bid: i64) -> Result<()> {
        sqlx::query(INSERT_ITEM_AUCTION_BID)
            .bind(auction_id)
            .bind(player_obj_id)
            .bind(bid)
            .bind(bid)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // Java: ItemAuction.updatePlayerBidInternal (delete)
    pub async fn delete_bid(&self, auction_id: i32, player_obj_id: i32) -> Result<()> {
        sqlx::query(DELETE_ITEM_AUCTION_BID)
            .bind(auction_id)
            .bind(player_obj_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// The auction row and then its bids, two statements like Java.
    // Java: ItemAuctionManager.deleteAuction
    pub async fn delete_auction(&self, auction_id: i32) -> Result<()> {
        sqlx::query(DELETE_AUCTION)
            .bind(auction_id)
            .execute(&self.pool)
            .await?;
        sqlx::query(DELETE_AUCTION_BIDS)
            .bind(auction_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
