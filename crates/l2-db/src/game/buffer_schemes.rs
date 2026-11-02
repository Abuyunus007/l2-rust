//! The `buffer_schemes` table: the saved buff schemes of the scheme buffer NPC.
// Java: gameserver/data/SchemeBufferTable.java (LOAD_SCHEMES, INSERT_SCHEME; DELETE_SCHEMES is the
// Java: whole-table `TRUNCATE`, which this server never runs, see `replace_schemes_of`)
//
// Same layout as bbs_favorites.rs: the Java SQL, one method per statement.

use sqlx::FromRow;

use crate::error::Result;
use crate::pool::Pool;

// Java: SchemeBufferTable.LOAD_SCHEMES
const LOAD_SCHEMES: &str = "SELECT `object_id`, `scheme_name`, `skills` FROM `buffer_schemes`";
// Java: SchemeBufferTable.INSERT_SCHEME
const INSERT_SCHEME: &str = "INSERT INTO buffer_schemes (object_id, scheme_name, skills) VALUES (?,?,?)";
// The per-player form of Java's `DELETE_SCHEMES` (`TRUNCATE TABLE buffer_schemes`).
const DELETE_SCHEMES_OF: &str = "DELETE FROM buffer_schemes WHERE object_id=?";

/// One row of `buffer_schemes`.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct BufferSchemeRow {
    /// `object_id`: the character (an unsigned `INT`).
    pub object_id: u32,
    pub scheme_name: String,
    /// The skill ids, comma separated.
    pub skills: String,
}

/// The queries on `buffer_schemes`.
#[derive(Debug, Clone)]
pub struct BufferSchemeRepository {
    pool: Pool,
}

impl BufferSchemeRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Every scheme of every character (`loadPlayerSchemes`, at start-up).
    pub async fn load_all(&self) -> Result<Vec<BufferSchemeRow>> {
        let rows = sqlx::query_as::<_, BufferSchemeRow>(LOAD_SCHEMES)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// Replaces the schemes of one character: its rows go, the given `(name, skills)` come.
    // NOTE: Java saves the whole table (`TRUNCATE` then a batch of inserts) at shutdown. That would
    // erase the rows the other game server wrote while both share the database (D-39), so this
    // server writes the rows of the one character that changed a scheme.
    pub async fn replace_schemes_of(&self, object_id: i32, schemes: &[(String, String)]) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(DELETE_SCHEMES_OF)
            .bind(object_id)
            .execute(&mut *tx)
            .await?;
        for (name, skills) in schemes {
            sqlx::query(INSERT_SCHEME)
                .bind(object_id)
                .bind(name)
                .bind(skills)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
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
    fn statements_have_one_placeholder_per_bind() {
        assert_eq!(placeholders(LOAD_SCHEMES), 0);
        assert_eq!(placeholders(INSERT_SCHEME), 3);
        assert_eq!(placeholders(DELETE_SCHEMES_OF), 1);
    }
}
