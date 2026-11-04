//! The `character_offline_trade` and `character_offline_trade_items` tables: the shops of the
//! characters that stay in the world without a client (offline trade).

use sqlx::FromRow;

use crate::error::Result;
use crate::pool::Pool;

const SAVE_OFFLINE_STATUS: &str =
    "INSERT INTO character_offline_trade (`charId`,`time`,`type`,`title`) VALUES (?,?,?,?)";
const SAVE_ITEMS: &str =
    "INSERT INTO character_offline_trade_items (`charId`,`item`,`count`,`price`) VALUES (?,?,?,?)";
const CLEAR_OFFLINE_TABLE: &str = "DELETE FROM character_offline_trade";
const CLEAR_OFFLINE_TABLE_PLAYER: &str = "DELETE FROM character_offline_trade WHERE `charId`=?";
const CLEAR_OFFLINE_TABLE_ITEMS: &str = "DELETE FROM character_offline_trade_items";
const CLEAR_OFFLINE_TABLE_ITEMS_PLAYER: &str = "DELETE FROM character_offline_trade_items WHERE `charId`=?";
const LOAD_OFFLINE_STATUS: &str = "SELECT * FROM character_offline_trade";
const LOAD_OFFLINE_ITEMS: &str = "SELECT * FROM character_offline_trade_items WHERE `charId`=?";

/// A row of `character_offline_trade`.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct OfflineTradeStatusRow {
    /// `charId`.
    #[sqlx(rename = "charId")]
    pub char_id: u32,
    /// `time`: Unix milliseconds the character went offline.
    pub time: u64,
    /// `type`: the `PrivateStoreType` id (`SELL_BUFFS` for a buff shop).
    #[sqlx(rename = "type")]
    pub store_type: i8,
    /// `title` (NULL when Java wrote none).
    pub title: Option<String>,
}

/// A row of `character_offline_trade_items`.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct OfflineTradeItemRow {
    /// `charId`.
    #[sqlx(rename = "charId")]
    pub char_id: u32,
    /// `item`: the item id (buy store), the item's object id (sell store) or the recipe id
    /// (workshop).
    pub item: u32,
    pub count: u64,
    pub price: u64,
}

/// Every query on the offline trade tables.
#[derive(Debug, Clone)]
pub struct OfflineTradeRepository {
    pool: Pool,
}

impl OfflineTradeRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// `DELETE FROM character_offline_trade` and `DELETE FROM character_offline_trade_items`.
    pub async fn clear_all(&self) -> Result<()> {
        sqlx::query(CLEAR_OFFLINE_TABLE).execute(&self.pool).await?;
        sqlx::query(CLEAR_OFFLINE_TABLE_ITEMS).execute(&self.pool).await?;
        Ok(())
    }

    /// `DELETE FROM character_offline_trade_items WHERE charId=?`.
    pub async fn clear_player_items(&self, char_id: i32) -> Result<()> {
        sqlx::query(CLEAR_OFFLINE_TABLE_ITEMS_PLAYER)
            .bind(char_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// `DELETE FROM character_offline_trade WHERE charId=?`.
    pub async fn clear_player_status(&self, char_id: i32) -> Result<()> {
        sqlx::query(CLEAR_OFFLINE_TABLE_PLAYER)
            .bind(char_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// `INSERT INTO character_offline_trade_items (charId,item,count,price) VALUES (?,?,?,?)`.
    pub async fn insert_item(&self, char_id: i32, item: i32, count: i64, price: i64) -> Result<()> {
        sqlx::query(SAVE_ITEMS)
            .bind(char_id)
            .bind(item)
            .bind(count)
            .bind(price)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// `INSERT INTO character_offline_trade (charId,time,type,title) VALUES (?,?,?,?)`.
    pub async fn insert_status(
        &self,
        char_id: i32,
        time: i64,
        store_type: i32,
        title: Option<&str>,
    ) -> Result<()> {
        sqlx::query(SAVE_OFFLINE_STATUS)
            .bind(char_id)
            .bind(time)
            .bind(store_type)
            .bind(title)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Every shop in the table (the rows `restoreOfflineTraders` walks).
    pub async fn load_status(&self) -> Result<Vec<OfflineTradeStatusRow>> {
        Ok(sqlx::query_as::<_, OfflineTradeStatusRow>(LOAD_OFFLINE_STATUS)
            .fetch_all(&self.pool)
            .await?)
    }

    /// The lines of one shop, in the order the rows come back.
    pub async fn load_items(&self, char_id: i32) -> Result<Vec<OfflineTradeItemRow>> {
        Ok(sqlx::query_as::<_, OfflineTradeItemRow>(LOAD_OFFLINE_ITEMS)
            .bind(char_id)
            .fetch_all(&self.pool)
            .await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statements_have_one_placeholder_per_bind() {
        assert_eq!(SAVE_OFFLINE_STATUS.matches('?').count(), 4);
        assert_eq!(SAVE_ITEMS.matches('?').count(), 4);
        assert_eq!(CLEAR_OFFLINE_TABLE_PLAYER.matches('?').count(), 1);
        assert_eq!(CLEAR_OFFLINE_TABLE_ITEMS_PLAYER.matches('?').count(), 1);
        assert_eq!(LOAD_OFFLINE_ITEMS.matches('?').count(), 1);
        assert_eq!(CLEAR_OFFLINE_TABLE.matches('?').count(), 0);
        assert_eq!(CLEAR_OFFLINE_TABLE_ITEMS.matches('?').count(), 0);
        assert_eq!(LOAD_OFFLINE_STATUS.matches('?').count(), 0);
    }
}
