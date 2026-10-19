//! A small, ordered XML element tree, the Rust stand-in for the W3C DOM nodes that Java's
//! `DocumentBase` walks.
//!
//! The datapack loaders deserialize with serde into typed structs, which cannot express the
//! free-form `<stats>`, `<skills>` and `<conditions>` blocks (the parser must see every element
//! and attribute, in document order). [`XmlNode`] implements `Deserialize`, so a raw struct can
//! simply declare a field of this type and get the whole subtree.
//!
//! What is kept from the DOM, and what is not:
//! - Only **element** children. Java skips text and comment nodes everywhere it walks siblings
//!   (it checks `ELEMENT_NODE`), so dropping them changes nothing.
//! - Attributes are sorted by name. Xerces' `NamedNodeMap` keeps its nodes sorted by name, and
//!   `DocumentBase.parse*Condition` loops over `attrs.item(i)`, so the order in which the
//!   attributes of one `<player ...>` element become `and` terms is the alphabetical one, not the
//!   order in the file.
//! - Whether the element's **first DOM child is a text node** is kept in [`XmlNode::leading_text`],
//!   but only by [`parse_dom`] (the serde path cannot see whitespace). Java's `parseTemplate` looks
//!   at `getFirstChild()` without skipping text, see the field.
//! - The text of the element itself is kept (`$text` pieces concatenated); [`XmlNode::text_content`]
//!   adds the text of the descendants, like `Node.getTextContent()`.

use std::fmt;

use quick_xml::XmlVersion;
use quick_xml::escape::resolve_predefined_entity;
use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;
use serde::Deserialize;
use serde::de::{self, MapAccess, Visitor};

/// One XML element: its name, attributes, own text and element children.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct XmlNode {
    /// Element name as written in the file (case preserved, Java compares with
    /// `equalsIgnoreCase` or `toLowerCase()` where it matters).
    pub name: String,
    /// `(name, value)` pairs, sorted by name (see the module docs).
    pub attrs: Vec<(String, String)>,
    /// The element's own text, untrimmed.
    pub text: String,
    /// Element children in document order.
    pub children: Vec<XmlNode>,
    /// True when the first DOM child of this element is a text node (any character data before
    /// the first child element, whitespace included; comments do not count because Java's parser
    /// drops them). Filled by [`parse_dom`]; always `false` for nodes made by serde or by hand.
    ///
    /// NOTE(java-quirk): `DocumentBase.parse()` uses a non-validating `DocumentBuilderFactory`
    /// that only ignores comments, so the indentation between tags stays in the DOM as text
    /// nodes. `parseTemplate` then tests `node.getFirstChild()` for `<conditions>` without
    /// skipping text: in an indented file the first child is whitespace, the `<conditions>` is not
    /// taken as the template condition, and the loop below it has no case for it. Only a
    /// `<conditions>` written directly after the opening tag counts.
    pub leading_text: bool,
}

impl XmlNode {
    /// Builds a node by hand. For tests and tools; attributes are sorted like the parser does.
    pub fn new(name: &str, attrs: &[(&str, &str)], children: Vec<XmlNode>) -> Self {
        let mut node = XmlNode {
            name: name.to_owned(),
            attrs: attrs.iter().map(|&(k, v)| (k.to_owned(), v.to_owned())).collect(),
            text: String::new(),
            children,
            leading_text: false,
        };
        node.attrs.sort_by(|a, b| a.0.cmp(&b.0));
        node
    }

    /// Same as [`XmlNode::new`] for a leaf with text, e.g. `<stat type="pAtk">8</stat>`.
    pub fn with_text(name: &str, attrs: &[(&str, &str)], text: &str) -> Self {
        let mut node = Self::new(name, attrs, Vec::new());
        node.text = text.to_owned();
        node
    }

    /// Java `getAttributes().getNamedItem(name)`: exact, case-sensitive name.
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    /// Java `Node.getTextContent()` for an element: own text, then the text of the descendants.
    ///
    /// NOTE(java-quirk): the DOM interleaves text and child elements in document order; this tree
    /// keeps the element's own text first. The two differ only for mixed content, which the
    /// datapack does not have (and the callers trim the result).
    pub fn text_content(&self) -> String {
        let mut out = self.text.clone();
        for child in &self.children {
            out.push_str(&child.text_content());
        }
        out
    }

    /// The first child element with this exact name.
    pub fn child(&self, name: &str) -> Option<&XmlNode> {
        self.children.iter().find(|c| c.name == name)
    }
}

impl<'de> Deserialize<'de> for XmlNode {
    /// Reads the *content* of an element. `name` stays empty here: the parent's `visit_map` knows
    /// the key (the child element's name) and fills it in, and the root of a raw struct is named
    /// by the field or enum variant that holds it (callers set it where it matters).
    fn deserialize<D: de::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct NodeVisitor;

