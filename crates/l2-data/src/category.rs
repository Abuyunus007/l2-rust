//! The class and creature categories: which ids belong to which `CategoryType`.
//!
//! Java: `gameserver/data/xml/CategoryData.java`, `data/enums/CategoryType.java`.
//! Data: `data/CategoryData.xml` (`xsd/CategoryData.xsd`).
//!
//! A category lists player class ids (`FIGHTER_GROUP`, `HEAL_GROUP`, ...) or NPC ids
//! (`SUMMON_NPC_GROUP`, `STRIDER`, ...). Java's `Creature.isInCategory(type)` asks it with the id
//! of the creature (the class id for a player, the template id for an NPC).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::Deserialize;
use tracing::warn;

use crate::loader::{DataError, load_file, log_loaded};
use crate::stats::CategoryType;

/// All categories, immutable after loading. Java `CategoryData`.
#[derive(Debug, Default, Clone)]
pub struct CategoryData {
    categories: BTreeMap<CategoryType, BTreeSet<i32>>,
}

impl CategoryData {
    const LOADER: &'static str = "CategoryData";

    /// Loads `<root>/data/CategoryData.xml` and logs `Loaded N categories.`.
    pub fn load(root: &Path) -> Self {
        let mut data = Self::default();
        load_file(&root.join("data/CategoryData.xml"), |doc: RawCategories| {
            data.add(doc);
            Ok(())
        });
        log_loaded(Self::LOADER, data.categories.len(), "categories");
        data
    }

    /// Builds the table from XML text. For tests and tools.
    pub fn from_xml(xml: &str) -> Result<Self, DataError> {
        let doc = quick_xml::de::from_str(xml).map_err(|source| DataError::Xml {
            path: "<inline>".into(),
            source,
        })?;
        let mut data = Self::default();
        data.add(doc);
        Ok(data)
    }

    /// Builds the table from `(category, ids)` pairs. For tests that need a few categories.
    pub fn from_lists(lists: impl IntoIterator<Item = (CategoryType, Vec<i32>)>) -> Self {
        Self {
            categories: lists
                .into_iter()
                .map(|(category, ids)| (category, ids.into_iter().collect()))
                .collect(),
        }
    }

    fn add(&mut self, doc: RawCategories) {
        for raw in doc.categories {
            let Some(category) = CategoryType::from_name(&raw.name) else {
                warn!("CategoryData: Can't find category by name :{}", raw.name);
                continue;
            };
            // `_categories.put(categoryType, ids)`: a second block of the same name replaces the
            // first one.
            self.categories.insert(category, raw.ids.into_iter().collect());
        }
    }

    /// Java `getCategoryByType(type)`: `None` when the file has no such category.
    pub fn get(&self, category: CategoryType) -> Option<&BTreeSet<i32>> {
        self.categories.get(&category)
    }

    /// Java `isInCategory(type, id)`. A category the file does not list answers `false` (Java also
    /// logs a warning).
    pub fn is_in_category(&self, category: CategoryType, id: i32) -> bool {
        match self.categories.get(&category) {
            Some(ids) => ids.contains(&id),
            None => {
                warn!("CategoryData: Can't find category type :{}", category.name());
                false
            }
        }
    }

    /// Number of categories.
    pub fn len(&self) -> usize {
        self.categories.len()
    }

    /// True when nothing was loaded.
    pub fn is_empty(&self) -> bool {
        self.categories.is_empty()
    }
}

// Raw XML shape (xsd/CategoryData.xsd).
#[derive(Deserialize)]
struct RawCategories {
    #[serde(rename = "category", default)]
    categories: Vec<RawCategory>,
}

#[derive(Deserialize)]
struct RawCategory {
    #[serde(rename = "@name")]
    name: String,
    #[serde(rename = "id", default)]
    ids: Vec<i32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<list xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:noNamespaceSchemaLocation="xsd/CategoryData.xsd">
	<category name="HEAL_GROUP">
		<id>16</id> <!-- bishop -->
		<id>30</id>
	</category>
	<category name="NOT_A_CATEGORY">
		<id>1</id>
	</category>
	<category name="KNIGHT_GROUP">
		<id>4</id>
	</category>
</list>"#;

    #[test]
    fn parses_known_categories_and_skips_unknown_names() {
        let data = CategoryData::from_xml(XML).unwrap();
        assert_eq!(data.len(), 2);
        assert!(data.is_in_category(CategoryType::HealGroup, 16));
        assert!(data.is_in_category(CategoryType::HealGroup, 30));
        assert!(!data.is_in_category(CategoryType::HealGroup, 4));
        assert!(data.is_in_category(CategoryType::KnightGroup, 4));
    }

    #[test]
    fn a_category_the_file_lacks_answers_false() {
        let data = CategoryData::from_xml(XML).unwrap();
        assert!(data.get(CategoryType::FighterGroup).is_none());
        assert!(!data.is_in_category(CategoryType::FighterGroup, 0));
        assert!(!CategoryData::default().is_in_category(CategoryType::HealGroup, 16));
    }

    #[test]
    fn a_repeated_block_replaces_the_first() {
        let data = CategoryData::from_xml(
            r#"<list><category name="HEAL_GROUP"><id>1</id></category>
            <category name="HEAL_GROUP"><id>2</id></category></list>"#,
        )
        .unwrap();
        assert!(!data.is_in_category(CategoryType::HealGroup, 1));
        assert!(data.is_in_category(CategoryType::HealGroup, 2));
    }

    /// Every block of the shipped file loads: no unknown name, no empty parse.
    #[test]
    fn the_shipped_file_loads_every_category() {
        let root = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game"));
        let path = root.join("data/CategoryData.xml");
        let Ok(text) = std::fs::read_to_string(&path) else {
            eprintln!("skipped: {} does not exist", path.display());
            return;
        };
        let blocks = text.matches("<category name=").count();
        let data = CategoryData::from_xml(&text).unwrap();
        // The file names each category once.
        assert_eq!(data.len(), blocks);
        for category in CategoryType::ALL {
            if text.contains(&format!("<category name=\"{}\">", category.name())) {
                assert!(data.get(*category).is_some(), "{}", category.name());
            }
        }
        // Samples that scripts rely on.
        assert!(data.is_in_category(CategoryType::HealGroup, 15));
        assert!(data.is_in_category(CategoryType::HealGroup, 50));
        assert!(!data.is_in_category(CategoryType::HealGroup, 16));
        assert!(data.is_in_category(CategoryType::KnightGroup, 32));
        assert!(data.is_in_category(CategoryType::FirstClassGroup, 0));
        assert!(!data.is_in_category(CategoryType::FirstClassGroup, 1));
    }
}
