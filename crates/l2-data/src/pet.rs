//! The pets: what a pet collar summons, its food and skills and the numbers of each level, from
//! `data/stats/pets/*.xml`.
//!
//! Java: `gameserver/data/xml/PetDataTable.java`, `gameserver/data/holders/{PetData,PetLevelData}.java`.
//! Data: `data/stats/pets/*.xml`, `xsd/PetData.xsd`.
//!
//! One `<pet id="NPC" itemId="COLLAR">` per pet. Its `<set>`s give the food, the carrying `load`,
//! the `hungry_limit` (percent of the food bar) and `sync_level`; its `<skills>` the skills the
//! pet learns (`skillLevel="0"`: by the pet's level, see [`PetData::available_level`]); its
//! `<stats>` one `<stat level="..">` of `<set name=.. val=..>` numbers per level.
//!
//! `index` and the `evolve` set are ignored, like Java ("Index ignored for now", "evolve ignored").
//!
//! # Errors
//! Java reads the level numbers with `StatSet.getInt`/`getFloat`, which throw for a missing key;
//! the exception stops the document, the pets before it stay. Here a missing or unreadable value
//! gives a [`DataError::Invalid`] for the file with the same effect (the loader logs it).

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use serde::Deserialize;

use crate::loader::{DataError, load_dir, log_loaded};

/// Java `PetData._minLevel` start value (`Byte.MAX_VALUE`).
const NO_MIN_LEVEL: i32 = 127;

/// Java `PetLevelData`: the numbers of one pet level.
#[derive(Debug, Clone, PartialEq)]
pub struct PetLevelData {
    /// `get_exp_type`: the percent of the exp a monster gives the owner that the pet takes
    /// (`_ownerExpTaken`).
    pub owner_exp_taken: i32,
    /// `consume_meal_in_battle` (`_petFeedBattle`).
    pub feed_battle: i32,
    /// `consume_meal_in_normal` (`_petFeedNormal`).
    pub feed_normal: i32,
    /// `org_mattack`.
    pub m_atk: f32,
    /// `exp`: the exp this level starts at (`(long) getDouble("exp")`).
    pub max_exp: i64,
    /// `max_meal`.
    pub max_feed: i32,
    /// `org_hp`.
    pub max_hp: f32,
    /// `org_mp`.
    pub max_mp: f32,
    /// `org_mdefend`.
    pub m_def: f32,
    /// `org_pattack`.
    pub p_atk: f32,
    /// `org_pdefend`.
    pub p_def: f32,
    /// `org_hp_regen`.
    pub regen_hp: f32,
    /// `org_mp_regen`.
    pub regen_mp: f32,
    /// `soulshot_count` (`short`).
    pub soul_shot: i16,
    /// `spiritshot_count` (`short`).
    pub spirit_shot: i16,
    /// The `speed_on_ride` numbers (mounts), 0 when the pet has none.
    pub walk_speed_on_ride: f64,
    pub run_speed_on_ride: f64,
    pub slow_swim_speed_on_ride: f64,
    pub fast_swim_speed_on_ride: f64,
    pub slow_fly_speed_on_ride: f64,
    pub fast_fly_speed_on_ride: f64,
}

/// Java `PetData.PetSkillLearn`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PetSkillLearn {
    pub skill_id: i32,
    /// 0 = the level comes from the pet's level.
    pub skill_level: i32,
    /// `minLevel`: the pet level this skill is available from.
    pub min_level: i32,
}

/// Java `PetData`: what is the same on every level of one pet.
#[derive(Debug, Clone, PartialEq)]
pub struct PetData {
    pub npc_id: i32,
    /// `itemId`: the collar that summons the pet.
    pub item_id: i32,
    /// `load` (default 20000).
    pub load: i32,
    /// `hungry_limit` (default 1).
    pub hungry_limit: i32,
    /// `sync_level`: the pet takes its owner's level.
    pub sync_level: bool,
    /// The food in file order; see [`PetData::food`] for Java's order.
    food: Vec<i32>,
    skills: Vec<PetSkillLearn>,
    levels: HashMap<i32, PetLevelData>,
    min_level: i32,
    max_level: i32,
}

