//! The residence tables: `castle`, `castle_functions`, `fort`, `fort_functions`, `clanhall`,
//! `clanhall_functions`, `siegable_clanhall`, the `hasCastle` column of `clan_data` and the circlet
//! rows of `items` (M7-RES-01).
// Java: org/l2jmobius/gameserver/managers/CastleManager.java (loadInstances, removeCirclet)
// Java: org/l2jmobius/gameserver/mechanics/siege/Castle.java (load, loadFunctions, removeFunction,
//       CastleFunction.dbSave, addToTreasuryNoTax, setTaxPercent, updateShowNpcCrest,
//       setTicketBuyCount, updateOwnerInDB)
// Java: org/l2jmobius/gameserver/managers/FortManager.java (loadInstances)
// Java: org/l2jmobius/gameserver/mechanics/siege/Fort.java (load, loadFunctions, removeFunction,
//       FortFunction.dbSave, saveFortVariables, updateOwnerInDB, setFortState)
// Java: org/l2jmobius/gameserver/data/sql/ClanHallTable.java (load)
// Java: org/l2jmobius/gameserver/entity/residences/{ClanHall,AuctionableHall}.java (loadFunctions,
//       removeFunction, ClanHallFunction.dbSave, updateDb)
// Java: org/l2jmobius/gameserver/managers/CHSiegeManager.java (loadClanHalls)
// Java: org/l2jmobius/gameserver/mechanics/siege/clanhalls/SiegableHall.java (updateDb)
//
// Same layout as clans.rs: verbatim SQL, one method per statement. D-39: the Java game server
// reads and writes the same rows. The three `*_by_owner` reads are Rust-only: the re-read of a
// clan's residences at `CharacterSelect` (D-50, D-51).

use sqlx::FromRow;

use super::sql_number::SqlNumber;
use crate::error::Result;
use crate::pool::Pool;

// Java: CastleManager.loadInstances
const SELECT_CASTLE_IDS: &str = "SELECT id FROM castle ORDER BY id";
// Java: Castle.load (ps1, ps2)
const SELECT_CASTLE: &str = "SELECT * FROM castle WHERE id = ?";
const SELECT_CASTLE_OWNER: &str = "SELECT clan_id FROM clan_data WHERE hasCastle = ?";
// Java: Castle.loadFunctions, CastleFunction.dbSave, Castle.removeFunction
const SELECT_CASTLE_FUNCTIONS: &str = "SELECT * FROM castle_functions WHERE castle_id = ?";
const REPLACE_CASTLE_FUNCTION: &str =
    "REPLACE INTO castle_functions (castle_id, type, lvl, lease, rate, endTime) VALUES (?,?,?,?,?,?)";
const DELETE_CASTLE_FUNCTION: &str = "DELETE FROM castle_functions WHERE castle_id=? AND type=?";
// Java: Castle.addToTreasuryNoTax, setTaxPercent, updateShowNpcCrest, setTicketBuyCount
const UPDATE_CASTLE_TREASURY: &str = "UPDATE castle SET treasury = ? WHERE id = ?";
const UPDATE_CASTLE_TAX_PERCENT: &str = "UPDATE castle SET taxPercent = ? WHERE id = ?";
const UPDATE_CASTLE_SHOW_NPC_CREST: &str = "UPDATE castle SET showNpcCrest = ? WHERE id = ?";
const UPDATE_CASTLE_TICKET_BUY_COUNT: &str = "UPDATE castle SET ticketBuyCount = ? WHERE id = ?";
// Java: Castle.updateOwnerInDB (two statements)
const CLEAR_CASTLE_OWNER: &str = "UPDATE clan_data SET hasCastle = 0 WHERE hasCastle = ?";
const SET_CASTLE_OWNER: &str = "UPDATE clan_data SET hasCastle = ? WHERE clan_id = ?";
// Java: CastleManager.removeCirclet (an offline member)
const DELETE_CIRCLETS: &str = "DELETE FROM items WHERE owner_id = ? and item_id = ?";

// Java: FortManager.loadInstances, Fort.load
const SELECT_FORT_IDS: &str = "SELECT id FROM fort ORDER BY id";
const SELECT_FORT: &str = "SELECT * FROM fort WHERE id = ?";
// Rust-only (D-51): the fortresses of one clan.
const SELECT_FORTS_OF_OWNER: &str = "SELECT * FROM fort WHERE owner = ?";
// Java: Fort.loadFunctions, FortFunction.dbSave, Fort.removeFunction
const SELECT_FORT_FUNCTIONS: &str = "SELECT * FROM fort_functions WHERE fort_id = ?";
const REPLACE_FORT_FUNCTION: &str =
    "REPLACE INTO fort_functions (fort_id, type, lvl, lease, rate, endTime) VALUES (?,?,?,?,?,?)";
