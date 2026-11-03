//! `SiegeSchedule.xml`: the siege day and hour of each castle.
//!
//! Java: `data/xml/SiegeScheduleData.java` (`day` is a `java.util.Calendar` constant name such as
//! `SUNDAY`, or a number), `mechanics/siege/SiegeScheduleDate.java` (defaults: day Sunday, hour
//! 16, `maxConcurrent` 5, siege disabled). Rust loader: `l2_data::SiegeSchedule`.
//!
//! The castles are fixed, so the form does not add or delete a schedule.

use std::collections::HashMap;

use l2_config::xml_edit::{Edit, XmlDoc};

use super::XmlForm;
use super::model::{Issue, RowList, parse_int};
use super::widgets::{flag_cell, issue_list, problem_mark, slider_int};

pub const FILE: &str = "SiegeSchedule.xml";

/// `Calendar.SUNDAY` (1) to `SATURDAY` (7).
pub const DAYS: [&str; 7] = [
    "SUNDAY",
    "MONDAY",
    "TUESDAY",
    "WEDNESDAY",
    "THURSDAY",
    "FRIDAY",
    "SATURDAY",
];

/// The day of a `day` attribute: a name or the `Calendar` number. Java's rule: a value that is
/// not numeric is looked up as a `Calendar` field name.
pub fn day_index(text: &str) -> Option<usize> {
    if !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit()) {
        let n: usize = text.parse().ok()?;
        return (1..=7).contains(&n).then(|| n - 1);
    }
    DAYS.iter().position(|d| *d == text)
}

/// The text to write for day `index`, in the style the file already uses for `current`.
pub fn day_text(index: usize, current: Option<&str>) -> String {
    let numeric = current.is_some_and(|c| !c.is_empty() && c.bytes().all(|b| b.is_ascii_digit()));
    if numeric {
        (index + 1).to_string()
    } else {
        DAYS[index.min(6)].to_owned()
    }
}

pub struct SiegeForm {
    list: RowList,
}

impl SiegeForm {
    pub fn load(text: &str) -> Result<Self, String> {
        let doc = XmlDoc::parse(text).map_err(|e| e.to_string())?;
        Ok(Self {
            list: RowList::load(&doc, doc.root(), "schedule", false),
        })
    }

    #[cfg(test)]
    pub fn rows_mut(&mut self) -> &mut Vec<super::model::Row> {
        &mut self.list.rows
    }

    #[cfg(test)]
    pub fn set_day(&mut self, row: usize, day: usize) {
        if let Some(r) = self.list.rows.get_mut(row) {
            let text = day_text(day, r.get("day"));
            r.set("day", &text);
        }
    }

    fn row_label(&self, index: usize) -> String {
        match self.list.rows.get(index) {
            Some(r) => format!(
                "Castle {} ({})",
                r.get_or("castleId", "?"),
                r.get_or("castleName", "")
            ),
            None => format!("Row {}", index + 1),
        }
    }
}

impl XmlForm for SiegeForm {
    fn is_dirty(&self) -> bool {
        self.list.is_dirty()
    }