impl PetData {
    fn new(npc_id: i32, item_id: i32) -> Self {
        Self {
            npc_id,
            item_id,
            load: 20000,
            hungry_limit: 1,
            sync_level: false,
            food: Vec::new(),
            skills: Vec::new(),
            levels: HashMap::new(),
            min_level: NO_MIN_LEVEL,
            max_level: 0,
        }
    }

    /// Java `getMinLevel()`.
    pub fn min_level(&self) -> i32 {
        self.min_level
    }

    /// Java `getMaxLevel()`.
    pub fn max_level(&self) -> i32 {
        self.max_level
    }

    /// Java `getPetLevelData(level)`: exactly that level.
    pub fn level_data(&self, level: i32) -> Option<&PetLevelData> {
        self.levels.get(&level)
    }

    /// Java `getAvailableSkills()`.
    pub fn skills(&self) -> &[PetSkillLearn] {
        &self.skills
    }

    /// Java `getFood().contains(item_id)`.
    pub fn can_eat(&self, item_id: i32) -> bool {
        self.food.contains(&item_id)
    }

    /// Java `getFood()` in the order a `HashSet<Integer>` iterates (the feed task takes the first
    /// food it finds in the pet's inventory): by bucket of the default table, which grows as the
    /// set does, and by insertion inside a bucket.
    // NOTE(java-quirk): the order of a Java `HashSet` is an implementation detail; it is
    // reproduced here so the same food is eaten first.
    pub fn food(&self) -> Vec<i32> {
        let mut capacity: usize = 16;
        while self.food.len() * 4 > capacity * 3 {
            capacity *= 2;
        }
        let mut ordered = self.food.clone();
        ordered.sort_by_key(|id| {
            // `HashMap.hash(key)`: `h ^ (h >>> 16)`, then `(n - 1) & hash`.
            let h = *id as u32;
            ((h ^ (h >> 16)) as usize) & (capacity - 1)
        });
        ordered
    }

    /// The food ids as listed in the file.
    pub fn food_in_file_order(&self) -> &[i32] {
        &self.food
    }

    /// Java `isSynchLevel()`.
    pub fn is_sync_level(&self) -> bool {
        self.sync_level
    }

    /// Java `addNewStat(level, data)`.
    fn add_level(&mut self, level: i32, data: PetLevelData) {
        if self.min_level > level {
            self.min_level = level;
        }
        if self.max_level < level {
            self.max_level = level;
        }
        self.levels.insert(level, data);
    }

    /// Java `getAvailableLevel(skillId, petLvl)`: the level of the skill at that pet level.
    /// `skill_max_level` is `SkillData.getMaxLevel(skillId)`.
    pub fn available_level(
        &self,
        skill_id: i32,
        pet_level: i32,
        skill_max_level: &dyn Fn(i32) -> i32,
    ) -> i32 {
        let mut level = 0;
        let mut found = false;
        for learn in &self.skills {
            if learn.skill_id != skill_id {
                continue;
            }
            found = true;
            if learn.skill_level == 0 {
                if pet_level < 70 {
                    level = pet_level / 10;
                    if level <= 0 {
                        level = 1;
                    }
                } else {
                    level = 7 + ((pet_level - 70) / 5);
                }
                // Formula usable for skill that have 10 or more skill levels.
                let max_level = skill_max_level(learn.skill_id);
                if level > max_level {
                    level = max_level;
                }
                break;
            } else if learn.min_level <= pet_level && learn.skill_level > level {
                level = learn.skill_level;
            }
        }
        if found && level == 0 {
            return 1;
        }
        level
    }
}

/// Java `PetDataTable`. Immutable once loaded.
#[derive(Debug, Clone, Default)]
pub struct PetDataTable {
    pets: BTreeMap<i32, PetData>,
}

impl PetDataTable {
    /// Loads `data/stats/pets/*.xml` below `root` (Java `parseDatapackDirectory("data/stats/pets",
    /// false)`).
    pub fn load(root: &Path) -> Self {
        let mut table = Self::default();
        load_dir::<RawFile, _>(&root.join("data").join("stats").join("pets"), false, |doc| {
            table.add_document(doc)
        });
        log_loaded("PetDataTable", table.pets.len(), "pets");
        table
    }

