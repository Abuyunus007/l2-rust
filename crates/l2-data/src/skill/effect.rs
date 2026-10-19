//! Effect templates: what a skill's `<effect name="..">` elements hold until the handlers exist.
//!
//! Java: `mechanics/effects/AbstractEffect.java` (name, ticks, attach condition, function
//! templates) and the effect parts of `util/DocumentBase.java` (`attachEffect`, `parseParameters`,
//! `hasLevelSpecificValues`, `parseLevelSpecificValues`, the `RestorationRandom` item lists).
//!
//! The handlers (`EffectHandler`, one class per effect name) are M5. Their constructors read the
//! raw `params` and the attributes; this template keeps both as Java built them, with the `#table`
//! references of the skill level already resolved.

use std::collections::BTreeMap;
use std::sync::Arc;

use tracing::warn;

use crate::stats::jnum::parse_int;
use crate::stats::{Condition, FuncTemplate, XmlNode};

use super::document::{DocumentError, jtrim};
use super::template::{EffectScope, parse_float};

/// One `<item>` of a `RestorationRandom` group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemEntry {
    /// `id` (greater than zero).
    pub id: i32,
    /// `count` (default 1).
    pub count: i64,
    /// `minEnchant` (default 0).
    pub min_enchant: i32,
    /// `maxEnchant` (default 0).
    pub max_enchant: i32,
}

/// One outer `<item chance="..">` of a `RestorationRandom` effect: the chance and what it gives.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemGroup {
    /// `chance` (default 100).
    pub chance: f32,
    /// The inner `<item>` elements.
    pub items: Vec<ItemEntry>,
}

/// A value of an effect's parameters.
#[derive(Debug, Clone, PartialEq)]
pub enum ParamValue {
    /// Text, the way `StatSet.set(String, String)` stored it (tables already resolved).
    Text(String),
    /// The `items` of a `RestorationRandom` effect.
    Items(Vec<ItemGroup>),
}

/// Java `AbstractEffect` as data: a handler name with its parameters, stat functions and
/// conditions.
#[derive(Debug, Clone, PartialEq)]
pub struct EffectTemplate {
    /// `name`: the handler class (`Buff`, `PhysicalAttack`, ...).
    pub name: String,
    /// The list of the skill this effect sits in.
    pub scope: EffectScope,
    /// `ticks` (default 0).
    pub ticks: i32,
    /// Java's attribute `StatSet` of the `<effect ..>` element (`name`, `ticks`, `abnormalTime`,
    /// ...), tables resolved.
    pub attrs: BTreeMap<String, String>,
    /// Java's parameter `StatSet`: `<param ..>` attributes and the named child elements
    /// (`<power>#table</power>`, `<value level="..">`, `items`).
    pub params: BTreeMap<String, ParamValue>,
    /// The `<add>`, `<mul>`, ... function templates inside the effect, in file order. These are
    /// what a passive skill applies to its owner.
    pub funcs: Vec<FuncTemplate>,
    /// Java `_attachCond`: the `<conditions>` block of the enclosing effects list, if it was
    /// written as the very first child (see `XmlNode::leading_text`). The failure message of the
    /// block is not kept: `AbstractEffect.testConditions` does not use it.
    pub attach_cond: Option<Arc<Condition>>,
    /// The apply condition Java parses from the effect's first non-`<value>` element child
    /// (`parseCondition(n.getFirstChild())`), passed to the handler constructor.
    pub apply_cond: Option<Arc<Condition>>,
}

impl EffectTemplate {
    /// The text of a parameter, if it is one.
    pub fn param(&self, key: &str) -> Option<&str> {
        match self.params.get(key) {
            Some(ParamValue::Text(text)) => Some(text),
            _ => None,
        }
    }
}

/// The effects of one skill level per [`EffectScope`] (Java `_effectLists`, an `EnumMap`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EffectLists {
    lists: [Vec<EffectTemplate>; 8],
}

