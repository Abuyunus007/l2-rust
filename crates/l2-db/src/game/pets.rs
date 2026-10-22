//! The pet tables: `pets` (one row per pet collar: name, level, exp, food, whether it comes back
//! after a relog) and `character_pet_skills_save` (the buffs a pet had when it was unsummoned).
// Java: org/l2jmobius/gameserver/entity/actor/instance/Pet.java (restore, storeMe,
// Java: destroyControlItem, ADD_SKILL_SAVE, RESTORE_SKILL_SAVE, DELETE_SKILL_SAVE)
// Java: org/l2jmobius/gameserver/data/sql/PetNameTable.java (CHECK_PET_NAME)
// Java: org/l2jmobius/gameserver/data/sql/CharSummonTable.java (INIT_PET)
//
// Same layout as summons.rs: verbatim SQL, one method per statement. Pet items are rows of
// `items` with `loc` `PET` / `PET_EQUIP` under the owner's id and need nothing here (see
// `items.rs`).

use sqlx::FromRow;

use crate::error::Result;
use crate::pool::Pool;

// Java: Pet.restore
const RESTORE_PET: &str = "SELECT item_obj_id, name, level, curHp, curMp, exp, sp, fed, ownerId, restore FROM pets WHERE item_obj_id=?";
// Java: Pet.restore for every collar a character holds at once (the Rust loader reads before the
// character enters the world; Java reads when the pet is summoned).
const RESTORE_PETS_OF_OWNER: &str = "SELECT p.item_obj_id, p.name, p.level, p.curHp, p.curMp, p.exp, p.sp, p.fed, p.ownerId, p.restore FROM pets p JOIN items i ON p.item_obj_id = i.object_id WHERE i.owner_id=? AND (i.loc='INVENTORY' OR i.loc='PAPERDOLL')";
// Java: Pet.storeMe (a pet that is not in the table yet)
const INSERT_PET: &str = "INSERT INTO pets (name,level,curHp,curMp,exp,sp,fed,ownerId,restore,item_obj_id) VALUES (?,?,?,?,?,?,?,?,?,?)";
// Java: Pet.storeMe (a pet that is in the table)
const UPDATE_PET: &str = "UPDATE pets SET name=?,level=?,curHp=?,curMp=?,exp=?,sp=?,fed=?,ownerId=?,restore=? WHERE item_obj_id = ?";
// Java: Player.storePetFood
const UPDATE_PET_FED: &str = "UPDATE pets SET fed=? WHERE item_obj_id = ?";
// Java: Pet.destroyControlItem
const DELETE_PET: &str = "DELETE FROM pets WHERE item_obj_id = ?";
// Java: CharSummonTable.INIT_PET
const INIT_PET: &str = "SELECT ownerId, item_obj_id FROM pets WHERE restore = 'true'";
// Java: PetNameTable.CHECK_PET_NAME
const CHECK_PET_NAME: &str =
    "SELECT p.name FROM pets p JOIN items i ON p.item_obj_id = i.object_id WHERE p.name=? AND i.item_id=?";
// Java: PetNameTable.CHECK_PET_NAME for every pet at once (the name table is read at startup).
const ALL_PET_NAMES: &str = "SELECT p.name, i.item_id FROM pets p JOIN items i ON p.item_obj_id = i.object_id WHERE p.name IS NOT NULL";
// Java: Pet.ADD_SKILL_SAVE
const ADD_SKILL_SAVE: &str = "INSERT INTO character_pet_skills_save (petObjItemId,skill_id,skill_level,remaining_time,buff_index) VALUES (?,?,?,?,?)";
// Java: Pet.RESTORE_SKILL_SAVE
const RESTORE_SKILL_SAVE: &str = "SELECT petObjItemId,skill_id,skill_level,remaining_time,buff_index FROM character_pet_skills_save WHERE petObjItemId=? ORDER BY buff_index ASC";
// Java: Pet.RESTORE_SKILL_SAVE for every collar a character holds at once.
const RESTORE_SKILL_SAVE_OF_OWNER: &str = "SELECT s.petObjItemId, s.skill_id, s.skill_level, s.remaining_time, s.buff_index FROM character_pet_skills_save s JOIN items i ON s.petObjItemId = i.object_id WHERE i.owner_id=? AND (i.loc='INVENTORY' OR i.loc='PAPERDOLL') ORDER BY s.petObjItemId, s.buff_index ASC";
// Java: Pet.DELETE_SKILL_SAVE
const DELETE_SKILL_SAVE: &str = "DELETE FROM character_pet_skills_save WHERE petObjItemId=?";