    /// Parses one pets file (tests, tools).
    pub fn from_xml(text: &str) -> Result<Self, DataError> {
        let doc = quick_xml::de::from_str(text).map_err(|source| DataError::Xml {
            path: "<inline>".into(),
            source,
        })?;
        let mut table = Self::default();
        table.add_document(doc)?;
        Ok(table)
    }

    // Java: PetDataTable.parseDocument
    fn add_document(&mut self, doc: RawFile) -> Result<(), DataError> {
        for child in doc.children {
            let RawChild::Pet(raw) = child else { continue };
            let npc_id = int_attr(raw.id.as_deref(), "id")?;
            let item_id = int_attr(raw.item_id.as_deref(), "itemId")?;
            let mut data = PetData::new(npc_id, item_id);
            for part in raw.children {
                match part {
                    RawPetChild::Set(set) => apply_pet_set(&mut data, &set)?,
                    RawPetChild::Skills(skills) => {
                        for entry in skills.children {
                            let RawSkillChild::Skill(skill) = entry else {
                                continue;
                            };
                            data.skills.push(PetSkillLearn {
                                skill_id: int_attr(skill.skill_id.as_deref(), "skillId")?,
                                skill_level: int_attr(skill.skill_level.as_deref(), "skillLevel")?,
                                min_level: int_attr(skill.min_level.as_deref(), "minLevel")?,
                            });
                        }
                    }
                    RawPetChild::Stats(stats) => {
                        for entry in stats.children {
                            let RawStatsChild::Stat(stat) = entry else {
                                continue;
                            };
                            let level = int_attr(stat.level.as_deref(), "level")?;
                            let values = StatValues::of(&stat)?;
                            data.add_level(level, values.into_level_data()?);
                        }
                    }
                    RawPetChild::Other => {}
                }
            }
            // `_pets.put(npcId, data)` replaces an earlier pet of the same npc.
            self.pets.insert(npc_id, data);
        }
        Ok(())
    }

    /// Java `getPetData(petId)`.
    pub fn pet_data(&self, npc_id: i32) -> Option<&PetData> {
        self.pets.get(&npc_id)
    }

    /// Java `getPetDataByItemId(itemId)`: the first pet (in npc id order) whose collar this is.
    // NOTE(java-quirk): Java walks a `ConcurrentHashMap`, so the pet it finds first for a collar
    // two pets share is not defined; no shipped collar is shared.
    pub fn pet_data_by_item_id(&self, item_id: i32) -> Option<&PetData> {
        self.pets.values().find(|d| d.item_id == item_id)
    }

    /// Java `getPetLevelData(petId, petLevel)`: above the pet's highest level the highest level's
    /// numbers.
    pub fn pet_level_data(&self, npc_id: i32, level: i32) -> Option<&PetLevelData> {
        let data = self.pets.get(&npc_id)?;
        if level > data.max_level {
            return data.level_data(data.max_level);
        }
        data.level_data(level)
    }

    /// Java `getPetMinLevel(petId)`.
    pub fn pet_min_level(&self, npc_id: i32) -> Option<i32> {
        self.pets.get(&npc_id).map(PetData::min_level)
    }

    /// Java `getPetItemsByNpc(npcId)`: the collar of a pet.
    pub fn pet_item_by_npc(&self, npc_id: i32) -> Option<i32> {
        self.pets.get(&npc_id).map(|d| d.item_id)
    }

    /// The number of pets (Java's "Loaded N pets.").
    pub fn len(&self) -> usize {
        self.pets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pets.is_empty()
    }
}

