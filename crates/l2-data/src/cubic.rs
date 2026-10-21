//! Cubic templates: what a cubic does, how often, for how long and with which skills.
//!
//! Java: `gameserver/data/xml/CubicData.java` and `gameserver/entity/actor/templates/
//! CubicTemplate.java`, `gameserver/entity/cubic/{CubicSkill,CubicTargetType,CubicSelectionMode,
//! ICubicConditionHolder}.java`, `gameserver/entity/cubic/conditions/*.java`. Data:
//! `data/stats/cubics/*.xml` (`xsd/cubics.xsd`). What a cubic does in the world (the timers, the
//! target choice, the skill casts) is `l2_world::summon::cubic`.
//!
//! One `<cubic id level ...>` is a template; the key is (id, level). A later template of the same
//! key replaces the earlier one (Java `put`). Java reads the attributes through a `StatSet`, so a
//! missing required attribute or an unknown enum name throws and ends the file: the templates
//! read before it stay (same here: the loader returns the error from the closure).

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::Arc;

use serde::Deserialize;
use tracing::warn;

use crate::loader::{DataError, load_dir, log_loaded};

/// Java `CubicTargetType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CubicTargetType {
    /// The owner's target (the default).
    Target,
    /// Each skill names its own target type.
    BySkill,
    /// The party member (or summon) with the lowest HP.
    Heal,
    /// The cubic's owner.
    Master,
}

impl CubicTargetType {
    /// `Enum.valueOf`.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "TARGET" => Self::Target,
            "BY_SKILL" => Self::BySkill,
            "HEAL" => Self::Heal,
            "MASTER" => Self::Master,
            _ => return None,
        })
    }
}

/// Java `CubicSelectionMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CubicSelectionMode {
    /// A random normal (not priority) skill (the default).
    Random,
    /// A skill by its trigger rate as a weight.
    Weighted,
    /// The first normal skill whose trigger rate passes.
    Ordered,
}

impl CubicSelectionMode {
    /// `Enum.valueOf`.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "RANDOM" => Self::Random,
            "WEIGHTED" => Self::Weighted,
            "ORDERED" => Self::Ordered,
            _ => return None,
        })
    }
}

/// Java `HpCondition.HpConditionType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HpConditionType {
    Greater,
    Lesser,
}

/// Java `ICubicCondition`: `HpCondition`, `RangeCondition` and `HealthCondition`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CubicCondition {
    /// `<hp type percent>`: the target's HP percent is above or below `percent`.
    Hp { kind: HpConditionType, percent: i32 },
    /// `<range value>`: the target is at most `value` away (2D) from the owner.
    Range(i32),
    /// `<healthPercent min max>`: the target's HP percent is strictly between `min` and `max`.
    HealthPercent { min: i32, max: i32 },
}

/// Java `CubicSkill`: one skill of a cubic with its own rate, target and conditions.
#[derive(Debug, Clone, PartialEq)]
pub struct CubicSkill {
    /// `SkillHolder` id.
    pub skill_id: i32,
    /// `SkillHolder` level.
    pub skill_level: i32,
    /// `triggerRate` (default 100).
    pub trigger_rate: i32,
    /// `target` (default `TARGET`); read when the template's target type is `BY_SKILL`.
    pub target_type: CubicTargetType,
    /// `targetDebuff`: only when the target has a dispellable debuff.
    pub target_debuff: bool,
    /// `priority`: tried before the normal skills on every run.
    pub priority: bool,
    /// The skill's `<conditions>`.
    pub conditions: Vec<CubicCondition>,
}

/// Java `CubicTemplate`.
#[derive(Debug, Clone, PartialEq)]
pub struct CubicTemplate {
    pub id: i32,
    pub level: i32,
    pub slot: i32,
    /// Seconds the cubic lasts.
    pub duration: i32,
    /// Seconds between two runs.
    pub delay: i32,
    /// `maxCount`: how many skills it uses before it stops (-1 = unlimited when given by a skill).
    pub max_count: i32,
    /// `useUp`: above 0, the cubic is removed after `max_count` uses.
    pub use_up: i32,
    /// `power` (default 0).
    pub power: f64,
    pub target_type: CubicTargetType,
    pub selection_mode: CubicSelectionMode,
    /// The template's own `<conditions>`.
    pub conditions: Vec<CubicCondition>,
    /// The `<skills>` in file order.
    pub skills: Vec<CubicSkill>,
}

