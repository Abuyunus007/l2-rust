//! The `petition_feedback` table: what players think of a GM petition consultation.
// Java: network/clientpackets/RequestPetitionFeedback.java (INSERT_FEEDBACK)
//
// Written only; nothing in the server reads it back.

use crate::error::Result;
use crate::pool::Pool;

// Java: RequestPetitionFeedback.INSERT_FEEDBACK
const INSERT_FEEDBACK: &str = "INSERT INTO petition_feedback VALUES (?,?,?,?,?)";

/// Every query on `petition_feedback`.
#[derive(Debug, Clone)]
pub struct PetitionFeedbackRepository {
    pool: Pool,
}

impl PetitionFeedbackRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// One feedback row: the character, the GM, the rate (0 to 4), the message and the time.
    pub async fn insert(&self, character: &str, gm: &str, rate: i32, message: &str, time: i64) -> Result<()> {
        sqlx::query(INSERT_FEEDBACK)
            .bind(character)
            .bind(gm)
            .bind(rate)
            .bind(message)
            .bind(time)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_statement_has_one_placeholder_per_column() {
        assert_eq!(INSERT_FEEDBACK.matches('?').count(), 5);
    }
}
