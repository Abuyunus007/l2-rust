//! `ClassMaster.xml`: the class change offers of the class master NPCs, with what a change costs
//! and rewards.
//!
//! Rust loader: `l2_data::ClassMasterConfig`.
//!
//! Structure: `classMaster` (attributes `classChangeEnabled`, `spawnClassMasters`,
//! `showPopupWindow`) holds `classChangeOption name=".."` elements, each with `appliesTo/category`
//! (a class group), optional `rewards/item id count` (given on a change) and the empty elements
//! `rewards/setNoble` and `rewards/setHero` (shown as two switches), and optional
//! `conditions/item id count` (the price).

use std::collections::HashSet;

use l2_config::xml_edit::{Edit, XmlDoc, escape_attr};

use super::XmlForm;
use super::model::{Issue, Row, RowList, parse_int, row_edits};
use super::widgets::{attr_cell, flag_cell, issue_list, problem_mark, text_cell};

pub const FILE: &str = "ClassMaster.xml";

const REWARD_FLAG_NAMES: [&str; 2] = [REWARD_FLAGS[0].0, REWARD_FLAGS[1].0];

/// Java `CategoryType` names, for the category drop-down.
fn category_names() -> impl Iterator<Item = &'static str> {
    l2_data::stats::CategoryType::ALL.iter().map(|c| c.name())
}

fn is_category(name: &str) -> bool {
    l2_data::stats::CategoryType::from_name(name).is_some()
}

/// The empty reward elements (`<setNoble />`, `<setHero />`) Java's `parseDocument` looks for next
/// to the reward items: the player becomes noblesse / hero on the class change.
pub const REWARD_FLAGS: [(&str, &str); 2] = [("setNoble", "noblesse"), ("setHero", "hero")];

/// A wrapper element (`appliesTo`, `rewards`, `conditions`) with its list of children.
#[derive(Clone, Debug)]
pub struct Part {
    wrapper: &'static str,
    element: &'static str,
    /// The wrapper in the file, if it has one.
    parent: Option<usize>,
    pub list: RowList,
    /// Empty marker elements next to the items, one list per name of [`REWARD_FLAGS`] (a list
    /// with a row = the element is there). Only `rewards` has them.
    flags: Vec<RowList>,
}

impl Part {
    fn load(
        doc: &XmlDoc,
        option: usize,
        wrapper: &'static str,
        element: &'static str,
        with_text: bool,
        flag_names: &[&str],
    ) -> Self {
        let parent = doc.first_child_named(option, wrapper);
        let list = match parent {
            Some(p) => RowList::load(doc, p, element, with_text),
            None => RowList::empty(element),
        };
        let flags = flag_names
            .iter()
            .map(|name| match parent {
                Some(p) => RowList::load(doc, p, name, false),
                None => RowList::empty(name),
            })
            .collect();
        Self {
            wrapper,
            element,
            parent,
            list,
            flags,
        }
    }

    /// A part with nothing in it, for a new option.
    fn blank(wrapper: &'static str, element: &'static str, flag_names: &[&str]) -> Self {
        Self {
            wrapper,
            element,
            parent: None,
            list: RowList::empty(element),
            flags: flag_names.iter().map(|name| RowList::empty(name)).collect(),
        }
    }

    /// Is the marker element `name` (`setNoble`, ...) there?
    pub fn has_flag(&self, name: &str) -> bool {
        self.flags.iter().any(|f| f.element == name && !f.rows.is_empty())
    }

    /// Adds or removes the marker element `name`.
    pub fn set_flag(&mut self, name: &str, on: bool) {
        if let Some(list) = self.flags.iter_mut().find(|f| f.element == name) {
            if on && list.rows.is_empty() {
                list.restore_or_add(Row::new(&[]));
            } else if !on {
                list.rows.clear();
            }
        }
    }

    fn is_empty(&self) -> bool {
        self.list.rows.is_empty() && self.flags.iter().all(|f| f.rows.is_empty())
    }

