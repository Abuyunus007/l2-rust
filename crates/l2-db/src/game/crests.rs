//! The `crests` table: clan, large clan and alliance crest images.
// Java: org/l2jmobius/gameserver/data/sql/CrestTable.java (load, createCrest, removeCrest)
//
// Same layout as clans.rs: verbatim SQL, one method per statement. D-39: the Java game server
// reads and writes the same rows.

use sqlx::FromRow;

use crate::error::Result;
use crate::pool::Pool;

// Java: CrestTable.load
const SELECT_CRESTS: &str = "SELECT `crest_id`, `data`, `type` FROM `crests` ORDER BY `crest_id` DESC";
// Kain only (M7-FU-03): the used ids, to take the lowest free one.
const SELECT_CREST_IDS: &str = "SELECT `crest_id` FROM `crests`";
// Java: CrestTable.createCrest
const INSERT_CREST: &str = "INSERT INTO `crests`(`crest_id`, `data`, `type`) VALUES(?, ?, ?)";
// Java: CrestTable.removeCrest
const DELETE_CREST: &str = "DELETE FROM `crests` WHERE `crest_id`=?";

/// One `crests` row.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct CrestRow {
    pub crest_id: i32,
    pub data: Vec<u8>,
    /// `TINYINT`, read with `getInt` by Java.
    #[sqlx(rename = "type")]
    pub crest_type: i8,
}

/// Every query on `crests`.
#[derive(Debug, Clone)]
pub struct CrestRepository {
    pool: Pool,
}

impl CrestRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Every crest, the highest id first.
    // Java: CrestTable.load
    pub async fn all(&self) -> Result<Vec<CrestRow>> {
        let rows = sqlx::query_as::<_, CrestRow>(SELECT_CRESTS)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// The ids of every crest (M7-FU-03: a new crest takes the lowest free one).
    pub async fn ids(&self) -> Result<Vec<i32>> {
        let rows: Vec<(i32,)> = sqlx::query_as(SELECT_CREST_IDS).fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    // Java: CrestTable.createCrest
    pub async fn insert(&self, crest_id: i32, data: &[u8], crest_type: i32) -> Result<()> {
        sqlx::query(INSERT_CREST)
            .bind(crest_id)
            .bind(data)
            .bind(crest_type)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // Java: CrestTable.removeCrest
    pub async fn delete(&self, crest_id: i32) -> Result<()> {
        sqlx::query(DELETE_CREST)
            .bind(crest_id)
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
        let count = |s: &str| s.matches('?').count();
        assert_eq!(count(SELECT_CRESTS), 0);
        assert_eq!(count(SELECT_CREST_IDS), 0);
        assert_eq!(count(INSERT_CREST), 3);
        assert_eq!(count(DELETE_CREST), 1);
    }
}
