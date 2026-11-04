//! The fortress siege tables: `fortsiege_clans`, `fort_spawnlist`, `fort_siege_guards` and the
//! `siegeDate` column of `fort` (M7-FORT-01).

use sqlx::FromRow;

use crate::error::Result;
use crate::pool::Pool;

const SELECT_SIEGE_CLANS: &str = "SELECT clan_id FROM fortsiege_clans WHERE fort_id=?";
const SELECT_IS_REGISTERED: &str = "SELECT clan_id FROM fortsiege_clans where clan_id=? and fort_id=?";
const INSERT_SIEGE_CLAN: &str = "INSERT INTO fortsiege_clans (clan_id,fort_id) values (?,?)";
const DELETE_SIEGE_CLAN: &str = "DELETE FROM fortsiege_clans WHERE fort_id = ? AND clan_id = ?";
const DELETE_SIEGE_CLANS: &str = "DELETE FROM fortsiege_clans WHERE fort_id = ?";
const CLEAR_SIEGE_CLANS: &str = "DELETE FROM fortsiege_clans WHERE fort_id=?";
const CLEAR_SIEGE_CLANS_OF_OWNER: &str = "DELETE FROM fortsiege_clans WHERE clan_id=?";
const UPDATE_SIEGE_DATE: &str = "UPDATE fort SET siegeDate = ? WHERE id = ?";
// for one `spawnType` each (`ORDER BY id`, `initNpcs` without it, which is the primary key order
// all the same); this reads the fortress's rows once and the caller sorts them by type.
const SELECT_SPAWNS: &str = "SELECT id, npcId, x, y, z, heading, spawnType, castleId FROM fort_spawnlist WHERE fortId = ? ORDER BY id";
const UPDATE_BLOOD_OATH_COUNT: &str = "UPDATE clan_data SET blood_oath_count=? WHERE clan_id=?";
const SELECT_BLOOD_OATH_COUNT: &str = "SELECT blood_oath_count FROM clan_data WHERE clan_id=?";
const SELECT_SIEGE_GUARDS: &str =
    "SELECT npcId, x, y, z, heading, respawnDelay FROM fort_siege_guards WHERE fortId = ?";

/// A `fort_spawnlist` row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FortSpawnRow {
    pub id: i32,
    pub npc_id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
    /// 0 always spawned, 1 commanders (despawned during a siege), 2 siege NPCs (despawned ten
    /// minutes before a siege), 3 special envoys (spawned after the fortress was taken).
    pub spawn_type: i32,
    /// The castle a special envoy speaks for.
    pub castle_id: i32,
}

#[derive(Debug, Clone, FromRow)]
struct FortSpawnDbRow {
    id: u16,
    #[sqlx(rename = "npcId")]
    npc_id: u16,
    x: i32,
    y: i32,
    z: i32,
    heading: i32,
    #[sqlx(rename = "spawnType")]
    spawn_type: u8,
    #[sqlx(rename = "castleId")]
    castle_id: u8,
}

impl From<FortSpawnDbRow> for FortSpawnRow {
    fn from(r: FortSpawnDbRow) -> Self {
        Self {
            id: i32::from(r.id),
            npc_id: i32::from(r.npc_id),
            x: r.x,
            y: r.y,
            z: r.z,
            heading: r.heading,
            spawn_type: i32::from(r.spawn_type),
            castle_id: i32::from(r.castle_id),
        }
    }
}

/// A `fort_siege_guards` row (the columns Java reads).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FortSiegeGuardRow {
    pub npc_id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
    /// Seconds.
    pub respawn_delay: i32,
}

#[derive(Debug, Clone, FromRow)]
struct FortSiegeGuardDbRow {
    #[sqlx(rename = "npcId")]
    npc_id: u16,
    x: i32,
    y: i32,
    z: i32,
    heading: i32,
    #[sqlx(rename = "respawnDelay")]
    respawn_delay: i32,
}

impl From<FortSiegeGuardDbRow> for FortSiegeGuardRow {
    fn from(r: FortSiegeGuardDbRow) -> Self {
        Self {
            npc_id: i32::from(r.npc_id),
            x: r.x,
            y: r.y,
            z: r.z,
            heading: r.heading,
            respawn_delay: r.respawn_delay,
        }
    }
}

/// Every query on the fortress siege tables.
#[derive(Debug, Clone)]
pub struct FortSiegeRepository {
    pool: Pool,
}

impl FortSiegeRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    pub async fn siege_clans(&self, fort_id: i32) -> Result<Vec<i32>> {
        let ids = sqlx::query_scalar::<_, i32>(SELECT_SIEGE_CLANS)
            .bind(fort_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(ids)
    }

    pub async fn is_registered(&self, clan_id: i32, fort_id: i32) -> Result<bool> {
        let rows = sqlx::query_scalar::<_, i32>(SELECT_IS_REGISTERED)
            .bind(clan_id)
            .bind(fort_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(!rows.is_empty())
    }

    pub async fn insert_siege_clan(&self, clan_id: i32, fort_id: i32) -> Result<()> {
        sqlx::query(INSERT_SIEGE_CLAN)
            .bind(clan_id)
            .bind(fort_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// One clan's registration (`clan_id` 0 deletes every registration of the fortress, as Java's
    /// `removeSiegeClan(0)` does).
    pub async fn delete_siege_clan(&self, fort_id: i32, clan_id: i32) -> Result<()> {
        if clan_id != 0 {
            sqlx::query(DELETE_SIEGE_CLAN)
                .bind(fort_id)
                .bind(clan_id)
                .execute(&self.pool)
                .await?;
        } else {
            sqlx::query(DELETE_SIEGE_CLANS)
                .bind(fort_id)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    /// Every registration of the fortress; with an owner also every registration of the owner
    /// clan at any fortress (Java runs the second statement only when the fortress has an owner).
    pub async fn clear_siege_clans(&self, fort_id: i32, owner_clan_id: Option<i32>) -> Result<()> {
        sqlx::query(CLEAR_SIEGE_CLANS)
            .bind(fort_id)
            .execute(&self.pool)
            .await?;
        if let Some(clan_id) = owner_clan_id {
            sqlx::query(CLEAR_SIEGE_CLANS_OF_OWNER)
                .bind(clan_id)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    pub async fn update_siege_date(&self, fort_id: i32, siege_date: i64) -> Result<()> {
        sqlx::query(UPDATE_SIEGE_DATE)
            .bind(siege_date)
            .bind(fort_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn spawns(&self, fort_id: i32) -> Result<Vec<FortSpawnRow>> {
        let rows = sqlx::query_as::<_, FortSpawnDbRow>(SELECT_SPAWNS)
            .bind(fort_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(FortSpawnRow::from).collect())
    }

    pub async fn update_blood_oath_count(&self, clan_id: i32, count: i32) -> Result<()> {
        sqlx::query(UPDATE_BLOOD_OATH_COUNT)
            .bind(count)
            .bind(clan_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn blood_oath_count(&self, clan_id: i32) -> Result<Option<i32>> {
        let count = sqlx::query_scalar::<_, u16>(SELECT_BLOOD_OATH_COUNT)
            .bind(clan_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(count.map(i32::from))
    }

    pub async fn siege_guards(&self, fort_id: i32) -> Result<Vec<FortSiegeGuardRow>> {
        let rows = sqlx::query_as::<_, FortSiegeGuardDbRow>(SELECT_SIEGE_GUARDS)
            .bind(fort_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(FortSiegeGuardRow::from).collect())
    }
}
