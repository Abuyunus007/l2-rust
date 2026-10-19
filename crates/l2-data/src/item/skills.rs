//! Skills attached to an item template: ids and levels only, resolved against the skill table
//! when the item is used or equipped (M4/M5).
//!
//! Java: the `<skills>` branch of `util/DocumentItem.java`, `ItemSkillType`, and the readers of
//! the resulting `StatSet` keys in `ItemTemplate.set` (`item_skill`, `unequip_skill`),
//! `Weapon.set` and `Armor.set` (`enchant4_skill`, `onmagic_skill`, `oncrit_skill`).
//!
//! Like Java, `<skills>` only writes strings into the item's `StatSet` (`"id-level"`, several
//! `item_skill` entries joined with `;`) and the item template reads them back, so a plain
//! `<set name="item_skill" val="2025-1"/>` works the same way and the same parse rules and
//! warnings apply to both.

use tracing::{info, warn};

use super::ItemSet;
use crate::loader::DataError;
use crate::stats::XmlNode;
use crate::stats::jnum::{decode_i32, java_split, parse_int};

use super::types::java_enum;

java_enum! {
    /// Java `ItemSkillType`: the `type` attribute of an item `<skill>`.
    ItemSkillType {
        /// `NORMAL`: `item_skill`, the skill the item casts or gives while worn.
        Normal = "NORMAL",
        /// `ON_ENCHANT_4`: `enchant4_skill`.
        OnEnchant4 = "ON_ENCHANT_4",
        /// `ON_UNEQUIP`: `unequip_skill`.
        OnUnequip = "ON_UNEQUIP",
        /// `ON_CRITICAL_SKILL`: `oncrit_skill` (+ `oncrit_chance`).
        OnCriticalSkill = "ON_CRITICAL_SKILL",
        /// `ON_MAGIC_SKILL`: `onmagic_skill` (+ `onmagic_chance`).
        OnMagicSkill = "ON_MAGIC_SKILL",
    }
}

/// Java `SkillHolder`, without the lookup: a skill id and level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SkillRef {
    /// Skill id.
    pub id: i32,
    /// Skill level.
    pub level: i32,
}

/// A weapon skill that fires by chance: `Weapon._skillsOnCrit` / `_skillsOnMagic` together with
/// their `ConditionGameChance(chance)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChanceSkill {
    /// The skill.
    pub skill: SkillRef,
    /// Percent chance (`oncrit_chance` / `onmagic_chance`, default 100).
    pub chance: i32,
}

/// All skills an item can carry, by Java's `ItemSkillType`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ItemSkills {
    /// `ItemTemplate.getSkills()`: `None` when the item has no `item_skill` at all (Java `null`,
    /// `hasSkills()` false). Entries with skill id 0 or level 0 are left out.
    pub item_skills: Option<Vec<SkillRef>>,
    /// `unequip_skill` (`ItemTemplate._unequipSkill`, any item type).
    pub unequip_skill: Option<SkillRef>,
    /// `enchant4_skill`: weapons and armor only.
    pub enchant4_skill: Option<SkillRef>,
    /// `oncrit_skill`: weapons only.
    pub on_crit: Option<ChanceSkill>,
    /// `onmagic_skill`: weapons only.
    pub on_magic: Option<ChanceSkill>,
}

