//! `default-ipconfig.xml` / `ipconfig.xml`: which address the login server shows to clients of
//! which network.
//!
//! the address for everybody else, each `<define subnet address>` is a network with its own
//! address. The first matching subnet wins, so the order of the rows matters. Rust loader:
//! `game-server/src/ls_link/ipconfig.rs`.
//!
//! NOTE: the servers only read `ipconfig.xml`. `default-ipconfig.xml` is a template; without an
//! `ipconfig.xml` the game server finds its addresses itself.

use l2_config::xml_edit::{Edit, XmlDoc};

use super::XmlForm;
use super::model::{Issue, Row, RowList, is_ipv4};
use super::widgets::{attr_cell, issue_list, problem_mark, text_cell};

pub const TEMPLATE_FILE: &str = "default-ipconfig.xml";
pub const FILE: &str = "ipconfig.xml";

/// `a.b.c.d/n` with n from 0 to 32.
pub fn is_subnet(text: &str) -> bool {
    match text.split_once('/') {
        Some((ip, bits)) => {
            is_ipv4(ip)
                && !bits.is_empty()
                && bits.bytes().all(|b| b.is_ascii_digit())
                && bits.parse::<u8>().is_ok_and(|n| n <= 32)
        }
        None => false,
    }
}

pub struct IpConfigForm {
    /// The `<gameserver>` element as a one-row list so the attribute diff is shared.
    root: Row,
    root_snapshot: Row,
    defines: RowList,
}

impl IpConfigForm {
    pub fn load(text: &str) -> Result<Self, String> {
        let doc = XmlDoc::parse(text).map_err(|e| e.to_string())?;
        if !doc.element(doc.root()).name.eq_ignore_ascii_case("gameserver") {
            return Err("the root element must be <gameserver>".into());
        }
        let root = Row::from_element(&doc, doc.root(), false);
        Ok(Self {
            root_snapshot: root.clone(),
            root,
            defines: RowList::load(&doc, doc.root(), "define", false),
        })
    }

    #[cfg(test)]
    pub fn default_address(&self) -> &str {
        self.root.get_or("address", "")
    }

    #[cfg(test)]
    pub fn set_default_address(&mut self, address: &str) {
        self.root.set("address", address);
    }

    #[cfg(test)]
    pub fn defines_mut(&mut self) -> &mut Vec<Row> {
        &mut self.defines.rows
    }

    pub fn add_define(&mut self) {
        self.defines.rows.push(Row::new(&[
            ("subnet", "192.168.0.0/24"),
            ("address", "192.168.0.1"),
        ]));
    }

    /// Moves a subnet up or down: the values swap, the elements stay where they are in the file.
    pub fn swap(&mut self, a: usize, b: usize) {
        if a.max(b) < self.defines.rows.len() && a != b {
            let (attrs_a, attrs_b) = (
                self.defines.rows[a].attrs.clone(),
                self.defines.rows[b].attrs.clone(),
            );
            self.defines.rows[a].attrs = attrs_b;
            self.defines.rows[b].attrs = attrs_a;
        }
    }

    fn row_label(&self, index: usize) -> String {
        match self.defines.rows.get(index) {
            Some(r) => format!("Network {}", r.get_or("subnet", "?")),
            None => format!("Row {}", index + 1),
        }
    }
}

impl XmlForm for IpConfigForm {
    fn is_dirty(&self) -> bool {
        self.root != self.root_snapshot || self.defines.is_dirty()
    }

    fn issues(&self) -> Vec<Issue> {
        let mut issues = Vec::new();
        match self.root.get("address") {
            None => issues.push(Issue::warning(
                None,
                "address",
                "no default address; Java then uses 127.0.0.1",
            )),
            Some(a) if !is_ipv4(a) => issues.push(Issue::error(
                None,
                "address",
                "must be an IPv4 address like 203.0.113.5",
            )),
            Some(_) => {}
        }
        for (i, r) in self.defines.rows.iter().enumerate() {
            let row = Some(i);
            if !is_subnet(r.get_or("subnet", "")) {
                issues.push(Issue::error(row, "subnet", "must look like 192.168.0.0/24"));
            }
            if !is_ipv4(r.get_or("address", "")) {
                issues.push(Issue::error(row, "address", "must be an IPv4 address"));
            }
            if self.defines.rows[..i]
                .iter()
                .any(|o| o.get("subnet") == r.get("subnet"))
            {
                issues.push(Issue::warning(row, "subnet", "listed twice; the first one wins"));
            }
        }
        issues
    }

