//! Player class templates: base stats, speeds, collision, creation points and per-level HP/MP/CP.
//!
//! Java: `gameserver/data/xml/PlayerTemplateData.java`, `entity/actor/templates/PlayerTemplate.java`
//! and the defaults in `CreatureTemplate.java`.
//! Data: `data/stats/players/templates/**/*.xml` (one file per class, `xsd/playerTemplate.xsd`).
//!
//! Reference loader: copy the layout of this file for other XML tables (see [`crate::loader`]).
//! Raw XML structs at the bottom, the public immutable table at the top.
//!
//! Not loaded (character creation and UserInfo do not use them, and Java only keeps them in its
//! generic `StatSet`): `baseAtkType` (the schema only allows `FIST`), `baseCanPenetrate`,
//! `baseDamRange`, the per-slot paperdoll defence (only the sums are kept).

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use crate::class_list::{ClassId, Race};
use crate::loader::{DataError, load_dir, log_loaded};

/// The highest player level in `experience.xml` (`maxLevel`) and Java's `MaximumPlayerLevel`
/// default. Java reads it from `ExperienceData`; until that loader is ported it is a constant.
pub const DEFAULT_MAX_PLAYER_LEVEL: usize = 85;

/// A position in the world. Java `entity/Location`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Location {
    /// X coordinate.
    pub x: i32,
    /// Y coordinate.
    pub y: i32,
    /// Z (height) coordinate.
    pub z: i32,
}

/// Collision cylinder of a character model.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Collision {
    /// Radius, as sent to the client.
    pub radius: f32,
    /// Height, as sent to the client.
    pub height: f32,
}

/// Java `PlayerTemplate` (plus the `CreatureTemplate` fields the player data fills in).
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerTemplate {
    /// The class this template describes.
    pub class_id: ClassId,
    /// Race of the class (Java `PlayerClass.getRace()`).
    pub race: Race,

    /// Base STR.
    pub base_str: i32,
    /// Base CON.
    pub base_con: i32,
    /// Base DEX.
    pub base_dex: i32,
    /// Base INT.
    pub base_int: i32,
    /// Base WIT.
    pub base_wit: i32,
    /// Base MEN.
    pub base_men: i32,

    /// Base physical attack.
    pub p_atk: i32,
    /// Base magical attack.
    pub m_atk: i32,
    /// Physical defence: chest + legs + head + feet + gloves + underwear + cloak.
    pub p_def: i32,
    /// Magical defence: rear ear + left ear + right finger + right finger + neck.
    pub m_def: i32,
    /// Base physical attack speed.
    pub p_atk_spd: i32,
    /// Base magical attack speed. Not in the XML: Java's `CreatureTemplate` default.
    pub m_atk_spd: i32,
    /// Base critical rate.
    pub crit_rate: i32,
    /// Base attack range.
    pub atk_range: i32,
    /// Random damage spread.
    pub rnd_dam: i32,
    /// Base breath time.
    pub breath: i32,
    /// Safe fall height.
    pub safe_fall: i32,

    /// Walk speed.
    pub walk_speed: f64,
    /// Run speed.
    pub run_speed: f64,
    /// Swim speed while walking (Java `SLOW_SWIM`).
    pub slow_swim_speed: f64,
    /// Swim speed while running (Java `FAST_SWIM`).
    pub fast_swim_speed: f64,

    /// Collision of male characters.
    pub collision_male: Collision,
    /// Collision of female characters.
    pub collision_female: Collision,

    /// Where a new character of this class can spawn. Java picks one with `Rnd.get(size)`; callers
    /// use the world RNG (D-13) to pick an index.
    pub creation_points: Vec<Location>,

    // Per-level tables, indexed by level (index 0 unused, like Java). Length is max level + 1.
    hp: Vec<f32>,
    mp: Vec<f32>,
    cp: Vec<f32>,
    hp_regen: Vec<f64>,
    mp_regen: Vec<f64>,
    cp_regen: Vec<f64>,
}

impl PlayerTemplate {
    /// Java `getBaseHpMax(level)`; `None` above the loaded max level (Java throws).
    pub fn base_hp_max(&self, level: usize) -> Option<f32> {
        self.hp.get(level).copied()
    }

