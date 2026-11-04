//! The castle siege tables: `siege_clans` (the registrations), `castle_siege_guards` (the guards
//! and the hired mercenaries), `castle_doorupgrade`, `castle_trapupgrade`, and the siege columns of
//! `castle` and `clan_data` (M7-SIEGE-01).

use sqlx::FromRow;

use crate::error::Result;
use crate::pool::Pool;

// Rust-only: Java's `SELECT clan_id,type FROM siege_clans where castle_id=?` for every castle.
const SELECT_SIEGE_CLANS: &str = "SELECT castle_id,clan_id,type FROM siege_clans";
const INSERT_SIEGE_CLAN: &str =
    "INSERT INTO siege_clans (clan_id,castle_id,type,castle_owner) values (?,?,?,0)";
const UPDATE_SIEGE_CLAN: &str = "UPDATE siege_clans SET type = ? WHERE castle_id = ? AND clan_id = ?";
const DELETE_SIEGE_CLAN: &str = "DELETE FROM siege_clans WHERE castle_id=? and clan_id=?";
const DELETE_SIEGE_CLANS_OF_CASTLE: &str = "DELETE FROM siege_clans WHERE castle_id=?";
const DELETE_SIEGE_CLANS_OF_CLAN: &str = "DELETE FROM siege_clans WHERE clan_id=?";
const DELETE_WAITING_SIEGE_CLANS: &str = "DELETE FROM siege_clans WHERE castle_id=? and type = 2";
const UPDATE_CASTLE_SIEGE_DATE: &str =
    "UPDATE castle SET siegeDate = ?, regTimeEnd = ?, regTimeOver = ?  WHERE id = ?";

// Rust-only: Java's `SELECT * FROM castle_siege_guards Where castleId = ? And isHired = ?` and
// `... Where isHired = 1` over the whole table.
const SELECT_SIEGE_GUARDS: &str = "SELECT * FROM castle_siege_guards";
const INSERT_SIEGE_GUARD: &str = "Insert Into castle_siege_guards (castleId, npcId, x, y, z, heading, respawnDelay, isHired) Values (?, ?, ?, ?, ?, ?, ?, ?)";
const DELETE_HIRED_GUARD: &str =
    "Delete From castle_siege_guards Where npcId = ? And x = ? AND y = ? AND z = ? AND isHired = 1";
const DELETE_HIRED_GUARDS: &str = "Delete From castle_siege_guards Where castleId = ? And isHired = 1";

// Rust-only: Java's `SELECT * FROM castle_doorupgrade WHERE castleId=?` of every castle.
const SELECT_DOOR_UPGRADES: &str = "SELECT * FROM castle_doorupgrade";
const REPLACE_DOOR_UPGRADE: &str = "REPLACE INTO castle_doorupgrade (doorId, ratio, castleId) values (?,?,?)";
const DELETE_DOOR_UPGRADES: &str = "DELETE FROM castle_doorupgrade WHERE castleId=?";
// Rust-only: Java's `SELECT * FROM castle_trapupgrade WHERE castleId=?` of every castle.
const SELECT_TRAP_UPGRADES: &str = "SELECT * FROM castle_trapupgrade";
const REPLACE_TRAP_UPGRADE: &str =
    "REPLACE INTO castle_trapupgrade (castleId, towerIndex, level) values (?,?,?)";
const DELETE_TRAP_UPGRADES: &str = "DELETE FROM castle_trapupgrade WHERE castleId=?";

const UPDATE_BLOOD_ALLIANCE_COUNT: &str = "UPDATE clan_data SET blood_alliance_count=? WHERE clan_id=?";

/// A `siege_clans` row (`type` NULL reads as 0, like Java's `getInt`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SiegeClanRow {
    pub castle_id: i32,
    pub clan_id: i32,
    pub kind: i32,
}

#[derive(Debug, Clone, FromRow)]
struct SiegeClanDbRow {
    castle_id: i32,
    clan_id: i32,
    #[sqlx(rename = "type")]
    kind: Option<i32>,
}

