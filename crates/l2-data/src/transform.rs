//! Transformations: the stats, skills, action list, item rules and level tables a player gets
//! while transformed.
//!
//! Java: `gameserver/data/xml/TransformData.java` and `gameserver/entity/actor/transform/
//! {Transform,TransformTemplate,TransformLevelData,TransformType,AdditionalSkillHolder,
//! AdditionalItemHolder}.java`. Data: `data/stats/transformations/*.xml` (only that folder, not its
//! subfolders: Java `parseDatapackDirectory(..., false)`). What a transformation does to a
//! player (the stat hooks, the skill list, ...) is the world's (`l2_world::transform`).
//!
//! # How Java reads a file (and what is ported as is)
//! One `StatSet` per `<transform>` collects the attributes of the element, then the attributes of
//! every `base`, `stats`, `defense`, `magicDefense`, `collision` and `moving` element of every
//! `common` block of the Male and the Female part. A `TransformTemplate` reads that set when it
//! is made. Three consequences are ported as they are:
//! - NOTE(java-quirk): the set is shared by the two sexes. A Female part whose `common` leaves
//!   out `stats` or `defense` (transformation 101 does) gets the values the Male part read.
//! - NOTE(java-quirk): a part without a `common` block gets its template from the set as it
//!   stands when its first `skills`/`actions`/`additionalSkills`/`items`/`levels` element comes
//!   (the stance transformations 312..318 have no `common` at all: collision 0, attack range 40,
//!   no stats).
//! - NOTE(java-quirk): `levels` keeps one entry only. Java collects the attributes of every
//!   `<level>` into one `StatSet` and builds the single `TransformLevelData` after the loop, so
//!   the attributes of the last `<level>` (level 99 in the datapack) overwrite the others.
//!   A transformed player below that level finds no level data: the template's own stats apply
//!   and the level modifier stays the class one.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use crate::armor_set::SkillHolder;
use crate::item::WeaponType;
use crate::loader::{DataError, load_dir, log_loaded};
use crate::player_template::PAPERDOLL_TOTAL_SLOTS;
use crate::stats::jnum::{number_format_message, parse_boolean, parse_double, parse_int};
use crate::stats::{Stat, XmlNode};

/// Java `TransformType`, in Java's declaration order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformType {
    /// `MODE_CHANGE` (a stance: not `isTransformed()`, but `isInStance()`).
    ModeChange,
    /// `NON_COMBAT`.
    NonCombat,
    /// `CURSED`.
    Cursed,
    /// `RIDING_MODE`.
    RidingMode,
    /// `PURE_STAT`.
    PureStat,
    /// `FLYING`.
    Flying,
    /// `COMBAT` (the default).
    Combat,
}

impl TransformType {
    /// `Enum.valueOf`: the exact Java constant name.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "MODE_CHANGE" => Self::ModeChange,
            "NON_COMBAT" => Self::NonCombat,
            "CURSED" => Self::Cursed,
            "RIDING_MODE" => Self::RidingMode,
            "PURE_STAT" => Self::PureStat,
            "FLYING" => Self::Flying,
            "COMBAT" => Self::Combat,
            _ => return None,
        })
    }
}

/// Java `AdditionalSkillHolder`: a skill the player gets from `min_level` on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdditionalSkill {
    /// Skill id.
    pub id: i32,
    /// Skill level.
    pub level: i32,
    /// The player level from which the skill is given.
    pub min_level: i32,
}

/// Java `AdditionalItemHolder`: an item id the inventory block lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdditionalItem {
    /// Item id.
    pub id: i32,
    /// `allowed`: in the "only these" list (block mode 1) or the "not these" list (mode 2).
    pub allowed: bool,
}

