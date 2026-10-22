//! Crafting recipes: what a recipe needs, what it makes, what it costs (HP or MP) and how likely
//! it is to work.
//!
//! Java: `gameserver/data/xml/RecipeData.java`, `gameserver/entity/item/recipe/RecipeList.java`,
//! `gameserver/data/holders/{RecipeHolder,RecipeStatHolder}.java`, `gameserver/data/enums/
//! StatType.java`. Data: `data/Recipes.xml` (`xsd/Recipes.xsd`).
//!
//! What a recipe does in the world (the recipe books, the maker, the manufacture shops) is
//! `l2_world::craft`.
//!
//! # Error policy (Java `RecipeData.parseDocument`)
//! - An `<item>` without `id`, `recipeId`, `name`, `craftLevel`, `type` or `successRate` is logged
//!   and skipped; the next one loads.
//! - A `statUse` or `altStatChange` whose `name` is not a `StatType` skips its item.
//! - Anything else Java reads with `Integer.parseInt` or a bare `getNamedItem(..).getNodeValue()`
//!   (a number that is not a number, a missing `value`, `ingredient` or `production` attribute)
//!   throws, which ends the file: the recipes read before it stay (the loader returns the error
//!   from the closure).
//! - A recipe without a `<production>` throws at `RecipeList`'s constructor (`StatSet.getInt`
//!   without a default), so it ends the file the same way.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;
use tracing::error;

use crate::loader::{DataError, load_file, log_loaded};

/// Java `StatType`: what a `statUse` / `altStatChange` entry names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatType {
    Hp,
    Mp,
    Xp,
    Sp,
    /// "Grab item modifier": multiplies how many ingredients one pass of the alternative
    /// crafting takes (`AltGameCreation`).
    Gim,
}

impl StatType {
    /// `Enum.valueOf(StatType.class, name)`: exact and case-sensitive, `None` where Java throws.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "HP" => Self::Hp,
            "MP" => Self::Mp,
            "XP" => Self::Xp,
            "SP" => Self::Sp,
            "GIM" => Self::Gim,
            _ => return None,
        })
    }
}

/// Java `RecipeHolder`: one line of a recipe, an item and how many of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecipeHolder {
    pub item_id: i32,
    pub quantity: i32,
}

/// Java `RecipeStatHolder`: a `statUse` or `altStatChange` entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecipeStat {
    pub kind: StatType,
    pub value: i32,
}

/// Java `RecipeList`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecipeList {
    /// `id`: the key of the recipe (what the recipe books and the shops store).
    pub id: i32,
    /// `craftLevel`: the crafting skill level it needs.
    pub level: i32,
    /// `recipeId`: the item id of the recipe item that teaches it.
    pub recipe_id: i32,
    /// `name`.
    pub recipe_name: String,
    /// `successRate`, in percent.
    pub success_rate: i32,
    /// `production` item id.
    pub item_id: i32,
    /// `production` count.
    pub count: i32,
    /// `productionRare` item id; 0 when the recipe has none (Java's field default).
    pub rare_item_id: i32,
    /// `productionRare` count.
    pub rare_count: i32,
    /// `productionRare` chance, in percent.
    pub rarity: i32,
    /// `type="dwarven"`; anything else is a common recipe.
    pub is_dwarven: bool,
    /// The `<ingredient>`s in file order.
    pub recipes: Vec<RecipeHolder>,
    /// The `<statUse>`s in file order.
    pub stat_use: Vec<RecipeStat>,
    /// The `<altStatChange>`s in file order.
    pub alt_stat_change: Vec<RecipeStat>,
}

/// Java `RecipeData`: the recipes by id. Immutable once loaded.
#[derive(Debug, Default)]
pub struct RecipeData {
    recipes: BTreeMap<i32, RecipeList>,
}

impl RecipeData {
    const LOADER: &'static str = "RecipeData";

    /// Loads `<root>/data/Recipes.xml` and logs Java's count line.
    pub fn load(root: &Path) -> Self {
        let mut table = Self::default();
        load_file(&root.join("data").join("Recipes.xml"), |doc: RawFile| {
            table.add(doc)
        });
        log_loaded(Self::LOADER, table.recipes.len(), "recipes");
        table
    }

    /// Builds a table from XML text. For tests and tools.
    pub fn from_xml(xml: &str) -> Result<Self, DataError> {
        let doc = quick_xml::de::from_str(xml).map_err(|source| DataError::Xml {
            path: "<inline>".into(),
            source,
        })?;
        let mut table = Self::default();
        table.add(doc)?;
        Ok(table)
    }

