//! `DynamicExpRates.xml`: an exp and sp rate per player level (used when any rate is not 1).
//!
//! Java: `data/xml/DynamicExpRateData.java` (`<dynamic level exp sp>`, floats).

use std::collections::HashMap;

use l2_config::xml_edit::{Edit, XmlDoc};

use super::XmlForm;
use super::model::{Issue, Row, RowList, format_float, parse_float, parse_int};
use super::widgets::{attr_cell, issue_list, problem_mark, slider_float};

pub const FILE: &str = "DynamicExpRates.xml";
/// The slider covers this; the number field next to it takes any value.
pub const SLIDER_MAX: f32 = 20.0;
/// Java `getByte("MaximumPlayerLevel")` is a `byte`.
pub const MAX_LEVEL: i32 = 127;

pub struct DynamicExpForm {
    list: RowList,
    /// The "set a range" tool: from level, to level, exp, sp.
    pub tool: (i32, i32, f32, f32),
}

impl DynamicExpForm {
    pub fn load(text: &str) -> Result<Self, String> {
        let doc = XmlDoc::parse(text).map_err(|e| e.to_string())?;
        Ok(Self {
            list: RowList::load(&doc, doc.root(), "dynamic", false),
            tool: (1, 85, 1.0, 1.0),
        })
    }

    #[cfg(test)]
    pub fn rows_mut(&mut self) -> &mut Vec<Row> {
        &mut self.list.rows
    }

    /// Sets exp and sp of every existing row whose level is in `from..=to`. Returns the count.
    pub fn set_range(&mut self, from: i32, to: i32, exp: f32, sp: f32) -> usize {
        let (exp, sp) = (format_float(exp), format_float(sp));
        let mut count = 0;
        for row in &mut self.list.rows {
            let Some(level) = row.get("level").and_then(parse_int) else {
                continue;
            };
            if (from..=to).contains(&level) {
                row.set("exp", &exp);
                row.set("sp", &sp);
                count += 1;
            }
        }
        count
    }

    /// Adds the level after the highest one, with rates 1.
    pub fn add_level(&mut self) {
        let next = self
            .list
            .rows
            .iter()
            .filter_map(|r| r.get("level").and_then(parse_int))
            .max()
            .map_or(1, |m| m.saturating_add(1));
        self.list.rows.push(Row::new(&[
            ("level", &next.to_string()),
            ("exp", "1"),
            ("sp", "1"),
        ]));
    }

    fn row_label(&self, index: usize) -> String {
        match self.list.rows.get(index) {
            Some(r) => format!("Level {}", r.get_or("level", "?")),
            None => format!("Row {}", index + 1),
        }
    }
}

impl XmlForm for DynamicExpForm {
    fn is_dirty(&self) -> bool {
        self.list.is_dirty()
    }