/// Java `TransformLevelData`: HP, MP, CP and their regeneration at one level.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransformLevelData {
    /// `val`.
    pub level: i32,
    /// `levelMod`.
    pub level_mod: f64,
    /// `hp` (`Stat.MAX_HP`).
    pub hp: f64,
    /// `mp` (`Stat.MAX_MP`).
    pub mp: f64,
    /// `cp` (`Stat.MAX_CP`).
    pub cp: f64,
    /// `hpRegen` (`Stat.REGENERATE_HP_RATE`).
    pub hp_regen: f64,
    /// `mpRegen` (`Stat.REGENERATE_MP_RATE`).
    pub mp_regen: f64,
    /// `cpRegen` (`Stat.REGENERATE_CP_RATE`).
    pub cp_regen: f64,
}

impl TransformLevelData {
    /// Java `getStats(stat)`: 0 for a stat the level data does not carry.
    pub fn stat(&self, stat: Stat) -> f64 {
        match stat {
            Stat::MaxHp => self.hp,
            Stat::MaxMp => self.mp,
            Stat::MaxCp => self.cp,
            Stat::RegenerateHpRate => self.hp_regen,
            Stat::RegenerateMpRate => self.mp_regen,
            Stat::RegenerateCpRate => self.cp_regen,
            _ => 0.0,
        }
    }
}

/// Java `MoveType` speeds of a template (`walk`, `run`, `waterWalk`, `waterRun`, `flyWalk`,
/// `flyRun`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TransformSpeeds {
    /// `MoveType.WALK`.
    pub walk: f64,
    /// `MoveType.RUN`.
    pub run: f64,
    /// `MoveType.SLOW_SWIM` (`waterWalk`).
    pub slow_swim: f64,
    /// `MoveType.FAST_SWIM` (`waterRun`).
    pub fast_swim: f64,
    /// `MoveType.SLOW_FLY` (`flyWalk`).
    pub slow_fly: f64,
    /// `MoveType.FAST_FLY` (`flyRun`).
    pub fast_fly: f64,
}

/// Java `TransformTemplate`: one sex of a transformation.
#[derive(Debug, Clone, PartialEq)]
pub struct TransformTemplate {
    /// `radius`.
    pub collision_radius: f32,
    /// `height`.
    pub collision_height: f32,
    /// `attackType` (default `FIST`).
    pub base_attack_type: WeaponType,
    /// `range` (default 40; the `POWER_ATTACK_RANGE` stat reads the same attribute with
    /// default 0).
    pub base_attack_range: i32,
    /// `randomDamage`.
    pub base_random_damage: f64,
    /// The `skills` block: skills given while transformed.
    pub skills: Vec<SkillHolder>,
    /// The `additionalSkills` block.
    pub additional_skills: Vec<AdditionalSkill>,
    /// The `items` block.
    pub additional_items: Vec<AdditionalItem>,
    /// `getDefense(slot)` by paperdoll slot (`chest`, `legs`, `head`, `feet`, `gloves`,
    /// `underwear`, `cloak`, `rear`, `lear`, `rfinger`, `lfinger`, `neck`); 0 elsewhere.
    pub defense: [i32; PAPERDOLL_TOTAL_SLOTS],
    /// `getStats(stat)`: pAtk, mAtk, range, attackSpeed, critRate and the six base stats.
    stats: Vec<(Stat, f64)>,
    /// The `moving` speeds.
    pub speeds: TransformSpeeds,
    /// The `actions` block: the action ids of the `ExBasicActionList`. `None` without one.
    pub actions: Option<Vec<i32>>,
    levels: BTreeMap<i32, TransformLevelData>,
}

impl TransformTemplate {
    /// Java `getStats(stat)`: 0 for a stat the template does not carry.
    pub fn stat(&self, stat: Stat) -> f64 {
        self.stats
            .iter()
            .find(|(s, _)| *s == stat)
            .map_or(0.0, |(_, v)| *v)
    }

    /// Java `getDefense(slot)`.
    pub fn defense(&self, slot: usize) -> i32 {
        self.defense.get(slot).copied().unwrap_or(0)
    }

    /// Java `getData(level)`.
    pub fn level_data(&self, level: i32) -> Option<&TransformLevelData> {
        self.levels.get(&level)
    }

