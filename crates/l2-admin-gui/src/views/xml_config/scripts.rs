//! `Scripts.xml`: which script folders and files Java skips when it compiles the scripts.
//!
//! are compiled into the server (D-16), so the file has no effect on the Rust game server. The
//! view is read-only and only shows what is in the file.

use l2_config::xml_edit::{Edit, XmlDoc};

use super::XmlForm;
use super::model::Issue;

pub const FILE: &str = "Scripts.xml";

/// One `<exclude file="..">` with its `<include file="..">` children.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Exclusion {
    pub file: String,
    pub includes: Vec<String>,
}

pub struct ScriptsForm {
    pub excludes: Vec<Exclusion>,
}

impl ScriptsForm {
    pub fn load(text: &str) -> Result<Self, String> {
        let doc = XmlDoc::parse(text).map_err(|e| e.to_string())?;
        let excludes = doc
            .children_named(doc.root(), "exclude")
            .into_iter()
            .map(|e| Exclusion {
                file: doc.attr(e, "file").unwrap_or("").to_owned(),
                includes: doc
                    .children_named(e, "include")
                    .into_iter()
                    .map(|i| doc.attr(i, "file").unwrap_or("").to_owned())
                    .collect(),
            })
            .collect();
        Ok(Self { excludes })
    }
}

impl XmlForm for ScriptsForm {
    fn is_dirty(&self) -> bool {
        false
    }

    fn issues(&self) -> Vec<Issue> {
        Vec::new()
    }

    fn edits(&self, _doc: &XmlDoc) -> Vec<Edit> {
        Vec::new()
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        ui.label(
            "Read-only. The Rust scripts are compiled into the game server (a script change means a restart), so this Java file has no effect on it.",
        );
        ui.separator();
        if self.excludes.is_empty() {
            ui.label("The file excludes nothing.");
        }
        for exclusion in &self.excludes {
            if exclusion.includes.is_empty() {
                ui.label(format!("Skip: {}", exclusion.file));
            } else {
                ui.label(format!("Skip: {}, except:", exclusion.file));
                for include in &exclusion.includes {
                    ui.label(format!("      keep {include}"));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_excludes_and_includes_and_ignores_comments() {
        let xml = "<list>\n\t<exclude file=\"a.java\" />\n\t<!-- <exclude file=\"hidden\" /> -->\n\t<exclude file=\"dir\">\n\t\t<include file=\"x.java\" />\n\t</exclude>\n</list>";
        let form = ScriptsForm::load(xml).unwrap();
        assert_eq!(
            form.excludes,
            [
                Exclusion {
                    file: "a.java".into(),
                    includes: vec![]
                },
                Exclusion {
                    file: "dir".into(),
                    includes: vec!["x.java".into()]
                },
            ]
        );
        assert!(!form.is_dirty());
        assert!(form.edits(&XmlDoc::parse(xml).unwrap()).is_empty());
    }
}