/// Java `PetDataTable.isMountable(npcId)`: `MountType.findByNpcId(npcId) != NONE`, i.e. the NPC is
/// in the `STRIDER`, `WYVERN_GROUP` or `WOLF_GROUP` category.
// Java: data/categoryData.xml (CategoryData is not loaded as a table yet, so the three categories
// are listed here).
pub fn is_mountable(npc_id: i32) -> bool {
    /// `STRIDER`.
    const STRIDER: [i32; 7] = [12526, 12527, 12528, 16038, 16039, 16040, 16068];
    /// `WYVERN_GROUP`.
    const WYVERN_GROUP: [i32; 1] = [12621];
    /// `WOLF_GROUP`.
    const WOLF_GROUP: [i32; 4] = [16030, 16037, 16041, 16042];
    STRIDER.contains(&npc_id) || WYVERN_GROUP.contains(&npc_id) || WOLF_GROUP.contains(&npc_id)
}

fn apply_pet_set(data: &mut PetData, set: &RawSet) -> Result<(), DataError> {
    let name = set.name.as_deref().unwrap_or_default();
    match name {
        "food" => {
            for id in value(set)?.split(';') {
                let id: i32 = id
                    .trim()
                    .parse()
                    .map_err(|_| DataError::Invalid(format!("pet {}: bad food id {id:?}", data.npc_id)))?;
                // `HashSet.add`: a repeated id is kept once.
                if !data.food.contains(&id) {
                    data.food.push(id);
                }
            }
        }
        "load" => data.load = int_attr(set.val.as_deref(), "val")?,
        "hungry_limit" => data.hungry_limit = int_attr(set.val.as_deref(), "val")?,
        "sync_level" => data.sync_level = int_attr(set.val.as_deref(), "val")? == 1,
        // evolve ignored
        _ => {}
    }
    Ok(())
}

fn value(set: &RawSet) -> Result<&str, DataError> {
    set.val
        .as_deref()
        .ok_or_else(|| DataError::Invalid(format!("set {:?} without a val", set.name)))
}

fn int_attr(text: Option<&str>, what: &str) -> Result<i32, DataError> {
    text.and_then(|t| t.trim().parse().ok())
        .ok_or_else(|| DataError::Invalid(format!("missing or bad integer {what}")))
}

/// The `<set>`s of one `<stat>` as Java's `StatSet`: the speeds of `speed_on_ride` under their
/// own keys, every other set under its name.
struct StatValues(HashMap<String, String>);

impl StatValues {
    fn of(stat: &RawStat) -> Result<Self, DataError> {
        let mut map = HashMap::new();
        for entry in &stat.children {
            let RawStatChild::Set(set) = entry else { continue };
            let name = set
                .name
                .as_deref()
                .ok_or_else(|| DataError::Invalid("stat set without a name".to_owned()))?;
            if name == "speed_on_ride" {
                let need = |text: &Option<String>, what: &str| {
                    text.clone()
                        .ok_or_else(|| DataError::Invalid(format!("speed_on_ride without {what}")))
                };
                map.insert("walkSpeedOnRide".to_owned(), need(&set.walk, "walk")?);
                map.insert("runSpeedOnRide".to_owned(), need(&set.run, "run")?);
                map.insert(
                    "slowSwimSpeedOnRide".to_owned(),
                    need(&set.slow_swim, "slowSwim")?,
                );
                map.insert(
                    "fastSwimSpeedOnRide".to_owned(),
                    need(&set.fast_swim, "fastSwim")?,
                );
                if let Some(v) = &set.slow_fly {
                    map.insert("slowFlySpeedOnRide".to_owned(), v.clone());
                }
                if let Some(v) = &set.fast_fly {
                    map.insert("fastFlySpeedOnRide".to_owned(), v.clone());
                }
            } else {
                map.insert(name.to_owned(), value(set)?.to_owned());
            }
        }
        Ok(Self(map))
    }

    fn raw(&self, key: &str) -> Result<&str, DataError> {
        self.0
            .get(key)
            .map(String::as_str)
            .ok_or_else(|| DataError::Invalid(format!("pet level without {key}")))
    }

    fn double(&self, key: &str) -> Result<f64, DataError> {
        self.raw(key)?
            .trim()
            .parse()
            .map_err(|_| DataError::Invalid(format!("pet level: bad number for {key}")))
    }

    fn double_or(&self, key: &str, default: f64) -> Result<f64, DataError> {
        match self.0.get(key) {
            Some(v) => v
                .trim()
                .parse()
                .map_err(|_| DataError::Invalid(format!("pet level: bad number for {key}"))),
            None => Ok(default),
        }
    }