    /// Java `getBaseMpMax(level)`.
    pub fn base_mp_max(&self, level: usize) -> Option<f32> {
        self.mp.get(level).copied()
    }

    /// Java `getBaseCpMax(level)`.
    pub fn base_cp_max(&self, level: usize) -> Option<f32> {
        self.cp.get(level).copied()
    }

    /// Java `getBaseHpRegen(level)`.
    pub fn base_hp_regen(&self, level: usize) -> Option<f64> {
        self.hp_regen.get(level).copied()
    }

    /// Java `getBaseMpRegen(level)`.
    pub fn base_mp_regen(&self, level: usize) -> Option<f64> {
        self.mp_regen.get(level).copied()
    }

    /// Java `getBaseCpRegen(level)`.
    pub fn base_cp_regen(&self, level: usize) -> Option<f64> {
        self.cp_regen.get(level).copied()
    }
}

/// All player templates, immutable after loading. Java `PlayerTemplateData`.
#[derive(Debug, Default)]
pub struct PlayerTemplates {
    templates: BTreeMap<ClassId, PlayerTemplate>,
    level_up_gain_count: usize,
}

impl PlayerTemplates {
    const LOADER: &'static str = "PlayerTemplateData";

    /// Loads every file below `<root>/data/stats/players/templates` and logs Java's two count lines
    /// (`Loaded N player templates.`, `Loaded N level up gain records.`). `max_level` is the highest
    /// player level (see [`DEFAULT_MAX_PLAYER_LEVEL`]).
    pub fn load(root: &Path, max_level: usize) -> Self {
        let mut table = Self::default();
        load_dir(
            &root.join("data/stats/players/templates"),
            true,
            |doc: RawTemplateFile| table.add(doc, max_level),
        );
        log_loaded(Self::LOADER, table.templates.len(), "player templates");
        log_loaded(Self::LOADER, table.level_up_gain_count, "level up gain records");
        table
    }

    /// Builds a table from the text of one template file. For tests and tools.
    pub fn from_xml(xml: &str, max_level: usize) -> Result<Self, DataError> {
        let doc = quick_xml::de::from_str(xml).map_err(|source| DataError::Xml {
            path: "<inline>".into(),
            source,
        })?;
        let mut table = Self::default();
        table.add(doc, max_level)?;
        Ok(table)
    }

