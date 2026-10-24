//! Fishing data: the fish, the fishing rods and the fishing monsters.
//!
//! Java: `gameserver/data/xml/{FishData,FishingRodsData,FishingMonstersData}.java` and
//! `gameserver/mechanics/fishing/{Fish,FishingRod,FishingMonster}.java`.
//! Data: `data/stats/fishing/{fishes,fishingRods,fishingMonsters}.xml`.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;
use tracing::warn;

use crate::loader::{DataError, load_file, log_loaded};

/// Java `Fish.getGroupId`: the group a fish swims in. The lures of `Player.getRandomFishGroup`
/// pick one of these ids.
pub fn fish_group_id(name: &str) -> i32 {
    match name {
        "swift" => 1,
        "ugly" => 2,
        "fish_box" => 3,
        "easy_wide" => 4,
        "easy_swift" => 5,
        "easy_ugly" => 6,
        "hard_wide" => 7,
        "hard_swift" => 8,
        "hard_ugly" => 9,
        // FIXME(java): "Verify the ID" says the Java comment.
        "hs_fish" => 10,
        // "wide" and every name Java does not know.
        _ => 0,
    }
}

/// Java `Fish.getGradeId`: `0` easy, `1` normal, `2` hard. Every unknown name is normal.
pub fn fish_grade_id(name: &str) -> i32 {
    match name {
        "fish_easy" => 0,
        "fish_hard" => 2,
        _ => 1,
    }
}

/// One fish (Java `Fish`). Java's `clone()` is `Clone`: a player's fish is a copy so the group can
/// be overwritten (`setFishGroup(-1)`) without touching the table.
#[derive(Debug, Clone, PartialEq)]
pub struct Fish {
    pub fish_id: i32,
    /// The item the player gets.
    pub item_id: i32,
    pub item_name: String,
    /// `Fish.getGroupId` of the `fishGroup` attribute.
    pub fish_group: i32,
    pub fish_level: i32,
    /// Java: "TODO: Support needed", never read.
    pub fish_bite_rate: f64,
    /// The percent chance, per check, that the fish takes the bait (`LookingForFishTask`).
    pub fish_guts: f64,
    pub fish_hp: i32,
    /// Java: "TODO: Support needed", never read.
    pub fish_max_length: i32,
    /// Java: "TODO: Support needed", never read.
    pub fish_length_rate: f64,
    pub hp_regen: f64,
    /// Seconds before the bait is lost when no fish bites.
    pub start_combat_time: i32,
    /// Seconds the fight lasts.
    pub combat_duration: i32,
    /// Base of the delay between two bite checks.
    pub guts_check_time: i32,
    /// Java: "TODO: Support needed", never read.
    pub guts_check_probability: f64,
    /// Java: "TODO: Support needed", never read.
    pub cheating_prob: f64,
    /// `Fish.getGradeId` of the `fishGrade` attribute.
    pub fish_grade: i32,
}

/// All fish, kept by grade like Java's three maps (`_fishEasy`, `_fishNormal`, `_fishHard`).
/// Java `FishData`.
#[derive(Debug, Default)]
pub struct FishData {
    easy: BTreeMap<i32, Fish>,
    normal: BTreeMap<i32, Fish>,
    hard: BTreeMap<i32, Fish>,
}

impl FishData {
    const LOADER: &'static str = "FishData";

    /// Loads `<root>/data/stats/fishing/fishes.xml` and logs Java's count line.
    pub fn load(root: &Path) -> Self {
        let mut table = Self::default();
        load_file(&root.join("data/stats/fishing/fishes.xml"), |doc: RawFishFile| {
            table.add(doc);
            Ok(())
        });
        log_loaded(Self::LOADER, table.len(), "fishes");
        table
    }

    /// Builds a table from XML text. For tests and tools.
    pub fn from_xml(xml: &str) -> Result<Self, DataError> {
        let doc = quick_xml::de::from_str(xml).map_err(|source| DataError::Xml {
            path: "<inline>".into(),
            source,
        })?;
        let mut table = Self::default();
        table.add(doc);
        Ok(table)
    }

    fn add(&mut self, doc: RawFishFile) {
        for raw in doc.fish {
            let fish = raw.into_fish();
            let map = match fish.fish_grade {
                0 => &mut self.easy,
                2 => &mut self.hard,
                _ => &mut self.normal,
            };
            map.insert(fish.fish_id, fish);
        }
    }

