//! Which classes each NPC teaches skills to.
//!
//! Java: `gameserver/data/xml/SkillLearnData.java` (parsing) and `NpcData.loadNpcsSkillLearn`
//! (copies the list into the NPC template as `getTeachInfo`), `NpcTemplate.canTeach`.
//! Data: `data/SkillLearn.xml`.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;
use tracing::warn;

use crate::class_list::ClassId;
use crate::loader::{DataError, load_file, log_loaded};

/// The classes every NPC teaches, immutable after loading. Java `SkillLearnData`.
#[derive(Debug, Default)]
pub struct SkillLearnData {
    by_npc: BTreeMap<i32, Vec<ClassId>>,
}

impl SkillLearnData {
    const LOADER: &'static str = "SkillLearnData";

    /// Loads `<root>/data/SkillLearn.xml` and logs Java's count line.
    pub fn load(root: &Path) -> Self {
        let mut data = Self::default();
        load_file(&root.join("data/SkillLearn.xml"), |doc: RawSkillLearnFile| {
            data.add(doc);
            Ok(())
        });
        log_loaded(Self::LOADER, data.by_npc.len(), "skill learn data");
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

    fn add(&mut self, doc: RawSkillLearnFile) {
        for npc in doc.npcs {
            let mut classes = Vec::new();
            for id in npc.class_ids {
                // Java `PlayerClass.getPlayerClass(id)` is null for an unknown id and the null stays
                // in the list; it can never equal a real class, so it is left out here.
                match u16::try_from(id).map(ClassId) {
                    Ok(class) if class.is_player_class() => classes.push(class),
                    _ => warn!("SkillLearnData: npc {} teaches unknown class id {id}.", npc.id),
                }
            }
            // `_skillLearn.put(id, classIds)`: a second entry for an NPC replaces the first.
            self.by_npc.insert(npc.id, classes);
        }
    }

    /// Java `getSkillLearnData(npcId)`: `None` for an NPC that is not listed.
    pub fn classes_of(&self, npc_id: i32) -> Option<&[ClassId]> {
        self.by_npc.get(&npc_id).map(Vec::as_slice)
    }

    /// Number of NPCs listed.
    pub fn len(&self) -> usize {
        self.by_npc.len()
    }

    /// True when no NPC is listed.
    pub fn is_empty(&self) -> bool {
        self.by_npc.is_empty()
    }
}

#[derive(Deserialize)]
struct RawSkillLearnFile {
    #[serde(rename = "npc", default)]
    npcs: Vec<RawNpc>,
}

#[derive(Deserialize)]
struct RawNpc {
    #[serde(rename = "@id")]
    id: i32,
    #[serde(rename = "classId", default)]
    class_ids: Vec<i32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<list>
	<npc id="30010"> <!-- Auron -->
		<classId>0</classId> <!-- Human Fighter -->
		<classId>1</classId>
		<classId>9999</classId>
	</npc>
	<npc id="30014"></npc>
	<npc id="30010">
		<classId>10</classId>
	</npc>
</list>"#;

    #[test]
    fn classes_are_listed_per_npc_and_the_last_entry_wins() {
        let data = SkillLearnData::from_xml(XML).unwrap();
        assert_eq!(data.len(), 2);
        assert_eq!(data.classes_of(30010), Some(&[ClassId(10)][..]));
        assert_eq!(data.classes_of(30014), Some(&[][..]));
        assert_eq!(data.classes_of(1), None);
    }

    #[test]
    fn unknown_class_ids_are_left_out() {
        let xml = r#"<list><npc id="1"><classId>0</classId><classId>9999</classId><classId>-3</classId></npc></list>"#;
        let data = SkillLearnData::from_xml(xml).unwrap();
        assert_eq!(data.classes_of(1), Some(&[ClassId(0)][..]));
    }
}