    fn int(&self, key: &str) -> Result<i32, DataError> {
        // `StatSet.getInt` parses with `Integer.parseInt` (no decimals).
        self.raw(key)?
            .trim()
            .parse()
            .map_err(|_| DataError::Invalid(format!("pet level: bad integer for {key}")))
    }

    fn float(&self, key: &str) -> Result<f32, DataError> {
        self.raw(key)?
            .trim()
            .parse()
            .map_err(|_| DataError::Invalid(format!("pet level: bad number for {key}")))
    }

    fn short(&self, key: &str) -> Result<i16, DataError> {
        self.raw(key)?
            .trim()
            .parse()
            .map_err(|_| DataError::Invalid(format!("pet level: bad short for {key}")))
    }

    /// Java `new PetLevelData(set)`.
    fn into_level_data(self) -> Result<PetLevelData, DataError> {
        Ok(PetLevelData {
            owner_exp_taken: self.int("get_exp_type")?,
            max_exp: self.double("exp")? as i64,
            max_hp: self.float("org_hp")?,
            max_mp: self.float("org_mp")?,
            p_atk: self.float("org_pattack")?,
            p_def: self.float("org_pdefend")?,
            m_atk: self.float("org_mattack")?,
            m_def: self.float("org_mdefend")?,
            max_feed: self.int("max_meal")?,
            feed_battle: self.int("consume_meal_in_battle")?,
            feed_normal: self.int("consume_meal_in_normal")?,
            regen_hp: self.float("org_hp_regen")?,
            regen_mp: self.float("org_mp_regen")?,
            soul_shot: self.short("soulshot_count")?,
            spirit_shot: self.short("spiritshot_count")?,
            walk_speed_on_ride: self.double_or("walkSpeedOnRide", 0.0)?,
            run_speed_on_ride: self.double_or("runSpeedOnRide", 0.0)?,
            slow_swim_speed_on_ride: self.double_or("slowSwimSpeedOnRide", 0.0)?,
            fast_swim_speed_on_ride: self.double_or("fastSwimSpeedOnRide", 0.0)?,
            slow_fly_speed_on_ride: self.double_or("slowFlySpeedOnRide", 0.0)?,
            fast_fly_speed_on_ride: self.double_or("fastFlySpeedOnRide", 0.0)?,
        })
    }
}

#[derive(Deserialize)]
struct RawFile {
    #[serde(rename = "$value", default)]
    children: Vec<RawChild>,
}

#[derive(Deserialize)]
enum RawChild {
    #[serde(rename = "pet")]
    Pet(RawPet),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct RawPet {
    #[serde(rename = "@id")]
    id: Option<String>,
    #[serde(rename = "@itemId")]
    item_id: Option<String>,
    #[serde(rename = "$value", default)]
    children: Vec<RawPetChild>,
}

#[derive(Deserialize)]
enum RawPetChild {
    #[serde(rename = "set")]
    Set(RawSet),
    #[serde(rename = "skills")]
    Skills(RawSkills),
    #[serde(rename = "stats")]
    Stats(RawStats),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct RawSet {
    #[serde(rename = "@name")]
    name: Option<String>,
    #[serde(rename = "@val")]
    val: Option<String>,
    #[serde(rename = "@walk")]
    walk: Option<String>,
    #[serde(rename = "@run")]
    run: Option<String>,
    #[serde(rename = "@slowSwim")]
    slow_swim: Option<String>,
    #[serde(rename = "@fastSwim")]
    fast_swim: Option<String>,
    #[serde(rename = "@slowFly")]
    slow_fly: Option<String>,
    #[serde(rename = "@fastFly")]
    fast_fly: Option<String>,
}

#[derive(Deserialize)]
struct RawSkills {
    #[serde(rename = "$value", default)]
    children: Vec<RawSkillChild>,
}

#[derive(Deserialize)]
enum RawSkillChild {
    #[serde(rename = "skill")]
    Skill(RawSkill),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct RawSkill {
    #[serde(rename = "@skillId")]
    skill_id: Option<String>,
    #[serde(rename = "@skillLevel")]
    skill_level: Option<String>,
    #[serde(rename = "@minLevel")]
    min_level: Option<String>,
}

#[derive(Deserialize)]
struct RawStats {
    #[serde(rename = "$value", default)]
    children: Vec<RawStatsChild>,
}

#[derive(Deserialize)]
enum RawStatsChild {
    #[serde(rename = "stat")]
    Stat(RawStat),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct RawStat {
    #[serde(rename = "@level")]
    level: Option<String>,
    #[serde(rename = "$value", default)]
    children: Vec<RawStatChild>,
}

#[derive(Deserialize)]
enum RawStatChild {
    #[serde(rename = "set")]
    Set(RawSet),
    #[serde(other)]
    Other,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn level(level: i32, exp: i64, hp: &str) -> String {
        format!(
            r#"<stat level="{level}"><set name="max_meal" val="12"/><set name="exp" val="{exp}"/>
            <set name="get_exp_type" val="70"/><set name="consume_meal_in_battle" val="2"/>
            <set name="consume_meal_in_normal" val="1"/><set name="org_pattack" val="3.5"/>
            <set name="org_pdefend" val="4"/><set name="org_mattack" val="5"/><set name="org_mdefend" val="6"/>
            <set name="org_hp" val="{hp}"/><set name="org_mp" val="13.2"/><set name="org_hp_regen" val="6"/>
            <set name="org_mp_regen" val="2.7"/><set name="soulshot_count" val="2"/>
            <set name="spiritshot_count" val="1"/></stat>"#
        )
    }