    /// Java `getFish(level, group, grade)`: the fish of that level and group in the grade's map.
    /// Java walks a `HashMap<Integer, Fish>`; its ids are below the table size, so the order is
    /// ascending by fish id, which is what a `BTreeMap` gives. An unknown grade or no fish at all
    /// gives an empty list and a warning.
    pub fn get_fish(&self, level: i32, group: i32, grade: i32) -> Vec<&Fish> {
        let map = match grade {
            0 => &self.easy,
            1 => &self.normal,
            2 => &self.hard,
            _ => {
                warn!("FishData: Unmanaged fish grade!");
                return Vec::new();
            }
        };
        let result: Vec<&Fish> = map
            .values()
            .filter(|f| f.fish_level == level && f.fish_group == group)
            .collect();
        if result.is_empty() {
            warn!("FishData: Cannot find any fish for level: {level} group: {group} and grade: {grade}!");
        }
        result
    }

    /// Number of fish in the three tables.
    pub fn len(&self) -> usize {
        self.easy.len() + self.normal.len() + self.hard.len()
    }

    /// True when nothing was loaded.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// One fishing rod (Java `FishingRod`).
#[derive(Debug, Clone, PartialEq)]
pub struct FishingRod {
    pub fishing_rod_id: i32,
    pub fishing_rod_item_id: i32,
    pub fishing_rod_level: i32,
    pub fishing_rod_name: String,
    pub fishing_rod_damage: f64,
}

/// The fishing rods by item id. Java `FishingRodsData`.
#[derive(Debug, Default)]
pub struct FishingRodsData {
    by_item_id: BTreeMap<i32, FishingRod>,
}

impl FishingRodsData {
    const LOADER: &'static str = "FishingRodsData";

    /// Loads `<root>/data/stats/fishing/fishingRods.xml` and logs Java's count line.
    pub fn load(root: &Path) -> Self {
        let mut table = Self::default();
        load_file(
            &root.join("data/stats/fishing/fishingRods.xml"),
            |doc: RawRodFile| {
                table.add(doc);
                Ok(())
            },
        );
        log_loaded(Self::LOADER, table.by_item_id.len(), "fishing rods");
        table
    }

    /// Builds a table from XML text. For tests and tools.
    pub fn from_xml(xml: &str) -> Result<Self, DataError> {
        let doc = quick_xml::de::from_str(xml).map_err(|source| DataError::Xml {
            path: "<inline>".into(),
            source,
        })?;
        let mut table = Self::default();
        table.add(doc);
        Ok(table)
    }

    fn add(&mut self, doc: RawRodFile) {
        for raw in doc.rods {
            let rod = FishingRod {
                fishing_rod_id: raw.fishing_rod_id,
                fishing_rod_item_id: raw.fishing_rod_item_id,
                fishing_rod_level: raw.fishing_rod_level,
                fishing_rod_name: raw.fishing_rod_name,
                fishing_rod_damage: raw.fishing_rod_damage,
            };
            self.by_item_id.insert(rod.fishing_rod_item_id, rod);
        }
    }

    /// Java `getFishingRod(itemId)`.
    pub fn get(&self, item_id: i32) -> Option<&FishingRod> {
        self.by_item_id.get(&item_id)
    }

    /// Number of rods.
    pub fn len(&self) -> usize {
        self.by_item_id.len()
    }

    /// True when nothing was loaded.
    pub fn is_empty(&self) -> bool {
        self.by_item_id.is_empty()
    }
}

/// One fishing monster (Java `FishingMonster`): the thing that can come out of the water instead
/// of a fish, for a range of fisher levels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FishingMonster {
    pub user_min_level: i32,
    pub user_max_level: i32,
    pub fishing_monster_id: i32,
    /// Percent; Java draws `Rnd.get(100) <= probability`.
    pub probability: i32,
}

/// The fishing monsters by NPC id. Java `FishingMonstersData`.
#[derive(Debug, Default)]
pub struct FishingMonstersData {
    by_id: BTreeMap<i32, FishingMonster>,
}

impl FishingMonstersData {
    const LOADER: &'static str = "FishingMonstersData";

    /// Loads `<root>/data/stats/fishing/fishingMonsters.xml` and logs Java's count line.
    pub fn load(root: &Path) -> Self {
        let mut table = Self::default();
        load_file(
            &root.join("data/stats/fishing/fishingMonsters.xml"),
            |doc: RawMonsterFile| {
                table.add(doc);
                Ok(())
            },
        );
        log_loaded(Self::LOADER, table.by_id.len(), "fishing monsters");
        table
    }

    /// Builds a table from XML text. For tests and tools.
    pub fn from_xml(xml: &str) -> Result<Self, DataError> {
        let doc = quick_xml::de::from_str(xml).map_err(|source| DataError::Xml {
            path: "<inline>".into(),
            source,
        })?;
        let mut table = Self::default();
        table.add(doc);
        Ok(table)
    }

    fn add(&mut self, doc: RawMonsterFile) {
        for raw in doc.monsters {
            let monster = FishingMonster {
                user_min_level: raw.user_min_level,
                user_max_level: raw.user_max_level,
                fishing_monster_id: raw.fishing_monster_id,
                probability: raw.probability,
            };
            self.by_id.insert(monster.fishing_monster_id, monster);
        }
    }

