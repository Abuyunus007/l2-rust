//! The `custom_mail` table: mails a server owner queues by hand (or from a web tool) for a
//! character, read and deleted by `CustomMailManager`.
// Java: org/l2jmobius/gameserver/managers/CustomMailManager.java (READ_SQL, DELETE_SQL)
//
// The rows are per receiver: the server that has the receiver online sends the mail and deletes
// the row, so two servers on one database do not send it twice.

use sqlx::FromRow;

use crate::error::Result;
use crate::pool::Pool;

// Java: CustomMailManager.READ_SQL. `date` is read as text because it is the key of the delete
// (Java binds `rs.getString("date")` back), whatever the column's time zone handling does.
const READ_ALL: &str =
    "SELECT CAST(`date` AS CHAR) AS date, receiver, subject, message, items FROM custom_mail";
// Java: CustomMailManager.DELETE_SQL
const DELETE_ENTRY: &str = "DELETE FROM custom_mail WHERE date=? AND receiver=?";

/// One row of `custom_mail`.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct CustomMailRow {
    /// `date`, as the database prints it (`YYYY-MM-DD HH:MM:SS`).
    pub date: String,
    /// `receiver`: the character's object id.
    pub receiver: u32,
    pub subject: String,
    pub message: String,
    /// `itemId1 count1 enchant1;itemId2 count2;itemId3 ...`.
    pub items: String,
}

/// The queries on `custom_mail`.
#[derive(Debug, Clone)]
pub struct CustomMailRepository {
    pool: Pool,
}

impl CustomMailRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// `SELECT * FROM custom_mail`.
    pub async fn load_all(&self) -> Result<Vec<CustomMailRow>> {
        Ok(sqlx::query_as(READ_ALL).fetch_all(&self.pool).await?)
    }

    /// `DELETE FROM custom_mail WHERE date=? AND receiver=?` (every row with that pair).
    pub async fn delete_entry(&self, date: &str, receiver: u32) -> Result<()> {
        sqlx::query(DELETE_ENTRY)
            .bind(date)
            .bind(receiver)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