const DELETE_FORT_FUNCTION: &str = "DELETE FROM fort_functions WHERE fort_id=? AND type=?";
// Java: Fort.saveFortVariables, updateOwnerInDB, setFortState
const UPDATE_FORT_SUPPLY_LEVEL: &str = "UPDATE fort SET supplyLvL=? WHERE id = ?";
const UPDATE_FORT_OWNER: &str = "UPDATE fort SET owner=?,lastOwnedTime=?,state=?,castleId=? WHERE id = ?";
const UPDATE_FORT_STATE: &str = "UPDATE fort SET state=?,castleId=? WHERE id = ?";

// Java: ClanHallTable.load
const SELECT_CLAN_HALLS: &str = "SELECT * FROM clanhall ORDER BY id";
// Rust-only (D-51): the halls of one clan.
const SELECT_CLAN_HALLS_OF_OWNER: &str = "SELECT * FROM clanhall WHERE ownerId = ?";
// Java: ClanHall.loadFunctions, ClanHallFunction.dbSave, ClanHall.removeFunction
const SELECT_CLAN_HALL_FUNCTIONS: &str = "SELECT * FROM clanhall_functions WHERE hall_id = ?";
const REPLACE_CLAN_HALL_FUNCTION: &str =
    "REPLACE INTO clanhall_functions (hall_id, type, lvl, lease, rate, endTime) VALUES (?,?,?,?,?,?)";
const DELETE_CLAN_HALL_FUNCTION: &str = "DELETE FROM clanhall_functions WHERE hall_id=? AND type=?";
// Java: AuctionableHall.updateDb
const UPDATE_CLAN_HALL: &str = "UPDATE clanhall SET ownerId=?, paidUntil=?, paid=? WHERE id=?";

// Java: CHSiegeManager.SQL_LOAD_HALLS
const SELECT_SIEGABLE_HALLS: &str = "SELECT * FROM siegable_clanhall";
// Rust-only (D-51): the conquerable halls of one clan.
const SELECT_SIEGABLE_HALLS_OF_OWNER: &str = "SELECT * FROM siegable_clanhall WHERE ownerId = ?";
// Java: SiegableHall.SQL_SAVE
const UPDATE_SIEGABLE_HALL: &str = "UPDATE siegable_clanhall SET ownerId=?, nextSiege=? WHERE clanHallId=?";

/// Which of the three function tables (`castle_functions`, `fort_functions`,
/// `clanhall_functions`): Java has one function class per residence kind with the same columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunctionTable {
    Castle,
    Fort,
    ClanHall,
}

/// A `castle` row (`SELECT *`). The `enum('true','false')` columns are their text; Java reads
/// them with `Boolean.parseBoolean`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastleRow {
    pub id: i32,
    pub name: String,
    pub tax_percent: i32,
    pub treasury: i64,
    /// `siegeDate` (`getLong`, epoch milliseconds).
    pub siege_date: i64,
    pub reg_time_over: String,
    pub reg_time_end: i64,
    pub show_npc_crest: String,
    pub ticket_buy_count: i32,
}

#[derive(Debug, Clone, FromRow)]
struct CastleDbRow {
    id: i32,
    name: String,
    #[sqlx(rename = "taxPercent")]
    tax_percent: i32,
    treasury: i64,
    #[sqlx(rename = "siegeDate")]
    siege_date: u64,
    #[sqlx(rename = "regTimeOver")]
    reg_time_over: String,
    #[sqlx(rename = "regTimeEnd")]
    reg_time_end: u64,
    #[sqlx(rename = "showNpcCrest")]
    show_npc_crest: String,
    #[sqlx(rename = "ticketBuyCount")]
    ticket_buy_count: i16,
}

impl From<CastleDbRow> for CastleRow {
    fn from(r: CastleDbRow) -> Self {
        Self {
            id: r.id,
            name: r.name,
            tax_percent: r.tax_percent,
            treasury: r.treasury,
            siege_date: long(r.siege_date),
            reg_time_over: r.reg_time_over,
            reg_time_end: long(r.reg_time_end),
            show_npc_crest: r.show_npc_crest,
            ticket_buy_count: i32::from(r.ticket_buy_count),
        }
    }
}

/// A `fort` row (`SELECT *`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FortRow {
    pub id: i32,
    pub name: String,
    pub siege_date: i64,
    pub last_owned_time: i64,
    /// `owner`: the clan id, 0 for none.
    pub owner: i32,
    pub fort_type: i32,
    pub state: i32,
    /// `castleId`: the castle the fortress is contracted with.
    pub castle_id: i32,
    pub supply_level: i32,
}

