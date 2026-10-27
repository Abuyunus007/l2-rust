//! The Olympiad tables: `olympiad_data` (the period), `olympiad_nobles` (the points of the
//! current period), `olympiad_nobles_eom` (the copy of the last period) and `olympiad_fights`
//! (one row per decided match), plus the `character_variables` rows of the unclaimed points
//! (M8-OLY-01).
// Java: mechanics/olympiad/Olympiad.java (load, loadNoblesRank, saveNobleData, saveOlympiadStatus,
//       updateMonthlyData, deleteNobles, getClassLeaderBoard, getLastNobleOlympiadPoints)
// Java: mechanics/olympiad/OlympiadGameNormal.java (saveResults)
//
// Same layout as cursed_weapons.rs: verbatim SQL, one method per statement. D-39, D-53: the Java
// game server writes the same rows; the world decides which writes go out (`CycleDatabaseWrites`).
// Java builds some statements with `OlympiadConfig.OLYMPIAD_MIN_MATCHES` concatenated into the
// text; here the value is a bound parameter (same rows).

use sqlx::Row;

use crate::error::Result;
use crate::pool::Pool;

// Java: Olympiad.OLYMPIAD_LOAD_DATA / OLYMPIAD_SAVE_DATA
const LOAD_DATA: &str = "SELECT current_cycle, period, olympiad_end, validation_end, next_weekly_change FROM olympiad_data WHERE id = 0";
const SAVE_DATA: &str = "INSERT INTO olympiad_data (id, current_cycle, period, olympiad_end, validation_end, next_weekly_change) VALUES (0,?,?,?,?,?) ON DUPLICATE KEY UPDATE current_cycle=?, period=?, olympiad_end=?, validation_end=?, next_weekly_change=?";
// Java: Olympiad.OLYMPIAD_LOAD_NOBLES
const LOAD_NOBLES: &str = "SELECT olympiad_nobles.charId, olympiad_nobles.class_id, characters.char_name, olympiad_nobles.olympiad_points, olympiad_nobles.competitions_done, olympiad_nobles.competitions_won, olympiad_nobles.competitions_lost, olympiad_nobles.competitions_drawn, olympiad_nobles.competitions_done_week, olympiad_nobles.competitions_done_week_classed, olympiad_nobles.competitions_done_week_non_classed, olympiad_nobles.competitions_done_week_team FROM olympiad_nobles, characters WHERE characters.charId = olympiad_nobles.charId";
// Java: Olympiad.OLYMPIAD_SAVE_NOBLES / OLYMPIAD_UPDATE_NOBLES
const SAVE_NOBLE: &str = "INSERT INTO olympiad_nobles (`charId`,`class_id`,`olympiad_points`,`competitions_done`,`competitions_won`,`competitions_lost`,`competitions_drawn`, `competitions_done_week`, `competitions_done_week_classed`, `competitions_done_week_non_classed`, `competitions_done_week_team`) VALUES (?,?,?,?,?,?,?,?,?,?,?)";
const UPDATE_NOBLE: &str = "UPDATE olympiad_nobles SET olympiad_points = ?, competitions_done = ?, competitions_won = ?, competitions_lost = ?, competitions_drawn = ?, competitions_done_week = ?, competitions_done_week_classed = ?, competitions_done_week_non_classed = ?, competitions_done_week_team = ? WHERE charId = ?";
// Java: Olympiad.GET_ALL_CLASSIFIED_NOBLESS
const ALL_CLASSIFIED: &str = "SELECT charId from olympiad_nobles_eom WHERE competitions_done >= ? ORDER BY olympiad_points DESC, competitions_done DESC, competitions_won DESC";
// Java: Olympiad.GET_EACH_CLASS_LEADER(_CURRENT)(_SOULHOUND)
const CLASS_LEADER: &str = "SELECT characters.char_name from olympiad_nobles_eom, characters WHERE characters.charId = olympiad_nobles_eom.charId AND olympiad_nobles_eom.class_id = ? AND olympiad_nobles_eom.competitions_done >= ? ORDER BY olympiad_nobles_eom.olympiad_points DESC, olympiad_nobles_eom.competitions_done DESC, olympiad_nobles_eom.competitions_won DESC LIMIT 10";
const CLASS_LEADER_CURRENT: &str = "SELECT characters.char_name from olympiad_nobles, characters WHERE characters.charId = olympiad_nobles.charId AND olympiad_nobles.class_id = ? AND olympiad_nobles.competitions_done >= ? ORDER BY olympiad_nobles.olympiad_points DESC, olympiad_nobles.competitions_done DESC, olympiad_nobles.competitions_won DESC LIMIT 10";
const CLASS_LEADER_SOULHOUND: &str = "SELECT characters.char_name from olympiad_nobles_eom, characters WHERE characters.charId = olympiad_nobles_eom.charId AND (olympiad_nobles_eom.class_id = ? OR olympiad_nobles_eom.class_id = 133) AND olympiad_nobles_eom.competitions_done >= ? ORDER BY olympiad_nobles_eom.olympiad_points DESC, olympiad_nobles_eom.competitions_done DESC, olympiad_nobles_eom.competitions_won DESC LIMIT 10";
const CLASS_LEADER_CURRENT_SOULHOUND: &str = "SELECT characters.char_name from olympiad_nobles, characters WHERE characters.charId = olympiad_nobles.charId AND (olympiad_nobles.class_id = ? OR olympiad_nobles.class_id = 133) AND olympiad_nobles.competitions_done >= ? ORDER BY olympiad_nobles.olympiad_points DESC, olympiad_nobles.competitions_done DESC, olympiad_nobles.competitions_won DESC LIMIT 10";
// Java: Olympiad.getLastNobleOlympiadPoints
const LAST_POINTS: &str = "SELECT olympiad_points FROM olympiad_nobles_eom WHERE charId = ?";
// Java: Olympiad.OLYMPIAD_DELETE_ALL / OLYMPIAD_MONTH_CLEAR / OLYMPIAD_MONTH_CREATE
const DELETE_ALL: &str = "TRUNCATE olympiad_nobles";
const MONTH_CLEAR: &str = "TRUNCATE olympiad_nobles_eom";
const MONTH_CREATE: &str = "INSERT INTO olympiad_nobles_eom SELECT charId, class_id, olympiad_points, competitions_done, competitions_won, competitions_lost, competitions_drawn FROM olympiad_nobles";
// Java: OlympiadGameNormal.saveResults
const INSERT_FIGHT: &str = "INSERT INTO olympiad_fights (charOneId, charTwoId, charOneClass, charTwoClass, winner, start, time, classed) values(?,?,?,?,?,?,?,?)";
// Java: Olympiad.REMOVE_UNCLAIMED_POINTS / INSERT_UNCLAIMED_POINTS
const REMOVE_UNCLAIMED: &str = "DELETE FROM character_variables WHERE charId=? AND var=?";
const INSERT_UNCLAIMED: &str = "INSERT INTO character_variables (charId, var, val) VALUES (?, ?, ?)";
// The value `OlympiadManagerLink` reads with `player.getVariables().getInt(...)`.
const SELECT_VARIABLE: &str = "SELECT val FROM character_variables WHERE charId = ? AND var = ?";