    /// Java `getFishingMonster(level)`: the first monster whose range holds the fisher's level.
    /// The ranges of the shipped file do not overlap, so Java's `HashMap` order does not matter.
    pub fn get_fishing_monster(&self, level: i32) -> Option<&FishingMonster> {
        self.by_id
            .values()
            .find(|m| level >= m.user_min_level && level <= m.user_max_level)
    }

    /// Java `getFishingMonsterById(id)`.
    pub fn get_fishing_monster_by_id(&self, id: i32) -> Option<&FishingMonster> {
        self.by_id.get(&id)
    }

    /// Number of monsters.
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// True when nothing was loaded.
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}

// ---- Raw XML shape (xsd/fishes.xsd, fishingRods.xsd, fishingMonsters.xsd) --------------------

#[derive(Deserialize)]
struct RawFishFile {
    #[serde(rename = "fish", default)]
    fish: Vec<RawFish>,
}

#[derive(Deserialize)]
struct RawFish {
    #[serde(rename = "@fishId")]
    fish_id: i32,
    #[serde(rename = "@itemId")]
    item_id: i32,
    #[serde(rename = "@itemName")]
    item_name: String,
    #[serde(rename = "@fishGroup")]
    fish_group: String,
    #[serde(rename = "@fishLevel")]
    fish_level: i32,
    #[serde(rename = "@fishBiteRate")]
    fish_bite_rate: f64,
    #[serde(rename = "@fishGuts")]
    fish_guts: f64,
    #[serde(rename = "@fishHp")]
    fish_hp: i32,
    #[serde(rename = "@fishMaxLength")]
    fish_max_length: i32,
    #[serde(rename = "@fishLengthRate")]
    fish_length_rate: f64,
    #[serde(rename = "@hpRegen")]
    hp_regen: f64,
    #[serde(rename = "@startCombatTime")]
    start_combat_time: i32,
    #[serde(rename = "@combatDuration")]
    combat_duration: i32,
    #[serde(rename = "@gutsCheckTime")]
    guts_check_time: i32,
    #[serde(rename = "@gutsCheckProbability")]
    guts_check_probability: f64,
    #[serde(rename = "@cheatingProb")]
    cheating_prob: f64,
    #[serde(rename = "@fishGrade")]
    fish_grade: String,
}

impl RawFish {
    fn into_fish(self) -> Fish {
        Fish {
            fish_id: self.fish_id,
            item_id: self.item_id,
            item_name: self.item_name,
            fish_group: fish_group_id(&self.fish_group),
            fish_level: self.fish_level,
            fish_bite_rate: self.fish_bite_rate,
            fish_guts: self.fish_guts,
            fish_hp: self.fish_hp,
            fish_max_length: self.fish_max_length,
            fish_length_rate: self.fish_length_rate,
            hp_regen: self.hp_regen,
            start_combat_time: self.start_combat_time,
            combat_duration: self.combat_duration,
            guts_check_time: self.guts_check_time,
            guts_check_probability: self.guts_check_probability,
            cheating_prob: self.cheating_prob,
            fish_grade: fish_grade_id(&self.fish_grade),
        }
    }
}

#[derive(Deserialize)]
struct RawRodFile {
    #[serde(rename = "fishingRod", default)]
    rods: Vec<RawRod>,
}

#[derive(Deserialize)]
struct RawRod {
    #[serde(rename = "@fishingRodId")]
    fishing_rod_id: i32,
    #[serde(rename = "@fishingRodItemId")]
    fishing_rod_item_id: i32,
    #[serde(rename = "@fishingRodLevel")]
    fishing_rod_level: i32,
    #[serde(rename = "@fishingRodName")]
    fishing_rod_name: String,
    #[serde(rename = "@fishingRodDamage")]
    fishing_rod_damage: f64,
}

#[derive(Deserialize)]
struct RawMonsterFile {
    #[serde(rename = "fishingMonster", default)]
    monsters: Vec<RawMonster>,
}

