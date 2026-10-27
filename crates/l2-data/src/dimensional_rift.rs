//! The monsters of the Dimensional Rift rooms: `data/DimensionalRift.xml` (M8-FEST-01).
//!
//! Java: `managers/DimensionalRiftManager.java` (`loadSpawns`). The rooms themselves are rows of
//! the `dimensional_rift` table; this file only says which monsters stand in which room.

use std::path::Path;

use serde::Deserialize;

use crate::loader::{DataError, load_file, log_loaded};

/// One `<spawn>` of a `<room>`: `count` monsters of `mob_id`, each with a respawn delay of
/// `delay` seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RiftSpawnDef {
    /// `<area type>`: 1 recruit ... 6 hero.
    pub area: i8,
    /// `<room id>`.
    pub room: i8,
    pub mob_id: i32,
    pub delay: i32,
    pub count: i32,
}

/// All the rift spawns, in file order (the order the rooms' spawns are made in).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RiftSpawns {
    spawns: Vec<RiftSpawnDef>,
}

impl RiftSpawns {
    const LOADER: &'static str = "DimensionalRiftManager";

    /// Loads `<root>/data/DimensionalRift.xml`. A missing file is Java's warning and no spawns.
    pub fn load(root: &Path) -> Self {
        let mut table = Self::default();
        load_file(&root.join("data/DimensionalRift.xml"), |doc: RawRift| {
            table.add(doc);
            Ok(())
        });
        log_loaded(Self::LOADER, table.spawns.len(), "dimensional rift spawn definition(s)");
        table
    }

    /// Builds the table from XML text. For tests and tools.
    pub fn from_xml(xml: &str) -> Result<Self, DataError> {
        let doc: RawRift = quick_xml::de::from_str(xml).map_err(|source| DataError::Xml {
            path: "<inline>".into(),
            source,
        })?;
        let mut table = Self::default();
        table.add(doc);
        Ok(table)
    }

    fn add(&mut self, doc: RawRift) {
        for area in doc.areas {
            for room in area.rooms {
                for spawn in room.spawns {
                    self.spawns.push(RiftSpawnDef {
                        area: area.kind,
                        room: room.id,
                        mob_id: spawn.mob_id,
                        delay: spawn.delay,
                        count: spawn.count,
                    });
                }
            }
        }
    }

    pub fn len(&self) -> usize {
        self.spawns.len()
    }

    pub fn is_empty(&self) -> bool {
        self.spawns.is_empty()
    }

    /// Every definition, in file order.
    pub fn iter(&self) -> impl Iterator<Item = &RiftSpawnDef> {
        self.spawns.iter()
    }
}

#[derive(Debug, Deserialize)]
struct RawRift {
    #[serde(rename = "area", default)]
    areas: Vec<RawArea>,
}

#[derive(Debug, Deserialize)]
struct RawArea {
    #[serde(rename = "@type")]
    kind: i8,
    #[serde(rename = "room", default)]
    rooms: Vec<RawRoom>,
}

#[derive(Debug, Deserialize)]
struct RawRoom {
    #[serde(rename = "@id")]
    id: i8,
    #[serde(rename = "spawn", default)]
    spawns: Vec<RawSpawn>,
}

#[derive(Debug, Deserialize)]
struct RawSpawn {
    #[serde(rename = "@mobId")]
    mob_id: i32,
    #[serde(rename = "@delay")]
    delay: i32,
    #[serde(rename = "@count")]
    count: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rift xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:noNamespaceSchemaLocation="xsd/DimensionalRift.xsd">
	<area type="1">
		<room id="1">
			<!-- Otherworldy Invader Soldier -->
			<spawn mobId="21659" count="2" delay="90" />
			<spawn mobId="21661" count="4" delay="80" />
		</room>
		<room id="2">
			<spawn mobId="21664" count="3" delay="90" />
		</room>
	</area>
	<area type="2">
		<room id="9">
			<spawn mobId="25001" count="1" delay="7" />
		</room>
	</area>
</rift>"#;

    #[test]
    fn parses_the_spawns_in_file_order() {
        let spawns = RiftSpawns::from_xml(XML).unwrap();
        let all: Vec<_> = spawns.iter().copied().collect();
        assert_eq!(
            all,
            vec![
                RiftSpawnDef { area: 1, room: 1, mob_id: 21659, delay: 90, count: 2 },
                RiftSpawnDef { area: 1, room: 1, mob_id: 21661, delay: 80, count: 4 },
                RiftSpawnDef { area: 1, room: 2, mob_id: 21664, delay: 90, count: 3 },
                RiftSpawnDef { area: 2, room: 9, mob_id: 25001, delay: 7, count: 1 },
            ]
        );
    }

    #[test]
    fn the_shipped_file_loads_when_the_reference_folder_exists() {
        let root = Path::new("C:/l2-rust/l2jmobius/dist/game");
        if !root.join("data/DimensionalRift.xml").exists() {
            eprintln!("skipped: the reference datapack is not here");
            return;
        }
        let spawns = RiftSpawns::load(root);
        assert!(spawns.len() > 100);
        assert!(spawns.iter().all(|s| (1..=6).contains(&s.area) && (1..=9).contains(&s.room)));
    }
}