    fn duplicate(&self) -> Self {
        Self {
            parent: None,
            list: RowList::from_rows(self.element, self.list.rows.iter().map(Row::duplicate).collect()),
            flags: self
                .flags
                .iter()
                .map(|f| RowList::from_rows(&f.element, f.rows.iter().map(Row::duplicate).collect()))
                .collect(),
            ..self.clone()
        }
    }

    fn is_dirty(&self) -> bool {
        self.list.is_dirty() || self.flags.iter().any(RowList::is_dirty)
    }

    /// The wrapper with its rows, one string per line, relative indentation by `unit`.
    fn lines(&self, unit: &str) -> Vec<String> {
        let mut lines = vec![format!("<{}>", self.wrapper)];
        lines.extend(
            self.list
                .rows
                .iter()
                .map(|r| format!("{unit}{}", r.snippet(self.element))),
        );
        for flag in &self.flags {
            lines.extend(
                flag.rows
                    .iter()
                    .map(|r| format!("{unit}{}", r.snippet(&flag.element))),
            );
        }
        lines.push(format!("</{}>", self.wrapper));
        lines
    }

    fn edits(&self, doc: &XmlDoc, option: usize, unit: &str) -> Vec<Edit> {
        match self.parent {
            Some(p) => {
                let mut edits = self.list.edits(doc, p);
                for flag in &self.flags {
                    edits.extend(flag.edits(doc, p));
                }
                edits
            }
            None if !self.is_empty() => {
                vec![doc.append_child(option, &self.lines(unit).join("\n"))]
            }
            None => Vec::new(),
        }
    }
}

/// One `classChangeOption`.
#[derive(Clone, Debug)]
pub struct OptionModel {
    pub orig: Option<usize>,
    /// Holds the `name` attribute.
    pub head: Row,
    head_snapshot: Row,
    pub categories: Part,
    pub rewards: Part,
    pub conditions: Part,
}

impl OptionModel {
    fn load(doc: &XmlDoc, index: usize) -> Self {
        let head = Row::from_element(doc, index, false);
        Self {
            orig: Some(index),
            head_snapshot: head.clone(),
            head,
            categories: Part::load(doc, index, "appliesTo", "category", true, &[]),
            rewards: Part::load(doc, index, "rewards", "item", false, &REWARD_FLAG_NAMES),
            conditions: Part::load(doc, index, "conditions", "item", false, &[]),
        }
    }

    /// A new option with one class group and nothing to pay.
    pub fn new(name: &str, category: &str) -> Self {
        let mut cat = Row::new(&[]);
        cat.text = Some(category.to_owned());
        Self {
            orig: None,
            head: Row::new(&[("name", name)]),
            head_snapshot: Row::default(),
            categories: Part {
                list: RowList::from_rows("category", vec![cat]),
                ..Part::blank("appliesTo", "category", &[])
            },
            rewards: Part::blank("rewards", "item", &REWARD_FLAG_NAMES),
            conditions: Part::blank("conditions", "item", &[]),
        }
    }

    /// A copy as a new option of the file.
    pub fn duplicate(&self) -> Self {
        let mut copy = Self {
            orig: None,
            head: self.head.duplicate(),
            head_snapshot: Row::default(),
            categories: self.categories.duplicate(),
            rewards: self.rewards.duplicate(),
            conditions: self.conditions.duplicate(),
        };
        let name = format!("{} (copy)", self.name());
        copy.head.set("name", &name);
        copy
    }

    pub fn name(&self) -> &str {
        self.head.get_or("name", "")
    }

    fn is_dirty(&self) -> bool {
        self.head != self.head_snapshot
            || self.categories.is_dirty()
            || self.rewards.is_dirty()
            || self.conditions.is_dirty()
    }

    fn edits(&self, doc: &XmlDoc, index: usize, unit: &str) -> Vec<Edit> {
        let mut edits = row_edits(doc, index, &self.head);
        for part in [&self.categories, &self.rewards, &self.conditions] {
            edits.extend(part.edits(doc, index, unit));
        }
        edits
    }

