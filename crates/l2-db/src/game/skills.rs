//! The `character_skills` and `character_skills_save` tables: the skills a character knows per
//! class index, and the buffs and reuse timers that survive a logout.
// Java: org/l2jmobius/gameserver/entity/actor/Player.java (RESTORE_SKILLS_FOR_CHAR,
// Java: UPDATE_CHARACTER_SKILL_LEVEL, ADD_NEW_SKILLS, DELETE_SKILL_FROM_CHAR, DELETE_CHAR_SKILLS,
// Java: ADD_SKILL_SAVE, RESTORE_SKILL_SAVE, DELETE_SKILL_SAVE; restoreSkills, storeSkill,
// Java: storeSkills, removeSkill, storeEffect, restoreEffects)
//
// Same layout as characters.rs: verbatim SQL, one method per statement.

use sqlx::FromRow;

use crate::error::Result;
use crate::pool::Pool;

// Java: Player.RESTORE_SKILLS_FOR_CHAR
const SELECT_SKILLS: &str =
    "SELECT skill_id,skill_level FROM character_skills WHERE charId=? AND class_index=?";
// Java: Player.UPDATE_CHARACTER_SKILL_LEVEL
const UPDATE_SKILL_LEVEL: &str =
    "UPDATE character_skills SET skill_level=? WHERE skill_id=? AND charId=? AND class_index=?";
// Java: Player.ADD_NEW_SKILLS
const REPLACE_SKILL: &str =
    "REPLACE INTO character_skills (charId,skill_id,skill_level,class_index) VALUES (?,?,?,?)";
// Java: Player.DELETE_SKILL_FROM_CHAR
const DELETE_SKILL: &str = "DELETE FROM character_skills WHERE skill_id=? AND charId=? AND class_index=?";
// Java: Player.DELETE_CHAR_SKILLS
const DELETE_CLASS_SKILLS: &str = "DELETE FROM character_skills WHERE charId=? AND class_index=?";
// Java: Player.ADD_SKILL_SAVE
const REPLACE_SKILL_SAVE: &str = "REPLACE INTO character_skills_save (charId,skill_id,skill_level,remaining_time,reuse_delay,systime,restore_type,class_index,buff_index) VALUES (?,?,?,?,?,?,?,?,?)";
// Java: Player.RESTORE_SKILL_SAVE
const SELECT_SKILL_SAVE: &str = "SELECT skill_id,skill_level,remaining_time, reuse_delay, systime, restore_type FROM character_skills_save WHERE charId=? AND class_index=? ORDER BY buff_index ASC";
// Java: Player.DELETE_SKILL_SAVE
const DELETE_SKILL_SAVE: &str = "DELETE FROM character_skills_save WHERE charId=? AND class_index=?";

/// A skill and its level, one row of `character_skills`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromRow)]
pub struct SkillLevel {
    /// `skill_id`.
    pub skill_id: i32,
    /// `skill_level`.
    pub skill_level: i32,
}

/// A row of `character_skills_save`: one buff or reuse timer kept over a logout.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct SkillSaveRow {
    /// `skill_id`.
    pub skill_id: i32,
    /// `skill_level`.
    pub skill_level: i32,
    /// `remaining_time`: seconds the effect still had.
    pub remaining_time: i32,
    /// `reuse_delay`: milliseconds of the reuse timer (Java `getLong`).
    pub reuse_delay: i64,
    /// `systime`: millisecond time the reuse timer ends.
    pub systime: u64,
    /// `restore_type`: 0 for an active effect, 1 for a reuse timer only.
    pub restore_type: i32,
}

/// The values of one `character_skills_save` row, as `Player.storeEffect` binds them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewSkillSave {
    /// `skill_id`.
    pub skill_id: i32,
    /// `skill_level`.
    pub skill_level: i32,
    /// `remaining_time`.
    pub remaining_time: i32,
    /// `reuse_delay`.
    pub reuse_delay: i64,
    /// `systime`.
    pub systime: i64,
    /// `restore_type`.
    pub restore_type: i32,
    /// `buff_index`: 1-based order of the effects, restored in this order.
    pub buff_index: i32,
}

/// Every query on the skill tables that M2 needs.
#[derive(Debug, Clone)]
pub struct SkillRepository {
    pool: Pool,
}