#[derive(Deserialize)]
struct RawMonster {
    #[serde(rename = "@userMinLevel")]
    user_min_level: i32,
    #[serde(rename = "@userMaxLevel")]
    user_max_level: i32,
    #[serde(rename = "@fishingMonsterId")]
    fishing_monster_id: i32,
    #[serde(rename = "@probability")]
    probability: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    const FISH_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<list>
	<fish fishId="1" itemId="6411" itemName="Small Nimble Green Fish" fishGroup="swift" fishLevel="1" fishBiteRate="100.0" fishGuts="50.0" fishHp="100" fishMaxLength="30000" fishLengthRate="10.0" hpRegen="4.0" startCombatTime="20" combatDuration="24" gutsCheckTime="5" gutsCheckProbability="50.0" cheatingProb="0.0" fishGrade="fish_normal" />
	<fish fishId="3" itemId="6413" itemName="Small Pudgy Green Fish" fishGroup="wide" fishLevel="1" fishBiteRate="100.0" fishGuts="50.4" fishHp="132" fishMaxLength="30000" fishLengthRate="10.0" hpRegen="4.0" startCombatTime="20" combatDuration="31" gutsCheckTime="5" gutsCheckProbability="50.4" cheatingProb="0.0" fishGrade="fish_normal" />
	<fish fishId="100" itemId="7000" itemName="Easy" fishGroup="easy_wide" fishLevel="1" fishBiteRate="100.0" fishGuts="60.0" fishHp="50" fishMaxLength="1" fishLengthRate="1.0" hpRegen="2.0" startCombatTime="25" combatDuration="20" gutsCheckTime="6" gutsCheckProbability="1.0" cheatingProb="0.0" fishGrade="fish_easy" />
	<fish fishId="200" itemId="8000" itemName="Hard" fishGroup="hs_fish" fishLevel="2" fishBiteRate="100.0" fishGuts="40.0" fishHp="500" fishMaxLength="1" fishLengthRate="1.0" hpRegen="9.0" startCombatTime="15" combatDuration="40" gutsCheckTime="3" gutsCheckProbability="1.0" cheatingProb="0.0" fishGrade="fish_hard" />
</list>"#;

    #[test]
    fn fish_is_read_by_grade_with_java_group_ids() {
        let table = FishData::from_xml(FISH_XML).unwrap();
        assert_eq!(table.len(), 4);
        let normal = table.get_fish(1, 1, 1);
        assert_eq!(normal.len(), 1);
        assert_eq!(normal[0].item_id, 6411);
        assert_eq!(normal[0].fish_hp, 100);
        assert_eq!(normal[0].hp_regen, 4.0);
        assert_eq!(normal[0].combat_duration, 24);
        // "wide" is group 0, "easy_wide" is group 4, "hs_fish" is 10.
        assert_eq!(table.get_fish(1, 0, 1)[0].fish_id, 3);
        assert_eq!(table.get_fish(1, 4, 0)[0].fish_id, 100);
        assert_eq!(table.get_fish(2, 10, 2)[0].fish_id, 200);
        // The grade decides the table: the easy fish is not a normal one.
        assert!(table.get_fish(1, 4, 1).is_empty());
        assert!(table.get_fish(1, 1, 3).is_empty());
    }

    #[test]
    fn group_and_grade_names_follow_java() {
        let groups = [
            ("wide", 0),
            ("swift", 1),
            ("ugly", 2),
            ("fish_box", 3),
            ("easy_wide", 4),
            ("easy_swift", 5),
            ("easy_ugly", 6),
            ("hard_wide", 7),
            ("hard_swift", 8),
            ("hard_ugly", 9),
            ("hs_fish", 10),
            ("whatever", 0),
        ];
        for (name, id) in groups {
            assert_eq!(fish_group_id(name), id, "{name}");
        }
        assert_eq!(fish_grade_id("fish_easy"), 0);
        assert_eq!(fish_grade_id("fish_normal"), 1);
        assert_eq!(fish_grade_id("fish_hard"), 2);
        assert_eq!(fish_grade_id("nonsense"), 1);
    }

    #[test]
    fn rods_and_monsters_are_looked_up_like_java() {
        let rods = FishingRodsData::from_xml(
            r#"<list>
	<fishingRod fishingRodId="1" fishingRodItemId="6529" fishingRodLevel="20" fishingRodName="Baby Duck Rod" fishingRodDamage="20.0" />
	<fishingRod fishingRodId="6" fishingRodItemId="6534" fishingRodLevel="80" fishingRodName="Triton Pole" fishingRodDamage="39.0" />
</list>"#,
        )
        .unwrap();
        let rod = rods.get(6534).unwrap();
        assert_eq!((rod.fishing_rod_level, rod.fishing_rod_damage), (80, 39.0));
        assert!(rods.get(6535).is_none());

        let monsters = FishingMonstersData::from_xml(
            r#"<list>
	<fishingMonster userMinLevel="6" userMaxLevel="15" fishingMonsterId="18319" probability="5" />
	<fishingMonster userMinLevel="16" userMaxLevel="25" fishingMonsterId="18320" probability="5" />
</list>"#,
        )
        .unwrap();
        assert_eq!(monsters.get_fishing_monster(6).unwrap().fishing_monster_id, 18319);
        assert_eq!(
            monsters.get_fishing_monster(25).unwrap().fishing_monster_id,
            18320
        );
        assert!(monsters.get_fishing_monster(5).is_none());
        assert!(monsters.get_fishing_monster(26).is_none());
        assert_eq!(monsters.get_fishing_monster_by_id(18320).unwrap().probability, 5);
    }
}
