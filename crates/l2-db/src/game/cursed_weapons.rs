//! The `cursed_weapons` table, and the two statements a cursed weapon's end runs on a character
//! that is offline (M8-CURSED-01).
// Java: managers/CursedWeaponsManager.java (restore, controlPlayers, removeFromDb)
// Java: entity/actor/holders/player/CursedWeapon.java (saveData, endOfLife)
//
// Same layout as manor.rs: verbatim SQL, one method per statement. D-39, D-53: the Java game
// server writes the same rows; the world decides which writes go out (`CycleDatabaseWrites`).

use sqlx::FromRow;

use crate::error::Result;
use crate::pool::Pool;

// Java: CursedWeaponsManager.restore
const SELECT_ALL: &str =
    "SELECT itemId, charId, playerKarma, playerPkKills, nbKills, endTime FROM cursed_weapons";
// Java: CursedWeapon.saveData
const DELETE_WEAPON: &str = "DELETE FROM cursed_weapons WHERE itemId = ?";
const INSERT_WEAPON: &str = "INSERT INTO cursed_weapons (itemId, charId, playerKarma, playerPkKills, nbKills, endTime) VALUES (?, ?, ?, ?, ?, ?)";
// Java: CursedWeaponsManager.controlPlayers
const SELECT_ITEM_OWNER: &str = "SELECT owner_id FROM items WHERE item_id=?";
// Java: CursedWeaponsManager.controlPlayers, CursedWeapon.endOfLife (offline)
const DELETE_OWNER_ITEM: &str = "DELETE FROM items WHERE owner_id=? AND item_id=?";
const RESTORE_KARMA: &str = "UPDATE characters SET karma=?, pkkills=? WHERE charId=?";

/// A `cursed_weapons` row: a weapon some character holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CursedWeaponRow {
    pub item_id: i32,
    pub char_id: i32,
    pub player_karma: i32,
    pub player_pk_kills: i32,
    pub nb_kills: i32,
    /// `endTime`: unix milliseconds.
    pub end_time: i64,
}

#[derive(Debug, Clone, FromRow)]
struct CursedWeaponDbRow {
    #[sqlx(rename = "itemId")]
    item_id: i32,
    #[sqlx(rename = "charId")]
    char_id: u32,
    #[sqlx(rename = "playerKarma")]
    player_karma: Option<i32>,
    #[sqlx(rename = "playerPkKills")]
    player_pk_kills: Option<i32>,
    #[sqlx(rename = "nbKills")]
    nb_kills: Option<i32>,
    #[sqlx(rename = "endTime")]
    end_time: u64,
}

impl From<CursedWeaponDbRow> for CursedWeaponRow {
    fn from(r: CursedWeaponDbRow) -> Self {
        Self {
            item_id: r.item_id,
            char_id: i32::try_from(r.char_id).unwrap_or(i32::MAX),
            // `rs.getInt` of a NULL is 0.
            player_karma: r.player_karma.unwrap_or(0),
            player_pk_kills: r.player_pk_kills.unwrap_or(0),
            nb_kills: r.nb_kills.unwrap_or(0),
            end_time: i64::try_from(r.end_time).unwrap_or(i64::MAX),
        }
    }
}

/// Every query on `cursed_weapons`.
#[derive(Debug, Clone)]
pub struct CursedWeaponRepository {
    pool: Pool,
}

impl CursedWeaponRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Every row (Java `restore`).
    pub async fn all(&self) -> Result<Vec<CursedWeaponRow>> {
        let rows = sqlx::query_as::<_, CursedWeaponDbRow>(SELECT_ALL)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(CursedWeaponRow::from).collect())
    }

    /// Java `CursedWeapon.saveData`: the old row goes, a new one is written when the weapon is
    /// held (`row` is `Some`). One transaction instead of Java's two statements on one connection.
    pub async fn save(&self, item_id: i32, row: Option<&CursedWeaponRow>) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(DELETE_WEAPON).bind(item_id).execute(&mut *tx).await?;
        if let Some(row) = row {
            sqlx::query(INSERT_WEAPON)
                .bind(row.item_id)
                .bind(row.char_id)
                .bind(row.player_karma)
                .bind(row.player_pk_kills)
                .bind(row.nb_kills)
                .bind(row.end_time)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Java `CursedWeaponsManager.removeFromDb`.
    pub async fn remove(&self, item_id: i32) -> Result<()> {
        sqlx::query(DELETE_WEAPON)
            .bind(item_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// The first `items` row of the weapon (`controlPlayers`): its owner.
    pub async fn item_owner(&self, item_id: i32) -> Result<Option<i32>> {
        let row: Option<(Option<i32>,)> = sqlx::query_as(SELECT_ITEM_OWNER)
            .bind(item_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|(owner,)| owner.unwrap_or(0)))
    }

    /// `DELETE FROM items WHERE owner_id=? AND item_id=?`: the rows deleted (Java warns when it
    /// is not exactly 1).
    pub async fn delete_owner_item(&self, owner_id: i32, item_id: i32) -> Result<u64> {
        let done = sqlx::query(DELETE_OWNER_ITEM)
            .bind(owner_id)
            .bind(item_id)
            .execute(&self.pool)
            .await?;
        Ok(done.rows_affected())
    }

    /// `UPDATE characters SET karma=?, pkkills=? WHERE charId=?`: the rows changed.
    pub async fn restore_karma(&self, char_id: i32, karma: i32, pk_kills: i32) -> Result<u64> {
        let done = sqlx::query(RESTORE_KARMA)
            .bind(karma)
            .bind(pk_kills)
            .bind(char_id)
            .execute(&self.pool)
            .await?;
        Ok(done.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_statements_have_one_placeholder_per_value() {
        assert_eq!(INSERT_WEAPON.matches('?').count(), 6);
        assert_eq!(DELETE_WEAPON.matches('?').count(), 1);
        assert_eq!(SELECT_ITEM_OWNER.matches('?').count(), 1);
        assert_eq!(DELETE_OWNER_ITEM.matches('?').count(), 2);
        assert_eq!(RESTORE_KARMA.matches('?').count(), 3);
    }
}