    fn snippet(&self, unit: &str) -> String {
        let mut lines = vec![format!(
            "<classChangeOption name=\"{}\">",
            escape_attr(self.name(), '"')
        )];
        for part in [&self.categories, &self.rewards, &self.conditions] {
            if !part.is_empty() {
                lines.extend(part.lines(unit).into_iter().map(|l| format!("{unit}{l}")));
            }
        }
        lines.push("</classChangeOption>".to_owned());
        lines.join("\n")
    }
}

pub struct ClassMasterForm {
    /// The `classMaster` element; `None` when the file has none (nothing to edit).
    cm: Option<usize>,
    settings: Row,
    settings_snapshot: Row,
    pub options: Vec<OptionModel>,
    snapshot_origs: Vec<Option<usize>>,
}

impl ClassMasterForm {
    pub fn load(text: &str) -> Result<Self, String> {
        let doc = XmlDoc::parse(text).map_err(|e| e.to_string())?;
        // Java reads every `classMaster`; the form edits the first (the file has one).
        let cm = doc.first_child_named(doc.root(), "classMaster");
        let (settings, options) = match cm {
            Some(cm) => (
                Row::from_element(&doc, cm, false),
                doc.children_named(cm, "classChangeOption")
                    .into_iter()
                    .map(|i| OptionModel::load(&doc, i))
                    .collect::<Vec<_>>(),
            ),
            None => (Row::default(), Vec::new()),
        };
        Ok(Self {
            cm,
            settings_snapshot: settings.clone(),
            snapshot_origs: options.iter().map(|o| o.orig).collect(),
            settings,
            options,
        })
    }

    #[cfg(test)]
    pub fn settings_mut(&mut self) -> &mut Row {
        &mut self.settings
    }

    fn option_label(&self, index: usize) -> String {
        match self.options.get(index) {
            Some(o) => format!("Option \"{}\"", o.name()),
            None => format!("Option {}", index + 1),
        }
    }
}

impl XmlForm for ClassMasterForm {
    fn is_dirty(&self) -> bool {
        self.settings != self.settings_snapshot
            || self.options.iter().map(|o| o.orig).collect::<Vec<_>>() != self.snapshot_origs
            || self.options.iter().any(OptionModel::is_dirty)
    }

    fn issues(&self) -> Vec<Issue> {
        let mut issues = Vec::new();
        for (i, option) in self.options.iter().enumerate() {
            let row = Some(i);
            if option.name().trim().is_empty() {
                issues.push(Issue::warning(row, "name", "the option has no name"));
            }
            if option.categories.list.rows.is_empty() {
                // The Rust and the Java loader both skip an option without a class group.
                issues.push(Issue::warning(
                    row,
                    "category",
                    "no class group: the option is skipped",
                ));
            }
            for (n, r) in option.categories.list.rows.iter().enumerate() {
                let name = r.text.as_deref().unwrap_or("").trim();
                if !is_category(name) {
                    issues.push(Issue::error(
                        row,
                        "category",
                        format!("class group {}: '{name}' is not a known group", n + 1),
                    ));
                }
            }
            for (parts, field, what) in [
                (&option.rewards, "reward", "reward item"),
                (&option.conditions, "cost", "cost item"),
            ] {
                for (n, r) in parts.list.rows.iter().enumerate() {
                    match r.get("id").and_then(parse_int) {
                        Some(id) if id > 0 => {}
                        _ => issues.push(Issue::error(
                            row,
                            field,
                            format!("{what} {}: the id must be an item id", n + 1),
                        )),
                    }
                    // `count` is optional (Java default 1).
                    if let Some(count) = r.get("count") {
                        match parse_int(count) {
                            Some(c) if c >= 1 => {}
                            _ => issues.push(Issue::error(
                                row,
                                field,
                                format!("{what} {}: the count must be 1 or more", n + 1),
                            )),
                        }
                    }
                }
            }
        }
        issues
    }