    fn add(&mut self, doc: RawTemplateFile, max_level: usize) -> Result<(), DataError> {
        let class_id = u16::try_from(doc.class_id)
            .map(ClassId)
            .ok()
            .ok_or_else(|| DataError::Invalid(format!("unknown class id {}", doc.class_id)))?;
        let race = class_id
            .race()
            .ok_or_else(|| DataError::Invalid(format!("unknown class id {}", doc.class_id)))?;
        let s = doc.static_data;

        let mut template = PlayerTemplate {
            class_id,
            race,
            base_str: s.base_str,
            base_con: s.base_con,
            base_dex: s.base_dex,
            base_int: s.base_int,
            base_wit: s.base_wit,
            base_men: s.base_men,
            p_atk: s.base_p_atk,
            m_atk: s.base_m_atk,
            p_def: s.base_p_def.chest
                + s.base_p_def.legs
                + s.base_p_def.head
                + s.base_p_def.feet
                + s.base_p_def.gloves
                + s.base_p_def.underwear
                + s.base_p_def.cloak,
            // NOTE(java-quirk): Java adds `rfinger` twice and never `lfinger`.
            m_def: s.base_m_def.rear
                + s.base_m_def.lear
                + s.base_m_def.rfinger
                + s.base_m_def.rfinger
                + s.base_m_def.neck,
            p_atk_spd: s.base_p_atk_spd,
            m_atk_spd: 333,
            crit_rate: s.base_crit_rate,
            atk_range: s.base_atk_range,
            rnd_dam: s.base_rnd_dam,
            breath: s.base_breath,
            safe_fall: s.base_safe_fall,
            walk_speed: 0.0,
            run_speed: 0.0,
            slow_swim_speed: 0.0,
            fast_swim_speed: 0.0,
            collision_male: Collision {
                radius: s.collision_male.radius,
                height: s.collision_male.height,
            },
            collision_female: Collision {
                radius: s.collision_female.radius,
                height: s.collision_female.height,
            },
            creation_points: s
                .creation_points
                .nodes
                .iter()
                .map(|n| Location {
                    x: n.x,
                    y: n.y,
                    z: n.z,
                })
                .collect(),
            hp: vec![0.0; max_level + 1],
            mp: vec![0.0; max_level + 1],
            cp: vec![0.0; max_level + 1],
            hp_regen: vec![0.0; max_level + 1],
            mp_regen: vec![0.0; max_level + 1],
            cp_regen: vec![0.0; max_level + 1],
        };
        // Java CreatureTemplate: swimming falls back to the matching ground speed.
        let m = s.base_move_spd;
        template.run_speed = m.run.unwrap_or(120.0);
        template.walk_speed = m.walk.unwrap_or(50.0);
        template.fast_swim_speed = m.fast_swim.unwrap_or(template.run_speed);
        template.slow_swim_speed = m.slow_swim.unwrap_or(template.walk_speed);

        // Java stores the template before reading the level table, so a broken level table keeps
        // the template (with zeroed rows). Same here.
        let mut result = Ok(());
        for level in doc.lvl_upgain_data.levels {
            let Ok(index) = usize::try_from(level.val) else {
                result = Err(DataError::Invalid(format!("negative level {}", level.val)));
                break;
            };
            // NOTE(java-quirk): Java `return`s from the whole file at the first level above the max,
            // ignoring everything after it. Files list levels in order, so this equals skipping them.
            if index > max_level {
                break;
            }
            // `+ 6` records at most; Java counts every hp*/mp*/cp* child it finds.
            let mut count = 0;
            for (value, table) in [
                (level.hp, &mut template.hp),
                (level.mp, &mut template.mp),
                (level.cp, &mut template.cp),
            ] {
                if let Some(v) = value {
                    table[index] = v as f32;
                    count += 1;
                }
            }
            for (value, table) in [
                (level.hp_regen, &mut template.hp_regen),
                (level.mp_regen, &mut template.mp_regen),
                (level.cp_regen, &mut template.cp_regen),
            ] {
                if let Some(v) = value {
                    table[index] = v;
                    count += 1;
                }
            }
            self.level_up_gain_count += count;
        }
        self.templates.insert(class_id, template);
        result
    }

    /// Java `getTemplate(PlayerClass)`.
    pub fn get(&self, class_id: ClassId) -> Option<&PlayerTemplate> {
        self.templates.get(&class_id)
    }

    /// Java `getTemplate(int)`.
    pub fn get_by_id(&self, id: i32) -> Option<&PlayerTemplate> {
        u16::try_from(id).ok().and_then(|id| self.get(ClassId(id)))
    }

    /// All templates, ordered by class id.
    pub fn iter(&self) -> impl Iterator<Item = &PlayerTemplate> {
        self.templates.values()
    }

    /// Number of templates.
    pub fn len(&self) -> usize {
        self.templates.len()
    }

    /// True when nothing was loaded.
    pub fn is_empty(&self) -> bool {
        self.templates.is_empty()
    }

    /// Java's second count line: number of hp/mp/cp/regen values read from the level tables.
    pub fn level_up_gain_count(&self) -> usize {
        self.level_up_gain_count
    }
}

// ---- Raw XML shape (xsd/playerTemplate.xsd) ------------------------------------------------
// Defaults are Java's `CreatureTemplate.set` defaults. Fields without `default` are the ones
// Java reads without a default (a missing value makes Java skip the file).

#[derive(Deserialize)]
struct RawTemplateFile {
    #[serde(rename = "classId")]
    class_id: i32,
    #[serde(rename = "staticData")]
    static_data: RawStaticData,
    #[serde(rename = "lvlUpgainData")]
    lvl_upgain_data: RawLvlUpgainData,
}

