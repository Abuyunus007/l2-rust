//! The `character_reco_bonus` table: recommendations a character has and may still give.
// Java: org/l2jmobius/gameserver/entity/actor/Player.java (storeRecommendations)
//
// Same layout as characters.rs: verbatim SQL, one method per statement.

use crate::error::Result;
use crate::pool::Pool;

// Java: Player.storeRecommendations
const REPLACE_RECO_BONUS: &str =
    "REPLACE INTO character_reco_bonus (charId,rec_have,rec_left,time_left) VALUES (?,?,?,?)";

// Java: Player.loadRecommendations
const SELECT_RECO_BONUS: &str =
    "SELECT rec_have,rec_left,time_left FROM character_reco_bonus WHERE charId=? LIMIT 1";

/// The row of `character_reco_bonus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecoBonus {
    /// `rec_have`.
    pub rec_have: i32,
    /// `rec_left`.
    pub rec_left: i32,
    /// `time_left`: milliseconds left of the Nevit hourglass.
    pub time_left: i64,
}

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

    /// The stored counters, `None` when the character has no row (Java then starts the hourglass
    /// with one hour and leaves the counters at 0).
    // Java: Player.loadRecommendations
    pub async fn reco_bonus_of(&self, char_id: i32) -> Result<Option<RecoBonus>> {
        let row: Option<(u8, u8, u64)> = sqlx::query_as(SELECT_RECO_BONUS)
            .bind(char_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|(rec_have, rec_left, time_left)| RecoBonus {
            rec_have: i32::from(rec_have),
            rec_left: i32::from(rec_left),
            time_left: i64::try_from(time_left).unwrap_or(i64::MAX),
        }))
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
