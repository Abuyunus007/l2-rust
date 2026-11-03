//! `AdminCommands.xml`: which access level may run which `//` command, and which commands ask
//! for a confirmation.
//!
//! Java: `data/xml/AdminData.java` (`<admin>`), `data/holders/AccessRight.java` (`accessLevel`
//! default 100, `confirmDlg` default false). The commands themselves are fixed (a handler must
//! exist for each), so the form edits their level and confirmation only; it neither adds nor
//! deletes a command.

use std::collections::HashMap;

use l2_config::xml_edit::{Edit, XmlDoc};

use super::XmlForm;
use super::model::{Issue, Row, RowList, parse_int};
use super::widgets::{flag_cell, issue_list, problem_mark};

pub const FILE: &str = "AdminCommands.xml";
/// Java `AccessRight`: the level of a command without `accessLevel`.
pub const DEFAULT_LEVEL: i32 = 100;

pub struct AdminCommandsForm {
    list: RowList,
    /// The levels of `AccessLevels.xml` (as currently edited): the drop-down and the check.
    levels: Vec<(i32, String)>,
    pub search: String,
    /// Only the commands of this level (`None` = all).
    pub level_filter: Option<i32>,
    /// The level the "Set for all shown" button writes.
    pub bulk_level: i32,
}

