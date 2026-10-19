//! The skill document parser: one `<skill>` element becomes one [`SkillTemplate`] per level and
//! per enchant level.
//!
//! Java: `util/DocumentSkill.java` (`parseSkill`, `parseTable`'s use, `makeSkills`, the enchant
//! route loops, `parseInlineLevelTable`) and `util/DocumentBase.java` (`parseTemplate`,
//! `attachEffect`, `parseBeanSet`, `parseElementValue`, `getValue`, `getTableValue`).
//!
//! # How a skill is built
//! 1. `<table name="#x">1 2 3</table>` entries are stored (whitespace separated, `#` required).
//! 2. One [`StatSet`] per level 1..=`levels` is filled from the other children: `<set name val>`
//!    and `<set name>text</set>`, direct value elements (`<mpConsume>#mpConsume</mpConsume>`) and
//!    inline tables (`<hitTime levelValues="1 2 3"/>`). A `#x` value takes entry `level - 1`.
//! 3. For every enchant route `enchantGroupN="g"` (group `g` of `EnchantSkillGroups.xml` says how
//!    many levels), one set per enchant level, numbered `N00 + k`. These take the plain values of
//!    the **last** plain level, then the `<enchantN name="..">` overrides (entry `k - 1`).
//! 4. `new Skill(set)` for every set, then the effect lists and `<conditions>` are parsed per
//!    level. An enchant level uses its own `enchantNeffects`, `enchantNconditions`, ... and falls
//!    back to the plain list for every kind it does not define.
//!
//! # Errors
//! Java parses a file in one `try`: any exception ends the file, the skills of the file that were
//! finished before it stay, and the rest are lost (logged as `Error loading file`). Same here.

use std::cell::Cell;
use std::collections::HashMap;
use std::sync::Arc;

use tracing::error;

use crate::stats::func::parse_func_element;
use crate::stats::jnum::{number_format_message, parse_int};
use crate::stats::node::parse_dom;
use crate::stats::{Condition, ConditionParser, ParseError, XmlNode};

use super::effect::{EffectTemplate, ParamContext, ParamValue, parse_parameters, parse_restoration_items};
use super::template::{EffectScope, SetError, SetValue, SkillTemplate, StatSet};
use super::{EnchantSkillGroups, SkillLoadOptions};

/// Java `String.trim()`: removes every character up to U+0020, not Unicode whitespace.
pub(super) fn jtrim(text: &str) -> &str {
    text.trim_matches(|c: char| c <= ' ')
}

/// Anything the Java parser throws for; the rest of the file is dropped.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub(super) struct DocumentError(String);

impl DocumentError {
    pub(super) fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }

    /// `NumberFormatException` for `text`.
    pub(super) fn number(text: &str) -> Self {
        Self(number_format_message(text))
    }
}

impl From<ParseError> for DocumentError {
    fn from(error: ParseError) -> Self {
        Self(error.to_string())
    }
}

impl From<SetError> for DocumentError {
    fn from(error: SetError) -> Self {
        Self(error.to_string())
    }
}

type Result<T> = std::result::Result<T, DocumentError>;

/// What `parseTemplate` attaches to: Java's `template` parameter (a `Skill` or an `AbstractEffect`).
enum Template<'t> {
    Skill(&'t mut SkillTemplate),
    Effect(&'t mut EffectTemplate),
}

/// The children of a `<skill>` that `DocumentSkill` reads by name for every level: the use
/// conditions and the effect lists. `enchantN` + the same suffix is the enchant version.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Conditions,
    Effects,
    Start,
    Channeling,
    Pve,
    Pvp,
    End,
    SelfEffects,
}

const KINDS: [(Kind, &str); 8] = [
    (Kind::Conditions, "conditions"),
    (Kind::Effects, "effects"),
    (Kind::Start, "startEffects"),
    (Kind::Channeling, "channelingEffects"),
    (Kind::Pve, "pveEffects"),
    (Kind::Pvp, "pvpEffects"),
    (Kind::End, "endEffects"),
    (Kind::SelfEffects, "selfEffects"),
];

/// Matches `<prefix><suffix>` ignoring case (Java `equalsIgnoreCase`), prefix `""` or `enchantN`.
fn classify(name: &str, prefix: &str) -> Option<Kind> {
    let rest = name
        .get(..prefix.len())
        .filter(|head| head.eq_ignore_ascii_case(prefix))?;
    let rest = name.get(rest.len()..)?;
    KINDS
        .iter()
        .find(|(_, suffix)| rest.eq_ignore_ascii_case(suffix))
        .map(|&(kind, _)| kind)
}