    /// Java `hasBasicActionList()`.
    pub fn has_basic_action_list(&self) -> bool {
        self.actions.is_some()
    }
}

/// Java `Transform`.
#[derive(Debug, Clone, PartialEq)]
pub struct Transform {
    /// `id`.
    pub id: i32,
    /// `displayId` (default: the id): what the client shows.
    pub display_id: i32,
    /// `type` (default `COMBAT`).
    pub kind: TransformType,
    /// `can_swim == 1` (default 0).
    pub can_swim: bool,
    /// `normal_attackable == 1` (default 1).
    pub can_attack: bool,
    /// `spawn_height`.
    pub spawn_height: i32,
    /// `setName`: the visible name while transformed.
    pub name: Option<String>,
    /// `setTitle`: the visible title while transformed (may be empty).
    pub title: Option<String>,
    male: Option<TransformTemplate>,
    female: Option<TransformTemplate>,
}

impl Transform {
    /// Java `getTemplate(player)`: the template of the player's sex.
    pub fn template(&self, female: bool) -> Option<&TransformTemplate> {
        if female {
            self.female.as_ref()
        } else {
            self.male.as_ref()
        }
    }

    /// Java `isStance()`.
    pub fn is_stance(&self) -> bool {
        self.kind == TransformType::ModeChange
    }

    /// Java `isCombat()`.
    pub fn is_combat(&self) -> bool {
        self.kind == TransformType::Combat
    }

    /// Java `isNonCombat()`.
    pub fn is_non_combat(&self) -> bool {
        self.kind == TransformType::NonCombat
    }

    /// Java `isFlying()`.
    pub fn is_flying(&self) -> bool {
        self.kind == TransformType::Flying
    }

    /// Java `isCursed()`.
    pub fn is_cursed(&self) -> bool {
        self.kind == TransformType::Cursed
    }

    /// Java `isRiding()`.
    pub fn is_riding(&self) -> bool {
        self.kind == TransformType::RidingMode
    }

    /// Java `isPureStats()`.
    pub fn is_pure_stats(&self) -> bool {
        self.kind == TransformType::PureStat
    }

    /// Java `canUseWeaponStats()`.
    pub fn can_use_weapon_stats(&self) -> bool {
        matches!(self.kind, TransformType::Combat | TransformType::ModeChange)
    }
}

/// Every transformation by id. Java `TransformData`.
#[derive(Debug, Default)]
pub struct Transforms {
    by_id: BTreeMap<i32, Transform>,
}

impl Transforms {
    const LOADER: &'static str = "TransformData";

    /// Loads every `.xml` file directly inside `<root>/data/stats/transformations` and logs
    /// Java's count line (`TransformData: Loaded N transform templates.`).
    pub fn load(root: &Path) -> Self {
        let mut table = Self::default();
        load_dir(
            &root.join("data/stats/transformations"),
            false,
            |doc: RawTransformFile| table.add(doc),
        );
        log_loaded(Self::LOADER, table.by_id.len(), "transform templates");
        table
    }

    /// Builds a table from the text of one file. For tests and tools.
    pub fn from_xml(xml: &str) -> Result<Self, DataError> {
        let doc = quick_xml::de::from_str(xml).map_err(|source| DataError::Xml {
            path: "<inline>".into(),
            source,
        })?;
        let mut table = Self::default();
        table.add(doc)?;
        Ok(table)
    }

    fn add(&mut self, doc: RawTransformFile) -> Result<(), DataError> {
        for node in &doc.transforms {
            // Java puts a transform into the map once it is fully read: a file stops at the
            // first broken one and keeps those before it.
            let transform = parse_transform(node)?;
            self.by_id.insert(transform.id, transform);
        }
        Ok(())
    }

    /// Java `getTransform(id)`.
    pub fn get(&self, id: i32) -> Option<&Transform> {
        self.by_id.get(&id)
    }

    /// Number of transformations.
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// True when nothing was loaded.
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}

fn invalid(message: String) -> DataError {
    DataError::Invalid(message)
}

