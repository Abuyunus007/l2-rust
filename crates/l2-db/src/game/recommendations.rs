//! The `character_reco_bonus` table: recommendations a character has and may still give.
// Java: org/l2jmobius/gameserver/entity/actor/Player.java (storeRecommendations)
//
// Same layout as characters.rs: verbatim SQL, one method per statement.

use crate::error::Result;
use crate::pool::Pool;

// Java: Player.storeRecommendations
const REPLACE_RECO_BONUS: &str =
    "REPLACE INTO character_reco_bonus (charId,rec_have,rec_left,time_left) VALUES (?,?,?,?)";

/// Every query on `character_reco_bonus` that M2 needs.
#[derive(Debug, Clone)]
pub struct RecommendationRepository {
    pool: Pool,
}

impl RecommendationRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Stores the recommendation counters (`REPLACE`, so it also covers an existing row).
    /// `time_left` is the milliseconds left of the Nevit hourglass task, 0 for none.
    // Java: Player.storeRecommendations
    pub async fn replace_reco_bonus(
        &self,
        char_id: i32,
        rec_have: i32,
        rec_left: i32,
        time_left: i64,
    ) -> Result<()> {
        sqlx::query(REPLACE_RECO_BONUS)
            .bind(char_id)
            .bind(rec_have)
            .bind(rec_left)
            .bind(time_left)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
