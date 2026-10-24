//! The clan hall auction tables: `auction`, `auction_bid` and the `auction_bid_at` column of
//! `clan_data` (M7-CH-01).
// Java: org/l2jmobius/gameserver/entity/residences/ClanHallAuction.java (load, loadBid,
//       saveAuctionDate, updateInDB, removeBids, deleteAuctionFromDB, cancelBid, confirmAuction)
// Java: org/l2jmobius/gameserver/managers/ClanHallAuctionManager.java (load, initNPC)
// Java: org/l2jmobius/gameserver/entity/clan/Clan.java (setAuctionBiddedAt)
//
// Same layout as residences.rs: verbatim SQL, one method per statement. D-39: the Java game
// server reads and writes the same rows.

use sqlx::FromRow;

use crate::error::Result;
use crate::pool::Pool;

// Java: ClanHallAuctionManager.load
const SELECT_AUCTION_IDS: &str = "SELECT id FROM auction ORDER BY id";
// Java: ClanHallAuction.load
const SELECT_AUCTION: &str = "Select * from auction where id = ?";
// Java: ClanHallAuction.loadBid
const SELECT_BIDS: &str = "SELECT bidderId, bidderName, maxBid, clan_name, time_bid FROM auction_bid WHERE auctionId = ? ORDER BY maxBid DESC";
// Java: ClanHallAuction.saveAuctionDate
const UPDATE_END_DATE: &str = "Update auction set endDate = ? where id = ?";
// Java: ClanHallAuction.updateInDB
const UPDATE_BID: &str = "UPDATE auction_bid SET bidderId=?, bidderName=?, maxBid=?, time_bid=? WHERE auctionId=? AND bidderId=?";
const INSERT_BID: &str = "INSERT INTO auction_bid (id, auctionId, bidderId, bidderName, maxBid, clan_name, time_bid) VALUES (?, ?, ?, ?, ?, ?, ?)";
// Java: ClanHallAuction.removeBids
const DELETE_BIDS: &str = "DELETE FROM auction_bid WHERE auctionId=?";
// Java: ClanHallAuction.deleteAuctionFromDB
const DELETE_AUCTION: &str = "DELETE FROM auction WHERE itemId=?";
// Java: ClanHallAuction.cancelBid
const DELETE_BID: &str = "DELETE FROM auction_bid WHERE auctionId=? AND bidderId=?";
// Java: ClanHallAuction.confirmAuction
const INSERT_AUCTION: &str = "INSERT INTO auction (id, sellerId, sellerName, sellerClanName, itemType, itemId, itemObjectId, itemName, itemQuantity, startingBid, currentBid, endDate) VALUES (?,?,?,?,?,?,?,?,?,?,?,?)";
// Java: ClanHallAuctionManager.initNPC (`ITEM_INIT_DATA`: id, 0, 'NPC', 'NPC Clan', 'ClanHall', id,
// 0, name, 1, starting bid, 0, end date)
const INSERT_NPC_AUCTION: &str = "INSERT INTO `auction` VALUES (?, 0, 'NPC', 'NPC Clan', 'ClanHall', ?, 0, ?, 1, ?, 0, ?)";
// Java: Clan.setAuctionBiddedAt
const UPDATE_CLAN_AUCTION_BID_AT: &str = "UPDATE clan_data SET auction_bid_at=? WHERE clan_id=?";

/// An `auction` row (`SELECT *`). `itemQuantity` is not read: Java keeps it at 0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuctionRow {
    pub id: i32,
    pub seller_id: i32,
    pub seller_name: String,
    pub seller_clan_name: String,
    pub item_type: String,
    pub item_id: i32,
    pub item_object_id: i32,
    pub item_name: String,
    pub starting_bid: i64,
    pub current_bid: i64,
    pub end_date: i64,
}

#[derive(Debug, Clone, FromRow)]
struct AuctionDbRow {
    id: i32,
    #[sqlx(rename = "sellerId")]
    seller_id: i32,
    #[sqlx(rename = "sellerName")]
    seller_name: String,
    #[sqlx(rename = "sellerClanName")]
    seller_clan_name: String,
    #[sqlx(rename = "itemType")]
    item_type: String,
    #[sqlx(rename = "itemId")]
    item_id: i32,
    #[sqlx(rename = "itemObjectId")]
    item_object_id: i32,
    #[sqlx(rename = "itemName")]
    item_name: String,
    #[sqlx(rename = "startingBid")]
    starting_bid: u64,
    #[sqlx(rename = "currentBid")]
    current_bid: u64,
    #[sqlx(rename = "endDate")]
    end_date: u64,
}

impl From<AuctionDbRow> for AuctionRow {
    fn from(r: AuctionDbRow) -> Self {
        Self {
            id: r.id,
            seller_id: r.seller_id,
            seller_name: r.seller_name,
            seller_clan_name: r.seller_clan_name,
            item_type: r.item_type,
            item_id: r.item_id,
            item_object_id: r.item_object_id,
            item_name: r.item_name,
            starting_bid: long(r.starting_bid),
            current_bid: long(r.current_bid),
            end_date: long(r.end_date),
        }
    }
}

/// An `auction_bid` row as `loadBid` reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuctionBidRow {
    pub bidder_id: i32,
    pub bidder_name: String,
    pub max_bid: i64,
    pub clan_name: String,
    pub time_bid: i64,
}

#[derive(Debug, Clone, FromRow)]
struct AuctionBidDbRow {
    #[sqlx(rename = "bidderId")]
    bidder_id: i32,
    #[sqlx(rename = "bidderName")]
    bidder_name: String,
    #[sqlx(rename = "maxBid")]
    max_bid: u64,
    clan_name: String,
    time_bid: u64,
}

