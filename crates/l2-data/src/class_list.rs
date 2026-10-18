//! The player class list: id, display name and parent of every class.
//!
//! Java: `gameserver/data/xml/ClassListData.java`, `ClassInfoHolder.java` and the id/race part of
//! `entity/actor/enums/player/PlayerClass.java`.
//! Data: `data/stats/players/classList.xml`.
//!
//! This is the simplest loader and shows the pattern from [`crate::loader`] in its smallest form.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use crate::loader::{DataError, load_file, log_loaded};

/// Java `PlayerClass.getId()`. Only ids that exist in Java's `PlayerClass` enum are valid, see
/// [`ClassId::race`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClassId(pub u16);

/// The playable races, with the numbers the client uses (Java `Race` ordinals). The non-playable
/// Java races (animal, beast, ...) are added by the NPC loader card when it needs them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Race {
    /// Java `Race.HUMAN`.
    Human = 0,
    /// Java `Race.ELF`.
    Elf = 1,
    /// Java `Race.DARK_ELF`.
    DarkElf = 2,
    /// Java `Race.ORC`.
    Orc = 3,
    /// Java `Race.DWARF`.
    Dwarf = 4,
    /// Java `Race.KAMAEL`.
    Kamael = 5,
}

impl ClassId {
    /// The race of this class, or `None` when the id is not a `PlayerClass` in Java
    /// (`PlayerClass.getPlayerClass` returns `null`). Ids are contiguous per race and level
    /// (1st/2nd class, then 3rd class), so ranges are enough.
    pub fn race(self) -> Option<Race> {
        match self.0 {
            0..=17 | 88..=98 => Some(Race::Human),
            18..=30 | 99..=105 => Some(Race::Elf),
            31..=43 | 106..=112 => Some(Race::DarkElf),
            44..=52 | 113..=116 => Some(Race::Orc),
            53..=57 | 117..=118 => Some(Race::Dwarf),
            123..=136 => Some(Race::Kamael),
            _ => None,
        }
    }
}

/// Java `ClassInfoHolder`.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassInfo {
    /// The class.
    pub class_id: ClassId,
    /// The class this one evolves from; `None` for base classes.
    pub parent: Option<ClassId>,
    /// Display name, e.g. `Human Fighter`.
    pub name: String,
}

/// All player classes, immutable after loading. Java `ClassListData`.
#[derive(Debug, Default)]
pub struct ClassList {
    classes: BTreeMap<ClassId, ClassInfo>,
}

impl ClassList {
    const LOADER: &'static str = "ClassListData";

    /// Loads `<root>/data/stats/players/classList.xml` and logs `Loaded N class data.`.
    pub fn load(root: &Path) -> Self {
        let mut list = Self::default();
        load_file(
            &root.join("data/stats/players/classList.xml"),
            |doc: RawClassList| list.add(doc),
        );
        log_loaded(Self::LOADER, list.classes.len(), "class data");
        list
    }

    /// Builds a list from XML text. For tests and tools.
    pub fn from_xml(xml: &str) -> Result<Self, DataError> {
        let doc = quick_xml::de::from_str(xml).map_err(|source| DataError::Xml {
            path: "<inline>".into(),
            source,
        })?;
        let mut list = Self::default();
        list.add(doc)?;
        Ok(list)
    }

    fn add(&mut self, doc: RawClassList) -> Result<(), DataError> {
        for raw in doc.classes {
            let class_id = u16::try_from(raw.class_id)
                .map(ClassId)
                .ok()
                .filter(|id| id.race().is_some())
                .ok_or_else(|| DataError::Invalid(format!("unknown class id {}", raw.class_id)))?;
            // NOTE(java-quirk): Java only looks up a parent when `parentClassId > 0`, so classes whose
            // parent is class 0 (Warrior, Human Knight, Rogue) get no parent. An unknown parent id
            // also becomes "no parent" instead of an error.
            let parent = raw
                .parent_class_id
                .filter(|&p| p > 0)
                .and_then(|p| u16::try_from(p).ok())
                .map(ClassId)
                .filter(|id| id.race().is_some());
            self.classes.insert(
                class_id,
                ClassInfo {
                    class_id,
                    parent,
                    name: raw.name,
                },
            );
        }
        Ok(())
    }