#[derive(Deserialize)]
struct RawStaticData {
    #[serde(rename = "baseSTR", default)]
    base_str: i32,
    #[serde(rename = "baseCON", default)]
    base_con: i32,
    #[serde(rename = "baseDEX", default)]
    base_dex: i32,
    #[serde(rename = "baseINT", default)]
    base_int: i32,
    #[serde(rename = "baseWIT", default)]
    base_wit: i32,
    #[serde(rename = "baseMEN", default)]
    base_men: i32,
    #[serde(rename = "creationPoints", default)]
    creation_points: RawCreationPoints,
    #[serde(rename = "basePAtk", default)]
    base_p_atk: i32,
    #[serde(rename = "baseMAtk", default)]
    base_m_atk: i32,
    #[serde(rename = "basePDef", default)]
    base_p_def: RawPDef,
    #[serde(rename = "baseMDef", default)]
    base_m_def: RawMDef,
    #[serde(rename = "basePAtkSpd", default = "default_p_atk_spd")]
    base_p_atk_spd: i32,
    #[serde(rename = "baseCritRate", default = "default_crit_rate")]
    base_crit_rate: i32,
    #[serde(rename = "baseAtkRange", default = "default_atk_range")]
    base_atk_range: i32,
    #[serde(rename = "baseRndDam", default)]
    base_rnd_dam: i32,
    #[serde(rename = "baseMoveSpd", default)]
    base_move_spd: RawMoveSpd,
    #[serde(rename = "baseBreath", default = "default_breath")]
    base_breath: i32,
    #[serde(rename = "baseSafeFall", default = "default_safe_fall")]
    base_safe_fall: i32,
    #[serde(rename = "collisionMale", default)]
    collision_male: RawCollisionMale,
    #[serde(rename = "collisionFemale")]
    collision_female: RawCollisionFemale,
}

fn default_p_atk_spd() -> i32 {
    300
}
fn default_crit_rate() -> i32 {
    4
}
fn default_atk_range() -> i32 {
    40
}
fn default_breath() -> i32 {
    100
}
fn default_safe_fall() -> i32 {
    333
}

#[derive(Deserialize, Default)]
struct RawCreationPoints {
    #[serde(rename = "node", default)]
    nodes: Vec<RawNode>,
}

#[derive(Deserialize)]
struct RawNode {
    #[serde(rename = "@x")]
    x: i32,
    #[serde(rename = "@y")]
    y: i32,
    #[serde(rename = "@z")]
    z: i32,
}

#[derive(Deserialize, Default)]
struct RawPDef {
    #[serde(default)]
    chest: i32,
    #[serde(default)]
    legs: i32,
    #[serde(default)]
    head: i32,
    #[serde(default)]
    feet: i32,
    #[serde(default)]
    gloves: i32,
    #[serde(default)]
    underwear: i32,
    #[serde(default)]
    cloak: i32,
}

#[derive(Deserialize, Default)]
struct RawMDef {
    #[serde(default)]
    rear: i32,
    #[serde(default)]
    lear: i32,
    #[serde(default)]
    rfinger: i32,
    // Present in the XML but Java never reads it (see the m_def quirk).
    #[serde(default)]
    #[allow(dead_code)]
    lfinger: i32,
    #[serde(default)]
    neck: i32,
}

#[derive(Deserialize, Default)]
struct RawMoveSpd {
    walk: Option<f64>,
    run: Option<f64>,
    #[serde(rename = "slowSwim")]
    slow_swim: Option<f64>,
    #[serde(rename = "fastSwim")]
    fast_swim: Option<f64>,
}

#[derive(Deserialize, Default)]
struct RawCollisionMale {
    #[serde(default)]
    radius: f32,
    #[serde(default)]
    height: f32,
}

#[derive(Deserialize)]
struct RawCollisionFemale {
    radius: f32,
    height: f32,
}

#[derive(Deserialize)]
struct RawLvlUpgainData {
    #[serde(rename = "level", default)]
    levels: Vec<RawLevel>,
}