    fn sample() -> String {
        format!(
            r#"<pets><pet id="12780" itemId="6648" index="9">
            <set name="food" val="7582;5168;5169"/><set name="hungry_limit" val="55"/>
            <set name="load" val="54510"/><set name="sync_level" val="1"/>
            <skills><skill minLevel="1" skillId="4717" skillLevel="0"/>
            <skill minLevel="20" skillId="4718" skillLevel="2"/><skill minLevel="40" skillId="4718" skillLevel="3"/></skills>
            <stats>{}{}{}</stats></pet>
            <pet id="12526" itemId="4422"><stats>{}</stats></pet></pets>"#,
            level(1, 1, "13.1"),
            level(2, 125_001, "15.9"),
            level(3, 250_001, "20"),
            level(1, 0, "31"),
        )
    }

    #[test]
    fn a_pet_has_its_sets_skills_and_levels() {
        let table = PetDataTable::from_xml(&sample()).unwrap();
        assert_eq!(table.len(), 2);
        let pet = table.pet_data(12780).unwrap();
        assert_eq!((pet.npc_id, pet.item_id), (12780, 6648));
        assert_eq!((pet.load, pet.hungry_limit, pet.sync_level), (54510, 55, true));
        assert_eq!((pet.min_level(), pet.max_level()), (1, 3));
        assert!(pet.can_eat(5168) && !pet.can_eat(1));
        let first = pet.level_data(1).unwrap();
        assert_eq!(first.max_hp, 13.1_f32);
        assert_eq!(
            (first.max_feed, first.owner_exp_taken, first.feed_battle),
            (12, 70, 2)
        );
        assert_eq!((first.soul_shot, first.spirit_shot), (2, 1));
        assert_eq!(first.p_atk, 3.5_f32);
        assert_eq!(first.max_exp, 1);
        // The other pet keeps Java's defaults.
        let strider = table.pet_data(12526).unwrap();
        assert_eq!(
            (strider.load, strider.hungry_limit, strider.sync_level),
            (20000, 1, false)
        );
        assert_eq!(table.pet_item_by_npc(12526), Some(4422));
        assert_eq!(table.pet_data_by_item_id(6648).map(|p| p.npc_id), Some(12780));
        assert!(table.pet_data_by_item_id(1).is_none());
    }

