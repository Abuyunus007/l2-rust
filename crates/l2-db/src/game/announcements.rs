//! The `announcements` table: the server announcements GMs add with `//announces`.

use sqlx::FromRow;

use crate::error::Result;
use crate::pool::Pool;

const SELECT_ALL: &str = "SELECT * FROM announcements";
const INSERT_NORMAL: &str = "INSERT INTO announcements (type, content, author) VALUES (?, ?, ?)";
const INSERT_AUTO: &str = "INSERT INTO announcements (`type`, `content`, `author`, `initial`, `delay`, `repeat`) VALUES (?, ?, ?, ?, ?, ?)";
const UPDATE_NORMAL: &str = "UPDATE announcements SET type = ?, content = ?, author = ? WHERE id = ?";
const UPDATE_AUTO: &str = "UPDATE announcements SET `type` = ?, `content` = ?, `author` = ?, `initial` = ?, `delay` = ?, `repeat` = ? WHERE id = ?";
const DELETE: &str = "DELETE FROM announcements WHERE id = ?";

/// A row of `announcements`.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct AnnouncementRow {
    pub id: u32,
    /// The `AnnouncementType` ordinal.
    #[sqlx(rename = "type")]
    pub kind: i32,
    pub initial: i64,
    pub delay: i64,
    pub repeat: i32,
    pub author: String,
    pub content: String,
}

/// The `initial`, `delay` and `repeat` of an automatic announcement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutoColumns {
    pub initial: i64,
    pub delay: i64,
    pub repeat: i32,
}

/// Every query on `announcements`.
#[derive(Debug, Clone)]
pub struct AnnouncementRepository {
    pool: Pool,
}

impl AnnouncementRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Every row.
    pub async fn load_all(&self) -> Result<Vec<AnnouncementRow>> {
        let rows = sqlx::query_as::<_, AnnouncementRow>(SELECT_ALL)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// Stores a new announcement and returns the id the database gave it.
    pub async fn insert(
        &self,
        kind: i32,
        content: &str,
        author: &str,
        auto: Option<AutoColumns>,
    ) -> Result<u64> {
        let result = match auto {
            None => {
                sqlx::query(INSERT_NORMAL)
                    .bind(kind)
                    .bind(content)
                    .bind(author)
                    .execute(&self.pool)
                    .await?
            }
            Some(auto) => {
                sqlx::query(INSERT_AUTO)
                    .bind(kind)
                    .bind(content)
                    .bind(author)
                    .bind(auto.initial)
                    .bind(auto.delay)
                    .bind(auto.repeat)
                    .execute(&self.pool)
                    .await?
            }
        };
        Ok(result.last_insert_id())
    }

    /// Updates an announcement.
    pub async fn update(
        &self,
        id: u32,
        kind: i32,
        content: &str,
        author: &str,
        auto: Option<AutoColumns>,
    ) -> Result<()> {
        match auto {
            None => {
                sqlx::query(UPDATE_NORMAL)
                    .bind(kind)
                    .bind(content)
                    .bind(author)
                    .bind(id)
                    .execute(&self.pool)
                    .await?;
            }
            Some(auto) => {
                sqlx::query(UPDATE_AUTO)
                    .bind(kind)
                    .bind(content)
                    .bind(author)
                    .bind(auto.initial)
                    .bind(auto.delay)
                    .bind(auto.repeat)
                    .bind(id)
                    .execute(&self.pool)
                    .await?;
            }
        }
        Ok(())
    }

    /// Deletes an announcement.
    pub async fn delete(&self, id: u32) -> Result<()> {
        sqlx::query(DELETE).bind(id).execute(&self.pool).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statements_have_one_placeholder_per_bind() {
        let count = |s: &str| s.matches('?').count();
        assert_eq!(count(SELECT_ALL), 0);
        assert_eq!(count(INSERT_NORMAL), 3);
        assert_eq!(count(INSERT_AUTO), 6);
        assert_eq!(count(UPDATE_NORMAL), 4);
        assert_eq!(count(UPDATE_AUTO), 7);
        assert_eq!(count(DELETE), 1);
    }
}
