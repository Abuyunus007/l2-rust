//! The hero tables: `heroes` (the heroes of the last period and everybody who was one) and
//! `heroes_diary` (the deeds of a hero), plus the reads of `olympiad_fights` the hero pages show
//! (M8-HERO-01).
// Java: mechanics/olympiad/Hero.java (init, loadMessage, loadDiary, loadFights, updateHeroes,
//       setDiaryData, saveHeroMessage, deleteItemsInDb)
//
// Same layout as olympiad.rs: verbatim SQL, one method per statement. D-39, D-53: the Java game
// server writes the same rows; the world decides which writes go out (`CycleDatabaseWrites`).

use sqlx::Row;

use super::sql_number::SqlNumber;
use crate::error::Result;
use crate::pool::Pool;

// Java: Hero.GET_ALL_HEROES (the played ones are the rows with `played = 1`, `GET_HEROES`)
const GET_ALL_HEROES: &str = "SELECT heroes.charId, characters.char_name, heroes.class_id, heroes.count, heroes.played, heroes.claimed FROM heroes, characters WHERE characters.charId = heroes.charId";
// Java: Hero.GET_CLAN_ALLY
const GET_CLAN_ALLY: &str = "SELECT characters.clanid AS clanid, coalesce(clan_data.ally_Id, 0) AS allyId FROM characters LEFT JOIN clan_data ON clan_data.clan_id = characters.clanid WHERE characters.charId = ?";
// Java: Hero.UPDATE_ALL / INSERT_HERO / UPDATE_HERO / DELETE_ITEMS
const UPDATE_ALL: &str = "UPDATE heroes SET played = 0";
const INSERT_HERO: &str = "INSERT INTO heroes (charId, class_id, count, played, claimed) VALUES (?,?,?,?,?)";
const UPDATE_HERO: &str =
    "UPDATE heroes SET class_id = ?, count = ?, played = ?, claimed = ? WHERE charId = ?";
const DELETE_ITEMS: &str = "DELETE FROM items WHERE item_id IN (6842, 6611, 6612, 6613, 6614, 6615, 6616, 6617, 6618, 6619, 6620, 6621, 9388, 9389, 9390) AND owner_id NOT IN (SELECT charId FROM characters WHERE accesslevel > 0)";
// Java: Hero.loadMessage / saveHeroMessage
const LOAD_MESSAGE: &str = "SELECT message FROM heroes WHERE charId=?";
const SAVE_MESSAGE: &str = "UPDATE heroes SET message=? WHERE charId=?;";
// Java: Hero.loadDiary / setDiaryData
const LOAD_DIARY: &str = "SELECT * FROM  heroes_diary WHERE charId=? ORDER BY time ASC";
const INSERT_DIARY: &str = "INSERT INTO heroes_diary (charId, time, action, param) values(?,?,?,?)";
// Java: Hero.loadFights
const LOAD_FIGHTS: &str =
    "SELECT * FROM olympiad_fights WHERE (charOneId=? OR charTwoId=?) AND start<? ORDER BY start ASC";
// Java: CharInfoTable.getNameById (its table is `SELECT charId, char_name, accesslevel FROM
// characters`)
const CHAR_NAME: &str = "SELECT char_name FROM characters WHERE charId=?";

/// One `heroes` row with the character's name and clan (Java `GET_ALL_HEROES` and
/// `GET_CLAN_ALLY`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HeroDbRow {
    pub char_id: i32,
    pub char_name: String,
    pub class_id: i32,
    pub count: i32,
    pub played: i32,
    /// `Boolean.parseBoolean(claimed)`.
    pub claimed: bool,
    /// `characters.clanid` (0 without a clan).
    pub clan_id: i32,
    /// `coalesce(clan_data.ally_Id, 0)`.
    pub ally_id: i32,
}

/// One `heroes_diary` row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeroDiaryRow {
    /// Unix milliseconds.
    pub time: i64,
    pub action: i32,
    pub param: i32,
}

/// One `olympiad_fights` row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeroFightRow {
    pub char_one_id: i32,
    pub char_one_class: i32,
    pub char_two_id: i32,
    pub char_two_class: i32,
    pub winner: i32,
    pub start: i64,
    pub time: i64,
    pub classed: i32,
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

/// Every statement of the hero tables.
#[derive(Debug, Clone)]
pub struct HeroRepository {
    pool: Pool,
}