/// A `castle_siege_guards` row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SiegeGuardRow {
    pub castle_id: i32,
    pub id: i32,
    pub npc_id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
    /// Seconds.
    pub respawn_delay: i32,
    /// `isHired` (Java `getInt(...) > 0` is not asked; the queries compare with 0 and 1).
    pub is_hired: i32,
}

#[derive(Debug, Clone, FromRow)]
struct SiegeGuardDbRow {
    #[sqlx(rename = "castleId")]
    castle_id: u8,
    id: u16,
    #[sqlx(rename = "npcId")]
    npc_id: u16,
    x: i32,
    y: i32,
    z: i32,
    heading: i32,
    #[sqlx(rename = "respawnDelay")]
    respawn_delay: i32,
    #[sqlx(rename = "isHired")]
    is_hired: i8,
}

impl From<SiegeGuardDbRow> for SiegeGuardRow {
    fn from(r: SiegeGuardDbRow) -> Self {
        Self {
            castle_id: i32::from(r.castle_id),
            id: i32::from(r.id),
            npc_id: i32::from(r.npc_id),
            x: r.x,
            y: r.y,
            z: r.z,
            heading: r.heading,
            respawn_delay: r.respawn_delay,
            is_hired: i32::from(r.is_hired),
        }
    }
}

/// A new `castle_siege_guards` row (Java `saveSiegeGuard`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewSiegeGuard {
    pub castle_id: i32,
    pub npc_id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
    pub respawn_delay: i32,
    pub is_hired: i32,
}

/// A `castle_doorupgrade` row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DoorUpgradeRow {
    pub door_id: i32,
    pub ratio: i32,
    pub castle_id: i32,
}

#[derive(Debug, Clone, FromRow)]
struct DoorUpgradeDbRow {
    #[sqlx(rename = "doorId")]
    door_id: u32,
    ratio: u8,
    #[sqlx(rename = "castleId")]
    castle_id: u8,
}

/// A `castle_trapupgrade` row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrapUpgradeRow {
    pub castle_id: i32,
    pub tower_index: i32,
    pub level: i32,
}

#[derive(Debug, Clone, FromRow)]
struct TrapUpgradeDbRow {
    #[sqlx(rename = "castleId")]
    castle_id: u8,
    #[sqlx(rename = "towerIndex")]
    tower_index: u8,
    level: u8,
}

/// Every query on the castle siege tables.
#[derive(Debug, Clone)]
pub struct SiegeRepository {
    pool: Pool,
}