/// A row of `pets`.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct PetRow {
    /// `item_obj_id`: the object id of the collar.
    pub item_obj_id: u32,
    /// `name` (NULL until the owner names the pet).
    pub name: Option<String>,
    /// `level`.
    pub level: u16,
    /// `curHp`.
    #[sqlx(rename = "curHp")]
    pub cur_hp: Option<u32>,
    /// `curMp`.
    #[sqlx(rename = "curMp")]
    pub cur_mp: Option<u32>,
    /// `exp`.
    pub exp: Option<u64>,
    /// `sp`.
    pub sp: Option<u32>,
    /// `fed`.
    pub fed: Option<u32>,
    /// `ownerId`.
    #[sqlx(rename = "ownerId")]
    pub owner_id: i32,
    /// `restore`: `"true"` when the pet comes back after a relog.
    pub restore: String,
}

/// The values `Pet.storeMe` binds, in `INSERT_PET`'s order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PetValues {
    pub name: String,
    pub level: i32,
    /// `getStatus().getCurrentHp()` is bound with `setDouble` into an integer column; the world
    /// hands over the value the column keeps.
    pub cur_hp: i32,
    pub cur_mp: i32,
    pub exp: i64,
    pub sp: i64,
    pub fed: i32,
    pub owner_id: i32,
    /// `String.valueOf(_restoreSummon)`.
    pub restore: bool,
    pub item_obj_id: i32,
}

/// A row of `character_pet_skills_save`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromRow)]
pub struct PetSkillSaveRow {
    /// `petObjItemId`.
    #[sqlx(rename = "petObjItemId")]
    pub pet_obj_item_id: i32,
    pub skill_id: i32,
    pub skill_level: i32,
    pub remaining_time: i32,
    pub buff_index: i32,
}

/// The values of one `character_pet_skills_save` row (`buff_index` is the position, from 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewPetSkillSave {
    pub skill_id: i32,
    pub skill_level: i32,
    pub remaining_time: i32,
}

/// Every query on the pet tables.
#[derive(Debug, Clone)]
pub struct PetRepository {
    pool: Pool,
}

