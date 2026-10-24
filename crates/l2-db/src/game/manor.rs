//! The manor tables: `castle_manor_production` (the seeds a castle sells) and
//! `castle_manor_procure` (the crops it buys), each with a current and a next period (M7-MANOR-01).
// Java: org/l2jmobius/gameserver/managers/CastleManorManager.java (loadDb, storeMe,
//       setNextSeedProduction, setNextCropProcure, updateCurrentProduction,
//       updateCurrentProcure, resetManorData)
//
// Same layout as residences.rs: verbatim SQL, one method per statement. D-39: the Java game server
// reads and writes the same rows (it rewrites both tables whole in `storeMe`). The world decides
// which writes go out (D-49, D-51).

use sqlx::FromRow;

use crate::error::Result;
use crate::pool::Pool;

// Java: CastleManorManager.loadDb
const SELECT_PRODUCTION: &str = "SELECT * FROM castle_manor_production WHERE castle_id=?";
const SELECT_PROCURE: &str = "SELECT * FROM castle_manor_procure WHERE castle_id=?";
// Java: CastleManorManager.INSERT_PRODUCT, INSERT_CROP
const INSERT_PRODUCT: &str = "INSERT INTO castle_manor_production VALUES (?, ?, ?, ?, ?, ?)";
const INSERT_CROP: &str = "INSERT INTO castle_manor_procure VALUES (?, ?, ?, ?, ?, ?, ?)";
// Java: CastleManorManager.storeMe
const DELETE_ALL_PRODUCTION: &str = "DELETE FROM castle_manor_production";
const DELETE_ALL_PROCURE: &str = "DELETE FROM castle_manor_procure";
// Java: CastleManorManager.setNextSeedProduction, setNextCropProcure
const DELETE_NEXT_PRODUCTION: &str =
    "DELETE FROM castle_manor_production WHERE castle_id = ? AND next_period = 1";
const DELETE_NEXT_PROCURE: &str = "DELETE FROM castle_manor_procure WHERE castle_id = ? AND next_period = 1";
// Java: CastleManorManager.updateCurrentProduction, updateCurrentProcure
const UPDATE_PRODUCTION_AMOUNT: &str =
    "UPDATE castle_manor_production SET amount = ? WHERE castle_id = ? AND seed_id = ? AND next_period = 0";
const UPDATE_PROCURE_AMOUNT: &str =
    "UPDATE castle_manor_procure SET amount = ? WHERE castle_id = ? AND crop_id = ? AND next_period = 0";
// Java: CastleManorManager.resetManorData
const DELETE_CASTLE_PRODUCTION: &str = "DELETE FROM castle_manor_production WHERE castle_id = ?";
const DELETE_CASTLE_PROCURE: &str = "DELETE FROM castle_manor_procure WHERE castle_id = ?";

/// A `castle_manor_production` row: a seed a castle sells in a period.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductionRow {
    pub castle_id: i32,
    pub seed_id: i32,
    /// `amount`: how many are left to buy.
    pub amount: i64,
    /// `start_amount`.
    pub start_amount: i64,
    pub price: i64,
    /// `next_period` (`getBoolean`: not 0).
    pub next_period: bool,
}

#[derive(Debug, Clone, FromRow)]
struct ProductionDbRow {
    castle_id: u8,
    seed_id: u32,
    amount: u32,
    start_amount: u32,
    price: u32,
    next_period: u8,
}

impl From<ProductionDbRow> for ProductionRow {
    fn from(r: ProductionDbRow) -> Self {
        Self {
            castle_id: i32::from(r.castle_id),
            seed_id: i32::try_from(r.seed_id).unwrap_or(i32::MAX),
            amount: i64::from(r.amount),
            start_amount: i64::from(r.start_amount),
            price: i64::from(r.price),
            next_period: r.next_period != 0,
        }
    }
}

/// A `castle_manor_procure` row: a crop a castle buys in a period.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcureRow {
    pub castle_id: i32,
    pub crop_id: i32,
    /// `amount`: how many the castle still wants.
    pub amount: i64,
    pub start_amount: i64,
    pub price: i64,
    /// `reward_type`.
    pub reward_type: i32,
    pub next_period: bool,
}

#[derive(Debug, Clone, FromRow)]
struct ProcureDbRow {
    castle_id: u8,
    crop_id: u32,
    amount: u32,
    start_amount: u32,
    price: u32,
    reward_type: u8,
    next_period: u8,
}

impl From<ProcureDbRow> for ProcureRow {
    fn from(r: ProcureDbRow) -> Self {
        Self {
            castle_id: i32::from(r.castle_id),
            crop_id: i32::try_from(r.crop_id).unwrap_or(i32::MAX),
            amount: i64::from(r.amount),
            start_amount: i64::from(r.start_amount),
            price: i64::from(r.price),
            reward_type: i32::from(r.reward_type),
            next_period: r.next_period != 0,
        }
    }
}