/// Java `PlayerClass.MALE_SOUL_HOUND.getId()`: its board also lists the female soul hounds (133).
pub const MALE_SOUL_HOUND: i32 = 132;

/// The `olympiad_data` row (id 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OlympiadDataRow {
    pub current_cycle: i32,
    pub period: i32,
    pub olympiad_end: i64,
    pub validation_end: i64,
    pub next_weekly_change: i64,
}

/// One `olympiad_nobles` row with the character's name (Java `OLYMPIAD_LOAD_NOBLES`; for a save
/// the name is not written).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OlympiadNobleRow {
    pub char_id: i32,
    pub class_id: i32,
    pub char_name: String,
    pub olympiad_points: i32,
    pub competitions_done: i32,
    pub competitions_won: i32,
    pub competitions_lost: i32,
    pub competitions_drawn: i32,
    pub competitions_done_week: i32,
    pub competitions_done_week_classed: i32,
    pub competitions_done_week_non_classed: i32,
    pub competitions_done_week_team: i32,
}

/// One `olympiad_fights` row (Java `saveResults`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OlympiadFightRow {
    pub char_one_id: i32,
    pub char_two_id: i32,
    pub char_one_class: i32,
    pub char_two_class: i32,
    /// 0 tie, 1 or 2 the winning side.
    pub winner: i32,
    /// Unix milliseconds.
    pub start: i64,
    /// Milliseconds the fight took.
    pub time: i64,
    pub classed: bool,
}

fn int(row: &sqlx::mysql::MySqlRow, column: &str) -> Result<i32> {
    // The columns are unsigned of several widths; read wide and clamp like `rs.getInt`.
    let value: u64 = row.try_get::<u64, _>(column)?;
    Ok(i32::try_from(value).unwrap_or(i32::MAX))
}

