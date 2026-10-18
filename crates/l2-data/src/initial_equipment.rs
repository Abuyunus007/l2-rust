//! The items every new character of a class starts with.
//!
//! Java: `gameserver/data/xml/InitialEquipmentData.java` and `entity/item/holders/InitialEquipment.java`.
//! Data: `data/stats/players/initialEquipment.xml` (`xsd/initialEquipment.xsd`).
//!
//! Java has no fallback: a class without an `equipment` element gets no items, because
//! `CharacterCreate` skips the loop when the lookup returns null. [`InitialEquipments::get`] returns
//! `None` for the same case, and the caller gives nothing.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use crate::class_list::ClassId;
use crate::loader::{DataError, load_file, log_loaded};

/// One starting item. Java `InitialEquipment` (an `ItemHolder` plus the equipped flag).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitialEquipment {
    /// Item template id.
    pub item_id: i32,
    /// How many items the character gets.
    pub count: i64,
    /// Whether the item is put on at creation (Java only does it for equipable items).
    pub equipped: bool,
}

/// All starting equipment, by class. Java `InitialEquipmentData`.
#[derive(Debug, Default)]
pub struct InitialEquipments {
    classes: BTreeMap<ClassId, Vec<InitialEquipment>>,
}

impl InitialEquipments {
    const LOADER: &'static str = "InitialEquipmentData";

    /// Loads `<root>/data/stats/players/initialEquipment.xml` and logs Java's count line.
    pub fn load(root: &Path) -> Self {
        let mut table = Self::default();
        load_file(
            &root.join("data/stats/players/initialEquipment.xml"),
            |doc: RawEquipmentFile| table.add(doc),
        );
        log_loaded(Self::LOADER, table.classes.len(), "initial equipment data");
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

    fn add(&mut self, doc: RawEquipmentFile) -> Result<(), DataError> {
        for raw in doc.equipment {
            let class_id = player_class(raw.class_id)?;
            let items = raw
                .items
                .into_iter()
                .map(|item| InitialEquipment {
                    item_id: item.id,
                    count: item.count,
                    // Java StatSet.getBoolean: only the text "true" (any case) is true.
                    equipped: item
                        .equipped
                        .is_some_and(|value| value.eq_ignore_ascii_case("true")),
                })
                .collect();
            // Java replaces the list when a class appears twice.
            self.classes.insert(class_id, items);
        }
        Ok(())
    }

    /// Java `getClassEquipment(PlayerClass)`. `None` when the class has no entry.
    pub fn get(&self, class_id: ClassId) -> Option<&[InitialEquipment]> {
        self.classes.get(&class_id).map(Vec::as_slice)
    }

    /// Number of classes with starting equipment.
    pub fn len(&self) -> usize {
        self.classes.len()
    }

    /// True when nothing was loaded.
    pub fn is_empty(&self) -> bool {
        self.classes.is_empty()
    }
}

/// Java `PlayerClass.getPlayerClass(id)`: an id that is not a player class is an error (Java
/// puts `null` into its `EnumMap`, which throws).
fn player_class(id: i32) -> Result<ClassId, DataError> {
    u16::try_from(id)
        .ok()
        .map(ClassId)
        .filter(|class_id| class_id.race().is_some())
        .ok_or_else(|| DataError::Invalid(format!("unknown class id {id}")))
}

// Raw XML shape (xsd/initialEquipment.xsd).
#[derive(Deserialize)]
struct RawEquipmentFile {
    #[serde(rename = "equipment", default)]
    equipment: Vec<RawEquipment>,
}

#[derive(Deserialize)]
struct RawEquipment {
    #[serde(rename = "@classId")]
    class_id: i32,
    #[serde(rename = "item", default)]
    items: Vec<RawItem>,
}

#[derive(Deserialize)]
struct RawItem {
    #[serde(rename = "@id")]
    id: i32,
    #[serde(rename = "@count")]
    count: i64,
    #[serde(rename = "@equipped", default)]
    equipped: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<list>
	<equipment classId="0">
		<item id="2369" count="1" equipped="true" />
		<item id="10" count="1" />
		<item id="10650" count="5" />
	</equipment>
	<equipment classId="10">
		<item id="6" count="1" equipped="TRUE" />
		<item id="1" count="2" equipped="1" />
	</equipment>
</list>"#;

    #[test]
    fn parses_items_per_class() {
        let table = InitialEquipments::from_xml(XML).unwrap();
        assert_eq!(table.len(), 2);
        let fighter = table.get(ClassId(0)).unwrap();
        assert_eq!(fighter.len(), 3);
        assert_eq!(
            fighter[0],
            InitialEquipment {
                item_id: 2369,
                count: 1,
                equipped: true
            }
        );
        assert_eq!(fighter[2].count, 5);
        assert!(table.get(ClassId(1)).is_none());
    }

    #[test]
    fn equipped_is_only_true_like_java_parse_boolean() {
        let table = InitialEquipments::from_xml(XML).unwrap();
        let mystic = table.get(ClassId(10)).unwrap();
        assert!(mystic[0].equipped);
        assert!(!mystic[1].equipped);
        assert!(!table.get(ClassId(0)).unwrap()[1].equipped);
    }

    #[test]
    fn a_class_listed_twice_keeps_the_last_list() {
        let xml = XML.replace(
            "</list>",
            r#"<equipment classId="0"><item id="7" count="3"/></equipment></list>"#,
        );
        let table = InitialEquipments::from_xml(&xml).unwrap();
        assert_eq!(table.get(ClassId(0)).unwrap().len(), 1);
    }

    #[test]
    fn unknown_class_id_is_an_error() {
        let xml = r#"<list><equipment classId="58"/></list>"#;
        assert!(matches!(
            InitialEquipments::from_xml(xml),
            Err(DataError::Invalid(_))
        ));
    }
}