        impl<'de> Visitor<'de> for NodeVisitor {
            type Value = XmlNode;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an XML element")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<XmlNode, A::Error> {
                let mut node = XmlNode::default();
                while let Some(key) = map.next_key::<String>()? {
                    if let Some(attribute) = key.strip_prefix('@') {
                        let value: String = map.next_value()?;
                        node.attrs.push((attribute.to_owned(), value));
                    } else if key == "$text" || key == "$value" {
                        let value: String = map.next_value()?;
                        node.text.push_str(&value);
                    } else {
                        let mut child: XmlNode = map.next_value()?;
                        child.name = key;
                        node.children.push(child);
                    }
                }
                node.attrs.sort_by(|a, b| a.0.cmp(&b.0));
                Ok(node)
            }

            // An empty element (`<stats/>`) may reach the visitor as a unit.
            fn visit_unit<E: de::Error>(self) -> Result<XmlNode, E> {
                Ok(XmlNode::default())
            }

            // `<tag>text</tag>` may reach the visitor as a plain string.
            fn visit_str<E: de::Error>(self, value: &str) -> Result<XmlNode, E> {
                Ok(XmlNode {
                    text: value.to_owned(),
                    ..XmlNode::default()
                })
            }

            fn visit_string<E: de::Error>(self, value: String) -> Result<XmlNode, E> {
                Ok(XmlNode {
                    text: value,
                    ..XmlNode::default()
                })
            }
        }

        deserializer.deserialize_any(NodeVisitor)
    }
}

/// Parses a whole document into a tree whose root is the document element. For unit tests and
/// tools; the loaders get their nodes through serde.
pub fn parse_xml(xml: &str) -> Result<XmlNode, quick_xml::DeError> {
    #[derive(Deserialize)]
    struct Doc(XmlNode);
    // serde needs a name for the root; the document element's own name is not exposed by
    // `quick_xml::de::from_str`, so wrap it in a synthetic parent.
    let wrapped = format!("<root>{}</root>", strip_prolog(xml));
    let doc: XmlNode = quick_xml::de::from_str::<Doc>(&wrapped)?.0;
    Ok(doc.children.into_iter().next().unwrap_or_default())
}

/// Why [`parse_dom`] failed (malformed XML, or a reference that is not a predefined entity).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct DomError(pub String);

fn dom_error(error: impl fmt::Display) -> DomError {
    DomError(error.to_string())
}

