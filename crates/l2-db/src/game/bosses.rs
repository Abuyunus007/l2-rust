//! The boss tables: `grandboss_data` (the state of each grand boss), `grandboss_list` (the players
//! a boss zone lets in), `raidboss_spawnlist` (the raid bosses with their respawn time) and
//! `character_raid_points` (the raid points of each player).
// Java: managers/GrandBossManager.java (init, initZones, storeMe, updateDb)
// Java: managers/RaidBossSpawnManager.java (load, addNewSpawn, deleteSpawn, updateDb)
// Java: managers/RaidBossPointsManager.java (init, updatePointsInDB, cleanUp)
//
// D-39: the Java game server reads and rewrites the same rows. Which of the writes below the
// server really sends is decided in the world (D-53: the boss states and the respawn times only
// with `CycleDatabaseWrites`; the raid points of a player as Java writes them).

use sqlx::FromRow;

use super::sql_number::SqlNumber;
use crate::error::Result;
use crate::pool::Pool;

// Java: GrandBossManager.init. The decimals are read as text (`SqlNumber`), like `getDouble`.
const SELECT_GRAND_BOSSES: &str = "SELECT * from grandboss_data ORDER BY boss_id";
// Rust-only (D-50): one boss again, for the page `//grandboss <id>` shows.
const SELECT_GRAND_BOSS: &str = "SELECT * from grandboss_data WHERE boss_id = ?";
// Java: GrandBossManager.initZones
const SELECT_GRAND_BOSS_LIST: &str = "SELECT * from grandboss_list ORDER BY player_id";
// Java: GrandBossManager.DELETE_GRAND_BOSS_LIST, INSERT_GRAND_BOSS_LIST, UPDATE_GRAND_BOSS_DATA,
//       UPDATE_GRAND_BOSS_DATA2
const DELETE_GRAND_BOSS_LIST: &str = "DELETE FROM grandboss_list";
const INSERT_GRAND_BOSS_LIST: &str = "INSERT INTO grandboss_list (player_id,zone) VALUES (?,?)";
const UPDATE_GRAND_BOSS_DATA: &str = "UPDATE grandboss_data set loc_x = ?, loc_y = ?, loc_z = ?, heading = ?, respawn_time = ?, currentHP = ?, currentMP = ?, status = ? where boss_id = ?";
const UPDATE_GRAND_BOSS_DATA2: &str = "UPDATE grandboss_data set status = ? where boss_id = ?";

// Java: RaidBossSpawnManager.load
const SELECT_RAID_BOSSES: &str = "SELECT * FROM raidboss_spawnlist ORDER BY boss_id";
// Java: RaidBossSpawnManager.addNewSpawn, deleteSpawn, updateDb
const INSERT_RAID_BOSS: &str = "INSERT INTO raidboss_spawnlist (boss_id,amount,loc_x,loc_y,loc_z,heading,respawn_time,currentHp,currentMp) VALUES(?,?,?,?,?,?,?,?,?)";
const DELETE_RAID_BOSS: &str = "DELETE FROM raidboss_spawnlist WHERE boss_id=?";
const UPDATE_RAID_BOSS: &str =
    "UPDATE raidboss_spawnlist SET respawn_time = ?, currentHP = ?, currentMP = ? WHERE boss_id = ?";

// Java: RaidBossPointsManager.init
const SELECT_RAID_POINTS: &str = "SELECT `charId`,`boss_id`,`points` FROM `character_raid_points`";
// Rust-only (D-50): the rows of one character, read again when it comes from the Java server.
const SELECT_RAID_POINTS_OF: &str =
    "SELECT `charId`,`boss_id`,`points` FROM `character_raid_points` WHERE `charId` = ?";
// Java: RaidBossPointsManager.updatePointsInDB, cleanUp
const REPLACE_RAID_POINTS: &str =
    "REPLACE INTO character_raid_points (`charId`,`boss_id`,`points`) VALUES (?,?,?)";
