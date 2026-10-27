//! The `character_quests` table: every quest state and quest variable of a character
//! (S-API-01).
// Java: org/l2jmobius/gameserver/mechanics/script/Quest.java (playerEnter, createQuestVarInDb,
// Java: updateQuestVarInDb, deleteQuestVarInDb, deleteQuestInDb)
//
// Same layout as hennas.rs: verbatim SQL, one method per statement. The rows are per character,
// so they are written as Java writes them side by side with the Java game server (D-54).

use sqlx::FromRow;

use crate::error::Result;
use crate::pool::Pool;

/// Java `Quest.playerEnter`'s `ps1`: the state rows (`var = '<state>'`).
const SELECT_STATES: &str = "SELECT name, value FROM character_quests WHERE charId = ? AND var = ?";
/// Java `Quest.playerEnter`'s `ps2`: the variables (`var <> '<state>'`).
const SELECT_VARIABLES: &str = "SELECT name, var, value FROM character_quests WHERE charId = ? AND var <> ?";
// Java: Quest.createQuestVarInDb
const INSERT_VARIABLE: &str =
    "INSERT INTO character_quests (charId,name,var,value) VALUES (?,?,?,?) ON DUPLICATE KEY UPDATE value=?";
// Java: Quest.updateQuestVarInDb
const UPDATE_VARIABLE: &str = "UPDATE character_quests SET value=? WHERE charId=? AND name=? AND var = ?";
// Java: Quest.deleteQuestVarInDb
const DELETE_VARIABLE: &str = "DELETE FROM character_quests WHERE charId=? AND name=? AND var=?";
// Java: Quest.QUEST_DELETE_FROM_CHAR_QUERY
const DELETE_QUEST: &str = "DELETE FROM character_quests WHERE charId=? AND name=?";
// Java: Quest.QUEST_DELETE_FROM_CHAR_QUERY_NON_REPEATABLE_QUERY
const DELETE_QUEST_KEEP_STATE: &str = "DELETE FROM character_quests WHERE charId=? AND name=? AND var!=?";

/// The `var` of the state row (Java `"<state>"`).
pub const QUEST_STATE_VAR: &str = "<state>";

/// A state row: the quest's name and its state name (`Start`, `Started`, `Completed`). The
/// column may be NULL.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct QuestStateRow {
    pub name: String,
    pub value: Option<String>,
}

/// A variable row of a quest. The value may be NULL.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct QuestVariableRow {
    pub name: String,
    pub var: String,
    pub value: Option<String>,
}

/// The queries on `character_quests`.
#[derive(Debug, Clone)]
pub struct QuestRepository {
    pool: Pool,
}

impl QuestRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// The state rows of one character, as Java reads them (no ordering).
    // Java: Quest.playerEnter (ps1)
    pub async fn quest_states_of(&self, char_id: i32) -> Result<Vec<QuestStateRow>> {
        let rows = sqlx::query_as::<_, QuestStateRow>(SELECT_STATES)
            .bind(char_id)
            .bind(QUEST_STATE_VAR)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// The variable rows of one character, as Java reads them (no ordering).
    // Java: Quest.playerEnter (ps2)
    pub async fn quest_variables_of(&self, char_id: i32) -> Result<Vec<QuestVariableRow>> {
        let rows = sqlx::query_as::<_, QuestVariableRow>(SELECT_VARIABLES)
            .bind(char_id)
            .bind(QUEST_STATE_VAR)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// Inserts a variable (or the state row), or replaces its value.
    // Java: Quest.createQuestVarInDb
    pub async fn insert_variable(&self, char_id: i32, name: &str, var: &str, value: &str) -> Result<()> {
        sqlx::query(INSERT_VARIABLE)
            .bind(char_id)
            .bind(name)
            .bind(var)
            .bind(value)
            .bind(value)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Updates the value of a variable (nothing happens when the row is not there).
    // Java: Quest.updateQuestVarInDb
    pub async fn update_variable(&self, char_id: i32, name: &str, var: &str, value: &str) -> Result<()> {
        sqlx::query(UPDATE_VARIABLE)
            .bind(value)
            .bind(char_id)
            .bind(name)
            .bind(var)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Deletes one variable.
    // Java: Quest.deleteQuestVarInDb
    pub async fn delete_variable(&self, char_id: i32, name: &str, var: &str) -> Result<()> {
        sqlx::query(DELETE_VARIABLE)
            .bind(char_id)
            .bind(name)
            .bind(var)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Deletes every row of a quest (`repeatable`), or every row but the state row.
    // Java: Quest.deleteQuestInDb
    pub async fn delete_quest(&self, char_id: i32, name: &str, repeatable: bool) -> Result<()> {
        if repeatable {
            sqlx::query(DELETE_QUEST)
                .bind(char_id)
                .bind(name)
                .execute(&self.pool)
                .await?;
        } else {
            sqlx::query(DELETE_QUEST_KEEP_STATE)
                .bind(char_id)
                .bind(name)
                .bind(QUEST_STATE_VAR)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_statements_are_javas() {
        assert!(SELECT_STATES.contains("var = ?"));
        assert!(SELECT_VARIABLES.contains("var <> ?"));
        assert!(INSERT_VARIABLE.ends_with("ON DUPLICATE KEY UPDATE value=?"));
        assert!(DELETE_QUEST_KEEP_STATE.ends_with("var!=?"));
        assert_eq!(QUEST_STATE_VAR, "<state>");
    }
}
