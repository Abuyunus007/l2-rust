//! The `character_offline_play` and `character_offline_play_group` tables: the auto play settings
//! of the characters that keep hunting without a client (offline play), and the groups they were
//! in at shutdown.

use sqlx::FromRow;

use crate::error::Result;
use crate::pool::Pool;

const LOAD_PLAYER_IDS: &str = "SELECT DISTINCT charId FROM character_offline_play";
const LOAD_PLAYER: &str = "SELECT * FROM character_offline_play WHERE charId=?";
const SAVE_PLAYER: &str = "INSERT INTO character_offline_play (charId, type, id) VALUES (?, ?, ?)";
const REMOVE_PLAYER: &str = "DELETE FROM character_offline_play WHERE charId=?";
const LOAD_GROUP_LEADER_IDS: &str = "SELECT DISTINCT leaderId FROM character_offline_play_group";
const LOAD_GROUP_MEMBERS: &str = "SELECT charId, type FROM character_offline_play_group WHERE leaderId=?";
const SAVE_GROUP_MEMBER: &str =
    "INSERT INTO character_offline_play_group (leaderId, charId, type) VALUES (?, ?, ?)";
const REMOVE_GROUP_MEMBERS: &str = "TRUNCATE TABLE character_offline_play_group";

/// A row of `character_offline_play` (without the `charId`).
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct OfflinePlayRow {
    /// `type`: 0 soulshot, 1 supply item, 2 buff, 3 skill, 4 action, 5 potion item, 100 the custom
    /// auto potion system.
    #[sqlx(rename = "type")]
    pub kind: u8,
    /// `id`.
    pub id: i32,
}

/// A row of `character_offline_play_group` for one leader.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct OfflinePlayGroupRow {
    /// `charId`: the member.
    #[sqlx(rename = "charId")]
    pub char_id: u32,
    /// `type`: the party's loot distribution id.
    #[sqlx(rename = "type")]
    pub kind: u8,
}

/// Every query on the offline play tables.
#[derive(Debug, Clone)]
pub struct OfflinePlayRepository {
    pool: Pool,
}

impl OfflinePlayRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// The characters with rows (`SELECT DISTINCT charId`).
    pub async fn load_player_ids(&self) -> Result<Vec<i32>> {
        let rows: Vec<(u32,)> = sqlx::query_as(LOAD_PLAYER_IDS).fetch_all(&self.pool).await?;
        Ok(rows
            .into_iter()
            .filter_map(|(id,)| i32::try_from(id).ok())
            .collect())
    }

    /// The rows of one character.
    pub async fn load_player(&self, char_id: i32) -> Result<Vec<OfflinePlayRow>> {
        Ok(sqlx::query_as::<_, OfflinePlayRow>(LOAD_PLAYER)
            .bind(char_id)
            .fetch_all(&self.pool)
            .await?)
    }

    /// `DELETE FROM character_offline_play WHERE charId=?`.
    pub async fn remove_player(&self, char_id: i32) -> Result<()> {
        sqlx::query(REMOVE_PLAYER)
            .bind(char_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// `INSERT INTO character_offline_play (charId, type, id) VALUES (?, ?, ?)`.
    pub async fn insert_player(&self, char_id: i32, kind: i32, id: i32) -> Result<()> {
        sqlx::query(SAVE_PLAYER)
            .bind(char_id)
            .bind(kind)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// The leaders with members (`SELECT DISTINCT leaderId`).
    pub async fn load_group_leader_ids(&self) -> Result<Vec<i32>> {
        let rows: Vec<(u32,)> = sqlx::query_as(LOAD_GROUP_LEADER_IDS)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .into_iter()
            .filter_map(|(id,)| i32::try_from(id).ok())
            .collect())
    }

    /// The members of one leader.
    pub async fn load_group_members(&self, leader_id: i32) -> Result<Vec<OfflinePlayGroupRow>> {
        Ok(sqlx::query_as::<_, OfflinePlayGroupRow>(LOAD_GROUP_MEMBERS)
            .bind(leader_id)
            .fetch_all(&self.pool)
            .await?)
    }

    /// `INSERT INTO character_offline_play_group (leaderId, charId, type) VALUES (?, ?, ?)`.
    pub async fn insert_group_member(&self, leader_id: i32, char_id: i32, kind: i32) -> Result<()> {
        sqlx::query(SAVE_GROUP_MEMBER)
            .bind(leader_id)
            .bind(char_id)
            .bind(kind)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// `TRUNCATE TABLE character_offline_play_group`.
    pub async fn clear_groups(&self) -> Result<()> {
        sqlx::query(REMOVE_GROUP_MEMBERS).execute(&self.pool).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statements_have_one_placeholder_per_bind() {
        assert_eq!(LOAD_PLAYER.matches('?').count(), 1);
        assert_eq!(SAVE_PLAYER.matches('?').count(), 3);
        assert_eq!(REMOVE_PLAYER.matches('?').count(), 1);
        assert_eq!(LOAD_GROUP_MEMBERS.matches('?').count(), 1);
        assert_eq!(SAVE_GROUP_MEMBER.matches('?').count(), 3);
        assert_eq!(LOAD_PLAYER_IDS.matches('?').count(), 0);
        assert_eq!(LOAD_GROUP_LEADER_IDS.matches('?').count(), 0);
        assert_eq!(REMOVE_GROUP_MEMBERS.matches('?').count(), 0);
    }
}
