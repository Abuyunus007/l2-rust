//! Augmentation options: what the two option ids of an augmentation give the player who wears the
//! item (stat functions, an active and a passive skill, chance skills).
//!
//! Java: `gameserver/data/xml/OptionData.java` and `mechanics/options/{Options,Augmentation,
//! OptionSkillHolder,OptionSkillType}.java`. Data: `data/stats/augmentation/options/*.xml`
//! (`xsd/optionsData.xsd`). Applying the options to a player is the world's
//! (`l2_world::inventory::augmentation`).
//!
//! An augmentation id packs two option ids: `id & 0xFFFF` and `id >> 16` (Java
//! `AugmentationStatBonus`).

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;
use tracing::info;

use crate::armor_set::SkillHolder;
use crate::loader::{DataError, load_dir, log_loaded};
use crate::skill::SkillTemplates;
use crate::stats::jnum::{number_format_message, parse_double, parse_int};
use crate::stats::{FuncTemplate, Stat, XmlNode};

/// Java `OptionSkillType`: which event rolls the chance of an activation skill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionSkillType {
    /// `attack_skill`.
    Attack,
    /// `magic_skill`.
    Magic,
    /// `critical_skill`.
    Critical,
}

/// Java `OptionSkillHolder`: a skill a chance triggers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OptionSkillHolder {
    /// The skill.
    pub skill: SkillHolder,
    /// The `chance` attribute.
    pub chance: f64,
    /// The event that rolls the chance.
    pub kind: OptionSkillType,
}

/// Java `Options`: one option id.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AugmentationOption {
    /// The option id.
    pub id: i32,
    /// The `<stats>` functions (`FuncTemplate(null, null, name, order, stat, val)`).
    pub funcs: Vec<FuncTemplate>,
    /// `active_skill`, when the skill exists.
    pub active_skill: Option<SkillHolder>,
    /// `passive_skill`, when the skill exists.
    pub passive_skill: Option<SkillHolder>,
    /// `attack_skill`, `magic_skill` and `critical_skill`, in file order.
    pub activation_skills: Vec<OptionSkillHolder>,
}

/// Java `OptionData`: every option by id.
#[derive(Debug, Default)]
pub struct AugmentationOptions {
    options: BTreeMap<i32, AugmentationOption>,
}

impl AugmentationOptions {
    const LOADER: &'static str = "OptionData";

    /// Loads `<root>/data/stats/augmentation/options/*.xml` and logs Java's count line. The
    /// skills an option names must exist in `skills` (Java `SkillData.getSkill`); one that does
    /// not is logged and left out.
    pub fn load(root: &Path, skills: &SkillTemplates) -> Self {
        let mut table = Self::default();
        load_dir(
            &root.join("data/stats/augmentation/options"),
            false,
            |doc: RawOptionsFile| table.add(doc, skills),
        );
        log_loaded(Self::LOADER, table.options.len(), "options");
        table
    }

    /// Builds a table from XML text. For tests and tools.
    pub fn from_xml(xml: &str, skills: &SkillTemplates) -> Result<Self, DataError> {
        let doc = quick_xml::de::from_str(xml).map_err(|source| DataError::Xml {
            path: "<inline>".into(),
            source,
        })?;
        let mut table = Self::default();
        table.add(doc, skills)?;
        Ok(table)
    }

    fn add(&mut self, doc: RawOptionsFile, skills: &SkillTemplates) -> Result<(), DataError> {
        for node in &doc.options {
            // Java puts every option into the map as soon as it is read, so the options of a file
            // before a broken one stay.
            let option = parse_option(node, skills)?;
            self.options.insert(option.id, option);
        }
        Ok(())
    }

    /// Java `OptionData.getOptions(id)`: `None` for an id without an option (and for a negative
    /// id, where Java would throw).
    pub fn get(&self, id: i32) -> Option<&AugmentationOption> {
        self.options.get(&id)
    }

    /// Number of options loaded.
    pub fn len(&self) -> usize {
        self.options.len()
    }

    /// True when nothing was loaded.
    pub fn is_empty(&self) -> bool {
        self.options.is_empty()
    }
}

