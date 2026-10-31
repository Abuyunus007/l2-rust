//! The tables an augmentation is rolled from: the skill map of the free-form generator and the
//! retail chance tables.
//!
//! Java: `gameserver/data/AugmentationData.java` (the loading part; the rolling is
//! `l2_world::augment`). Data: `data/stats/augmentation/{augmentation_skillmap,retailchances,
//! retailchances_accessory}.xml` (`xsd/augmentation_skillmap.xsd`, `xsd/augmentation_retailchances.xsd`).
//! What an option id gives is [`crate::augmentation`] (`OptionData`).
//!
//! # Deviation
//! Java loads the skill map only with `RetailLikeAugmentation = False`, the weapon chances only
//! with `True` and the accessory chances only with `RetailLikeAugmentationAccessory = True`.
//! `Datapack::load` has no config, so all three files are read and `l2_world::augment` decides
//! which table it uses.

use std::path::Path;

use serde::Deserialize;
use tracing::{info, warn};

use crate::armor_set::SkillHolder;
use crate::loader::{DataError, load_file};
use crate::stats::jnum::{number_format_message, parse_int};

/// Java `STAT_BLOCKSIZE`.
pub const STAT_BLOCKSIZE: i32 = 3640;
/// Java `STAT_SUBBLOCKSIZE`.
pub const STAT_SUBBLOCKSIZE: i32 = 91;
/// Java `MIN_SKILL_ID`.
pub const MIN_SKILL_ID: i32 = STAT_BLOCKSIZE * 4;
/// Java `BLUE_START`: the first augmentation id of the skill map.
pub const BLUE_START: i32 = 14561;
/// Java `SKILLS_BLOCKSIZE`: augmentation ids per life stone level in the skill map.
pub const SKILLS_BLOCKSIZE: i32 = 178;
/// Java `BASESTAT_STR`.
pub const BASESTAT_STR: i32 = 16341;
/// Java `BASESTAT_MEN`.
pub const BASESTAT_MEN: i32 = 16344;
/// Java `ACC_START`.
pub const ACC_START: i32 = 16669;
/// Java `ACC_BLOCKS_NUM`.
pub const ACC_BLOCKS_NUM: i32 = 10;
/// Java `ACC_STAT_SUBBLOCKSIZE`.
pub const ACC_STAT_SUBBLOCKSIZE: i32 = 21;
/// Java `ACC_RING_START`.
pub const ACC_RING_START: i32 = ACC_START;
/// Java `ACC_RING_SKILLS`.
pub const ACC_RING_SKILLS: i32 = 18;
/// Java `ACC_RING_BLOCKSIZE`.
pub const ACC_RING_BLOCKSIZE: i32 = ACC_RING_SKILLS + (4 * ACC_STAT_SUBBLOCKSIZE);
/// Java `ACC_RING_END`.
pub const ACC_RING_END: i32 = (ACC_RING_START + (ACC_BLOCKS_NUM * ACC_RING_BLOCKSIZE)) - 1;
/// Java `ACC_EAR_START`.
pub const ACC_EAR_START: i32 = ACC_RING_END + 1;
/// Java `ACC_EAR_SKILLS`.
pub const ACC_EAR_SKILLS: i32 = 18;
/// Java `ACC_EAR_BLOCKSIZE`.
pub const ACC_EAR_BLOCKSIZE: i32 = ACC_EAR_SKILLS + (4 * ACC_STAT_SUBBLOCKSIZE);
/// Java `ACC_EAR_END`.
pub const ACC_EAR_END: i32 = (ACC_EAR_START + (ACC_BLOCKS_NUM * ACC_EAR_BLOCKSIZE)) - 1;
/// Java `ACC_NECK_START`.
pub const ACC_NECK_START: i32 = ACC_EAR_END + 1;
/// Java `ACC_NECK_SKILLS`.
pub const ACC_NECK_SKILLS: i32 = 24;
/// Java `ACC_NECK_BLOCKSIZE`.
pub const ACC_NECK_BLOCKSIZE: i32 = ACC_NECK_SKILLS + (4 * ACC_STAT_SUBBLOCKSIZE);

