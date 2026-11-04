//! `SecondaryAuth.xml`: the second password at character selection.
//!
//! (minutes), `recoveryLink` and `forbiddenPasswords/password`, all as element text.
//! Rust loader: `game-server/src/client/secondary.rs` (`SecondaryAuthData::parse`).
//!
//! The file says the forbidden passwords already in it must not be removed or edited, because the
//! client checks the same list. They are locked until the user ticks "Allow editing".

use l2_config::xml_edit::{Edit, XmlDoc, element_snippet};

use super::XmlForm;
use super::model::{Issue, Row, RowList, parse_int};
use super::widgets::{issue_list, show_problem, switch, text_cell};

pub const FILE: &str = "SecondaryAuth.xml";

/// An element with text, directly below the root.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scalar {
    pub name: &'static str,
    /// Index of the element in the file; `None` when the file has none (the Java default holds).
    pub orig: Option<usize>,
    pub value: String,
    snapshot: String,
}

impl Scalar {
    fn load(doc: &XmlDoc, name: &'static str, default: &str) -> Self {
        let orig = doc.first_child_named(doc.root(), name);
        let value = orig.map_or_else(|| default.to_owned(), |i| doc.text_of(i));
        Self {
            name,
            orig,
            snapshot: value.clone(),
            value,
        }
    }

    fn is_dirty(&self) -> bool {
        self.value != self.snapshot
    }

    fn edits(&self, doc: &XmlDoc) -> Vec<Edit> {
        if !self.is_dirty() {
            return Vec::new();
        }
        match self.orig {
            Some(i) => doc.set_text(i, &self.value).into_iter().collect(),
            None => {
                vec![doc.insert_first_child(doc.root(), &element_snippet(self.name, &[], Some(&self.value)))]
            }
        }
    }
}

pub struct SecondaryAuthForm {
    pub enabled: Scalar,
    pub max_attempts: Scalar,
    pub ban_time: Scalar,
    pub recovery_link: Scalar,
    passwords: RowList,
    /// The `forbiddenPasswords` element, if the file has one.
    password_parent: Option<usize>,
    /// "Allow editing" of the passwords that are in the file.
    pub unlocked: bool,
    new_password: String,
}

impl SecondaryAuthForm {
    pub fn load(text: &str) -> Result<Self, String> {
        let doc = XmlDoc::parse(text).map_err(|e| e.to_string())?;
        let parent = doc.first_child_named(doc.root(), "forbiddenPasswords");
        let passwords = match parent {
            Some(p) => RowList::load(&doc, p, "password", true),
            None => RowList::empty("password"),
        };
        Ok(Self {
            // Java field defaults: off, 5 attempts, 480 minutes, no link.
            enabled: Scalar::load(&doc, "enabled", "false"),
            max_attempts: Scalar::load(&doc, "maxAttempts", "5"),
            ban_time: Scalar::load(&doc, "banTime", "480"),
            recovery_link: Scalar::load(&doc, "recoveryLink", ""),
            passwords,
            password_parent: parent,
            unlocked: false,
            new_password: String::new(),
        })
    }

    #[cfg(test)]
    pub fn passwords(&self) -> Vec<&str> {
        self.passwords
            .rows
            .iter()
            .map(|r| r.text.as_deref().unwrap_or(""))
            .collect()
    }

    pub fn add_password(&mut self, password: &str) {
        let mut row = Row::new(&[]);
        row.text = Some(password.to_owned());
        self.passwords.rows.push(row);
    }

    pub fn remove_password(&mut self, index: usize) {
        if index < self.passwords.rows.len() {
            self.passwords.rows.remove(index);
        }
    }

    #[cfg(test)]
    pub fn set_password(&mut self, index: usize, password: &str) {
        if let Some(r) = self.passwords.rows.get_mut(index) {
            r.text = Some(password.to_owned());
        }
    }

    /// Java `Boolean.parseBoolean(text)` for `enabled`.
    pub fn is_enabled(&self) -> bool {
        self.enabled.value.eq_ignore_ascii_case("true")
    }
}

impl XmlForm for SecondaryAuthForm {
    fn is_dirty(&self) -> bool {
        self.enabled.is_dirty()
            || self.max_attempts.is_dirty()
            || self.ban_time.is_dirty()
            || self.recovery_link.is_dirty()
            || self.passwords.is_dirty()
    }