const DELETE_RAID_POINTS: &str = "DELETE from character_raid_points WHERE charId > 0";

/// A `grandboss_data` row.
#[derive(Debug, Clone, PartialEq)]
pub struct GrandBossRow {
    pub boss_id: i32,
    pub loc_x: i32,
    pub loc_y: i32,
    pub loc_z: i32,
    pub heading: i32,
    /// `getLong("respawn_time")`, epoch milliseconds.
    pub respawn_time: i64,
    pub current_hp: f64,
    pub current_mp: f64,
    pub status: i32,
}

#[derive(Debug, Clone, FromRow)]
struct GrandBossDbRow {
    boss_id: u16,
    loc_x: i32,
    loc_y: i32,
    loc_z: i32,
    heading: i32,
    respawn_time: u64,
    #[sqlx(rename = "currentHP")]
    current_hp: SqlNumber<f64>,
    #[sqlx(rename = "currentMP")]
    current_mp: SqlNumber<f64>,
    status: u8,
}

impl From<GrandBossDbRow> for GrandBossRow {
    fn from(r: GrandBossDbRow) -> Self {
        Self {
            boss_id: i32::from(r.boss_id),
            loc_x: r.loc_x,
            loc_y: r.loc_y,
            loc_z: r.loc_z,
            heading: r.heading,
            respawn_time: i64::try_from(r.respawn_time).unwrap_or(i64::MAX),
            current_hp: r.current_hp.0,
            current_mp: r.current_mp.0,
            status: i32::from(r.status),
        }
    }
}

/// A `grandboss_list` row: a player a boss zone lets in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GrandBossListRow {
    pub player_id: i32,
    pub zone: i32,
}

#[derive(Debug, Clone, FromRow)]
struct GrandBossListDbRow {
    player_id: SqlNumber<i32>,
    zone: SqlNumber<i32>,
}

/// A `raidboss_spawnlist` row.
#[derive(Debug, Clone, PartialEq)]
pub struct RaidBossRow {
    pub boss_id: i32,
    pub amount: i32,
    pub loc_x: i32,
    pub loc_y: i32,
    pub loc_z: i32,
    pub heading: i32,
    pub respawn_delay: i32,
    pub respawn_random: i32,
    /// `getLong("respawn_time")`, epoch milliseconds; 0 for a boss that is alive.
    pub respawn_time: i64,
    /// `getDouble("currentHP")`: a NULL reads as 0.
    pub current_hp: f64,
    /// `getDouble("currentMP")`: a NULL reads as 0.
    pub current_mp: f64,
}

#[derive(Debug, Clone, FromRow)]
struct RaidBossDbRow {
    boss_id: u16,
    amount: u8,
    loc_x: i32,
    loc_y: i32,
    loc_z: i32,
    heading: i32,
    respawn_delay: u32,
    respawn_random: u32,
    respawn_time: u64,
    #[sqlx(rename = "currentHp")]
    current_hp: Option<SqlNumber<f64>>,
    #[sqlx(rename = "currentMp")]
    current_mp: Option<SqlNumber<f64>>,
}

impl From<RaidBossDbRow> for RaidBossRow {
    fn from(r: RaidBossDbRow) -> Self {
        Self {
            boss_id: i32::from(r.boss_id),
            amount: i32::from(r.amount),
            loc_x: r.loc_x,
            loc_y: r.loc_y,
            loc_z: r.loc_z,
            heading: r.heading,
            respawn_delay: i32::try_from(r.respawn_delay).unwrap_or(i32::MAX),
            respawn_random: i32::try_from(r.respawn_random).unwrap_or(i32::MAX),
            respawn_time: i64::try_from(r.respawn_time).unwrap_or(i64::MAX),
            current_hp: r.current_hp.map_or(0.0, |v| v.0),
            current_mp: r.current_mp.map_or(0.0, |v| v.0),
        }
    }
}

/// A `character_raid_points` row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RaidPointsRow {
    pub char_id: i32,
    pub boss_id: i32,
    pub points: i32,
}