impl From<AuctionBidDbRow> for AuctionBidRow {
    fn from(r: AuctionBidDbRow) -> Self {
        Self {
            bidder_id: r.bidder_id,
            bidder_name: r.bidder_name,
            max_bid: long(r.max_bid),
            clan_name: r.clan_name,
            time_bid: long(r.time_bid),
        }
    }
}

/// The values of `confirmAuction`'s `INSERT INTO auction`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewAuction {
    pub id: i32,
    pub seller_id: i32,
    pub seller_name: String,
    pub seller_clan_name: String,
    pub item_type: String,
    pub item_id: i32,
    pub item_object_id: i32,
    pub item_name: String,
    pub item_quantity: i64,
    pub starting_bid: i64,
    pub current_bid: i64,
    pub end_date: i64,
}

/// `getLong` of an unsigned `BIGINT`.
fn long(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

/// Every query on the auction tables.
#[derive(Debug, Clone)]
pub struct AuctionRepository {
    pool: Pool,
}

impl AuctionRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    // Java: ClanHallAuctionManager.load
    pub async fn auction_ids(&self) -> Result<Vec<i32>> {
        let ids = sqlx::query_scalar::<_, i32>(SELECT_AUCTION_IDS)
            .fetch_all(&self.pool)
            .await?;
        Ok(ids)
    }

    /// The `auction` row of an id; Java loops over the result and keeps the last row.
    // Java: ClanHallAuction.load
    pub async fn auction(&self, id: i32) -> Result<Option<AuctionRow>> {
        let rows = sqlx::query_as::<_, AuctionDbRow>(SELECT_AUCTION)
            .bind(id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().last().map(AuctionRow::from))
    }

    /// The bids of an auction, the highest first.
    // Java: ClanHallAuction.loadBid
    pub async fn bids(&self, auction_id: i32) -> Result<Vec<AuctionBidRow>> {
        let rows = sqlx::query_as::<_, AuctionBidDbRow>(SELECT_BIDS)
            .bind(auction_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(AuctionBidRow::from).collect())
    }

    // Java: ClanHallAuction.saveAuctionDate
    pub async fn update_end_date(&self, id: i32, end_date: i64) -> Result<()> {
        sqlx::query(UPDATE_END_DATE)
            .bind(end_date)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // Java: ClanHallAuction.updateInDB (a clan that bid before)
    pub async fn update_bid(
        &self,
        auction_id: i32,
        bidder_id: i32,
        bidder_name: &str,
        max_bid: i64,
        time_bid: i64,
    ) -> Result<()> {
        sqlx::query(UPDATE_BID)
            .bind(bidder_id)
            .bind(bidder_name)
            .bind(max_bid)
            .bind(time_bid)
            .bind(auction_id)
            .bind(bidder_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // Java: ClanHallAuction.updateInDB (a new bidder)
    #[allow(clippy::too_many_arguments)] // Java's statement has seven columns.
    pub async fn insert_bid(
        &self,
        id: i32,
        auction_id: i32,
        bidder_id: i32,
        bidder_name: &str,
        max_bid: i64,
        clan_name: &str,
        time_bid: i64,
    ) -> Result<()> {
        sqlx::query(INSERT_BID)
            .bind(id)
            .bind(auction_id)
            .bind(bidder_id)
            .bind(bidder_name)
            .bind(max_bid)
            .bind(clan_name)
            .bind(time_bid)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // Java: ClanHallAuction.removeBids
    pub async fn delete_bids(&self, auction_id: i32) -> Result<()> {
        sqlx::query(DELETE_BIDS)
            .bind(auction_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // Java: ClanHallAuction.deleteAuctionFromDB (by the item id)
    pub async fn delete_auction(&self, item_id: i32) -> Result<()> {
        sqlx::query(DELETE_AUCTION)
            .bind(item_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // Java: ClanHallAuction.cancelBid
    pub async fn delete_bid(&self, auction_id: i32, bidder_id: i32) -> Result<()> {
        sqlx::query(DELETE_BID)
            .bind(auction_id)
            .bind(bidder_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // Java: ClanHallAuction.confirmAuction
    pub async fn insert_auction(&self, auction: &NewAuction) -> Result<()> {
        sqlx::query(INSERT_AUCTION)
            .bind(auction.id)
            .bind(auction.seller_id)
            .bind(&auction.seller_name)
            .bind(&auction.seller_clan_name)
            .bind(&auction.item_type)
            .bind(auction.item_id)
            .bind(auction.item_object_id)
            .bind(&auction.item_name)
            .bind(auction.item_quantity)
            .bind(auction.starting_bid)
            .bind(auction.current_bid)
            .bind(auction.end_date)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // Java: ClanHallAuctionManager.initNPC
    pub async fn insert_npc_auction(
        &self,
        hall_id: i32,
        name: &str,
        starting_bid: i64,
        end_date: i64,
    ) -> Result<()> {
        sqlx::query(INSERT_NPC_AUCTION)
            .bind(hall_id)
            .bind(hall_id)
            .bind(name)
            .bind(starting_bid)
            .bind(end_date)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // Java: Clan.setAuctionBiddedAt(id, true)
    pub async fn update_clan_auction_bid_at(&self, clan_id: i32, auction_id: i32) -> Result<()> {
        sqlx::query(UPDATE_CLAN_AUCTION_BID_AT)
            .bind(auction_id)
            .bind(clan_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