/// The shared `StatSet` of one `<transform>`: attribute name to value.
type StatSet = BTreeMap<String, String>;

fn set_int(set: &StatSet, key: &str, default: i32) -> Result<i32, DataError> {
    match set.get(key) {
        None => Ok(default),
        Some(text) => {
            parse_int(text).ok_or_else(|| invalid(format!("Integer value required, but found: {text}")))
        }
    }
}

fn set_double(set: &StatSet, key: &str, default: f64) -> Result<f64, DataError> {
    match set.get(key) {
        None => Ok(default),
        Some(text) => {
            parse_double(text).ok_or_else(|| invalid(format!("Double value required, but found: {text}")))
        }
    }
}

fn set_float(set: &StatSet, key: &str, default: f32) -> Result<f32, DataError> {
    Ok(set_double(set, key, f64::from(default))? as f32)
}

/// Java `StatSet.getDouble(key)` without a default: a missing key throws.
fn set_double_required(set: &StatSet, key: &str) -> Result<f64, DataError> {
    if !set.contains_key(key) {
        return Err(invalid(format!(
            "Double value required, but not specified: {key}"
        )));
    }
    set_double(set, key, 0.0)
}

/// Java `IXmlReader.parseInteger(attrs, name)`: a missing or bad attribute fails the file.
fn attr_int(node: &XmlNode, name: &str) -> Result<i32, DataError> {
    let text = node
        .attr(name)
        .ok_or_else(|| invalid(format!("missing attribute {name} in <{}>", node.name)))?;
    parse_int(text).ok_or_else(|| invalid(number_format_message(text)))
}

fn attr_bool(node: &XmlNode, name: &str) -> Result<bool, DataError> {
    node.attr(name)
        .map(parse_boolean)
        .ok_or_else(|| invalid(format!("missing attribute {name} in <{}>", node.name)))
}

/// Java `new TransformTemplate(set)`.
fn new_template(set: &StatSet) -> Result<TransformTemplate, DataError> {
    let attack_type = match set.get("attackType") {
        None => WeaponType::Fist,
        Some(name) => WeaponType::from_name(name).ok_or_else(|| {
            invalid(format!(
                "Enum value of type WeaponType required, but found: {name}"
            ))
        })?,
    };
    let speeds = TransformSpeeds {
        walk: set_double(set, "walk", 0.0)?,
        run: set_double(set, "run", 0.0)?,
        slow_swim: set_double(set, "waterWalk", 0.0)?,
        fast_swim: set_double(set, "waterRun", 0.0)?,
        slow_fly: set_double(set, "flyWalk", 0.0)?,
        fast_fly: set_double(set, "flyRun", 0.0)?,
    };
    let int = |key: &str| set_int(set, key, 0).map(f64::from);
    let stats = vec![
        (Stat::PowerAttack, set_double(set, "pAtk", 0.0)?),
        (Stat::MagicAttack, set_double(set, "mAtk", 0.0)?),
        (Stat::PowerAttackRange, int("range")?),
        (Stat::PowerAttackSpeed, int("attackSpeed")?),
        (Stat::CriticalRate, int("critRate")?),
        (Stat::StatStr, int("str")?),
        (Stat::StatInt, int("int")?),
        (Stat::StatCon, int("con")?),
        (Stat::StatDex, int("dex")?),
        (Stat::StatWit, int("wit")?),
        (Stat::StatMen, int("men")?),
    ];
    // Java `Inventory.PAPERDOLL_*`.
    let mut defense = [0; PAPERDOLL_TOTAL_SLOTS];
    for (slot, key) in [
        (6, "chest"),
        (11, "legs"),
        (1, "head"),
        (12, "feet"),
        (10, "gloves"),
        (0, "underwear"),
        (23, "cloak"),
        (8, "rear"),
        (9, "lear"),
        (13, "rfinger"),
        (14, "lfinger"),
        (4, "neck"),
    ] {
        defense[slot] = set_int(set, key, 0)?;
    }
    Ok(TransformTemplate {
        collision_radius: set_float(set, "radius", 0.0)?,
        collision_height: set_float(set, "height", 0.0)?,
        base_attack_type: attack_type,
        base_attack_range: set_int(set, "range", 40)?,
        base_random_damage: set_double(set, "randomDamage", 0.0)?,
        skills: Vec::new(),
        additional_skills: Vec::new(),
        additional_items: Vec::new(),
        defense,
        stats,
        speeds,
        actions: None,
        levels: BTreeMap::new(),
    })
}