#[derive(Debug, Clone, FromRow)]
struct RaidPointsDbRow {
    #[sqlx(rename = "charId")]
    char_id: u32,
    boss_id: u32,
    points: u32,
}

impl From<RaidPointsDbRow> for RaidPointsRow {
    fn from(r: RaidPointsDbRow) -> Self {
        // `getInt` of an unsigned column.
        Self {
            char_id: r.char_id as i32,
            boss_id: r.boss_id as i32,
            points: r.points as i32,
        }
    }
}

/// The values of `UPDATE_GRAND_BOSS_DATA`.
#[derive(Debug, Clone, PartialEq)]
pub struct GrandBossUpdate {
    pub boss_id: i32,
    pub loc_x: i32,
    pub loc_y: i32,
    pub loc_z: i32,
    pub heading: i32,
    pub respawn_time: i64,
    pub current_hp: f64,
    pub current_mp: f64,
    pub status: i32,
}

/// One boss of `storeMe`: the whole row, or the status only (`UPDATE_GRAND_BOSS_DATA2`).
#[derive(Debug, Clone, PartialEq)]
pub enum GrandBossStore {
    Full(GrandBossUpdate),
    StatusOnly { boss_id: i32, status: i32 },
}

/// Reads and writes the boss tables.
#[derive(Clone)]
pub struct BossRepository {
    pool: Pool,
}