/// Java `AugmentationChance` and `augmentationChanceAcc` (the same shape): one augment id of a
/// retail table with its chances.
#[derive(Debug, Clone, PartialEq)]
pub struct AugmentationChance {
    /// `<weapon type>`: `warrior` or `mage`.
    pub weapon_type: String,
    /// `<stone id>`: the life stone item id.
    pub stone_id: i32,
    /// `<variation id>`: 1 holds the first two stats, the others the second pair.
    pub variation_id: i32,
    /// `<category probability>`.
    pub category_chance: i32,
    /// `<augment id>`.
    pub augment_id: i32,
    /// `<augment chance>`: a Java `float`, in percent.
    pub augment_chance: f32,
}

/// Java `AugmentationData`: the skill ids of the generator by colour and life stone level, and the
/// retail chance tables.
#[derive(Debug, Default)]
pub struct AugmentationData {
    blue_skills: [Vec<i32>; 10],
    purple_skills: [Vec<i32>; 10],
    red_skills: [Vec<i32>; 10],
    all_skills: std::collections::BTreeMap<i32, SkillHolder>,
    chances: Vec<AugmentationChance>,
    chances_acc: Vec<AugmentationChance>,
}

/// The colour of a skill-map entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillColor {
    /// `blue`
    Blue,
    /// `purple`
    Purple,
    /// anything else (`red`)
    Red,
}

impl AugmentationData {
    const LOADER: &'static str = "AugmentationData";

    /// Loads the three files under `<root>/data/stats/augmentation/` and logs Java's count lines.
    pub fn load(root: &Path) -> Self {
        let mut data = Self::default();
        let dir = root.join("data/stats/augmentation");

        // The skill map. Java: a missing file is a warning and `return` (so the accessory table
        // below is not read either); a broken entry ends the loading the same way.
        let skill_map = dir.join("augmentation_skillmap.xml");
        if skill_map.is_file() {
            let mut bad = 0;
            load_file(&skill_map, |doc: RawSkillMap| data.add_skill_map(doc, &mut bad));
            if bad != 0 {
                info!("{}: {bad} negative effect skill(s) were skipped.", Self::LOADER);
            }
            for level in 0..10 {
                info!(
                    "{}: Loaded {} blue, {} purple and {} red skills for lifeStoneLevel {level}",
                    Self::LOADER,
                    data.blue_skills[level].len(),
                    data.purple_skills[level].len(),
                    data.red_skills[level].len()
                );
            }
        } else {
            warn!(
                "{}: ERROR The augmentation skillmap file is missing.",
                Self::LOADER
            );
        }

        let retail = dir.join("retailchances.xml");
        if retail.is_file() {
            load_file(&retail, |doc: RawChances| {
                data.chances = flatten_chances(&doc)?;
                Ok(())
            });
        } else {
            warn!(
                "{}: ERROR The retailchances.xml data file is missing.",
                Self::LOADER
            );
        }
        info!("{}: Loaded {} augmentations.", Self::LOADER, data.chances.len());

        let retail_acc = dir.join("retailchances_accessory.xml");
        if retail_acc.is_file() {
            load_file(&retail_acc, |doc: RawChances| {
                data.chances_acc = flatten_chances(&doc)?;
                Ok(())
            });
        } else {
            warn!(
                "{}: ERROR The retailchances_accessory.xml data file is missing.",
                Self::LOADER
            );
        }
        info!(
            "{}: Loaded {} accessory augmentations.",
            Self::LOADER,
            data.chances_acc.len()
        );
        data
    }

    /// Builds the skill map from XML text. For tests and tools.
    pub fn skill_map_from_xml(xml: &str) -> Result<Self, DataError> {
        let doc: RawSkillMap = quick_xml::de::from_str(xml).map_err(|source| DataError::Xml {
            path: "<inline>".into(),
            source,
        })?;
        let mut data = Self::default();
        data.add_skill_map(doc, &mut 0)?;
        Ok(data)
    }

    /// Builds a data set with retail chance tables from XML text. For tests and tools.
    pub fn chances_from_xml(weapons: &str, accessories: &str) -> Result<Self, DataError> {
        let parse = |xml: &str| -> Result<Vec<AugmentationChance>, DataError> {
            let doc: RawChances = quick_xml::de::from_str(xml).map_err(|source| DataError::Xml {
                path: "<inline>".into(),
                source,
            })?;
            flatten_chances(&doc)
        };
        Ok(Self {
            chances: parse(weapons)?,
            chances_acc: parse(accessories)?,
            ..Self::default()
        })
    }

