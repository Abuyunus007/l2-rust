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
// Java: MacroList.restoreMe
const SELECT_MACROS: &str =
    "SELECT charId, id, icon, name, descr, acronym, commands FROM character_macroses WHERE charId=?";
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

/// A row of the macro select (`MacroList.restoreMe`). The columns are nullable in the schema;
/// Java reads them with `getInt`/`getString`, so NULL is 0 / null.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct MacroRow {
    /// `id`.
    pub id: i32,
    /// `icon`.
    pub icon: Option<i32>,
    /// `name`.
    pub name: Option<String>,
    /// `descr`.
    pub descr: Option<String>,
    /// `acronym`.
    pub acronym: Option<String>,
    /// `commands`: `type,d1,d2,cmd;` entries.
    pub commands: Option<String>,
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

    /// The macros of the character, in primary key order (Java does not order the select; the
    /// `LinkedHashMap` it fills keeps whatever order the server returns).
    // Java: MacroList.restoreMe
    pub async fn macros_of(&self, char_id: i32) -> Result<Vec<MacroRow>> {
        let rows = sqlx::query_as::<_, MacroRow>(SELECT_MACROS)
            .bind(char_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
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
