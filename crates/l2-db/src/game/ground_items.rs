//! The `itemsonground` table: the items lying on the ground, saved and restored only when
//! `SaveDroppedItem` is on (`General.ini`, default off).
// Java: org/l2jmobius/gameserver/managers/ItemsOnGroundManager.java (load, run, emptyTable)
//
// D-39: the Java game server uses the same table. This repository is only called when the owner
// turned `SaveDroppedItem` (or `ClearDroppedItemTable`) on, exactly like Java does, so a default
// setup never writes the table. Startup only *reads* the id column (`IdRepository`).

use sqlx::FromRow;

use crate::error::Result;
use crate::pool::Pool;

// Java: ItemsOnGroundManager.load (DESTROY_EQUIPABLE_PLAYER_ITEM off)
const RECYCLE_MISC_ITEMS: &str =
    "UPDATE itemsonground SET drop_time = ? WHERE drop_time = -1 AND equipable = 0";
// Java: ItemsOnGroundManager.load (DESTROY_EQUIPABLE_PLAYER_ITEM on)
const RECYCLE_ALL_ITEMS: &str = "UPDATE itemsonground SET drop_time = ? WHERE drop_time = -1";
// Java: ItemsOnGroundManager.load
const SELECT_GROUND_ITEMS: &str =
    "SELECT object_id,item_id,count,enchant_level,x,y,z,drop_time,equipable FROM itemsonground";
// Java: ItemsOnGroundManager.emptyTable
const EMPTY_TABLE: &str = "DELETE FROM itemsonground";
// Java: ItemsOnGroundManager.run
const INSERT_GROUND_ITEM: &str = "INSERT INTO itemsonground(object_id,item_id,count,enchant_level,x,y,z,drop_time,equipable) VALUES(?,?,?,?,?,?,?,?,?)";

/// One row of `itemsonground`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroundItemRecord {
    pub object_id: i32,
    pub item_id: i32,
    pub count: i64,
    pub enchant_level: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// Unix milliseconds the item was dropped, or -1 for an item that is never auto destroyed.
    pub drop_time: i64,
    pub equipable: bool,
}

/// Database shape of [`GroundItemRecord`]: every column but the key is nullable in the schema.
#[derive(Debug, Clone, FromRow)]
struct GroundItemDbRow {
    object_id: i32,
    item_id: Option<i32>,
    count: u64,
    enchant_level: Option<i32>,
    x: Option<i32>,
    y: Option<i32>,
    z: Option<i32>,
    drop_time: i64,
    equipable: Option<i32>,
}

impl From<GroundItemDbRow> for GroundItemRecord {
    fn from(r: GroundItemDbRow) -> Self {
        Self {
            object_id: r.object_id,
            // Java reads the columns with `getInt`, where NULL is 0.
            item_id: r.item_id.unwrap_or(0),
            count: i64::try_from(r.count).unwrap_or(i64::MAX),
            enchant_level: r.enchant_level.unwrap_or(0),
            x: r.x.unwrap_or(0),
            y: r.y.unwrap_or(0),
            z: r.z.unwrap_or(0),
            drop_time: r.drop_time,
            equipable: r.equipable.unwrap_or(0) != 0,
        }
    }
}

/// Reads and writes `itemsonground`.
#[derive(Clone)]
pub struct GroundItemRepository {
    pool: Pool,
}

impl GroundItemRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Java `ItemsOnGroundManager.emptyTable()`.
    pub async fn empty_table(&self) -> Result<()> {
        sqlx::query(EMPTY_TABLE).execute(&self.pool).await?;
        Ok(())
    }

    /// Java `ItemsOnGroundManager.load`'s recycling of protected items when
    /// `DestroyPlayerDroppedItem` is on: their `drop_time` becomes `now_ms` so the auto destroy
    /// picks them up. `include_equipable` is `DestroyEquipableItem`.
    pub async fn recycle_protected(&self, now_ms: i64, include_equipable: bool) -> Result<()> {
        let sql = if include_equipable {
            RECYCLE_ALL_ITEMS
        } else {
            RECYCLE_MISC_ITEMS
        };
        sqlx::query(sql).bind(now_ms).execute(&self.pool).await?;
        Ok(())
    }

    /// Java `ItemsOnGroundManager.load`'s select, ordered like the table scan (no `ORDER BY`).
    pub async fn load_all(&self) -> Result<Vec<GroundItemRecord>> {
        let rows: Vec<GroundItemDbRow> = sqlx::query_as(SELECT_GROUND_ITEMS).fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(GroundItemRecord::from).collect())
    }

    /// Java `ItemsOnGroundManager.run()`: the table is emptied, then every item is inserted. A row
    /// that fails to insert is logged and skipped (Java logs and goes on).
    pub async fn replace_all(&self, rows: &[GroundItemRecord]) -> Result<()> {
        self.empty_table().await?;
        for row in rows {
            let result = sqlx::query(INSERT_GROUND_ITEM)
                .bind(row.object_id)
                .bind(row.item_id)
                .bind(row.count)
                .bind(row.enchant_level)
                .bind(row.x)
                .bind(row.y)
                .bind(row.z)
                .bind(row.drop_time)
                .bind(i32::from(row.equipable))
                .execute(&self.pool)
                .await;
            if let Err(e) = result {
                tracing::error!("Error while inserting into table ItemsOnGround: {e}");
            }
        }
        Ok(())
    }
}
