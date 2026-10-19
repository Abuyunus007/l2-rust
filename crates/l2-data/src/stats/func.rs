//! Stat function templates: "set pAtk to 8", "add maxMp 50", "enchant pDef", with their order and
//! optional conditions. Data only, no evaluation (M4-STAT-01 turns them into calculator entries).
//!
//! Java: `mechanics/stats/functions/{FuncTemplate,StatFunction}.java` and the function parts of
//! `util/DocumentBase.java` (`parseTemplate`'s `add/sub/mul/div/set/share/enchant/enchanthp` case,
//! `attachFunc`, `processAlternativeFuncFormat`).
//!
//! Items build their templates in `item.rs` (`<stats>`); skills (M4-DATA-11) feed their effect
//! elements through [`parse_func_element`].

use std::sync::Arc;

use tracing::warn;

use super::condition::{Condition, ConditionParser, ParseError};
use super::jnum::{number_format_message, parse_double, parse_int};
use super::node::XmlNode;
use super::stat::Stat;

/// Java `StatFunction`: which `Func*` class a template creates, with its default order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FuncKind {
    /// `FuncSet` (order 0): replaces the value.
    Set,
    /// `FuncAdd` (30).
    Add,
    /// `FuncSub` (30).
    Sub,
    /// `FuncMul` (20).
    Mul,
    /// `FuncDiv` (20).
    Div,
    /// `FuncShare` (30).
    Share,
    /// `FuncEnchant` (0): the enchant level bonus of an item.
    Enchant,
    /// `FuncEnchantHp` (40).
    EnchantHp,
}

impl FuncKind {
    /// All kinds, in the order the card lists them.
    pub const ALL: [FuncKind; 8] = [
        FuncKind::Set,
        FuncKind::Add,
        FuncKind::Sub,
        FuncKind::Mul,
        FuncKind::Div,
        FuncKind::Share,
        FuncKind::Enchant,
        FuncKind::EnchantHp,
    ];

    /// Java `StatFunction.getOrder()`.
    pub fn default_order(self) -> i32 {
        match self {
            FuncKind::Set | FuncKind::Enchant => 0,
            FuncKind::Mul | FuncKind::Div => 20,
            FuncKind::Add | FuncKind::Sub | FuncKind::Share => 30,
            FuncKind::EnchantHp => 40,
        }
    }

    /// Java `StatFunction.getName()`: the suffix of the `Func*` class name.
    pub fn name(self) -> &'static str {
        match self {
            FuncKind::Set => "Set",
            FuncKind::Add => "Add",
            FuncKind::Sub => "Sub",
            FuncKind::Mul => "Mul",
            FuncKind::Div => "Div",
            FuncKind::Share => "Share",
            FuncKind::Enchant => "Enchant",
            FuncKind::EnchantHp => "EnchantHp",
        }
    }

    /// Java `StatFunction.valueOf(functionName.toUpperCase())`: the XML element name (`add`,
    /// `enchanthp`, ...) in any case. `None` where Java throws `IllegalArgumentException`.
    pub fn from_function_name(name: &str) -> Option<FuncKind> {
        Some(match name.to_uppercase().as_str() {
            "SET" => FuncKind::Set,
            "ADD" => FuncKind::Add,
            "SUB" => FuncKind::Sub,
            "MUL" => FuncKind::Mul,
            "DIV" => FuncKind::Div,
            "SHARE" => FuncKind::Share,
            "ENCHANT" => FuncKind::Enchant,
            "ENCHANTHP" => FuncKind::EnchantHp,
            _ => return None,
        })
    }
}

/// Java `FuncTemplate`. The conditions are shared (`Arc`): one `<conditions>` block is the attach
/// condition of every function of the same template, like Java's single `Condition` object.
#[derive(Debug, Clone, PartialEq)]
pub struct FuncTemplate {
    /// The stat it changes.
    pub stat: Stat,
    /// Which function it is.
    pub kind: FuncKind,
    /// Execution order: functions with a lower order run first. The XML `order` attribute, or
    /// the kind's default when it is absent (or negative).
    pub order: i32,
    /// The function's value.
    pub value: f64,
    /// Java `_attachCond`: tested when the template is turned into a function (for items: when
    /// equipped). `None` means always.
    pub attach_cond: Option<Arc<Condition>>,
    /// Java `_applyCond`: tested every time the function is calculated.
    pub apply_cond: Option<Arc<Condition>>,
}