    fn edits(&self, doc: &XmlDoc) -> Vec<Edit> {
        let Some(cm) = self.cm else { return Vec::new() };
        let unit = doc.indent_unit();
        let mut edits = row_edits(doc, cm, &self.settings);
        let kept: HashSet<usize> = self.options.iter().filter_map(|o| o.orig).collect();
        for element in doc.children_named(cm, "classChangeOption") {
            if !kept.contains(&element) {
                edits.push(doc.remove_element(element));
            }
        }
        let mut anchor: Option<usize> = None;
        for option in &self.options {
            match option.orig {
                Some(index) => {
                    edits.extend(option.edits(doc, index, &unit));
                    anchor = Some(index);
                }
                None => {
                    let snippet = option.snippet(&unit);
                    edits.push(match anchor {
                        Some(a) => doc.insert_after(a, &snippet),
                        None => doc.insert_first_child(cm, &snippet),
                    });
                }
            }
        }
        edits
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        let issues = self.issues();
        issue_list(ui, &issues, &|r| self.option_label(r));
        if self.cm.is_none() {
            ui.label("The file has no <classMaster> element, so there is nothing to edit.");
            return;
        }
        egui::Grid::new("class_master_settings")
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label("Class change enabled");
                flag_cell(ui, &mut self.settings, "classChangeEnabled", false);
                ui.weak("Off: the server reads no option at all.");
                ui.end_row();
                ui.label("Spawn class masters");
                flag_cell(ui, &mut self.settings, "spawnClassMasters", true);
                ui.weak("Puts Mr. Cat and Miss Queen in the towns.");
                ui.end_row();
                ui.label("Show popup window");
                flag_cell(ui, &mut self.settings, "showPopupWindow", false);
                ui.weak("Offers the class change when a player reaches the level.");
                ui.end_row();
            });
        ui.separator();
        ui.horizontal(|ui| {
            ui.strong("Class change options");
            if ui.button("Add option").clicked() {
                let n = self.options.len() + 1;
                self.options
                    .push(OptionModel::new(&format!("New option {n}"), "FIRST_CLASS_GROUP"));
            }
        });
        let mut action: Option<(usize, bool)> = None; // (option, delete)
        egui::ScrollArea::vertical()
            .id_salt("class_master_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for i in 0..self.options.len() {
                    let title = format!("{}. {}", i + 1, self.options[i].name());
                    egui::CollapsingHeader::new(title)
                        .id_salt(("cm_option", i))
                        .default_open(false)
                        .show(ui, |ui| {
                            let option = &mut self.options[i];
                            ui.horizontal(|ui| {
                                ui.label("Name:");
                                attr_cell(ui, &mut option.head, "name", 260.0, false, false);
                                problem_mark(ui, &issues, Some(i), "name");
                                if ui.small_button("Copy").clicked() {
                                    action = Some((i, false));
                                }
                                if ui.small_button("Delete").clicked() {
                                    action = Some((i, true));
                                }
                            });
                            draw_categories(ui, i, &mut option.categories.list);
                            problem_mark(ui, &issues, Some(i), "category");
                            draw_items(
                                ui,
                                ("rewards", i),
                                "Rewards (given on the change)",
                                &mut option.rewards.list,
                            );
                            draw_reward_flags(ui, &mut option.rewards);
                            problem_mark(ui, &issues, Some(i), "reward");
                            draw_items(
                                ui,
                                ("costs", i),
                                "Costs (taken on the change)",
                                &mut option.conditions.list,
                            );
                            problem_mark(ui, &issues, Some(i), "cost");
                        });
                }
            });
        match action {
            Some((i, true)) => {
                self.options.remove(i);
            }
            Some((i, false)) => {
                let copy = self.options[i].duplicate();
                self.options.insert(i + 1, copy);
            }
            None => {}
        }
    }
}

fn draw_categories(ui: &mut egui::Ui, option: usize, list: &mut RowList) {
    ui.label("Applies to (class groups):");
    let mut remove: Option<usize> = None;
    for (n, row) in list.rows.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            let current = row.text.clone().unwrap_or_default();
            egui::ComboBox::from_id_salt(("cm_category", option, n))
                .selected_text(current.trim().to_owned())
                .width(240.0)
                .show_ui(ui, |ui| {
                    for name in category_names() {
                        if ui.selectable_label(current.trim() == name, name).clicked() {
                            row.text = Some(name.to_owned());
                        }
                    }
                });
            if ui.small_button("x").on_hover_text("Remove").clicked() {
                remove = Some(n);
            }
        });
    }
    if let Some(n) = remove {
        list.rows.remove(n);
    }
    if ui.small_button("Add class group").clicked() {
        let mut row = Row::new(&[]);
        row.text = Some("FIRST_CLASS_GROUP".to_owned());
        list.rows.push(row);
    }
}

