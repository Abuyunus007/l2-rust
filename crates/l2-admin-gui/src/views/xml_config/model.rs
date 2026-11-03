//! The data model behind the XML forms (card G-08), without egui, so it is tested directly.
//!
//! Every form shows lists of elements (`<access .../>`, `<password>..</password>`). A [`Row`] is
//! one element as the user edits it: its attributes in file order, and its text. A [`RowList`]
//! is the children of one parent with a given name. [`RowList::edits`] compares the rows with
//! the file and produces the smallest set of [`l2_config::xml_edit::Edit`]s, so only what the
//! user changed is written (comments, order, tabs and line endings stay).

use std::collections::HashSet;

use l2_config::xml_edit::{Edit, XmlDoc, element_snippet};

/// One element of a list, as edited.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Row {
    /// Index of the element in the loaded file; `None` for a row the user added.
    pub orig: Option<usize>,
    /// The attributes in file order.
    pub attrs: Vec<(String, String)>,
    /// The text of an element like `<password>123456</password>`; `None` for `<a ... />`.
    pub text: Option<String>,
}

impl Row {
    pub fn from_element(doc: &XmlDoc, index: usize, with_text: bool) -> Self {
        let e = doc.element(index);
        Self {
            orig: Some(index),
            attrs: e
                .attrs
                .iter()
                .map(|a| (a.name.clone(), a.value.clone()))
                .collect(),
            text: with_text.then(|| doc.text_of(index)),
        }
    }

    /// A new row with these attributes.
    pub fn new(attrs: &[(&str, &str)]) -> Self {
        Self {
            orig: None,
            attrs: attrs
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
            text: None,
        }
    }

    /// A copy for "Copy": same values, but a new row of the file.
    pub fn duplicate(&self) -> Self {
        Self {
            orig: None,
            ..self.clone()
        }
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    pub fn get_or<'a>(&'a self, name: &str, default: &'a str) -> &'a str {
        self.get(name).unwrap_or(default)
    }

    /// Sets (or adds at the end) an attribute.
    pub fn set(&mut self, name: &str, value: &str) {
        match self.attrs.iter_mut().find(|(k, _)| k == name) {
            Some((_, v)) => *v = value.to_owned(),
            None => self.attrs.push((name.to_owned(), value.to_owned())),
        }
    }

    pub fn clear(&mut self, name: &str) {
        self.attrs.retain(|(k, _)| k != name);
    }

    /// Java `StatSet.getBoolean(name, default)`: a present value is `Boolean.parseBoolean`, so
    /// anything but "true" (any case) is false; a missing attribute is `default`.
    pub fn flag(&self, name: &str, default: bool) -> bool {
        self.get(name)
            .map_or(default, |v| v.trim().eq_ignore_ascii_case("true"))
    }

    /// Writes `true` / `false`, in the case the attribute already has (`True`), else lower case
    /// like the dist files.
    pub fn set_flag(&mut self, name: &str, on: bool) {
        let capital = self.get(name).is_some_and(|v| v.starts_with(char::is_uppercase));
        let text = match (on, capital) {
            (true, false) => "true",
            (false, false) => "false",
            (true, true) => "True",
            (false, true) => "False",
        };
        self.set(name, text);
    }

    pub fn snippet(&self, element: &str) -> String {
        element_snippet(element, &self.attrs, self.text.as_deref())
    }
}

/// The edits that make element `index` look like `row`: changed values, new attributes,
/// removed attributes, changed text.
pub fn row_edits(doc: &XmlDoc, index: usize, row: &Row) -> Vec<Edit> {
    let mut edits = Vec::new();
    for (name, value) in &row.attrs {
        edits.extend(doc.set_attr(index, name, value));
    }
    for attr in &doc.element(index).attrs {
        if !row.attrs.iter().any(|(k, _)| *k == attr.name) {
            edits.extend(doc.remove_attr(index, &attr.name));
        }
    }
    if let Some(text) = &row.text {
        edits.extend(doc.set_text(index, text));
    }
    edits
}

/// All `element` children of one parent, in file order, as edited.
#[derive(Clone, Debug, Default)]
pub struct RowList {
    pub element: String,
    pub rows: Vec<Row>,
    snapshot: Vec<Row>,
}

impl RowList {
    pub fn load(doc: &XmlDoc, parent: usize, element: &str, with_text: bool) -> Self {
        let rows: Vec<Row> = doc
            .children_named(parent, element)
            .into_iter()
            .map(|i| Row::from_element(doc, i, with_text))
            .collect();
        Self {
            element: element.to_owned(),
            snapshot: rows.clone(),
            rows,
        }
    }

    /// A list of new rows (no element of the file behind it).
    pub fn from_rows(element: &str, rows: Vec<Row>) -> Self {
        Self {
            element: element.to_owned(),
            rows,
            snapshot: Vec::new(),
        }
    }

