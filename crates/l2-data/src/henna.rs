//! Hennas (dyes): the stat bonuses, the cost to add and remove them, and the classes that can wear
//! them.
//!
//! Java: `gameserver/data/xml/HennaData.java` and `gameserver/entity/item/Henna.java`.
//! Data: `data/stats/hennaList.xml` (`xsd/hennaList.xsd`).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::Deserialize;

use crate::class_list::ClassId;
use crate::loader::{DataError, load_file, log_loaded};

/// One henna (dye). Java `Henna`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Henna {
    /// Dye id: the key of the henna list (Java `getDyeId`).
    pub dye_id: i32,
    /// Item id of the dye item needed to draw it (Java `getDyeItemId`).
    pub dye_item_id: i32,
    /// The `dyeLevel` attribute. Java never reads it, so it is kept only for the data browser.
    pub level: i32,
    /// STR bonus (Java `getStatSTR`).
    pub stat_str: i32,
    /// CON bonus (Java `getStatCON`).
    pub stat_con: i32,
    /// DEX bonus (Java `getStatDEX`).
    pub stat_dex: i32,
    /// INT bonus (Java `getStatINT`).
    pub stat_int: i32,
    /// MEN bonus (Java `getStatMEN`).
    pub stat_men: i32,
    /// WIT bonus (Java `getStatWIT`).
    pub stat_wit: i32,
    /// Adena to pay to draw the dye (Java `getWearFee`).
    pub wear_fee: i32,
    /// Dye items needed to draw it (Java `getWearCount`).
    pub wear_count: i32,
    /// Adena to pay to remove it (Java `getCancelFee`).
    pub cancel_fee: i32,
    /// Dye items given back when it is removed (Java `getCancelCount`).
    pub cancel_count: i32,
    /// Classes that can wear this dye (Java `getAllowedWearClass`).
    pub allowed_classes: BTreeSet<ClassId>,
}

impl Henna {
    /// Java `isAllowedClass(playerClass)`.
    pub fn is_allowed_class(&self, class_id: ClassId) -> bool {
        self.allowed_classes.contains(&class_id)
    }
}

/// All hennas, keyed by dye id. Immutable after loading. Java `HennaData`.
#[derive(Debug, Default)]
pub struct Hennas {
    by_dye_id: BTreeMap<i32, Henna>,
}

impl Hennas {
    const LOADER: &'static str = "HennaData";