impl CubicTemplate {
    /// Java `getBaseMAtk()` (and `getBasePAtk()`): `(int) _power`.
    pub fn base_m_atk(&self) -> i32 {
        self.power as i32
    }
}

/// Java `CubicData`: the templates by cubic id and level. Immutable once loaded.
#[derive(Debug, Clone, Default)]
pub struct CubicData {
    cubics: HashMap<i32, BTreeMap<i32, Arc<CubicTemplate>>>,
}

impl CubicData {
    /// Loads `data/stats/cubics` below `root`.
    pub fn load(root: &Path) -> Self {
        let mut data = Self::default();
        load_dir(&root.join("data/stats/cubics"), true, |doc: RawFile| {
            data.add(doc)
        });
        let count = data.len();
        if count == 0 {
            // Java keeps the previous data; there is none at the first load.
            warn!("CubicData: No cubic templates loaded. Keeping previous data.");
        } else {
            log_loaded("CubicData", count, "cubic templates");
        }
        data
    }

    /// Builds the table from the text of one file. For tests and tools.
    pub fn from_xml(xml: &str) -> Result<Self, DataError> {
        let doc = quick_xml::de::from_str(xml).map_err(|source| DataError::Xml {
            path: "<inline>".into(),
            source,
        })?;
        let mut data = Self::default();
        data.add(doc)?;
        Ok(data)
    }

    /// Java `getCubicTemplate(id, level)`.
    pub fn get(&self, id: i32, level: i32) -> Option<&Arc<CubicTemplate>> {
        self.cubics.get(&id).and_then(|levels| levels.get(&level))
    }

