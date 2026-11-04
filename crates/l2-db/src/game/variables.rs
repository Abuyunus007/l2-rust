//! The `character_variables` table: per-character key/value settings (UI key mapping, restore
//! location, Nevit points...).

use crate::error::Result;
use crate::pool::Pool;

const SELECT_VARIABLES: &str = "SELECT * FROM character_variables WHERE charId = ?";
const DELETE_VARIABLE: &str = "DELETE FROM character_variables WHERE charId = ? AND var = ?";
const UPSERT_VARIABLE: &str = "INSERT INTO character_variables (charId, var, val) VALUES (?, ?, ?) ON DUPLICATE KEY UPDATE val = VALUES(val)";

/// Every query on `character_variables` that M2 and M4 need.
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
    pub async fn variables_of(&self, char_id: i32) -> Result<Vec<(String, String)>> {
        let rows: Vec<(u32, String, String)> = sqlx::query_as(SELECT_VARIABLES)
            .bind(char_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(|(_, var, val)| (var, val)).collect())
    }

    /// Inserts a variable or updates its value.
    pub async fn upsert_variable(&self, char_id: i32, var: &str, val: &str) -> Result<()> {
        sqlx::query(UPSERT_VARIABLE)
            .bind(char_id)
            .bind(var)
            .bind(val)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Deletes a variable (nothing happens when the row is not there).
    pub async fn delete_variable(&self, char_id: i32, var: &str) -> Result<()> {
        sqlx::query(DELETE_VARIABLE)
            .bind(char_id)
            .bind(var)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
