//! The Territory War tables: `territory_spawnlist`, `territories`, `territory_registrations` and
//! the `nextTWStartDate` row of `global_variables` (M7-TW-01).

use sqlx::FromRow;

use crate::error::Result;
use crate::pool::Pool;

// the rows out in primary key order, which is the order the ward places are filled in).
const SELECT_SPAWNS: &str =
    "SELECT castleId, id, npcId, x, y, z, heading, spawnType FROM territory_spawnlist";
const SELECT_TERRITORIES: &str = "SELECT territoryId, castleId, fortId, ownedWardIds FROM territories";
const SELECT_REGISTRATIONS: &str = "SELECT castleId, registeredId FROM territory_registrations";
const INSERT_REGISTRATION: &str =
    "INSERT INTO territory_registrations (castleId, registeredId) values (?, ?)";
const DELETE_REGISTRATION: &str =
    "DELETE FROM territory_registrations WHERE castleId = ? and registeredId = ?";
const UPDATE_OWNED_WARD_IDS: &str = "UPDATE territories SET ownedWardIds=? WHERE territoryId=?";
const SELECT_VARIABLE: &str = "SELECT `value` FROM global_variables WHERE `var` = ?";
const REPLACE_VARIABLE: &str = "REPLACE INTO global_variables (var, value) VALUES (?, ?)";

/// Java `TerritoryWarManager.GLOBAL_VARIABLE`: the next start of the war (epoch milliseconds).
pub const NEXT_TW_START_DATE: &str = "nextTWStartDate";

/// A `territory_spawnlist` row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerritorySpawnRow {
    pub castle_id: i32,
    pub id: i32,
    pub npc_id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
    /// 0 spawned while the castle has an owner, 1 fortress NPCs and 2 castle NPCs of the war, 3 a
    /// ward place.
    pub spawn_type: i32,
}

#[derive(Debug, Clone, FromRow)]
struct TerritorySpawnDbRow {
    #[sqlx(rename = "castleId")]
    castle_id: u8,
    id: u16,
    #[sqlx(rename = "npcId")]
    npc_id: u16,
    x: i32,
    y: i32,
    z: i32,
    heading: i32,
    #[sqlx(rename = "spawnType")]
    spawn_type: u8,
}

impl From<TerritorySpawnDbRow> for TerritorySpawnRow {
    fn from(r: TerritorySpawnDbRow) -> Self {
        Self {
            castle_id: i32::from(r.castle_id),
            id: i32::from(r.id),
            npc_id: i32::from(r.npc_id),
            x: r.x,
            y: r.y,
            z: r.z,
            heading: r.heading,
            spawn_type: i32::from(r.spawn_type),
        }
    }
}

/// A `territories` row.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct TerritoryRow {
    #[sqlx(rename = "territoryId")]
    pub territory_id: i32,
    #[sqlx(rename = "castleId")]
    pub castle_id: i32,
    #[sqlx(rename = "fortId")]
    pub fort_id: i32,
    /// The ward ids, each followed by `;` (`"81;85;"`).
    #[sqlx(rename = "ownedWardIds")]
    pub owned_ward_ids: String,
}

/// A `territory_registrations` row: a clan id or, when no clan has the id, a character's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromRow)]
pub struct TerritoryRegistrationRow {
    #[sqlx(rename = "castleId")]
    pub castle_id: i32,
    #[sqlx(rename = "registeredId")]
    pub registered_id: i32,
}

/// Every query on the Territory War tables.
#[derive(Debug, Clone)]
pub struct TerritoryRepository {
    pool: Pool,
}

impl TerritoryRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    pub async fn spawns(&self) -> Result<Vec<TerritorySpawnRow>> {
        let rows = sqlx::query_as::<_, TerritorySpawnDbRow>(SELECT_SPAWNS)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(TerritorySpawnRow::from).collect())
    }

    pub async fn territories(&self) -> Result<Vec<TerritoryRow>> {
        let rows = sqlx::query_as::<_, TerritoryRow>(SELECT_TERRITORIES)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    pub async fn registrations(&self) -> Result<Vec<TerritoryRegistrationRow>> {
        let rows = sqlx::query_as::<_, TerritoryRegistrationRow>(SELECT_REGISTRATIONS)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    pub async fn insert_registration(&self, castle_id: i32, registered_id: i32) -> Result<()> {
        sqlx::query(INSERT_REGISTRATION)
            .bind(castle_id)
            .bind(registered_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete_registration(&self, castle_id: i32, registered_id: i32) -> Result<()> {
        sqlx::query(DELETE_REGISTRATION)
            .bind(castle_id)
            .bind(registered_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn update_owned_ward_ids(&self, territory_id: i32, ward_ids: &str) -> Result<()> {
        sqlx::query(UPDATE_OWNED_WARD_IDS)
            .bind(ward_ids)
            .bind(territory_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// The `nextTWStartDate` value as the table has it (`None` without a row).
    pub async fn next_tw_start_date(&self) -> Result<Option<String>> {
        let value = sqlx::query_scalar::<_, Option<String>>(SELECT_VARIABLE)
            .bind(NEXT_TW_START_DATE)
            .fetch_optional(&self.pool)
            .await?;
        Ok(value.flatten())
    }

    pub async fn store_next_tw_start_date(&self, start_ms: i64) -> Result<()> {
        sqlx::query(REPLACE_VARIABLE)
            .bind(NEXT_TW_START_DATE)
            .bind(start_ms.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
