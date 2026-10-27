//! The `bbs_favorites` table: the favorite links of the community board.
// Java: dist/game/data/scripts/handlers/bypass/communityboard/FavoriteBoard.java (SELECT_FAVORITES,
// Java: DELETE_FAVORITE, ADD_FAVORITE), HomeBoard.java (COUNT_FAVORITES)
//
// Same layout as bookmarks.rs: the Java SQL, one method per statement.
//
// NOTE: Java reads `SELECT *` and `rs.getTimestamp("favAddDate")`. sqlx without a date library
// cannot decode a `TIMESTAMP`, so the list statement names the columns and asks for the date as
// epoch seconds (`UNIX_TIMESTAMP` reads the column in the session time zone, the same instant
// `getTimestamp` gives). The rows and their order are the same.

use sqlx::FromRow;

use crate::error::Result;
use crate::pool::Pool;

// Java: FavoriteBoard.SELECT_FAVORITES
const SELECT_FAVORITES: &str = "SELECT `favId`, `favTitle`, `favBypass`, CAST(UNIX_TIMESTAMP(`favAddDate`) AS SIGNED) AS `favAddEpoch` FROM `bbs_favorites` WHERE `playerId`=? ORDER BY `favAddDate` DESC";
// Java: FavoriteBoard.DELETE_FAVORITE
const DELETE_FAVORITE: &str = "DELETE FROM `bbs_favorites` WHERE `playerId`=? AND `favId`=?";
// Java: FavoriteBoard.ADD_FAVORITE
const ADD_FAVORITE: &str =
    "REPLACE INTO `bbs_favorites`(`playerId`, `favTitle`, `favBypass`) VALUES(?, ?, ?)";
// Java: HomeBoard.COUNT_FAVORITES
const COUNT_FAVORITES: &str = "SELECT COUNT(*) AS favorites FROM `bbs_favorites` WHERE `playerId`=?";

/// One row of `bbs_favorites`.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct FavoriteRow {
    /// `favId`: the auto increment key (an unsigned `INT`).
    #[sqlx(rename = "favId")]
    pub fav_id: u32,
    #[sqlx(rename = "favTitle")]
    pub title: String,
    #[sqlx(rename = "favBypass")]
    pub bypass: String,
    /// `favAddDate` in epoch seconds.
    #[sqlx(rename = "favAddEpoch")]
    pub added_epoch_seconds: i64,
}

/// The queries on `bbs_favorites`.
#[derive(Debug, Clone)]
pub struct BbsFavoriteRepository {
    pool: Pool,
}

impl BbsFavoriteRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// The favorites of a player, the newest first.
    // Java: FavoriteBoard.onCommand (`_bbsgetfav`)
    pub async fn favorites_of(&self, player_id: i32) -> Result<Vec<FavoriteRow>> {
        let rows = sqlx::query_as::<_, FavoriteRow>(SELECT_FAVORITES)
            .bind(player_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// How many favorites a player has.
    // Java: HomeBoard.getFavoriteCount
    pub async fn count_favorites(&self, player_id: i32) -> Result<i64> {
        let (count,): (i64,) = sqlx::query_as(COUNT_FAVORITES)
            .bind(player_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(count)
    }

    /// A link added (`bbs_add_fav`). The date is the column default.
    // Java: FavoriteBoard.onCommand (`bbs_add_fav`)
    pub async fn add_favorite(&self, player_id: i32, title: &str, bypass: &str) -> Result<()> {
        sqlx::query(ADD_FAVORITE)
            .bind(player_id)
            .bind(title)
            .bind(bypass)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// A link deleted (`_bbsdelfav_<id>`), only when it is the player's.
    // Java: FavoriteBoard.onCommand (`_bbsdelfav_`)
    pub async fn delete_favorite(&self, player_id: i32, fav_id: i32) -> Result<()> {
        sqlx::query(DELETE_FAVORITE)
            .bind(player_id)
            .bind(fav_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placeholders(sql: &str) -> usize {
        sql.matches('?').count()
    }

    #[test]
    fn statements_have_one_placeholder_per_bind() {
        assert_eq!(placeholders(SELECT_FAVORITES), 1);
        assert_eq!(placeholders(DELETE_FAVORITE), 2);
        assert_eq!(placeholders(ADD_FAVORITE), 3);
        assert_eq!(placeholders(COUNT_FAVORITES), 1);
    }
}