    /// Java `load()`, the skill map part. Entries before a broken one stay.
    fn add_skill_map(&mut self, doc: RawSkillMap, bad: &mut i32) -> Result<(), DataError> {
        for entry in doc.augmentations {
            let augmentation_id = int_of(&entry.id)?;
            let mut skill_id = 0;
            let mut skill_level = 0;
            let mut color = "blue".to_owned();
            if let Some(v) = &entry.skill_id {
                skill_id = int_of(&v.val)?;
            }
            if let Some(v) = &entry.skill_level {
                skill_level = int_of(&v.val)?;
            }
            if let Some(v) = &entry.kind {
                color = v.val.clone();
            }
            if skill_id == 0 || skill_level == 0 {
                *bad += 1;
                continue;
            }
            // Java integer division truncates toward zero; an index outside 0..10 throws
            // `IndexOutOfBoundsException`, which ends the loading.
            let k = (augmentation_id - BLUE_START) / SKILLS_BLOCKSIZE;
            let slot = usize::try_from(k)
                .ok()
                .filter(|k| *k < 10)
                .ok_or_else(|| DataError::Invalid(format!("Index {k} out of bounds for length 10")))?;
            if color.eq_ignore_ascii_case("blue") {
                self.blue_skills[slot].push(augmentation_id);
            } else if color.eq_ignore_ascii_case("purple") {
                self.purple_skills[slot].push(augmentation_id);
            } else {
                self.red_skills[slot].push(augmentation_id);
            }
            self.all_skills.insert(
                augmentation_id,
                SkillHolder {
                    id: skill_id,
                    level: skill_level,
                },
            );
        }
        Ok(())
    }

    /// Java `_blueSkills.get(level)`: the augmentation ids of the blue skills of a life stone level
    /// (0 to 9); empty for any other level.
    pub fn blue_skills(&self, level: usize) -> &[i32] {
        self.blue_skills.get(level).map_or(&[], Vec::as_slice)
    }

    /// Java `_purpleSkills.get(level)`.
    pub fn purple_skills(&self, level: usize) -> &[i32] {
        self.purple_skills.get(level).map_or(&[], Vec::as_slice)
    }

    /// Java `_redSkills.get(level)`.
    pub fn red_skills(&self, level: usize) -> &[i32] {
        self.red_skills.get(level).map_or(&[], Vec::as_slice)
    }

    /// The skills of one colour of a life stone level, by colour.
    pub fn skills(&self, color: SkillColor, level: usize) -> &[i32] {
        match color {
            SkillColor::Blue => self.blue_skills(level),
            SkillColor::Purple => self.purple_skills(level),
            SkillColor::Red => self.red_skills(level),
        }
    }

    /// Java `_allSkills.get(augmentationId)`: the skill an augmentation id of the skill map gives.
    pub fn skill_of(&self, augmentation_id: i32) -> Option<SkillHolder> {
        self.all_skills.get(&augmentation_id).copied()
    }

    /// Java `_augmentationChances`: the weapon table, in file order.
    pub fn chances(&self) -> &[AugmentationChance] {
        &self.chances
    }

    /// Java `_augmentationChancesAcc`: the accessory table, in file order.
    pub fn accessory_chances(&self) -> &[AugmentationChance] {
        &self.chances_acc
    }
}

fn int_of(text: &str) -> Result<i32, DataError> {
    parse_int(text).ok_or_else(|| DataError::Invalid(number_format_message(text)))
}

/// Java `Float.parseFloat` for the plain decimals of the tables.
fn float_of(text: &str) -> Result<f32, DataError> {
    let trimmed = text.trim();
    let number = trimmed.strip_suffix(['f', 'F', 'd', 'D']).unwrap_or(trimmed);
    number
        .parse::<f32>()
        .map_err(|_| DataError::Invalid(format!("For input string: \"{text}\"")))
}