/// The list a kind of effects element feeds; `None` is "the plain list" (passive or general).
fn scope_of(kind: Kind) -> Option<EffectScope> {
    match kind {
        Kind::Conditions | Kind::Effects => None,
        Kind::Start => Some(EffectScope::Start),
        Kind::Channeling => Some(EffectScope::Channeling),
        Kind::Pve => Some(EffectScope::Pve),
        Kind::Pvp => Some(EffectScope::Pvp),
        Kind::End => Some(EffectScope::End),
        Kind::SelfEffects => Some(EffectScope::SelfScope),
    }
}

/// Java `DocumentSkill.parseDocument` for one file's text: every skill that was finished before an
/// error. Problems are logged like Java does.
pub(super) fn parse_document(
    file: &str,
    xml: &str,
    groups: &EnchantSkillGroups,
    options: &SkillLoadOptions,
) -> Vec<SkillTemplate> {
    let mut skills = Vec::new();
    let root = match parse_dom(xml) {
        Ok(root) => root,
        Err(e) => {
            error!("Error loading file {file}: {e}");
            return skills;
        }
    };
    if let Err(e) = parse_root(file, &root, groups, options, &mut skills) {
        error!("Error loading file {file}: {e}");
    }
    skills
}

fn parse_root(
    file: &str,
    root: &XmlNode,
    groups: &EnchantSkillGroups,
    options: &SkillLoadOptions,
    skills: &mut Vec<SkillTemplate>,
) -> Result<()> {
    if root.name.eq_ignore_ascii_case("list") {
        for d in root
            .children
            .iter()
            .filter(|d| d.name.eq_ignore_ascii_case("skill"))
        {
            skills.extend(SkillDoc::new(file, options).parse_skill(d, groups)?);
        }
    } else if root.name.eq_ignore_ascii_case("skill") {
        skills.extend(SkillDoc::new(file, options).parse_skill(root, groups)?);
    }
    Ok(())
}

/// Java `DocumentSkill` plus `DocumentSkillDataHolder`, for one skill.
struct SkillDoc<'a> {
    file: &'a str,
    options: &'a SkillLoadOptions,
    id: i32,
    /// Java `_tables`.
    tables: HashMap<String, Vec<String>>,
    /// Java `_currentSkill.currentLevel`: the 0-based table index `getTableValue(name)` uses.
    current_level: Cell<usize>,
}

impl<'a> SkillDoc<'a> {
    fn new(file: &'a str, options: &'a SkillLoadOptions) -> Self {
        Self {
            file,
            options,
            id: 0,
            tables: HashMap::new(),
            current_level: Cell::new(0),
        }
    }

    // ---- tables and `getValue` ----

    /// Java `parseTable`.
    fn parse_table(&mut self, n: &XmlNode) -> Result<()> {
        let name = n
            .attr("name")
            .ok_or_else(|| DocumentError::new("NullPointerException: table without name"))?;
        if !name.starts_with('#') {
            return Err(DocumentError::new("Table name must start with #"));
        }
        // `n.getFirstChild().getNodeValue()`: the first child must be the text node.
        if !n.leading_text {
            return Err(DocumentError::new(format!(
                "NullPointerException: table {name} has no text"
            )));
        }
        // `StringTokenizer` default delimiters.
        let values = n
            .text
            .split([' ', '\t', '\n', '\r', '\u{c}'])
            .filter(|token| !token.is_empty())
            .map(str::to_owned)
            .collect();
        self.tables.insert(name.to_owned(), values);
        Ok(())
    }

    /// Java `getTableValue(name)`: the entry of the current level, `""` (logged) when missing.
    fn table_current(&self, name: &str) -> String {
        match self
            .tables
            .get(name)
            .and_then(|table| table.get(self.current_level.get()))
        {
            Some(value) => value.clone(),
            None => {
                error!("Error in table: {name} of Skill Id {}", self.id);
                String::new()
            }
        }
    }

    /// Java `getTableValue(name, idx)`: the entry `idx - 1`, `""` (logged) when missing.
    fn table_at(&self, name: &str, idx: usize) -> String {
        match idx
            .checked_sub(1)
            .and_then(|i| self.tables.get(name).and_then(|table| table.get(i)))
        {
            Some(value) => value.clone(),
            None => {
                error!(
                    "Wrong level count in skill Id {} name: {name} index : {idx}",
                    self.id
                );
                String::new()
            }
        }
    }

    /// Java `getValue(value, skill)`.
    fn value_for_skill(&self, value: &str) -> String {
        if value.starts_with('#') {
            self.table_current(value)
        } else {
            value.to_owned()
        }
    }