    /// An empty list (the parent element does not exist yet).
    pub fn empty(element: &str) -> Self {
        Self {
            element: element.to_owned(),
            ..Self::default()
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.rows != self.snapshot
    }

    /// Puts the rows of the file back when it had any (so a switch turned off and on again is not
    /// a change), else makes `fresh` the only row.
    pub fn restore_or_add(&mut self, fresh: Row) {
        self.rows = if self.snapshot.is_empty() {
            vec![fresh]
        } else {
            self.snapshot.clone()
        };
    }

    #[cfg(test)]
    pub fn discard(&mut self) {
        self.rows = self.snapshot.clone();
    }

    /// Removes the elements whose row is gone, edits the kept ones, and adds the new rows after
    /// the row before them (or as first children of `parent`).
    pub fn edits(&self, doc: &XmlDoc, parent: usize) -> Vec<Edit> {
        let mut edits = Vec::new();
        let kept: HashSet<usize> = self.rows.iter().filter_map(|r| r.orig).collect();
        for element in doc.children_named(parent, &self.element) {
            if !kept.contains(&element) {
                edits.push(doc.remove_element(element));
            }
        }
        let mut anchor: Option<usize> = None;
        for row in &self.rows {
            match row.orig {
                Some(index) => {
                    edits.extend(row_edits(doc, index, row));
                    anchor = Some(index);
                }
                None => {
                    let snippet = row.snippet(&self.element);
                    edits.push(match anchor {
                        Some(a) => doc.insert_after(a, &snippet),
                        None => doc.insert_first_child(parent, &snippet),
                    });
                }
            }
        }
        edits
    }
}

/// A problem found while validating a form.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Issue {
    /// Row number in the form's main list, or `None` for a setting of the whole file.
    pub row: Option<usize>,
    /// The field (column) the message belongs to, shown next to it.
    pub field: &'static str,
    pub message: String,
    /// A warning is shown but does not block saving.
    pub warning: bool,
}

impl Issue {
    pub fn error(row: Option<usize>, field: &'static str, message: impl Into<String>) -> Self {
        Self {
            row,
            field,
            message: message.into(),
            warning: false,
        }
    }