/// Java `new TransformLevelData(levelsSet)`.
fn new_level_data(set: &StatSet) -> Result<TransformLevelData, DataError> {
    let level = match set.get("val") {
        None => return Err(invalid("Integer value required, but not specified: val!".into())),
        Some(text) => {
            parse_int(text).ok_or_else(|| invalid(format!("Integer value required, but found: {text}!")))?
        }
    };
    Ok(TransformLevelData {
        level,
        level_mod: set_double_required(set, "levelMod")?,
        hp: set_double_required(set, "hp")?,
        mp: set_double_required(set, "mp")?,
        cp: set_double_required(set, "cp")?,
        hp_regen: set_double_required(set, "hpRegen")?,
        mp_regen: set_double_required(set, "mpRegen")?,
        cp_regen: set_double_required(set, "cpRegen")?,
    })
}

/// Java `TransformData.parseDocument`, one `<transform>`.
fn parse_transform(node: &XmlNode) -> Result<Transform, DataError> {
    let mut set: StatSet = node.attrs.iter().cloned().collect();

    // Java `new Transform(set)`.
    let id = match set.get("id") {
        None => return Err(invalid("Integer value required, but not specified: id!".into())),
        Some(text) => {
            parse_int(text).ok_or_else(|| invalid(format!("Integer value required, but found: {text}!")))?
        }
    };
    let kind = match set.get("type") {
        None => TransformType::Combat,
        Some(name) => TransformType::from_name(name).ok_or_else(|| {
            invalid(format!(
                "Enum value of type TransformType required, but found: {name}"
            ))
        })?,
    };
    let mut transform = Transform {
        id,
        display_id: set_int(&set, "displayId", id)?,
        kind,
        can_swim: set_int(&set, "can_swim", 0)? == 1,
        can_attack: set_int(&set, "normal_attackable", 1)? == 1,
        spawn_height: set_int(&set, "spawn_height", 0)?,
        name: set.get("setName").cloned(),
        title: set.get("setTitle").cloned(),
        male: None,
        female: None,
    };

    for part in &node.children {
        let is_male = part.name.eq_ignore_ascii_case("Male");
        if !is_male && !part.name.eq_ignore_ascii_case("Female") {
            continue;
        }
        // Java keeps the template as an object that `setTemplate` hands to the transform when it
        // is made and that the later elements fill in; here it is handed over at the end.
        let mut template: Option<TransformTemplate> = None;
        for block in &part.children {
            match block.name.as_str() {
                "common" => {
                    for entry in &block.children {
                        if matches!(
                            entry.name.as_str(),
                            "base" | "stats" | "defense" | "magicDefense" | "collision" | "moving"
                        ) {
                            for (key, value) in &entry.attrs {
                                set.insert(key.clone(), value.clone());
                            }
                        }
                    }
                    template = Some(new_template(&set)?);
                }
                "skills" | "actions" | "additionalSkills" | "items" | "levels" => {
                    if template.is_none() {
                        template = Some(new_template(&set)?);
                    }
                    if let Some(template) = template.as_mut() {
                        fill_template(template, block, &mut set)?;
                    }
                }
                _ => {}
            }
        }
        if template.is_some() {
            if is_male {
                transform.male = template;
            } else {
                transform.female = template;
            }
        }
    }
    Ok(transform)
}

