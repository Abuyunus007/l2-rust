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
//! - The text of the element itself is kept (`$text` pieces concatenated); [`XmlNode::text_content`]
//!   adds the text of the descendants, like `Node.getTextContent()`.

use std::fmt;

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
}

impl XmlNode {
    /// Builds a node by hand. For tests and tools; attributes are sorted like the parser does.
    pub fn new(name: &str, attrs: &[(&str, &str)], children: Vec<XmlNode>) -> Self {
        let mut node = XmlNode {
            name: name.to_owned(),
            attrs: attrs.iter().map(|&(k, v)| (k.to_owned(), v.to_owned())).collect(),
            text: String::new(),
            children,
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
    fn entities_are_decoded() {
        let root = parse_xml(r#"<a v="x &amp; y">a &lt; b</a>"#).unwrap();
        assert_eq!(root.attr("v"), Some("x & y"));
        assert_eq!(root.text, "a < b");
    }
}