impl SiegeRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    // ------------------------------------------------------------------------- siege_clans

    /// Every registration, in the database's order.
    pub async fn siege_clans(&self) -> Result<Vec<SiegeClanRow>> {
        let rows = sqlx::query_as::<_, SiegeClanDbRow>(SELECT_SIEGE_CLANS)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .into_iter()
            .map(|r| SiegeClanRow {
                castle_id: r.castle_id,
                clan_id: r.clan_id,
                kind: r.kind.unwrap_or(0),
            })
            .collect())
    }

    pub async fn insert_siege_clan(&self, clan_id: i32, castle_id: i32, kind: i32) -> Result<()> {
        sqlx::query(INSERT_SIEGE_CLAN)
            .bind(clan_id)
            .bind(castle_id)
            .bind(kind)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn update_siege_clan(&self, castle_id: i32, clan_id: i32, kind: i32) -> Result<()> {
        sqlx::query(UPDATE_SIEGE_CLAN)
            .bind(kind)
            .bind(castle_id)
            .bind(clan_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete_siege_clan(&self, castle_id: i32, clan_id: i32) -> Result<()> {
        sqlx::query(DELETE_SIEGE_CLAN)
            .bind(castle_id)
            .bind(clan_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete_siege_clans_of_castle(&self, castle_id: i32) -> Result<()> {
        sqlx::query(DELETE_SIEGE_CLANS_OF_CASTLE)
            .bind(castle_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete_siege_clans_of_clan(&self, clan_id: i32) -> Result<()> {
        sqlx::query(DELETE_SIEGE_CLANS_OF_CLAN)
            .bind(clan_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete_waiting_siege_clans(&self, castle_id: i32) -> Result<()> {
        sqlx::query(DELETE_WAITING_SIEGE_CLANS)
            .bind(castle_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// The siege date, the end of the registration and whether it is over (`String.valueOf`).
    pub async fn update_castle_siege_date(
        &self,
        castle_id: i32,
        siege_date: i64,
        reg_time_end: i64,
        reg_time_over: bool,
    ) -> Result<()> {
        sqlx::query(UPDATE_CASTLE_SIEGE_DATE)
            .bind(siege_date)
            .bind(reg_time_end)
            .bind(if reg_time_over { "true" } else { "false" })
            .bind(castle_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ------------------------------------------------------------------ castle_siege_guards

    /// Every guard and mercenary, in the database's order.
    pub async fn siege_guards(&self) -> Result<Vec<SiegeGuardRow>> {
        let rows = sqlx::query_as::<_, SiegeGuardDbRow>(SELECT_SIEGE_GUARDS)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(SiegeGuardRow::from).collect())
    }

    pub async fn insert_siege_guard(&self, guard: &NewSiegeGuard) -> Result<()> {
        sqlx::query(INSERT_SIEGE_GUARD)
            .bind(guard.castle_id)
            .bind(guard.npc_id)
            .bind(guard.x)
            .bind(guard.y)
            .bind(guard.z)
            .bind(guard.heading)
            .bind(guard.respawn_delay)
            .bind(guard.is_hired)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete_hired_guard(&self, npc_id: i32, x: i32, y: i32, z: i32) -> Result<()> {
        sqlx::query(DELETE_HIRED_GUARD)
            .bind(npc_id)
            .bind(x)
            .bind(y)
            .bind(z)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete_hired_guards(&self, castle_id: i32) -> Result<()> {
        sqlx::query(DELETE_HIRED_GUARDS)
            .bind(castle_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ---------------------------------------------------------------------- the upgrades

    /// Every door upgrade.
    pub async fn door_upgrades(&self) -> Result<Vec<DoorUpgradeRow>> {
        let rows = sqlx::query_as::<_, DoorUpgradeDbRow>(SELECT_DOOR_UPGRADES)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .into_iter()
            .map(|r| DoorUpgradeRow {
                door_id: i32::try_from(r.door_id).unwrap_or(i32::MAX),
                ratio: i32::from(r.ratio),
                castle_id: i32::from(r.castle_id),
            })
            .collect())
    }

    pub async fn replace_door_upgrade(&self, door_id: i32, ratio: i32, castle_id: i32) -> Result<()> {
        sqlx::query(REPLACE_DOOR_UPGRADE)
            .bind(door_id)
            .bind(ratio)
            .bind(castle_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete_door_upgrades(&self, castle_id: i32) -> Result<()> {
        sqlx::query(DELETE_DOOR_UPGRADES)
            .bind(castle_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Every trap upgrade.
    pub async fn trap_upgrades(&self) -> Result<Vec<TrapUpgradeRow>> {
        let rows = sqlx::query_as::<_, TrapUpgradeDbRow>(SELECT_TRAP_UPGRADES)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .into_iter()
            .map(|r| TrapUpgradeRow {
                castle_id: i32::from(r.castle_id),
                tower_index: i32::from(r.tower_index),
                level: i32::from(r.level),
            })
            .collect())
    }

    pub async fn replace_trap_upgrade(&self, castle_id: i32, tower_index: i32, level: i32) -> Result<()> {
        sqlx::query(REPLACE_TRAP_UPGRADE)
            .bind(castle_id)
            .bind(tower_index)
            .bind(level)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete_trap_upgrades(&self, castle_id: i32) -> Result<()> {
        sqlx::query(DELETE_TRAP_UPGRADES)
            .bind(castle_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ------------------------------------------------------------------------- clan_data

    pub async fn update_blood_alliance_count(&self, clan_id: i32, count: i32) -> Result<()> {
        sqlx::query(UPDATE_BLOOD_ALLIANCE_COUNT)
            .bind(count)
            .bind(clan_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
