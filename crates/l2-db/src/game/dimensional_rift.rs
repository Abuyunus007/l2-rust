//! The `dimensional_rift` table: the rooms of the Dimensional Rift (M8-FEST-01). Read only.
// Java: managers/DimensionalRiftManager.java (loadRooms: `SELECT * FROM dimensional_rift`)

use sqlx::FromRow;

use crate::error::Result;
use crate::pool::Pool;

// Java: DimensionalRiftManager.loadRooms
const LOAD_ROOMS: &str = "SELECT * FROM dimensional_rift";

/// One room: `type` 0 holds the waiting room (`room_id` 0, the peace zone) and the rift zone
/// (`room_id` 1), 1 to 6 are the areas (recruit ... hero) with their nine rooms.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct DimensionalRiftRow {
    #[sqlx(rename = "type")]
    pub kind: u8,
    pub room_id: u8,
    #[sqlx(rename = "xMin")]
    pub x_min: i32,
    #[sqlx(rename = "xMax")]
    pub x_max: i32,
    #[sqlx(rename = "yMin")]
    pub y_min: i32,
    #[sqlx(rename = "yMax")]
    pub y_max: i32,
    #[sqlx(rename = "zMin")]
    pub z_min: i32,
    #[sqlx(rename = "zMax")]
    pub z_max: i32,
    /// Where the party arrives.
    #[sqlx(rename = "xT")]
    pub x_t: i32,
    #[sqlx(rename = "yT")]
    pub y_t: i32,
    #[sqlx(rename = "zT")]
    pub z_t: i32,
    /// `boss`: the room has the boss (the jump timer is longer).
    pub boss: u8,
}

/// Every query on `dimensional_rift`.
#[derive(Debug, Clone)]
pub struct DimensionalRiftRepository {
    pool: Pool,
}

impl DimensionalRiftRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Every room, in the database's order.
    pub async fn load_all(&self) -> Result<Vec<DimensionalRiftRow>> {
        Ok(sqlx::query_as::<_, DimensionalRiftRow>(LOAD_ROOMS)
            .fetch_all(&self.pool)
            .await?)
    }
}
