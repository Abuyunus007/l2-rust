//! The `global_variables` table as Java's `GlobalVariablesManager` reads it (the whole table)
//! and as the script engine stores it (the changed variables, S-API-01).
// Java: org/l2jmobius/gameserver/managers/GlobalVariablesManager.java (SELECT_QUERY,
// Java: DELETE_QUERY, INSERT_QUERY, restoreMe, storeMe)
//
// D-54: the rows are global. The game server stores them only with `CycleDatabaseWrites = True`
// (`config/SideBySide.ini`); the repository itself does what it is told.

use sqlx::FromRow;

use crate::error::Result;
use crate::pool::Pool;

// The re-read of named variables before a script decides (D-50, S-API-04): Java's `SELECT_QUERY`
// restricted to one name.
const SELECT_ONE: &str = "SELECT * FROM global_variables WHERE var = ?";
// Java: GlobalVariablesManager.SELECT_QUERY
const SELECT_ALL: &str = "SELECT * FROM global_variables";
// The delete of one removed variable. Java's `storeMe` deletes the whole table
// (`DELETE_QUERY`) and writes its map back; this server's other systems write their own rows
// of the table, so the script engine only touches the names it changed (see
// `l2_world::script::globals`).
const DELETE_VARIABLE: &str = "DELETE FROM global_variables WHERE var = ?";
// Java: GlobalVariablesManager.INSERT_QUERY
const REPLACE_VARIABLE: &str = "REPLACE INTO global_variables (var, value) VALUES (?, ?)";

#[derive(Debug, Clone, FromRow)]
struct GlobalVariableDbRow {
    var: String,
    value: Option<String>,
}

/// The whole-table queries on `global_variables`.
#[derive(Debug, Clone)]
pub struct GlobalVariableRepository {
    pool: Pool,
}

impl GlobalVariableRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Every row as `(var, value)`, in the database's order. A NULL value is `None`; Java's
    /// `StatSet.set(name, null)` ignores such a row, so the caller skips it.
    // Java: GlobalVariablesManager.restoreMe
    pub async fn load_all(&self) -> Result<Vec<(String, Option<String>)>> {
        let rows: Vec<GlobalVariableDbRow> = sqlx::query_as(SELECT_ALL).fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(|r| (r.var, r.value)).collect())
    }

    /// The rows of the named variables as `(var, value)`; a name without a row is not in the
    /// answer. A NULL value is `None`. Used to re-read the rows the Java GS changes (D-50).
    pub async fn load_variables(&self, names: &[String]) -> Result<Vec<(String, Option<String>)>> {
        let mut out = Vec::new();
        for name in names {
            let rows: Vec<GlobalVariableDbRow> = sqlx::query_as(SELECT_ONE)
                .bind(name)
                .fetch_all(&self.pool)
                .await?;
            out.extend(rows.into_iter().map(|r| (r.var, r.value)));
        }
        Ok(out)
    }

    /// Writes one variable.
    // Java: GlobalVariablesManager.storeMe (INSERT_QUERY)
    pub async fn replace_variable(&self, var: &str, value: &str) -> Result<()> {
        sqlx::query(REPLACE_VARIABLE)
            .bind(var)
            .bind(value)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Deletes one variable (nothing happens when the row is not there).
    pub async fn delete_variable(&self, var: &str) -> Result<()> {
        sqlx::query(DELETE_VARIABLE).bind(var).execute(&self.pool).await?;
        Ok(())
    }
}
