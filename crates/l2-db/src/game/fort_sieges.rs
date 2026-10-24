//! The fortress siege tables: `fortsiege_clans`, `fort_spawnlist`, `fort_siege_guards` and the
//! `siegeDate` column of `fort` (M7-FORT-01).
// Java: org/l2jmobius/gameserver/mechanics/siege/FortSiege.java (loadSiegeClan, saveSiegeClan,
//       removeSiegeClan, clearSiegeClan, saveSiegeDate)
// Java: org/l2jmobius/gameserver/managers/FortSiegeManager.java (checkIsRegistered)
// Java: org/l2jmobius/gameserver/managers/FortSiegeGuardManager.java (loadSiegeGuard)
// Java: org/l2jmobius/gameserver/mechanics/siege/Fort.java (initNpcs, initSiegeNpcs,
//       initNpcCommanders, initSpecialEnvoys)
//
// Same layout as residences.rs: verbatim SQL, one method per statement. D-39: the Java game
// server reads and writes the same rows.
//
// NOTE(java-quirk): `Fort.loadDoorUpgrade` selects `fort_doorupgrade` but reads a column `id` the
// table does not have (it is `doorId`), and nothing ever inserts a row: the table stays empty and
// the method does nothing. So there is no repository method for it.

use sqlx::FromRow;

use crate::error::Result;
use crate::pool::Pool;

// Java: FortSiege.loadSiegeClan
const SELECT_SIEGE_CLANS: &str = "SELECT clan_id FROM fortsiege_clans WHERE fort_id=?";
// Java: FortSiegeManager.checkIsRegistered
const SELECT_IS_REGISTERED: &str = "SELECT clan_id FROM fortsiege_clans where clan_id=? and fort_id=?";
// Java: FortSiege.saveSiegeClan
const INSERT_SIEGE_CLAN: &str = "INSERT INTO fortsiege_clans (clan_id,fort_id) values (?,?)";
// Java: FortSiege.removeSiegeClan (DELETE_FORT_SIEGECLANS_BY_CLAN_ID, DELETE_FORT_SIEGECLANS)
const DELETE_SIEGE_CLAN: &str = "DELETE FROM fortsiege_clans WHERE fort_id = ? AND clan_id = ?";
const DELETE_SIEGE_CLANS: &str = "DELETE FROM fortsiege_clans WHERE fort_id = ?";
// Java: FortSiege.clearSiegeClan (two statements)
const CLEAR_SIEGE_CLANS: &str = "DELETE FROM fortsiege_clans WHERE fort_id=?";
const CLEAR_SIEGE_CLANS_OF_OWNER: &str = "DELETE FROM fortsiege_clans WHERE clan_id=?";
// Java: FortSiege.saveSiegeDate
const UPDATE_SIEGE_DATE: &str = "UPDATE fort SET siegeDate = ? WHERE id = ?";
// Java: Fort.initNpcs, initSiegeNpcs, initNpcCommanders, initSpecialEnvoys read the same table
// for one `spawnType` each (`ORDER BY id`, `initNpcs` without it, which is the primary key order
// all the same); this reads the fortress's rows once and the caller sorts them by type.
const SELECT_SPAWNS: &str = "SELECT id, npcId, x, y, z, heading, spawnType, castleId FROM fort_spawnlist WHERE fortId = ? ORDER BY id";
// Java: Clan.updateBloodOathCountInDB
const UPDATE_BLOOD_OATH_COUNT: &str = "UPDATE clan_data SET blood_oath_count=? WHERE clan_id=?";
// Rust-only (D-50): the blood oaths of a clan, read before they are handed out.
const SELECT_BLOOD_OATH_COUNT: &str = "SELECT blood_oath_count FROM clan_data WHERE clan_id=?";
// Java: FortSiegeGuardManager.loadSiegeGuard
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

    // Java: FortSiege.loadSiegeClan
    pub async fn siege_clans(&self, fort_id: i32) -> Result<Vec<i32>> {
        let ids = sqlx::query_scalar::<_, i32>(SELECT_SIEGE_CLANS)
            .bind(fort_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(ids)
    }

    // Java: FortSiegeManager.checkIsRegistered
    pub async fn is_registered(&self, clan_id: i32, fort_id: i32) -> Result<bool> {
        let rows = sqlx::query_scalar::<_, i32>(SELECT_IS_REGISTERED)
            .bind(clan_id)
            .bind(fort_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(!rows.is_empty())
    }

    // Java: FortSiege.saveSiegeClan
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
    // Java: FortSiege.removeSiegeClan
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
    // Java: FortSiege.clearSiegeClan
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

    // Java: FortSiege.saveSiegeDate
    pub async fn update_siege_date(&self, fort_id: i32, siege_date: i64) -> Result<()> {
        sqlx::query(UPDATE_SIEGE_DATE)
            .bind(siege_date)
            .bind(fort_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // Java: Fort.initNpcs, initSiegeNpcs, initNpcCommanders, initSpecialEnvoys
    pub async fn spawns(&self, fort_id: i32) -> Result<Vec<FortSpawnRow>> {
        let rows = sqlx::query_as::<_, FortSpawnDbRow>(SELECT_SPAWNS)
            .bind(fort_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(FortSpawnRow::from).collect())
    }

    // Java: Clan.updateBloodOathCountInDB
    pub async fn update_blood_oath_count(&self, clan_id: i32, count: i32) -> Result<()> {
        sqlx::query(UPDATE_BLOOD_OATH_COUNT)
            .bind(count)
            .bind(clan_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Rust-only (D-50): the clan's blood oaths as the table has them (`None` for no such clan).
    pub async fn blood_oath_count(&self, clan_id: i32) -> Result<Option<i32>> {
        let count = sqlx::query_scalar::<_, u16>(SELECT_BLOOD_OATH_COUNT)
            .bind(clan_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(count.map(i32::from))
    }

    // Java: FortSiegeGuardManager.loadSiegeGuard
    pub async fn siege_guards(&self, fort_id: i32) -> Result<Vec<FortSiegeGuardRow>> {
        let rows = sqlx::query_as::<_, FortSiegeGuardDbRow>(SELECT_SIEGE_GUARDS)
            .bind(fort_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(FortSiegeGuardRow::from).collect())
    }
}
