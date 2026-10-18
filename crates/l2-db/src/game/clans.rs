//! The `clan_data` table, read only: who leads a clan.
// Java: org/l2jmobius/gameserver/entity/clan/Clan.java (SELECT_CLAN_DATA, `restore`),
// Java: data/sql/ClanTable.java (`getClan`), network/GameClient.java (markToDeleteChar)
//
// Java keeps every clan in `ClanTable` (loaded at startup) and answers `getClan(id)` from
// memory. Clans are not modelled in Rust yet, so the character screen asks the table instead.
// Same layout as characters.rs: verbatim SQL, one method per statement.

use sqlx::FromRow;

use crate::error::Result;
use crate::pool::Pool;

// Java: Clan.SELECT_CLAN_DATA
const SELECT_CLAN_DATA: &str = "SELECT * FROM clan_data where clan_id=?";

/// The part of a `clan_data` row the character screen needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromRow)]
struct ClanLeaderRow {
    /// `leader_id`: nullable in the schema, `getInt` reads NULL as 0.
    leader_id: Option<i32>,
}

/// Read access to `clan_data`.
#[derive(Debug, Clone)]
pub struct ClanRepository {
    pool: Pool,
}

impl ClanRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// The leader's character id of the clan, or `None` when no such clan exists
    /// (`ClanTable.getClan` returns `null`).
    // Java: Clan.restore (leader_id), ClanTable.getClan
    pub async fn leader_id(&self, clan_id: i32) -> Result<Option<i32>> {
        let row = sqlx::query_as::<_, ClanLeaderRow>(SELECT_CLAN_DATA)
            .bind(clan_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|r| r.leader_id.unwrap_or(0)))
    }
}
