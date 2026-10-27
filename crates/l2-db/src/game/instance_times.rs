//! The `character_instance_time` table: when a character may enter an instance zone again.
// Java: org/l2jmobius/gameserver/managers/InstanceManager.java (ADD_INSTANCE_TIME in
// Java: setInstanceTime, RESTORE_INSTANCE_TIMES in restoreInstanceTimes, DELETE_INSTANCE_TIME in
// Java: deleteInstanceTime)
//
// Same layout as hennas.rs: verbatim SQL, one method per statement. The rows are per character,
// so they are written as usual side by side with the Java game server (D-53).

use sqlx::FromRow;

use crate::error::Result;
use crate::pool::Pool;

// Java: InstanceManager.RESTORE_INSTANCE_TIMES
const SELECT_INSTANCE_TIMES: &str = "SELECT instanceId,time FROM character_instance_time WHERE charId=?";
// Java: InstanceManager.ADD_INSTANCE_TIME
const ADD_INSTANCE_TIME: &str = "INSERT INTO character_instance_time (charId,instanceId,time) values (?,?,?) ON DUPLICATE KEY UPDATE time=?";
// Java: InstanceManager.DELETE_INSTANCE_TIME
const DELETE_INSTANCE_TIME: &str = "DELETE FROM character_instance_time WHERE charId=? AND instanceId=?";

/// One row of `character_instance_time`: the template id and the millisecond time the
/// character may enter it again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromRow)]
pub struct InstanceTimeRow {
    /// `instanceId`: the instance template id.
    #[sqlx(rename = "instanceId")]
    pub instance_id: i32,
    /// `time` (`bigint unsigned`, Java `getLong`).
    pub time: u64,
}

/// The queries on `character_instance_time`.
#[derive(Debug, Clone)]
pub struct InstanceTimeRepository {
    pool: Pool,
}

impl InstanceTimeRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Every row of one character, as Java reads them (no ordering).
    // Java: InstanceManager.restoreInstanceTimes
    pub async fn instance_times_of(&self, char_id: i32) -> Result<Vec<InstanceTimeRow>> {
        let rows = sqlx::query_as::<_, InstanceTimeRow>(SELECT_INSTANCE_TIMES)
            .bind(char_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// Inserts the time, or replaces the one the character has for the template.
    // Java: InstanceManager.setInstanceTime
    pub async fn set_instance_time(&self, char_id: i32, instance_id: i32, time: i64) -> Result<()> {
        sqlx::query(ADD_INSTANCE_TIME)
            .bind(char_id)
            .bind(instance_id)
            .bind(time)
            .bind(time)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Removes the row of one template.
    // Java: InstanceManager.deleteInstanceTime
    pub async fn delete_instance_time(&self, char_id: i32, instance_id: i32) -> Result<()> {
        sqlx::query(DELETE_INSTANCE_TIME)
            .bind(char_id)
            .bind(instance_id)
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
        assert_eq!(placeholders(SELECT_INSTANCE_TIMES), 1);
        assert_eq!(placeholders(ADD_INSTANCE_TIME), 4);
        assert_eq!(placeholders(DELETE_INSTANCE_TIME), 2);
    }
}
