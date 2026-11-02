//! The `account_premium` table: the end of the premium status of an account (or a character,
//! `AccountWidePremium = False`) of the premium system (`EnablePremiumSystem`).
// Java: org/l2jmobius/gameserver/managers/PremiumManager.java (LOAD_SQL, UPDATE_SQL, DELETE_SQL)
//
// Same layout as wedding.rs: the Java SQL, one method per statement. `enddate` is a
// `decimal(20,0)` of Unix milliseconds.

use sqlx::FromRow;

use super::sql_number::SqlNumber;
use crate::error::Result;
use crate::pool::Pool;

// Java: PremiumManager.LOAD_SQL
const SELECT_ENDDATE: &str = "SELECT account_name,enddate FROM account_premium WHERE account_name = ?";
// Java: PremiumManager.UPDATE_SQL
const REPLACE_ROW: &str = "REPLACE INTO account_premium (account_name,enddate) VALUE (?,?)";
// Java: PremiumManager.DELETE_SQL
const DELETE_ROW: &str = "DELETE FROM account_premium WHERE account_name = ?";

#[derive(Debug, FromRow)]
struct PremiumDbRow {
    enddate: SqlNumber<i64>,
}

/// The queries on `account_premium`.
#[derive(Debug, Clone)]
pub struct PremiumRepository {
    pool: Pool,
}

impl PremiumRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// The `enddate` of the row of `account_name` (lower case), `None` when there is none.
    pub async fn end_date_of(&self, account_name: &str) -> Result<Option<i64>> {
        let row = sqlx::query_as::<_, PremiumDbRow>(SELECT_ENDDATE)
            .bind(account_name)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|r| r.enddate.0))
    }

    /// `REPLACE INTO account_premium`: the row of the account with its new end.
    pub async fn replace(&self, account_name: &str, end_date: i64) -> Result<()> {
        sqlx::query(REPLACE_ROW)
            .bind(account_name)
            .bind(end_date)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// `DELETE FROM account_premium`: the premium of the account is gone.
    pub async fn delete(&self, account_name: &str) -> Result<()> {
        sqlx::query(DELETE_ROW)
            .bind(account_name)
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
        assert_eq!(placeholders(SELECT_ENDDATE), 1);
        assert_eq!(placeholders(REPLACE_ROW), 2);
        assert_eq!(placeholders(DELETE_ROW), 1);
    }
}