impl FuncTemplate {
    /// Java `new FuncTemplate(attachCond, applyCond, functionName, order, stat, value)`.
    /// A negative `order` selects the kind's default. An unknown `function_name` is an error
    /// (Java: `IllegalArgumentException`).
    pub fn new(
        attach_cond: Option<Arc<Condition>>,
        apply_cond: Option<Arc<Condition>>,
        function_name: &str,
        order: i32,
        stat: Stat,
        value: f64,
    ) -> Result<Self, ParseError> {
        let kind = FuncKind::from_function_name(function_name).ok_or_else(|| {
            ParseError::invalid(format!(
                "No enum constant StatFunction.{}",
                function_name.to_uppercase()
            ))
        })?;
        Ok(Self::with_kind(attach_cond, apply_cond, kind, order, stat, value))
    }

    /// Same as [`FuncTemplate::new`] with a kind that is already known (cannot fail).
    pub fn with_kind(
        attach_cond: Option<Arc<Condition>>,
        apply_cond: Option<Arc<Condition>>,
        kind: FuncKind,
        order: i32,
        stat: Stat,
        value: f64,
    ) -> Self {
        Self {
            stat,
            kind,
            order: if order >= 0 { order } else { kind.default_order() },
            value,
            attach_cond,
            apply_cond,
        }
    }
}

/// Java `Stat.valueOfXml` as an error: `NoSuchElementException`.
fn unknown_stat(name: &str) -> ParseError {
    ParseError::invalid(format!("Unknown name '{name}' for enum BaseStats"))
}

/// The value of a function element or an alternative-format stat element: `#table` references go
/// through the parser's resolver, everything else is `Double.parseDouble`.
fn function_value(parser: &ConditionParser<'_>, text: &str) -> Result<f64, ParseError> {
    let number = if text.starts_with('#') {
        parser.value(text)?
    } else {
        text.to_owned()
    };
    parse_double(&number).ok_or_else(|| ParseError::invalid(number_format_message(&number)))
}

/// Java `DocumentBase.attachFunc`: `<add stat="pAtk" val="5" order="10">` and the `<value>` /
/// text forms. `Ok(None)` when the element has no `stat` attribute (Java returns silently).
///
/// `attach_cond` is the condition of the enclosing `<conditions>` block, if any.
pub fn attach_func(
    parser: &ConditionParser<'_>,
    n: &XmlNode,
    function_name: &str,
    attach_cond: Option<&Arc<Condition>>,
) -> Result<Option<FuncTemplate>, ParseError> {
    let Some(stat_name) = n.attr("stat") else {
        return Ok(None);
    };
    let stat = Stat::from_xml(stat_name).ok_or_else(|| unknown_stat(stat_name))?;
    let order = match n.attr("order") {
        Some(text) => parse_int(text).ok_or_else(|| ParseError::invalid(number_format_message(text)))?,
        None => -1,
    };

    // "Hybrid logic for capturing the value": `val`, else a `<value>` child, else the element text.
    let value_string = if let Some(val) = n.attr("val") {
        val.to_owned()
    } else {
        let from_child = n
            .children
            .iter()
            .find(|child| child.name.eq_ignore_ascii_case("value"))
            .map(|child| child.text_content().trim().to_owned());
        match from_child {
            Some(text) if !text.is_empty() => text,
            _ => n.text_content().trim().to_owned(),
        }
    };
    let value = if value_string.is_empty() {
        0.0
    } else {
        function_value(parser, &value_string)?
    };

    let apply_cond = parser.parse_condition(&n.children, 0)?.map(Arc::new);
    FuncTemplate::new(
        attach_cond.cloned(),
        apply_cond,
        function_name,
        order,
        stat,
        value,
    )
    .map(Some)
}

/// Java `DocumentBase.processAlternativeFuncFormat`: `<enchant><pDef>0</pDef><mDef>1</mDef></enchant>`.
/// Every child element is a stat; a child that fails (unknown stat, bad number, bad condition) is
/// logged with Java's text and skipped, the others still count. Never fails as a whole.
pub fn process_alternative_func_format(
    parser: &ConditionParser<'_>,
    n: &XmlNode,
    function_name: &str,
    attach_cond: Option<&Arc<Condition>>,
) -> Vec<FuncTemplate> {
    let mut out = Vec::new();
    for stat_node in &n.children {
        let stat_name = stat_node.name.as_str();
        let value_string = stat_node.text_content().trim().to_owned();
        let result = (|| -> Result<FuncTemplate, AlternativeError> {
            let stat =
                Stat::from_xml(stat_name).ok_or_else(|| AlternativeError::Other(unknown_stat(stat_name)))?;
            let number = if !value_string.is_empty() && value_string.starts_with('#') {
                parser.value(&value_string).map_err(AlternativeError::Other)?
            } else {
                value_string.clone()
            };
            let value = parse_double(&number)
                .ok_or_else(|| AlternativeError::Number(number_format_message(&number)))?;
            let apply_cond = parser
                .parse_condition(&stat_node.children, 0)
                .map_err(AlternativeError::Other)?
                .map(Arc::new);
            FuncTemplate::new(attach_cond.cloned(), apply_cond, function_name, -1, stat, value)
                .map_err(AlternativeError::Other)
        })();
        match result {
            Ok(template) => out.push(template),
            Err(AlternativeError::Number(message)) => {
                warn!("Invalid numeric value: '{value_string}' for stat: {stat_name}: {message}");
            }
            Err(AlternativeError::Other(error)) => {
                warn!("Error processing alternative {function_name} format for {stat_name}: {error}");
            }
        }
    }
    out
}

