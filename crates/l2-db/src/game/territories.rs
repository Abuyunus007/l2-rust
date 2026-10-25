//! The Territory War tables: `territory_spawnlist`, `territories`, `territory_registrations` and
//! the `nextTWStartDate` row of `global_variables` (M7-TW-01).
// Java: org/l2jmobius/gameserver/managers/TerritoryWarManager.java (load, changeRegistration,
//       updateTerritoryData, setNextTWDate)
// Java: org/l2jmobius/gameserver/managers/GlobalVariablesManager.java (the one variable)
//
// Same layout as residences.rs: verbatim SQL, one method per statement. D-39: the Java game
// server reads and writes the same rows; the world sends the writes only with
// `SiegeDatabaseWrites` (D-51).

use sqlx::FromRow;

use crate::error::Result;
use crate::pool::Pool;

// Java: TerritoryWarManager.load ("SELECT * FROM territory_spawnlist", no ORDER BY: InnoDB hands
// the rows out in primary key order, which is the order the ward places are filled in).
const SELECT_SPAWNS: &str =
    "SELECT castleId, id, npcId, x, y, z, heading, spawnType FROM territory_spawnlist";
// Java: TerritoryWarManager.load ("SELECT * FROM territories")
const SELECT_TERRITORIES: &str = "SELECT territoryId, castleId, fortId, ownedWardIds FROM territories";
// Java: TerritoryWarManager.load ("SELECT * FROM territory_registrations")
const SELECT_REGISTRATIONS: &str = "SELECT castleId, registeredId FROM territory_registrations";
// Java: TerritoryWarManager.INSERT
const INSERT_REGISTRATION: &str =
    "INSERT INTO territory_registrations (castleId, registeredId) values (?, ?)";
// Java: TerritoryWarManager.DELETE
const DELETE_REGISTRATION: &str =
    "DELETE FROM territory_registrations WHERE castleId = ? and registeredId = ?";
// Java: TerritoryWarManager.updateTerritoryData
const UPDATE_OWNED_WARD_IDS: &str = "UPDATE territories SET ownedWardIds=? WHERE territoryId=?";
// Java: GlobalVariablesManager (load and storeMe) for `TerritoryWarManager.GLOBAL_VARIABLE`.
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

    // Java: TerritoryWarManager.load (the spawn list)
    pub async fn spawns(&self) -> Result<Vec<TerritorySpawnRow>> {
        let rows = sqlx::query_as::<_, TerritorySpawnDbRow>(SELECT_SPAWNS)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(TerritorySpawnRow::from).collect())
    }

    // Java: TerritoryWarManager.load (the territories)
    pub async fn territories(&self) -> Result<Vec<TerritoryRow>> {
        let rows = sqlx::query_as::<_, TerritoryRow>(SELECT_TERRITORIES)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    // Java: TerritoryWarManager.load (the registrations)
    pub async fn registrations(&self) -> Result<Vec<TerritoryRegistrationRow>> {
        let rows = sqlx::query_as::<_, TerritoryRegistrationRow>(SELECT_REGISTRATIONS)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    // Java: TerritoryWarManager.changeRegistration(castleId, objId, false)
    pub async fn insert_registration(&self, castle_id: i32, registered_id: i32) -> Result<()> {
        sqlx::query(INSERT_REGISTRATION)
            .bind(castle_id)
            .bind(registered_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // Java: TerritoryWarManager.changeRegistration(castleId, objId, true)
    pub async fn delete_registration(&self, castle_id: i32, registered_id: i32) -> Result<()> {
        sqlx::query(DELETE_REGISTRATION)
            .bind(castle_id)
            .bind(registered_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // Java: TerritoryWarManager.updateTerritoryData
    pub async fn update_owned_ward_ids(&self, territory_id: i32, ward_ids: &str) -> Result<()> {
        sqlx::query(UPDATE_OWNED_WARD_IDS)
            .bind(ward_ids)
            .bind(territory_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// The `nextTWStartDate` value as the table has it (`None` without a row).
    // Java: GlobalVariablesManager (the load of the variables)
    pub async fn next_tw_start_date(&self) -> Result<Option<String>> {
        let value = sqlx::query_scalar::<_, Option<String>>(SELECT_VARIABLE)
            .bind(NEXT_TW_START_DATE)
            .fetch_optional(&self.pool)
            .await?;
        Ok(value.flatten())
    }

    // Java: GlobalVariablesManager.storeMe for `nextTWStartDate`
    pub async fn store_next_tw_start_date(&self, start_ms: i64) -> Result<()> {
        sqlx::query(REPLACE_VARIABLE)
            .bind(NEXT_TW_START_DATE)
            .bind(start_ms.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