    fn issues(&self) -> Vec<Issue> {
        let mut issues = Vec::new();
        let mut count: HashMap<i32, usize> = HashMap::new();
        for r in &self.list.rows {
            if let Some(level) = r.get("level").and_then(parse_int) {
                *count.entry(level).or_default() += 1;
            }
        }
        for (i, r) in self.list.rows.iter().enumerate() {
            let row = Some(i);
            match r.get("level").and_then(parse_int) {
                None => issues.push(Issue::error(row, "level", "the level must be a whole number")),
                Some(l) if !(1..=MAX_LEVEL).contains(&l) => {
                    issues.push(Issue::error(
                        row,
                        "level",
                        format!("level must be 1 to {MAX_LEVEL}"),
                    ));
                }
                Some(l) if count.get(&l).copied().unwrap_or(0) > 1 => {
                    issues.push(Issue::error(row, "level", format!("level {l} is listed twice")));
                }
                Some(_) => {}
            }
            for field in ["exp", "sp"] {
                match r.get(field).and_then(parse_float) {
                    None => issues.push(Issue::error(row, field, "must be a number")),
                    Some(v) if v < 0.0 => issues.push(Issue::error(row, field, "must not be negative")),
                    Some(_) => {}
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
        ui.label("Rates of 1 everywhere switch the feature off. The sliders go to 20; type a bigger number if you need one.");
        ui.horizontal(|ui| {
            ui.label("Set levels");
            ui.add(egui::DragValue::new(&mut self.tool.0).range(1..=MAX_LEVEL));
            ui.label("to");
            ui.add(egui::DragValue::new(&mut self.tool.1).range(1..=MAX_LEVEL));
            ui.label("exp");
            ui.add(
                egui::DragValue::new(&mut self.tool.2)
                    .speed(0.1)
                    .range(0.0..=1000.0),
            );
            ui.label("sp");
            ui.add(
                egui::DragValue::new(&mut self.tool.3)
                    .speed(0.1)
                    .range(0.0..=1000.0),
            );
            if ui.button("Apply").clicked() {
                let (from, to, exp, sp) = self.tool;
                self.set_range(from, to, exp, sp);
            }
            if ui.button("Add level").clicked() {
                self.add_level();
            }
        });
        ui.separator();
        let mut delete: Option<usize> = None;
        egui::ScrollArea::vertical()
            .id_salt("dynamic_exp_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("dynamic_exp_grid")
                    .striped(true)
                    .spacing([10.0, 3.0])
                    .show(ui, |ui| {
                        ui.strong("Level");
                        ui.strong("Exp rate");
                        ui.strong("Sp rate");
                        ui.label("");
                        ui.end_row();
                        for i in 0..self.list.rows.len() {
                            let row = &mut self.list.rows[i];
                            ui.horizontal(|ui| {
                                attr_cell(ui, row, "level", 40.0, false, false);
                                problem_mark(ui, &issues, Some(i), "level");
                            });
                            for field in ["exp", "sp"] {
                                ui.horizontal(|ui| {
                                    let mut value = row.get(field).and_then(parse_float).unwrap_or(1.0);
                                    if slider_float(ui, &mut value, 0.0..=SLIDER_MAX, 200.0) {
                                        row.set(field, &format_float(value));
                                    }
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

    const XML: &str = "<list>\n\t<dynamic level=\"1\" exp=\"1\" sp=\"1\" />\n\t<dynamic level=\"2\" exp=\"1\" sp=\"1\" />\n\t<dynamic level=\"3\" exp=\"1\" sp=\"1\" />\n</list>\n";

    fn save(f: &DynamicExpForm) -> String {
        apply_edits(XML, f.edits(&XmlDoc::parse(XML).unwrap())).unwrap()
    }

    #[test]
    fn unchanged_is_identical() {
        let f = DynamicExpForm::load(XML).unwrap();
        assert!(!f.is_dirty() && f.issues().is_empty());
        assert_eq!(save(&f), XML);
    }

    #[test]
    fn one_rate_one_attribute() {
        let mut f = DynamicExpForm::load(XML).unwrap();
        f.rows_mut()[1].set("exp", &format_float(2.5));
        assert_eq!(
            save(&f),
            XML.replace("level=\"2\" exp=\"1\"", "level=\"2\" exp=\"2.5\"")
        );
    }

    #[test]
    fn range_tool_and_add() {
        let mut f = DynamicExpForm::load(XML).unwrap();
        assert_eq!(f.set_range(2, 3, 3.0, 1.5), 2);
        f.add_level();
        let out = save(&f);
        assert!(out.contains("<dynamic level=\"1\" exp=\"1\" sp=\"1\" />"));
        assert!(out.contains("<dynamic level=\"2\" exp=\"3\" sp=\"1.5\" />"));
        assert!(out.contains("<dynamic level=\"4\" exp=\"1\" sp=\"1\" />\n</list>"));
    }

    #[test]
    fn validation() {
        let mut f = DynamicExpForm::load(XML).unwrap();
        f.rows_mut()[0].set("level", "0");
        f.rows_mut()[1].set("level", "3");
        f.rows_mut()[2].set("exp", "-1");
        f.rows_mut()[2].set("sp", "x");
        let issues = f.issues();
        let has = |row, field| issues.iter().any(|i| i.row == Some(row) && i.field == field);
        assert!(has(0, "level") && has(1, "level") && has(2, "level"));
        assert!(has(2, "exp") && has(2, "sp"));
    }
}
