//! The `character_macroses` table: the macros of a character.
// Java: org/l2jmobius/gameserver/entity/actor/holders/player/MacroList.java (registerMacroInDb,
// Java: deleteMacroFromDb)
//
// Same layout as characters.rs: verbatim SQL, one method per statement. Only the writes of
// character creation exist so far (`InitialShortcutData.registerAllShortcuts` registers the
// macro presets of the shortcut bar).

use crate::error::Result;
use crate::pool::Pool;

// Java: MacroList.registerMacroInDb
const INSERT_MACRO: &str =
    "INSERT INTO character_macroses (charId,id,icon,name,descr,acronym,commands) values(?,?,?,?,?,?,?)";
// Java: MacroList.deleteMacroFromDb
const DELETE_MACRO: &str = "DELETE FROM character_macroses WHERE charId=? AND id=?";

/// One macro to store, the values of `INSERT_MACRO` after the character id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewMacro {
    /// `id`.
    pub id: i32,
    /// `icon`.
    pub icon: i32,
    /// `name`.
    pub name: String,
    /// `descr`: `None` is SQL `NULL` (Java binds a null `String`).
    pub descr: Option<String>,
    /// `acronym`.
    pub acronym: String,
    /// `commands`: the command list already joined and cut to 255 characters the way
    /// `registerMacroInDb` does.
    pub commands: String,
}

/// Every query on `character_macroses` that M2 needs.
#[derive(Debug, Clone)]
pub struct MacroRepository {
    pool: Pool,
}

impl MacroRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Stores a macro. Fails on a duplicate `(charId, id)` (primary key), like Java, which
    /// deletes the old row first when it replaces a macro.
    // Java: MacroList.registerMacroInDb
    pub async fn insert_macro(&self, char_id: i32, m: &NewMacro) -> Result<()> {
        sqlx::query(INSERT_MACRO)
            .bind(char_id)
            .bind(m.id)
            .bind(m.icon)
            .bind(&m.name)
            .bind(m.descr.as_deref())
            .bind(&m.acronym)
            .bind(&m.commands)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Removes one macro of the character.
    // Java: MacroList.deleteMacroFromDb
    pub async fn delete_macro(&self, char_id: i32, id: i32) -> Result<()> {
        sqlx::query(DELETE_MACRO)
            .bind(char_id)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