#[derive(Debug, Clone, FromRow)]
struct FortDbRow {
    id: i32,
    name: String,
    #[sqlx(rename = "siegeDate")]
    siege_date: u64,
    #[sqlx(rename = "lastOwnedTime")]
    last_owned_time: u64,
    owner: i32,
    #[sqlx(rename = "fortType")]
    fort_type: i32,
    state: i32,
    #[sqlx(rename = "castleId")]
    castle_id: i32,
    #[sqlx(rename = "supplyLvL")]
    supply_level: i32,
}

impl From<FortDbRow> for FortRow {
    fn from(r: FortDbRow) -> Self {
        Self {
            id: r.id,
            name: r.name,
            siege_date: long(r.siege_date),
            last_owned_time: long(r.last_owned_time),
            owner: r.owner,
            fort_type: r.fort_type,
            state: r.state,
            castle_id: r.castle_id,
            supply_level: r.supply_level,
        }
    }
}

/// A `clanhall` row (`SELECT *`): an auctionable hall.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClanHallRow {
    pub id: i32,
    pub name: String,
    pub owner_id: i32,
    pub lease: i32,
    pub desc: String,
    pub location: String,
    pub paid_until: i64,
    /// `Grade` (a `DECIMAL(1,0)`, `getInt`).
    pub grade: i32,
    /// `paid` (`getBoolean` of an int: not 0).
    pub paid: bool,
}

#[derive(Debug, Clone, FromRow)]
struct ClanHallDbRow {
    id: i32,
    name: String,
    #[sqlx(rename = "ownerId")]
    owner_id: i32,
    lease: i32,
    desc: String,
    location: String,
    #[sqlx(rename = "paidUntil")]
    paid_until: u64,
    #[sqlx(rename = "Grade")]
    grade: SqlNumber<i32>,
    paid: i32,
}

impl From<ClanHallDbRow> for ClanHallRow {
    fn from(r: ClanHallDbRow) -> Self {
        Self {
            id: r.id,
            name: r.name,
            owner_id: r.owner_id,
            lease: r.lease,
            desc: r.desc,
            location: r.location,
            paid_until: long(r.paid_until),
            grade: r.grade.0,
            paid: r.paid != 0,
        }
    }
}

/// A `siegable_clanhall` row (`SELECT *`). Every column but the id may be NULL; Java's `getInt` /
/// `getLong` read NULL as 0, `getString` as `null`.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct SiegableHallRow {
    #[sqlx(rename = "clanHallId")]
    pub clan_hall_id: i32,
    pub name: Option<String>,
    #[sqlx(rename = "ownerId")]
    pub owner_id: Option<i32>,
    pub desc: Option<String>,
    pub location: Option<String>,
    #[sqlx(rename = "nextSiege")]
    pub next_siege: Option<i64>,
    #[sqlx(rename = "siegeLength")]
    pub siege_length: Option<i32>,
    pub schedule_config: Option<String>,
}

/// A row of one of the three function tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunctionRow {
    /// `type`: the function (`FUNC_TELEPORT` ...).
    pub function_type: i32,
    pub lvl: i32,
    pub lease: i32,
    /// `rate` (a `DECIMAL(20,0)`): milliseconds between two fees.
    pub rate: i64,
    /// `endTime`: epoch milliseconds the paid time ends.
    pub end_time: i64,
}

#[derive(Debug, Clone, FromRow)]
struct FunctionDbRow {
    #[sqlx(rename = "type")]
    function_type: i32,
    lvl: i32,
    lease: i32,
    rate: SqlNumber<i64>,
    #[sqlx(rename = "endTime")]
    end_time: u64,
}

impl From<FunctionDbRow> for FunctionRow {
    fn from(r: FunctionDbRow) -> Self {
        Self {
            function_type: r.function_type,
            lvl: r.lvl,
            lease: r.lease,
            rate: r.rate.0,
            end_time: long(r.end_time),
        }
    }
}

/// The values a `dbSave` of a function binds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunctionSave {
    pub table: FunctionTable,
    pub residence_id: i32,
    pub function_type: i32,
    pub lvl: i32,
    pub lease: i32,
    pub rate: i64,
    /// Java writes -1 for a function whose lease went up (`updateFunctions`), which the unsigned
    /// column takes as 0 (or refuses in strict mode, which Java logs).
    pub end_time: i64,
}