impl BossRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Java `GrandBossManager.init`: every state row, ordered by boss id.
    pub async fn grand_bosses(&self) -> Result<Vec<GrandBossRow>> {
        let rows: Vec<GrandBossDbRow> = sqlx::query_as(SELECT_GRAND_BOSSES).fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(GrandBossRow::from).collect())
    }

    /// One state row again (D-50), `None` when the table has no such boss.
    pub async fn grand_boss(&self, boss_id: i32) -> Result<Option<GrandBossRow>> {
        let row: Option<GrandBossDbRow> = sqlx::query_as(SELECT_GRAND_BOSS)
            .bind(boss_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(GrandBossRow::from))
    }

    /// Java `GrandBossManager.initZones`: the players of every boss zone, ordered by player id.
    pub async fn grand_boss_list(&self) -> Result<Vec<GrandBossListRow>> {
        let rows: Vec<GrandBossListDbRow> = sqlx::query_as(SELECT_GRAND_BOSS_LIST)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .into_iter()
            .map(|r| GrandBossListRow {
                player_id: r.player_id.0,
                zone: r.zone.0,
            })
            .collect())
    }

    /// Java `GrandBossManager.updateDb` / `storeMe` with `UPDATE_GRAND_BOSS_DATA2`: the status only.
    pub async fn update_grand_boss_status(&self, boss_id: i32, status: i32) -> Result<()> {
        sqlx::query(UPDATE_GRAND_BOSS_DATA2)
            .bind(status)
            .bind(boss_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Java `GrandBossManager.updateDb` / `storeMe` with `UPDATE_GRAND_BOSS_DATA`: place, respawn
    /// time, HP, MP and status.
    pub async fn update_grand_boss(&self, row: &GrandBossUpdate) -> Result<()> {
        sqlx::query(UPDATE_GRAND_BOSS_DATA)
            .bind(row.loc_x)
            .bind(row.loc_y)
            .bind(row.loc_z)
            .bind(row.heading)
            .bind(row.respawn_time)
            .bind(row.current_hp)
            .bind(row.current_mp)
            .bind(row.status)
            .bind(row.boss_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Java `GrandBossManager.storeMe`: `grandboss_list` is deleted and filled again, then every
    /// boss row is updated.
    /// Java runs the statements one by one; one transaction leaves the same rows and never half.
    pub async fn store_all(&self, list: &[GrandBossListRow], rows: &[GrandBossStore]) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(DELETE_GRAND_BOSS_LIST).execute(&mut *tx).await?;
        for entry in list {
            sqlx::query(INSERT_GRAND_BOSS_LIST)
                .bind(entry.player_id)
                .bind(entry.zone)
                .execute(&mut *tx)
                .await?;
        }
        for entry in rows {
            match entry {
                GrandBossStore::Full(row) => {
                    sqlx::query(UPDATE_GRAND_BOSS_DATA)
                        .bind(row.loc_x)
                        .bind(row.loc_y)
                        .bind(row.loc_z)
                        .bind(row.heading)
                        .bind(row.respawn_time)
                        .bind(row.current_hp)
                        .bind(row.current_mp)
                        .bind(row.status)
                        .bind(row.boss_id)
                        .execute(&mut *tx)
                        .await?;
                }
                GrandBossStore::StatusOnly { boss_id, status } => {
                    sqlx::query(UPDATE_GRAND_BOSS_DATA2)
                        .bind(status)
                        .bind(boss_id)
                        .execute(&mut *tx)
                        .await?;
                }
            }
        }
        tx.commit().await?;
        Ok(())
    }

    /// Java `RaidBossSpawnManager.load`: every raid boss, ordered by boss id.
    pub async fn raid_bosses(&self) -> Result<Vec<RaidBossRow>> {
        let rows: Vec<RaidBossDbRow> = sqlx::query_as(SELECT_RAID_BOSSES).fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(RaidBossRow::from).collect())
    }

    /// Java `RaidBossSpawnManager.addNewSpawn` with `storeInDb`. The delay columns keep their
    /// defaults, like Java's statement.
    pub async fn insert_raid_boss(&self, row: &RaidBossRow) -> Result<()> {
        sqlx::query(INSERT_RAID_BOSS)
            .bind(row.boss_id)
            .bind(row.amount)
            .bind(row.loc_x)
            .bind(row.loc_y)
            .bind(row.loc_z)
            .bind(row.heading)
            .bind(row.respawn_time)
            .bind(row.current_hp)
            .bind(row.current_mp)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Java `RaidBossSpawnManager.deleteSpawn` with `updateDb`.
    pub async fn delete_raid_boss(&self, boss_id: i32) -> Result<()> {
        sqlx::query(DELETE_RAID_BOSS)
            .bind(boss_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Java `RaidBossSpawnManager.updateDb`: one row's respawn time, HP and MP.
    pub async fn update_raid_boss(
        &self,
        boss_id: i32,
        respawn_time: i64,
        current_hp: f64,
        current_mp: f64,
    ) -> Result<()> {
        sqlx::query(UPDATE_RAID_BOSS)
            .bind(respawn_time)
            .bind(current_hp)
            .bind(current_mp)
            .bind(boss_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Java `RaidBossPointsManager.init`: every row.
    pub async fn raid_points(&self) -> Result<Vec<RaidPointsRow>> {
        let rows: Vec<RaidPointsDbRow> = sqlx::query_as(SELECT_RAID_POINTS).fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(RaidPointsRow::from).collect())
    }

    /// The rows of one character (D-50).
    pub async fn raid_points_of(&self, char_id: i32) -> Result<Vec<RaidPointsRow>> {
        let rows: Vec<RaidPointsDbRow> = sqlx::query_as(SELECT_RAID_POINTS_OF)
            .bind(char_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(RaidPointsRow::from).collect())
    }

    /// Java `RaidBossPointsManager.updatePointsInDB`: `REPLACE` of the new total.
    pub async fn replace_raid_points(&self, char_id: i32, boss_id: i32, points: i32) -> Result<()> {
        sqlx::query(REPLACE_RAID_POINTS)
            .bind(char_id)
            .bind(boss_id)
            .bind(points)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Java `RaidBossPointsManager.cleanUp`: the weekly reset deletes every row.
    pub async fn delete_all_raid_points(&self) -> Result<()> {
        sqlx::query(DELETE_RAID_POINTS).execute(&self.pool).await?;
        Ok(())
    }
}
