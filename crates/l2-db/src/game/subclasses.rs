//! The `character_subclasses` table: the subclasses of a character, each with its own exp, sp and
//! level, keyed by class index.
// Java: org/l2jmobius/gameserver/entity/actor/Player.java (RESTORE_CHAR_SUBCLASSES,
// Java: ADD_CHAR_SUBCLASS, UPDATE_CHAR_SUBCLASS, DELETE_CHAR_SUBCLASS; restoreSubClassData,
// Java: addSubClass, storeCharSub, modifySubClass)
//
// Same layout as characters.rs: verbatim SQL, one method per statement.

use sqlx::FromRow;

use crate::error::Result;
use crate::pool::Pool;

// Java: Player.RESTORE_CHAR_SUBCLASSES
const SELECT_SUBCLASSES: &str = "SELECT class_id,exp,sp,level,class_index FROM character_subclasses WHERE charId=? ORDER BY class_index ASC";
// Java: Player.ADD_CHAR_SUBCLASS
const INSERT_SUBCLASS: &str =
    "INSERT INTO character_subclasses (charId,class_id,exp,sp,level,class_index) VALUES (?,?,?,?,?,?)";
// Java: Player.UPDATE_CHAR_SUBCLASS
const UPDATE_SUBCLASS: &str =
    "UPDATE character_subclasses SET exp=?,sp=?,level=?,class_id=? WHERE charId=? AND class_index =?";
// Java: Player.DELETE_CHAR_SUBCLASS
const DELETE_SUBCLASS: &str = "DELETE FROM character_subclasses WHERE charId=? AND class_index=?";

/// A row of `character_subclasses`.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct SubclassRow {
    /// `class_id`: the class of this slot.
    pub class_id: i32,
    /// `exp`.
    pub exp: i64,
    /// `sp`.
    pub sp: i64,
    /// `level`.
    pub level: i32,
    /// `class_index`: 1..=3 for subclasses, 0 is the base class (kept in `characters`).
    pub class_index: i32,
}

/// The values of a subclass, in the order the Java statements bind them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubclassValues {
    /// `class_id`.
    pub class_id: i32,
    /// `exp`.
    pub exp: i64,
    /// `sp`.
    pub sp: i64,
    /// `level`.
    pub level: i32,
}

/// Every query on `character_subclasses` that M2 needs.
#[derive(Debug, Clone)]
pub struct SubclassRepository {
    pool: Pool,
}

impl SubclassRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// The character's subclasses, lowest class index first.
    // Java: Player.restoreSubClassData
    pub async fn subclasses_of(&self, char_id: i32) -> Result<Vec<SubclassRow>> {
        let rows = sqlx::query_as::<_, SubclassRow>(SELECT_SUBCLASSES)
            .bind(char_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// Adds a subclass at `class_index`. Fails on a duplicate key (charId, class_id).
    // Java: Player.addSubClass
    pub async fn insert_subclass(&self, char_id: i32, class_index: i32, sub: &SubclassValues) -> Result<()> {
        sqlx::query(INSERT_SUBCLASS)
            .bind(char_id)
            .bind(sub.class_id)
            .bind(sub.exp)
            .bind(sub.sp)
            .bind(sub.level)
            .bind(class_index)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Writes exp, sp, level and class id of the subclass at `class_index`. Java loops this
    /// over every subclass in one batch; the caller does the same.
    // Java: Player.storeCharSub
    pub async fn update_subclass(&self, char_id: i32, class_index: i32, sub: &SubclassValues) -> Result<()> {
        sqlx::query(UPDATE_SUBCLASS)
            .bind(sub.exp)
            .bind(sub.sp)
            .bind(sub.level)
            .bind(sub.class_id)
            .bind(char_id)
            .bind(class_index)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Removes the subclass at `class_index`. The caller removes its skills, shortcuts and
    /// effects first, like `Player.modifySubClass`.
    // Java: Player.modifySubClass (DELETE_CHAR_SUBCLASS)
    pub async fn delete_subclass(&self, char_id: i32, class_index: i32) -> Result<()> {
        sqlx::query(DELETE_SUBCLASS)
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
        assert_eq!(placeholders(SELECT_SUBCLASSES), 1);
        assert_eq!(placeholders(INSERT_SUBCLASS), 6);
        assert_eq!(placeholders(UPDATE_SUBCLASS), 6);
        assert_eq!(placeholders(DELETE_SUBCLASS), 2);
    }
}