/// The `<skills>` branch of `DocumentItem.parseItem`: writes the `StatSet` keys.
pub(super) fn apply_skills_node(node: &XmlNode, set: &mut ItemSet) -> Result<(), DataError> {
    for child in &node.children {
        if !child.name.eq_ignore_ascii_case("skill") {
            continue;
        }
        // `parseInteger(attrs, "id")` is an `Integer.decode`; a missing attribute is a null that
        // fails on unboxing.
        let id = required_int(child, "id")?;
        let level = required_int(child, "level")?;
        let chance = match child.attr("type_chance") {
            Some(text) => decode(text)?,
            None => 100,
        };
        let skill_type = match child.attr("type") {
            None => ItemSkillType::Normal,
            Some(text) => ItemSkillType::from_name(text).unwrap_or_else(|| {
                warn!(
                    "Invalid value for node: type, specified value: {text} should be an enum of type \"ItemSkillType\". Using default value: NORMAL"
                );
                ItemSkillType::Normal
            }),
        };
        let pair = format!("{id}-{level}");
        match skill_type {
            ItemSkillType::Normal => {
                let joined = match set.string("item_skill") {
                    None => pair,
                    Some(existing) => format!("{existing};{pair}"),
                };
                set.0.insert("item_skill".into(), joined);
            }
            ItemSkillType::OnEnchant4 => {
                set.0.insert("enchant4_skill".into(), pair);
            }
            ItemSkillType::OnUnequip => {
                set.0.insert("unequip_skill".into(), pair);
            }
            ItemSkillType::OnCriticalSkill => {
                set.0.insert("oncrit_skill".into(), pair);
                set.0.insert("oncrit_chance".into(), chance.to_string());
            }
            ItemSkillType::OnMagicSkill => {
                set.0.insert("onmagic_skill".into(), pair);
                set.0.insert("onmagic_chance".into(), chance.to_string());
            }
        }
    }
    Ok(())
}

fn required_int(node: &XmlNode, name: &str) -> Result<i32, DataError> {
    let text = node
        .attr(name)
        .ok_or_else(|| DataError::Invalid(format!("skill without {name}")))?;
    decode(text)
}

fn decode(text: &str) -> Result<i32, DataError> {
    decode_i32(text).ok_or_else(|| DataError::Invalid(format!("integer value required, found: {text}")))
}

/// Reads the skill keys of the `StatSet` like `ItemTemplate.set`, `Weapon.set` and `Armor.set`.
/// `class` is the item class (`Weapon`, `Armor`, `EtcItem`), `label` is Java's `toString()`
/// (`name(id)`) for the log lines.
pub(super) fn build_skills(set: &ItemSet, class: &str, label: &str) -> Result<ItemSkills, DataError> {
    let mut skills = ItemSkills::default();

    if let Some(text) = set.string("item_skill") {
        let mut list = Vec::new();
        for element in java_split(text, ';') {
            if let Some(skill) = parse_item_skill(element, label) {
                list.push(skill);
            }
        }
        skills.item_skills = Some(list);
    }

    if let Some(text) = set.string("unequip_skill") {
        skills.unequip_skill = parse_pair(
            text,
            &format!("Could not parse {text} in weapon unequip skills! item {label}"),
        );
    }

    if class == "Weapon" || class == "Armor" {
        if let Some(text) = set.string("enchant4_skill") {
            let what = if class == "Weapon" { "weapon" } else { "armor" };
            skills.enchant4_skill = parse_pair(
                text,
                &format!("> Could not parse {text} in {what} enchant skills! item {label}"),
            );
        }
    }

    if class == "Weapon" {
        // `chance` is read (and must be an integer) as soon as the skill key exists.
        if let Some(text) = set.string("onmagic_skill") {
            let chance = set.int("onmagic_chance", 100)?;
            let skill = parse_pair(
                text,
                &format!("> Could not parse {text} in weapon onmagic skills! item {label}"),
            );
            skills.on_magic = skill
                .filter(|_| chance > 0)
                .map(|skill| ChanceSkill { skill, chance });
        }
        if let Some(text) = set.string("oncrit_skill") {
            let chance = set.int("oncrit_chance", 100)?;
            let skill = parse_pair(
                text,
                &format!("> Could not parse {text} in weapon oncrit skills! item {label}"),
            );
            skills.on_crit = skill
                .filter(|_| chance > 0)
                .map(|skill| ChanceSkill { skill, chance });
        }
    }
    Ok(skills)
}