impl HeroRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Every hero who ever was one, with the clan and alliance ids of the character
    /// (`GET_ALL_HEROES`, then `GET_CLAN_ALLY` for each).
    pub async fn all_heroes(&self) -> Result<Vec<HeroDbRow>> {
        let rows = sqlx::query(GET_ALL_HEROES).fetch_all(&self.pool).await?;
        let mut heroes = Vec::with_capacity(rows.len());
        for row in &rows {
            let mut hero = HeroDbRow {
                char_id: int(row, "charId")?,
                char_name: row.try_get::<String, _>("char_name")?,
                class_id: row.try_get::<SqlNumber<i32>, _>("class_id")?.0,
                count: row.try_get::<SqlNumber<i32>, _>("count")?.0,
                played: row.try_get::<SqlNumber<i32>, _>("played")?.0,
                // `Boolean.parseBoolean`: "true" in any case.
                claimed: row.try_get::<String, _>("claimed")?.eq_ignore_ascii_case("true"),
                clan_id: 0,
                ally_id: 0,
            };
            if let Some(clan) = sqlx::query(GET_CLAN_ALLY)
                .bind(hero.char_id)
                .fetch_optional(&self.pool)
                .await?
            {
                // `rs.getInt` of a NULL (a character without a clan) is 0.
                hero.clan_id = clan
                    .try_get::<Option<u32>, _>("clanid")?
                    .map_or(0, |v| i32::try_from(v).unwrap_or(i32::MAX));
                hero.ally_id = int(&clan, "allyId")?;
            }
            heroes.push(hero);
        }
        Ok(heroes)
    }

    /// Java `loadMessage`: the message of a hero, `None` without a row.
    pub async fn message(&self, char_id: i32) -> Result<Option<String>> {
        let row = sqlx::query(LOAD_MESSAGE)
            .bind(char_id)
            .fetch_optional(&self.pool)
            .await?;
        match row {
            Some(row) => Ok(Some(row.try_get::<String, _>("message")?)),
            None => Ok(None),
        }
    }

    /// Java `loadDiary`: the deeds of a hero, oldest first.
    pub async fn diary(&self, char_id: i32) -> Result<Vec<HeroDiaryRow>> {
        let rows = sqlx::query(LOAD_DIARY)
            .bind(char_id)
            .fetch_all(&self.pool)
            .await?;
        let mut diary = Vec::with_capacity(rows.len());
        for row in &rows {
            diary.push(HeroDiaryRow {
                time: long(row, "time")?,
                action: int(row, "action")?,
                param: int(row, "param")?,
            });
        }
        Ok(diary)
    }

    /// Java `loadFights`: the matches of the character that started before `before` (unix
    /// milliseconds), oldest first.
    pub async fn fights(&self, char_id: i32, before: i64) -> Result<Vec<HeroFightRow>> {
        let rows = sqlx::query(LOAD_FIGHTS)
            .bind(char_id)
            .bind(char_id)
            .bind(before)
            .fetch_all(&self.pool)
            .await?;
        let mut fights = Vec::with_capacity(rows.len());
        for row in &rows {
            fights.push(HeroFightRow {
                char_one_id: int(row, "charOneId")?,
                char_one_class: int(row, "charOneClass")?,
                char_two_id: int(row, "charTwoId")?,
                char_two_class: int(row, "charTwoClass")?,
                winner: int(row, "winner")?,
                start: long(row, "start")?,
                time: long(row, "time")?,
                classed: int(row, "classed")?,
            });
        }
        Ok(fights)
    }

    /// Java `CharInfoTable.getNameById`: `None` for an unknown character.
    pub async fn char_name(&self, char_id: i32) -> Result<Option<String>> {
        let row = sqlx::query(CHAR_NAME)
            .bind(char_id)
            .fetch_optional(&self.pool)
            .await?;
        match row {
            Some(row) => Ok(Some(row.try_get::<String, _>("char_name")?)),
            None => Ok(None),
        }
    }

    /// Java `updateHeroes(true)`: `UPDATE heroes SET played = 0`.
    pub async fn reset_played(&self) -> Result<()> {
        sqlx::query(UPDATE_ALL).execute(&self.pool).await?;
        Ok(())
    }

    /// Java `updateHeroes(false)` for a hero `COMPLETE_HEROS` does not know.
    pub async fn insert_hero(
        &self,
        char_id: i32,
        class_id: i32,
        count: i32,
        played: i32,
        claimed: bool,
    ) -> Result<()> {
        sqlx::query(INSERT_HERO)
            .bind(char_id)
            .bind(class_id)
            .bind(count)
            .bind(played)
            .bind(claimed.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Java `updateHeroes(false)` for a known hero.
    pub async fn update_hero(
        &self,
        char_id: i32,
        class_id: i32,
        count: i32,
        played: i32,
        claimed: bool,
    ) -> Result<()> {
        sqlx::query(UPDATE_HERO)
            .bind(class_id)
            .bind(count)
            .bind(played)
            .bind(claimed.to_string())
            .bind(char_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Java `deleteItemsInDb`: the hero weapons and circlets of everybody but the GMs.
    pub async fn delete_hero_items(&self) -> Result<()> {
        sqlx::query(DELETE_ITEMS).execute(&self.pool).await?;
        Ok(())
    }

    /// Java `setDiaryData`.
    pub async fn insert_diary(&self, char_id: i32, time: i64, action: i32, param: i32) -> Result<()> {
        sqlx::query(INSERT_DIARY)
            .bind(char_id)
            .bind(time)
            .bind(action)
            .bind(param)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Java `saveHeroMessage`.
    pub async fn save_message(&self, char_id: i32, message: &str) -> Result<()> {
        sqlx::query(SAVE_MESSAGE)
            .bind(message)
            .bind(char_id)
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
        assert_eq!(INSERT_HERO.matches('?').count(), 5);
        assert_eq!(UPDATE_HERO.matches('?').count(), 5);
        assert_eq!(GET_CLAN_ALLY.matches('?').count(), 1);
        assert_eq!(LOAD_MESSAGE.matches('?').count(), 1);
        assert_eq!(SAVE_MESSAGE.matches('?').count(), 2);
        assert_eq!(LOAD_DIARY.matches('?').count(), 1);
        assert_eq!(INSERT_DIARY.matches('?').count(), 4);
        assert_eq!(LOAD_FIGHTS.matches('?').count(), 3);
        assert_eq!(CHAR_NAME.matches('?').count(), 1);
        assert_eq!(UPDATE_ALL.matches('?').count(), 0);
    }

    #[test]
    fn the_hero_item_list_is_javas() {
        // The circlet and the 14 weapons of `MonumentOfHeroes`.
        for id in [
            6842, 6611, 6612, 6613, 6614, 6615, 6616, 6617, 6618, 6619, 6620, 6621, 9388, 9389, 9390,
        ] {
            assert!(DELETE_ITEMS.contains(&format!("{id}")), "{id}");
        }
    }
}
