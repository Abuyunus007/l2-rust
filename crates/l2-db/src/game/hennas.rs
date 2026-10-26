//! The `character_hennas` table: the dyes (hennas) a character wears, per class index.
// Java: org/l2jmobius/gameserver/entity/actor/Player.java (RESTORE_CHAR_HENNAS in restoreHenna,
// Java: ADD_CHAR_HENNA in addHenna, DELETE_CHAR_HENNA in removeHenna)
//
// Same layout as skills.rs: verbatim SQL, one method per statement.

use sqlx::FromRow;

use crate::error::Result;
use crate::pool::Pool;

// Java: Player.RESTORE_CHAR_HENNAS
const SELECT_HENNAS: &str = "SELECT slot,symbol_id FROM character_hennas WHERE charId=? AND class_index=?";

// Java: Player.ADD_CHAR_HENNA
const INSERT_HENNA: &str =
    "INSERT INTO character_hennas (charId,symbol_id,slot,class_index) VALUES (?,?,?,?)";
// Java: Player.DELETE_CHAR_HENNA
const DELETE_HENNA: &str = "DELETE FROM character_hennas WHERE charId=? AND slot=? AND class_index=?";

/// One row of `character_hennas`: a dye slot and the dye (symbol) id in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromRow)]
pub struct HennaRow {
    /// `slot`: Java keeps 1 to 3 only; the caller skips the others.
    pub slot: i32,
    /// `symbol_id`: the dye id. `None` for SQL NULL, which Java's `getInt` reads as 0.
    pub symbol_id: Option<i32>,
}

/// The queries on `character_hennas` that the game server reads.
#[derive(Debug, Clone)]
pub struct HennaRepository {
    pool: Pool,
}

impl HennaRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// The dye rows of one class index, as Java reads them (no ordering).
    // Java: Player.restoreHenna
    pub async fn hennas_of(&self, char_id: i32, class_index: i32) -> Result<Vec<HennaRow>> {
        let rows = sqlx::query_as::<_, HennaRow>(SELECT_HENNAS)
            .bind(char_id)
            .bind(class_index)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// One dye drawn into a slot (1 to 3) of a class index.
    // Java: Player.addHenna
    pub async fn add_henna(&self, char_id: i32, symbol_id: i32, slot: i32, class_index: i32) -> Result<()> {
        sqlx::query(INSERT_HENNA)
            .bind(char_id)
            .bind(symbol_id)
            .bind(slot)
            .bind(class_index)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// The dye of one slot removed.
    // Java: Player.removeHenna
    pub async fn delete_henna(&self, char_id: i32, slot: i32, class_index: i32) -> Result<()> {
        sqlx::query(DELETE_HENNA)
            .bind(char_id)
            .bind(slot)
            .bind(class_index)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placeholders(sql: &str) -> usize {
        sql.matches('?').count()
    }

    #[test]
    fn statement_has_one_placeholder_per_bind() {
        assert_eq!(placeholders(SELECT_HENNAS), 2);
        assert_eq!(placeholders(INSERT_HENNA), 4);
        assert_eq!(placeholders(DELETE_HENNA), 3);
    }
}
