//! The skills the scheme buffer NPC offers.
//!
//! `entity/actor/holders/npc/BuffSkillHolder.java`.
//! Data: `data/SchemeBufferSkills.xml` (`xsd/SchemeBufferSkills.xsd`).
//!
//! The file lists categories (`Buffs`, `Dances`, ..., and the two hidden auto-buff groups
//! `MAGE_GROUP` / `FIGHTER_GROUP`) of skills with a level, a price and a text. The same skill id
//! may appear in several categories with different levels, so a lookup can be asked "in this
//! category" or "anywhere" (the highest level wins).

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

use crate::loader::{DataError, load_file, log_loaded};

/// Java `BuffSkillHolder`: one skill of a category.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuffSkillHolder {
    pub skill_id: i32,
    pub level: i32,
    pub price: i32,
    pub category: String,
    pub description: String,
}

/// One category: the skill ids in file order and the holder of each id (a later entry of the same
/// id replaces the earlier one, as Java's `LinkedHashMap.put`).
#[derive(Debug, Clone, Default)]
struct Category {
    name: String,
    ids: Vec<i32>,
    holders: HashMap<i32, BuffSkillHolder>,
}

/// Java `SchemeBufferTable`'s skill registry, immutable after loading.
#[derive(Debug, Clone, Default)]
pub struct SchemeBufferSkills {
    /// `_availableBuffs`: the holder with the highest level of each skill id.
    available: HashMap<i32, BuffSkillHolder>,
    /// `_skillTypesOrder` with the data of each category (names compare case-insensitively).
    categories: Vec<Category>,
    skill_count: usize,
}

impl SchemeBufferSkills {
    const LOADER: &'static str = "SchemeBufferTable";