/// The switches for `<setNoble />` and `<setHero />` of the rewards.
fn draw_reward_flags(ui: &mut egui::Ui, rewards: &mut Part) {
    ui.horizontal(|ui| {
        for (name, label) in REWARD_FLAGS {
            let mut on = rewards.has_flag(name);
            if ui
                .checkbox(&mut on, format!("Makes the player {label}"))
                .on_hover_text(format!("<{name} /> in the rewards"))
                .changed()
            {
                rewards.set_flag(name, on);
            }
        }
    });
}

fn draw_items(ui: &mut egui::Ui, id: (&str, usize), title: &str, list: &mut RowList) {
    ui.label(title);
    let mut remove: Option<usize> = None;
    if !list.rows.is_empty() {
        egui::Grid::new(("cm_items", id.0, id.1))
            .spacing([10.0, 3.0])
            .show(ui, |ui| {
                ui.strong("Item id");
                ui.strong("Count");
                ui.label("");
                ui.end_row();
                for (n, row) in list.rows.iter_mut().enumerate() {
                    attr_cell(ui, row, "id", 70.0, false, false);
                    // `count` is optional and means 1 when missing; typing creates it.
                    let mut count = row.get_or("count", "1").to_owned();
                    if text_cell(ui, &mut count, 90.0, false).changed() {
                        row.set("count", &count);
                    }
                    if ui.small_button("x").on_hover_text("Remove").clicked() {
                        remove = Some(n);
                    }
                    ui.end_row();
                }
            });
    }
    if let Some(n) = remove {
        list.rows.remove(n);
    }
    if ui.small_button("Add item").clicked() {
        list.rows.push(Row::new(&[("id", "57"), ("count", "1")]));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use l2_config::xml_edit::apply_edits;

    const XML: &str = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\r\n<list>\r\n\t<classMaster classChangeEnabled=\"false\" spawnClassMasters=\"false\" showPopupWindow=\"false\">\r\n\t\t<classChangeOption name=\"Free\">\r\n\t\t\t<appliesTo>\r\n\t\t\t\t<category>FIRST_CLASS_GROUP</category>\r\n\t\t\t</appliesTo>\r\n\t\t</classChangeOption>\r\n\t\t<classChangeOption name=\"1,000,000 Adena\">\r\n\t\t\t<appliesTo>\r\n\t\t\t\t<category>SECOND_CLASS_GROUP</category>\r\n\t\t\t</appliesTo>\r\n\t\t\t<conditions>\r\n\t\t\t\t<item id=\"57\" count=\"1000000\" />\r\n\t\t\t</conditions>\r\n\t\t</classChangeOption>\r\n\t\t<!-- Example:\r\n\t\t<classChangeOption name=\"x\">\r\n\t\t</classChangeOption> -->\r\n\t</classMaster>\r\n</list>\r\n";

    fn save(f: &ClassMasterForm) -> String {
        apply_edits(XML, f.edits(&XmlDoc::parse(XML).unwrap())).unwrap()
    }

    #[test]
    fn unchanged_is_identical() {
        let f = ClassMasterForm::load(XML).unwrap();
        assert!(!f.is_dirty() && f.issues().is_empty());
        assert_eq!(f.options.len(), 2, "the commented example is not an option");
        assert_eq!(save(&f), XML);
    }

    #[test]
    fn a_flag_touches_one_attribute() {
        let mut f = ClassMasterForm::load(XML).unwrap();
        f.settings_mut().set_flag("classChangeEnabled", true);
        assert_eq!(
            save(&f),
            XML.replace("classChangeEnabled=\"false\"", "classChangeEnabled=\"true\"")
        );
    }

    #[test]
    fn the_price_changes_in_place() {
        let mut f = ClassMasterForm::load(XML).unwrap();
        f.options[1].conditions.list.rows[0].set("count", "2000000");
        assert_eq!(save(&f), XML.replace("1000000\" />", "2000000\" />"));
    }

    #[test]
    fn a_reward_creates_its_wrapper() {
        let mut f = ClassMasterForm::load(XML).unwrap();
        f.options[0]
            .rewards
            .list
            .rows
            .push(Row::new(&[("id", "6622"), ("count", "1")]));
        let out = save(&f);
        assert!(out.contains("\t\t\t</appliesTo>\r\n\t\t\t<rewards>\r\n\t\t\t\t<item id=\"6622\" count=\"1\" />\r\n\t\t\t</rewards>\r\n\t\t</classChangeOption>"), "{out}");
        assert!(!out.replace("\r\n", "").contains('\n'));
        // The Rust loader reads it.
        let enabled = out.replace("classChangeEnabled=\"false\"", "classChangeEnabled=\"true\"");
        let config = l2_data::ClassMasterConfig::from_xml(&enabled).unwrap();
        assert_eq!(config.class_change_data[0].items_rewarded.len(), 1);
        assert_eq!(config.class_change_data[0].items_rewarded[0].id, 6622);
    }

    #[test]
    fn the_noblesse_and_hero_switches_create_the_rewards_wrapper() {
        let mut f = ClassMasterForm::load(XML).unwrap();
        assert!(!f.options[0].rewards.has_flag("setNoble") && !f.options[0].rewards.has_flag("setHero"));
        f.options[0].rewards.set_flag("setNoble", true);
        f.options[0].rewards.set_flag("setHero", true);
        assert!(f.is_dirty());
        let out = save(&f);
        assert!(out.contains("\t\t\t</appliesTo>\r\n\t\t\t<rewards>\r\n\t\t\t\t<setNoble />\r\n\t\t\t\t<setHero />\r\n\t\t\t</rewards>\r\n\t\t</classChangeOption>"), "{out}");
        assert!(!out.replace("\r\n", "").contains('\n'));
        // The Rust loader reads both.
        let enabled = out.replace("classChangeEnabled=\"false\"", "classChangeEnabled=\"true\"");
        let config = l2_data::ClassMasterConfig::from_xml(&enabled).unwrap();
        assert!(config.class_change_data[0].reward_noblesse && config.class_change_data[0].reward_hero);
        assert!(!config.class_change_data[1].reward_noblesse);
    }

    const WITH_REWARDS: &str = "<list>\n\t<classMaster classChangeEnabled=\"true\">\n\t\t<classChangeOption name=\"A\">\n\t\t\t<appliesTo>\n\t\t\t\t<category>FIRST_CLASS_GROUP</category>\n\t\t\t</appliesTo>\n\t\t\t<rewards>\n\t\t\t\t<item id=\"6622\" count=\"1\" />\n\t\t\t\t<setNoble />\n\t\t\t</rewards>\n\t\t</classChangeOption>\n\t</classMaster>\n</list>\n";

    fn save_rewards(f: &ClassMasterForm) -> String {
        apply_edits(WITH_REWARDS, f.edits(&XmlDoc::parse(WITH_REWARDS).unwrap())).unwrap()
    }

    #[test]
    fn the_switches_show_what_the_file_has_and_change_only_their_line() {
        let mut f = ClassMasterForm::load(WITH_REWARDS).unwrap();
        assert!(f.options[0].rewards.has_flag("setNoble") && !f.options[0].rewards.has_flag("setHero"));
        assert!(!f.is_dirty());
        assert_eq!(save_rewards(&f), WITH_REWARDS);

        // Off: its line goes, the item stays.
        f.options[0].rewards.set_flag("setNoble", false);
        assert_eq!(
            save_rewards(&f),
            WITH_REWARDS.replace("\t\t\t\t<setNoble />\n", "")
        );
        // Switching an element on that is on already adds no second one.
        f.options[0].rewards.set_flag("setNoble", true);
        f.options[0].rewards.set_flag("setNoble", true);
        assert_eq!(save_rewards(&f), WITH_REWARDS);

        // The hero switch adds one element to the existing wrapper.
        f.options[0].rewards.set_flag("setHero", true);
        let out = save_rewards(&f);
        assert_eq!(out.matches("<setHero />").count(), 1);
        assert_eq!(out.matches("<setNoble />").count(), 1);
        assert_eq!(out.matches("<item ").count(), 1);
        let config = l2_data::ClassMasterConfig::from_xml(&out).unwrap();
        assert!(config.class_change_data[0].reward_noblesse && config.class_change_data[0].reward_hero);
        assert_eq!(config.class_change_data[0].items_rewarded.len(), 1);
    }

    #[test]
    fn a_copy_and_a_new_option_carry_the_switches() {
        let mut f = ClassMasterForm::load(WITH_REWARDS).unwrap();
        let copy = f.options[0].duplicate();
        assert!(copy.rewards.has_flag("setNoble"));
        f.options.push(copy);
        let mut fresh = OptionModel::new("Fresh", "SECOND_CLASS_GROUP");
        fresh.rewards.set_flag("setHero", true);
        f.options.push(fresh);
        let config = l2_data::ClassMasterConfig::from_xml(&save_rewards(&f)).unwrap();
        let flags: Vec<(bool, bool)> = config
            .class_change_data
            .iter()
            .map(|d| (d.reward_noblesse, d.reward_hero))
            .collect();
        assert_eq!(flags, [(true, false), (true, false), (false, true)]);
    }

    #[test]
    fn options_are_added_copied_and_deleted() {
        let mut f = ClassMasterForm::load(XML).unwrap();
        f.options.push(OptionModel::new("Third", "THIRD_CLASS_GROUP"));
        let copy = f.options[1].duplicate();
        f.options.insert(2, copy);
        f.options.remove(0);
        f.options[2]
            .conditions
            .list
            .rows
            .push(Row::new(&[("id", "5575"), ("count", "300000")]));
        let out = save(&f);
        assert!(!out.contains("name=\"Free\""));
        assert!(out.contains("<classChangeOption name=\"1,000,000 Adena (copy)\">"));
        assert!(out.contains("\t\t<classChangeOption name=\"Third\">\r\n\t\t\t<appliesTo>\r\n\t\t\t\t<category>THIRD_CLASS_GROUP</category>\r\n\t\t\t</appliesTo>\r\n\t\t\t<conditions>\r\n\t\t\t\t<item id=\"5575\" count=\"300000\" />\r\n\t\t\t</conditions>\r\n\t\t</classChangeOption>\r\n"));
        assert!(out.contains("<!-- Example:"));
        // Everything still loads, with the options in the new order.
        let enabled = out.replace("classChangeEnabled=\"false\"", "classChangeEnabled=\"true\"");
        let config = l2_data::ClassMasterConfig::from_xml(&enabled).unwrap();
        let names: Vec<&str> = config.class_change_data.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, ["1,000,000 Adena", "1,000,000 Adena (copy)", "Third"]);
        assert_eq!(config.class_change_data[2].items_required[0].count, 300_000);
    }

    #[test]
    fn validation() {
        let mut f = ClassMasterForm::load(XML).unwrap();
        f.options[0].categories.list.rows[0].text = Some("NOT_A_GROUP".into());
        f.options[1].conditions.list.rows[0].set("id", "0");
        f.options[1].conditions.list.rows[0].set("count", "x");
        f.options[1].categories.list.rows.clear();
        let issues = f.issues();
        let has = |row, field, warning| {
            issues
                .iter()
                .any(|i| i.row == Some(row) && i.field == field && i.warning == warning)
        };
        assert!(has(0, "category", false));
        assert!(has(1, "cost", false));
        assert!(has(1, "category", true), "no group: skipped, a warning");
        assert_eq!(issues.iter().filter(|i| i.field == "cost").count(), 2);
        assert!(is_category("FIRST_CLASS_GROUP") && !is_category("first_class_group"));
    }
}