    /// Java `getClass(PlayerClass)`.
    pub fn get(&self, class_id: ClassId) -> Option<&ClassInfo> {
        self.classes.get(&class_id)
    }

    /// Java `getClass(int)`.
    pub fn get_by_id(&self, id: i32) -> Option<&ClassInfo> {
        u16::try_from(id).ok().and_then(|id| self.get(ClassId(id)))
    }

    /// All classes, ordered by id.
    pub fn iter(&self) -> impl Iterator<Item = &ClassInfo> {
        self.classes.values()
    }

    /// Number of classes.
    pub fn len(&self) -> usize {
        self.classes.len()
    }

    /// True when nothing was loaded.
    pub fn is_empty(&self) -> bool {
        self.classes.is_empty()
    }
}

// Raw XML shape (xsd/classList.xsd).
#[derive(Deserialize)]
struct RawClassList {
    #[serde(rename = "class", default)]
    classes: Vec<RawClass>,
}

#[derive(Deserialize)]
struct RawClass {
    #[serde(rename = "@classId")]
    class_id: i32,
    #[serde(rename = "@parentClassId")]
    parent_class_id: Option<i32>,
    #[serde(rename = "@name")]
    name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<list xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:noNamespaceSchemaLocation="../../xsd/classList.xsd">
	<class classId="0" name="Human Fighter" />
	<class classId="1" name="Warrior" parentClassId="0" />
	<class classId="2" name="Gladiator" parentClassId="1" />
	<class classId="88" name="Duelist" parentClassId="2" />
</list>"#;

    #[test]
    fn parses_classes_and_parents() {
        let list = ClassList::from_xml(XML).unwrap();
        assert_eq!(list.len(), 4);
        assert_eq!(list.get(ClassId(0)).unwrap().name, "Human Fighter");
        assert_eq!(list.get(ClassId(0)).unwrap().parent, None);
        assert_eq!(list.get(ClassId(2)).unwrap().parent, Some(ClassId(1)));
        assert_eq!(list.get_by_id(88).unwrap().parent, Some(ClassId(2)));
        assert!(list.get_by_id(3).is_none());
        assert!(list.get_by_id(-1).is_none());
    }

    #[test]
    fn parent_class_zero_means_no_parent_like_java() {
        let list = ClassList::from_xml(XML).unwrap();
        assert_eq!(list.get(ClassId(1)).unwrap().parent, None);
    }

    #[test]
    fn unknown_class_id_is_an_error() {
        let xml = r#"<list><class classId="58" name="Nobody"/></list>"#;
        assert!(matches!(ClassList::from_xml(xml), Err(DataError::Invalid(_))));
    }

    #[test]
    fn missing_required_attribute_is_an_error() {
        let xml = r#"<list><class classId="1"/></list>"#;
        assert!(matches!(ClassList::from_xml(xml), Err(DataError::Xml { .. })));
    }

    #[test]
    fn race_follows_the_id_ranges() {
        assert_eq!(ClassId(0).race(), Some(Race::Human));
        assert_eq!(ClassId(30).race(), Some(Race::Elf));
        assert_eq!(ClassId(31).race(), Some(Race::DarkElf));
        assert_eq!(ClassId(52).race(), Some(Race::Orc));
        assert_eq!(ClassId(118).race(), Some(Race::Dwarf));
        assert_eq!(ClassId(123).race(), Some(Race::Kamael));
        assert_eq!(ClassId(58).race(), None);
        assert_eq!(ClassId(87).race(), None);
        assert_eq!(ClassId(119).race(), None);
        assert_eq!(ClassId(137).race(), None);
    }
}