impl EffectLists {
    /// Java `getEffects(scope)`: empty when the skill has none.
    pub fn get(&self, scope: EffectScope) -> &[EffectTemplate] {
        self.lists.get(scope as usize).map_or(&[], Vec::as_slice)
    }

    /// Java `addEffect(scope, effect)`.
    pub fn push(&mut self, scope: EffectScope, effect: EffectTemplate) {
        if let Some(list) = self.lists.get_mut(scope as usize) {
            list.push(effect);
        }
    }

    /// True when no scope has an effect.
    pub fn is_empty(&self) -> bool {
        self.lists.iter().all(Vec::is_empty)
    }

    /// Every effect with its scope, scopes in Java's declaration order.
    pub fn iter(&self) -> impl Iterator<Item = (EffectScope, &EffectTemplate)> {
        EffectScope::ALL
            .iter()
            .copied()
            .flat_map(|scope| self.get(scope).iter().map(move |effect| (scope, effect)))
    }
}

/// `DocumentBase.IGNORED_ELEMENTS`: children of an effect that are not parameters.
const IGNORED_ELEMENTS: [&str; 11] = [
    "param",
    "conditions",
    "effect",
    "add",
    "sub",
    "mul",
    "div",
    "set",
    "share",
    "enchant",
    "enchanthp",
];

/// What the parameter parsers need from the skill being built.
pub(super) struct ParamContext<'a> {
    /// Java `skill.getLevel()` of the template the effect belongs to.
    pub skill_level: i32,
    /// Java `getValue(value, skill)`: `#table` to the entry of the current level.
    pub value: &'a dyn Fn(&str) -> String,
}

/// Java `parseParameters(n.getFirstChild(), template)` for a `Skill` template. `children` are the
/// element children of the `<effect>`. Java returns an empty set when nothing matched; so does
/// this.
///
/// NOTE(java-quirk): every child element that is not in [`IGNORED_ELEMENTS`] becomes a parameter,
/// the condition elements too: a leaf `<player level="40"/>` directly under `<effect>` is stored as
/// the parameter `player` with an empty value, `<and>` (which has children) is skipped.
pub(super) fn parse_parameters(
    children: &[XmlNode],
    cx: &ParamContext<'_>,
) -> Result<BTreeMap<String, ParamValue>, DocumentError> {
    let mut parameters = BTreeMap::new();

    // First pass: `<param>` tags (traditional format).
    for n in children {
        if n.name == "param" {
            for (key, value) in &n.attrs {
                parameters.insert(key.clone(), ParamValue::Text((cx.value)(value)));
            }
        }
    }

    // Second pass: direct named elements as parameters.
    for n in children {
        if IGNORED_ELEMENTS.contains(&n.name.to_lowercase().as_str()) {
            continue;
        }
        if has_level_specific_values(n) {
            let by_level = parse_level_specific_values(n, cx)?;
            if let Some(value) = by_level.get(&cx.skill_level) {
                parameters.insert(n.name.clone(), ParamValue::Text(value.clone()));
            }
        } else if n.children.is_empty() {
            // `hasOnlyTextContent`: a single value for all levels.
            let value = jtrim(&n.text_content()).to_owned();
            parameters.insert(n.name.clone(), ParamValue::Text((cx.value)(&value)));
        }
        // else: other child elements, not a simple parameter.
    }
    Ok(parameters)
}

/// Java `hasLevelSpecificValues`: a `<value level="..">` child.
fn has_level_specific_values(node: &XmlNode) -> bool {
    node.children
        .iter()
        .any(|child| child.name == "value" && child.attr("level").is_some())
}

