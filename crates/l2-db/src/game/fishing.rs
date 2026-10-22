//! The fishing championship tables: `fishing_championship` (the ranking) and the one
//! `global_variables` row it keeps (`fishChampionshipEnd`).
// Java: org/l2jmobius/gameserver/managers/FishingChampionshipManager.java (SELECT, DELETE,
//       INSERT, restoreData, shutdown)
// Java: org/l2jmobius/gameserver/managers/GlobalVariablesManager.java (INSERT_QUERY)
//
// D-39: the Java game server reads and rewrites the same rows. The server reads them at startup,
// and writes them only when the owner allows it (`FishingChampionshipDatabaseWrites`, D-49).

use sqlx::FromRow;

use crate::error::Result;
use crate::pool::Pool;

// Java: FishingChampionshipManager.SELECT
const SELECT_ROWS: &str = "SELECT `player_name`, `fish_length`, `rewarded` FROM fishing_championship";
// Java: FishingChampionshipManager.DELETE
const DELETE_ROWS: &str = "DELETE FROM fishing_championship";
// Java: FishingChampionshipManager.INSERT
const INSERT_ROW: &str = "INSERT INTO fishing_championship(player_name,fish_length,rewarded) VALUES (?,?,?)";
// Java: GlobalVariablesManager.SELECT_QUERY, for the one variable the manager reads.
const SELECT_VARIABLE: &str = "SELECT `value` FROM global_variables WHERE `var` = ?";
// Java: GlobalVariablesManager.INSERT_QUERY
const REPLACE_VARIABLE: &str = "REPLACE INTO global_variables (var, value) VALUES (?, ?)";

/// The name of the global variable that holds the end of the championship period.
pub const FISH_CHAMPIONSHIP_END: &str = "fishChampionshipEnd";

/// One row of `fishing_championship`.
#[derive(Debug, Clone, PartialEq)]
pub struct FishingChampionshipRow {
    pub player_name: String,
    pub fish_length: f64,
    pub rewarded: i32,
}

#[derive(Debug, Clone, FromRow)]
struct FishingChampionshipDbRow {
    player_name: String,
    fish_length: f64,
    rewarded: i32,
}

/// Reads and writes the championship.
#[derive(Clone)]
pub struct FishingChampionshipRepository {
    pool: Pool,
}

impl FishingChampionshipRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Java `restoreData`: every row, in the database's order.
    pub async fn load_rows(&self) -> Result<Vec<FishingChampionshipRow>> {
        let rows: Vec<FishingChampionshipDbRow> = sqlx::query_as(SELECT_ROWS).fetch_all(&self.pool).await?;
        Ok(rows
            .into_iter()
            .map(|r| FishingChampionshipRow {
                player_name: r.player_name,
                fish_length: r.fish_length,
                rewarded: r.rewarded,
            })
            .collect())
    }

    /// Java `GlobalVariablesManager.getLong("fishChampionshipEnd", 0)`: the end of the period, 0
    /// when the variable is missing or is not a number.
    pub async fn end_date(&self) -> Result<i64> {
        let value: Option<(Option<String>,)> = sqlx::query_as(SELECT_VARIABLE)
            .bind(FISH_CHAMPIONSHIP_END)
            .fetch_optional(&self.pool)
            .await?;
        Ok(value
            .and_then(|(v,)| v)
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0))
    }

    /// Java `FishingChampionshipManager.shutdown()`: `fishChampionshipEnd`, then the old rows go
    /// and one row per fisher is inserted. Java runs the statements one by one without a
    /// transaction; one transaction leaves the same rows and never half of them.
    pub async fn save(&self, end_date: i64, rows: &[FishingChampionshipRow]) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(REPLACE_VARIABLE)
            .bind(FISH_CHAMPIONSHIP_END)
            .bind(end_date.to_string())
            .execute(&mut *tx)
            .await?;
        sqlx::query(DELETE_ROWS).execute(&mut *tx).await?;
        for row in rows {
            sqlx::query(INSERT_ROW)
                .bind(&row.player_name)
                .bind(row.fish_length)
                .bind(row.rewarded)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }
}
