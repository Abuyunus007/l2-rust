//! In-place editor for the XML config files (`AccessLevels.xml`, `SiegeSchedule.xml`, ...).
//!
//! Java: none (tooling). The servers read these files with their own loaders; this module only
//! changes them without rewriting them. A normal XML writer would lose the comments, the tabs,
//! the attribute order and the line endings, so it is not used (card G-08, same idea as
//! [`crate::ini_edit`] for `.ini` files).
//!
//! # How it works
//! [`XmlDoc::parse`] tokenises the text with `quick-xml` and keeps, for every element and
//! attribute, the byte range it has in the original text. Changes are [`Edit`]s: "replace
//! `start..end` of the original by this text". They are collected against the *original* text and
//! applied together by [`apply_edits`], so no edit has to know about the others. Nothing outside
//! the edited ranges is touched, which gives the round-trip rule: no edits, identical bytes.
//!
//! New lines use the file's own line ending (`\r\n` or `\n`) and indentation.

use quick_xml::Reader;
use quick_xml::events::Event;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum XmlEditError {
    #[error("not valid XML at byte {pos}: {message}")]
    Parse { pos: usize, message: String },
    #[error("two changes overlap at byte {0}")]
    Overlap(usize),
}

/// One attribute of a start tag. The offsets are into the original text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmlAttr {
    pub name: String,
    /// The value with the entities (`&amp;`) resolved.
    pub value: String,
    /// Start of the name.
    pub start: usize,
    /// The value without its quotes.
    pub value_start: usize,
    pub value_end: usize,
    /// Just after the closing quote.
    pub end: usize,
    /// The quote character the file uses (`"` or `'`).
    pub quote: char,
}

/// One element. Index = position in document order (the root is 0).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmlElement {
    pub name: String,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    pub attrs: Vec<XmlAttr>,
    /// The `<` of the start tag.
    pub start: usize,
    /// Just after the `>` of the start tag (`end` for `<a/>`).
    pub open_end: usize,
    /// The `<` of the end tag (`end` for `<a/>`).
    pub close_start: usize,
    /// Just after the element.
    pub end: usize,
    /// `<a/>`: no end tag.
    pub empty: bool,
}

/// "Replace `start..end` of the original text by `text`". `start == end` is an insertion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    pub start: usize,
    pub end: usize,
    pub text: String,
}

/// A parsed file. It never changes; edits are built from it and applied to its text.
#[derive(Debug, Clone)]
pub struct XmlDoc {
    text: String,
    elements: Vec<XmlElement>,
    eol: &'static str,
}

fn offset(pos: u64) -> usize {
    usize::try_from(pos).unwrap_or(usize::MAX)
}

/// `&amp;`, `&lt;`, `&gt;`, `&quot;`, `&apos;` and `&#10;` / `&#xA;`. Unknown entities stay as
/// they are.
pub fn unescape(raw: &str) -> String {
    if !raw.contains('&') {
        return raw.to_owned();
    }
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let Some(semi) = rest.find(';') else { break };
        let entity = &rest[1..semi];
        let replacement = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => entity.strip_prefix('#').and_then(|n| {
                let code = match n.strip_prefix(['x', 'X']) {
                    Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                    None => n.parse().ok()?,
                };
                char::from_u32(code)
            }),
        };
        match replacement {
            Some(c) => {
                out.push(c);
                rest = &rest[semi + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Escapes an attribute value for `quote`.
pub fn escape_attr(value: &str, quote: char) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '"' if quote == '"' => out.push_str("&quot;"),
            '\'' if quote == '\'' => out.push_str("&apos;"),
            '\n' => out.push_str("&#10;"),
            '\r' => out.push_str("&#13;"),
            '\t' => out.push_str("&#9;"),
            _ => out.push(c),
        }
    }
    out
}

/// Escapes element text.
pub fn escape_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// `<name a="1" b="2" />` (or `<name a="1">text</name>`), the way the dist files write elements.
pub fn element_snippet(name: &str, attrs: &[(String, String)], text: Option<&str>) -> String {
    let mut out = format!("<{name}");
    for (key, value) in attrs {
        out.push_str(&format!(" {key}=\"{}\"", escape_attr(value, '"')));
    }
    match text {
        Some(text) => out.push_str(&format!(">{}</{name}>", escape_text(text))),
        None => out.push_str(" />"),
    }
    out
}