/// The two option ids of an augmentation (Java `AugmentationStatBonus`): the low and the high
/// 16 bits.
pub fn augmentation_option_ids(augmentation_id: i32) -> [i32; 2] {
    [0x0000_FFFF & augmentation_id, augmentation_id >> 16]
}

fn invalid(message: String) -> DataError {
    DataError::Invalid(message)
}

/// Java `parseInteger(attrs, name)`: a missing or bad attribute throws and fails the file.
fn int_attr(node: &XmlNode, name: &str) -> Result<i32, DataError> {
    let text = node
        .attr(name)
        .ok_or_else(|| invalid(format!("missing attribute {name} in <{}>", node.name)))?;
    parse_int(text).ok_or_else(|| invalid(number_format_message(text)))
}

fn double_attr(node: &XmlNode, name: &str) -> Result<f64, DataError> {
    let text = node
        .attr(name)
        .ok_or_else(|| invalid(format!("missing attribute {name} in <{}>", node.name)))?;
    parse_double(text).ok_or_else(|| invalid(number_format_message(text)))
}

/// Java `OptionData.parseDocument`, one `<option>`.
fn parse_option(node: &XmlNode, skills: &SkillTemplates) -> Result<AugmentationOption, DataError> {
    let id = int_attr(node, "id")?;
    let mut option = AugmentationOption {
        id,
        ..AugmentationOption::default()
    };
    // The skill of an `*_skill` element, or Java's "Could not find skill" line.
    let skill_of = |child: &XmlNode| -> Result<Option<SkillHolder>, DataError> {
        let skill_id = int_attr(child, "id")?;
        let level = int_attr(child, "level")?;
        if skills.get_skill(skill_id, level).is_some() {
            Ok(Some(SkillHolder { id: skill_id, level }))
        } else {
            info!("OptionData: Could not find skill {skill_id}({level}) used by option {id}.");
            Ok(None)
        }
    };
    for child in &node.children {
        match child.name.as_str() {
            "stats" => {
                for func in &child.children {
                    if matches!(
                        func.name.as_str(),
                        "add" | "sub" | "mul" | "div" | "set" | "share" | "enchant" | "enchanthp"
                    ) {
                        option.funcs.push(parse_func(func)?);
                    }
                }
            }
            "active_skill" => option.active_skill = skill_of(child)?.or(option.active_skill),
            "passive_skill" => option.passive_skill = skill_of(child)?.or(option.passive_skill),
            "attack_skill" | "magic_skill" | "critical_skill" => {
                let kind = match child.name.as_str() {
                    "attack_skill" => OptionSkillType::Attack,
                    "magic_skill" => OptionSkillType::Magic,
                    _ => OptionSkillType::Critical,
                };
                if let Some(skill) = skill_of(child)? {
                    option.activation_skills.push(OptionSkillHolder {
                        skill,
                        chance: double_attr(child, "chance")?,
                        kind,
                    });
                }
            }
            _ => {}
        }
    }
    Ok(option)
}

/// Java `OptionData.parseFuncs`: `new FuncTemplate(null, null, functionName, order, stat, val)`.
fn parse_func(node: &XmlNode) -> Result<FuncTemplate, DataError> {
    let stat_name = node
        .attr("stat")
        .ok_or_else(|| invalid(format!("missing attribute stat in <{}>", node.name)))?;
    let stat = Stat::from_xml(stat_name)
        .ok_or_else(|| invalid(format!("Unknown name '{stat_name}' for enum BaseStats")))?;
    let value = double_attr(node, "val")?;
    let order = match node.attr("order") {
        Some(text) => parse_int(text).ok_or_else(|| invalid(number_format_message(text)))?,
        None => -1,
    };
    FuncTemplate::new(None, None, &node.name, order, stat, value).map_err(|e| invalid(e.to_string()))
}

// ---- Raw XML shape (xsd/optionsData.xsd) ------------------------------------------------------