    fn issues(&self) -> Vec<Issue> {
        let mut issues = Vec::new();
        let mut count: HashMap<i32, usize> = HashMap::new();
        for r in &self.list.rows {
            if let Some(id) = r.get("castleId").and_then(parse_int) {
                *count.entry(id).or_default() += 1;
            }
        }
        for (i, r) in self.list.rows.iter().enumerate() {
            let row = Some(i);
            match r.get("castleId").and_then(parse_int) {
                None => issues.push(Issue::error(row, "castleId", "castleId must be a whole number")),
                Some(id) if count.get(&id).copied().unwrap_or(0) > 1 => {
                    issues.push(Issue::error(
                        row,
                        "castleId",
                        format!("castle {id} is listed twice"),
                    ));
                }
                Some(_) => {}
            }
            if let Some(day) = r.get("day")
                && day_index(day).is_none()
            {
                issues.push(Issue::error(
                    row,
                    "day",
                    "a day name (SUNDAY...SATURDAY) or 1 to 7",
                ));
            }
            if let Some(hour) = r.get("hour") {
                match parse_int(hour) {
                    Some(h) if (0..=23).contains(&h) => {}
                    _ => issues.push(Issue::error(row, "hour", "the hour must be 0 to 23")),
                }
            }
            if let Some(max) = r.get("maxConcurrent") {
                match parse_int(max) {
                    Some(m) if m >= 0 => {}
                    _ => issues.push(Issue::error(row, "maxConcurrent", "must be 0 or more")),
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
        ui.label("The siege starts on this day of the week at this hour (the machine's local time).");
        egui::ScrollArea::vertical()
            .id_salt("siege_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("siege_grid")
                    .striped(true)
                    .spacing([12.0, 5.0])
                    .show(ui, |ui| {
                        for head in ["Castle", "Siege", "Day", "Hour", "Max. concurrent"] {
                            ui.strong(head);
                        }
                        ui.end_row();
                        for i in 0..self.list.rows.len() {
                            let row = &mut self.list.rows[i];
                            ui.horizontal(|ui| {
                                ui.label(format!(
                                    "{} {}",
                                    row.get_or("castleId", "?"),
                                    row.get_or("castleName", "")
                                ));
                                problem_mark(ui, &issues, Some(i), "castleId");
                            });
                            flag_cell(ui, row, "siegeEnabled", false);
                            ui.horizontal(|ui| {
                                // Java: a missing day is Sunday.
                                let current = day_index(row.get_or("day", "SUNDAY"));
                                let mut chosen = current;
                                let shown = current.map_or("?", |d| DAYS[d]);
                                egui::ComboBox::from_id_salt(("siege_day", i))
                                    .selected_text(shown)
                                    .show_ui(ui, |ui| {
                                        for (d, name) in DAYS.iter().enumerate() {
                                            ui.selectable_value(&mut chosen, Some(d), *name);
                                        }
                                    });
                                if chosen != current
                                    && let Some(d) = chosen
                                {
                                    let text = day_text(d, row.get("day"));
                                    row.set("day", &text);
                                }
                                problem_mark(ui, &issues, Some(i), "day");
                            });
                            ui.horizontal(|ui| {
                                let mut hour = row.get_or("hour", "16").parse::<i32>().unwrap_or(16);
                                if slider_int(ui, &mut hour, 0..=23, 160.0) {
                                    row.set("hour", &hour.to_string());
                                }
                                problem_mark(ui, &issues, Some(i), "hour");
                            });
                            ui.horizontal(|ui| {
                                let mut max = row.get_or("maxConcurrent", "5").parse::<i32>().unwrap_or(5);
                                if ui.add(egui::DragValue::new(&mut max).range(0..=1000)).changed() {
                                    row.set("maxConcurrent", &max.to_string());
                                }
                                problem_mark(ui, &issues, Some(i), "maxConcurrent");
                            });
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

    const XML: &str = "<list>\n\t<schedule castleId=\"1\" castleName=\"Gludio\" siegeEnabled=\"true\" day=\"SUNDAY\" hour=\"16\" maxConcurrent=\"5\" />\n\t<schedule castleId=\"2\" castleName=\"Dion\" siegeEnabled=\"false\" day=\"3\" hour=\"20\" />\n</list>\n";

    fn save(f: &SiegeForm) -> String {
        apply_edits(XML, f.edits(&XmlDoc::parse(XML).unwrap())).unwrap()
    }

    #[test]
    fn unchanged_is_identical() {
        let f = SiegeForm::load(XML).unwrap();
        assert!(!f.is_dirty() && f.issues().is_empty());
        assert_eq!(save(&f), XML);
    }

    #[test]
    fn day_names_and_numbers() {
        assert_eq!(day_index("SUNDAY"), Some(0));
        assert_eq!(day_index("SATURDAY"), Some(6));
        assert_eq!(day_index("3"), Some(2));
        assert_eq!(day_index("0"), None);
        assert_eq!(day_index("8"), None);
        assert_eq!(
            day_index("sunday"),
            None,
            "Calendar field names are case-sensitive"
        );
        assert_eq!(day_text(2, Some("SUNDAY")), "TUESDAY");
        assert_eq!(day_text(2, Some("1")), "3");
        assert_eq!(day_text(5, None), "FRIDAY");
    }

    #[test]
    fn one_change_each_touches_one_attribute() {
        let mut f = SiegeForm::load(XML).unwrap();
        f.set_day(0, 5);
        assert_eq!(save(&f), XML.replace("day=\"SUNDAY\"", "day=\"FRIDAY\""));
        let mut f = SiegeForm::load(XML).unwrap();
        f.set_day(1, 6); // numeric file: stays numeric
        f.rows_mut()[1].set("hour", "21");
        f.rows_mut()[0].set_flag("siegeEnabled", false);
        let out = save(&f);
        assert_eq!(
            out,
            XML.replace("day=\"3\" hour=\"20\"", "day=\"7\" hour=\"21\"")
                .replace("siegeEnabled=\"true\"", "siegeEnabled=\"false\"")
        );
    }

    #[test]
    fn a_missing_max_concurrent_is_added_at_the_end() {
        let mut f = SiegeForm::load(XML).unwrap();
        f.rows_mut()[1].set("maxConcurrent", "3");
        assert!(save(&f).contains("hour=\"20\" maxConcurrent=\"3\" />"));
    }

    #[test]
    fn validation() {
        let mut f = SiegeForm::load(XML).unwrap();
        f.rows_mut()[0].set("hour", "24");
        f.rows_mut()[1].set("day", "Funday");
        f.rows_mut()[1].set("castleId", "1");
        f.rows_mut()[1].set("maxConcurrent", "-1");
        let issues = f.issues();
        let has = |row, field| issues.iter().any(|i| i.row == Some(row) && i.field == field);
        assert!(has(0, "hour") && has(1, "day") && has(0, "castleId") && has(1, "castleId"));
        assert!(has(1, "maxConcurrent"));
        f.rows_mut()[0].set("hour", "0");
        assert!(!f.issues().iter().any(|i| i.field == "hour"), "0 is a valid hour");
    }
}