fn is_blank(s: &str) -> bool {
    s.chars().all(|c| matches!(c, ' ' | '\t'))
}

impl XmlDoc {
    pub fn parse(text: &str) -> Result<Self, XmlEditError> {
        let mut reader = Reader::from_str(text);
        let mut elements: Vec<XmlElement> = Vec::new();
        let mut stack: Vec<usize> = Vec::new();
        loop {
            let before = offset(reader.buffer_position());
            let event = reader.read_event().map_err(|e| XmlEditError::Parse {
                pos: before,
                message: e.to_string(),
            })?;
            let after = offset(reader.buffer_position());
            match event {
                Event::Start(e) | Event::Empty(e) => {
                    let empty = text[before..after].ends_with("/>");
                    let index = elements.len();
                    let parent = stack.last().copied();
                    if let Some(p) = parent {
                        elements[p].children.push(index);
                    }
                    elements.push(XmlElement {
                        name: e.name().as_ref().to_owned(),
                        parent,
                        children: Vec::new(),
                        attrs: parse_attrs(text, before, after)?,
                        start: before,
                        open_end: after,
                        close_start: after,
                        end: after,
                        empty,
                    });
                    if !empty {
                        stack.push(index);
                    }
                }
                Event::End(_) => {
                    if let Some(index) = stack.pop() {
                        elements[index].close_start = before;
                        elements[index].end = after;
                    }
                }
                Event::Eof => break,
                _ => {}
            }
        }
        if elements.is_empty() {
            return Err(XmlEditError::Parse {
                pos: 0,
                message: "no root element".into(),
            });
        }
        let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
        Ok(Self {
            text: text.to_owned(),
            elements,
            eol,
        })
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// The line ending of the file (`\r\n` when the file has any).
    pub fn eol(&self) -> &'static str {
        self.eol
    }

    pub fn elements(&self) -> &[XmlElement] {
        &self.elements
    }

    pub fn element(&self, index: usize) -> &XmlElement {
        &self.elements[index]
    }

    /// The root element (always index 0).
    pub fn root(&self) -> usize {
        0
    }

    /// The children of `parent` called `name` (ASCII case-insensitive, like Java's
    /// `equalsIgnoreCase` in most of the loaders).
    pub fn children_named(&self, parent: usize, name: &str) -> Vec<usize> {
        self.elements[parent]
            .children
            .iter()
            .copied()
            .filter(|&c| self.elements[c].name.eq_ignore_ascii_case(name))
            .collect()
    }

    pub fn first_child_named(&self, parent: usize, name: &str) -> Option<usize> {
        self.children_named(parent, name).first().copied()
    }

    pub fn attr(&self, element: usize, name: &str) -> Option<&str> {
        self.elements[element]
            .attrs
            .iter()
            .find(|a| a.name == name)
            .map(|a| a.value.as_str())
    }

    /// The text of an element without children, entities resolved. Empty for `<a/>`.
    pub fn text_of(&self, element: usize) -> String {
        let e = &self.elements[element];
        if e.empty {
            return String::new();
        }
        unescape(&self.text[e.open_end..e.close_start])
    }

    /// One indentation step: the indent of the first child of the root, else a tab.
    pub fn indent_unit(&self) -> String {
        let root = &self.elements[0];
        let Some(&child) = root.children.first() else {
            return "\t".into();
        };
        let indent = self.line_indent(self.elements[child].start);
        let base = self.line_indent(root.start);
        match indent.strip_prefix(base.as_str()) {
            Some(unit) if !unit.is_empty() => unit.to_owned(),
            _ => "\t".into(),
        }
    }

    /// The blanks at the start of the line that contains `pos`.
    fn line_indent(&self, pos: usize) -> String {
        let line_start = self.text[..pos].rfind('\n').map_or(0, |i| i + 1);
        self.text[line_start..pos]
            .chars()
            .take_while(|c| matches!(c, ' ' | '\t'))
            .collect()
    }

    /// Is `pos` preceded by nothing but blanks on its line?
    fn starts_line(&self, pos: usize) -> bool {
        let line_start = self.text[..pos].rfind('\n').map_or(0, |i| i + 1);
        is_blank(&self.text[line_start..pos])
    }

