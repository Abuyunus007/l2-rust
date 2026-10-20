//! The `punishments` table: bans, chat bans, party bans and jail terms of the punishment
//! manager.
// Java: managers/PunishmentManager.java (load), mechanics/punishment/PunishmentTask.java
// Java: (INSERT_QUERY, UPDATE_QUERY)
//
// Same layout as variables.rs: verbatim SQL, one method per statement. D-39: the Java game server
// reads and writes the same rows, so the Rust one writes exactly what `PunishmentTask` writes: an
// `INSERT` when a punishment starts and an `UPDATE` of `expiration` to the time it ended.

use sqlx::FromRow;

use crate::error::Result;
use crate::pool::Pool;

// Java: PunishmentManager.load
const SELECT_ALL: &str = "SELECT * FROM punishments";
// Java: PunishmentTask.INSERT_QUERY
const INSERT_PUNISHMENT: &str = "INSERT INTO punishments (`key`, `affect`, `type`, `expiration`, `reason`, `punishedBy`) VALUES (?, ?, ?, ?, ?, ?)";
// Java: PunishmentTask.UPDATE_QUERY
const UPDATE_EXPIRATION: &str = "UPDATE punishments SET expiration = ? WHERE id = ?";

/// A row of `punishments`.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct PunishmentRow {
    pub id: u32,
    /// The character id, account name, IP or hardware id the punishment affects.
    pub key: String,
    /// `PunishmentAffect.name()`: `ACCOUNT`, `CHARACTER`, `IP` or `HWID`.
    pub affect: String,
    /// `PunishmentType.name()`: `BAN`, `CHAT_BAN`, `PARTY_BAN` or `JAIL`.
    #[sqlx(rename = "type")]
    pub kind: String,
    /// Epoch milliseconds the punishment ends at; 0 or less is forever.
    pub expiration: i64,
    pub reason: String,
    #[sqlx(rename = "punishedBy")]
    pub punished_by: String,
}

/// The values `PunishmentTask.onStart` binds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewPunishment {
    pub key: String,
    pub affect: String,
    pub kind: String,
    pub expiration: i64,
    pub reason: String,
    pub punished_by: String,
}

/// Every query on `punishments`.
#[derive(Debug, Clone)]
pub struct PunishmentRepository {
    pool: Pool,
}

impl PunishmentRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Every stored punishment, expired ones included (the manager counts them).
    // Java: PunishmentManager.load
    pub async fn load_all(&self) -> Result<Vec<PunishmentRow>> {
        let rows = sqlx::query_as::<_, PunishmentRow>(SELECT_ALL)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// Stores a new punishment and returns the id the database gave it.
    // Java: PunishmentTask.onStart (INSERT_QUERY with RETURN_GENERATED_KEYS)
    pub async fn insert(&self, punishment: &NewPunishment) -> Result<u64> {
        let result = sqlx::query(INSERT_PUNISHMENT)
            .bind(&punishment.key)
            .bind(&punishment.affect)
            .bind(&punishment.kind)
            .bind(punishment.expiration)
            .bind(&punishment.reason)
            .bind(&punishment.punished_by)
            .execute(&self.pool)
            .await?;
        Ok(result.last_insert_id())
    }

    /// Marks a punishment as ended: its expiration becomes `ended_at`.
    // Java: PunishmentTask.onEnd (UPDATE_QUERY)
    pub async fn update_expiration(&self, id: u32, ended_at: i64) -> Result<()> {
        sqlx::query(UPDATE_EXPIRATION)
            .bind(ended_at)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statements_have_one_placeholder_per_bind() {
        assert_eq!(INSERT_PUNISHMENT.matches('?').count(), 6);
        assert_eq!(UPDATE_EXPIRATION.matches('?').count(), 2);
        assert_eq!(SELECT_ALL.matches('?').count(), 0);
    }
}
