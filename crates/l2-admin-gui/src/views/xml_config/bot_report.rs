//! `BotReportPunishments.xml`: the skill a reported player gets at a number of reports, and the
//! system message he sees.
//!
//! Java: `data/BotReportTable.java` (`PunishmentsLoader`): `neededReportCount`, `skillId`,
//! `skillLevel` (default 1) and `sysMessageId` (optional). A negative count is a range: the
//! punishment applies from that many reports up. Rust loader:
//! `game-server/src/world_data.rs` (`bot_report_punishments_from_xml`).

use std::collections::HashMap;

use l2_config::xml_edit::{Edit, XmlDoc};

use super::XmlForm;
use super::model::{Issue, Row, RowList, parse_int};
use super::widgets::{attr_cell, issue_list, problem_mark};

pub const FILE: &str = "BotReportPunishments.xml";

pub struct BotReportForm {
    list: RowList,
}

impl BotReportForm {
    pub fn load(text: &str) -> Result<Self, String> {
        let doc = XmlDoc::parse(text).map_err(|e| e.to_string())?;
        Ok(Self {
            list: RowList::load(&doc, doc.root(), "punishment", false),
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

    /// A punishment below the last one: 25 more reports, the same skill.
    pub fn add_punishment(&mut self) {
        let last = self.list.rows.last();
        let count = last
            .and_then(|r| r.get("neededReportCount").and_then(parse_int))
            .map_or(25, |c| c.saturating_add(25));
        self.list.rows.push(Row::new(&[
            ("neededReportCount", &count.to_string()),
            ("skillId", last.map_or("6038", |r| r.get_or("skillId", "6038"))),
            ("skillLevel", "1"),
        ]));
    }

    fn row_label(&self, index: usize) -> String {
        match self.list.rows.get(index) {
            Some(r) => format!("Punishment at {} reports", r.get_or("neededReportCount", "?")),
            None => format!("Row {}", index + 1),
        }
    }
}

impl XmlForm for BotReportForm {
    fn is_dirty(&self) -> bool {
        self.list.is_dirty()
    }

    fn issues(&self) -> Vec<Issue> {
        let mut issues = Vec::new();
        let mut count: HashMap<i32, usize> = HashMap::new();
        for r in &self.list.rows {
            if let Some(c) = r.get("neededReportCount").and_then(parse_int) {
                *count.entry(c).or_default() += 1;
            }
        }
        for (i, r) in self.list.rows.iter().enumerate() {
            let row = Some(i);
            match r.get("neededReportCount").and_then(parse_int) {
                None => issues.push(Issue::error(row, "neededReportCount", "must be a whole number")),
                Some(0) => issues.push(Issue::error(row, "neededReportCount", "must not be 0")),
                Some(c) if count.get(&c).copied().unwrap_or(0) > 1 => {
                    // NOTE(java-quirk): the punishments are a map by count; a repeated count
                    // replaces the earlier one.
                    issues.push(Issue::error(
                        row,
                        "neededReportCount",
                        format!("{c} is used twice"),
                    ));
                }
                Some(_) => {}
            }
            match r.get("skillId").and_then(parse_int) {
                Some(s) if s > 0 => {}
                _ => issues.push(Issue::error(row, "skillId", "must be a skill id (1 or more)")),
            }
            if let Some(level) = r.get("skillLevel") {
                match parse_int(level) {
                    Some(l) if l >= 1 => {}
                    _ => issues.push(Issue::error(row, "skillLevel", "must be 1 or more")),
                }
            }
            if let Some(msg) = r.get("sysMessageId") {
                match parse_int(msg) {
                    Some(m) if m > 0 => {}
                    _ => issues.push(Issue::error(
                        row,
                        "sysMessageId",
                        "must be a system message id; leave empty for none",
                    )),
                }
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
        ui.horizontal(|ui| {
            if ui.button("Add punishment").clicked() {
                self.add_punishment();
            }
            ui.weak("A negative report count is a range: the punishment applies from that many reports up.");
        });
        let mut delete: Option<usize> = None;
        egui::ScrollArea::vertical()
            .id_salt("bot_report_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("bot_report_grid")
                    .striped(true)
                    .spacing([12.0, 4.0])
                    .show(ui, |ui| {
                        for head in ["Reports", "Skill id", "Skill level", "System message id", ""] {
                            ui.strong(head);
                        }
                        ui.end_row();
                        for i in 0..self.list.rows.len() {
                            let bad =
                                |field: &str| super::model::messages_for(&issues, Some(i), field).is_some();
                            let row = &mut self.list.rows[i];
                            for (field, optional, width) in [
                                ("neededReportCount", false, 70.0),
                                ("skillId", false, 70.0),
                                ("skillLevel", true, 50.0),
                                ("sysMessageId", true, 70.0),
                            ] {
                                ui.horizontal(|ui| {
                                    attr_cell(ui, row, field, width, optional, bad(field));
                                    problem_mark(ui, &issues, Some(i), field);
                                });
                            }
                            if ui.small_button("Delete").clicked() {
                                delete = Some(i);
                            }
                            ui.end_row();
                        }
                    });
            });
        if let Some(i) = delete {
            self.list.rows.remove(i);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use l2_config::xml_edit::apply_edits;

    const XML: &str = "<list>\n\t<!-- Range -->\n\t<punishment neededReportCount=\"25\" skillId=\"6038\" skillLevel=\"1\" sysMessageId=\"2473\" />\n\t<punishment neededReportCount=\"-150\" skillId=\"6040\" skillLevel=\"1\" />\n</list>\n";

    fn save(f: &BotReportForm) -> String {
        apply_edits(XML, f.edits(&XmlDoc::parse(XML).unwrap())).unwrap()
    }

    #[test]
    fn unchanged_is_identical() {
        let f = BotReportForm::load(XML).unwrap();
        assert!(!f.is_dirty() && f.issues().is_empty());
        assert_eq!(save(&f), XML);
    }

    #[test]
    fn edit_add_and_remove_an_optional_attribute() {
        let mut f = BotReportForm::load(XML).unwrap();
        f.rows_mut()[0].set("neededReportCount", "30");
        f.rows_mut()[0].clear("sysMessageId");
        f.rows_mut()[1].set("sysMessageId", "2480");
        let out = save(&f);
        assert_eq!(
            out,
            "<list>\n\t<!-- Range -->\n\t<punishment neededReportCount=\"30\" skillId=\"6038\" skillLevel=\"1\" />\n\t<punishment neededReportCount=\"-150\" skillId=\"6040\" skillLevel=\"1\" sysMessageId=\"2480\" />\n</list>\n"
        );
    }

    #[test]
    fn add_and_delete() {
        let mut f = BotReportForm::load(XML).unwrap();
        f.add_punishment();
        assert_eq!(f.rows()[2].get("neededReportCount"), Some("-125"));
        f.rows_mut().remove(0);
        let out = save(&f);
        assert!(!out.contains("\"25\""));
        assert!(out.ends_with(
            "<punishment neededReportCount=\"-125\" skillId=\"6040\" skillLevel=\"1\" />\n</list>\n"
        ));
    }

    #[test]
    fn validation() {
        let mut f = BotReportForm::load(XML).unwrap();
        f.rows_mut()[0].set("neededReportCount", "-150");
        f.rows_mut()[1].set("skillId", "0");
        f.rows_mut()[1].set("skillLevel", "x");
        f.rows_mut()[0].set("sysMessageId", "");
        let issues = f.issues();
        let has = |row, field| issues.iter().any(|i| i.row == Some(row) && i.field == field);
        assert!(has(0, "neededReportCount") && has(1, "neededReportCount"));
        assert!(has(1, "skillId") && has(1, "skillLevel") && has(0, "sysMessageId"));
        f.rows_mut()[0].set("neededReportCount", "0");
        assert!(
            f.issues()
                .iter()
                .any(|i| i.row == Some(0) && i.message.contains("0"))
        );
    }
}
