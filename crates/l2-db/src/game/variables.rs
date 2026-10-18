//! The `character_variables` table: per-character key/value settings (UI key mapping, restore
//! location, Nevit points...).
// Java: org/l2jmobius/gameserver/mechanics/variables/PlayerVariables.java (SELECT_QUERY)
//
// Same layout as characters.rs: verbatim SQL, one method per statement. Only the read exists:
// the Rust game server does not write variables yet (D-39: it leaves rows it does not own alone).

use crate::error::Result;
use crate::pool::Pool;

// Java: PlayerVariables.SELECT_QUERY (the table has exactly `charId`, `var`, `val`)
const SELECT_VARIABLES: &str = "SELECT * FROM character_variables WHERE charId = ?";

/// Every query on `character_variables` that M2 needs.
#[derive(Debug, Clone)]
pub struct VariableRepository {
    pool: Pool,
}

impl VariableRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// All variables of the character as `(var, val)`.
    // Java: PlayerVariables.restoreMe
    pub async fn variables_of(&self, char_id: i32) -> Result<Vec<(String, String)>> {
        let rows: Vec<(u32, String, String)> = sqlx::query_as(SELECT_VARIABLES)
            .bind(char_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(|(_, var, val)| (var, val)).collect())
    }
}
