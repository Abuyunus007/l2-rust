//! Armor set bonuses: the pieces that make a set, its set skills and its six stat bonuses.
//!
//! Java: `gameserver/data/xml/ArmorSetData.java` and `gameserver/data/holders/ArmorSet.java`.
//! Data: `data/stats/armorsets/*.xml` (`xsd/armorSets.xsd`). Only that folder is read, not its
//! subfolders (Java `parseDatapackDirectory(..., false)`).
//!
//! Only the data and the lookup by chest id are here. The checks Java runs on a player's
//! paperdoll (`containAll`, `containShield`, `isEnchanted6`) and the effect of the bonuses are
//! M4-STAT-01.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use crate::loader::{DataError, load_dir, log_loaded};

/// A skill id with its level. Java `SkillHolder`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkillHolder {
    /// Skill id.
    pub id: i32,
    /// Skill level.
    pub level: i32,
}

/// One armor set. Java `ArmorSet`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ArmorSet {
    /// Chest item id. The set is looked up by it. 0 when the file has no `chest` element, which
    /// is what Java's default gives.
    pub chest_id: i32,
    /// Legs item ids. Any one of them completes the set. Empty means the set needs no legs.
    pub legs: Vec<i32>,
    /// Head item ids (same rule as `legs`).
    pub head: Vec<i32>,
    /// Gloves item ids (same rule as `legs`).
    pub gloves: Vec<i32>,
    /// Feet item ids (same rule as `legs`).
    pub feet: Vec<i32>,
    /// Shield item ids. The shield is not part of the whole-set check.
    pub shield: Vec<i32>,
    /// Skills while the whole set is worn (`skill` elements, in file order).
    pub skills: Vec<SkillHolder>,
    /// Skills while the set's shield is worn (`shield_skill` elements).
    pub shield_skills: Vec<SkillHolder>,
    /// Skills while the set is enchanted to +6 (`enchant6skill` elements).
    pub enchant6_skills: Vec<SkillHolder>,
    /// CON bonus (`con` element; 0 when absent).
    pub con: i32,
    /// DEX bonus (`dex` element; 0 when absent).
    pub dex: i32,
    /// STR bonus (`str` element; 0 when absent).
    pub str_: i32,
    /// MEN bonus (`men` element; 0 when absent).
    pub men: i32,
    /// WIT bonus (`wit` element; 0 when absent).
    pub wit: i32,
    /// INT bonus (`int` element; 0 when absent).
    pub int_: i32,
}

/// All armor sets, keyed by chest item id. Immutable after loading. Java `ArmorSetData`.
#[derive(Debug, Default)]
pub struct ArmorSets {
    sets: BTreeMap<i32, ArmorSet>,
}

impl ArmorSets {
    const LOADER: &'static str = "ArmorSetData";

    /// Loads every `.xml` file directly inside `<root>/data/stats/armorsets` and logs Java's count
    /// line (`ArmorSetData: Loaded N armor sets.`).
    ///
    /// NOTE(java-quirk): Java sizes its chest-id array with `Collections.max` of the keys, which
    /// throws on an empty map, so Java cannot start without at least one armor set. Here an empty
    /// table is loaded instead (the datapack loads with no armor sets, nothing crashes).
    pub fn load(root: &Path) -> Self {
        let mut table = Self::default();
        load_dir(
            &root.join("data/stats/armorsets"),
            false,
            |doc: RawArmorSetFile| table.add(doc),
        );
        log_loaded(Self::LOADER, table.sets.len(), "armor sets");
        table
    }

    /// Builds a table from the text of one armor set file. For tests and tools.
    pub fn from_xml(xml: &str) -> Result<Self, DataError> {
        let doc = quick_xml::de::from_str(xml).map_err(|source| DataError::Xml {
            path: "<inline>".into(),
            source,
        })?;
        let mut table = Self::default();
        table.add(doc)?;
        Ok(table)
    }

    fn add(&mut self, doc: RawArmorSetFile) -> Result<(), DataError> {
        for raw in doc.sets {
            let set = raw.into_set();
            // Java `_armorSetMap.put(chestId, set)`: a later set with the same chest id replaces
            // the earlier one.
            self.sets.insert(set.chest_id, set);
        }
        Ok(())
    }

