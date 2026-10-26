//! The `mods_wedding` table: the couples of the wedding system (`AllowWedding`).
// Java: org/l2jmobius/gameserver/managers/CoupleManager.java (load)
// Java: org/l2jmobius/gameserver/entity/actor/holders/player/Couple.java (constructors,
//       marry, divorce)
//
// Same layout as characters.rs: verbatim SQL, one method per statement. The `id` of a new couple
// is an object id the world took from its `IdFactory` (Java: `IdManager.getNextId()`).

use sqlx::FromRow;

use super::sql_number::SqlNumber;
use crate::error::Result;
use crate::pool::Pool;

// Java: CoupleManager.load
const SELECT_IDS: &str = "SELECT id FROM mods_wedding ORDER BY id";
// Java: Couple(int coupleId)
const SELECT_COUPLE: &str = "SELECT * FROM mods_wedding WHERE id = ?";
// Java: Couple(Player, Player)
const INSERT_COUPLE: &str = "INSERT INTO mods_wedding (id, player1Id, player2Id, married, affianceDate, weddingDate) VALUES (?, ?, ?, ?, ?, ?)";
// Java: Couple.marry
const UPDATE_MARRY: &str = "UPDATE mods_wedding set married = ?, weddingDate = ? where id = ?";
// Java: Couple.divorce
const DELETE_COUPLE: &str = "DELETE FROM mods_wedding WHERE id=?";

/// One couple, as `Couple` holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoupleRow {
    pub id: i32,
    pub player1_id: i32,
    pub player2_id: i32,
    /// `Boolean.parseBoolean(rs.getString("married"))`.
    pub married: bool,
    /// `affianceDate`: Unix milliseconds.
    pub affiance_date: i64,
    /// `weddingDate`: Unix milliseconds.
    pub wedding_date: i64,
}

#[derive(Debug, FromRow)]
struct CoupleDbRow {
    #[sqlx(rename = "player1Id")]
    player1_id: i32,
    #[sqlx(rename = "player2Id")]
    player2_id: i32,
    married: Option<String>,
    #[sqlx(rename = "affianceDate")]
    affiance_date: Option<SqlNumber<i64>>,
    #[sqlx(rename = "weddingDate")]
    wedding_date: Option<SqlNumber<i64>>,
}

/// `Boolean.parseBoolean(text)`: "true" in any case; NULL and everything else is false.
fn java_parse_boolean(text: Option<&str>) -> bool {
    text.is_some_and(|t| t.eq_ignore_ascii_case("true"))
}

/// The text of `married` this server writes. Java's `setBoolean` on a `varchar(5)` column.
// NOTE(java-quirk): with MySQL Connector/J `setBoolean(true)` may be sent as `1`, which
// `Boolean.parseBoolean` reads back as false, so a Java couple would look engaged only after a
// restart. This server writes the words `true` and `false`, which parse back as written.
pub fn married_text(married: bool) -> &'static str {
    if married { "true" } else { "false" }
}

/// Every query the wedding system needs.
#[derive(Debug, Clone)]
pub struct WeddingRepository {
    pool: Pool,
}

impl WeddingRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Java `CoupleManager.load()` with `new Couple(id)` for each id: every couple, by id.
    pub async fn load_all(&self) -> Result<Vec<CoupleRow>> {
        let ids: Vec<(i32,)> = sqlx::query_as(SELECT_IDS).fetch_all(&self.pool).await?;
        let mut couples = Vec::with_capacity(ids.len());
        for (id,) in ids {
            let rows: Vec<CoupleDbRow> = sqlx::query_as(SELECT_COUPLE)
                .bind(id)
                .fetch_all(&self.pool)
                .await?;
            // Java loops over the (one) row; a vanished row leaves the couple empty.
            let mut couple = CoupleRow {
                id,
                player1_id: 0,
                player2_id: 0,
                married: false,
                affiance_date: 0,
                wedding_date: 0,
            };
            for row in rows {
                couple.player1_id = row.player1_id;
                couple.player2_id = row.player2_id;
                couple.married = java_parse_boolean(row.married.as_deref());
                couple.affiance_date = row.affiance_date.map_or(0, |n| n.0);
                couple.wedding_date = row.wedding_date.map_or(0, |n| n.0);
            }
            couples.push(couple);
        }
        Ok(couples)
    }

    /// Java `Couple(Player, Player)`: a new engaged couple.
    pub async fn insert(&self, couple: &CoupleRow) -> Result<()> {
        sqlx::query(INSERT_COUPLE)
            .bind(couple.id)
            .bind(couple.player1_id)
            .bind(couple.player2_id)
            .bind(married_text(couple.married))
            .bind(couple.affiance_date)
            .bind(couple.wedding_date)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Java `Couple.marry()`.
    pub async fn marry(&self, id: i32, wedding_date: i64) -> Result<()> {
        sqlx::query(UPDATE_MARRY)
            .bind(married_text(true))
            .bind(wedding_date)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Java `Couple.divorce()`.
    pub async fn delete(&self, id: i32) -> Result<()> {
        sqlx::query(DELETE_COUPLE).bind(id).execute(&self.pool).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statements_have_one_placeholder_per_bind() {
        assert_eq!(SELECT_IDS.matches('?').count(), 0);
        assert_eq!(SELECT_COUPLE.matches('?').count(), 1);
        assert_eq!(INSERT_COUPLE.matches('?').count(), 6);
        assert_eq!(UPDATE_MARRY.matches('?').count(), 3);
        assert_eq!(DELETE_COUPLE.matches('?').count(), 1);
    }

    #[test]
    fn married_reads_like_boolean_parse_boolean() {
        assert!(java_parse_boolean(Some("true")));
        assert!(java_parse_boolean(Some("TRUE")));
        assert!(!java_parse_boolean(Some("1")));
        assert!(!java_parse_boolean(Some("false")));
        assert!(!java_parse_boolean(None));
        assert!(java_parse_boolean(Some(married_text(true))));
        assert!(!java_parse_boolean(Some(married_text(false))));
    }
}