/// Java `parseLevelSpecificValues`: level to resolved text.
fn parse_level_specific_values(
    node: &XmlNode,
    cx: &ParamContext<'_>,
) -> Result<BTreeMap<i32, String>, DocumentError> {
    let mut by_level = BTreeMap::new();
    for child in &node.children {
        if child.name != "value" {
            continue;
        }
        let Some(level) = child.attr("level") else {
            continue;
        };
        let level = parse_int(level).ok_or_else(|| DocumentError::number(level))?;
        let value = jtrim(&child.text_content()).to_owned();
        by_level.insert(level, (cx.value)(&value));
    }
    Ok(by_level)
}

/// Java's `RestorationRandom` branch of `attachEffect`: the `<items>` child becomes the `items`
/// parameter. The level format (`<items><value level="..">`) keeps the group of the skill's own
/// level; the simple format applies to every level. `Ok(None)` when there is no `<items>`.
pub(super) fn parse_restoration_items(
    effect: &XmlNode,
    skill_level: i32,
) -> Result<Option<Vec<ItemGroup>>, DocumentError> {
    // Both lookups are case sensitive in Java.
    let Some(items_node) = effect.children.iter().find(|child| child.name == "items") else {
        return Ok(None);
    };
    let level_format = items_node
        .children
        .first()
        .is_some_and(|first| first.name == "value");
    if !level_format {
        return Ok(Some(parse_item_groups(items_node)?));
    }
    let mut items = None;
    for value in items_node.children.iter().filter(|child| child.name == "value") {
        let level = match value.attr("level") {
            Some(text) => parse_int(text).ok_or_else(|| DocumentError::number(text))?,
            None => 1,
        };
        if level != skill_level {
            continue;
        }
        // The last matching `<value>` wins, like the repeated `parameters.set("items", ..)`.
        items = Some(parse_item_groups(value)?);
    }
    Ok(items)
}

fn parse_item_groups(parent: &XmlNode) -> Result<Vec<ItemGroup>, DocumentError> {
    parent
        .children
        .iter()
        .filter(|child| child.name == "item")
        .map(parse_item_node)
        .collect()
}