/// The two catch blocks of `processAlternativeFuncFormat` that log differently.
enum AlternativeError {
    /// `NumberFormatException`.
    Number(String),
    /// Everything else (`catch (Exception e)`).
    Other(ParseError),
}

/// The `add/sub/mul/div/set/share/enchant/enchanthp` case of `DocumentBase.parseTemplate`.
/// `name` is the element name (any case). An element with neither `stat` nor `val` attribute is
/// the alternative format with one child element per stat; otherwise it is the attribute form.
/// Returns the templates in the order Java attaches them.
pub fn parse_func_element(
    parser: &ConditionParser<'_>,
    n: &XmlNode,
    attach_cond: Option<&Arc<Condition>>,
) -> Result<Vec<FuncTemplate>, ParseError> {
    let name = n.name.to_lowercase();
    if n.attr("stat").is_none() && n.attr("val").is_none() {
        Ok(process_alternative_func_format(parser, n, &name, attach_cond))
    } else {
        Ok(attach_func(parser, n, &name, attach_cond)?.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::node::parse_xml;

    fn no_tables(_: &str) -> Option<String> {
        None
    }

    fn funcs(xml: &str) -> Result<Vec<FuncTemplate>, ParseError> {
        let node = parse_xml(xml).unwrap();
        parse_func_element(&ConditionParser::new("t.xml", &no_tables), &node, None)
    }

    #[test]
    fn default_orders_follow_stat_function() {
        let expected = [
            (FuncKind::Set, 0),
            (FuncKind::Add, 30),
            (FuncKind::Sub, 30),
            (FuncKind::Mul, 20),
            (FuncKind::Div, 20),
            (FuncKind::Share, 30),
            (FuncKind::Enchant, 0),
            (FuncKind::EnchantHp, 40),
        ];
        for (kind, order) in expected {
            assert_eq!(kind.default_order(), order, "{kind:?}");
            let t = FuncTemplate::with_kind(None, None, kind, -1, Stat::PowerAttack, 1.0);
            assert_eq!(t.order, order);
        }
        assert_eq!(FuncKind::ALL.len(), 8);
    }

    #[test]
    fn explicit_order_wins_and_zero_is_an_order() {
        let t = FuncTemplate::with_kind(None, None, FuncKind::Add, 0, Stat::MaxHp, 1.0);
        assert_eq!(t.order, 0);
        let t = FuncTemplate::with_kind(None, None, FuncKind::Add, 7, Stat::MaxHp, 1.0);
        assert_eq!(t.order, 7);
        // Negative means "use the default".
        let t = FuncTemplate::with_kind(None, None, FuncKind::Mul, -5, Stat::MaxHp, 1.0);
        assert_eq!(t.order, 20);
    }

    #[test]
    fn function_names_are_upper_cased_before_matching() {
        for kind in FuncKind::ALL {
            assert_eq!(FuncKind::from_function_name(kind.name()), Some(kind));
            assert_eq!(
                FuncKind::from_function_name(&kind.name().to_lowercase()),
                Some(kind)
            );
        }
        assert_eq!(
            FuncKind::from_function_name("enchanthp"),
            Some(FuncKind::EnchantHp)
        );
        assert_eq!(FuncKind::from_function_name("enchant_hp"), None);
        assert_eq!(FuncKind::from_function_name(""), None);
        assert!(FuncTemplate::new(None, None, "nope", -1, Stat::MaxHp, 0.0).is_err());
    }

    #[test]
    fn attribute_form() {
        let t = funcs(r#"<add stat="pAtk" val="5" order="10"/>"#).unwrap();
        assert_eq!(t.len(), 1);
        assert_eq!(
            (t[0].stat, t[0].kind, t[0].order, t[0].value),
            (Stat::PowerAttack, FuncKind::Add, 10, 5.0)
        );
        let t = funcs(r#"<MUL stat="runSpd" val="1.1"/>"#).unwrap();
        assert_eq!((t[0].kind, t[0].order, t[0].value), (FuncKind::Mul, 20, 1.1));
    }

    #[test]
    fn value_forms() {
        // <value> child
        let t = funcs(r#"<set stat="maxHp"><value> 12 </value></set>"#).unwrap();
        assert_eq!(t[0].value, 12.0);
        // element text (no val attribute, so it is the attribute form because `stat` is present)
        let t = funcs(r#"<add stat="maxHp"> 7 </add>"#).unwrap();
        assert_eq!(t[0].value, 7.0);
        // empty: 0
        let t = funcs(r#"<add stat="maxHp"/>"#).unwrap();
        assert_eq!(t[0].value, 0.0);
        // an empty <value> falls back to the element text, which is empty too
        let t = funcs(r#"<add stat="maxHp"><value/></add>"#).unwrap();
        assert_eq!(t[0].value, 0.0);
    }

    #[test]
    fn apply_condition_is_parsed_from_the_children() {
        let t = funcs(r#"<mul stat="pDef" val="1.5"><using kind="HEAVY"/></mul>"#).unwrap();
        assert!(matches!(
            t[0].apply_cond.as_deref(),
            Some(Condition::UsingItemType { .. })
        ));
        assert!(t[0].attach_cond.is_none());
    }

    #[test]
    fn attach_condition_is_shared() {
        let cond = Arc::new(Condition::PlayerLevel { level: 5 });
        let node = parse_xml(r#"<add stat="pAtk" val="5"/>"#).unwrap();
        let parser = ConditionParser::new("t.xml", &no_tables);
        let t = attach_func(&parser, &node, "add", Some(&cond)).unwrap().unwrap();
        assert!(Arc::ptr_eq(t.attach_cond.as_ref().unwrap(), &cond));
    }

    #[test]
    fn element_without_stat_is_silently_ignored() {
        let node = parse_xml(r#"<add val="5"/>"#).unwrap();
        let parser = ConditionParser::new("t.xml", &no_tables);
        assert_eq!(attach_func(&parser, &node, "add", None).unwrap(), None);
    }

    #[test]
    fn bad_attribute_form_fails() {
        assert!(funcs(r#"<add stat="nope" val="1"/>"#).is_err());
        assert!(funcs(r#"<add stat="patk" val="1"/>"#).is_err()); // case sensitive
        assert!(funcs(r#"<add stat="pAtk" val="x"/>"#).is_err());
        assert!(funcs(r#"<add stat="pAtk" val="1" order="x"/>"#).is_err());
        assert!(funcs(r##"<add stat="pAtk" val="#t"/>"##).is_err());
    }

    #[test]
    fn table_values_use_the_resolver() {
        let tables = |name: &str| (name == "#pAtk").then(|| "42".to_owned());
        let parser = ConditionParser::new("skill.xml", &tables);
        let node = parse_xml(r##"<add stat="pAtk" val="#pAtk"/>"##).unwrap();
        let t = attach_func(&parser, &node, "add", None).unwrap().unwrap();
        assert_eq!(t.value, 42.0);
        let node = parse_xml(r##"<enchant><pDef>#pAtk</pDef></enchant>"##).unwrap();
        let t = process_alternative_func_format(&parser, &node, "enchant", None);
        assert_eq!(t[0].value, 42.0);
    }

    #[test]
    fn alternative_format_skips_bad_children_and_keeps_the_rest() {
        let t = funcs(r#"<enchant><pDef>1</pDef><nope>2</nope><mDef>x</mDef><sDef> 3 </sDef></enchant>"#)
            .unwrap();
        assert_eq!(
            t.iter()
                .map(|f| (f.stat, f.kind, f.order, f.value))
                .collect::<Vec<_>>(),
            vec![
                (Stat::PowerDefence, FuncKind::Enchant, 0, 1.0),
                (Stat::ShieldDefence, FuncKind::Enchant, 0, 3.0),
            ]
        );
        let t = funcs(r#"<enchanthp><maxHp>2</maxHp></enchanthp>"#).unwrap();
        assert_eq!((t[0].kind, t[0].order), (FuncKind::EnchantHp, 40));
        // An empty value is a bad number there (unlike the attribute form).
        assert!(funcs(r#"<share><maxHp/></share>"#).unwrap().is_empty());
        // Conditions inside a stat element are its apply condition.
        let t = funcs(r#"<enchant><pDef><player level="5"/></pDef></enchant>"#).unwrap();
        assert!(t.is_empty(), "the empty value fails before the condition");
        let t = funcs(r#"<enchant><pDef>1<player level="5"/></pDef></enchant>"#).unwrap();
        assert_eq!(
            t[0].apply_cond.as_deref(),
            Some(&Condition::PlayerLevel { level: 5 })
        );
    }
}