    /// The end of the line that holds `pos` (the `\n`, or the end of the text) and whether the
    /// rest of that line, from `pos`, is only blanks (a `\r` counts as blank).
    fn rest_of_line(&self, pos: usize) -> (usize, bool) {
        let nl = self.text[pos..].find('\n').map_or(self.text.len(), |i| pos + i);
        let blank = self.text[pos..nl].chars().all(|c| matches!(c, ' ' | '\t' | '\r'));
        (nl, blank)
    }

    // ---- edits ----------------------------------------------------------------------------

    /// Sets an attribute: replaces its value, or appends ` name="value"` to the start tag.
    /// `None` when the value is already `value`.
    pub fn set_attr(&self, element: usize, name: &str, value: &str) -> Option<Edit> {
        let e = &self.elements[element];
        if let Some(a) = e.attrs.iter().find(|a| a.name == name) {
            if a.value == value {
                return None;
            }
            return Some(Edit {
                start: a.value_start,
                end: a.value_end,
                text: escape_attr(value, a.quote),
            });
        }
        let at = e.attrs.last().map_or(e.start + 1 + e.name.len(), |a| a.end);
        Some(Edit {
            start: at,
            end: at,
            text: format!(" {name}=\"{}\"", escape_attr(value, '"')),
        })
    }

    /// Removes an attribute with the blanks before it. `None` when it is not there.
    pub fn remove_attr(&self, element: usize, name: &str) -> Option<Edit> {
        let a = self.elements[element].attrs.iter().find(|a| a.name == name)?;
        let blanks = self.text[..a.start]
            .chars()
            .rev()
            .take_while(|c| c.is_whitespace())
            .map(char::len_utf8)
            .sum::<usize>();
        Some(Edit {
            start: a.start - blanks,
            end: a.end,
            text: String::new(),
        })
    }

    /// Sets the text of an element that has no child elements. `None` when it is unchanged.
    /// An empty element `<a />` becomes `<a>text</a>`.
    pub fn set_text(&self, element: usize, value: &str) -> Option<Edit> {
        let e = &self.elements[element];
        if !e.children.is_empty() {
            return None;
        }
        if e.empty {
            if value.is_empty() {
                return None;
            }
            let tag = &self.text[e.start..e.end];
            let slash = e.start + tag.rfind('/')?;
            let from = slash - (self.text[..slash].len() - self.text[..slash].trim_end().len());
            return Some(Edit {
                start: from,
                end: e.end,
                text: format!(">{}</{}>", escape_text(value), e.name),
            });
        }
        if self.text_of(element) == value {
            return None;
        }
        Some(Edit {
            start: e.open_end,
            end: e.close_start,
            text: escape_text(value),
        })
    }

    /// Removes an element. When it sits alone on its lines, the lines go (indent and line break
    /// included); otherwise only the element.
    pub fn remove_element(&self, element: usize) -> Edit {
        let e = &self.elements[element];
        let (nl, blank_after) = self.rest_of_line(e.end);
        if self.starts_line(e.start) && blank_after {
            let line_start = self.text[..e.start].rfind('\n').map_or(0, |i| i + 1);
            let end = if nl < self.text.len() { nl + 1 } else { nl };
            return Edit {
                start: line_start,
                end,
                text: String::new(),
            };
        }
        Edit {
            start: e.start,
            end: e.end,
            text: String::new(),
        }
    }

    /// Puts `snippet` (one or more lines, joined by `\n`, relative indentation as tabs of
    /// [`Self::indent_unit`]) after `element`, on its own line(s) with the element's indent.
    pub fn insert_after(&self, element: usize, snippet: &str) -> Edit {
        let e = &self.elements[element];
        let (nl, _) = self.rest_of_line(e.end);
        let rest = &self.text[e.end..nl];
        if self.starts_line(e.start) && nl < self.text.len() && !rest.contains("</") {
            let indent = self.line_indent(e.start);
            return Edit {
                start: nl + 1,
                end: nl + 1,
                text: format!("{}{}", self.indent_lines(snippet, &indent), self.eol),
            };
        }
        // `<list><a/></list>` on one line: stay on that line.
        Edit {
            start: e.end,
            end: e.end,
            text: snippet.replace('\n', ""),
        }
    }