/// Every query on the manor tables.
#[derive(Debug, Clone)]
pub struct ManorRepository {
    pool: Pool,
}

impl ManorRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// The seeds a castle sells, both periods, in the database's order.
    // Java: CastleManorManager.loadDb (stProduction)
    pub async fn production(&self, castle_id: i32) -> Result<Vec<ProductionRow>> {
        let rows = sqlx::query_as::<_, ProductionDbRow>(SELECT_PRODUCTION)
            .bind(castle_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(ProductionRow::from).collect())
    }

    /// The crops a castle buys, both periods, in the database's order.
    // Java: CastleManorManager.loadDb (stProcure)
    pub async fn procure(&self, castle_id: i32) -> Result<Vec<ProcureRow>> {
        let rows = sqlx::query_as::<_, ProcureDbRow>(SELECT_PROCURE)
            .bind(castle_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(ProcureRow::from).collect())
    }

    /// Java `storeMe`: both tables are emptied and written again, every row with its period.
    /// Java runs the statements one by one on one connection; one transaction leaves the same
    /// rows and never half of them.
    // Java: CastleManorManager.storeMe
    pub async fn store_all(&self, production: &[ProductionRow], procure: &[ProcureRow]) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(DELETE_ALL_PRODUCTION).execute(&mut *tx).await?;
        for row in production {
            insert_production(&mut tx, row).await?;
        }
        sqlx::query(DELETE_ALL_PROCURE).execute(&mut *tx).await?;
        for row in procure {
            insert_procure(&mut tx, row).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Java `setNextSeedProduction` (with `ALT_MANOR_SAVE_ALL_ACTIONS`): the castle's next-period
    /// rows go, the new list is written.
    // Java: CastleManorManager.setNextSeedProduction
    pub async fn replace_next_production(&self, castle_id: i32, list: &[ProductionRow]) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(DELETE_NEXT_PRODUCTION)
            .bind(castle_id)
            .execute(&mut *tx)
            .await?;
        for row in list {
            insert_production(&mut tx, row).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Java `setNextCropProcure` (with `ALT_MANOR_SAVE_ALL_ACTIONS`).
    // Java: CastleManorManager.setNextCropProcure
    pub async fn replace_next_procure(&self, castle_id: i32, list: &[ProcureRow]) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(DELETE_NEXT_PROCURE)
            .bind(castle_id)
            .execute(&mut *tx)
            .await?;
        for row in list {
            insert_procure(&mut tx, row).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Java `updateCurrentProduction`: the amounts left of the current period (`(seed id,
    /// amount)`).
    // Java: CastleManorManager.updateCurrentProduction
    pub async fn update_current_production(&self, castle_id: i32, amounts: &[(i32, i64)]) -> Result<()> {
        for (seed_id, amount) in amounts {
            sqlx::query(UPDATE_PRODUCTION_AMOUNT)
                .bind(*amount)
                .bind(castle_id)
                .bind(*seed_id)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    /// Java `updateCurrentProcure`: the amounts left of the current period (`(crop id, amount)`).
    // Java: CastleManorManager.updateCurrentProcure
    pub async fn update_current_procure(&self, castle_id: i32, amounts: &[(i32, i64)]) -> Result<()> {
        for (crop_id, amount) in amounts {
            sqlx::query(UPDATE_PROCURE_AMOUNT)
                .bind(*amount)
                .bind(castle_id)
                .bind(*crop_id)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    /// Java `resetManorData(castleId)` (with `ALT_MANOR_SAVE_ALL_ACTIONS`): every row of the castle.
    // Java: CastleManorManager.resetManorData
    pub async fn delete_castle(&self, castle_id: i32) -> Result<()> {
        sqlx::query(DELETE_CASTLE_PRODUCTION)
            .bind(castle_id)
            .execute(&self.pool)
            .await?;
        sqlx::query(DELETE_CASTLE_PROCURE)
            .bind(castle_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

async fn insert_production(tx: &mut sqlx::MySqlConnection, row: &ProductionRow) -> Result<()> {
    sqlx::query(INSERT_PRODUCT)
        .bind(row.castle_id)
        .bind(row.seed_id)
        .bind(row.amount)
        .bind(row.start_amount)
        .bind(row.price)
        .bind(row.next_period)
        .execute(tx)
        .await?;
    Ok(())
}

async fn insert_procure(tx: &mut sqlx::MySqlConnection, row: &ProcureRow) -> Result<()> {
    sqlx::query(INSERT_CROP)
        .bind(row.castle_id)
        .bind(row.crop_id)
        .bind(row.amount)
        .bind(row.start_amount)
        .bind(row.price)
        .bind(row.reward_type)
        .bind(row.next_period)
        .execute(tx)
        .await?;
    Ok(())
}