    /// Java `getSet(chestId)`: the set whose chest item is `chest_id`.
    pub fn get(&self, chest_id: i32) -> Option<&ArmorSet> {
        self.sets.get(&chest_id)
    }

    /// Java `isArmorSet(chestId)`.
    pub fn is_armor_set(&self, chest_id: i32) -> bool {
        self.sets.contains_key(&chest_id)
    }

    /// All sets, ordered by chest id.
    pub fn iter(&self) -> impl Iterator<Item = &ArmorSet> {
        self.sets.values()
    }

    /// Number of sets (distinct chest ids).
    pub fn len(&self) -> usize {
        self.sets.len()
    }

    /// True when nothing was loaded.
    pub fn is_empty(&self) -> bool {
        self.sets.is_empty()
    }
}

// ---- Raw XML shape (xsd/armorSets.xsd) ------------------------------------------------------
// Java reads each piece element as it comes, so a repeated `chest` or stat element keeps the last
// value, while every `legs`/`head`/`skill`/... element adds to its list. The `Vec`s below keep
// that order.

#[derive(Deserialize)]
struct RawArmorSetFile {
    #[serde(rename = "set", default)]
    sets: Vec<RawSet>,
}

#[derive(Deserialize)]
struct RawSet {
    #[serde(default)]
    chest: Vec<RawId>,
    #[serde(default)]
    legs: Vec<RawId>,
    #[serde(default)]
    head: Vec<RawId>,
    #[serde(default)]
    gloves: Vec<RawId>,
    #[serde(default)]
    feet: Vec<RawId>,
    #[serde(default)]
    shield: Vec<RawId>,
    #[serde(default)]
    skill: Vec<RawSkill>,
    #[serde(default)]
    shield_skill: Vec<RawSkill>,
    #[serde(default)]
    enchant6skill: Vec<RawSkill>,
    #[serde(default)]
    con: Vec<RawValue>,
    #[serde(default)]
    dex: Vec<RawValue>,
    #[serde(rename = "str", default)]
    str_: Vec<RawValue>,
    #[serde(default)]
    men: Vec<RawValue>,
    #[serde(default)]
    wit: Vec<RawValue>,
    #[serde(rename = "int", default)]
    int_: Vec<RawValue>,
}

impl RawSet {
    fn into_set(self) -> ArmorSet {
        // Java: a set without a `chest` element keeps chest id 0.
        let last = |values: &[RawValue]| values.last().map_or(0, |v| v.val);
        ArmorSet {
            chest_id: self.chest.last().map_or(0, |c| c.id),
            legs: ids(&self.legs),
            head: ids(&self.head),
            gloves: ids(&self.gloves),
            feet: ids(&self.feet),
            shield: ids(&self.shield),
            skills: skills(&self.skill),
            shield_skills: skills(&self.shield_skill),
            enchant6_skills: skills(&self.enchant6skill),
            con: last(&self.con),
            dex: last(&self.dex),
            str_: last(&self.str_),
            men: last(&self.men),
            wit: last(&self.wit),
            int_: last(&self.int_),
        }
    }
}

fn ids(items: &[RawId]) -> Vec<i32> {
    items.iter().map(|item| item.id).collect()
}

fn skills(items: &[RawSkill]) -> Vec<SkillHolder> {
    items
        .iter()
        .map(|item| SkillHolder {
            id: item.id,
            level: item.level,
        })
        .collect()
}

#[derive(Deserialize)]
struct RawId {
    #[serde(rename = "@id")]
    id: i32,
}

#[derive(Deserialize)]
struct RawSkill {
    #[serde(rename = "@id")]
    id: i32,
    #[serde(rename = "@level")]
    level: i32,
}

#[derive(Deserialize)]
struct RawValue {
    #[serde(rename = "@val")]
    val: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<list>
	<set id="1">
		<chest id="23" />
		<legs id="2386" />
		<head id="43" />
		<skill id="3006" level="1" />
		<skill id="3500" level="1" />
	</set>
</list>"#;

    #[test]
    fn set_is_found_by_chest_id() {
        let sets = ArmorSets::from_xml(XML).unwrap();
        assert!(sets.is_armor_set(23));
        assert!(!sets.is_armor_set(43));
        let set = sets.get(23).unwrap();
        assert_eq!(set.legs, vec![2386]);
        assert_eq!(set.head, vec![43]);
        assert_eq!(set.skills.len(), 2);
    }
}