    // Java: RecipeData.parseDocument
    fn add(&mut self, doc: RawFile) -> Result<(), DataError> {
        for child in doc.children {
            let RawChild::Item(raw) = child else { continue };
            if let Some(recipe) = raw.into_recipe()? {
                // `_recipes.put(id, recipeList)`: a later recipe with the same id replaces the
                // earlier one.
                self.recipes.insert(recipe.id, recipe);
            }
        }
        Ok(())
    }

    /// Java `getRecipeList(listId)`.
    pub fn get(&self, id: i32) -> Option<&RecipeList> {
        self.recipes.get(&id)
    }

    /// Java `getRecipeByItemId(itemId)`: the recipe the recipe item teaches.
    // NOTE(java-quirk): Java walks a `HashMap` and returns the first match. Two recipes share the
    // recipe item 5008 in the shipped file; here the lower recipe id wins (the order a table of
    // that size iterates small keys in).
    pub fn by_item_id(&self, item_id: i32) -> Option<&RecipeList> {
        self.recipes.values().find(|r| r.recipe_id == item_id)
    }

    /// Java `getAllItemIds()`: the recipe item of every recipe, in recipe id order.
    pub fn all_item_ids(&self) -> Vec<i32> {
        self.recipes.values().map(|r| r.recipe_id).collect()
    }

    /// All recipes, ordered by id.
    pub fn iter(&self) -> impl Iterator<Item = &RecipeList> {
        self.recipes.values()
    }

    /// Number of recipes.
    pub fn len(&self) -> usize {
        self.recipes.len()
    }

    /// True when nothing was loaded.
    pub fn is_empty(&self) -> bool {
        self.recipes.is_empty()
    }
}

// ---- Raw XML shape (xsd/Recipes.xsd) --------------------------------------------------------
// Every attribute is read as text, like Java's `getNodeValue()`, so the parse rules above are
// Java's.

#[derive(Deserialize)]
struct RawFile {
    #[serde(rename = "$value", default)]
    children: Vec<RawChild>,
}

