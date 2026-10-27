//! The `underground_coliseum` table (the best team of each arena) and the two
//! `global_variables` rows of the battle period (M8-GAMES-01).
// Java: managers/games/UndergroundColiseumManager.java (restoreBestTeams, saveBestTeam,
//       generateNewDate)
// Java: managers/GlobalVariablesManager.java (SELECT_QUERY by variable, INSERT_QUERY)
//
// Same layout as fishing.rs: verbatim SQL, one method per statement. D-39, D-53: the Java game
// server writes the same rows; the world decides which writes go out (`CycleDatabaseWrites`).

use sqlx::FromRow;

use crate::error::Result;
use crate::pool::Pool;

// Java: UndergroundColiseumManager.restoreBestTeams
const SELECT_ALL: &str = "SELECT * FROM underground_coliseum ORDER BY arenaId";
// Java: UndergroundColiseumManager.saveBestTeam (new)
const INSERT_TEAM: &str = "INSERT INTO underground_coliseum (`arenaId`, `leader`, `wins`) VALUES (?,?,?) ";
// Java: UndergroundColiseumManager.saveBestTeam (existing). The first parameter is bound with
// `setInt(1, team.getArenaId())`, so the leader column gets the arena id.
const UPDATE_TEAM: &str = "UPDATE underground_coliseum SET leader = ?, wins = ?  WHERE arenaId = ?";
// Java: GlobalVariablesManager.SELECT_QUERY, for one variable.
const SELECT_VARIABLE: &str = "SELECT `value` FROM global_variables WHERE `var` = ?";
// Java: GlobalVariablesManager.INSERT_QUERY
const REPLACE_VARIABLE: &str = "REPLACE INTO global_variables (var, value) VALUES (?, ?)";

/// A row of `underground_coliseum`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColiseumBestTeamRow {
    pub arena_id: i32,
    pub leader: String,
    pub wins: i32,
}

#[derive(Debug, Clone, FromRow)]
struct ColiseumBestTeamDbRow {
    #[sqlx(rename = "arenaId")]
    arena_id: u32,
    leader: String,
    wins: i32,
}

/// The queries of the Underground Coliseum.
#[derive(Debug, Clone)]
pub struct UndergroundColiseumRepository {
    pool: Pool,
}

impl UndergroundColiseumRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Java `restoreBestTeams`: every row, by arena id.
    pub async fn best_teams(&self) -> Result<Vec<ColiseumBestTeamRow>> {
        let rows: Vec<ColiseumBestTeamDbRow> = sqlx::query_as(SELECT_ALL).fetch_all(&self.pool).await?;
        Ok(rows
            .into_iter()
            .map(|r| ColiseumBestTeamRow {
                arena_id: i32::try_from(r.arena_id).unwrap_or(i32::MAX),
                leader: r.leader,
                wins: r.wins,
            })
            .collect())
    }

    /// Java `saveBestTeam(team, true)`.
    pub async fn insert_best_team(&self, arena_id: i32, leader: &str, wins: i32) -> Result<()> {
        sqlx::query(INSERT_TEAM)
            .bind(arena_id)
            .bind(leader)
            .bind(wins)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Java `saveBestTeam(team, false)`: the leader column is set to the arena id like Java's
    /// `setInt(1, team.getArenaId())` does.
    pub async fn update_best_team(&self, arena_id: i32, wins: i32) -> Result<()> {
        sqlx::query(UPDATE_TEAM)
            .bind(arena_id)
            .bind(wins)
            .bind(arena_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Java `GlobalVariablesManager.getLong(name, 0)`: 0 when the variable is missing or is not a
    /// number.
    pub async fn variable_long(&self, name: &str) -> Result<i64> {
        let value: Option<(Option<String>,)> = sqlx::query_as(SELECT_VARIABLE)
            .bind(name)
            .fetch_optional(&self.pool)
            .await?;
        Ok(value
            .and_then(|(v,)| v)
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0))
    }

    /// Java `GlobalVariablesManager.set(name, long)` as the row it ends up as.
    pub async fn set_variable_long(&self, name: &str, value: i64) -> Result<()> {
        sqlx::query(REPLACE_VARIABLE)
            .bind(name)
            .bind(value.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
