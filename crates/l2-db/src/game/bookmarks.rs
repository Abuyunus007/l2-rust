//! The `character_tpbookmark` table: the teleport bookmarks ("My Teleports") of a character.
// Java: org/l2jmobius/gameserver/entity/actor/Player.java (INSERT_TP_BOOKMARK, UPDATE_TP_BOOKMARK,
// Java: RESTORE_TP_BOOKMARK, DELETE_TP_BOOKMARK)
//
// Same layout as hennas.rs: verbatim SQL, one method per statement.

use sqlx::FromRow;

use crate::error::Result;
use crate::pool::Pool;

// Java: Player.RESTORE_TP_BOOKMARK
const SELECT_BOOKMARKS: &str = "SELECT Id,x,y,z,icon,tag,name FROM character_tpbookmark WHERE charId=?";
// Java: Player.INSERT_TP_BOOKMARK
const INSERT_BOOKMARK: &str =
    "INSERT INTO character_tpbookmark (charId,Id,x,y,z,icon,tag,name) values (?,?,?,?,?,?,?,?)";
// Java: Player.UPDATE_TP_BOOKMARK
const UPDATE_BOOKMARK: &str = "UPDATE character_tpbookmark SET icon=?,tag=?,name=? where charId=? AND Id=?";
// Java: Player.DELETE_TP_BOOKMARK
const DELETE_BOOKMARK: &str = "DELETE FROM character_tpbookmark WHERE charId=? AND Id=?";

/// One row of `character_tpbookmark`.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct BookmarkRow {
    /// `Id`: the slot number, 1 and up.
    #[sqlx(rename = "Id")]
    pub id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub icon: i32,
    /// `tag`: NULL is possible in the table; Java's `getString` gives null for it.
    pub tag: Option<String>,
    pub name: String,
}

/// The queries on `character_tpbookmark`.
#[derive(Debug, Clone)]
pub struct BookmarkRepository {
    pool: Pool,
}

impl BookmarkRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// The bookmarks of a character, as Java reads them (no ordering).
    // Java: Player.restoreTeleportBookmark
    pub async fn bookmarks_of(&self, char_id: i32) -> Result<Vec<BookmarkRow>> {
        let rows = sqlx::query_as::<_, BookmarkRow>(SELECT_BOOKMARKS)
            .bind(char_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// A bookmark saved in a slot.
    // Java: Player.teleportBookmarkAdd
    pub async fn add_bookmark(&self, char_id: i32, bookmark: &BookmarkRow) -> Result<()> {
        sqlx::query(INSERT_BOOKMARK)
            .bind(char_id)
            .bind(bookmark.id)
            .bind(bookmark.x)
            .bind(bookmark.y)
            .bind(bookmark.z)
            .bind(bookmark.icon)
            .bind(bookmark.tag.as_deref())
            .bind(&bookmark.name)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// The icon, tag and name of a bookmark changed.
    // Java: Player.teleportBookmarkModify
    pub async fn update_bookmark(
        &self,
        char_id: i32,
        id: i32,
        icon: i32,
        tag: Option<&str>,
        name: &str,
    ) -> Result<()> {
        sqlx::query(UPDATE_BOOKMARK)
            .bind(icon)
            .bind(tag)
            .bind(name)
            .bind(char_id)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// A bookmark deleted.
    // Java: Player.teleportBookmarkDelete
    pub async fn delete_bookmark(&self, char_id: i32, id: i32) -> Result<()> {
        sqlx::query(DELETE_BOOKMARK)
            .bind(char_id)
            .bind(id)
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
        assert_eq!(placeholders(SELECT_BOOKMARKS), 1);
        assert_eq!(placeholders(INSERT_BOOKMARK), 8);
        assert_eq!(placeholders(UPDATE_BOOKMARK), 5);
        assert_eq!(placeholders(DELETE_BOOKMARK), 2);
    }
}