    pub fn warning(row: Option<usize>, field: &'static str, message: impl Into<String>) -> Self {
        Self {
            warning: true,
            ..Self::error(row, field, message)
        }
    }
}

/// The messages of `issues` for one field of one row, joined.
pub fn messages_for(issues: &[Issue], row: Option<usize>, field: &str) -> Option<String> {
    let found: Vec<&str> = issues
        .iter()
        .filter(|i| i.row == row && i.field == field)
        .map(|i| i.message.as_str())
        .collect();
    (!found.is_empty()).then(|| found.join("; "))
}

pub fn has_errors(issues: &[Issue]) -> bool {
    issues.iter().any(|i| !i.warning)
}

/// Java `Integer.parseInt`: optional sign, digits only, 32-bit. (No surrounding blanks.)
pub fn parse_int(text: &str) -> Option<i32> {
    let digits = text.strip_prefix(['+', '-']).unwrap_or(text);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

/// Java `Float.parseFloat` for plain numbers: finite, optional exponent, optional `f`/`d`.
pub fn parse_float(text: &str) -> Option<f32> {
    let t = text.trim();
    let first = t.chars().next()?;
    if !(first.is_ascii_digit() || matches!(first, '+' | '-' | '.')) {
        return None; // Rust would accept "inf" and "nan"
    }
    let t = t.strip_suffix(['f', 'F', 'd', 'D']).unwrap_or(t);
    t.parse::<f32>().ok().filter(|v| v.is_finite())
}

/// A float as the shortest text that reads back the same (`1`, `2.5`).
pub fn format_float(value: f32) -> String {
    format!("{value}")
}

/// Is `text` an IPv4 address (four numbers 0-255)?
pub fn is_ipv4(text: &str) -> bool {
    text.parse::<std::net::Ipv4Addr>().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use l2_config::xml_edit::apply_edits;

    const XML: &str = "<list>\n\t<!-- first -->\n\t<a id=\"1\" on=\"true\" />\n\t<a id=\"2\" />\n\t<a id=\"3\" on=\"True\" />\n</list>\n";

    fn load() -> (XmlDoc, RowList) {
        let doc = XmlDoc::parse(XML).unwrap();
        let list = RowList::load(&doc, 0, "a", false);
        (doc, list)
    }

    #[test]
    fn untouched_rows_make_no_edits() {
        let (doc, list) = load();
        assert!(!list.is_dirty());
        assert!(list.edits(&doc, 0).is_empty());
    }

    #[test]
    fn one_change_is_one_edit() {
        let (doc, mut list) = load();
        list.rows[1].set("id", "20");
        assert!(list.is_dirty());
        let out = apply_edits(XML, list.edits(&doc, 0)).unwrap();
        assert_eq!(out, XML.replace("id=\"2\"", "id=\"20\""));
        list.discard();
        assert!(!list.is_dirty());
    }

    #[test]
    fn flags_follow_java_and_keep_their_spelling() {
        let (_, mut list) = load();
        assert!(list.rows[0].flag("on", false));
        assert!(!list.rows[1].flag("on", false));
        assert!(list.rows[1].flag("on", true), "missing: the default");
        assert!(list.rows[2].flag("on", false), "any case");
        list.rows[2].set_flag("on", false);
        assert_eq!(list.rows[2].get("on"), Some("False"));
        list.rows[0].set_flag("on", false);
        assert_eq!(list.rows[0].get("on"), Some("false"));
        list.rows[1].set_flag("on", true);
        assert_eq!(list.rows[1].get("on"), Some("true"));
        let mut r = Row::new(&[("on", "yes")]);
        assert!(!r.flag("on", true), "Boolean.parseBoolean: not 'true' is false");
        r.clear("on");
        assert!(r.attrs.is_empty());
    }

    #[test]
    fn add_copy_delete_and_remove_attribute() {
        let (doc, mut list) = load();
        let copy = list.rows[0].duplicate();
        list.rows.insert(1, copy); // after row 0
        list.rows[1].set("id", "10");
        list.rows.remove(2); // the old second row (id 2)
        list.rows[0].clear("on");
        list.rows.push(Row::new(&[("id", "99"), ("on", "false")]));
        let out = apply_edits(XML, list.edits(&doc, 0)).unwrap();
        assert_eq!(
            out,
            "<list>\n\t<!-- first -->\n\t<a id=\"1\" />\n\t<a id=\"10\" on=\"true\" />\n\t<a id=\"3\" on=\"True\" />\n\t<a id=\"99\" on=\"false\" />\n</list>\n"
        );
        // And the result parses back to the rows we edited.
        let again = RowList::load(&XmlDoc::parse(&out).unwrap(), 0, "a", false);
        let ids: Vec<&str> = again.rows.iter().map(|r| r.get_or("id", "")).collect();
        assert_eq!(ids, ["1", "10", "3", "99"]);
    }

    #[test]
    fn a_new_first_row_goes_to_the_top() {
        let (doc, mut list) = load();
        list.rows.insert(0, Row::new(&[("id", "0")]));
        let out = apply_edits(XML, list.edits(&doc, 0)).unwrap();
        assert!(out.starts_with("<list>\n\t<a id=\"0\" />\n\t<!-- first -->"));
    }

    #[test]
    fn all_rows_gone_and_new_ones_added_in_order() {
        let (doc, mut list) = load();
        list.rows.clear();
        list.rows.push(Row::new(&[("id", "7")]));
        list.rows.push(Row::new(&[("id", "8")]));
        let out = apply_edits(XML, list.edits(&doc, 0)).unwrap();
        assert_eq!(
            out,
            "<list>\n\t<a id=\"7\" />\n\t<a id=\"8\" />\n\t<!-- first -->\n</list>\n"
        );
    }

    #[test]
    fn text_rows() {
        let xml = "<l>\n\t<p>a</p>\n\t<p>b</p>\n</l>";
        let doc = XmlDoc::parse(xml).unwrap();
        let mut list = RowList::load(&doc, 0, "p", true);
        list.rows[0].text = Some("x&y".into());
        let mut new = Row::new(&[]);
        new.text = Some("c".into());
        list.rows.push(new);
        let out = apply_edits(xml, list.edits(&doc, 0)).unwrap();
        assert_eq!(out, "<l>\n\t<p>x&amp;y</p>\n\t<p>b</p>\n\t<p>c</p>\n</l>");
    }

    #[test]
    fn number_parsing_follows_java() {
        assert_eq!(parse_int("-5"), Some(-5));
        assert_eq!(parse_int("+5"), Some(5));
        assert_eq!(parse_int("2147483648"), None);
        assert_eq!(parse_int(" 5"), None);
        assert_eq!(parse_int(""), None);
        assert_eq!(parse_int("1.0"), None);
        assert_eq!(parse_float("1.5"), Some(1.5));
        assert_eq!(parse_float("2f"), Some(2.0));
        assert_eq!(parse_float("inf"), None);
        assert_eq!(parse_float("NaN"), None);
        assert_eq!(parse_float(""), None);
        assert_eq!(format_float(1.0), "1");
        assert_eq!(format_float(2.5), "2.5");
        assert!(is_ipv4("127.0.0.1") && !is_ipv4("256.1.1.1") && !is_ipv4("1.2.3"));
    }

    #[test]
    fn messages_are_found_by_row_and_field() {
        let issues = vec![
            Issue::error(Some(1), "level", "taken"),
            Issue::warning(Some(1), "level", "odd"),
            Issue::error(None, "file", "bad"),
        ];
        assert_eq!(
            messages_for(&issues, Some(1), "level").as_deref(),
            Some("taken; odd")
        );
        assert!(messages_for(&issues, Some(2), "level").is_none());
        assert_eq!(messages_for(&issues, None, "file").as_deref(), Some("bad"));
        assert!(has_errors(&issues));
        assert!(!has_errors(&issues[1..2]));
    }
}