    /// Java `getValue(value, level)` with an `Integer` template.
    fn value_for_level(&self, value: &str, level: usize) -> String {
        if value.starts_with('#') {
            self.table_at(value, level)
        } else {
            value.to_owned()
        }
    }

    // ---- values into the StatSet ----

    /// Java `parseBeanSet`.
    fn parse_bean_set(&self, n: &XmlNode, set: &mut StatSet, level: usize) {
        let Some(name) = n.attr("name") else {
            return;
        };
        let name = jtrim(name);
        let value = match n.attr("val") {
            Some(val) => jtrim(val).to_owned(),
            None => jtrim(&n.text_content()).to_owned(),
        };
        self.set_value(set, name, &value, level);
    }

    /// Java `parseElementValue`: `<reuseDelay>3000</reuseDelay>`, or `<enchant1 name="power">`.
    fn parse_element_value(&self, n: &XmlNode, set: &mut StatSet, level: usize) {
        let value = jtrim(&n.text_content()).to_owned();
        let name = match n.attr("name") {
            Some(name) => jtrim(name),
            None => jtrim(&n.name),
        };
        self.set_value(set, name, &value, level);
    }

    /// The common tail of both: empty is ignored, `#`, `-` and digits go through the table.
    fn set_value(&self, set: &mut StatSet, name: &str, value: &str, level: usize) {
        let Some(first) = value.chars().next() else {
            return;
        };
        // NOTE(java-quirk): `Character.isDigit` also accepts non ASCII digits; the datapack has none.
        if first == '#' || first == '-' || first.is_ascii_digit() {
            set.set_str(name, &self.value_for_level(value, level));
        } else {
            set.set_str(name, value);
        }
    }

    /// Java `parseInlineLevelTable`: `levelValues` (route 0) or `subLevelNValues` (route N), the
    /// value of entry `level - 1` typed by `StringUtil.parseValue`.
    fn parse_inline_level_table(n: &XmlNode, set: &mut StatSet, level: usize, route: usize) {
        let attribute = if route == 0 {
            "levelValues".to_owned()
        } else {
            format!("subLevel{route}Values")
        };
        let Some(table) = n.attr(&attribute) else {
            return;
        };
        let table = jtrim(table);
        // `String.split("\\s+")`: an empty string gives one empty entry.
        let values: Vec<&str> = if table.is_empty() {
            vec![""]
        } else {
            table
                .split([' ', '\t', '\n', '\u{b}', '\u{c}', '\r'])
                .filter(|v| !v.is_empty())
                .collect()
        };
        if level > 0
            && let Some(value) = values.get(level - 1)
        {
            set.set(jtrim(&n.name), SetValue::parse(value));
        }
    }

    // ---- skill ----

