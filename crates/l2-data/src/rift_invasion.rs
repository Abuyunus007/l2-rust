//! The zones of the rift invasions (L2Rust's own feature, D-64): `data/RiftInvasion.xml`.
//!
//! Each `<zone>` has a name, a level range, the points a rift may open at, the monsters of each
//! wave and the rift boss. The script `custom/RiftInvasion` reads them through the world.

use std::path::Path;

use serde::Deserialize;

use crate::loader::{DataError, load_file, log_loaded};

/// One wave of a zone: `count` monsters, each one picked from `mobs`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiftWaveDef {
    pub count: i32,
    pub mobs: Vec<i32>,
}

/// One `<zone>`.
#[derive(Debug, Clone, PartialEq)]
pub struct RiftZoneDef {
    pub name: String,
    pub min_level: i32,
    pub max_level: i32,
    /// The points a rift may open at (`x, y, z`).
    pub spawns: Vec<(i32, i32, i32)>,
    /// In file order: wave 1 first. A rift with more waves than the zone has repeats the last.
    pub waves: Vec<RiftWaveDef>,
    pub boss_id: i32,
    /// Factor on the boss's max HP (`bossHpMultiplier`, default 1).
    pub boss_hp_multiplier: f64,
}

/// All the zones, in file order.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct RiftZones {
    zones: Vec<RiftZoneDef>,
}

impl RiftZones {
    const LOADER: &'static str = "RiftInvasion";

    /// Loads `<root>/data/RiftInvasion.xml`. A missing file is a warning and no zones (the rift
    /// invasions then stay off).
    pub fn load(root: &Path) -> Self {
        let mut table = Self::default();
        load_file(&root.join("data/RiftInvasion.xml"), |doc: RawList| {
            table.add(doc);
            Ok(())
        });
        log_loaded(Self::LOADER, table.zones.len(), "rift zone(s)");
        table
    }

    /// Builds the table from XML text. For tests and tools.
    pub fn from_xml(xml: &str) -> Result<Self, DataError> {
        let doc: RawList = quick_xml::de::from_str(xml).map_err(|source| DataError::Xml {
            path: "<inline>".into(),
            source,
        })?;
        let mut table = Self::default();
        table.add(doc);
        Ok(table)
    }

    fn add(&mut self, doc: RawList) {
        for zone in doc.zones {
            let waves: Vec<RiftWaveDef> = zone
                .waves
                .into_iter()
                .map(|w| RiftWaveDef {
                    count: w.count,
                    mobs: w.mobs.into_iter().map(|m| m.id).collect(),
                })
                .filter(|w| w.count > 0 && !w.mobs.is_empty())
                .collect();
            let spawns: Vec<(i32, i32, i32)> = zone.spawns.into_iter().map(|s| (s.x, s.y, s.z)).collect();
            // A zone without a point, a wave or a boss cannot open a rift.
            if spawns.is_empty() || waves.is_empty() || zone.boss_id <= 0 {
                tracing::warn!(
                    "{}: zone '{}' has no spawn point, wave or boss, skipped.",
                    Self::LOADER,
                    zone.name
                );
                continue;
            }
            self.zones.push(RiftZoneDef {
                name: zone.name,
                min_level: zone.min_level,
                max_level: zone.max_level.max(zone.min_level),
                spawns,
                waves,
                boss_id: zone.boss_id,
                boss_hp_multiplier: zone.boss_hp_multiplier.unwrap_or(1.0).max(0.1),
            });
        }
    }

    pub fn len(&self) -> usize {
        self.zones.len()
    }

    pub fn is_empty(&self) -> bool {
        self.zones.is_empty()
    }

    /// Every zone, in file order.
    pub fn zones(&self) -> &[RiftZoneDef] {
        &self.zones
    }

    /// Every zone, in file order.
    pub fn into_zones(self) -> Vec<RiftZoneDef> {
        self.zones
    }
}

#[derive(Debug, Deserialize)]
struct RawList {
    #[serde(rename = "zone", default)]
    zones: Vec<RawZone>,
}

#[derive(Debug, Deserialize)]
struct RawZone {
    #[serde(rename = "@name")]
    name: String,
    #[serde(rename = "@minLevel")]
    min_level: i32,
    #[serde(rename = "@maxLevel")]
    max_level: i32,
    #[serde(rename = "@bossId")]
    boss_id: i32,
    #[serde(rename = "@bossHpMultiplier", default)]
    boss_hp_multiplier: Option<f64>,
    #[serde(rename = "spawn", default)]
    spawns: Vec<RawSpawn>,
    #[serde(rename = "wave", default)]
    waves: Vec<RawWave>,
}

#[derive(Debug, Deserialize)]
struct RawSpawn {
    #[serde(rename = "@x")]
    x: i32,
    #[serde(rename = "@y")]
    y: i32,
    #[serde(rename = "@z")]
    z: i32,
}

#[derive(Debug, Deserialize)]
struct RawWave {
    #[serde(rename = "@count")]
    count: i32,
    #[serde(rename = "mob", default)]
    mobs: Vec<RawMob>,
}

#[derive(Debug, Deserialize)]
struct RawMob {
    #[serde(rename = "@id")]
    id: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<list>
	<!-- a comment -->
	<zone name="Test Field" minLevel="40" maxLevel="46" bossId="25335" bossHpMultiplier="0.5">
		<spawn x="1" y="2" z="3" />
		<spawn x="4" y="5" z="-6" />
		<wave count="6"><mob id="21682" /><mob id="21687" /></wave>
		<wave count="8"><mob id="21695" /></wave>
	</zone>
	<zone name="No Boss" minLevel="60" maxLevel="66" bossId="0">
		<spawn x="1" y="2" z="3" />
		<wave count="6"><mob id="21682" /></wave>
	</zone>
	<zone name="Empty Waves" minLevel="70" maxLevel="60" bossId="25338">
		<spawn x="1" y="2" z="3" />
		<wave count="0"><mob id="21682" /></wave>
	</zone>
</list>"#;

    #[test]
    fn a_zone_has_its_points_waves_and_boss() {
        let zones = RiftZones::from_xml(SAMPLE).unwrap();
        // The zone without a boss and the zone without a usable wave are skipped.
        assert_eq!(zones.len(), 1);
        let zone = &zones.zones()[0];
        assert_eq!(zone.name, "Test Field");
        assert_eq!((zone.min_level, zone.max_level, zone.boss_id), (40, 46, 25335));
        assert_eq!(zone.boss_hp_multiplier, 0.5);
        assert_eq!(zone.spawns, vec![(1, 2, 3), (4, 5, -6)]);
        assert_eq!(
            zone.waves,
            vec![
                RiftWaveDef {
                    count: 6,
                    mobs: vec![21682, 21687]
                },
                RiftWaveDef {
                    count: 8,
                    mobs: vec![21695]
                },
            ]
        );
    }

    #[test]
    fn the_shipped_file_has_at_least_four_zones_over_the_level_ranges() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/game");
        let zones = RiftZones::load(&root);
        assert!(zones.len() >= 4, "{} zones", zones.len());
        for band in [40, 60, 70, 80] {
            assert!(
                zones.zones().iter().any(|z| z.min_level / 10 * 10 == band),
                "no zone of the {band}s"
            );
        }
    }
}
