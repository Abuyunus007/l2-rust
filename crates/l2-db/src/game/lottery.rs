//! The `lottery` table: the draws of the Lucky Lottery (`AllowLottery`).
// Java: org/l2jmobius/gameserver/managers/games/LotteryManager.java (the statements)
//
// Same layout as bbs_favorites.rs: Java's SQL, one method per statement. The table is called
// `lottery` (the card says `games`; that is what Java's older versions named it, the SQL file of
// this datapack creates `lottery`). `id` is always 1 in Java.

use sqlx::FromRow;

use crate::error::Result;
use crate::pool::Pool;

// Java: LotteryManager.INSERT_LOTTERY
const INSERT_LOTTERY: &str = "INSERT INTO lottery(id, idnr, enddate, prize, newprize) VALUES (?, ?, ?, ?, ?)";
// Java: LotteryManager.UPDATE_PRICE
const UPDATE_PRICE: &str = "UPDATE lottery SET prize=?, newprize=? WHERE id = 1 AND idnr = ?";
// Java: LotteryManager.UPDATE_LOTTERY
const UPDATE_LOTTERY: &str = "UPDATE lottery SET finished=1, prize=?, newprize=?, number1=?, number2=?, prize1=?, prize2=?, prize3=? WHERE id=1 AND idnr=?";
// Java: LotteryManager.SELECT_LAST_LOTTERY
const SELECT_LAST_LOTTERY: &str =
    "SELECT idnr, prize, newprize, enddate, finished FROM lottery WHERE id = 1 ORDER BY idnr DESC LIMIT 1";
// Java: LotteryManager.SELECT_LOTTERY_ITEM
const SELECT_LOTTERY_ITEM: &str =
    "SELECT enchant_level, custom_type2 FROM items WHERE item_id = 4442 AND custom_type1 = ?";
// Java: LotteryManager.SELECT_LOTTERY_TICKET, for every draw at once (the world keeps the rows
// and looks a ticket up in them).
const SELECT_LOTTERY_RESULTS: &str =
    "SELECT idnr, number1, number2, prize1, prize2, prize3 FROM lottery WHERE id = 1";

/// The last row (`SELECT_LAST_LOTTERY`).
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct LotteryDbRow {
    pub idnr: i32,
    pub prize: i32,
    pub newprize: i32,
    /// `enddate`, a `bigint unsigned`: Unix milliseconds.
    pub enddate: u64,
    pub finished: i32,
}

/// The numbers and prizes of one draw (`SELECT_LOTTERY_TICKET`).
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct LotteryResultRow {
    pub idnr: i32,
    pub number1: i32,
    pub number2: i32,
    pub prize1: i32,
    pub prize2: i32,
    pub prize3: i32,
}

/// The queries on `lottery`.
#[derive(Debug, Clone)]
pub struct LotteryRepository {
    pool: Pool,
}

impl LotteryRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// The newest draw (`startLottery`).
    pub async fn last(&self) -> Result<Option<LotteryDbRow>> {
        Ok(sqlx::query_as::<_, LotteryDbRow>(SELECT_LAST_LOTTERY)
            .fetch_optional(&self.pool)
            .await?)
    }

    /// Every draw's numbers and prizes (`checkTicket`).
    pub async fn results(&self) -> Result<Vec<LotteryResultRow>> {
        Ok(sqlx::query_as::<_, LotteryResultRow>(SELECT_LOTTERY_RESULTS)
            .fetch_all(&self.pool)
            .await?)
    }

    /// `(enchant_level, custom_type2)` of every ticket (item 4442) of a draw (`finishLottery`).
    pub async fn tickets(&self, number: i32) -> Result<Vec<(i32, i32)>> {
        let rows: Vec<(Option<i32>, Option<i32>)> = sqlx::query_as(SELECT_LOTTERY_ITEM)
            .bind(number)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .into_iter()
            .map(|(enchant, type2)| (enchant.unwrap_or(0), type2.unwrap_or(0)))
            .collect())
    }

    /// A new draw (`startLottery`).
    pub async fn insert(&self, idnr: i32, enddate: i64, prize: i64, newprize: i64) -> Result<()> {
        sqlx::query(INSERT_LOTTERY)
            .bind(1_i32)
            .bind(idnr)
            .bind(enddate)
            .bind(prize)
            .bind(newprize)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// The prize grew (`increasePrize`).
    pub async fn update_price(&self, idnr: i32, prize: i64) -> Result<()> {
        sqlx::query(UPDATE_PRICE)
            .bind(prize)
            .bind(prize)
            .bind(idnr)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// A draw finished (`finishLottery`).
    #[allow(clippy::too_many_arguments)]
    pub async fn finish(
        &self,
        idnr: i32,
        prize: i64,
        newprize: i64,
        number1: i32,
        number2: i32,
        prize1: i64,
        prize2: i64,
        prize3: i64,
    ) -> Result<()> {
        sqlx::query(UPDATE_LOTTERY)
            .bind(prize)
            .bind(newprize)
            .bind(number1)
            .bind(number2)
            .bind(prize1)
            .bind(prize2)
            .bind(prize3)
            .bind(idnr)
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
        assert_eq!(placeholders(INSERT_LOTTERY), 5);
        assert_eq!(placeholders(UPDATE_PRICE), 3);
        assert_eq!(placeholders(UPDATE_LOTTERY), 8);
        assert_eq!(placeholders(SELECT_LOTTERY_ITEM), 1);
        assert_eq!(placeholders(SELECT_LAST_LOTTERY), 0);
        assert_eq!(placeholders(SELECT_LOTTERY_RESULTS), 0);
    }
}