/// The `skills`, `actions`, `additionalSkills`, `items` and `levels` blocks of a part.
fn fill_template(
    template: &mut TransformTemplate,
    block: &XmlNode,
    set: &mut StatSet,
) -> Result<(), DataError> {
    match block.name.as_str() {
        "skills" => {
            for skill in block.children.iter().filter(|s| s.name == "skill") {
                template.skills.push(SkillHolder {
                    id: attr_int(skill, "id")?,
                    level: attr_int(skill, "level")?,
                });
            }
        }
        "actions" => {
            // Java `set.getIntArray("actions", " ")` over the text content.
            let text = block.text_content();
            set.insert("actions".to_owned(), text.clone());
            let mut actions = Vec::new();
            for piece in crate::stats::jnum::java_split(&text, ' ') {
                actions.push(
                    parse_int(piece)
                        .ok_or_else(|| invalid(format!("Integer value required, but found: {text}")))?,
                );
            }
            template.actions = Some(actions);
        }
        "additionalSkills" => {
            for skill in block.children.iter().filter(|s| s.name == "skill") {
                template.additional_skills.push(AdditionalSkill {
                    id: attr_int(skill, "id")?,
                    level: attr_int(skill, "level")?,
                    min_level: attr_int(skill, "minLevel")?,
                });
            }
        }
        "items" => {
            for item in block.children.iter().filter(|s| s.name == "item") {
                template.additional_items.push(AdditionalItem {
                    id: attr_int(item, "id")?,
                    allowed: attr_bool(item, "allowed")?,
                });
            }
        }
        "levels" => {
            // NOTE(java-quirk): see the module docs: one `StatSet` for every `<level>`, one
            // `TransformLevelData` after the loop.
            let mut levels_set = StatSet::new();
            for level in block.children.iter().filter(|s| s.name == "level") {
                for (key, value) in &level.attrs {
                    levels_set.insert(key.clone(), value.clone());
                }
            }
            let data = new_level_data(&levels_set)?;
            template.levels.insert(data.level, data);
        }
        _ => {}
    }
    Ok(())
}

// ---- Raw XML shape (xsd/transformations.xsd) --------------------------------------------------

// NOTE: Java matches the element names `list` and `transform` with `equalsIgnoreCase`; serde
// matches `transform` exactly (every file of the datapack spells it so).
#[derive(Deserialize)]
struct RawTransformFile {
    #[serde(rename = "transform", default)]
    transforms: Vec<XmlNode>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<list>
	<transform id="7" displayId="70" type="FLYING" can_swim="1" normal_attackable="0" spawn_height="300" setName="Zed" setTitle="">
		<Male>
			<common>
				<base range="20" attackSpeed="300" attackType="SWORD" critRate="5" mAtk="6" pAtk="5" randomDamage="10" />
				<stats str="40" int="21" con="43" dex="30" wit="11" men="25" />
				<defense chest="31" legs="18" head="12" feet="7" gloves="8" underwear="3" cloak="1" />
				<magicDefense rear="9" lear="9" rfinger="5" lfinger="5" neck="13" />
				<collision radius="12" height="14.5" />
				<moving walk="30" run="125" waterWalk="50" waterRun="51" flyWalk="96" flyRun="176" />
			</common>
			<actions>1 2 3 1000</actions>
			<skills>
				<skill id="584" level="1" /> <!-- comment -->
				<skill id="619" level="1" />
			</skills>
			<additionalSkills>
				<skill id="884" level="2" minLevel="76" />
			</additionalSkills>
			<items>
				<item id="9819" allowed="true" />
				<item id="57" allowed="false" />
			</items>
			<levels>
				<level val="1" levelMod="0.9" hp="80.8" mp="30.3" cp="40.4" hpRegen="2.0" mpRegen="0.9" cpRegen="2.0" />
				<level val="2" levelMod="0.91" hp="92.7" mp="35.8" cp="46.3" hpRegen="2.05" mpRegen="0.9" cpRegen="2.0" />
			</levels>
		</Male>
		<Female>
			<common>
				<base range="22" attackSpeed="300" attackType="BOW" critRate="5" mAtk="6" pAtk="5" randomDamage="10" />
				<collision radius="10" height="24" />
				<moving walk="50" run="115" />
			</common>
		</Female>
	</transform>
	<transform id="312" type="MODE_CHANGE">
		<Male>
			<actions>0 1</actions>
			<additionalSkills>
				<skill id="28" level="19" minLevel="44" />
			</additionalSkills>
		</Male>
	</transform>
</list>"#;