/// `getLong` of an unsigned `BIGINT`.
fn long(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

/// Java `String.valueOf(boolean)` for the `enum('true','false')` columns.
fn bool_text(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

/// Every query on the residence tables.
#[derive(Debug, Clone)]
pub struct ResidenceRepository {
    pool: Pool,
}

impl ResidenceRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    // ------------------------------------------------------------------------------ castles

    // Java: CastleManager.loadInstances
    pub async fn castle_ids(&self) -> Result<Vec<i32>> {
        let ids = sqlx::query_scalar::<_, i32>(SELECT_CASTLE_IDS)
            .fetch_all(&self.pool)
            .await?;
        Ok(ids)
    }

    /// The `castle` row; Java loops over the result, so `None` leaves every field at its start
    /// value.
    // Java: Castle.load (ps1)
    pub async fn castle(&self, castle_id: i32) -> Result<Option<CastleRow>> {
        let row = sqlx::query_as::<_, CastleDbRow>(SELECT_CASTLE)
            .bind(castle_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(CastleRow::from))
    }

    /// The clans whose `hasCastle` is the castle, in the database's order (Java keeps the last).
    // Java: Castle.load (ps2)
    pub async fn castle_owner_ids(&self, castle_id: i32) -> Result<Vec<i32>> {
        let ids = sqlx::query_scalar::<_, i32>(SELECT_CASTLE_OWNER)
            .bind(castle_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(ids)
    }

    // Java: Castle.addToTreasuryNoTax
    pub async fn update_castle_treasury(&self, castle_id: i32, treasury: i64) -> Result<()> {
        sqlx::query(UPDATE_CASTLE_TREASURY)
            .bind(treasury)
            .bind(castle_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // Java: Castle.setTaxPercent
    pub async fn update_castle_tax_percent(&self, castle_id: i32, tax_percent: i32) -> Result<()> {
        sqlx::query(UPDATE_CASTLE_TAX_PERCENT)
            .bind(tax_percent)
            .bind(castle_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // Java: Castle.updateShowNpcCrest
    pub async fn update_castle_show_npc_crest(&self, castle_id: i32, show: bool) -> Result<()> {
        sqlx::query(UPDATE_CASTLE_SHOW_NPC_CREST)
            .bind(bool_text(show))
            .bind(castle_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // Java: Castle.setTicketBuyCount
    pub async fn update_castle_ticket_buy_count(&self, castle_id: i32, count: i32) -> Result<()> {
        sqlx::query(UPDATE_CASTLE_TICKET_BUY_COUNT)
            .bind(count)
            .bind(castle_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// The castle's old owner loses `hasCastle`, then the new one (0 for none) gets it. Java runs
    /// the two statements on their own (no transaction); so does this.
    // Java: Castle.updateOwnerInDB
    pub async fn update_castle_owner(&self, castle_id: i32, clan_id: i32) -> Result<()> {
        sqlx::query(CLEAR_CASTLE_OWNER)
            .bind(castle_id)
            .execute(&self.pool)
            .await?;
        sqlx::query(SET_CASTLE_OWNER)
            .bind(castle_id)
            .bind(clan_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// The circlets of an offline clan member.
    // Java: CastleManager.removeCirclet
    pub async fn delete_circlets(&self, owner_id: i32, item_id: i32) -> Result<()> {
        sqlx::query(DELETE_CIRCLETS)
            .bind(owner_id)
            .bind(item_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ------------------------------------------------------------------------------ forts

    // Java: FortManager.loadInstances
    pub async fn fort_ids(&self) -> Result<Vec<i32>> {
        let ids = sqlx::query_scalar::<_, i32>(SELECT_FORT_IDS)
            .fetch_all(&self.pool)
            .await?;
        Ok(ids)
    }

    // Java: Fort.load
    pub async fn fort(&self, fort_id: i32) -> Result<Option<FortRow>> {
        let row = sqlx::query_as::<_, FortDbRow>(SELECT_FORT)
            .bind(fort_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(FortRow::from))
    }

    /// Rust-only (D-51): the fortresses a clan owns by the table (the re-read at
    /// `CharacterSelect`).
    pub async fn forts_of_owner(&self, clan_id: i32) -> Result<Vec<FortRow>> {
        let rows = sqlx::query_as::<_, FortDbRow>(SELECT_FORTS_OF_OWNER)
            .bind(clan_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(FortRow::from).collect())
    }

    // Java: Fort.saveFortVariables
    pub async fn update_fort_supply_level(&self, fort_id: i32, level: i32) -> Result<()> {
        sqlx::query(UPDATE_FORT_SUPPLY_LEVEL)
            .bind(level)
            .bind(fort_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Java binds `state` and `castleId` as 0.
    // Java: Fort.updateOwnerInDB
    pub async fn update_fort_owner(&self, fort_id: i32, owner: i32, last_owned_time: i64) -> Result<()> {
        sqlx::query(UPDATE_FORT_OWNER)
            .bind(owner)
            .bind(last_owned_time)
            .bind(0)
            .bind(0)
            .bind(fort_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // Java: Fort.setFortState
    pub async fn update_fort_state(&self, fort_id: i32, state: i32, castle_id: i32) -> Result<()> {
        sqlx::query(UPDATE_FORT_STATE)
            .bind(state)
            .bind(castle_id)
            .bind(fort_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ------------------------------------------------------------------------------ clan halls

    // Java: ClanHallTable.load
    pub async fn clan_halls(&self) -> Result<Vec<ClanHallRow>> {
        let rows = sqlx::query_as::<_, ClanHallDbRow>(SELECT_CLAN_HALLS)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(ClanHallRow::from).collect())
    }

    /// Rust-only (D-51): the halls a clan owns by the table.
    pub async fn clan_halls_of_owner(&self, clan_id: i32) -> Result<Vec<ClanHallRow>> {
        let rows = sqlx::query_as::<_, ClanHallDbRow>(SELECT_CLAN_HALLS_OF_OWNER)
            .bind(clan_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(ClanHallRow::from).collect())
    }

    // Java: AuctionableHall.updateDb
    pub async fn update_clan_hall(
        &self,
        hall_id: i32,
        owner_id: i32,
        paid_until: i64,
        paid: bool,
    ) -> Result<()> {
        sqlx::query(UPDATE_CLAN_HALL)
            .bind(owner_id)
            .bind(paid_until)
            .bind(i32::from(paid))
            .bind(hall_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // Java: CHSiegeManager.loadClanHalls
    pub async fn siegable_halls(&self) -> Result<Vec<SiegableHallRow>> {
        let rows = sqlx::query_as::<_, SiegableHallRow>(SELECT_SIEGABLE_HALLS)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// Rust-only (D-51): the conquerable halls a clan owns by the table.
    pub async fn siegable_halls_of_owner(&self, clan_id: i32) -> Result<Vec<SiegableHallRow>> {
        let rows = sqlx::query_as::<_, SiegableHallRow>(SELECT_SIEGABLE_HALLS_OF_OWNER)
            .bind(clan_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    // Java: SiegableHall.updateDb
    pub async fn update_siegable_hall(&self, hall_id: i32, owner_id: i32, next_siege: i64) -> Result<()> {
        sqlx::query(UPDATE_SIEGABLE_HALL)
            .bind(owner_id)
            .bind(next_siege)
            .bind(hall_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ------------------------------------------------------------------------------ functions

    // Java: Castle.loadFunctions, Fort.loadFunctions, ClanHall.loadFunctions
    pub async fn functions(&self, table: FunctionTable, residence_id: i32) -> Result<Vec<FunctionRow>> {
        let sql = match table {
            FunctionTable::Castle => SELECT_CASTLE_FUNCTIONS,
            FunctionTable::Fort => SELECT_FORT_FUNCTIONS,
            FunctionTable::ClanHall => SELECT_CLAN_HALL_FUNCTIONS,
        };
        let rows = sqlx::query_as::<_, FunctionDbRow>(sql)
            .bind(residence_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(FunctionRow::from).collect())
    }

    // Java: CastleFunction.dbSave, FortFunction.dbSave, ClanHallFunction.dbSave
    pub async fn save_function(&self, save: &FunctionSave) -> Result<()> {
        let sql = match save.table {
            FunctionTable::Castle => REPLACE_CASTLE_FUNCTION,
            FunctionTable::Fort => REPLACE_FORT_FUNCTION,
            FunctionTable::ClanHall => REPLACE_CLAN_HALL_FUNCTION,
        };
        sqlx::query(sql)
            .bind(save.residence_id)
            .bind(save.function_type)
            .bind(save.lvl)
            .bind(save.lease)
            .bind(save.rate)
            .bind(save.end_time)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // Java: Castle.removeFunction, Fort.removeFunction, ClanHall.removeFunction
    pub async fn delete_function(
        &self,
        table: FunctionTable,
        residence_id: i32,
        function_type: i32,
    ) -> Result<()> {
        let sql = match table {
            FunctionTable::Castle => DELETE_CASTLE_FUNCTION,
            FunctionTable::Fort => DELETE_FORT_FUNCTION,
            FunctionTable::ClanHall => DELETE_CLAN_HALL_FUNCTION,
        };
        sqlx::query(sql)
            .bind(residence_id)
            .bind(function_type)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