impl AdminCommandsForm {
    pub fn load(text: &str) -> Result<Self, String> {
        let doc = XmlDoc::parse(text).map_err(|e| e.to_string())?;
        Ok(Self {
            list: RowList::load(&doc, doc.root(), "admin", false),
            levels: Vec::new(),
            search: String::new(),
            level_filter: None,
            bulk_level: DEFAULT_LEVEL,
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

    /// Hands the form the levels of the access levels form.
    pub fn set_levels(&mut self, levels: Vec<(i32, String)>) {
        self.levels = levels;
    }

    /// The level of a row as the server reads it.
    pub fn level_of(row: &Row) -> Option<i32> {
        match row.get("accessLevel") {
            None => Some(DEFAULT_LEVEL),
            Some(v) => parse_int(v),
        }
    }

    /// Indices of the rows the search and the level filter show.
    pub fn shown(&self) -> Vec<usize> {
        let query = self.search.trim().to_lowercase();
        self.list
            .rows
            .iter()
            .enumerate()
            .filter(|(_, r)| {
                query.is_empty()
                    || r.get_or("command", "").to_lowercase().contains(&query)
                    || r.get_or("description", "").to_lowercase().contains(&query)
            })
            .filter(|(_, r)| self.level_filter.is_none_or(|l| Self::level_of(r) == Some(l)))
            .map(|(i, _)| i)
            .collect()
    }

    /// Sets the level of every shown command; returns how many rows changed.
    pub fn set_level_for_shown(&mut self, level: i32) -> usize {
        let text = level.to_string();
        let mut changed = 0;
        for i in self.shown() {
            let row = &mut self.list.rows[i];
            if row.get("accessLevel") != Some(text.as_str()) {
                row.set("accessLevel", &text);
                changed += 1;
            }
        }
        changed
    }

    fn level_label(&self, level: i32) -> String {
        match self.levels.iter().find(|(l, _)| *l == level) {
            Some((_, name)) => format!("{level} {name}"),
            None => format!("{level} (no such level)"),
        }
    }

    fn row_label(&self, index: usize) -> String {
        match self.list.rows.get(index) {
            Some(r) => format!("Command {}", r.get_or("command", "?")),
            None => format!("Row {}", index + 1),
        }
    }
}

impl XmlForm for AdminCommandsForm {
    fn is_dirty(&self) -> bool {
        self.list.is_dirty()
    }

    fn issues(&self) -> Vec<Issue> {
        let mut issues = Vec::new();
        let mut count: HashMap<&str, usize> = HashMap::new();
        for r in &self.list.rows {
            *count.entry(r.get_or("command", "")).or_default() += 1;
        }
        for (i, r) in self.list.rows.iter().enumerate() {
            let row = Some(i);
            let command = r.get_or("command", "");
            if command.trim().is_empty() {
                issues.push(Issue::error(row, "command", "a command needs a name"));
            } else if count.get(command).copied().unwrap_or(0) > 1 {
                // NOTE(java-quirk): Java keeps the last of a repeated command (LinkedHashMap.put).
                issues.push(Issue::warning(
                    row,
                    "command",
                    "listed twice; the last one counts",
                ));
            }
            match Self::level_of(r) {
                None => issues.push(Issue::error(
                    row,
                    "accessLevel",
                    "the level must be a whole number",
                )),
                Some(level) if !self.levels.is_empty() && !self.levels.iter().any(|(l, _)| *l == level) => {
                    issues.push(Issue::warning(
                        row,
                        "accessLevel",
                        format!("level {level} is not in AccessLevels.xml"),
                    ));
                }
                Some(_) => {}
            }
        }
        issues
    }

    fn edits(&self, doc: &XmlDoc) -> Vec<Edit> {
        self.list.edits(doc, doc.root())
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        let issues = self.issues();
        issue_list(ui, &issues, &|r| self.row_label(r));
        let shown = self.shown();
        ui.horizontal(|ui| {
            ui.label("Search:");
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .desired_width(180.0)
                    .hint_text("command or text"),
            );
            ui.label("Level:");
            let filter_text = self
                .level_filter
                .map_or("all".to_owned(), |l| self.level_label(l));
            egui::ComboBox::from_id_salt("admin_cmd_filter")
                .selected_text(filter_text)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.level_filter, None, "all");
                    let levels = self.levels.clone();
                    for (level, name) in levels {
                        ui.selectable_value(&mut self.level_filter, Some(level), format!("{level} {name}"));
                    }
                });
            ui.label(format!("{} of {} shown", shown.len(), self.list.rows.len()));
        });
        ui.horizontal(|ui| {
            ui.label("Set the level of all shown commands to");
            let current = self.level_label(self.bulk_level);
            egui::ComboBox::from_id_salt("admin_cmd_bulk")
                .selected_text(current)
                .show_ui(ui, |ui| {
                    let levels = self.levels.clone();
                    for (level, name) in levels {
                        ui.selectable_value(&mut self.bulk_level, level, format!("{level} {name}"));
                    }
                });
            if ui
                .add_enabled(
                    !shown.is_empty(),
                    egui::Button::new(format!("Set for {} shown", shown.len())),
                )
                .clicked()
            {
                let level = self.bulk_level;
                self.set_level_for_shown(level);
            }
        });
        ui.separator();
        let levels = self.levels.clone();
        egui::ScrollArea::both()
            .id_salt("admin_commands_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("admin_commands_grid")
                    .striped(true)
                    .spacing([10.0, 3.0])
                    .show(ui, |ui| {
                        ui.strong("Command");
                        ui.strong("Access level");
                        ui.strong("Confirm");
                        ui.strong("Description");
                        ui.end_row();
                        for &i in &shown {
                            let row = &mut self.list.rows[i];
                            ui.horizontal(|ui| {
                                ui.monospace(row.get_or("command", "").to_owned());
                                problem_mark(ui, &issues, Some(i), "command");
                            });
                            ui.horizontal(|ui| {
                                let mut level = Self::level_of(row).unwrap_or(DEFAULT_LEVEL);
                                let text = match levels.iter().find(|(l, _)| *l == level) {
                                    Some((_, name)) => format!("{level} {name}"),
                                    None => format!("{level} (no such level)"),
                                };
                                let before = level;
                                egui::ComboBox::from_id_salt(("admin_level", i))
                                    .selected_text(text)
                                    .width(150.0)
                                    .show_ui(ui, |ui| {
                                        for (l, name) in &levels {
                                            ui.selectable_value(&mut level, *l, format!("{l} {name}"));
                                        }
                                    });
                                if level != before {
                                    row.set("accessLevel", &level.to_string());
                                }
                                problem_mark(ui, &issues, Some(i), "accessLevel");
                            });
                            flag_cell(ui, row, "confirmDlg", false);
                            ui.label(row.get_or("description", "").to_owned());
                            ui.end_row();
                        }
                    });
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use l2_config::xml_edit::apply_edits;

    const XML: &str = "<list>\n\t<!-- ADMIN -->\n\t<admin command=\"admin\" description=\"Open menu.\" accessLevel=\"30\" />\n\t<admin command=\"gmon\" description=\"GM mode.\" accessLevel=\"100\" />\n\t<admin command=\"endolympiad\" description=\"End.\" accessLevel=\"100\" confirmDlg=\"true\" />\n\t<admin command=\"plain\" description=\"No level.\" />\n</list>\n";

    fn form() -> AdminCommandsForm {
        let mut f = AdminCommandsForm::load(XML).unwrap();
        f.set_levels(vec![
            (0, "User".into()),
            (30, "General GM".into()),
            (100, "Master".into()),
        ]);
        f
    }

    fn save(f: &AdminCommandsForm) -> String {
        let doc = XmlDoc::parse(XML).unwrap();
        apply_edits(XML, f.edits(&doc)).unwrap()
    }

    #[test]
    fn unchanged_is_identical_and_clean() {
        let f = form();
        assert!(!f.is_dirty());
        assert_eq!(save(&f), XML);
        assert!(f.issues().is_empty());
        // A missing level reads as Java's 100.
        assert_eq!(AdminCommandsForm::level_of(&f.rows()[3]), Some(100));
    }

    #[test]
    fn one_level_one_attribute() {
        let mut f = form();
        f.rows_mut()[1].set("accessLevel", "30");
        assert_eq!(
            save(&f),
            XML.replace(
                "command=\"gmon\" description=\"GM mode.\" accessLevel=\"100\"",
                "command=\"gmon\" description=\"GM mode.\" accessLevel=\"30\""
            )
        );
    }

    #[test]
    fn confirm_switch_adds_and_changes_the_attribute() {
        let mut f = form();
        f.rows_mut()[1].set_flag("confirmDlg", true);
        f.rows_mut()[2].set_flag("confirmDlg", false);
        let out = save(&f);
        assert!(out.contains("accessLevel=\"100\" confirmDlg=\"true\" />\n"));
        assert!(out.contains("confirmDlg=\"false\" />"));
    }

    #[test]
    fn search_and_filter_and_bulk_set() {
        let mut f = form();
        f.search = "OLYMP".into();
        assert_eq!(f.shown(), [2]);
        f.search.clear();
        f.level_filter = Some(100);
        assert_eq!(f.shown(), [1, 2, 3]); // the one without a level counts as 100
        f.search = "GM".into(); // "GM mode." in the description
        assert_eq!(f.shown(), [1]);
        f.search.clear();
        assert_eq!(f.set_level_for_shown(30), 3);
        assert_eq!(f.set_level_for_shown(30), 0, "already there");
        let out = save(&f);
        assert_eq!(out.matches("accessLevel=\"30\"").count(), 4);
        // The row that had no level got the attribute at the end of its tag.
        assert!(out.contains("description=\"No level.\" accessLevel=\"30\" />"));
        assert!(out.contains("command=\"admin\""), "the first row is untouched");
    }

    #[test]
    fn validation() {
        let mut f = form();
        f.rows_mut()[0].set("accessLevel", "55");
        f.rows_mut()[1].set("accessLevel", "abc");
        f.rows_mut()[2].set("command", "admin");
        let issues = f.issues();
        assert!(
            issues
                .iter()
                .any(|i| i.row == Some(0) && i.field == "accessLevel" && i.warning)
        );
        assert!(
            issues
                .iter()
                .any(|i| i.row == Some(1) && i.field == "accessLevel" && !i.warning)
        );
        assert!(
            issues
                .iter()
                .any(|i| i.row == Some(2) && i.field == "command" && i.warning)
        );
        f.rows_mut()[3].set("command", " ");
        assert!(
            f.issues()
                .iter()
                .any(|i| i.row == Some(3) && i.field == "command" && !i.warning)
        );
    }
}
