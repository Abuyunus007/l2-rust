//! The crafting tables: `character_recipebook` (the recipes a character knows) and
//! `character_recipeshoplist` (the recipes it has for sale in its manufacture shop).
// Java: org/l2jmobius/gameserver/entity/actor/Player.java (insertNewRecipeData, deleteRecipeData,
// Java: restoreRecipeBook, storeRecipeShopList, restoreRecipeShopList)
//
// Same layout as shortcuts.rs: verbatim SQL, one method per statement. `character_recipebook.id`
// is a DECIMAL column, read through `SqlNumber`.

use sqlx::FromRow;

use super::sql_number::SqlNumber;
use crate::error::Result;
use crate::pool::Pool;

// Java: Player.insertNewRecipeData
const INSERT_RECIPE: &str = "INSERT INTO character_recipebook (charId, id, classIndex, type) values(?,?,?,?)";
// Java: Player.deleteRecipeData
const DELETE_RECIPE: &str = "DELETE FROM character_recipebook WHERE charId=? AND id=? AND classIndex=?";
// Java: Player.restoreRecipeBook (loadCommon = true)
const SELECT_RECIPES: &str = "SELECT id, type, classIndex FROM character_recipebook WHERE charId=?";
// Java: Player.DELETE_CHAR_RECIPE_SHOP
const DELETE_RECIPE_SHOP: &str = "DELETE FROM character_recipeshoplist WHERE charId=?";
// Java: Player.INSERT_CHAR_RECIPE_SHOP
const INSERT_RECIPE_SHOP: &str =
    "REPLACE INTO character_recipeshoplist (`charId`, `recipeId`, `price`, `index`) VALUES (?, ?, ?, ?)";
// Java: Player.RESTORE_CHAR_RECIPE_SHOP
const SELECT_RECIPE_SHOP: &str = "SELECT * FROM character_recipeshoplist WHERE charId=? ORDER BY `index`";

/// A row of `character_recipebook` as `restoreRecipeBook` reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecipeBookRow {
    /// `id`: the recipe list id.
    pub recipe_id: i32,
    /// `type`: 1 for a dwarven recipe, else common.
    pub kind: i32,
    /// `classIndex`: the class index a dwarven recipe belongs to (0 for common ones).
    pub class_index: i32,
}

#[derive(Debug, Clone, FromRow)]
struct RecipeBookDbRow {
    id: SqlNumber<i32>,
    #[sqlx(rename = "type")]
    kind: i32,
    #[sqlx(rename = "classIndex")]
    class_index: i8,
}

/// A row of `character_recipeshoplist`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecipeShopRow {
    pub recipe_id: i32,
    pub price: i64,
}

#[derive(Debug, Clone, FromRow)]
struct RecipeShopDbRow {
    #[sqlx(rename = "recipeId")]
    recipe_id: u32,
    price: u64,
}

/// Every query on the crafting tables.
#[derive(Debug, Clone)]
pub struct RecipeRepository {
    pool: Pool,
}

impl RecipeRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Java `Player.insertNewRecipeData`: a recipe joins the book.
    pub async fn insert_recipe(
        &self,
        char_id: i32,
        recipe_id: i32,
        class_index: i32,
        dwarven: bool,
    ) -> Result<()> {
        sqlx::query(INSERT_RECIPE)
            .bind(char_id)
            .bind(recipe_id)
            .bind(class_index)
            .bind(i32::from(dwarven))
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Java `Player.deleteRecipeData`.
    pub async fn delete_recipe(&self, char_id: i32, recipe_id: i32, class_index: i32) -> Result<()> {
        sqlx::query(DELETE_RECIPE)
            .bind(char_id)
            .bind(recipe_id)
            .bind(class_index)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Java `Player.restoreRecipeBook(true)`: every row of the character, in the database's order.
    pub async fn recipes_of(&self, char_id: i32) -> Result<Vec<RecipeBookRow>> {
        let rows = sqlx::query_as::<_, RecipeBookDbRow>(SELECT_RECIPES)
            .bind(char_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .into_iter()
            .map(|r| RecipeBookRow {
                recipe_id: r.id.0,
                kind: r.kind,
                class_index: i32::from(r.class_index),
            })
            .collect())
    }

    /// Java `Player.storeRecipeShopList`: the old rows go, then one `REPLACE` per recipe with the
    /// 1-based position as `index`.
    pub async fn store_shop(&self, char_id: i32, items: &[RecipeShopRow]) -> Result<()> {
        sqlx::query(DELETE_RECIPE_SHOP)
            .bind(char_id)
            .execute(&self.pool)
            .await?;
        for (index, item) in items.iter().enumerate() {
            sqlx::query(INSERT_RECIPE_SHOP)
                .bind(char_id)
                .bind(item.recipe_id)
                .bind(item.price)
                .bind(i32::try_from(index + 1).unwrap_or(i32::MAX))
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    /// Java `Player.restoreRecipeShopList`: the rows in `index` order.
    pub async fn shop_of(&self, char_id: i32) -> Result<Vec<RecipeShopRow>> {
        let rows = sqlx::query_as::<_, RecipeShopDbRow>(SELECT_RECIPE_SHOP)
            .bind(char_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .into_iter()
            .map(|r| RecipeShopRow {
                recipe_id: i32::try_from(r.recipe_id).unwrap_or(i32::MAX),
                price: i64::try_from(r.price).unwrap_or(i64::MAX),
            })
            .collect())
    }
}