    #[test]
    fn level_data_above_the_highest_level_is_the_highest() {
        let table = PetDataTable::from_xml(&sample()).unwrap();
        assert_eq!(table.pet_level_data(12780, 99).unwrap().max_hp, 20.0);
        assert_eq!(table.pet_level_data(12780, 1).unwrap().max_hp, 13.1_f32);
        assert_eq!(table.pet_level_data(12780, 2).unwrap().max_exp, 125_001);
        assert!(table.pet_level_data(12780, 0).is_none());
        assert!(table.pet_level_data(1, 1).is_none());
        assert_eq!(table.pet_min_level(12780), Some(1));
    }

    #[test]
    fn available_level_follows_pet_data() {
        let table = PetDataTable::from_xml(&sample()).unwrap();
        let pet = table.pet_data(12780).unwrap();
        let max = |_: i32| 9;
        // skillLevel 0: by the pet's level.
        assert_eq!(pet.available_level(4717, 5, &max), 1);
        assert_eq!(pet.available_level(4717, 45, &max), 4);
        assert_eq!(pet.available_level(4717, 69, &max), 6);
        assert_eq!(pet.available_level(4717, 70, &max), 7);
        assert_eq!(pet.available_level(4717, 80, &max), 9);
        assert_eq!(pet.available_level(4717, 200, &|_| 8), 8);
        // Fixed levels from their `minLevel` on, the highest that applies.
        assert_eq!(
            pet.available_level(4718, 10, &max),
            1,
            "found but nothing yet: level 1"
        );
        assert_eq!(pet.available_level(4718, 20, &max), 2);
        assert_eq!(pet.available_level(4718, 45, &max), 3);
        assert_eq!(pet.available_level(9999, 45, &max), 0);
    }

    #[test]
    fn food_comes_in_the_order_of_a_java_hash_set() {
        let table = PetDataTable::from_xml(&sample()).unwrap();
        let pet = table.pet_data(12780).unwrap();
        assert_eq!(pet.food_in_file_order(), &[7582, 5168, 5169]);
        // Buckets of a 16 slot table: 7582 & 15 = 14, 5168 & 15 = 0, 5169 & 15 = 1.
        assert_eq!(pet.food(), vec![5168, 5169, 7582]);
    }

    #[test]
    fn speed_on_ride_fills_the_ride_speeds() {
        let xml = r#"<pets><pet id="1" itemId="2"><stats><stat level="1">
            <set name="max_meal" val="1"/><set name="exp" val="0"/><set name="get_exp_type" val="1"/>
            <set name="consume_meal_in_battle" val="1"/><set name="consume_meal_in_normal" val="1"/>
            <set name="org_pattack" val="1"/><set name="org_pdefend" val="1"/><set name="org_mattack" val="1"/>
            <set name="org_mdefend" val="1"/><set name="org_hp" val="1"/><set name="org_mp" val="1"/>
            <set name="org_hp_regen" val="1"/><set name="org_mp_regen" val="1"/>
            <set name="soulshot_count" val="1"/><set name="spiritshot_count" val="1"/>
            <set name="speed_on_ride" val="0" walk="130" run="140" slowSwim="70" fastSwim="80" fastFly="300"/>
            <set name="attack_speed_on_ride" val="350"/></stat></stats></pet></pets>"#;
        let table = PetDataTable::from_xml(xml).unwrap();
        let level = table.pet_level_data(1, 1).unwrap();
        assert_eq!(
            (
                level.walk_speed_on_ride,
                level.run_speed_on_ride,
                level.slow_swim_speed_on_ride,
                level.fast_swim_speed_on_ride,
                level.slow_fly_speed_on_ride,
                level.fast_fly_speed_on_ride
            ),
            (130.0, 140.0, 70.0, 80.0, 0.0, 300.0)
        );
    }

    #[test]
    fn a_level_without_a_required_number_is_an_error() {
        let xml = r#"<pets><pet id="1" itemId="2"><stats><stat level="1"><set name="exp" val="0"/></stat></stats></pet></pets>"#;
        assert!(matches!(PetDataTable::from_xml(xml), Err(DataError::Invalid(_))));
    }

    #[test]
    fn mountable_pets_are_the_striders_wyverns_and_wolves() {
        assert!(is_mountable(12526));
        assert!(is_mountable(12621));
        assert!(is_mountable(16041));
        assert!(!is_mountable(12780));
        assert!(!is_mountable(12564));
    }
}
