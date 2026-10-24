//! The `bot_reported_char_data` table: the reports of the bot report button.
// Java: org/l2jmobius/gameserver/data/BotReportTable.java (SQL_LOAD_REPORTED_CHAR_DATA,
// Java: SQL_INSERT_REPORTED_CHAR_DATA, SQL_CLEAR_REPORTED_CHAR_DATA)
//
// Same layout as characters.rs: verbatim SQL, one method per statement. The Rust game server never
// runs Java's `DELETE FROM bot_reported_char_data` (D-39, D-49: the Java game server shares the
// table); a new report is written when it is made.

use sqlx::FromRow;

use crate::error::Result;
use crate::pool::Pool;

// Java: BotReportTable.SQL_LOAD_REPORTED_CHAR_DATA
const SELECT_ALL: &str = "SELECT * FROM bot_reported_char_data";
// Java: BotReportTable.SQL_INSERT_REPORTED_CHAR_DATA. The `ON DUPLICATE KEY UPDATE` is ours: the
// Java game server may have written the same pair since this server read the table.
const INSERT_REPORT: &str = "INSERT INTO bot_reported_char_data VALUES (?,?,?) ON DUPLICATE KEY UPDATE reportDate = VALUES(reportDate)";

/// One row of the table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromRow)]
pub struct BotReportDbRow {
    /// `botId`.
    #[sqlx(rename = "botId")]
    pub bot_id: u32,
    /// `reporterId`.
    #[sqlx(rename = "reporterId")]
    pub reporter_id: u32,
    /// `reportDate`: Unix milliseconds.
    #[sqlx(rename = "reportDate")]
    pub report_date: u64,
}

/// Every query on `bot_reported_char_data` that the bot report button needs.
#[derive(Debug, Clone)]
pub struct BotReportRepository {
    pool: Pool,
}

impl BotReportRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Every report, in the order the rows come back.
    // Java: BotReportTable.loadReportedCharData
    pub async fn load_all(&self) -> Result<Vec<BotReportDbRow>> {
        Ok(sqlx::query_as::<_, BotReportDbRow>(SELECT_ALL)
            .fetch_all(&self.pool)
            .await?)
    }

    /// Writes one report (a row of Java's `saveReportedCharData` loop).
    // Java: BotReportTable.saveReportedCharData
    pub async fn insert_report(&self, bot_id: i32, reporter_id: i32, report_date: i64) -> Result<()> {
        sqlx::query(INSERT_REPORT)
            .bind(bot_id)
            .bind(reporter_id)
            .bind(report_date)
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
        assert_eq!(INSERT_REPORT.matches('?').count(), 3);
        assert_eq!(SELECT_ALL.matches('?').count(), 0);
    }
}
