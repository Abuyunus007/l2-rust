//! The `rebirth_system` table: the Rebirth custom feature (`Custom/Rebirth.ini`, M9-C05).

use sqlx::FromRow;

use crate::error::Result;
use crate::pool::Pool;

const SELECT_REBIRTH: &str = "SELECT rebirthCount, selectedSkills FROM rebirth_system WHERE charId=?";
const INSERT_FIRST_REBIRTH: &str = "INSERT INTO rebirth_system (charId, rebirthCount) VALUES (?,1)";
const UPDATE_REBIRTH_COUNT: &str = "UPDATE rebirth_system SET rebirthCount=? WHERE charId=?";
const UPDATE_SELECTED_SKILLS: &str = "UPDATE rebirth_system SET selectedSkills=? WHERE charId=?";

/// The row of one character.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct RebirthRow {
    /// `rebirthCount`.
    #[sqlx(rename = "rebirthCount")]
    pub rebirth_count: i32,
    /// `selectedSkills`: skill ids joined by commas (`''` for none).
    #[sqlx(rename = "selectedSkills")]
    pub selected_skills: String,
}

/// The queries on `rebirth_system`.
#[derive(Debug, Clone)]
pub struct RebirthRepository {
    pool: Pool,
}

impl RebirthRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// The row of a character, `None` when it never was reborn.
    pub async fn rebirth_of(&self, char_id: i32) -> Result<Option<RebirthRow>> {
        Ok(sqlx::query_as::<_, RebirthRow>(SELECT_REBIRTH)
            .bind(char_id)
            .fetch_optional(&self.pool)
            .await?)
    }

    /// The first rebirth: a new row with count 1. `false` when no row was written.
    pub async fn insert_first(&self, char_id: i32) -> Result<bool> {
        let done = sqlx::query(INSERT_FIRST_REBIRTH)
            .bind(char_id)
            .execute(&self.pool)
            .await?;
        Ok(done.rows_affected() > 0)
    }

    /// A later rebirth: the new count. `false` when no row changed.
    pub async fn update_count(&self, char_id: i32, new_count: i32) -> Result<bool> {
        let done = sqlx::query(UPDATE_REBIRTH_COUNT)
            .bind(new_count)
            .bind(char_id)
            .execute(&self.pool)
            .await?;
        Ok(done.rows_affected() > 0)
    }

    /// The selected skills, ids joined by commas.
    pub async fn save_selected_skills(&self, char_id: i32, skills: &str) -> Result<()> {
        sqlx::query(UPDATE_SELECTED_SKILLS)
            .bind(skills)
            .bind(char_id)
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
    fn statements_have_one_placeholder_per_bind() {
        assert_eq!(placeholders(SELECT_REBIRTH), 1);
        assert_eq!(placeholders(INSERT_FIRST_REBIRTH), 1);
        assert_eq!(placeholders(UPDATE_REBIRTH_COUNT), 2);
        assert_eq!(placeholders(UPDATE_SELECTED_SKILLS), 2);
    }
}
