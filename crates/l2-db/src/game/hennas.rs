//! The `character_hennas` table: the dyes (hennas) a character wears, per class index.
// Java: org/l2jmobius/gameserver/entity/actor/Player.java (RESTORE_CHAR_HENNAS, used by
// Java: restoreHenna; ADD_CHAR_HENNA and DELETE_CHAR_HENNA come with the symbol maker)
//
// Same layout as skills.rs: verbatim SQL, one method per statement.

use sqlx::FromRow;

use crate::error::Result;
use crate::pool::Pool;

// Java: Player.RESTORE_CHAR_HENNAS
const SELECT_HENNAS: &str = "SELECT slot,symbol_id FROM character_hennas WHERE charId=? AND class_index=?";

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
    }
}