    fn issues(&self) -> Vec<Issue> {
        let mut issues = Vec::new();
        if !["true", "false"].contains(&self.enabled.value.to_ascii_lowercase().as_str()) {
            issues.push(Issue::error(None, "enabled", "must be true or false"));
        }
        for (scalar, field) in [(&self.max_attempts, "maxAttempts"), (&self.ban_time, "banTime")] {
            match parse_int(&scalar.value) {
                Some(v) if v >= 0 => {}
                _ => issues.push(Issue::error(None, field, "must be a whole number, 0 or more")),
            }
        }
        let link = self.recovery_link.value.trim();
        if !link.is_empty() && !(link.starts_with("http://") || link.starts_with("https://")) {
            issues.push(Issue::warning(
                None,
                "recoveryLink",
                "does not start with http:// or https://",
            ));
        }
        for (i, r) in self.passwords.rows.iter().enumerate() {
            let text = r.text.as_deref().unwrap_or("");
            let row = Some(i);
            if text.is_empty() {
                issues.push(Issue::error(row, "password", "a password must not be empty"));
            } else if !(text.bytes().all(|b| b.is_ascii_digit()) && (6..=8).contains(&text.len())) {
                // A secondary password is 6 to 8 digits, so any other entry never matches.
                issues.push(Issue::warning(
                    row,
                    "password",
                    "only 6 to 8 digits can ever match",
                ));
            }
            if self.passwords.rows[..i]
                .iter()
                .any(|o| o.text.as_deref().unwrap_or("") == text)
            {
                issues.push(Issue::warning(row, "password", "listed twice"));
            }
        }
        issues
    }