    fn table() -> Transforms {
        Transforms::from_xml(XML).unwrap()
    }

    #[test]
    fn header_attributes_and_defaults() {
        let t = table();
        assert_eq!(t.len(), 2);
        let flying = t.get(7).unwrap();
        assert_eq!((flying.display_id, flying.kind), (70, TransformType::Flying));
        assert!(flying.can_swim && !flying.can_attack && flying.is_flying());
        assert_eq!(flying.spawn_height, 300);
        // `setTitle=""` is a title (not null): the player shows an empty title.
        assert_eq!(flying.name.as_deref(), Some("Zed"));
        assert_eq!(flying.title.as_deref(), Some(""));
        let stance = t.get(312).unwrap();
        // No displayId: the id. No type attribute would be COMBAT; no can_swim: false;
        // no normal_attackable: true.
        assert_eq!(stance.display_id, 312);
        assert!(stance.is_stance() && !stance.can_swim && stance.can_attack);
        assert!(stance.can_use_weapon_stats());
        assert_eq!((stance.name.clone(), stance.title.clone()), (None, None));
        assert!(t.get(8).is_none());
    }

    #[test]
    fn male_template_values() {
        let t = table();
        let m = t.get(7).unwrap().template(false).unwrap();
        assert_eq!((m.collision_radius, m.collision_height), (12.0, 14.5));
        assert_eq!((m.base_attack_type, m.base_attack_range), (WeaponType::Sword, 20));
        assert_eq!(m.base_random_damage, 10.0);
        assert_eq!(m.stat(Stat::PowerAttack), 5.0);
        assert_eq!(m.stat(Stat::MagicAttack), 6.0);
        assert_eq!(m.stat(Stat::PowerAttackRange), 20.0);
        assert_eq!(m.stat(Stat::PowerAttackSpeed), 300.0);
        assert_eq!(m.stat(Stat::StatCon), 43.0);
        assert_eq!(m.stat(Stat::MaxHp), 0.0);
        // chest, legs, head, feet, gloves, underwear, cloak and the five jewel slots.
        assert_eq!(m.defense(6), 31);
        assert_eq!(m.defense(11), 18);
        assert_eq!(m.defense(1), 12);
        assert_eq!(m.defense(23), 1);
        assert_eq!(m.defense(4), 13);
        assert_eq!(m.defense(5), 0);
        assert_eq!(
            m.speeds,
            TransformSpeeds {
                walk: 30.0,
                run: 125.0,
                slow_swim: 50.0,
                fast_swim: 51.0,
                slow_fly: 96.0,
                fast_fly: 176.0
            }
        );
        assert_eq!(m.actions.as_deref(), Some(&[1, 2, 3, 1000][..]));
        assert_eq!(m.skills.len(), 2);
        assert_eq!(m.skills[1], SkillHolder { id: 619, level: 1 });
        assert_eq!(
            m.additional_skills,
            [AdditionalSkill {
                id: 884,
                level: 2,
                min_level: 76
            }]
        );
        assert_eq!(
            m.additional_items,
            [
                AdditionalItem {
                    id: 9819,
                    allowed: true
                },
                AdditionalItem {
                    id: 57,
                    allowed: false
                }
            ]
        );
    }

