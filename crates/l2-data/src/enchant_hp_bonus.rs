//! Enchant HP bonus: the HP an armor gets from its enchant level, per crystal grade.
//!
//! Java: `gameserver/data/xml/EnchantItemHPBonusData.java` (`getHPBonus`). Attaching the
//! `ENCHANTHP` function to the armors is M4-STAT-01.
//! Data: `data/stats/enchantHPBonus.xml` (`xsd/enchantHPBonus.xsd`).

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use crate::item::types::CrystalType;
use crate::loader::{DataError, load_file, log_loaded};

/// Java `FULL_ARMOR_MODIFIER`: full-body armor gets 1.5 times the bonus of one piece.
const FULL_ARMOR_MODIFIER: f32 = 1.5;

/// The HP bonus per enchant level for each crystal grade. Java `EnchantItemHPBonusData`.
#[derive(Debug, Default)]
pub struct EnchantHpBonus {
    /// `bonuses[grade]`: the bonus for enchant level 1, 2, ... (Java `List<Integer>`).
    bonuses: BTreeMap<CrystalType, Vec<i32>>,
}

impl EnchantHpBonus {
    const LOADER: &'static str = "EnchantItemHPBonusData";

    /// Loads `<root>/data/stats/enchantHPBonus.xml` and logs Java's count line.
    pub fn load(root: &Path) -> Self {
        let mut table = Self::default();
        load_file(
            &root.join("data/stats/enchantHPBonus.xml"),
            |doc: RawEnchantHpFile| table.add(doc),
        );
        log_loaded(Self::LOADER, table.bonuses.len(), "enchant HP bonuses");
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

    fn add(&mut self, doc: RawEnchantHpFile) -> Result<(), DataError> {
        for entry in doc.entries {
            // Java: `parseEnum` gives null for an unknown grade and `EnumMap.put(null, ..)` throws.
            let grade = CrystalType::from_name(&entry.grade)
                .ok_or_else(|| DataError::Invalid(format!("unknown crystal grade {}", entry.grade)))?;
            // Java `put`: a grade that appears twice keeps the later list.
            self.bonuses.insert(grade, entry.bonus);
        }
        Ok(())
    }

    /// Java `getHPBonus(item)` with the item's values passed in: `grade` is the template's crystal
    /// type, `enchant_level` is `Item.getOlyEnchantLevel()` (the caller applies the Olympiad cap),
    /// `full_armor` is `BodyPart.FULL_ARMOR`.
    pub fn hp_bonus(&self, grade: CrystalType, enchant_level: i32, full_armor: bool) -> i32 {
        // Java looks up `getCrystalTypePlus()`, so S80 and S84 use the S list.
        let Some(values) = self.bonuses.get(&grade.plus()) else {
            return 0;
        };
        if values.is_empty() || enchant_level <= 0 {
            return 0;
        }
        // Java: enchant levels above the list use its last entry.
        let index = usize::try_from(enchant_level)
            .unwrap_or(0)
            .min(values.len())
            .saturating_sub(1);
        let bonus = values.get(index).copied().unwrap_or(0);
        if full_armor {
            // Java `(int) (bonus * FULL_ARMOR_MODIFIER)`: float product, truncated toward zero.
            (bonus as f32 * FULL_ARMOR_MODIFIER) as i32
        } else {
            bonus
        }
    }

    /// Number of grades loaded.
    pub fn len(&self) -> usize {
        self.bonuses.len()
    }

    /// True when nothing was loaded.
    pub fn is_empty(&self) -> bool {
        self.bonuses.is_empty()
    }
}

// ---- Raw XML shape (xsd/enchantHPBonus.xsd) -------------------------------------------------

#[derive(Deserialize)]
struct RawEnchantHpFile {
    #[serde(rename = "enchantHP", default)]
    entries: Vec<RawEnchantHp>,
}

#[derive(Deserialize)]
struct RawEnchantHp {
    #[serde(rename = "@grade")]
    grade: String,
    #[serde(rename = "bonus", default)]
    bonus: Vec<i32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<list>
	<enchantHP grade="D">
		<bonus>0</bonus>
		<bonus>9</bonus>
		<bonus>26</bonus>
	</enchantHP>
</list>"#;

    #[test]
    fn enchant_level_indexes_the_grade_list() {
        let table = EnchantHpBonus::from_xml(XML).unwrap();
        assert_eq!(table.hp_bonus(CrystalType::D, 2, false), 9);
        // Above the list, the last entry is used.
        assert_eq!(table.hp_bonus(CrystalType::D, 9, false), 26);
        assert_eq!(table.hp_bonus(CrystalType::D, 0, false), 0);
        assert_eq!(table.hp_bonus(CrystalType::C, 2, false), 0);
    }

    #[test]
    fn full_armor_is_multiplied_and_truncated() {
        let table = EnchantHpBonus::from_xml(XML).unwrap();
        // 9 * 1.5 = 13.5, truncated to 13 like Java's (int) cast.
        assert_eq!(table.hp_bonus(CrystalType::D, 2, true), 13);
    }
}