/// One `id-level` element of `item_skill`.
fn parse_item_skill(element: &str, label: &str) -> Option<SkillRef> {
    let fail = || {
        warn!(
            "Failed to parse item_skill({element}) for item {label}! Format: SkillId0-SkillLevel0[;SkillIdN-SkillLevelN]"
        );
        None
    };
    let parts = java_split(element, '-');
    let Some(id) = parts.first().and_then(|part| parse_int(part)) else {
        return fail();
    };
    if id == 0 {
        info!("Ignoring item_skill({element}) for item {label}. Skill id is 0!");
        return None;
    }
    let Some(level) = parts.get(1).and_then(|part| parse_int(part)) else {
        return fail();
    };
    if level == 0 {
        info!("Ignoring item_skill({element}) for item {label}. Skill level is 0!");
        return None;
    }
    Some(SkillRef { id, level })
}

/// The shared reader of `unequip_skill`, `enchant4_skill`, `oncrit_skill` and `onmagic_skill`:
/// exactly two `-` separated parts, both positive. A part that is not a number logs `message`
/// and the skill is dropped.
fn parse_pair(text: &str, message: &str) -> Option<SkillRef> {
    let info_parts = java_split(text, '-');
    if info_parts.len() != 2 {
        return None;
    }
    let (id, level) = match (parse_int(info_parts[0]), parse_int(info_parts[1])) {
        (Some(id), Some(level)) => (id, level),
        // Java parses the id first and keeps it when the level fails; either way level stays 0.
        _ => {
            info!("{message}");
            return None;
        }
    };
    (id > 0 && level > 0).then_some(SkillRef { id, level })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::stats::node::parse_xml;

    fn set_with(entries: &[(&str, &str)]) -> ItemSet {
        ItemSet(
            entries
                .iter()
                .map(|&(k, v)| (k.to_owned(), v.to_owned()))
                .collect::<HashMap<_, _>>(),
        )
    }

    fn from_skills_xml(xml: &str) -> ItemSet {
        let mut set = set_with(&[]);
        apply_skills_node(&parse_xml(xml).unwrap(), &mut set).unwrap();
        set
    }

    #[test]
    fn skills_branch_writes_the_stat_set_keys() {
        let set = from_skills_xml(
            r#"<skills>
                <skill id="1" level="2"/>
                <skill id="3" level="4"/>
                <skill id="5" level="6" type="ON_ENCHANT_4"/>
                <skill id="7" level="8" type="ON_UNEQUIP"/>
                <skill id="9" level="10" type="ON_CRITICAL_SKILL" type_chance="25"/>
                <skill id="11" level="12" type="ON_MAGIC_SKILL"/>
                <skill id="13" level="14" type="NOPE"/>
            </skills>"#,
        );
        assert_eq!(set.string("item_skill"), Some("1-2;3-4;13-14"));
        assert_eq!(set.string("enchant4_skill"), Some("5-6"));
        assert_eq!(set.string("unequip_skill"), Some("7-8"));
        assert_eq!(set.string("oncrit_skill"), Some("9-10"));
        assert_eq!(set.string("oncrit_chance"), Some("25"));
        assert_eq!(set.string("onmagic_skill"), Some("11-12"));
        assert_eq!(set.string("onmagic_chance"), Some("100"));
    }

    #[test]
    fn skills_are_read_back_per_item_class() {
        let set = from_skills_xml(
            r#"<skills>
                <skill id="1" level="2"/>
                <skill id="5" level="6" type="ON_ENCHANT_4"/>
                <skill id="7" level="8" type="ON_UNEQUIP"/>
                <skill id="9" level="10" type="ON_CRITICAL_SKILL" type_chance="25"/>
                <skill id="11" level="12" type="ON_MAGIC_SKILL" type_chance="0"/>
            </skills>"#,
        );
        let weapon = build_skills(&set, "Weapon", "w(1)").unwrap();
        assert_eq!(weapon.item_skills, Some(vec![SkillRef { id: 1, level: 2 }]));
        assert_eq!(weapon.enchant4_skill, Some(SkillRef { id: 5, level: 6 }));
        assert_eq!(weapon.unequip_skill, Some(SkillRef { id: 7, level: 8 }));
        assert_eq!(
            weapon.on_crit,
            Some(ChanceSkill {
                skill: SkillRef { id: 9, level: 10 },
                chance: 25
            })
        );
        // Chance 0 drops the skill.
        assert_eq!(weapon.on_magic, None);

        let armor = build_skills(&set, "Armor", "a(2)").unwrap();
        assert_eq!(armor.enchant4_skill, Some(SkillRef { id: 5, level: 6 }));
        assert_eq!(armor.on_crit, None);
        assert_eq!(armor.unequip_skill, Some(SkillRef { id: 7, level: 8 }));

        let etc = build_skills(&set, "EtcItem", "e(3)").unwrap();
        assert_eq!(etc.item_skills, Some(vec![SkillRef { id: 1, level: 2 }]));
        assert_eq!(etc.enchant4_skill, None);
        assert_eq!(etc.on_crit, None);
    }

    #[test]
    fn no_skills_means_none() {
        let skills = build_skills(&set_with(&[]), "Weapon", "w(1)").unwrap();
        assert_eq!(skills, ItemSkills::default());
        assert!(skills.item_skills.is_none());
    }

    #[test]
    fn item_skill_entries_are_validated_one_by_one() {
        let set = set_with(&[("item_skill", "0-1;5-0;7-2;x-1;8;9-3;")]);
        let skills = build_skills(&set, "EtcItem", "e(1)").unwrap();
        assert_eq!(
            skills.item_skills,
            Some(vec![SkillRef { id: 7, level: 2 }, SkillRef { id: 9, level: 3 }])
        );
        // Everything dropped: the list exists but is empty (Java: empty array, not null).
        let set = set_with(&[("item_skill", "0-1")]);
        assert_eq!(
            build_skills(&set, "EtcItem", "e(1)").unwrap().item_skills,
            Some(vec![])
        );
    }

    #[test]
    fn pair_skills_need_two_positive_parts() {
        for text in ["1", "1-2-3", "x-2", "1-x", "0-5", "5-0", "-1-2", ""] {
            let set = set_with(&[("unequip_skill", text)]);
            assert_eq!(
                build_skills(&set, "Weapon", "w(1)").unwrap().unequip_skill,
                None,
                "{text:?}"
            );
        }
        let set = set_with(&[("unequip_skill", "10-3")]);
        assert_eq!(
            build_skills(&set, "Weapon", "w(1)").unwrap().unequip_skill,
            Some(SkillRef { id: 10, level: 3 })
        );
    }

    #[test]
    fn a_bad_chance_fails_the_item_but_only_with_a_skill() {
        let set = set_with(&[("onmagic_skill", "1-1"), ("onmagic_chance", "abc")]);
        assert!(build_skills(&set, "Weapon", "w(1)").is_err());
        // Without the skill key the chance is never read; for other classes neither.
        let set = set_with(&[("onmagic_chance", "abc")]);
        assert!(build_skills(&set, "Weapon", "w(1)").is_ok());
        let set = set_with(&[("onmagic_skill", "1-1"), ("onmagic_chance", "abc")]);
        assert!(build_skills(&set, "Armor", "a(1)").is_ok());
    }

    #[test]
    fn missing_or_bad_ids_fail() {
        let mut set = set_with(&[]);
        let bad = parse_xml(r#"<skills><skill level="1"/></skills>"#).unwrap();
        assert!(apply_skills_node(&bad, &mut set).is_err());
        let bad = parse_xml(r#"<skills><skill id="x" level="1"/></skills>"#).unwrap();
        assert!(apply_skills_node(&bad, &mut set).is_err());
        // Hex is fine (`Integer.decode`).
        let ok = from_skills_xml(r#"<skills><skill id="0x10" level="1"/></skills>"#);
        assert_eq!(ok.string("item_skill"), Some("16-1"));
    }
}