    /// Puts `snippet` as the first child of `parent` (an empty element `<a />` is opened up).
    pub fn insert_first_child(&self, parent: usize, snippet: &str) -> Edit {
        let p = &self.elements[parent];
        let base = if self.starts_line(p.start) {
            self.line_indent(p.start)
        } else {
            String::new()
        };
        let indent = format!("{base}{}", self.indent_unit());
        if p.empty {
            let tag = &self.text[p.start..p.end];
            let slash = p.start + tag.rfind('/').unwrap_or(tag.len() - 2);
            let from = slash - (self.text[..slash].len() - self.text[..slash].trim_end().len());
            return Edit {
                start: from,
                end: p.end,
                text: format!(
                    ">{eol}{}{eol}{base}</{}>",
                    self.indent_lines(snippet, &indent),
                    p.name,
                    eol = self.eol
                ),
            };
        }
        let (nl, blank) = self.rest_of_line(p.open_end);
        if blank && nl < self.text.len() {
            return Edit {
                start: nl + 1,
                end: nl + 1,
                text: format!("{}{}", self.indent_lines(snippet, &indent), self.eol),
            };
        }
        Edit {
            start: p.open_end,
            end: p.open_end,
            text: snippet.replace('\n', ""),
        }
    }

    /// Puts `snippet` after the last child element of `parent`, or as its first child.
    pub fn append_child(&self, parent: usize, snippet: &str) -> Edit {
        match self.elements[parent].children.last() {
            Some(&last) => self.insert_after(last, snippet),
            None => self.insert_first_child(parent, snippet),
        }
    }

    /// Every line of `snippet` with `indent` in front; lines joined by the file's line ending.
    fn indent_lines(&self, snippet: &str, indent: &str) -> String {
        snippet
            .lines()
            .map(|line| format!("{indent}{line}"))
            .collect::<Vec<_>>()
            .join(self.eol)
    }
}

/// The attributes of the start tag `text[start..end]`.
fn parse_attrs(text: &str, start: usize, end: usize) -> Result<Vec<XmlAttr>, XmlEditError> {
    let bytes = text.as_bytes();
    let fail = |pos: usize, message: &str| XmlEditError::Parse {
        pos,
        message: message.into(),
    };
    let mut i = start + 1;
    while i < end && !bytes[i].is_ascii_whitespace() && !matches!(bytes[i], b'/' | b'>') {
        i += 1; // the tag name
    }
    let mut attrs = Vec::new();
    loop {
        while i < end && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= end || matches!(bytes[i], b'/' | b'>') {
            return Ok(attrs);
        }
        let name_start = i;
        while i < end && bytes[i] != b'=' && !bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let name = text[name_start..i].to_owned();
        while i < end && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= end || bytes[i] != b'=' {
            return Err(fail(i, "attribute without a value"));
        }
        i += 1;
        while i < end && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= end || !matches!(bytes[i], b'"' | b'\'') {
            return Err(fail(i, "attribute value without quotes"));
        }
        let quote = bytes[i];
        let value_start = i + 1;
        let Some(len) = text[value_start..end].find(quote as char) else {
            return Err(fail(i, "unterminated attribute value"));
        };
        let value_end = value_start + len;
        attrs.push(XmlAttr {
            name,
            value: unescape(&text[value_start..value_end]),
            start: name_start,
            value_start,
            value_end,
            end: value_end + 1,
            quote: quote as char,
        });
        i = value_end + 1;
    }
}