impl SkillRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// The skills of one class index.
    // Java: Player.restoreSkills
    pub async fn skills_of(&self, char_id: i32, class_index: i32) -> Result<Vec<SkillLevel>> {
        let rows = sqlx::query_as::<_, SkillLevel>(SELECT_SKILLS)
            .bind(char_id)
            .bind(class_index)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// Sets the level of a skill the character already has.
    // Java: Player.storeSkill (old and new skill)
    pub async fn update_skill_level(
        &self,
        char_id: i32,
        class_index: i32,
        skill_id: i32,
        skill_level: i32,
    ) -> Result<()> {
        sqlx::query(UPDATE_SKILL_LEVEL)
            .bind(skill_level)
            .bind(skill_id)
            .bind(char_id)
            .bind(class_index)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Adds or replaces skills (`REPLACE`). One row is `Player.storeSkill`; a list is
    /// `Player.storeSkills`, run on one connection like Java's batch.
    // Java: Player.storeSkill (new skill), Player.storeSkills
    pub async fn replace_skills(&self, char_id: i32, class_index: i32, skills: &[SkillLevel]) -> Result<()> {
        if skills.is_empty() {
            return Ok(());
        }
        let mut conn = self.pool.acquire().await?;
        for skill in skills {
            sqlx::query(REPLACE_SKILL)
                .bind(char_id)
                .bind(skill.skill_id)
                .bind(skill.skill_level)
                .bind(class_index)
                .execute(&mut *conn)
                .await?;
        }
        Ok(())
    }

    /// Removes one skill of the character.
    // Java: Player.removeSkill
    pub async fn delete_skill(&self, char_id: i32, class_index: i32, skill_id: i32) -> Result<()> {
        sqlx::query(DELETE_SKILL)
            .bind(skill_id)
            .bind(char_id)
            .bind(class_index)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Removes every skill of one class index (subclass deletion).
    // Java: Player.modifySubClass (DELETE_CHAR_SKILLS)
    pub async fn delete_class_skills(&self, char_id: i32, class_index: i32) -> Result<()> {
        sqlx::query(DELETE_CLASS_SKILLS)
            .bind(char_id)
            .bind(class_index)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// The saved buffs and reuse timers of one class index, in buff order.
    // Java: Player.restoreEffects
    pub async fn skill_effects_of(&self, char_id: i32, class_index: i32) -> Result<Vec<SkillSaveRow>> {
        let rows = sqlx::query_as::<_, SkillSaveRow>(SELECT_SKILL_SAVE)
            .bind(char_id)
            .bind(class_index)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// Removes the saved effects of one class index.
    // Java: Player.storeEffect (before the new rows), Player.restoreEffects (after reading)
    pub async fn delete_skill_effects(&self, char_id: i32, class_index: i32) -> Result<()> {
        sqlx::query(DELETE_SKILL_SAVE)
            .bind(char_id)
            .bind(class_index)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Saves effects and reuse timers (`REPLACE`), on one connection like Java's batch.
    // Java: Player.storeEffect
    pub async fn add_skill_effects(
        &self,
        char_id: i32,
        class_index: i32,
        effects: &[NewSkillSave],
    ) -> Result<()> {
        if effects.is_empty() {
            return Ok(());
        }
        let mut conn = self.pool.acquire().await?;
        for e in effects {
            sqlx::query(REPLACE_SKILL_SAVE)
                .bind(char_id)
                .bind(e.skill_id)
                .bind(e.skill_level)
                .bind(e.remaining_time)
                .bind(e.reuse_delay)
                .bind(e.systime)
                .bind(e.restore_type)
                .bind(class_index)
                .bind(e.buff_index)
                .execute(&mut *conn)
                .await?;
        }
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
        assert_eq!(placeholders(SELECT_SKILLS), 2);
        assert_eq!(placeholders(UPDATE_SKILL_LEVEL), 4);
        assert_eq!(placeholders(REPLACE_SKILL), 4);
        assert_eq!(placeholders(DELETE_SKILL), 3);
        assert_eq!(placeholders(DELETE_CLASS_SKILLS), 2);
        assert_eq!(placeholders(REPLACE_SKILL_SAVE), 9);
        assert_eq!(placeholders(SELECT_SKILL_SAVE), 2);
        assert_eq!(placeholders(DELETE_SKILL_SAVE), 2);
    }
}