    /// Loads `<root>/data/SchemeBufferSkills.xml`.
    pub fn load(root: &Path) -> Self {
        let mut data = Self::default();
        load_file(&root.join("data/SchemeBufferSkills.xml"), |doc: RawList| {
            data.add(doc);
            Ok(())
        });
        log_loaded(
            Self::LOADER,
            data.categories.len(),
            &format!("categories and {} entries", data.skill_count),
        );
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

    fn category_index(&self, name: &str) -> Option<usize> {
        self.categories
            .iter()
            .position(|c| c.name.eq_ignore_ascii_case(name))
    }

    fn add(&mut self, doc: RawList) {
        for raw in doc.categories {
            // `typeNode == null`: the category is skipped.
            let Some(name) = raw.name else { continue };
            let index = match self.category_index(&name) {
                Some(index) => index,
                None => {
                    self.categories.push(Category {
                        name: name.clone(),
                        ..Category::default()
                    });
                    self.categories.len() - 1
                }
            };
            for buff in raw.buffs {
                let (Some(skill_id), Some(level), Some(price)) = (buff.id, buff.level, buff.price) else {
                    continue;
                };
                let category = &mut self.categories[index];
                if !category.ids.contains(&skill_id) {
                    category.ids.push(skill_id);
                }
                let holder = BuffSkillHolder {
                    skill_id,
                    level,
                    price,
                    // Java passes the `type` text of the node.
                    category: name.clone(),
                    description: buff.desc.unwrap_or_default(),
                };
                category.holders.insert(skill_id, holder.clone());
                let replace = self
                    .available
                    .get(&skill_id)
                    .is_none_or(|existing| holder.level > existing.level);
                if replace {
                    self.available.insert(skill_id, holder);
                }
                self.skill_count += 1;
            }
        }
    }

    /// Java `getSkillsIdsByType(groupType)`: the ids of a category in file order (empty when the
    /// category is unknown).
    pub fn skill_ids_by_type(&self, group_type: &str) -> &[i32] {
        self.category_index(group_type)
            .map_or(&[], |i| self.categories[i].ids.as_slice())
    }

    /// Java `getSkillTypes()`: the category names in file order, without the two internal groups
    /// `MAGE_GROUP` and `FIGHTER_GROUP`.
    pub fn skill_types(&self) -> Vec<&str> {
        self.categories
            .iter()
            .map(|c| c.name.as_str())
            .filter(|n| !n.eq_ignore_ascii_case("MAGE_GROUP") && !n.eq_ignore_ascii_case("FIGHTER_GROUP"))
            .collect()
    }

    /// Java `getAvailableBuff(skillId)`: the holder with the highest level of the id.
    pub fn available_buff(&self, skill_id: i32) -> Option<&BuffSkillHolder> {
        self.available.get(&skill_id)
    }

    /// Java `getAvailableBuff(groupType, skillId)`: the holder of the category, else the global
    /// one.
    pub fn available_buff_in(&self, group_type: &str, skill_id: i32) -> Option<&BuffSkillHolder> {
        match self.category_index(group_type) {
            Some(i) => self.categories[i]
                .holders
                .get(&skill_id)
                .or_else(|| self.available.get(&skill_id)),
            None => self.available.get(&skill_id),
        }
    }

    /// Number of skill ids that have a global holder.
    pub fn len(&self) -> usize {
        self.available.len()
    }

    /// True when nothing was loaded.
    pub fn is_empty(&self) -> bool {
        self.available.is_empty()
    }
}

// Raw XML shape (xsd/SchemeBufferSkills.xsd).
#[derive(Deserialize)]
struct RawList {
    #[serde(rename = "category", default)]
    categories: Vec<RawCategory>,
}

#[derive(Deserialize)]
struct RawCategory {
    #[serde(rename = "@type")]
    name: Option<String>,
    #[serde(rename = "buff", default)]
    buffs: Vec<RawBuff>,
}

#[derive(Deserialize)]
struct RawBuff {
    #[serde(rename = "@id")]
    id: Option<i32>,
    #[serde(rename = "@level")]
    level: Option<i32>,
    #[serde(rename = "@price")]
    price: Option<i32>,
    #[serde(rename = "@desc")]
    desc: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<list xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:noNamespaceSchemaLocation="xsd/SchemeBufferSkills.xsd">
	<category type="Buffs">
		<buff id="1035" level="1" price="10" desc="Mental." /> <!-- Mental Shield -->
		<buff id="1036" level="2" price="20" desc="M. Def." />
		<buff id="1040" level="1" desc="no price: skipped" />
	</category>
	<category type="Songs">
		<buff id="1036" level="3" price="30" desc="Song." />
	</category>
	<category type="MAGE_GROUP">
		<buff id="1035" level="1" price="0" desc="" />
	</category>
</list>"#;

    /// The shipped file (`dist/game/data/SchemeBufferSkills.xml`) has the auto-buff groups and
    /// offers the visible categories.
    #[test]
    fn the_shipped_file_loads() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/game");
        if !root.join("data/SchemeBufferSkills.xml").is_file() {
            eprintln!("skipped: no dist/game snapshot");
            return;
        }
        let data = SchemeBufferSkills::load(&root);
        assert!(!data.skill_ids_by_type("MAGE_GROUP").is_empty());
        assert!(!data.skill_ids_by_type("FIGHTER_GROUP").is_empty());
        assert_eq!(data.skill_types().first(), Some(&"Buffs"));
        assert!(data.available_buff(1035).is_some());
    }

    #[test]
    fn categories_ids_and_holders() {
        let data = SchemeBufferSkills::from_xml(XML).unwrap();
        assert_eq!(data.skill_ids_by_type("buffs"), [1035, 1036]);
        assert_eq!(data.skill_ids_by_type("Songs"), [1036]);
        assert!(data.skill_ids_by_type("nothing").is_empty());
        assert_eq!(data.skill_types(), ["Buffs", "Songs"]);
        // The global holder keeps the highest level, the category one its own.
        assert_eq!(data.available_buff(1036).unwrap().level, 3);
        assert_eq!(data.available_buff_in("Buffs", 1036).unwrap().level, 2);
        assert_eq!(data.available_buff_in("Songs", 1035).unwrap().price, 10);
        assert!(data.available_buff(1040).is_none());
    }
}
