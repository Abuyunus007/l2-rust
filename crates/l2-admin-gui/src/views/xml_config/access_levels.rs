//! `AccessLevels.xml`: the GM levels, one row each, the permission flags as switches.
//!
//! their defaults). Rust loader: `game-server/src/world_data.rs` (`AccessLevels::from_xml`).

use std::collections::HashMap;

use l2_config::xml_edit::{Edit, XmlDoc};

use super::XmlForm;
use super::model::{Issue, Row, RowList, parse_int};
use super::widgets::{attr_cell, color_cell, flag_cell, issue_list, problem_mark};

pub const FILE: &str = "AccessLevels.xml";

/// A permission flag: attribute, short header, Java default, tooltip.
pub struct Flag {
    pub attr: &'static str,
    pub label: &'static str,
    pub default: bool,
    pub help: &'static str,
}

/// The flags of Java `AccessLevel`, with the defaults of its constructor.
pub const FLAGS: [Flag; 8] = [
    Flag {
        attr: "isGM",
        label: "GM",
        default: false,
        help: "isGM: the level counts as a game master",
    },
    Flag {
        attr: "allowPeaceAttack",
        label: "Peace atk",
        default: false,
        help: "allowPeaceAttack: may attack in peace zones",
    },
    Flag {
        attr: "allowFixedRes",
        label: "Fixed res",
        default: false,
        help: "allowFixedRes: may resurrect with a fixed resurrection",
    },
    Flag {
        attr: "allowTransaction",
        label: "Trade",
        default: true,
        help: "allowTransaction: may trade and use stores",
    },
    Flag {
        attr: "allowAltg",
        label: "Alt+G",
        default: false,
        help: "allowAltg: may use Alt+G commands",
    },
    Flag {
        attr: "giveDamage",
        label: "Damage",
        default: true,
        help: "giveDamage: deals damage",
    },
    Flag {
        attr: "takeAggro",
        label: "Aggro",
        default: true,
        help: "takeAggro: monsters attack this level",
    },
    Flag {
        attr: "gainExp",
        label: "Exp",
        default: true,
        help: "gainExp: gains experience",
    },
];

const KNOWN: [&str; 5] = ["level", "name", "nameColor", "titleColor", "childAccess"];

/// Java `Integer.decode("0x" + value)`: 1 to 8 hex digits that fit a signed int.
pub fn valid_color(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|b| b.is_ascii_hexdigit()) && i32::from_str_radix(text, 16).is_ok()
}

pub struct AccessLevelsForm {
    list: RowList,
}

impl AccessLevelsForm {
    pub fn load(text: &str) -> Result<Self, String> {
        let doc = XmlDoc::parse(text).map_err(|e| e.to_string())?;
        Ok(Self {
            list: RowList::load(&doc, doc.root(), "access", false),
        })
    }

    #[cfg(test)]
    pub fn rows(&self) -> &[Row] {
        &self.list.rows
    }

    #[cfg(test)]
    pub fn rows_mut(&mut self) -> &mut Vec<Row> {
        &mut self.list.rows
    }

    /// (level, name) of every row with a valid level, sorted: the drop-down of the other forms.
    pub fn level_list(&self) -> Vec<(i32, String)> {
        let mut levels: Vec<(i32, String)> = self
            .list
            .rows
            .iter()
            .filter_map(|r| Some((parse_int(r.get("level")?)?, r.get_or("name", "").to_owned())))
            .collect();
        levels.sort();
        levels.dedup_by_key(|l| l.0);
        levels
    }

    fn used_levels(&self) -> Vec<i32> {
        self.list
            .rows
            .iter()
            .filter_map(|r| parse_int(r.get("level")?))
            .collect()
    }

    /// The first level number from `from` up that no row uses.
    pub fn next_free_level(&self, from: i32) -> i32 {
        let used = self.used_levels();
        (from..=i32::MAX).find(|l| !used.contains(l)).unwrap_or(from)
    }