    /// Loads `<root>/data/stats/hennaList.xml` and logs Java's count line.
    pub fn load(root: &Path) -> Self {
        let mut table = Self::default();
        load_file(&root.join("data/stats/hennaList.xml"), |doc: RawHennaFile| {
            table.add(doc)
        });
        log_loaded(Self::LOADER, table.by_dye_id.len(), "henna data");
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

    fn add(&mut self, doc: RawHennaFile) -> Result<(), DataError> {
        for raw in doc.hennas {
            let henna = raw.into_henna()?;
            // Java `_hennaList.put(dyeId, henna)`: a later henna with the same dye id replaces the
            // earlier one.
            self.by_dye_id.insert(henna.dye_id, henna);
        }
        Ok(())
    }

    /// Java `getHenna(id)`: the dye with that dye id.
    pub fn get(&self, dye_id: i32) -> Option<&Henna> {
        self.by_dye_id.get(&dye_id)
    }

    /// Java `getHennaList(playerClass)`: every dye the class can wear, in dye id order (Java's
    /// `HashMap` order is not defined).
    pub fn henna_list(&self, class_id: ClassId) -> Vec<&Henna> {
        self.by_dye_id
            .values()
            .filter(|henna| henna.is_allowed_class(class_id))
            .collect()
    }

    /// All hennas, ordered by dye id.
    pub fn iter(&self) -> impl Iterator<Item = &Henna> {
        self.by_dye_id.values()
    }

    /// Number of hennas.
    pub fn len(&self) -> usize {
        self.by_dye_id.len()
    }

    /// True when nothing was loaded.
    pub fn is_empty(&self) -> bool {
        self.by_dye_id.is_empty()
    }
}

// ---- Raw XML shape (xsd/hennaList.xsd) ------------------------------------------------------

#[derive(Deserialize)]
struct RawHennaFile {
    #[serde(rename = "henna", default)]
    hennas: Vec<RawHenna>,
}

#[derive(Deserialize)]
struct RawHenna {
    #[serde(rename = "@dyeId")]
    dye_id: i32,
    #[serde(rename = "@dyeItemId")]
    dye_item_id: i32,
    #[serde(rename = "@dyeLevel", default)]
    level: i32,
    #[serde(rename = "stats", default)]
    stats: Vec<RawStats>,
    #[serde(rename = "wear", default)]
    wear: Vec<RawCount>,
    #[serde(rename = "cancel", default)]
    cancel: Vec<RawCount>,
    #[serde(rename = "classId", default)]
    class_ids: Vec<i32>,
}

impl RawHenna {
    fn into_henna(self) -> Result<Henna, DataError> {
        // Java `StatSet.getInt("wear_fee")` throws when the `wear` element is missing, and the
        // file stops there. The stat attributes default to 0 (`getInt(key, 0)`).
        let wear = self
            .wear
            .last()
            .ok_or_else(|| DataError::Invalid(format!("henna {} has no wear element", self.dye_id)))?;
        let cancel = self
            .cancel
            .last()
            .ok_or_else(|| DataError::Invalid(format!("henna {} has no cancel element", self.dye_id)))?;

        // Java copies each `stats` element's attributes into one set, so a later attribute
        // overrides an earlier one and a missing one keeps the earlier value.
        let mut stat = [0_i32; 6];
        for stats in &self.stats {
            for (slot, value) in stat.iter_mut().zip(stats.values()) {
                if let Some(value) = value {
                    *slot = value;
                }
            }
        }

        // Java `PlayerClass.getPlayerClass(id)` gives null for an id that is not a class; a null
        // never matches a real class, so such ids are dropped here.
        let allowed_classes = self
            .class_ids
            .iter()
            .filter_map(|id| u16::try_from(*id).ok().map(ClassId))
            .filter(|class_id| class_id.race().is_some())
            .collect();

        Ok(Henna {
            dye_id: self.dye_id,
            dye_item_id: self.dye_item_id,
            level: self.level,
            stat_str: stat[0],
            stat_con: stat[1],
            stat_dex: stat[2],
            stat_int: stat[3],
            stat_men: stat[4],
            stat_wit: stat[5],
            wear_fee: wear.fee,
            wear_count: wear.count,
            cancel_fee: cancel.fee,
            cancel_count: cancel.count,
            allowed_classes,
        })
    }
}

#[derive(Deserialize)]
struct RawStats {
    #[serde(rename = "@str")]
    str_: Option<i32>,
    #[serde(rename = "@con")]
    con: Option<i32>,
    #[serde(rename = "@dex")]
    dex: Option<i32>,
    #[serde(rename = "@int")]
    int_: Option<i32>,
    #[serde(rename = "@men")]
    men: Option<i32>,
    #[serde(rename = "@wit")]
    wit: Option<i32>,
}

impl RawStats {
    /// The six values in the order STR, CON, DEX, INT, MEN, WIT.
    fn values(&self) -> [Option<i32>; 6] {
        [self.str_, self.con, self.dex, self.int_, self.men, self.wit]
    }
}

#[derive(Deserialize)]
struct RawCount {
    #[serde(rename = "@count")]
    count: i32,
    #[serde(rename = "@fee")]
    fee: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<list>
	<henna dyeId="1" dyeName="dye_s1c3_d" dyeItemId="4445" dyeLevel="28">
		<stats str="1" con="-3" dex="0" int="0" men="0" wit="0" />
		<wear count="10" fee="37000" />
		<cancel count="5" fee="7400" />
		<classId>1</classId>
		<classId>11</classId>
	</henna>
</list>"#;

    #[test]
    fn henna_is_read_with_its_costs_and_classes() {
        let table = Hennas::from_xml(XML).unwrap();
        let henna = table.get(1).unwrap();
        assert_eq!((henna.dye_item_id, henna.level), (4445, 28));
        assert_eq!((henna.stat_str, henna.stat_con), (1, -3));
        assert_eq!((henna.wear_fee, henna.wear_count), (37000, 10));
        assert_eq!((henna.cancel_fee, henna.cancel_count), (7400, 5));
        assert!(henna.is_allowed_class(ClassId(11)));
        assert!(!henna.is_allowed_class(ClassId(2)));
    }
}