/// The nested `weapon > stone > variation > category > augment` loops of Java, in document order.
fn flatten_chances(doc: &RawChances) -> Result<Vec<AugmentationChance>, DataError> {
    let mut out = Vec::new();
    for weapon in &doc.weapons {
        for stone in &weapon.stones {
            let stone_id = int_of(&stone.id)?;
            for variation in &stone.variations {
                let variation_id = int_of(&variation.id)?;
                for category in &variation.categories {
                    let category_chance = int_of(&category.probability)?;
                    for augment in &category.augments {
                        out.push(AugmentationChance {
                            weapon_type: weapon.kind.clone(),
                            stone_id,
                            variation_id,
                            category_chance,
                            augment_id: int_of(&augment.id)?,
                            augment_chance: float_of(&augment.chance)?,
                        });
                    }
                }
            }
        }
    }
    Ok(out)
}

// ---- Raw XML shape ----------------------------------------------------------------------------

#[derive(Deserialize)]
struct RawSkillMap {
    #[serde(rename = "augmentation", default)]
    augmentations: Vec<RawSkillMapEntry>,
}

#[derive(Deserialize)]
struct RawSkillMapEntry {
    #[serde(rename = "@id")]
    id: String,
    #[serde(rename = "skillId")]
    skill_id: Option<RawVal>,
    #[serde(rename = "skillLevel")]
    skill_level: Option<RawVal>,
    #[serde(rename = "type")]
    kind: Option<RawVal>,
}

#[derive(Deserialize)]
struct RawVal {
    #[serde(rename = "@val")]
    val: String,
}

#[derive(Deserialize)]
struct RawChances {
    #[serde(rename = "weapon", default)]
    weapons: Vec<RawWeapon>,
}

#[derive(Deserialize)]
struct RawWeapon {
    #[serde(rename = "@type")]
    kind: String,
    #[serde(rename = "stone", default)]
    stones: Vec<RawStone>,
}

#[derive(Deserialize)]
struct RawStone {
    #[serde(rename = "@id")]
    id: String,
    #[serde(rename = "variation", default)]
    variations: Vec<RawVariation>,
}

#[derive(Deserialize)]
struct RawVariation {
    #[serde(rename = "@id")]
    id: String,
    #[serde(rename = "category", default)]
    categories: Vec<RawCategory>,
}

#[derive(Deserialize)]
struct RawCategory {
    #[serde(rename = "@probability")]
    probability: String,
    #[serde(rename = "augment", default)]
    augments: Vec<RawAugment>,
}

#[derive(Deserialize)]
struct RawAugment {
    #[serde(rename = "@id")]
    id: String,
    #[serde(rename = "@chance")]
    chance: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_accessory_blocks_follow_java() {
        // Java: ACC_RING_BLOCKSIZE = 18 + 4 * 21 = 102, ACC_RING_END = 16669 + 10 * 102 - 1.
        assert_eq!(ACC_RING_BLOCKSIZE, 102);
        assert_eq!(ACC_RING_END, 17688);
        assert_eq!(ACC_EAR_START, 17689);
        assert_eq!(ACC_EAR_END, 18708);
        assert_eq!(ACC_NECK_START, 18709);
        assert_eq!(ACC_NECK_BLOCKSIZE, 108);
        assert_eq!(MIN_SKILL_ID, 14560);
    }

    const SKILL_MAP: &str = r#"<list>
	<augmentation id="14561"><skillId val="3203" /><skillLevel val="1" /><type val="blue" /></augmentation>
	<augmentation id="14562"><skillId val="0" /><skillLevel val="1" /><type val="blue" /></augmentation>
	<augmentation id="14739"><skillId val="3204" /><skillLevel val="2" /><type val="Purple" /></augmentation>
	<augmentation id="14740"><skillId val="3205" /><skillLevel val="3" /><type val="red" /></augmentation>
	<augmentation id="14741"><skillId val="3206" /><skillLevel val="3" /></augmentation>
</list>"#;