impl PetRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// The row of one collar.
    // Java: Pet.restore
    pub async fn pet_of_control_item(&self, item_obj_id: i32) -> Result<Option<PetRow>> {
        let row = sqlx::query_as::<_, PetRow>(RESTORE_PET)
            .bind(item_obj_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    /// The rows of every collar a character holds in its inventory.
    // Java: Pet.restore (all collars at once)
    pub async fn pets_of_owner(&self, owner_id: i32) -> Result<Vec<PetRow>> {
        let rows = sqlx::query_as::<_, PetRow>(RESTORE_PETS_OF_OWNER)
            .bind(owner_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// `INSERT INTO pets`: a pet that was not stored yet.
    // Java: Pet.storeMe
    pub async fn insert_pet(&self, pet: &PetValues) -> Result<()> {
        self.write(INSERT_PET, pet).await
    }

    /// `UPDATE pets`: a pet that was stored before.
    // Java: Pet.storeMe
    pub async fn update_pet(&self, pet: &PetValues) -> Result<()> {
        self.write(UPDATE_PET, pet).await
    }

    async fn write(&self, sql: &'static str, pet: &PetValues) -> Result<()> {
        sqlx::query(sql)
            .bind(&pet.name)
            .bind(pet.level)
            .bind(pet.cur_hp)
            .bind(pet.cur_mp)
            .bind(pet.exp)
            .bind(pet.sp)
            .bind(pet.fed)
            .bind(pet.owner_id)
            .bind(if pet.restore { "true" } else { "false" })
            .bind(pet.item_obj_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// `UPDATE pets SET fed`: the food a mounted pet had left when its rider got off.
    // Java: Player.storePetFood
    pub async fn update_fed(&self, item_obj_id: i32, fed: i32) -> Result<()> {
        sqlx::query(UPDATE_PET_FED)
            .bind(fed)
            .bind(item_obj_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Forgets a pet whose collar is gone.
    // Java: Pet.destroyControlItem
    pub async fn delete_pet(&self, item_obj_id: i32) -> Result<()> {
        sqlx::query(DELETE_PET)
            .bind(item_obj_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// `(ownerId, item_obj_id)` of every pet that comes back after a relog.
    // Java: CharSummonTable.init
    pub async fn restoring_pets(&self) -> Result<Vec<(i32, u32)>> {
        let rows = sqlx::query_as::<_, (i32, u32)>(INIT_PET)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// Whether a pet of this collar item already has that name.
    // Java: PetNameTable.doesPetNameExist
    pub async fn pet_name_exists(&self, name: &str, collar_item_id: i32) -> Result<bool> {
        let row = sqlx::query_as::<_, (String,)>(CHECK_PET_NAME)
            .bind(name)
            .bind(collar_item_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.is_some())
    }

    /// `(name, collar item id)` of every named pet.
    // Java: PetNameTable.doesPetNameExist (read once, at startup)
    pub async fn all_pet_names(&self) -> Result<Vec<(String, i32)>> {
        let rows = sqlx::query_as::<_, (String, i32)>(ALL_PET_NAMES)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// The saved buffs of one pet, in `buff_index` order.
    // Java: Pet.restoreEffects
    pub async fn effects_of(&self, pet_obj_item_id: i32) -> Result<Vec<PetSkillSaveRow>> {
        let rows = sqlx::query_as::<_, PetSkillSaveRow>(RESTORE_SKILL_SAVE)
            .bind(pet_obj_item_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// The saved buffs of every collar a character holds, by collar and `buff_index`.
    // Java: Pet.restoreEffects (all collars at once)
    pub async fn effects_of_owner(&self, owner_id: i32) -> Result<Vec<PetSkillSaveRow>> {
        let rows = sqlx::query_as::<_, PetSkillSaveRow>(RESTORE_SKILL_SAVE_OF_OWNER)
            .bind(owner_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// Replaces the saved buffs of one pet: the `DELETE`, then the rows (none = only the delete),
    /// on one connection like Java's batch.
    // Java: Pet.storeEffect, Pet.restoreEffects (the DELETE)
    pub async fn store_effects(&self, pet_obj_item_id: i32, rows: &[NewPetSkillSave]) -> Result<()> {
        let mut conn = self.pool.acquire().await?;
        sqlx::query(DELETE_SKILL_SAVE)
            .bind(pet_obj_item_id)
            .execute(&mut *conn)
            .await?;
        for (index, row) in rows.iter().enumerate() {
            let buff_index = i32::try_from(index + 1).unwrap_or(i32::MAX);
            sqlx::query(ADD_SKILL_SAVE)
                .bind(pet_obj_item_id)
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
        assert_eq!(RESTORE_PET.matches('?').count(), 1);
        assert_eq!(RESTORE_PETS_OF_OWNER.matches('?').count(), 1);
        assert_eq!(INSERT_PET.matches('?').count(), 10);
        assert_eq!(UPDATE_PET.matches('?').count(), 10);
        assert_eq!(DELETE_PET.matches('?').count(), 1);
        assert_eq!(UPDATE_PET_FED.matches('?').count(), 2);
        assert_eq!(CHECK_PET_NAME.matches('?').count(), 2);
        assert_eq!(ADD_SKILL_SAVE.matches('?').count(), 5);
        assert_eq!(RESTORE_SKILL_SAVE.matches('?').count(), 1);
        assert_eq!(RESTORE_SKILL_SAVE_OF_OWNER.matches('?').count(), 1);
        assert_eq!(DELETE_SKILL_SAVE.matches('?').count(), 1);
    }
}
