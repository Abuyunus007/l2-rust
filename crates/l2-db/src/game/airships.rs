//! The `airships` table: the fuel of the air ship of each clan (M8-VEHICLE-01).
// Java: managers/AirShipManager.java (load, registerLicense, storeInDb)

use sqlx::FromRow;

use crate::error::Result;
use crate::pool::Pool;

// Java: AirShipManager.LOAD_DB, ADD_DB, UPDATE_DB. `fuel` is a `decimal(5,0)`; the cast makes it
// the integer `rs.getInt("fuel")` reads.
const SELECT_ALL: &str = "SELECT owner_id, CAST(fuel AS SIGNED) AS fuel FROM airships";
const INSERT: &str = "INSERT INTO airships (owner_id,fuel) VALUES (?,?)";
const UPDATE: &str = "UPDATE airships SET fuel=? WHERE owner_id=?";

/// A row of `airships`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AirshipRow {
    pub owner_id: i32,
    pub fuel: i32,
}

#[derive(Debug, FromRow)]
struct AirshipDbRow {
    owner_id: i32,
    fuel: i64,
}

/// Every query on `airships`.
#[derive(Debug, Clone)]
pub struct AirshipRepository {
    pool: Pool,
}

impl AirshipRepository {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Every row (`AirShipManager.load`).
    pub async fn all(&self) -> Result<Vec<AirshipRow>> {
        let rows = sqlx::query_as::<_, AirshipDbRow>(SELECT_ALL)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .into_iter()
            .map(|r| AirshipRow {
                owner_id: r.owner_id,
                fuel: i32::try_from(r.fuel).unwrap_or(0),
            })
            .collect())
    }

    /// `registerLicense`.
    pub async fn insert(&self, owner_id: i32, fuel: i32) -> Result<()> {
        sqlx::query(INSERT)
            .bind(owner_id)
            .bind(fuel)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// `storeInDb`.
    pub async fn update_fuel(&self, owner_id: i32, fuel: i32) -> Result<()> {
        sqlx::query(UPDATE)
            .bind(fuel)
            .bind(owner_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_statements_have_one_placeholder_per_value() {
        assert_eq!(INSERT.matches('?').count(), 2);
        assert_eq!(UPDATE.matches('?').count(), 2);
    }
}