#[derive(Deserialize)]
struct RawLevel {
    #[serde(rename = "@val")]
    val: i32,
    hp: Option<f64>,
    mp: Option<f64>,
    cp: Option<f64>,
    #[serde(rename = "hpRegen")]
    hp_regen: Option<f64>,
    #[serde(rename = "mpRegen")]
    mp_regen: Option<f64>,
    #[serde(rename = "cpRegen")]
    cp_regen: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small but complete template file (class 31, Dark Elf Fighter) with `levels` level rows.
    fn xml(class_id: i32, levels: usize) -> String {
        let mut rows = String::new();
        for l in 1..=levels {
            rows.push_str(&format!(
                "<level val=\"{l}\"><hp>{}.5</hp><mp>{l}.25</mp><cp>{l}.75</cp>\
                 <hpRegen>2.0</hpRegen><mpRegen>0.9</mpRegen><cpRegen>1.5</cpRegen></level>",
                l * 10
            ));
        }
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<list xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:noNamespaceSchemaLocation="../../xsd/playerTemplate.xsd">
	<classId>{class_id}</classId>
	<staticData>
		<baseINT>25</baseINT><baseSTR>41</baseSTR><baseCON>32</baseCON>
		<baseMEN>26</baseMEN><baseDEX>34</baseDEX><baseWIT>12</baseWIT>
		<creationPoints>
			<node x="28295" y="11063" z="-4224" />
			<node x="28302" y="11008" z="-4224" />
		</creationPoints>
		<basePAtk>4</basePAtk><baseCritRate>5</baseCritRate><baseAtkType>FIST</baseAtkType>
		<basePAtkSpd>330</basePAtkSpd>
		<basePDef><chest>31</chest><legs>18</legs><head>12</head><feet>7</feet><gloves>8</gloves><underwear>3</underwear><cloak>1</cloak></basePDef>
		<baseMAtk>6</baseMAtk>
		<baseMDef><rear>9</rear><lear>8</lear><rfinger>5</rfinger><lfinger>4</lfinger><neck>13</neck></baseMDef>
		<baseAtkRange>20</baseAtkRange>
		<baseRndDam>10</baseRndDam>
		<baseMoveSpd><walk>85</walk><run>122</run><slowSwim>50</slowSwim><fastSwim>60</fastSwim></baseMoveSpd>
		<baseBreath>150</baseBreath>
		<baseSafeFall>350</baseSafeFall>
		<someFutureElement><x>1</x></someFutureElement>
		<collisionMale><radius>7.5</radius><height>24</height></collisionMale>
		<collisionFemale><radius>7.0</radius><height>23.5</height></collisionFemale>
	</staticData>
	<lvlUpgainData>{rows}</lvlUpgainData>
</list>"#
        )
    }

    #[test]
    fn parses_static_data() {
        let table = PlayerTemplates::from_xml(&xml(31, 3), 85).unwrap();
        let t = table.get(ClassId(31)).unwrap();
        assert_eq!(t.race, Race::DarkElf);
        assert_eq!((t.base_str, t.base_con, t.base_dex), (41, 32, 34));
        assert_eq!((t.base_int, t.base_wit, t.base_men), (25, 12, 26));
        assert_eq!((t.p_atk, t.m_atk, t.p_atk_spd, t.crit_rate), (4, 6, 330, 5));
        assert_eq!(
            (t.atk_range, t.rnd_dam, t.breath, t.safe_fall),
            (20, 10, 150, 350)
        );
        assert_eq!(t.p_def, 31 + 18 + 12 + 7 + 8 + 3 + 1);
        assert_eq!(t.m_atk_spd, 333);
        assert_eq!(
            t.collision_male,
            Collision {
                radius: 7.5,
                height: 24.0
            }
        );
        assert_eq!(
            t.collision_female,
            Collision {
                radius: 7.0,
                height: 23.5
            }
        );
        assert_eq!(
            t.creation_points,
            vec![
                Location {
                    x: 28295,
                    y: 11063,
                    z: -4224
                },
                Location {
                    x: 28302,
                    y: 11008,
                    z: -4224
                },
            ]
        );
        assert_eq!(
            (t.walk_speed, t.run_speed, t.slow_swim_speed, t.fast_swim_speed),
            (85.0, 122.0, 50.0, 60.0)
        );
        assert!(table.get_by_id(32).is_none());
    }

    #[test]
    fn magical_defence_counts_right_finger_twice_like_java() {
        let table = PlayerTemplates::from_xml(&xml(31, 1), 85).unwrap();
        // rear 9 + lear 8 + rfinger 5 + rfinger 5 + neck 13 (lfinger 4 is ignored)
        assert_eq!(table.get(ClassId(31)).unwrap().m_def, 40);
    }

    #[test]
    fn level_tables_are_indexed_by_level() {
        let table = PlayerTemplates::from_xml(&xml(31, 3), 85).unwrap();
        let t = table.get(ClassId(31)).unwrap();
        assert_eq!(t.base_hp_max(0), Some(0.0));
        assert_eq!(t.base_hp_max(1), Some(10.5));
        assert_eq!(t.base_hp_max(3), Some(30.5));
        assert_eq!(t.base_mp_max(2), Some(2.25));
        assert_eq!(t.base_cp_max(2), Some(2.75));
        assert_eq!(t.base_hp_regen(2), Some(2.0));
        assert_eq!(t.base_mp_regen(2), Some(0.9));
        assert_eq!(t.base_cp_regen(2), Some(1.5));
        assert_eq!(t.base_hp_max(4), Some(0.0)); // row missing in the file
        assert_eq!(t.base_hp_max(86), None); // beyond the table
        assert_eq!(table.level_up_gain_count(), 18);
    }

    #[test]
    fn hp_mp_cp_are_single_precision_like_java() {
        let xml = xml(31, 1).replace("<hp>10.5</hp>", "<hp>0.1</hp>");
        let table = PlayerTemplates::from_xml(&xml, 85).unwrap();
        assert_eq!(table.get(ClassId(31)).unwrap().base_hp_max(1), Some(0.1_f32));
    }

    #[test]
    fn levels_above_the_max_are_dropped() {
        let table = PlayerTemplates::from_xml(&xml(31, 6), 4).unwrap();
        let t = table.get(ClassId(31)).unwrap();
        assert_eq!(t.base_hp_max(4), Some(40.5));
        assert_eq!(t.base_hp_max(5), None);
        assert_eq!(table.level_up_gain_count(), 24);
    }

    #[test]
    fn missing_values_use_java_defaults() {
        let xml = r#"<list><classId>0</classId>
            <staticData><collisionFemale><radius>1</radius><height>2</height></collisionFemale></staticData>
            <lvlUpgainData/></list>"#;
        let table = PlayerTemplates::from_xml(xml, 85).unwrap();
        let t = table.get(ClassId(0)).unwrap();
        assert_eq!((t.base_str, t.p_atk, t.m_def), (0, 0, 0));
        assert_eq!((t.p_atk_spd, t.crit_rate, t.atk_range), (300, 4, 40));
        assert_eq!((t.breath, t.safe_fall), (100, 333));
        assert_eq!((t.run_speed, t.walk_speed), (120.0, 50.0));
        assert_eq!((t.fast_swim_speed, t.slow_swim_speed), (120.0, 50.0));
        assert_eq!(t.collision_male, Collision::default());
        assert!(t.creation_points.is_empty());
    }

    #[test]
    fn female_collision_is_required() {
        let xml = r#"<list><classId>0</classId><staticData/><lvlUpgainData/></list>"#;
        assert!(matches!(
            PlayerTemplates::from_xml(xml, 85),
            Err(DataError::Xml { .. })
        ));
    }

    #[test]
    fn unknown_class_id_is_rejected() {
        assert!(matches!(
            PlayerTemplates::from_xml(&xml(58, 1), 85),
            Err(DataError::Invalid(_))
        ));
        assert!(matches!(
            PlayerTemplates::from_xml(&xml(-1, 1), 85),
            Err(DataError::Invalid(_))
        ));
    }

    #[test]
    fn load_reads_subfolders_and_last_file_wins_for_a_class() {
        let dir = tempfile::tempdir().unwrap();
        let templates = dir.path().join("data/stats/players/templates");
        std::fs::create_dir_all(templates.join("1stClass")).unwrap();
        std::fs::create_dir_all(templates.join("2ndClass")).unwrap();
        std::fs::write(templates.join("1stClass/a.xml"), xml(31, 2)).unwrap();
        std::fs::write(templates.join("2ndClass/b.xml"), xml(32, 2)).unwrap();
        std::fs::write(templates.join("2ndClass/c.xml"), xml(32, 3)).unwrap();
        std::fs::write(templates.join("2ndClass/broken.xml"), "<list>").unwrap();
        let table = PlayerTemplates::load(dir.path(), 85);
        assert_eq!(table.len(), 2);
        assert_eq!(table.get(ClassId(32)).unwrap().base_hp_max(3), Some(30.5));
        assert_eq!(
            table.iter().map(|t| t.class_id.0).collect::<Vec<_>>(),
            vec![31, 32]
        );
    }
}
