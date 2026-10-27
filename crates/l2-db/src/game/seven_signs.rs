//! The Seven Signs tables: `seven_signs` (one row per registered character) and
//! `seven_signs_status` (the one row `id = 0` of the cycle), M8-SS-01.
// Java: org/l2jmobius/gameserver/mechanics/sevensigns/SevenSigns.java (LOAD_DATA, LOAD_STATUS,
//       INSERT_PLAYER, UPDATE_PLAYER, UPDATE_STATUS)
//
// Same layout as manor.rs: verbatim SQL, one method per statement. D-39: the Java game server
// reads and writes the same rows; the world decides which writes go out (D-53: the status row and
// the whole-table saves only with `CycleDatabaseWrites`).

use sqlx::FromRow;

use super::sql_number::SqlNumber;
use crate::error::Result;
use crate::pool::Pool;

// Java: SevenSigns.LOAD_DATA
const LOAD_DATA: &str = "SELECT charId, cabal, seal, red_stones, green_stones, blue_stones, ancient_adena_amount, contribution_score FROM seven_signs";
// The one row of LOAD_DATA a character select reads again (D-50).
const LOAD_PLAYER: &str = "SELECT charId, cabal, seal, red_stones, green_stones, blue_stones, ancient_adena_amount, contribution_score FROM seven_signs WHERE charId=?";
// Java: SevenSigns.LOAD_STATUS
const LOAD_STATUS: &str = "SELECT * FROM seven_signs_status WHERE id=0";
// Java: SevenSigns.INSERT_PLAYER
const INSERT_PLAYER: &str = "INSERT INTO seven_signs (charId, cabal, seal) VALUES (?,?,?)";
// Java: SevenSigns.UPDATE_PLAYER
const UPDATE_PLAYER: &str = "UPDATE seven_signs SET cabal=?, seal=?, red_stones=?, green_stones=?, blue_stones=?, ancient_adena_amount=?, contribution_score=? WHERE charId=?";
// Java: SevenSigns.UPDATE_STATUS
const UPDATE_STATUS: &str = "UPDATE seven_signs_status SET current_cycle=?, active_period=?, previous_winner=?, dawn_stone_score=?, dawn_festival_score=?, dusk_stone_score=?, dusk_festival_score=?, avarice_owner=?, gnosis_owner=?, strife_owner=?, avarice_dawn_score=?, gnosis_dawn_score=?, strife_dawn_score=?, avarice_dusk_score=?, gnosis_dusk_score=?, strife_dusk_score=?, festival_cycle=?, accumulated_bonus0=?, accumulated_bonus1=?, accumulated_bonus2=?,accumulated_bonus3=?, accumulated_bonus4=?, date=? WHERE id=0";

/// A `seven_signs` row. Java keeps the two `DECIMAL` columns as doubles (`getDouble`).
#[derive(Debug, Clone, PartialEq)]
pub struct SevenSignsRow {
    pub char_id: i32,
    /// `cabal`: `""`, `"dawn"` or `"dusk"`.
    pub cabal: String,
    pub seal: i32,
    pub red_stones: i32,
    pub green_stones: i32,
    pub blue_stones: i32,
    pub ancient_adena_amount: f64,
    pub contribution_score: f64,
}

#[derive(Debug, FromRow)]
struct SevenSignsDbRow {
    #[sqlx(rename = "charId")]
    char_id: u32,
    cabal: String,
    seal: i32,
    red_stones: i32,
    green_stones: i32,
    blue_stones: i32,
    ancient_adena_amount: SqlNumber<f64>,
    contribution_score: SqlNumber<f64>,
}

impl From<SevenSignsDbRow> for SevenSignsRow {
    fn from(r: SevenSignsDbRow) -> Self {
        Self {
            char_id: i32::try_from(r.char_id).unwrap_or(i32::MAX),
            cabal: r.cabal,
            seal: r.seal,
            red_stones: r.red_stones,
            green_stones: r.green_stones,
            blue_stones: r.blue_stones,
            ancient_adena_amount: r.ancient_adena_amount.0,
            contribution_score: r.contribution_score.0,
        }
    }
}

/// The `seven_signs_status` row (`id = 0`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SevenSignsStatusRow {
    pub current_cycle: i32,
    pub festival_cycle: i32,
    pub active_period: i32,
    /// `date`: the milliseconds of the last `saveSevenSignsStatus`.
    pub date: i64,
    pub previous_winner: i32,
    pub dawn_stone_score: f64,
    pub dawn_festival_score: i32,
    pub dusk_stone_score: f64,
    pub dusk_festival_score: i32,
    pub avarice_owner: i32,
    pub gnosis_owner: i32,
    pub strife_owner: i32,
    pub avarice_dawn_score: i32,
    pub gnosis_dawn_score: i32,
    pub strife_dawn_score: i32,
    pub avarice_dusk_score: i32,
    pub gnosis_dusk_score: i32,
    pub strife_dusk_score: i32,
    /// `accumulated_bonus0` .. `accumulated_bonus4` (the festivals).
    pub accumulated_bonus: [i32; 5],
}

#[derive(Debug, FromRow)]
struct SevenSignsStatusDbRow {
    current_cycle: i32,
    festival_cycle: i32,
    active_period: i32,
    date: u64,
    previous_winner: i32,
    dawn_stone_score: SqlNumber<f64>,
    dawn_festival_score: i32,
    dusk_stone_score: SqlNumber<f64>,
    dusk_festival_score: i32,
    avarice_owner: i32,
    gnosis_owner: i32,
    strife_owner: i32,
    avarice_dawn_score: i32,
    gnosis_dawn_score: i32,
    strife_dawn_score: i32,
    avarice_dusk_score: i32,
    gnosis_dusk_score: i32,
    strife_dusk_score: i32,
    accumulated_bonus0: i32,
    accumulated_bonus1: i32,
    accumulated_bonus2: i32,
    accumulated_bonus3: i32,
    accumulated_bonus4: i32,
}