/// Java `parseItemNode`.
fn parse_item_node(node: &XmlNode) -> Result<ItemGroup, DocumentError> {
    let chance = match node.attr("chance") {
        Some(text) => parse_float(text).ok_or_else(|| DocumentError::number(text))?,
        None => 100.0,
    };
    let mut items = Vec::new();
    for inner in node.children.iter().filter(|child| child.name == "item") {
        let Some(id) = inner.attr("id") else {
            warn!("Missing item ID in RestorationRandom effect");
            continue;
        };
        let id = parse_int(id).ok_or_else(|| DocumentError::number(id))?;
        if id <= 0 {
            warn!("Invalid item ID found in RestorationRandom effect: {id}");
            continue;
        }
        let count = match inner.attr("count") {
            Some(text) => text.parse::<i64>().map_err(|_| DocumentError::number(text))?,
            None => 1,
        };
        let enchant = |key: &str| -> Result<i32, DocumentError> {
            match inner.attr(key) {
                Some(text) => parse_int(text).ok_or_else(|| DocumentError::number(text)),
                None => Ok(0),
            }
        };
        items.push(ItemEntry {
            id,
            count,
            min_enchant: enchant("minEnchant")?,
            max_enchant: enchant("maxEnchant")?,
        });
    }
    Ok(ItemGroup { chance, items })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::node::parse_dom;

    fn params(xml: &str, level: i32) -> BTreeMap<String, ParamValue> {
        let node = parse_dom(xml).unwrap();
        let resolve = |v: &str| v.replace("#t", "TABLE");
        parse_parameters(
            &node.children,
            &ParamContext {
                skill_level: level,
                value: &resolve,
            },
        )
        .unwrap()
    }

    fn text(map: &BTreeMap<String, ParamValue>, key: &str) -> Option<String> {
        match map.get(key) {
            Some(ParamValue::Text(t)) => Some(t.clone()),
            _ => None,
        }
    }

    #[test]
    fn param_attributes_and_named_elements() {
        let p = params(
            r##"<effect name="X"><param power="#t" chance="30"/><heal> #t </heal><add stat="pAtk" val="1"/></effect>"##,
            1,
        );
        assert_eq!(text(&p, "power").as_deref(), Some("TABLE"));
        assert_eq!(text(&p, "chance").as_deref(), Some("30"));
        assert_eq!(text(&p, "heal").as_deref(), Some("TABLE"));
        // `add` is an ignored element; `param` itself is not a parameter name.
        assert!(!p.contains_key("add") && !p.contains_key("param"));
    }

    #[test]
    fn level_specific_values_take_the_skill_level() {
        let xml = r#"<effect name="X"><rate><value level="1">10</value><value level="3">30</value></rate></effect>"#;
        assert_eq!(text(&params(xml, 3), "rate").as_deref(), Some("30"));
        assert_eq!(text(&params(xml, 1), "rate").as_deref(), Some("10"));
        // A level without an entry sets nothing.
        assert!(params(xml, 2).is_empty());
    }

    #[test]
    fn condition_elements_under_effect_are_parameters_too() {
        let p = params(
            r#"<effect name="X"><using kind="BOW"/><and><player level="1"/></and></effect>"#,
            1,
        );
        // NOTE(java-quirk): a leaf condition element has only text content (none), so it is
        // stored with an empty value; `<and>` has element children and is skipped.
        assert_eq!(text(&p, "using").as_deref(), Some(""));
        assert!(!p.contains_key("and"));
    }

    #[test]
    fn restoration_random_simple_and_level_formats() {
        let simple = parse_dom(
            r#"<effect name="RestorationRandom"><items>
                <item chance="40"><item id="57" count="10"/><item id="0"/><item id="5" minEnchant="1" maxEnchant="3"/></item>
                <item><item id="9"/></item>
            </items></effect>"#,
        )
        .unwrap();
        let groups = parse_restoration_items(&simple, 7).unwrap().unwrap();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].chance, 40.0);
        assert_eq!(
            groups[0].items,
            [
                ItemEntry {
                    id: 57,
                    count: 10,
                    min_enchant: 0,
                    max_enchant: 0
                },
                ItemEntry {
                    id: 5,
                    count: 1,
                    min_enchant: 1,
                    max_enchant: 3
                },
            ]
        );
        assert_eq!(groups[1].chance, 100.0);

        let by_level = parse_dom(
            r#"<effect name="RestorationRandom"><items>
                <value level="1"><item><item id="1"/></item></value>
                <value level="2"><item><item id="2"/></item></value>
            </items></effect>"#,
        )
        .unwrap();
        assert_eq!(
            parse_restoration_items(&by_level, 2).unwrap().unwrap()[0].items[0].id,
            2
        );
        assert!(parse_restoration_items(&by_level, 5).unwrap().is_none());
        assert!(parse_restoration_items(&simple, 1).unwrap().is_some());
        assert!(
            parse_restoration_items(&parse_dom("<effect/>").unwrap(), 1)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn effect_lists_by_scope() {
        let mut lists = EffectLists::default();
        assert!(lists.is_empty() && lists.get(EffectScope::General).is_empty());
        let effect = |name: &str, scope| EffectTemplate {
            name: name.into(),
            scope,
            ticks: 0,
            attrs: BTreeMap::new(),
            params: BTreeMap::new(),
            funcs: Vec::new(),
            attach_cond: None,
            apply_cond: None,
        };
        lists.push(EffectScope::Pvp, effect("B", EffectScope::Pvp));
        lists.push(EffectScope::General, effect("A", EffectScope::General));
        lists.push(EffectScope::General, effect("C", EffectScope::General));
        assert_eq!(lists.get(EffectScope::General).len(), 2);
        let order: Vec<_> = lists.iter().map(|(_, e)| e.name.as_str()).collect();
        assert_eq!(order, ["A", "C", "B"]);
    }
}
