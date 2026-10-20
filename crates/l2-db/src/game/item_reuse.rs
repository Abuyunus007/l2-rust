//! The `character_item_reuse_save` table: the item reuse timers that survive a logout.
// Java: org/l2jmobius/gameserver/entity/actor/Player.java (ADD_ITEM_REUSE_SAVE,
// Java: RESTORE_ITEM_REUSE_SAVE, DELETE_ITEM_REUSE_SAVE; storeItemReuseDelay, restoreItemReuse)
//
// Same layout as skills.rs: verbatim SQL, one method per statement.

use sqlx::FromRow;

use crate::error::Result;
use crate::pool::Pool;

// Java: Player.ADD_ITEM_REUSE_SAVE
const ADD_ITEM_REUSE_SAVE: &str =
    "INSERT INTO character_item_reuse_save (charId,itemId,itemObjId,reuseDelay,systime) VALUES (?,?,?,?,?)";
// Java: Player.RESTORE_ITEM_REUSE_SAVE
const RESTORE_ITEM_REUSE_SAVE: &str =
    "SELECT charId,itemId,itemObjId,reuseDelay,systime FROM character_item_reuse_save WHERE charId=?";
// Java: Player.DELETE_ITEM_REUSE_SAVE
const DELETE_ITEM_REUSE_SAVE: &str = "DELETE FROM character_item_reuse_save WHERE charId=?";

/// A row of `character_item_reuse_save`: one item reuse timer kept over a logout.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct ItemReuseRow {
    /// `itemId`.
    #[sqlx(rename = "itemId")]
    pub item_id: i32,
    /// `itemObjId`.
    #[sqlx(rename = "itemObjId")]
    pub item_obj_id: i32,
    /// `reuseDelay`: milliseconds of the full delay (Java `getLong`).
    #[sqlx(rename = "reuseDelay")]
    pub reuse_delay: i32,
    /// `systime`: millisecond time the timer ends.
    pub systime: u64,
}

/// The values of one row, as `Player.storeItemReuseDelay` binds them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewItemReuse {
    /// `itemId`.
    pub item_id: i32,
    /// `itemObjId`.
    pub item_obj_id: i32,
    /// `reuseDelay`.
    pub reuse_delay: i64,
    /// `systime`.
    pub systime: i64,
}

/// Every query on `character_item_reuse_save`.
#[derive(Debug, Clone)]
pub struct ItemReuseRepository {
    pool: Pool,
}

impl ItemReuseRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// The saved item reuse timers of a character.
    // Java: Player.restoreItemReuse (the SELECT)
    pub async fn item_reuse_of(&self, char_id: i32) -> Result<Vec<ItemReuseRow>> {
        let rows = sqlx::query_as::<_, ItemReuseRow>(RESTORE_ITEM_REUSE_SAVE)
            .bind(char_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// Removes the saved timers of a character.
    // Java: Player.storeItemReuseDelay (before the new rows), Player.restoreItemReuse (after reading)
    pub async fn delete_item_reuse(&self, char_id: i32) -> Result<()> {
        sqlx::query(DELETE_ITEM_REUSE_SAVE)
            .bind(char_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Saves timers, on one connection like Java's batch.
    // Java: Player.storeItemReuseDelay
    pub async fn add_item_reuse(&self, char_id: i32, rows: &[NewItemReuse]) -> Result<()> {
        if rows.is_empty() {
            return Ok(());
        }
        let mut conn = self.pool.acquire().await?;
        for row in rows {
            sqlx::query(ADD_ITEM_REUSE_SAVE)
                .bind(char_id)
                .bind(row.item_id)
                .bind(row.item_obj_id)
                .bind(row.reuse_delay)
                .bind(row.systime)
                .execute(&mut *conn)
                .await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statements_have_one_placeholder_per_bind() {
        assert_eq!(ADD_ITEM_REUSE_SAVE.matches('?').count(), 5);
        assert_eq!(RESTORE_ITEM_REUSE_SAVE.matches('?').count(), 1);
        assert_eq!(DELETE_ITEM_REUSE_SAVE.matches('?').count(), 1);
    }
}