    /// The number of templates (Java sums the levels of every cubic).
    pub fn len(&self) -> usize {
        self.cubics.values().map(BTreeMap::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.cubics.is_empty()
    }

    fn add(&mut self, doc: RawFile) -> Result<(), DataError> {
        for raw in doc.cubics {
            let template = raw.into_template()?;
            self.cubics
                .entry(template.id)
                .or_default()
                .insert(template.level, Arc::new(template));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------------- raw XML

#[derive(Deserialize)]
struct RawFile {
    #[serde(rename = "cubic", default)]
    cubics: Vec<RawCubic>,
}

#[derive(Deserialize)]
struct RawCubic {
    #[serde(rename = "@id")]
    id: i32,
    #[serde(rename = "@level")]
    level: i32,
    #[serde(rename = "@slot")]
    slot: i32,
    #[serde(rename = "@duration")]
    duration: i32,
    #[serde(rename = "@delay")]
    delay: i32,
    #[serde(rename = "@maxCount")]
    max_count: i32,
    #[serde(rename = "@useUp")]
    use_up: i32,
    #[serde(rename = "@power", default)]
    power: f64,
    #[serde(rename = "@targetType")]
    target_type: Option<String>,
    #[serde(rename = "@selectionMode")]
    selection_mode: Option<String>,
    conditions: Option<RawConditions>,
    skills: Option<RawSkills>,
}

#[derive(Deserialize, Default)]
struct RawConditions {
    #[serde(rename = "$value", default)]
    children: Vec<RawCondition>,
}

#[derive(Deserialize)]
enum RawCondition {
    #[serde(rename = "hp")]
    Hp(RawHp),
    #[serde(rename = "range")]
    Range(RawRange),
    #[serde(rename = "healthPercent")]
    HealthPercent(RawHealth),
    /// Java: "Unsupported cubic condition".
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct RawHp {
    #[serde(rename = "@type")]
    kind: String,
    #[serde(rename = "@percent")]
    percent: i32,
}

#[derive(Deserialize)]
struct RawRange {
    #[serde(rename = "@value")]
    value: i32,
}

#[derive(Deserialize)]
struct RawHealth {
    #[serde(rename = "@min")]
    min: i32,
    #[serde(rename = "@max")]
    max: i32,
}

#[derive(Deserialize, Default)]
struct RawSkills {
    #[serde(rename = "skill", default)]
    skills: Vec<RawSkill>,
}

#[derive(Deserialize)]
struct RawSkill {
    #[serde(rename = "@id")]
    id: i32,
    #[serde(rename = "@level")]
    level: i32,
    #[serde(rename = "@triggerRate", default = "default_trigger_rate")]
    trigger_rate: i32,
    #[serde(rename = "@target")]
    target: Option<String>,
    #[serde(rename = "@targetDebuff", default)]
    target_debuff: bool,
    #[serde(rename = "@priority", default)]
    priority: bool,
    conditions: Option<RawConditions>,
}

fn default_trigger_rate() -> i32 {
    100
}

fn conditions(raw: Option<RawConditions>, owner: &str) -> Result<Vec<CubicCondition>, DataError> {
    let mut out = Vec::new();
    for child in raw.unwrap_or_default().children {
        out.push(match child {
            RawCondition::Hp(hp) => CubicCondition::Hp {
                kind: match hp.kind.as_str() {
                    "GREATER" => HpConditionType::Greater,
                    "LESSER" => HpConditionType::Lesser,
                    other => {
                        return Err(DataError::Invalid(format!(
                            "cubic {owner}: unknown hp condition type {other}"
                        )));
                    }
                },
                percent: hp.percent,
            },
            RawCondition::Range(range) => CubicCondition::Range(range.value),
            RawCondition::HealthPercent(health) => CubicCondition::HealthPercent {
                min: health.min,
                max: health.max,
            },
            RawCondition::Other => {
                warn!("Unsupported cubic condition: cubic {owner}");
                continue;
            }
        });
    }
    Ok(out)
}

fn target_type(name: Option<&str>, what: &str) -> Result<CubicTargetType, DataError> {
    match name {
        None => Ok(CubicTargetType::Target),
        Some(name) => CubicTargetType::from_name(name)
            .ok_or_else(|| DataError::Invalid(format!("{what}: unknown cubic target type {name}"))),
    }
}

impl RawCubic {
    fn into_template(self) -> Result<CubicTemplate, DataError> {
        let owner = format!("{} level {}", self.id, self.level);
        let target = target_type(self.target_type.as_deref(), &owner)?;
        let selection_mode = match self.selection_mode.as_deref() {
            None => CubicSelectionMode::Random,
            Some(name) => CubicSelectionMode::from_name(name)
                .ok_or_else(|| DataError::Invalid(format!("{owner}: unknown selection mode {name}")))?,
        };
        let template_conditions = conditions(self.conditions, &owner)?;
        let mut skills = Vec::new();
        for raw in self.skills.unwrap_or_default().skills {
            skills.push(CubicSkill {
                skill_id: raw.id,
                skill_level: raw.level,
                trigger_rate: raw.trigger_rate,
                target_type: target_type(raw.target.as_deref(), &owner)?,
                target_debuff: raw.target_debuff,
                priority: raw.priority,
                conditions: conditions(raw.conditions, &owner)?,
            });
        }
        Ok(CubicTemplate {
            id: self.id,
            level: self.level,
            slot: self.slot,
            duration: self.duration,
            delay: self.delay,
            max_count: self.max_count,
            use_up: self.use_up,
            power: self.power,
            target_type: target,
            selection_mode,
            conditions: template_conditions,
            skills,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<list>
	<!-- a comment -->
	<cubic id="1" level="1" slot="1" duration="900" delay="10" maxCount="30" useUp="1" power="282.0" selectionMode="RANDOM" targetType="TARGET">
		<skills>
			<skill id="4049" level="1" />
		</skills>
	</cubic>
	<cubic id="10" level="1" slot="10" duration="900" delay="10" maxCount="50" useUp="0" power="2106.0" targetType="BY_SKILL">
		<conditions>
			<hp type="GREATER" percent="10" />
			<range value="900" />
		</conditions>
		<skills>
			<skill id="4165" level="9" target="TARGET" triggerRate="40" />
			<skill id="5579" level="1" target="MASTER" targetDebuff="true" priority="true">
				<conditions><healthPercent min="0" max="100" /></conditions>
			</skill>
		</skills>
	</cubic>
</list>"#;

    #[test]
    fn parses_templates_skills_and_conditions() {
        let data = CubicData::from_xml(SAMPLE).unwrap();
        assert_eq!(data.len(), 2);
        let strike = data.get(1, 1).unwrap();
        assert_eq!((strike.slot, strike.duration, strike.delay), (1, 900, 10));
        assert_eq!((strike.max_count, strike.use_up), (30, 1));
        assert_eq!(strike.base_m_atk(), 282);
        assert_eq!(strike.target_type, CubicTargetType::Target);
        assert_eq!(strike.selection_mode, CubicSelectionMode::Random);
        assert_eq!(strike.skills.len(), 1);
        // Java defaults: triggerRate 100, target TARGET, no debuff test, no priority.
        let skill = &strike.skills[0];
        assert_eq!(
            (skill.skill_id, skill.skill_level, skill.trigger_rate),
            (4049, 1, 100)
        );
        assert_eq!(skill.target_type, CubicTargetType::Target);
        assert!(!skill.target_debuff && !skill.priority && skill.conditions.is_empty());

        let smart = data.get(10, 1).unwrap();
        assert_eq!(smart.target_type, CubicTargetType::BySkill);
        // `selectionMode` defaults to RANDOM.
        assert_eq!(smart.selection_mode, CubicSelectionMode::Random);
        assert_eq!(
            smart.conditions,
            vec![
                CubicCondition::Hp {
                    kind: HpConditionType::Greater,
                    percent: 10
                },
                CubicCondition::Range(900)
            ]
        );
        assert_eq!(smart.skills[0].trigger_rate, 40);
        let cure = &smart.skills[1];
        assert_eq!(cure.target_type, CubicTargetType::Master);
        assert!(cure.target_debuff && cure.priority);
        assert_eq!(
            cure.conditions,
            vec![CubicCondition::HealthPercent { min: 0, max: 100 }]
        );
    }

    #[test]
    fn a_later_template_of_the_same_key_replaces_the_earlier_one() {
        let data = CubicData::from_xml(
            r#"<list>
<cubic id="3" level="1" slot="3" duration="1" delay="1" maxCount="1" useUp="0" targetType="HEAL"/>
<cubic id="3" level="1" slot="3" duration="2" delay="1" maxCount="1" useUp="0" targetType="HEAL"/>
</list>"#,
        )
        .unwrap();
        assert_eq!(data.len(), 1);
        assert_eq!(data.get(3, 1).unwrap().duration, 2);
        assert_eq!(data.get(3, 1).unwrap().power, 0.0, "power defaults to 0");
    }

    #[test]
    fn an_unknown_enum_name_or_missing_attribute_is_an_error() {
        assert!(
            CubicData::from_xml(
                r#"<list><cubic id="1" level="1" slot="1" duration="1" delay="1" maxCount="1" useUp="0" targetType="NOPE"/></list>"#
            )
            .is_err()
        );
        assert!(
            CubicData::from_xml(
                r#"<list><cubic id="1" level="1" slot="1" delay="1" maxCount="1" useUp="0"/></list>"#
            )
            .is_err()
        );
    }

    #[test]
    fn real_datapack_when_present() {
        let root = std::env::var_os("L2JMOBIUS_GAME_DIR").map_or_else(
            || std::path::PathBuf::from(r"C:\l2-rust\l2jmobius\dist\game"),
            Into::into,
        );
        if !root.join("data/stats/cubics").is_dir() {
            eprintln!("skipped: no cubics folder under {}", root.display());
            return;
        }
        let data = CubicData::load(&root);
        // 14 files: 12+12+12+11+12+12+12+12+9 levels and the five smart cubics.
        assert_eq!(data.len(), 109);
        // Storm Cubic level 1: Cubic Drain (4049), power 282.
        let storm = data.get(1, 1).unwrap();
        assert_eq!(storm.base_m_atk(), 282);
        assert_eq!(storm.skills[0].skill_id, 4049);
        // Life Cubic heals.
        assert_eq!(data.get(3, 1).unwrap().target_type, CubicTargetType::Heal);
        // Smart Cubic - Eva's Templar: by-skill targets and a priority cure.
        let smart = data.get(10, 1).unwrap();
        assert_eq!(smart.target_type, CubicTargetType::BySkill);
        let cure = smart.skills.iter().find(|s| s.skill_id == 5579).unwrap();
        assert!(cure.priority && cure.target_debuff);
        assert_eq!(cure.target_type, CubicTargetType::Master);
    }
}
