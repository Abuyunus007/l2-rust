//! The cursed weapons: `data/CursedWeapons.xml` (M8-CURSED-01).
//!
//! Java: `managers/CursedWeaponsManager.java` (`load`). The definitions are immutable; where a
//! cursed weapon is, who owns it and how long it lasts is the world's (`l2_world::cursed`).
//!
//! # The order of Java's `HashMap`
//! `CursedWeaponsManager._cursedWeapons` is a `HashMap<Integer, CursedWeapon>` keyed by the item
//! id. `checkDrop` tries the weapons in its iteration order and stops at the first that drops, and
//! the admin pages list them in it, so [`CursedWeaponDefs::iter`] gives that order (the same
//! helper as the manor seeds).

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use crate::loader::{DataError, load_file, log_loaded};
use crate::seed::java_hash_order;

/// One `<item>` of `CursedWeapons.xml` (Java `CursedWeapon`, the fields `load` sets).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CursedWeaponDef {
    /// `id`: the item id of the weapon.
    pub item_id: i32,
    /// `skillId`: the passive skill of the owner.
    pub skill_id: i32,
    /// `name`.
    pub name: String,
    /// `dropRate`: out of 100000 per kill of a monster.
    pub drop_rate: i32,
    /// `duration`: minutes the weapon lives.
    pub duration: i32,
    /// `durationLost`: minutes a kill takes off the weapon's life; also the period of the check
    /// (`durationLost * 12000` ms).
    pub duration_lost: i32,
    /// `disapearChance` (sic): percent that the weapon disappears instead of dropping when its
    /// owner dies.
    pub disapear_chance: i32,
    /// `stageKills`: kills per skill level.
    pub stage_kills: i32,
}

/// All cursed weapons. Immutable after loading. Java `CursedWeaponsManager.load`.
#[derive(Debug, Default, Clone)]
pub struct CursedWeaponDefs {
    by_id: BTreeMap<i32, CursedWeaponDef>,
    java_order: Vec<i32>,
}

impl CursedWeaponDefs {
    const LOADER: &'static str = "CursedWeaponsManager";

    /// Loads `<root>/data/CursedWeapons.xml`. A missing file is Java's warning and an empty list.
    pub fn load(root: &Path) -> Self {
        let mut table = Self::default();
        let mut inserted = Vec::new();
        load_file(&root.join("data/CursedWeapons.xml"), |doc: RawList| {
            table.add(doc, &mut inserted);
            Ok(())
        });
        table.java_order = java_hash_order(&inserted);
        log_loaded(Self::LOADER, table.by_id.len(), "cursed weapon(s)");
        table
    }

    /// Builds the table from XML text. For tests and tools.
    pub fn from_xml(xml: &str) -> Result<Self, DataError> {
        let doc: RawList = quick_xml::de::from_str(xml).map_err(|source| DataError::Xml {
            path: "<inline>".into(),
            source,
        })?;
        let mut table = Self::default();
        let mut inserted = Vec::new();
        table.add(doc, &mut inserted);
        table.java_order = java_hash_order(&inserted);
        Ok(table)
    }

    fn add(&mut self, doc: RawList, inserted: &mut Vec<i32>) {
        for raw in doc.items {
            let def = CursedWeaponDef {
                item_id: raw.id,
                skill_id: raw.skill_id,
                name: raw.name,
                drop_rate: last_val(&raw.drop_rate),
                duration: last_val(&raw.duration),
                duration_lost: last_val(&raw.duration_lost),
                disapear_chance: last_val(&raw.disapear_chance),
                stage_kills: last_val(&raw.stage_kills),
            };
            let id = def.item_id;
            // `_cursedWeapons.put(id, cw)`: a repeated id replaces the value and keeps its place.
            if self.by_id.insert(id, def).is_none() {
                inserted.push(id);
            }
        }
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    /// Java `getCursedWeapon(itemId)` (the definition).
    pub fn get(&self, item_id: i32) -> Option<&CursedWeaponDef> {
        self.by_id.get(&item_id)
    }

    /// The weapons in the order of Java's `_cursedWeapons.values()`.
    pub fn iter(&self) -> impl Iterator<Item = &CursedWeaponDef> {
        self.java_order.iter().filter_map(|id| self.by_id.get(id))
    }
}

/// A repeated child element sets the value again, so the last one counts; a missing one leaves
/// the field at 0 (Java's `int` default).
fn last_val(values: &[RawVal]) -> i32 {
    values.last().map_or(0, |v| v.val)
}

#[derive(Debug, Deserialize)]
struct RawList {
    #[serde(rename = "item", default)]
    items: Vec<RawItem>,
}

#[derive(Debug, Deserialize)]
struct RawItem {
    #[serde(rename = "@id")]
    id: i32,
    #[serde(rename = "@skillId")]
    skill_id: i32,
    #[serde(rename = "@name")]
    name: String,
    #[serde(rename = "dropRate", default)]
    drop_rate: Vec<RawVal>,
    #[serde(rename = "duration", default)]
    duration: Vec<RawVal>,
    #[serde(rename = "durationLost", default)]
    duration_lost: Vec<RawVal>,
    #[serde(rename = "disapearChance", default)]
    disapear_chance: Vec<RawVal>,
    #[serde(rename = "stageKills", default)]
    stage_kills: Vec<RawVal>,
}

#[derive(Debug, Deserialize)]
struct RawVal {
    #[serde(rename = "@val")]
    val: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<list>
	<item id="8190" name="Demonic Sword Zariche" skillId="3603">
		<disapearChance val="50" /> <!-- in % -->
		<dropRate val="1" />
		<duration val="300" />
		<durationLost val="3" />
		<stageKills val="10" />
	</item>
	<item id="8689" name="Blood Sword Akamanah" skillId="3629">
		<disapearChance val="50" />
		<dropRate val="1" />
		<duration val="300" />
		<durationLost val="3" />
		<stageKills val="10" />
	</item>
</list>"#;

    #[test]
    fn parses_both_weapons_in_java_hash_order() {
        let defs = CursedWeaponDefs::from_xml(XML).unwrap();
        assert_eq!(defs.len(), 2);
        let zariche = defs.get(8190).unwrap();
        assert_eq!(zariche.name, "Demonic Sword Zariche");
        assert_eq!(zariche.skill_id, 3603);
        assert_eq!(zariche.drop_rate, 1);
        assert_eq!(zariche.duration, 300);
        assert_eq!(zariche.duration_lost, 3);
        assert_eq!(zariche.disapear_chance, 50);
        assert_eq!(zariche.stage_kills, 10);
        // 8689 & 15 = 1 comes before 8190 & 15 = 14 in a HashMap with 16 buckets.
        let order: Vec<i32> = defs.iter().map(|d| d.item_id).collect();
        assert_eq!(order, vec![8689, 8190]);
    }

    #[test]
    fn the_shipped_file_loads_when_the_reference_folder_exists() {
        let root = Path::new("C:/l2-rust/l2jmobius/dist/game");
        if !root.join("data/CursedWeapons.xml").exists() {
            eprintln!("skipped: the reference datapack is not here");
            return;
        }
        let defs = CursedWeaponDefs::load(root);
        assert_eq!(defs.len(), 2);
        assert_eq!(defs.get(8689).unwrap().skill_id, 3629);
    }
}