    /// Java `parseSkill`: every level and enchant level of the element.
    fn parse_skill(mut self, node: &XmlNode, groups: &EnchantSkillGroups) -> Result<Vec<SkillTemplate>> {
        let required = |name: &str| {
            node.attr(name)
                .ok_or_else(|| DocumentError::new(format!("NullPointerException: skill without {name}")))
        };
        let id_text = required("id")?;
        let id = parse_int(id_text).ok_or_else(|| DocumentError::number(id_text))?;
        let name = required("name")?.to_owned();
        let levels_text = required("levels")?;
        let last_lvl = parse_int(levels_text).ok_or_else(|| DocumentError::number(levels_text))?;
        self.id = id;

        let mut route_counts = [0usize; 8];
        for (index, count) in route_counts.iter_mut().enumerate() {
            let route = index + 1;
            if let Some(group_text) = node.attr(&format!("enchantGroup{route}")) {
                let group = parse_int(group_text).ok_or_else(|| DocumentError::number(group_text))?;
                *count = groups.add_new_route_for_skill(id, route, group);
            }
        }
        let last = usize::try_from(last_lvl).map_err(|_| DocumentError::new("NegativeArraySizeException"))?;

        let new_set = |level: i32| {
            let mut set = StatSet::default();
            set.set_int("skill_id", id);
            set.set_int("level", level);
            set.set_str("name", &name);
            set
        };
        let mut sets: Vec<StatSet> = (1..=last_lvl).map(new_set).collect();

        for n in &node.children {
            if n.name.eq_ignore_ascii_case("table") {
                self.parse_table(n)?;
            }
        }

        for (index, set) in sets.iter_mut().enumerate() {
            let level = index + 1;
            for n in &node.children {
                let lower = n.name.to_ascii_lowercase();
                if lower == "table"
                    || lower.starts_with("enchant")
                    || lower.ends_with("effects")
                    || lower == "conditions"
                {
                    continue;
                }
                if lower == "set" {
                    self.parse_bean_set(n, set, level);
                } else if n.attr("levelValues").is_some() {
                    Self::parse_inline_level_table(n, set, level, 0);
                } else {
                    self.parse_element_value(n, set, level);
                }
            }
        }

        // Enchant routes 1..=8: levels `route * 100 + k`.
        let mut route_sets: Vec<Vec<StatSet>> = Vec::with_capacity(8);
        for (index, &count) in route_counts.iter().enumerate() {
            let route = index + 1;
            let route_name = format!("enchant{route}");
            let sub_level_attr = format!("subLevel{route}Values");
            let mut route_levels = Vec::with_capacity(count);
            for k in 0..count {
                let mut set = new_set((route * 100 + k + 1) as i32);
                for n in &node.children {
                    let lower = n.name.to_ascii_lowercase();
                    if lower == "table"
                        || lower == "conditions"
                        || (lower.starts_with("enchant") && lower != route_name)
                    {
                        continue;
                    }
                    if lower == "set" {
                        self.parse_bean_set(n, &mut set, last);
                    } else if lower == route_name {
                        self.parse_bean_set(n, &mut set, k + 1);
                    } else if !lower.ends_with("effects") {
                        if n.attr(&sub_level_attr).is_some() {
                            Self::parse_inline_level_table(n, &mut set, k + 1, route);
                        } else {
                            self.parse_element_value(n, &mut set, last);
                        }
                    }
                }
                route_levels.push(set);
            }
            route_sets.push(route_levels);
        }

        // `makeSkills`. NOTE(java-quirk): when one constructor throws, Java logs it and the list
        // shifts, so the next `currentSkills.get(i)` fails and ends the file. Same result here.
        let mut skills = Vec::with_capacity(last + route_counts.iter().sum::<usize>());
        for set in sets.iter().chain(route_sets.iter().flatten()) {
            match SkillTemplate::from_set(set, self.options) {
                Ok(skill) => skills.push(skill),
                Err(e) => {
                    error!(
                        "Skill id={}level{}: {e}",
                        set.int_or("skill_id", 0).unwrap_or(0),
                        set.int_or("level", 0).unwrap_or(0)
                    );
                    return Err(DocumentError::from(e));
                }
            }
        }

        // Effects and conditions of the plain levels.
        for index in 0..last {
            self.current_level.set(index);
            let Some(skill) = skills.get_mut(index) else {
                break;
            };
            for n in &node.children {
                if let Some(kind) = classify(&n.name, "") {
                    self.apply_kind(kind, n, skill)?;
                }
            }
        }

        // Effects and conditions of the enchant levels.
        let mut offset = last;
        for (index, &count) in route_counts.iter().enumerate() {
            let route = index + 1;
            let prefix = format!("enchant{route}");
            for k in 0..count {
                let Some(skill) = skills.get_mut(offset + k) else {
                    break;
                };
                self.current_level.set(k);
                let mut found = [false; 8];
                for n in &node.children {
                    if let Some(kind) = classify(&n.name, &prefix) {
                        found[kind as usize] = true;
                        self.apply_kind(kind, n, skill)?;
                    }
                }

                // "If none found, the enchanted skill will take effects from maxLvL of norm skill."
                if found.iter().any(|f| !f) {
                    self.current_level.set(last.saturating_sub(1));
                    for n in &node.children {
                        if let Some(kind) = classify(&n.name, "")
                            && fallback_applies(route, kind, &found)
                        {
                            self.apply_kind(kind, n, skill)?;
                        }
                    }
                }
            }
            offset += count;
        }
        Ok(skills)
    }

    /// One `conditions` / `*Effects` element of a skill level.
    fn apply_kind(&self, kind: Kind, n: &XmlNode, skill: &mut SkillTemplate) -> Result<()> {
        if kind == Kind::Conditions {
            return self.attach_conditions(n, skill);
        }
        self.parse_template(n, &mut Template::Skill(skill), scope_of(kind))
    }

    /// Java `Skill.attach(condition, false)` for a `<conditions>` element.
    fn attach_conditions(&self, n: &XmlNode, skill: &mut SkillTemplate) -> Result<()> {
        let resolve = |name: &str| Some(self.table_current(name));
        let parser = ConditionParser::new(self.file, &resolve);
        // NOTE(java-quirk): Java attaches a `null` when the block holds no recognised condition
        // (and the cast check would throw); such a block is dropped here.
        if let Some(condition) = parser.parse_conditions_block(n)? {
            skill.pre_conditions.push(condition);
        }
        Ok(())
    }