/// Parses a document into the tree Java's `DocumentBuilder` would build, as far as the skill
/// loader needs it: comments are dropped, text is kept, and [`XmlNode::leading_text`] records
/// whether an element starts with a text node. Returns the document element.
///
/// Unlike the serde path this keeps whitespace, so text is never trimmed here. Attribute values
/// are normalized like Xerces does (tabs and line breaks become spaces, predefined entities and
/// character references are resolved). DTD-defined entities are not supported (Java would need the
/// DTD; the datapack has none).
pub fn parse_dom(xml: &str) -> Result<XmlNode, DomError> {
    let xml = xml.strip_prefix('\u{feff}').unwrap_or(xml);
    let mut reader = Reader::from_str(xml);
    let mut stack: Vec<XmlNode> = Vec::new();
    let mut root: Option<XmlNode> = None;

    fn close(node: XmlNode, stack: &mut [XmlNode], root: &mut Option<XmlNode>) {
        match stack.last_mut() {
            Some(parent) => parent.children.push(node),
            None => {
                // Only the first top level element is the document element.
                root.get_or_insert(node);
            }
        }
    }

    fn add_text(stack: &mut [XmlNode], text: &str) {
        if text.is_empty() {
            return;
        }
        if let Some(top) = stack.last_mut() {
            if top.children.is_empty() {
                top.leading_text = true;
            }
            top.text.push_str(text);
        }
    }

    loop {
        match reader.read_event().map_err(dom_error)? {
            Event::Start(e) => stack.push(element(&e)?),
            Event::Empty(e) => {
                let node = element(&e)?;
                close(node, &mut stack, &mut root);
            }
            Event::End(_) => {
                let node = stack.pop().ok_or_else(|| dom_error("unexpected end tag"))?;
                close(node, &mut stack, &mut root);
            }
            Event::Text(t) => add_text(&mut stack, &t.xml_content(XmlVersion::Implicit1_0)),
            Event::CData(t) => add_text(&mut stack, &t.xml_content(XmlVersion::Implicit1_0)),
            Event::GeneralRef(r) => {
                let mut buffer = [0u8; 4];
                let text: &str = match r.resolve_char_ref().map_err(dom_error)? {
                    Some(c) => c.encode_utf8(&mut buffer),
                    None => resolve_predefined_entity(&r)
                        .ok_or_else(|| dom_error(format!("unknown entity &{};", &*r)))?,
                };
                add_text(&mut stack, text);
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if !stack.is_empty() {
        return Err(dom_error("unclosed element"));
    }
    root.ok_or_else(|| dom_error("no document element"))
}

fn element(start: &BytesStart<'_>) -> Result<XmlNode, DomError> {
    let mut node = XmlNode {
        name: start.name().as_ref().to_owned(),
        ..XmlNode::default()
    };
    for attribute in start.attributes() {
        let attribute = attribute.map_err(dom_error)?;
        let value = attribute
            .normalized_value(XmlVersion::Implicit1_0)
            .map_err(dom_error)?;
        node.attrs
            .push((attribute.key.as_ref().to_owned(), value.into_owned()));
    }
    node.attrs.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(node)
}

fn strip_prolog(xml: &str) -> &str {
    let trimmed = xml.trim_start();
    if let Some(rest) = trimmed.strip_prefix("<?xml") {
        if let Some(end) = rest.find("?>") {
            return &rest[end + 2..];
        }
    }
    trimmed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attributes_text_and_children_in_order() {
        let root = parse_xml(
            r#"<conditions msgId="1518" addName="1">
                <player races="HUMAN,ELF" level="40"/>
                <and><player flyMounted="false"/><target playable="true"/></and>
                <stat type="pAtk">8</stat>
            </conditions>"#,
        )
        .unwrap();
        assert_eq!(root.name, "conditions");
        // Sorted by name, like Xerces.
        assert_eq!(
            root.attrs,
            vec![
                ("addName".to_owned(), "1".to_owned()),
                ("msgId".to_owned(), "1518".to_owned())
            ]
        );
        let names: Vec<&str> = root.children.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["player", "and", "stat"]);
        assert_eq!(root.children[0].attr("level"), Some("40"));
        assert_eq!(root.children[1].children.len(), 2);
        assert_eq!(root.children[2].text, "8");
        assert_eq!(root.children[2].attr("type"), Some("pAtk"));
    }

    #[test]
    fn repeated_and_interleaved_elements_keep_document_order() {
        let root = parse_xml("<a><x/><y/><x/><y/><x/></a>").unwrap();
        let names: Vec<&str> = root.children.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["x", "y", "x", "y", "x"]);
    }

    #[test]
    fn text_content_includes_descendants() {
        let root = parse_xml("<a>1<b>2</b><c><d>3</d></c></a>").unwrap();
        assert_eq!(root.text_content(), "123");
        let root = parse_xml("<add stat=\"x\"> 5 </add>").unwrap();
        assert_eq!(root.text_content().trim(), "5");
    }

    #[test]
    fn dom_marks_elements_that_start_with_text() {
        let root = parse_dom(
            "<?xml version=\"1.0\"?>\n<!-- c --><skill id=\"1\">\n\t<!-- c --><effects><conditions><player level=\"1\"/></conditions>\
             <effect name=\"A\"/></effects>\n\t<other> <!-- c --> <x/></other><e2><!-- only a comment --></e2>\n</skill>",
        )
        .unwrap();
        assert_eq!(root.name, "skill");
        // Whitespace before the first child element.
        assert!(root.leading_text);
        let effects = root.child("effects").unwrap();
        // Written back to back: the first child is the <conditions> element itself.
        assert!(!effects.leading_text);
        assert_eq!(effects.children[0].name, "conditions");
        assert!(!effects.children[0].leading_text);
        // Text, comment, text: still text first.
        assert!(root.child("other").unwrap().leading_text);
        // A comment alone leaves no child at all.
        let e2 = root.child("e2").unwrap();
        assert!(!e2.leading_text && e2.children.is_empty() && e2.text.is_empty());
        // The serde path does not know and says false.
        assert!(!parse_xml("<a> <b/></a>").unwrap().leading_text);
    }

    #[test]
    fn dom_keeps_text_decodes_entities_and_normalizes_attributes() {
        let root = parse_dom("<a v=\"x &amp; y\tz\"> 1 &lt; 2 &#65;<![CDATA[<b>]]></a>").unwrap();
        assert_eq!(root.attr("v"), Some("x & y z"));
        assert_eq!(root.text, " 1 < 2 A<b>");
        assert!(parse_dom("<a><b></a>").is_err());
        assert!(parse_dom("<a>&nope;</a>").is_err());
        assert!(parse_dom("   ").is_err());
    }

    #[test]
    fn entities_are_decoded() {
        let root = parse_xml(r#"<a v="x &amp; y">a &lt; b</a>"#).unwrap();
        assert_eq!(root.attr("v"), Some("x & y"));
        assert_eq!(root.text, "a < b");
    }
}