fn long(row: &sqlx::mysql::MySqlRow, column: &str) -> Result<i64> {
    let value: u64 = row.try_get::<u64, _>(column)?;
    Ok(i64::try_from(value).unwrap_or(i64::MAX))
}

/// Every statement of the Olympiad tables.
#[derive(Debug, Clone)]
pub struct OlympiadRepository {
    pool: Pool,
}

impl OlympiadRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// The `olympiad_data` row, `None` without one (Java then starts cycle 1, period 0).
    pub async fn load_data(&self) -> Result<Option<OlympiadDataRow>> {
        let rows = sqlx::query(LOAD_DATA).fetch_all(&self.pool).await?;
        // Java's `while (rset.next())` keeps the last row.
        let mut result = None;
        for row in &rows {
            result = Some(OlympiadDataRow {
                current_cycle: int(row, "current_cycle")?,
                period: int(row, "period")?,
                olympiad_end: long(row, "olympiad_end")?,
                validation_end: long(row, "validation_end")?,
                next_weekly_change: long(row, "next_weekly_change")?,
            });
        }
        Ok(result)
    }

    /// Java `saveOlympiadStatus`'s `OLYMPIAD_SAVE_DATA`.
    pub async fn save_data(&self, data: &OlympiadDataRow) -> Result<()> {
        sqlx::query(SAVE_DATA)
            .bind(data.current_cycle)
            .bind(data.period)
            .bind(data.olympiad_end)
            .bind(data.validation_end)
            .bind(data.next_weekly_change)
            .bind(data.current_cycle)
            .bind(data.period)
            .bind(data.olympiad_end)
            .bind(data.validation_end)
            .bind(data.next_weekly_change)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Every noble with its character's name (Java `OLYMPIAD_LOAD_NOBLES`).
    pub async fn load_nobles(&self) -> Result<Vec<OlympiadNobleRow>> {
        let rows = sqlx::query(LOAD_NOBLES).fetch_all(&self.pool).await?;
        let mut nobles = Vec::with_capacity(rows.len());
        for row in &rows {
            nobles.push(OlympiadNobleRow {
                char_id: int(row, "charId")?,
                class_id: int(row, "class_id")?,
                char_name: row.try_get::<String, _>("char_name")?,
                olympiad_points: int(row, "olympiad_points")?,
                competitions_done: int(row, "competitions_done")?,
                competitions_won: int(row, "competitions_won")?,
                competitions_lost: int(row, "competitions_lost")?,
                competitions_drawn: int(row, "competitions_drawn")?,
                competitions_done_week: int(row, "competitions_done_week")?,
                competitions_done_week_classed: int(row, "competitions_done_week_classed")?,
                competitions_done_week_non_classed: int(row, "competitions_done_week_non_classed")?,
                competitions_done_week_team: int(row, "competitions_done_week_team")?,
            });
        }
        Ok(nobles)
    }

    /// Java `saveNobleData` for one noble: the `INSERT` of a new noble (`to_save`) or the
    /// `UPDATE` of a known one.
    pub async fn save_noble(&self, noble: &OlympiadNobleRow, insert: bool) -> Result<()> {
        if insert {
            sqlx::query(SAVE_NOBLE)
                .bind(noble.char_id)
                .bind(noble.class_id)
                .bind(noble.olympiad_points)
                .bind(noble.competitions_done)
                .bind(noble.competitions_won)
                .bind(noble.competitions_lost)
                .bind(noble.competitions_drawn)
                .bind(noble.competitions_done_week)
                .bind(noble.competitions_done_week_classed)
                .bind(noble.competitions_done_week_non_classed)
                .bind(noble.competitions_done_week_team)
                .execute(&self.pool)
                .await?;
        } else {
            sqlx::query(UPDATE_NOBLE)
                .bind(noble.olympiad_points)
                .bind(noble.competitions_done)
                .bind(noble.competitions_won)
                .bind(noble.competitions_lost)
                .bind(noble.competitions_drawn)
                .bind(noble.competitions_done_week)
                .bind(noble.competitions_done_week_classed)
                .bind(noble.competitions_done_week_non_classed)
                .bind(noble.competitions_done_week_team)
                .bind(noble.char_id)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    /// Java `deleteNobles`: `TRUNCATE olympiad_nobles`.
    pub async fn delete_nobles(&self) -> Result<()> {
        sqlx::query(DELETE_ALL).execute(&self.pool).await?;
        Ok(())
    }

    /// Java `updateMonthlyData`: the last period's copy is replaced by the current table.
    pub async fn update_monthly_data(&self) -> Result<()> {
        sqlx::query(MONTH_CLEAR).execute(&self.pool).await?;
        sqlx::query(MONTH_CREATE).execute(&self.pool).await?;
        Ok(())
    }

    /// Java `GET_ALL_CLASSIFIED_NOBLESS`: the char ids of the last period with at least
    /// `min_matches` matches, best first.
    pub async fn classified_nobles(&self, min_matches: i32) -> Result<Vec<i32>> {
        let rows = sqlx::query(ALL_CLASSIFIED)
            .bind(min_matches)
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(|row| int(row, "charId")).collect()
    }

    /// Java `getClassLeaderBoard(classId)`: up to ten names, of the last period's copy when
    /// `monthly` (`OlympiadShowMonthlyWinners`), else of the current table.
    pub async fn class_leader_board(&self, class_id: i32, monthly: bool, min_matches: i32) -> Result<Vec<String>> {
        let soul_hound = class_id == MALE_SOUL_HOUND;
        let query = match (monthly, soul_hound) {
            (true, true) => CLASS_LEADER_SOULHOUND,
            (true, false) => CLASS_LEADER,
            (false, true) => CLASS_LEADER_CURRENT_SOULHOUND,
            (false, false) => CLASS_LEADER_CURRENT,
        };
        let rows = sqlx::query(query)
            .bind(class_id)
            .bind(min_matches)
            .fetch_all(&self.pool)
            .await?;
        rows.iter()
            .map(|row| row.try_get::<String, _>("char_name").map_err(Into::into))
            .collect()
    }

    /// Java `getLastNobleOlympiadPoints`: the points of the last period, 0 without a row.
    pub async fn last_noble_points(&self, char_id: i32) -> Result<i32> {
        let row = sqlx::query(LAST_POINTS)
            .bind(char_id)
            .fetch_optional(&self.pool)
            .await?;
        match row {
            Some(row) => int(&row, "olympiad_points"),
            None => Ok(0),
        }
    }

    /// Java `saveResults`.
    pub async fn insert_fight(&self, fight: &OlympiadFightRow) -> Result<()> {
        sqlx::query(INSERT_FIGHT)
            .bind(fight.char_one_id)
            .bind(fight.char_two_id)
            .bind(fight.char_one_class)
            .bind(fight.char_two_class)
            .bind(fight.winner)
            .bind(fight.start)
            .bind(fight.time)
            .bind(i32::from(fight.classed))
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Java `loadNoblesRank` for a noble who is offline: the old unclaimed points row goes, the
    /// new value is inserted. Two statements, like Java (each logs its own failure there).
    pub async fn store_unclaimed_points(&self, char_id: i32, var: &str, points: i32) -> Result<()> {
        sqlx::query(REMOVE_UNCLAIMED)
            .bind(char_id)
            .bind(var)
            .execute(&self.pool)
            .await?;
        sqlx::query(INSERT_UNCLAIMED)
            .bind(char_id)
            .bind(var)
            .bind(points.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// The value of one character variable (`None` without the row). The world reads the
    /// unclaimed points again before it hands the passes out (D-50).
    pub async fn variable(&self, char_id: i32, var: &str) -> Result<Option<String>> {
        let row = sqlx::query(SELECT_VARIABLE)
            .bind(char_id)
            .bind(var)
            .fetch_optional(&self.pool)
            .await?;
        match row {
            Some(row) => Ok(Some(row.try_get::<String, _>("val")?)),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_statements_have_one_placeholder_per_value() {
        assert_eq!(SAVE_DATA.matches('?').count(), 10);
        assert_eq!(SAVE_NOBLE.matches('?').count(), 11);
        assert_eq!(UPDATE_NOBLE.matches('?').count(), 10);
        assert_eq!(ALL_CLASSIFIED.matches('?').count(), 1);
        for query in [
            CLASS_LEADER,
            CLASS_LEADER_CURRENT,
            CLASS_LEADER_SOULHOUND,
            CLASS_LEADER_CURRENT_SOULHOUND,
        ] {
            assert_eq!(query.matches('?').count(), 2);
        }
        assert_eq!(LAST_POINTS.matches('?').count(), 1);
        assert_eq!(INSERT_FIGHT.matches('?').count(), 8);
        assert_eq!(REMOVE_UNCLAIMED.matches('?').count(), 2);
        assert_eq!(INSERT_UNCLAIMED.matches('?').count(), 3);
    }
}