    /// Adds a plain level at the end, with the Java defaults written out.
    pub fn add_level(&mut self) {
        let max = self.used_levels().into_iter().max().unwrap_or(0);
        let level = self.next_free_level(max.saturating_add(10));
        let mut row = Row::new(&[
            ("level", &level.to_string()),
            ("name", "New level"),
            ("nameColor", "FFFFFF"),
            ("titleColor", "ECF9A2"),
            ("childAccess", "0"),
        ]);
        for flag in &FLAGS {
            row.set_flag(flag.attr, flag.default);
        }
        self.list.rows.push(row);
    }

    /// Copies row `index` below itself with the next free level number.
    pub fn copy_level(&mut self, index: usize) {
        let Some(source) = self.list.rows.get(index) else {
            return;
        };
        let mut row = source.duplicate();
        let from = parse_int(source.get_or("level", "0"))
            .unwrap_or(0)
            .saturating_add(1);
        row.set("level", &self.next_free_level(from).to_string());
        row.set("name", &format!("{} (copy)", source.get_or("name", "")));
        self.list.rows.insert(index + 1, row);
    }

    pub fn delete_level(&mut self, index: usize) {
        if index < self.list.rows.len() {
            self.list.rows.remove(index);
        }
    }

    fn row_label(&self, index: usize) -> String {
        match self.list.rows.get(index) {
            Some(r) => format!("Level {} ({})", r.get_or("level", "?"), r.get_or("name", "")),
            None => format!("Row {}", index + 1),
        }
    }
}

impl XmlForm for AccessLevelsForm {
    fn is_dirty(&self) -> bool {
        self.list.is_dirty()
    }

    fn issues(&self) -> Vec<Issue> {
        let mut issues = Vec::new();
        let mut seen: HashMap<i32, usize> = HashMap::new();
        for r in &self.list.rows {
            if let Some(level) = r.get("level").and_then(parse_int) {
                *seen.entry(level).or_default() += 1;
            }
        }
        let levels: Vec<i32> = seen.keys().copied().collect();
        for (i, r) in self.list.rows.iter().enumerate() {
            let row = Some(i);
            match r.get("level").map(|v| (v, parse_int(v))) {
                None | Some((_, None)) => {
                    issues.push(Issue::error(row, "level", "the level must be a whole number"));
                }
                Some((_, Some(level))) if seen.get(&level).copied().unwrap_or(0) > 1 => {
                    issues.push(Issue::error(row, "level", format!("level {level} is used twice")));
                }
                Some(_) => {}
            }
            if r.get_or("name", "").trim().is_empty() {
                issues.push(Issue::error(row, "name", "the name must not be empty"));
            }
            for color in ["nameColor", "titleColor"] {
                if let Some(v) = r.get(color)
                    && !valid_color(v)
                {
                    issues.push(Issue::error(row, color, "1 to 8 hex digits (BBGGRR)"));
                }
            }
            if let Some(v) = r.get("childAccess") {
                match parse_int(v) {
                    None => issues.push(Issue::error(row, "childAccess", "must be a whole number")),
                    Some(0) => {}
                    Some(child) if !levels.contains(&child) => {
                        issues.push(Issue::warning(
                            row,
                            "childAccess",
                            format!("level {child} does not exist"),
                        ));
                    }
                    Some(_) => {}
                }
            }
        }
        if !seen.contains_key(&-1) {
            issues.push(Issue::warning(
                None,
                "file",
                "Level -1 (banned) is missing; Java then fails for banned accounts",
            ));
        }
        issues
    }