    fn edits(&self, doc: &XmlDoc) -> Vec<Edit> {
        let mut edits = Vec::new();
        for s in [
            &self.enabled,
            &self.max_attempts,
            &self.ban_time,
            &self.recovery_link,
        ] {
            edits.extend(s.edits(doc));
        }
        match self.password_parent {
            Some(parent) => edits.extend(self.passwords.edits(doc, parent)),
            None if !self.passwords.rows.is_empty() => {
                let unit = doc.indent_unit();
                let mut snippet = String::from("<forbiddenPasswords>");
                for r in &self.passwords.rows {
                    snippet.push_str(&format!("\n{unit}{}", r.snippet("password")));
                }
                snippet.push_str("\n</forbiddenPasswords>");
                edits.push(doc.append_child(doc.root(), &snippet));
            }
            None => {}
        }
        edits
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        let issues = self.issues();
        issue_list(ui, &issues, &|r| format!("Password {}", r + 1));
        egui::ScrollArea::vertical()
            .id_salt("secondary_auth_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("secondary_auth_grid")
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("Enabled");
                        let mut on = self.is_enabled();
                        if switch(ui, &mut on).changed() {
                            self.enabled.value = on.to_string();
                        }
                        ui.end_row();
                        ui.label("Max. attempts");
                        ui.horizontal(|ui| {
                            let mut v = parse_int(&self.max_attempts.value).unwrap_or(5);
                            if ui.add(egui::DragValue::new(&mut v).range(0..=1000)).changed() {
                                self.max_attempts.value = v.to_string();
                            }
                            ui.weak("wrong tries before the ban (0 = no limit)");
                        });
                        ui.end_row();
                        ui.label("Ban time (minutes)");
                        ui.horizontal(|ui| {
                            let mut v = parse_int(&self.ban_time.value).unwrap_or(480);
                            if ui.add(egui::DragValue::new(&mut v).range(0..=1_000_000)).changed() {
                                self.ban_time.value = v.to_string();
                            }
                        });
                        ui.end_row();
                        ui.label("Recovery link");
                        let bad = super::model::messages_for(&issues, None, "recoveryLink").is_some();
                        text_cell(ui, &mut self.recovery_link.value, 360.0, bad);
                        ui.end_row();
                    });
                for field in ["enabled", "maxAttempts", "banTime", "recoveryLink"] {
                    show_problem(ui, &issues, None, field);
                }
                ui.separator();
                ui.strong("Forbidden passwords");
                ui.label(
                    "The client checks the passwords that are in the file, so they are locked. New ones can be added freely.",
                );
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.unlocked, "Allow editing the passwords in the file");
                    text_cell(ui, &mut self.new_password, 100.0, false);
                    if ui.button("Add").clicked() && !self.new_password.is_empty() {
                        let text = std::mem::take(&mut self.new_password);
                        self.add_password(&text);
                    }
                });
                let mut remove: Option<usize> = None;
                let unlocked = self.unlocked;
                egui::Grid::new("secondary_auth_passwords")
                    .num_columns(8)
                    .spacing([10.0, 4.0])
                    .show(ui, |ui| {
                        for i in 0..self.passwords.rows.len() {
                            let locked = self.passwords.rows[i].orig.is_some() && !unlocked;
                            let bad = super::model::messages_for(&issues, Some(i), "password").is_some();
                            let text = self.passwords.rows[i].text.get_or_insert_with(String::new);
                            ui.horizontal(|ui| {
                                if locked {
                                    ui.monospace(text.as_str());
                                } else {
                                    text_cell(ui, text, 70.0, bad);
                                    if ui.small_button("x").on_hover_text("Remove").clicked() {
                                        remove = Some(i);
                                    }
                                }
                            });
                            if (i + 1) % 4 == 0 {
                                ui.end_row();
                            }
                        }
                    });
                for i in 0..self.passwords.rows.len() {
                    show_problem(ui, &issues, Some(i), "password");
                }
                if let Some(i) = remove {
                    self.remove_password(i);
                }
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use l2_config::xml_edit::apply_edits;

    const XML: &str = "<list>\r\n\t<enabled>false</enabled> <!-- on? -->\r\n\t<maxAttempts>5</maxAttempts>\r\n\t<banTime>480</banTime>\r\n\t<recoveryLink>http://www.google.com</recoveryLink>\r\n\t<forbiddenPasswords>\r\n\t\t<!-- do not edit -->\r\n\t\t<password>000000</password>\r\n\t\t<password>111111</password>\r\n\t</forbiddenPasswords>\r\n</list>\r\n";

    fn save(f: &SecondaryAuthForm, xml: &str) -> String {
        apply_edits(xml, f.edits(&XmlDoc::parse(xml).unwrap())).unwrap()
    }

    #[test]
    fn unchanged_is_identical() {
        let f = SecondaryAuthForm::load(XML).unwrap();
        assert!(!f.is_dirty() && f.issues().is_empty());
        assert_eq!(save(&f, XML), XML);
        assert_eq!(f.passwords(), ["000000", "111111"]);
        assert!(!f.is_enabled());
    }

    #[test]
    fn each_setting_touches_only_its_text() {
        let mut f = SecondaryAuthForm::load(XML).unwrap();
        f.enabled.value = "true".into();
        assert_eq!(
            save(&f, XML),
            XML.replace("<enabled>false</enabled>", "<enabled>true</enabled>")
        );
        let mut f = SecondaryAuthForm::load(XML).unwrap();
        f.max_attempts.value = "3".into();
        f.ban_time.value = "60".into();
        f.recovery_link.value = "https://x.test/?a=1&b=2".into();
        let out = save(&f, XML);
        assert!(out.contains("<maxAttempts>3</maxAttempts>\r\n\t<banTime>60</banTime>"));
        assert!(out.contains("<recoveryLink>https://x.test/?a=1&amp;b=2</recoveryLink>"));
        assert!(out.contains("<!-- on? -->"));
        // And the Java-like reader gets the value back.
        let doc = XmlDoc::parse(&out).unwrap();
        let link = doc.first_child_named(0, "recoveryLink").unwrap();
        assert_eq!(doc.text_of(link), "https://x.test/?a=1&b=2");
    }

    #[test]
    fn passwords_are_added_removed_and_edited() {
        let mut f = SecondaryAuthForm::load(XML).unwrap();
        f.add_password("123123");
        f.remove_password(0);
        f.set_password(0, "222222");
        let out = save(&f, XML);
        assert_eq!(
            out,
            XML.replace(
                "<password>000000</password>\r\n\t\t<password>111111</password>\r\n",
                "<password>222222</password>\r\n\t\t<password>123123</password>\r\n"
            )
        );
    }

    #[test]
    fn missing_elements_are_created() {
        let xml = "<list>\n\t<enabled>false</enabled>\n</list>\n";
        let mut f = SecondaryAuthForm::load(xml).unwrap();
        assert_eq!(f.max_attempts.value, "5", "the Java default");
        f.max_attempts.value = "7".into();
        f.add_password("654321");
        let out = save(&f, xml);
        assert!(out.contains("\t<maxAttempts>7</maxAttempts>\n\t<enabled>"));
        assert!(out.contains(
            "\t<forbiddenPasswords>\n\t\t<password>654321</password>\n\t</forbiddenPasswords>\n</list>"
        ));
        XmlDoc::parse(&out).unwrap();
    }

    #[test]
    fn validation() {
        let mut f = SecondaryAuthForm::load(XML).unwrap();
        f.enabled.value = "yes".into();
        f.max_attempts.value = "-1".into();
        f.ban_time.value = "x".into();
        f.recovery_link.value = "ftp://x".into();
        f.add_password("");
        f.add_password("12");
        f.add_password("000000");
        let issues = f.issues();
        let has = |row: Option<usize>, field: &str, warning: bool| {
            issues
                .iter()
                .any(|i| i.row == row && i.field == field && i.warning == warning)
        };
        assert!(
            has(None, "enabled", false) && has(None, "maxAttempts", false) && has(None, "banTime", false)
        );
        assert!(has(None, "recoveryLink", true));
        assert!(has(Some(2), "password", false), "empty");
        assert!(has(Some(3), "password", true), "not 6 to 8 digits");
        assert!(has(Some(4), "password", true), "duplicate");
    }
}
