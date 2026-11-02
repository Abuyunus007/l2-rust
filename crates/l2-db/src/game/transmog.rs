//! The transmog tables: `character_transmogs` (the appearance collection of a character or of
//! an account) and the `transmogId` rows of `item_variables` (`Transmog.ini`, M9-C07).
// Java: dist/game/data/scripts/custom/Transmog/Transmog.java (LOAD_SQL, SAVE_SQL)
// Java: mechanics/variables/ItemVariables.java (UPSERT_QUERY, DELETE_QUERY)

use crate::error::Result;
use crate::pool::Pool;

// Java: Transmog.LOAD_SQL
const SELECT_COLLECTION: &str = "SELECT itemId FROM character_transmogs WHERE owner=? ORDER BY itemId ASC;";
// Java: Transmog.SAVE_SQL
const REPLACE_COLLECTION: &str = "REPLACE INTO character_transmogs (owner,itemId) VALUE (?,?)";
// Java: ItemVariables.UPSERT_QUERY
const UPSERT_VARIABLE: &str =
    "INSERT INTO item_variables (id, var, val) VALUES (?, ?, ?) ON DUPLICATE KEY UPDATE val = VALUES(val)";
// Java: ItemVariables.SELECT_QUERY, for the one variable
const SELECT_ITEM_TRANSMOG: &str = "SELECT val FROM item_variables WHERE id = ? AND var = 'transmogId'";
// Java: ItemVariables.DELETE_QUERY
const DELETE_VARIABLE: &str = "DELETE FROM item_variables WHERE id = ? AND var = ?";

/// `ItemVariables.TRANSMOG_ID`.
pub const TRANSMOG_ID: &str = "transmogId";

/// The transmog rows.
#[derive(Debug, Clone)]
pub struct TransmogRepository {
    pool: Pool,
}

impl TransmogRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// The item ids of the collection of `owner` (the account name or the object id as text),
    /// in ascending order.
    pub async fn load_collection(&self, owner: &str) -> Result<Vec<i32>> {
        let rows: Vec<(u32,)> = sqlx::query_as(SELECT_COLLECTION)
            .bind(owner)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(|(id,)| i32::try_from(id).unwrap_or(i32::MAX)).collect())
    }

    /// One `REPLACE` per item of the collection (Java batches them).
    pub async fn save_collection(&self, owner: &str, item_ids: &[i32]) -> Result<()> {
        for item_id in item_ids {
            sqlx::query(REPLACE_COLLECTION)
                .bind(owner)
                .bind(u32::try_from(*item_id).unwrap_or(0))
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    /// The `transmogId` of each of the items that has one, as `(object id, transmog id)`.
    // Java: `new ItemVariables(objectId).getInt(TRANSMOG_ID, 0)`, for the items of one inventory.
    pub async fn load_item_transmogs(&self, object_ids: &[i32]) -> Result<Vec<(i32, i32)>> {
        let mut out = Vec::new();
        for object_id in object_ids {
            let row: Option<(String,)> = sqlx::query_as(SELECT_ITEM_TRANSMOG)
                .bind(u32::try_from(*object_id).unwrap_or(0))
                .fetch_optional(&self.pool)
                .await?;
            if let Some((val,)) = row {
                // `getInt`: a value that is no number reads as the default 0.
                out.push((*object_id, val.trim().parse::<i32>().unwrap_or(0)));
            }
        }
        Ok(out)
    }

    /// `item.setTransmogId(id)` stored: the `transmogId` row of the item.
    pub async fn set_item_transmog(&self, object_id: i32, transmog_id: i32) -> Result<()> {
        sqlx::query(UPSERT_VARIABLE)
            .bind(u32::try_from(object_id).unwrap_or(0))
            .bind(TRANSMOG_ID)
            .bind(transmog_id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// `item.removeTransmog()` stored: the `transmogId` row of the item is deleted.
    pub async fn remove_item_transmog(&self, object_id: i32) -> Result<()> {
        sqlx::query(DELETE_VARIABLE)
            .bind(u32::try_from(object_id).unwrap_or(0))
            .bind(TRANSMOG_ID)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