    // ---- effects ----

    /// Java `parseTemplate(node, template, effectScope)`.
    fn parse_template(
        &self,
        node: &XmlNode,
        template: &mut Template<'_>,
        scope: Option<EffectScope>,
    ) -> Result<()> {
        let resolve = |name: &str| Some(self.table_current(name));
        let parser = ConditionParser::new(self.file, &resolve);

        // NOTE(java-quirk): `node.getFirstChild()` is tested without skipping text nodes. With the
        // usual indentation the first child is whitespace, so a `<conditions>` block is not the
        // template condition, and the loop below has no case for it: it is ignored.
        let mut attach_cond: Option<Arc<Condition>> = None;
        let mut start = 0;
        if !node.leading_text
            && let Some(first) = node.children.first()
            && first.name.eq_ignore_ascii_case("conditions")
        {
            attach_cond = parser
                .parse_conditions_block(first)?
                .map(|messaged| Arc::new(messaged.condition));
            start = 1;
        }

        for child in node.children.iter().skip(start) {
            match child.name.to_lowercase().as_str() {
                "effect" => match template {
                    Template::Effect(_) => return Err(DocumentError::new("Nested effects")),
                    Template::Skill(skill) => {
                        self.attach_effect(child, skill, attach_cond.as_ref(), scope)?
                    }
                },
                "add" | "sub" | "mul" | "div" | "set" | "share" | "enchant" | "enchanthp" => {
                    // Parsed (and validated) for every template; only an effect keeps them.
                    let funcs = parse_func_element(&parser, child, attach_cond.as_ref())?;
                    if let Template::Effect(effect) = template {
                        effect.funcs.extend(funcs);
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Java `attachEffect`: one `<effect name="..">` of a skill level.
    fn attach_effect(
        &self,
        n: &XmlNode,
        skill: &mut SkillTemplate,
        attach_cond: Option<&Arc<Condition>>,
        scope: Option<EffectScope>,
    ) -> Result<()> {
        let attrs: std::collections::BTreeMap<String, String> = n
            .attrs
            .iter()
            .map(|(key, value)| (key.clone(), self.value_for_skill(value)))
            .collect();

        let value = |text: &str| self.value_for_skill(text);
        let mut params = parse_parameters(
            &n.children,
            &ParamContext {
                skill_level: skill.level,
                value: &value,
            },
        )?;

        let resolve = |name: &str| Some(self.table_current(name));
        let parser = ConditionParser::new(self.file, &resolve);
        let apply_cond = parser.parse_condition(&n.children, 0)?.map(Arc::new);

        let name = attrs
            .get("name")
            .ok_or_else(|| DocumentError::new("String value required, but not specified"))?
            .clone();
        if name == "RestorationRandom"
            && let Some(items) = parse_restoration_items(n, skill.level)?
        {
            params.insert("items".to_owned(), ParamValue::Items(items));
        }

        let ticks = match attrs.get("ticks") {
            Some(text) => match parse_int(text) {
                Some(ticks) => ticks,
                None => {
                    // Java's handler constructor throws here and `createEffect` returns null.
                    tracing::warn!(
                        "AbstractEffect: Unable to initialize effect handler: {name}: Integer value required, but found: {text}"
                    );
                    return Ok(());
                }
            },
            None => 0,
        };

        let mut effect = EffectTemplate {
            name,
            scope: scope.unwrap_or(if skill.is_passive() {
                EffectScope::Passive
            } else {
                EffectScope::General
            }),
            ticks,
            attrs,
            params,
            funcs: Vec::new(),
            attach_cond: attach_cond.cloned(),
            apply_cond,
        };
        self.parse_template(n, &mut Template::Effect(&mut effect), None)?;
        let target_scope = effect.scope;
        skill.effects.push(target_scope, effect);
        Ok(())
    }
}

/// Which branch of the "take it from the plain list" loop of an enchant route runs for `kind`.
///
/// NOTE(java-quirk): copy-paste differences in `DocumentSkill`: route 2 has no `startEffects`
/// branch at all, and route 7 guards its `startEffects` branch with `!foundChannelingEffects`
/// instead of `!foundStartEffects`.
fn fallback_applies(route: usize, kind: Kind, found: &[bool; 8]) -> bool {
    match kind {
        Kind::Start => match route {
            2 => false,
            7 => !found[Kind::Channeling as usize],
            _ => !found[Kind::Start as usize],
        },
        other => !found[other as usize],
    }
}