#[derive(Deserialize)]
struct RawOptionsFile {
    #[serde(rename = "option", default)]
    options: Vec<XmlNode>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::FuncKind;

    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<list>
	<option id="1" name="o_f_46_1g_1">
		<stats>
			<add stat="pDef" val="15.45" />
			<mul stat="mAtk" val="1.1" order="25" />
		</stats>
	</option>
	<option id="2" name="skills">
		<active_skill id="100" level="2" />
		<passive_skill id="101" level="1" />
		<attack_skill id="100" level="1" chance="3.5" />
		<magic_skill id="999" level="1" chance="1" />
		<critical_skill id="101" level="1" chance="7" />
	</option>
</list>"#;

    fn no_skills() -> SkillTemplates {
        SkillTemplates::default()
    }

    #[test]
    fn stats_become_function_templates_with_java_orders() {
        let table = AugmentationOptions::from_xml(XML, &no_skills()).unwrap();
        assert_eq!(table.len(), 2);
        let option = table.get(1).unwrap();
        assert_eq!(option.funcs.len(), 2);
        assert_eq!(
            (option.funcs[0].stat, option.funcs[0].kind, option.funcs[0].order),
            (Stat::PowerDefence, FuncKind::Add, 30)
        );
        assert_eq!(option.funcs[0].value, 15.45);
        // The `order` attribute wins over the kind's default.
        assert_eq!((option.funcs[1].kind, option.funcs[1].order), (FuncKind::Mul, 25));
        assert!(option.funcs[0].attach_cond.is_none() && option.funcs[0].apply_cond.is_none());
    }

    #[test]
    fn skills_that_do_not_exist_are_left_out() {
        // No skill tables: every skill of option 2 is "not found".
        let table = AugmentationOptions::from_xml(XML, &no_skills()).unwrap();
        let option = table.get(2).unwrap();
        assert!(option.funcs.is_empty());
        assert_eq!((option.active_skill, option.passive_skill), (None, None));
        assert!(option.activation_skills.is_empty());
        assert!(table.get(3).is_none());
        assert!(table.get(-1).is_none());
    }

    #[test]
    fn existing_skills_are_kept_and_chances_read() {
        let skills = SkillTemplates::from_xml(
            r#"<list>
	<skill id="100" levels="2" name="A"><operateType>A1</operateType></skill>
	<skill id="101" levels="1" name="P"><operateType>P</operateType></skill>
</list>"#,
            &crate::skill::EnchantSkillGroups::default(),
            &crate::skill::SkillLoadOptions::default(),
        );
        let table = AugmentationOptions::from_xml(XML, &skills).unwrap();
        let option = table.get(2).unwrap();
        assert_eq!(option.active_skill, Some(SkillHolder { id: 100, level: 2 }));
        assert_eq!(option.passive_skill, Some(SkillHolder { id: 101, level: 1 }));
        // Skill 999 does not exist: its magic_skill is left out, the others stay in file order.
        let kinds: Vec<_> = option
            .activation_skills
            .iter()
            .map(|h| (h.kind, h.skill.id, h.chance))
            .collect();
        assert_eq!(
            kinds,
            [
                (OptionSkillType::Attack, 100, 3.5),
                (OptionSkillType::Critical, 101, 7.0)
            ]
        );
    }

    #[test]
    fn an_augmentation_id_holds_two_option_ids() {
        assert_eq!(augmentation_option_ids(0x0003_0001), [1, 3]);
        assert_eq!(augmentation_option_ids(0x0001_0000 | 0xFFFF), [0xFFFF, 1]);
        assert_eq!(augmentation_option_ids(5), [5, 0]);
    }

    #[test]
    fn a_broken_function_fails_the_file_but_keeps_the_options_before_it() {
        let xml = r#"<list>
	<option id="1"><stats><add stat="pDef" val="1"/></stats></option>
	<option id="2"><stats><add stat="notAStat" val="1"/></stats></option>
	<option id="3"><stats><add stat="pDef" val="1"/></stats></option>
</list>"#;
        let mut table = AugmentationOptions::default();
        let doc = quick_xml::de::from_str(xml).unwrap();
        assert!(table.add(doc, &no_skills()).is_err());
        assert_eq!(table.len(), 1);
        assert!(table.get(1).is_some());
    }
}