/// Applies the edits (all against the same original text). They must not overlap; insertions at
/// the same position keep their order.
pub fn apply_edits(text: &str, mut edits: Vec<Edit>) -> Result<String, XmlEditError> {
    edits.sort_by_key(|e| (e.start, e.end)); // stable: equal keys keep the caller's order
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0;
    for edit in &edits {
        if edit.start < cursor || edit.end < edit.start || edit.end > text.len() {
            return Err(XmlEditError::Overlap(edit.start));
        }
        out.push_str(&text[cursor..edit.start]);
        out.push_str(&edit.text);
        cursor = edit.end;
    }
    out.push_str(&text[cursor..]);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LF: &str = "<?xml version=\"1.0\"?>\n<list a=\"1\">\n\t<!-- c <x/> -->\n\t<item id=\"1\" name=\"A &amp; B\" />\n\t<item id=\"2\"/>\n\t<box>\n\t\t<item id=\"3\" />\n\t</box>\n\t<t>text</t> <!-- tail -->\n\t<e />\n</list>\n";

    fn crlf(s: &str) -> String {
        s.replace('\n', "\r\n")
    }

    #[test]
    fn parses_elements_attributes_and_text() {
        let doc = XmlDoc::parse(LF).unwrap();
        let names: Vec<&str> = doc.elements().iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["list", "item", "item", "box", "item", "t", "e"]);
        assert_eq!(doc.attr(1, "name"), Some("A & B"));
        assert_eq!(doc.attr(1, "id"), Some("1"));
        assert_eq!(doc.children_named(0, "ITEM"), [1, 2]);
        assert_eq!(doc.text_of(5), "text");
        assert!(doc.element(2).empty && doc.element(6).empty);
        assert_eq!(doc.indent_unit(), "\t");
        // The offsets point at the right bytes.
        let a = &doc.element(1).attrs[0];
        assert_eq!(&LF[a.value_start..a.value_end], "1");
    }

    #[test]
    fn no_edits_is_the_same_text() {
        assert_eq!(apply_edits(LF, vec![]).unwrap(), LF);
    }

    #[test]
    fn set_attr_touches_only_the_value() {
        let doc = XmlDoc::parse(LF).unwrap();
        assert!(doc.set_attr(1, "id", "1").is_none());
        let edit = doc.set_attr(1, "id", "7").unwrap();
        assert_eq!(
            apply_edits(LF, vec![edit]).unwrap(),
            LF.replace("<item id=\"1\"", "<item id=\"7\"")
        );
        // Entities are escaped and resolved again.
        let edit = doc.set_attr(1, "name", "x<\"y\"").unwrap();
        let out = apply_edits(LF, vec![edit]).unwrap();
        let again = XmlDoc::parse(&out).unwrap();
        assert_eq!(again.attr(1, "name"), Some("x<\"y\""));
    }

    #[test]
    fn a_new_attribute_goes_after_the_last_one() {
        let doc = XmlDoc::parse(LF).unwrap();
        let out = apply_edits(LF, vec![doc.set_attr(1, "extra", "v").unwrap()]).unwrap();
        assert!(out.contains("name=\"A &amp; B\" extra=\"v\" />"));
        let out = apply_edits(LF, vec![doc.set_attr(6, "x", "1").unwrap()]).unwrap();
        assert!(out.contains("<e x=\"1\" />"));
    }

    #[test]
    fn remove_attr_takes_the_blank_before_it() {
        let doc = XmlDoc::parse(LF).unwrap();
        let out = apply_edits(LF, vec![doc.remove_attr(1, "name").unwrap()]).unwrap();
        assert!(out.contains("<item id=\"1\" />"));
        assert!(doc.remove_attr(1, "nope").is_none());
    }

    #[test]
    fn set_text_changes_only_the_text() {
        let doc = XmlDoc::parse(LF).unwrap();
        assert!(doc.set_text(5, "text").is_none());
        let out = apply_edits(LF, vec![doc.set_text(5, "a<b").unwrap()]).unwrap();
        assert!(out.contains("<t>a&lt;b</t> <!-- tail -->"));
        let out = apply_edits(LF, vec![doc.set_text(6, "now").unwrap()]).unwrap();
        assert!(out.contains("<e>now</e>\n"));
        assert!(
            doc.set_text(3, "x").is_none(),
            "an element with children has no text"
        );
    }

    #[test]
    fn remove_element_takes_its_lines() {
        let doc = XmlDoc::parse(LF).unwrap();
        let out = apply_edits(LF, vec![doc.remove_element(2)]).unwrap();
        assert_eq!(out, LF.replace("\t<item id=\"2\"/>\n", ""));
        // A multi-line element goes whole.
        let out = apply_edits(LF, vec![doc.remove_element(3)]).unwrap();
        assert_eq!(out, LF.replace("\t<box>\n\t\t<item id=\"3\" />\n\t</box>\n", ""));
        // With a comment behind it on the line, only the element goes.
        let out = apply_edits(LF, vec![doc.remove_element(5)]).unwrap();
        assert!(out.contains("\t <!-- tail -->") || out.contains("<!-- tail -->"));
        assert!(!out.contains("<t>"));
    }

    #[test]
    fn insert_after_keeps_indent_and_line_endings() {
        for text in [LF.to_owned(), crlf(LF)] {
            let doc = XmlDoc::parse(&text).unwrap();
            let eol = doc.eol();
            let edit = doc.insert_after(1, "<item id=\"9\" />");
            let out = apply_edits(&text, vec![edit]).unwrap();
            let expected = text.replace(
                &format!("A &amp; B\" />{eol}"),
                &format!("A &amp; B\" />{eol}\t<item id=\"9\" />{eol}"),
            );
            assert_eq!(out, expected);
            // Nested: the indent of the sibling.
            let edit = doc.insert_after(4, "<item id=\"10\" />");
            let out = apply_edits(&text, vec![edit]).unwrap();
            assert!(out.contains(&format!(
                "<item id=\"3\" />{eol}\t\t<item id=\"10\" />{eol}\t</box>"
            )));
        }
    }

    #[test]
    fn insert_first_child_and_append_child() {
        let doc = XmlDoc::parse(LF).unwrap();
        let out = apply_edits(LF, vec![doc.insert_first_child(3, "<item id=\"0\" />")]).unwrap();
        assert!(out.contains("<box>\n\t\t<item id=\"0\" />\n\t\t<item id=\"3\" />"));
        // An empty parent is opened up.
        let out = apply_edits(LF, vec![doc.append_child(6, "<n />")]).unwrap();
        assert!(out.contains("<e>\n\t\t<n />\n\t</e>\n"));
        // Multi-line snippets are indented per line.
        let out = apply_edits(LF, vec![doc.append_child(3, "<g>\n\t<n />\n</g>")]).unwrap();
        assert!(out.contains("\t\t<item id=\"3\" />\n\t\t<g>\n\t\t\t<n />\n\t\t</g>\n\t</box>"));
    }

    #[test]
    fn one_line_documents_stay_on_one_line() {
        let text = "<list><a/></list>";
        let doc = XmlDoc::parse(text).unwrap();
        let out = apply_edits(text, vec![doc.insert_after(1, "<b />")]).unwrap();
        assert_eq!(out, "<list><a/><b /></list>");
    }

    #[test]
    fn insertions_at_one_place_keep_their_order_and_touching_edits_work() {
        let doc = XmlDoc::parse(LF).unwrap();
        let e1 = doc.insert_after(1, "<n id=\"a\" />");
        let e2 = doc.insert_after(1, "<n id=\"b\" />");
        // Removing the next element starts where the insertions are.
        let rm = doc.remove_element(2);
        let out = apply_edits(LF, vec![e1, e2, rm]).unwrap();
        assert!(out.contains("\t<n id=\"a\" />\n\t<n id=\"b\" />\n\t<box>"));
        assert!(!out.contains("id=\"2\""));
    }

    #[test]
    fn overlapping_edits_are_refused() {
        let doc = XmlDoc::parse(LF).unwrap();
        let a = doc.remove_element(3);
        let b = doc.set_attr(4, "id", "8").unwrap();
        assert_eq!(
            apply_edits(LF, vec![a, b]),
            Err(XmlEditError::Overlap(b_start(&doc)))
        );
        fn b_start(doc: &XmlDoc) -> usize {
            doc.set_attr(4, "id", "8").unwrap().start
        }
    }

    #[test]
    fn single_quotes_and_odd_spacing_are_kept() {
        let text = "<l>\n  <a  x = 'q'   y=\"2\"/>\n</l>";
        let doc = XmlDoc::parse(text).unwrap();
        assert_eq!(doc.attr(1, "x"), Some("q"));
        assert_eq!(doc.indent_unit(), "  ");
        let out = apply_edits(text, vec![doc.set_attr(1, "x", "it's").unwrap()]).unwrap();
        assert_eq!(out, "<l>\n  <a  x = 'it&apos;s'   y=\"2\"/>\n</l>");
    }

    #[test]
    fn entity_helpers() {
        assert_eq!(
            unescape("a &amp; b &lt;&#65;&#x42;&bogus; &"),
            "a & b <AB&bogus; &"
        );
        assert_eq!(escape_text("a&b<c>"), "a&amp;b&lt;c&gt;");
        assert_eq!(escape_attr("a\"b'", '"'), "a&quot;b'");
        assert_eq!(
            element_snippet("x", &[("a".into(), "1&".into())], None),
            "<x a=\"1&amp;\" />"
        );
        assert_eq!(element_snippet("p", &[], Some("1<2")), "<p>1&lt;2</p>");
    }

    #[test]
    fn broken_xml_is_an_error() {
        assert!(XmlDoc::parse("<a><b></a>").is_err());
        assert!(XmlDoc::parse("").is_err());
        assert!(XmlDoc::parse("<a x=1/>").is_err());
    }
}
