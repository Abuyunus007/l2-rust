//! The `buylists` table: the stock and the next restock time of the limited products of NPC
//! buy lists.
// Java: org/l2jmobius/gameserver/data/xml/BuyListData.java (the `SELECT * FROM buylists`),
//       org/l2jmobius/gameserver/mechanics/buylist/Product.java (save)
//
// D-39: the Java game server writes and reads the same rows. The Rust server only reads the
// table at startup and upserts the row of a product when its stock changes, with Java's
// statement.

use sqlx::FromRow;

use crate::error::Result;
use crate::pool::Pool;

// Java: BuyListData.load
const SELECT_BUYLISTS: &str = "SELECT * FROM `buylists`";
// Java: Product.save
const SAVE_PRODUCT: &str = "INSERT INTO `buylists`(`buylist_id`, `item_id`, `count`, `next_restock_time`) VALUES(?, ?, ?, ?) ON DUPLICATE KEY UPDATE `count` = ?, `next_restock_time` = ?";

/// One row of `buylists`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuyListRow {
    pub buylist_id: i32,
    pub item_id: i32,
    pub count: i64,
    pub next_restock_time: i64,
}

/// Database shape of [`BuyListRow`]: `buylist_id` and `item_id` can be NULL in the schema (they
/// are part of the key, but `NOT NULL` is not declared), the counters are `BIGINT UNSIGNED`.
#[derive(Debug, Clone, FromRow)]
struct BuyListDbRow {
    buylist_id: Option<u32>,
    item_id: Option<u32>,
    count: u64,
    next_restock_time: u64,
}

impl From<BuyListDbRow> for BuyListRow {
    fn from(r: BuyListDbRow) -> Self {
        Self {
            // Java reads them with `getInt`, where NULL is 0.
            buylist_id: i32::try_from(r.buylist_id.unwrap_or(0)).unwrap_or(i32::MAX),
            item_id: i32::try_from(r.item_id.unwrap_or(0)).unwrap_or(i32::MAX),
            count: i64::try_from(r.count).unwrap_or(i64::MAX),
            next_restock_time: i64::try_from(r.next_restock_time).unwrap_or(i64::MAX),
        }
    }
}

/// Reads and writes `buylists`.
#[derive(Clone)]
pub struct BuyListRepository {
    pool: Pool,
}

impl BuyListRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Java `BuyListData.load`: every row.
    pub async fn load_all(&self) -> Result<Vec<BuyListRow>> {
        let rows: Vec<BuyListDbRow> = sqlx::query_as(SELECT_BUYLISTS).fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(BuyListRow::from).collect())
    }

    /// Java `Product.save`: inserts the row or updates the count and the restock time.
    pub async fn save_product(&self, row: &BuyListRow) -> Result<()> {
        sqlx::query(SAVE_PRODUCT)
            .bind(row.buylist_id)
            .bind(row.item_id)
            .bind(row.count)
            .bind(row.next_restock_time)
            .bind(row.count)
            .bind(row.next_restock_time)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
