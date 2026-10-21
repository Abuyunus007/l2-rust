//! The servitor tables: `character_summons` (a servitor stored for a restore after a relog) and
//! `character_summon_skills_save` (the buffs a servitor had when it was unsummoned).
// Java: org/l2jmobius/gameserver/data/sql/CharSummonTable.java (SAVE_SUMMON, REMOVE_SUMMON)
// Java: org/l2jmobius/gameserver/entity/actor/instance/Servitor.java (ADD_SKILL_SAVE,
// Java: RESTORE_SKILL_SAVE, DELETE_SKILL_SAVE)
//
// Same layout as skills.rs: verbatim SQL, one method per statement.

use sqlx::FromRow;

use crate::error::Result;
use crate::pool::Pool;

// Java: CharSummonTable.SAVE_SUMMON
const SAVE_SUMMON: &str =
    "REPLACE INTO character_summons (ownerId,summonSkillId,curHp,curMp,time) VALUES (?,?,?,?,?)";
// Java: CharSummonTable.REMOVE_SUMMON
const REMOVE_SUMMON: &str = "DELETE FROM character_summons WHERE ownerId = ?";
// Java: Servitor.ADD_SKILL_SAVE
const ADD_SKILL_SAVE: &str = "REPLACE INTO character_summon_skills_save (ownerId,ownerClassIndex,summonSkillId,skill_id,skill_level,remaining_time,buff_index) VALUES (?,?,?,?,?,?,?)";
// Java: Servitor.RESTORE_SKILL_SAVE, without the `summonSkillId` filter: the loader reads every
// reference skill of the character's class index at once (the world keeps them by reference
// skill, `Servitor.restoreEffects` picks its own).
const RESTORE_SKILL_SAVE_ALL: &str = "SELECT summonSkillId,skill_id,skill_level,remaining_time FROM character_summon_skills_save WHERE ownerId=? AND ownerClassIndex=? ORDER BY buff_index ASC";
// Java: Servitor.DELETE_SKILL_SAVE
const DELETE_SKILL_SAVE: &str =
    "DELETE FROM character_summon_skills_save WHERE ownerId=? AND ownerClassIndex=? AND summonSkillId=?";

/// A row of `character_summon_skills_save` as the loader reads it.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct SummonSkillSaveRow {
    /// `summonSkillId`: the skill that summoned the servitor.
    #[sqlx(rename = "summonSkillId")]
    pub summon_skill_id: i32,
    pub skill_id: i32,
    pub skill_level: i32,
    pub remaining_time: i32,
}

/// The values of one `character_summon_skills_save` row, as `Servitor.storeEffect` binds them
/// (`buff_index` is the position, from 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewSummonSkillSave {
    pub skill_id: i32,
    pub skill_level: i32,
    pub remaining_time: i32,
}

/// Every query on the servitor tables.
#[derive(Debug, Clone)]
pub struct SummonRepository {
    pool: Pool,
}

impl SummonRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Stores a servitor for a restore.
    // Java: CharSummonTable.saveSummon
    pub async fn save_summon(
        &self,
        owner_id: i32,
        summon_skill_id: i32,
        cur_hp: i32,
        cur_mp: i32,
        time: i32,
    ) -> Result<()> {
        sqlx::query(SAVE_SUMMON)
            .bind(owner_id)
            .bind(summon_skill_id)
            .bind(cur_hp)
            .bind(cur_mp)
            .bind(time)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Forgets the stored servitor of a character.
    // Java: CharSummonTable.removeServitor
    pub async fn remove_summon(&self, owner_id: i32) -> Result<()> {
        sqlx::query(REMOVE_SUMMON)
            .bind(owner_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// The saved buffs of every servitor of a character's class index, in `buff_index` order.
    // Java: Servitor.restoreEffects (the SELECT, for every reference skill at once)
    pub async fn servitor_effects_of(
        &self,
        owner_id: i32,
        class_index: i32,
    ) -> Result<Vec<SummonSkillSaveRow>> {
        let rows = sqlx::query_as::<_, SummonSkillSaveRow>(RESTORE_SKILL_SAVE_ALL)
            .bind(owner_id)
            .bind(class_index)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// Replaces the saved buffs of one servitor: the `DELETE`, then the rows (none = only the
    /// delete), on one connection like Java's batch.
    // Java: Servitor.storeEffect, Servitor.restoreEffects (the DELETE)
    pub async fn store_servitor_effects(
        &self,
        owner_id: i32,
        class_index: i32,
        summon_skill_id: i32,
        rows: &[NewSummonSkillSave],
    ) -> Result<()> {
        let mut conn = self.pool.acquire().await?;
        sqlx::query(DELETE_SKILL_SAVE)
            .bind(owner_id)
            .bind(class_index)
            .bind(summon_skill_id)
            .execute(&mut *conn)
            .await?;
        for (index, row) in rows.iter().enumerate() {
            let buff_index = i32::try_from(index + 1).unwrap_or(i32::MAX);
            sqlx::query(ADD_SKILL_SAVE)
                .bind(owner_id)
                .bind(class_index)
                .bind(summon_skill_id)
                .bind(row.skill_id)
                .bind(row.skill_level)
                .bind(row.remaining_time)
                .bind(buff_index)
                .execute(&mut *conn)
                .await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statements_have_one_placeholder_per_bind() {
        assert_eq!(SAVE_SUMMON.matches('?').count(), 5);
        assert_eq!(REMOVE_SUMMON.matches('?').count(), 1);
        assert_eq!(ADD_SKILL_SAVE.matches('?').count(), 7);
        assert_eq!(RESTORE_SKILL_SAVE_ALL.matches('?').count(), 2);
        assert_eq!(DELETE_SKILL_SAVE.matches('?').count(), 3);
    }
}