    fn edits(&self, doc: &XmlDoc) -> Vec<Edit> {
        let mut edits = super::model::row_edits(doc, doc.root(), &self.root);
        edits.extend(self.defines.edits(doc, doc.root()));
        edits
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        let issues = self.issues();
        issue_list(ui, &issues, &|r| self.row_label(r));
        ui.horizontal(|ui| {
            ui.label("Address for everybody else:");
            let mut address = self.root.get_or("address", "").to_owned();
            let bad = super::model::messages_for(&issues, None, "address").is_some();
            if text_cell(ui, &mut address, 140.0, bad).changed() {
                self.root.set("address", &address);
            }
        });
        ui.weak("Players whose address is in one of the networks below get that network's address. The first match wins.");
        if ui.button("Add network").clicked() {
            self.add_define();
        }
        let mut action: Option<(usize, i8)> = None; // (row, -1 up / 1 down / 0 delete)
        let count = self.defines.rows.len();
        egui::Grid::new("ipconfig_grid")
            .striped(true)
            .spacing([12.0, 4.0])
            .show(ui, |ui| {
                ui.strong("Network (subnet)");
                ui.strong("Address shown");
                ui.label("");
                ui.end_row();
                for i in 0..count {
                    let bad = |field: &str| super::model::messages_for(&issues, Some(i), field).is_some();
                    let row = &mut self.defines.rows[i];
                    for (field, width) in [("subnet", 150.0), ("address", 130.0)] {
                        ui.horizontal(|ui| {
                            attr_cell(ui, row, field, width, false, bad(field));
                            problem_mark(ui, &issues, Some(i), field);
                        });
                    }
                    ui.horizontal(|ui| {
                        if ui.add_enabled(i > 0, egui::Button::new("Up").small()).clicked() {
                            action = Some((i, -1));
                        }
                        if ui
                            .add_enabled(i + 1 < count, egui::Button::new("Down").small())
                            .clicked()
                        {
                            action = Some((i, 1));
                        }
                        if ui.small_button("Delete").clicked() {
                            action = Some((i, 0));
                        }
                    });
                    ui.end_row();
                }
            });
        match action {
            Some((i, -1)) => self.swap(i, i - 1),
            Some((i, 1)) => self.swap(i, i + 1),
            Some((i, _)) => {
                self.defines.rows.remove(i);
            }
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use l2_config::xml_edit::apply_edits;

    const XML: &str = "<?xml version=\"1.0\"?>\r\n<!-- Note -->\r\n<gameserver address=\"127.0.0.1\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:noNamespaceSchemaLocation=\"../data/xsd/ipconfig.xsd\">\r\n\t<!-- Localhost -->\r\n\t<define subnet=\"127.0.0.0/8\" address=\"127.0.0.1\" />\r\n\t<define subnet=\"10.0.0.0/8\" address=\"10.0.0.0\" />\r\n</gameserver>\r\n";

    fn save(f: &IpConfigForm) -> String {
        apply_edits(XML, f.edits(&XmlDoc::parse(XML).unwrap())).unwrap()
    }

    #[test]
    fn unchanged_is_identical() {
        let f = IpConfigForm::load(XML).unwrap();
        assert!(!f.is_dirty() && f.issues().is_empty());
        assert_eq!(save(&f), XML);
        assert_eq!(f.default_address(), "127.0.0.1");
    }

    #[test]
    fn the_default_address_changes_only_that_value() {
        let mut f = IpConfigForm::load(XML).unwrap();
        f.set_default_address("203.0.113.5");
        assert_eq!(
            save(&f),
            XML.replacen("address=\"127.0.0.1\" xmlns", "address=\"203.0.113.5\" xmlns", 1)
        );
    }

    #[test]
    fn moving_swaps_the_values_and_keeps_the_elements() {
        let mut f = IpConfigForm::load(XML).unwrap();
        f.swap(0, 1);
        let out = save(&f);
        assert!(out.contains("\t<define subnet=\"10.0.0.0/8\" address=\"10.0.0.0\" />\r\n\t<define subnet=\"127.0.0.0/8\" address=\"127.0.0.1\" />\r\n"));
        assert!(out.contains("<!-- Localhost -->"));
    }

    #[test]
    fn add_and_delete() {
        let mut f = IpConfigForm::load(XML).unwrap();
        f.add_define();
        f.defines_mut().remove(0);
        let out = save(&f);
        assert!(!out.contains("127.0.0.0/8"));
        assert!(
            out.contains("\t<define subnet=\"192.168.0.0/24\" address=\"192.168.0.1\" />\r\n</gameserver>")
        );
    }

    #[test]
    fn validation() {
        assert!(is_subnet("10.0.0.0/8") && is_subnet("0.0.0.0/0") && is_subnet("1.2.3.4/32"));
        assert!(
            !is_subnet("10.0.0.0")
                && !is_subnet("10.0.0.0/33")
                && !is_subnet("10.0.0/8")
                && !is_subnet("1.2.3.4/")
        );
        let mut f = IpConfigForm::load(XML).unwrap();
        f.set_default_address("nope");
        f.defines_mut()[0].set("subnet", "10.0.0.0/8");
        f.defines_mut()[1].set("address", "1.2.3");
        let issues = f.issues();
        assert!(
            issues
                .iter()
                .any(|i| i.row.is_none() && i.field == "address" && !i.warning)
        );
        assert!(
            issues
                .iter()
                .any(|i| i.row == Some(1) && i.field == "subnet" && i.warning)
        );
        assert!(
            issues
                .iter()
                .any(|i| i.row == Some(1) && i.field == "address" && !i.warning)
        );
    }
}