#[derive(Deserialize)]
enum RawChild {
    #[serde(rename = "item")]
    Item(Box<RawItem>),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct RawItem {
    #[serde(rename = "@id")]
    id: Option<String>,
    #[serde(rename = "@recipeId")]
    recipe_id: Option<String>,
    #[serde(rename = "@name")]
    name: Option<String>,
    #[serde(rename = "@craftLevel")]
    craft_level: Option<String>,
    #[serde(rename = "@type")]
    kind: Option<String>,
    #[serde(rename = "@successRate")]
    success_rate: Option<String>,
    #[serde(rename = "$value", default)]
    children: Vec<RawPart>,
}

#[derive(Deserialize)]
enum RawPart {
    #[serde(rename = "statUse")]
    StatUse(RawStat),
    #[serde(rename = "altStatChange")]
    AltStatChange(RawStat),
    #[serde(rename = "ingredient")]
    Ingredient(RawAmount),
    #[serde(rename = "production")]
    Production(RawAmount),
    #[serde(rename = "productionRare")]
    ProductionRare(RawAmount),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct RawStat {
    #[serde(rename = "@name")]
    name: Option<String>,
    #[serde(rename = "@value")]
    value: Option<String>,
}

#[derive(Deserialize)]
struct RawAmount {
    #[serde(rename = "@id")]
    id: Option<String>,
    #[serde(rename = "@count")]
    count: Option<String>,
    #[serde(rename = "@rarity")]
    rarity: Option<String>,
}

/// `Integer.parseInt(text)`: an optional sign and decimal digits that fit an `int`.
fn parse_int(what: &str, text: Option<&str>) -> Result<i32, DataError> {
    let text = text.ok_or_else(|| DataError::Invalid(format!("{what}: missing attribute")))?;
    let digits = text.strip_prefix(['+', '-']).unwrap_or(text);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(DataError::Invalid(format!("{what}: not an int: {text:?}")));
    }
    text.strip_prefix('+')
        .unwrap_or(text)
        .parse()
        .map_err(|_| DataError::Invalid(format!("{what}: not an int: {text:?}")))
}

impl RawItem {
    /// `Ok(None)` for an item Java skips, `Err` for one that ends the file.
    fn into_recipe(self) -> Result<Option<RecipeList>, DataError> {
        const LOADER: &str = "RecipeData";
        let Some(id_text) = self.id.as_deref() else {
            error!("{LOADER}: Missing id for recipe item, skipping");
            return Ok(None);
        };
        let id = parse_int("item id", Some(id_text))?;
        let Some(recipe_id) = self.recipe_id.as_deref() else {
            error!("{LOADER}: Missing recipeId for recipe item id: {id}, skipping");
            return Ok(None);
        };
        let recipe_id = parse_int("recipeId", Some(recipe_id))?;
        let Some(name) = self.name else {
            error!("{LOADER}: Missing name for recipe item id: {id}, skipping");
            return Ok(None);
        };
        let Some(level) = self.craft_level.as_deref() else {
            error!("{LOADER}: Missing level for recipe item id: {id}, skipping");
            return Ok(None);
        };
        let level = parse_int("craftLevel", Some(level))?;
        let Some(kind) = self.kind else {
            error!("{LOADER}: Missing type for recipe item id: {id}, skipping");
            return Ok(None);
        };
        let Some(success_rate) = self.success_rate.as_deref() else {
            error!("{LOADER}: Missing successRate for recipe item id: {id}, skipping");
            return Ok(None);
        };
        let success_rate = parse_int("successRate", Some(success_rate))?;

        let mut recipes = Vec::new();
        let mut stat_use = Vec::new();
        let mut alt_stat_change = Vec::new();
        let mut production: Option<(i32, i32)> = None;
        let mut rare: Option<(i32, i32, i32)> = None;
        for part in &self.children {
            match part {
                RawPart::StatUse(stat) | RawPart::AltStatChange(stat) => {
                    let is_use = matches!(part, RawPart::StatUse(_));
                    // `getNamedItem("name").getNodeValue()` and the `parseInt` of the value are
                    // outside Java's try block.
                    let name = stat
                        .name
                        .as_deref()
                        .ok_or_else(|| DataError::Invalid("statUse: missing name".into()))?;
                    let value = parse_int("statUse value", stat.value.as_deref())?;
                    let Some(kind) = StatType::from_name(name) else {
                        // `Enum.valueOf` throws inside the try: the item is skipped.
                        error!(
                            "{LOADER}: Error in {} parameter for recipe item id: {id}, skipping",
                            if is_use { "StatUse" } else { "AltStatChange" }
                        );
                        return Ok(None);
                    };
                    let entry = RecipeStat { kind, value };
                    if is_use {
                        stat_use.push(entry);
                    } else {
                        alt_stat_change.push(entry);
                    }
                }
                RawPart::Ingredient(amount) => {
                    recipes.push(RecipeHolder {
                        item_id: parse_int("ingredient id", amount.id.as_deref())?,
                        quantity: parse_int("ingredient count", amount.count.as_deref())?,
                    });
                }
                RawPart::Production(amount) => {
                    production = Some((
                        parse_int("production id", amount.id.as_deref())?,
                        parse_int("production count", amount.count.as_deref())?,
                    ));
                }
                RawPart::ProductionRare(amount) => {
                    rare = Some((
                        parse_int("productionRare id", amount.id.as_deref())?,
                        parse_int("productionRare count", amount.count.as_deref())?,
                        parse_int("productionRare rarity", amount.rarity.as_deref())?,
                    ));
                }
                RawPart::Other => {}
            }
        }

        // `set.getInt("itemId")` has no default: a recipe without `<production>` throws.
        let (item_id, count) = production
            .ok_or_else(|| DataError::Invalid(format!("recipe item id {id} has no production")))?;
        let (rare_item_id, rare_count, rarity) = rare.unwrap_or((0, 0, 0));
        Ok(Some(RecipeList {
            id,
            level,
            recipe_id,
            recipe_name: name,
            success_rate,
            item_id,
            count,
            rare_item_id,
            rare_count,
            rarity,
            is_dwarven: kind.eq_ignore_ascii_case("dwarven"),
            recipes,
            stat_use,
            alt_stat_change,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<list>
	<!-- a comment -->
	<item id="1" recipeId="1666" name="mk_wooden_arrow" craftLevel="1" type="dwarven" successRate="100">
		<ingredient id="1864" count="4" />
		<ingredient id="1869" count="2" />
		<production id="17" count="500" />
		<statUse name="MP" value="30" />
	</item>
	<item id="2" recipeId="1786" name="mk_rare" craftLevel="3" type="common" successRate="60">
		<ingredient id="2005" count="1" />
		<production id="3" count="1" />
		<productionRare id="910" count="2" rarity="20" />
		<statUse name="MP" value="30" />
		<statUse name="HP" value="5" />
		<altStatChange name="XP" value="7" />
		<altStatChange name="GIM" value="2" />
	</item>
	<item id="3" recipeId="1786" name="same_recipe_item" craftLevel="2" type="Dwarven" successRate="100">
		<production id="9" count="1" />
	</item>
</list>"#;

    #[test]
    fn reads_a_dwarven_recipe() {
        let data = RecipeData::from_xml(XML).unwrap();
        assert_eq!(data.len(), 3);
        let arrow = data.get(1).unwrap();
        assert_eq!(arrow.recipe_id, 1666);
        assert_eq!(arrow.recipe_name, "mk_wooden_arrow");
        assert_eq!(arrow.level, 1);
        assert!(arrow.is_dwarven);
        assert_eq!(arrow.success_rate, 100);
        assert_eq!((arrow.item_id, arrow.count), (17, 500));
        assert_eq!(
            arrow.recipes,
            vec![
                RecipeHolder {
                    item_id: 1864,
                    quantity: 4
                },
                RecipeHolder {
                    item_id: 1869,
                    quantity: 2
                }
            ]
        );
        assert_eq!(
            arrow.stat_use,
            vec![RecipeStat {
                kind: StatType::Mp,
                value: 30
            }]
        );
        // No `productionRare`: Java's int fields stay 0.
        assert_eq!((arrow.rare_item_id, arrow.rare_count, arrow.rarity), (0, 0, 0));
    }

    #[test]
    fn reads_rare_production_and_alt_stat_changes() {
        let data = RecipeData::from_xml(XML).unwrap();
        let rare = data.get(2).unwrap();
        assert!(!rare.is_dwarven);
        assert_eq!((rare.rare_item_id, rare.rare_count, rare.rarity), (910, 2, 20));
        assert_eq!(rare.stat_use.len(), 2);
        assert_eq!(rare.stat_use[1].kind, StatType::Hp);
        assert_eq!(
            rare.alt_stat_change,
            vec![
                RecipeStat {
                    kind: StatType::Xp,
                    value: 7
                },
                RecipeStat {
                    kind: StatType::Gim,
                    value: 2
                }
            ]
        );
    }

    #[test]
    fn type_is_matched_without_case() {
        let data = RecipeData::from_xml(XML).unwrap();
        assert!(data.get(3).unwrap().is_dwarven);
    }

    #[test]
    fn recipe_by_item_id_takes_the_lowest_id() {
        let data = RecipeData::from_xml(XML).unwrap();
        assert_eq!(data.by_item_id(1786).unwrap().id, 2);
        assert_eq!(data.by_item_id(1666).unwrap().id, 1);
        assert!(data.by_item_id(7).is_none());
        assert_eq!(data.all_item_ids(), vec![1666, 1786, 1786]);
    }

    #[test]
    fn an_item_with_a_missing_attribute_is_skipped() {
        let data = RecipeData::from_xml(
            r#"<list>
              <item id="1" name="x" craftLevel="1" type="dwarven" successRate="100"><production id="1" count="1"/></item>
              <item id="2" recipeId="5" name="y" craftLevel="1" type="dwarven" successRate="100"><production id="1" count="1"/></item>
            </list>"#,
        )
        .unwrap();
        assert_eq!(data.len(), 1);
        assert!(data.get(1).is_none());
        assert!(data.get(2).is_some());
    }

    #[test]
    fn an_unknown_stat_name_skips_the_item() {
        let data = RecipeData::from_xml(
            r#"<list>
              <item id="1" recipeId="5" name="x" craftLevel="1" type="dwarven" successRate="100">
                <statUse name="XYZ" value="3"/><production id="1" count="1"/>
              </item>
              <item id="2" recipeId="6" name="y" craftLevel="1" type="dwarven" successRate="100">
                <production id="1" count="1"/>
              </item>
            </list>"#,
        )
        .unwrap();
        assert!(data.get(1).is_none());
        assert!(data.get(2).is_some());
    }

    #[test]
    fn a_bad_number_ends_the_file_but_keeps_what_was_read() {
        let mut table = RecipeData::default();
        let doc: RawFile = quick_xml::de::from_str(
            r#"<list>
              <item id="1" recipeId="5" name="x" craftLevel="1" type="dwarven" successRate="100"><production id="1" count="1"/></item>
              <item id="2" recipeId="abc" name="y" craftLevel="1" type="dwarven" successRate="100"><production id="1" count="1"/></item>
              <item id="3" recipeId="7" name="z" craftLevel="1" type="dwarven" successRate="100"><production id="1" count="1"/></item>
            </list>"#,
        )
        .unwrap();
        assert!(table.add(doc).is_err());
        assert!(table.get(1).is_some());
        assert!(table.get(3).is_none());
    }

    #[test]
    fn a_recipe_without_production_ends_the_file() {
        let mut table = RecipeData::default();
        let doc: RawFile = quick_xml::de::from_str(
            r#"<list>
              <item id="1" recipeId="5" name="x" craftLevel="1" type="dwarven" successRate="100"><ingredient id="1" count="1"/></item>
            </list>"#,
        )
        .unwrap();
        assert!(table.add(doc).is_err());
        assert!(table.is_empty());
    }

}