    #[test]
    fn female_inherits_the_shared_set() {
        let t = table();
        let f = t.get(7).unwrap().template(true).unwrap();
        // The Female `common` brings its own base, collision and moving ...
        assert_eq!((f.base_attack_type, f.base_attack_range), (WeaponType::Bow, 22));
        assert_eq!((f.collision_radius, f.collision_height), (10.0, 24.0));
        assert_eq!((f.speeds.walk, f.speeds.run), (50.0, 115.0));
        // ... and, as in Java, the stats and defence the Male part read stay in the shared set;
        // the moving values the Female part leaves out too.
        assert_eq!(f.stat(Stat::StatStr), 40.0);
        assert_eq!(f.defense(6), 31);
        assert_eq!(f.speeds.fast_fly, 176.0);
        // The Female part has no actions, skills or levels of its own.
        assert!(f.actions.is_none() && f.skills.is_empty() && f.level_data(1).is_none());
    }

    #[test]
    fn only_the_last_level_survives() {
        let t = table();
        let m = t.get(7).unwrap().template(false).unwrap();
        assert!(m.level_data(1).is_none());
        let data = m.level_data(2).unwrap();
        assert_eq!(data.level_mod, 0.91);
        assert_eq!(data.stat(Stat::MaxHp), 92.7);
        assert_eq!(data.stat(Stat::RegenerateCpRate), 2.0);
        assert_eq!(data.stat(Stat::PowerAttack), 0.0);
    }

    #[test]
    fn part_without_common_gets_a_default_template() {
        let t = table();
        let s = t.get(312).unwrap().template(false).unwrap();
        assert_eq!((s.collision_radius, s.collision_height), (0.0, 0.0));
        // `getInt("range", 40)` for the attack range, `getInt("range", 0)` for the stat.
        assert_eq!(s.base_attack_range, 40);
        assert_eq!(s.stat(Stat::PowerAttackRange), 0.0);
        assert_eq!(s.base_attack_type, WeaponType::Fist);
        assert_eq!(s.actions.as_deref(), Some(&[0, 1][..]));
        assert_eq!(s.additional_skills.len(), 1);
        assert!(t.get(312).unwrap().template(true).is_none());
    }

    #[test]
    fn a_broken_transform_stops_the_file() {
        let xml = r#"<list>
<transform id="1"><Male><common><base attackType="NOPE"/></common></Male></transform>
</list>"#;
        let mut table = Transforms::default();
        let doc = quick_xml::de::from_str(xml).unwrap();
        assert!(table.add(doc).is_err());
        assert!(table.is_empty());

        // Level data needs every attribute.
        let xml = r#"<list>
<transform id="2"><Male><levels><level val="1" levelMod="1"/></levels></Male></transform>
</list>"#;
        assert!(Transforms::from_xml(xml).is_err());
    }

    #[test]
    fn real_datapack_when_present() {
        let root = std::env::var_os("L2JMOBIUS_GAME_DIR").map_or_else(
            || std::path::PathBuf::from(r"C:\l2-rust\l2jmobius\dist\game"),
            Into::into,
        );
        if !root.join("data/stats/transformations").is_dir() {
            eprintln!("skipped: no transformations folder under {}", root.display());
            return;
        }
        let t = Transforms::load(&root);
        assert_eq!(t.len(), 111);
        // Transformation 1: a combat form with both sexes, male values from the file.
        let one = t.get(1).unwrap();
        assert_eq!(one.kind, TransformType::Combat);
        let male = one.template(false).unwrap();
        assert_eq!(male.collision_height, 14.5);
        assert_eq!(male.speeds.run, 125.0);
        assert_eq!(male.skills.len(), 4);
        // Only level 99 is kept (the Java quirk).
        let data = male.level_data(99).unwrap();
        assert_eq!((data.hp, data.level_mod), (2348.7085, 1.88));
        assert!(male.level_data(1).is_none());
        // The cursed weapon forms rename the player.
        assert_eq!(t.get(301).unwrap().name.as_deref(), Some("Zariche"));
        assert_eq!(t.get(301).unwrap().title.as_deref(), Some(""));
        // A stance without a `common` block.
        let stance = t.get(312).unwrap();
        assert!(stance.is_stance());
        assert_eq!(stance.template(false).unwrap().base_attack_range, 40);
    }
}