    #[test]
    fn the_skill_map_is_sorted_into_colours_and_life_stone_levels() {
        let data = AugmentationData::skill_map_from_xml(SKILL_MAP).unwrap();
        // k = (id - 14561) / 178: 14561 is level 0, 14739 and up level 1.
        assert_eq!(data.blue_skills(0), [14561]);
        // Without a type the colour is blue (Java's default).
        assert_eq!(data.blue_skills(1), [14741]);
        assert_eq!(data.purple_skills(1), [14739]);
        assert_eq!(data.red_skills(1), [14740]);
        assert_eq!(data.skill_of(14739), Some(SkillHolder { id: 3204, level: 2 }));
        // A skill id of 0 is "bad data": skipped.
        assert_eq!(data.skill_of(14562), None);
        assert!(data.blue_skills(10).is_empty());
    }

    #[test]
    fn an_id_beyond_the_tenth_block_ends_the_loading() {
        // k = (16341 - 14561) / 178 = 10: Java's `ArrayList.get(10)` throws.
        let xml = r#"<list>
	<augmentation id="14561"><skillId val="1" /><skillLevel val="1" /></augmentation>
	<augmentation id="16341"><skillId val="2" /><skillLevel val="1" /></augmentation>
	<augmentation id="14562"><skillId val="3" /><skillLevel val="1" /></augmentation>
</list>"#;
        assert!(AugmentationData::skill_map_from_xml(xml).is_err());
        let doc: RawSkillMap = quick_xml::de::from_str(xml).unwrap();
        let mut data = AugmentationData::default();
        assert!(data.add_skill_map(doc, &mut 0).is_err());
        assert_eq!(data.blue_skills(0), [14561]);
    }

    const CHANCES: &str = r#"<list>
	<weapon type="warrior">
		<stone id="8723">
			<variation id="1"><category probability="100"><augment id="1" chance="1.1" /><augment id="2" chance="2.5" /></category></variation>
			<variation id="2">
				<category probability="55"><augment id="7300" chance="0.5" /></category>
				<category probability="3"><augment id="7400" chance="100" /></category>
			</variation>
		</stone>
	</weapon>
	<weapon type="mage"><stone id="8724"><variation id="1"><category probability="100"><augment id="9" chance="1" /></category></variation></stone></weapon>
</list>"#;

    #[test]
    fn the_retail_tables_flatten_in_document_order() {
        let data = AugmentationData::chances_from_xml(CHANCES, "<list></list>").unwrap();
        let rows: Vec<_> = data
            .chances()
            .iter()
            .map(|c| {
                (
                    c.weapon_type.as_str(),
                    c.stone_id,
                    c.variation_id,
                    c.category_chance,
                    c.augment_id,
                    c.augment_chance,
                )
            })
            .collect();
        assert_eq!(
            rows,
            [
                ("warrior", 8723, 1, 100, 1, 1.1),
                ("warrior", 8723, 1, 100, 2, 2.5),
                ("warrior", 8723, 2, 55, 7300, 0.5),
                ("warrior", 8723, 2, 3, 7400, 100.0),
                ("mage", 8724, 1, 100, 9, 1.0),
            ]
        );
        assert!(data.accessory_chances().is_empty());
    }

    #[test]
    fn a_bad_number_fails_the_table() {
        let xml = r#"<list><weapon type="warrior"><stone id="x"></stone></weapon></list>"#;
        assert!(AugmentationData::chances_from_xml(xml, "<list></list>").is_err());
    }

    /// The shipped files, when the reference folder is there: the counts and the invariants the
    /// generator relies on.
    #[test]
    fn the_shipped_tables_load() {
        let root = std::env::var_os("L2JMOBIUS_GAME_DIR").map_or_else(
            || std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game")),
            Into::into,
        );
        if !root.join("data/stats/augmentation").is_dir() {
            println!("skipped: no datapack at {}", root.display());
            return;
        }
        let data = AugmentationData::load(&root);
        assert!(!data.chances().is_empty() && !data.accessory_chances().is_empty());
        let skill_entries: usize = (0..10)
            .map(|l| data.blue_skills(l).len() + data.purple_skills(l).len() + data.red_skills(l).len())
            .sum();
        assert!(skill_entries > 0);
        // Every skill-map entry sits in the block of its life stone level.
        for level in 0..10 {
            for id in data.skills(SkillColor::Blue, level) {
                assert_eq!(
                    usize::try_from((id - BLUE_START) / SKILLS_BLOCKSIZE).unwrap(),
                    level
                );
            }
        }
    }
}