    fn edits(&self, doc: &XmlDoc) -> Vec<Edit> {
        self.list.edits(doc, doc.root())
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        let issues = self.issues();
        issue_list(ui, &issues, &|r| self.row_label(r));
        ui.horizontal(|ui| {
            if ui.button("Add level").clicked() {
                self.add_level();
            }
            ui.weak("Name and title colours are BBGGRR hex digits, as the client reads them.");
        });
        let level_names = self.level_list();
        let mut action: Option<(usize, bool)> = None; // (row, delete)
        egui::ScrollArea::both()
            .id_salt("access_levels_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("access_levels_grid")
                    .striped(true)
                    .spacing([8.0, 4.0])
                    .show(ui, |ui| {
                        for head in ["Level", "Name", "Name colour", "Title colour", "Child"] {
                            ui.strong(head);
                        }
                        for flag in &FLAGS {
                            ui.strong(flag.label).on_hover_text(flag.help);
                        }
                        ui.strong("Other attributes");
                        ui.label("");
                        ui.end_row();
                        for i in 0..self.list.rows.len() {
                            let row = &mut self.list.rows[i];
                            let bad =
                                |field: &str| super::model::messages_for(&issues, Some(i), field).is_some();
                            ui.horizontal(|ui| {
                                attr_cell(ui, row, "level", 50.0, false, bad("level"));
                                problem_mark(ui, &issues, Some(i), "level");
                            });
                            ui.horizontal(|ui| {
                                attr_cell(ui, row, "name", 130.0, false, bad("name"));
                                problem_mark(ui, &issues, Some(i), "name");
                            });
                            for field in ["nameColor", "titleColor"] {
                                let mut hex = row.get_or(field, "").to_owned();
                                ui.horizontal(|ui| {
                                    if color_cell(ui, &mut hex, bad(field)) {
                                        row.set(field, &hex);
                                    }
                                    problem_mark(ui, &issues, Some(i), field);
                                });
                            }
                            ui.horizontal(|ui| {
                                // A drop-down of the existing levels, and the number itself.
                                attr_cell(ui, row, "childAccess", 40.0, true, bad("childAccess"));
                                egui::ComboBox::from_id_salt(("child", i))
                                    .selected_text("")
                                    .width(14.0)
                                    .show_ui(ui, |ui| {
                                        for (level, name) in &level_names {
                                            if ui.selectable_label(false, format!("{level} {name}")).clicked()
                                            {
                                                row.set("childAccess", &level.to_string());
                                            }
                                        }
                                    });
                                problem_mark(ui, &issues, Some(i), "childAccess");
                            });
                            for flag in &FLAGS {
                                flag_cell(ui, row, flag.attr, flag.default);
                            }
                            ui.horizontal(|ui| {
                                let extra: Vec<String> = row
                                    .attrs
                                    .iter()
                                    .map(|(k, _)| k.clone())
                                    .filter(|k| {
                                        !KNOWN.contains(&k.as_str()) && !FLAGS.iter().any(|f| f.attr == k)
                                    })
                                    .collect();
                                for key in extra {
                                    ui.label(format!("{key}:"));
                                    attr_cell(ui, row, &key, 80.0, false, false);
                                }
                            });
                            ui.horizontal(|ui| {
                                if ui
                                    .small_button("Copy")
                                    .on_hover_text("Copy below with a free level")
                                    .clicked()
                                {
                                    action = Some((i, false));
                                }
                                if ui.small_button("Delete").clicked() {
                                    action = Some((i, true));
                                }
                            });
                            ui.end_row();
                        }
                    });
            });
        match action {
            Some((i, true)) => self.delete_level(i),
            Some((i, false)) => self.copy_level(i),
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::model::has_errors;
    use super::*;
    use l2_config::xml_edit::apply_edits;

    const XML: &str = "<?xml version=\"1.0\"?>\r\n<list>\r\n\t<!-- Access levels -->\r\n\t<access level=\"-1\" name=\"Banned\" nameColor=\"FFFFFF\" titleColor=\"ECF9A2\" isGM=\"false\" allowTransaction=\"false\" />\r\n\t<access level=\"0\" name=\"User\" nameColor=\"FFFFFF\" titleColor=\"ECF9A2\" childAccess=\"0\" isGM=\"false\" />\r\n\t<access level=\"100\" name=\"Master\" nameColor=\"00CCFF\" titleColor=\"00CCFF\" childAccess=\"0\" isGM=\"true\" />\r\n</list>\r\n";

    fn save(form: &AccessLevelsForm) -> String {
        let doc = XmlDoc::parse(XML).unwrap();
        apply_edits(XML, form.edits(&doc)).unwrap()
    }

    #[test]
    fn nothing_changed_nothing_written() {
        let form = AccessLevelsForm::load(XML).unwrap();
        assert!(!form.is_dirty());
        assert_eq!(save(&form), XML);
        assert!(form.issues().is_empty());
    }

    #[test]
    fn one_switch_touches_one_attribute() {
        let mut form = AccessLevelsForm::load(XML).unwrap();
        form.rows_mut()[1].set_flag("isGM", true);
        assert_eq!(
            save(&form),
            XML.replace(
                "name=\"User\" nameColor=\"FFFFFF\" titleColor=\"ECF9A2\" childAccess=\"0\" isGM=\"false\"",
                "name=\"User\" nameColor=\"FFFFFF\" titleColor=\"ECF9A2\" childAccess=\"0\" isGM=\"true\""
            )
        );
    }

    #[test]
    fn flags_use_the_java_defaults_when_the_attribute_is_missing() {
        let form = AccessLevelsForm::load(XML).unwrap();
        let banned = &form.rows()[0];
        let shown: Vec<bool> = FLAGS.iter().map(|f| banned.flag(f.attr, f.default)).collect();
        // isGM written false, allowTransaction written false, the rest by default.
        assert_eq!(shown, [false, false, false, false, false, true, true, true]);
    }

    #[test]
    fn copy_add_delete() {
        let mut form = AccessLevelsForm::load(XML).unwrap();
        form.copy_level(1);
        assert_eq!(form.rows()[2].get("level"), Some("1"));
        assert_eq!(form.rows()[2].get("name"), Some("User (copy)"));
        form.add_level();
        assert_eq!(form.rows().last().and_then(|r| r.get("level")), Some("110"));
        form.delete_level(3);
        let out = save(&form);
        assert!(out.contains("\t<access level=\"1\" name=\"User (copy)\" nameColor=\"FFFFFF\" titleColor=\"ECF9A2\" childAccess=\"0\" isGM=\"false\" />\r\n"));
        assert!(!out.contains("Master"));
        assert!(out.contains("name=\"New level\""));
        assert!(out.contains("<!-- Access levels -->\r\n"));
        // CRLF stays CRLF: no bare LF anywhere.
        assert!(!out.replace("\r\n", "").contains('\n'));
        XmlDoc::parse(&out).unwrap();
    }

    #[test]
    fn validation() {
        let mut form = AccessLevelsForm::load(XML).unwrap();
        form.rows_mut()[1].set("level", "100"); // duplicate of the last row
        form.rows_mut()[0].set("nameColor", "XYZ");
        form.rows_mut()[2].set("name", " ");
        form.rows_mut()[2].set("childAccess", "55");
        let issues = form.issues();
        let has = |row: usize, field: &str, warning: bool| {
            issues
                .iter()
                .any(|i| i.row == Some(row) && i.field == field && i.warning == warning)
        };
        assert!(has(1, "level", false) && has(2, "level", false), "{issues:?}");
        assert!(has(0, "nameColor", false));
        assert!(has(2, "name", false));
        assert!(has(2, "childAccess", true), "an unknown child is only a warning");
        assert!(has_errors(&form.issues()));
        form.rows_mut()[1].set("level", "0");
        form.rows_mut()[0].set("nameColor", "ABCDEF");
        form.rows_mut()[2].set("name", "M");
        assert!(!has_errors(&form.issues()));
        form.rows_mut()[0].set("level", "7");
        assert!(
            form.issues().iter().any(|i| i.row.is_none()),
            "banned level missing"
        );
        form.rows_mut()[0].set("level", "x");
        assert!(has_errors(&form.issues()));
        assert!(valid_color("7FFFFFFF") && !valid_color("80000000") && !valid_color(""));
    }

    #[test]
    fn other_attributes_survive() {
        let xml = XML.replace("isGM=\"true\" />", "isGM=\"true\" custom=\"x\" />");
        let mut form = AccessLevelsForm::load(&xml).unwrap();
        assert_eq!(form.rows()[2].get("custom"), Some("x"));
        form.rows_mut()[2].set("name", "Boss");
        let doc = XmlDoc::parse(&xml).unwrap();
        let out = apply_edits(&xml, form.edits(&doc)).unwrap();
        assert_eq!(out, xml.replace("Master", "Boss"));
    }
}