impl From<SevenSignsStatusDbRow> for SevenSignsStatusRow {
    fn from(r: SevenSignsStatusDbRow) -> Self {
        Self {
            current_cycle: r.current_cycle,
            festival_cycle: r.festival_cycle,
            active_period: r.active_period,
            date: i64::try_from(r.date).unwrap_or(i64::MAX),
            previous_winner: r.previous_winner,
            dawn_stone_score: r.dawn_stone_score.0,
            dawn_festival_score: r.dawn_festival_score,
            dusk_stone_score: r.dusk_stone_score.0,
            dusk_festival_score: r.dusk_festival_score,
            avarice_owner: r.avarice_owner,
            gnosis_owner: r.gnosis_owner,
            strife_owner: r.strife_owner,
            avarice_dawn_score: r.avarice_dawn_score,
            gnosis_dawn_score: r.gnosis_dawn_score,
            strife_dawn_score: r.strife_dawn_score,
            avarice_dusk_score: r.avarice_dusk_score,
            gnosis_dusk_score: r.gnosis_dusk_score,
            strife_dusk_score: r.strife_dusk_score,
            accumulated_bonus: [
                r.accumulated_bonus0,
                r.accumulated_bonus1,
                r.accumulated_bonus2,
                r.accumulated_bonus3,
                r.accumulated_bonus4,
            ],
        }
    }
}

/// Every query on the Seven Signs tables.
#[derive(Debug, Clone)]
pub struct SevenSignsRepository {
    pool: Pool,
}

impl SevenSignsRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Every `seven_signs` row, in the database's order.
    // Java: SevenSigns.restoreSevenSignsData (LOAD_DATA)
    pub async fn load_all(&self) -> Result<Vec<SevenSignsRow>> {
        let rows = sqlx::query_as::<_, SevenSignsDbRow>(LOAD_DATA)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(SevenSignsRow::from).collect())
    }

    /// The row of one character, `None` when it never registered.
    pub async fn load_player(&self, char_id: i32) -> Result<Option<SevenSignsRow>> {
        let row = sqlx::query_as::<_, SevenSignsDbRow>(LOAD_PLAYER)
            .bind(char_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(SevenSignsRow::from))
    }

    /// The status row; `None` when the table has no `id = 0` row (Java leaves its defaults).
    // Java: SevenSigns.restoreSevenSignsData (LOAD_STATUS, `while (rs.next())`: the last row wins)
    pub async fn load_status(&self) -> Result<Option<SevenSignsStatusRow>> {
        let rows = sqlx::query_as::<_, SevenSignsStatusDbRow>(LOAD_STATUS)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().last().map(SevenSignsStatusRow::from))
    }

    /// A new member (only the three columns, the others keep their defaults).
    // Java: SevenSigns.setPlayerInfo (INSERT_PLAYER)
    pub async fn insert_player(&self, char_id: i32, cabal: &str, seal: i32) -> Result<()> {
        sqlx::query(INSERT_PLAYER)
            .bind(char_id)
            .bind(cabal)
            .bind(seal)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// One member's row.
    // Java: SevenSigns.saveSevenSignsData(objectId) (UPDATE_PLAYER)
    pub async fn update_player(&self, row: &SevenSignsRow) -> Result<()> {
        bind_update(sqlx::query(UPDATE_PLAYER), row)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Every member's row. Java sends one batch; one transaction writes the same rows.
    // Java: SevenSigns.saveSevenSignsData() (UPDATE_PLAYER, addBatch)
    pub async fn update_all_players(&self, rows: &[SevenSignsRow]) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        for row in rows {
            bind_update(sqlx::query(UPDATE_PLAYER), row)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// The status row.
    // Java: SevenSigns.saveSevenSignsStatus (UPDATE_STATUS)
    pub async fn update_status(&self, s: &SevenSignsStatusRow) -> Result<()> {
        let mut q = sqlx::query(UPDATE_STATUS)
            .bind(s.current_cycle)
            .bind(s.active_period)
            .bind(s.previous_winner)
            .bind(s.dawn_stone_score)
            .bind(s.dawn_festival_score)
            .bind(s.dusk_stone_score)
            .bind(s.dusk_festival_score)
            .bind(s.avarice_owner)
            .bind(s.gnosis_owner)
            .bind(s.strife_owner)
            .bind(s.avarice_dawn_score)
            .bind(s.gnosis_dawn_score)
            .bind(s.strife_dawn_score)
            .bind(s.avarice_dusk_score)
            .bind(s.gnosis_dusk_score)
            .bind(s.strife_dusk_score)
            .bind(s.festival_cycle);
        for bonus in s.accumulated_bonus {
            q = q.bind(bonus);
        }
        q.bind(s.date).execute(&self.pool).await?;
        Ok(())
    }
}

fn bind_update<'q>(
    q: sqlx::query::Query<'q, sqlx::MySql, sqlx::mysql::MySqlArguments>,
    row: &'q SevenSignsRow,
) -> sqlx::query::Query<'q, sqlx::MySql, sqlx::mysql::MySqlArguments> {
    q.bind(row.cabal.as_str())
        .bind(row.seal)
        .bind(row.red_stones)
        .bind(row.green_stones)
        .bind(row.blue_stones)
        .bind(row.ancient_adena_amount)
        .bind(row.contribution_score)
        .bind(row.char_id)
}
