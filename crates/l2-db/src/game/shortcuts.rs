//! The `character_shortcuts` table: the shortcut bars of one class index.
// Java: org/l2jmobius/gameserver/entity/actor/holders/player/Shortcuts.java (registerShortcutInDb,
// Java: deleteShortcutFromDb, restoreMe), entity/actor/Player.java (DELETE_CHAR_SHORTCUTS, used
// Java: by modifySubClass)
//
// Same layout as characters.rs: verbatim SQL, one method per statement. The DECIMAL columns and
// the VARCHAR `level` are read through `SqlNumber` (see sql_number.rs).

use sqlx::FromRow;

use super::sql_number::SqlNumber;
use crate::error::Result;
use crate::pool::Pool;

// Java: Shortcuts.registerShortcutInDb
const REPLACE_SHORTCUT: &str = "REPLACE INTO character_shortcuts (charId,slot,page,type,shortcut_id,level,class_index) values(?,?,?,?,?,?,?)";
// Java: Shortcuts.deleteShortcutFromDb
const DELETE_SHORTCUT: &str =
    "DELETE FROM character_shortcuts WHERE charId=? AND slot=? AND page=? AND class_index=?";
// Java: Shortcuts.restoreMe
const SELECT_SHORTCUTS: &str = "SELECT charId, slot, page, type, shortcut_id, level FROM character_shortcuts WHERE charId=? AND class_index=?";
// Java: Player.DELETE_CHAR_SHORTCUTS
const DELETE_CLASS_SHORTCUTS: &str = "DELETE FROM character_shortcuts WHERE charId=? AND class_index=?";

/// One shortcut to store, the values of `REPLACE_SHORTCUT` after the ids.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewShortcut {
    /// `slot`: position on the bar, 0..=11.
    pub slot: i32,
    /// `page`: bar number.
    pub page: i32,
    /// `type`: the `ShortcutType` ordinal.
    pub kind: i32,
    /// `shortcut_id`: item object id, skill id or action id.
    pub shortcut_id: i32,
    /// `level`: skill level (0 for other kinds).
    pub level: i32,
}

/// A row of the shortcut select. The `DECIMAL` and `VARCHAR` columns are read as integers, and
/// the nullable ones are `None` where Java's `getInt` would read 0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutRow {
    /// `charId`.
    pub char_id: u32,
    /// `slot`.
    pub slot: i32,
    /// `page`.
    pub page: i32,
    /// `type`: the `ShortcutType` ordinal.
    pub kind: Option<i32>,
    /// `shortcut_id`.
    pub shortcut_id: Option<i64>,
    /// `level`.
    pub level: Option<i32>,
}

/// Database shape of [`ShortcutRow`].
#[derive(Debug, Clone, FromRow)]
struct ShortcutDbRow {
    #[sqlx(rename = "charId")]
    char_id: u32,
    slot: SqlNumber<i32>,
    page: SqlNumber<i32>,
    #[sqlx(rename = "type")]
    kind: Option<SqlNumber<i32>>,
    shortcut_id: Option<SqlNumber<i64>>,
    level: Option<SqlNumber<i32>>,
}

impl From<ShortcutDbRow> for ShortcutRow {
    fn from(r: ShortcutDbRow) -> Self {
        Self {
            char_id: r.char_id,
            slot: r.slot.0,
            page: r.page.0,
            kind: r.kind.map(|v| v.0),
            shortcut_id: r.shortcut_id.map(|v| v.0),
            level: r.level.map(|v| v.0),
        }
    }
}

/// Every query on `character_shortcuts` that M2 needs.
#[derive(Debug, Clone)]
pub struct ShortcutRepository {
    pool: Pool,
}

impl ShortcutRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Stores a shortcut in its slot, replacing whatever the slot held (`REPLACE`).
    // Java: Shortcuts.registerShortcutInDb
    pub async fn replace_shortcut(
        &self,
        char_id: i32,
        class_index: i32,
        shortcut: &NewShortcut,
    ) -> Result<()> {
        sqlx::query(REPLACE_SHORTCUT)
            .bind(char_id)
            .bind(shortcut.slot)
            .bind(shortcut.page)
            .bind(shortcut.kind)
            .bind(shortcut.shortcut_id)
            .bind(shortcut.level)
            .bind(class_index)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Removes the shortcut of one slot.
    // Java: Shortcuts.deleteShortcutFromDb
    pub async fn delete_shortcut(&self, char_id: i32, class_index: i32, slot: i32, page: i32) -> Result<()> {
        sqlx::query(DELETE_SHORTCUT)
            .bind(char_id)
            .bind(slot)
            .bind(page)
            .bind(class_index)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// The shortcuts of one class index.
    // Java: Shortcuts.restoreMe
    pub async fn shortcuts_of(&self, char_id: i32, class_index: i32) -> Result<Vec<ShortcutRow>> {
        let rows = sqlx::query_as::<_, ShortcutDbRow>(SELECT_SHORTCUTS)
            .bind(char_id)
            .bind(class_index)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(ShortcutRow::from).collect())
    }

    /// Removes every shortcut of one class index (subclass deletion).
    // Java: Player.modifySubClass (DELETE_CHAR_SHORTCUTS)
    pub async fn delete_class_shortcuts(&self, char_id: i32, class_index: i32) -> Result<()> {
        sqlx::query(DELETE_CLASS_SHORTCUTS)
            .bind(char_id)
            .bind(class_index)
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
        assert_eq!(placeholders(REPLACE_SHORTCUT), 7);
        assert_eq!(placeholders(DELETE_SHORTCUT), 4);
        assert_eq!(placeholders(SELECT_SHORTCUTS), 2);
        assert_eq!(placeholders(DELETE_CLASS_SHORTCUTS), 2);
    }
}
