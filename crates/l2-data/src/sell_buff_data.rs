//! The skills a player may put up for sale in the buff shop.
//!
//! Data: `data/SellBuffData.xml` (`xsd/SellBuffData.xsd`).

use std::collections::BTreeSet;
use std::path::Path;

use serde::Deserialize;

use crate::loader::{DataError, load_file, log_loaded};

/// Java `SellBuffsManager.ALLOWED_BUFFS`.
#[derive(Debug, Clone, Default)]
pub struct SellBuffData {
    allowed: BTreeSet<i32>,
}

impl SellBuffData {
    /// Loads `<root>/data/SellBuffData.xml`.
    pub fn load(root: &Path) -> Self {
        let mut data = Self::default();
        load_file(&root.join("data/SellBuffData.xml"), |doc: RawList| {
            data.allowed.extend(doc.skills.into_iter().map(|s| s.id));
            Ok(())
        });
        log_loaded("SellBuffsManager", data.allowed.len(), "allowed buffs");
        data
    }

    /// Builds the table from XML text. For tests and tools.
    pub fn from_xml(xml: &str) -> Result<Self, DataError> {
        let doc: RawList = quick_xml::de::from_str(xml).map_err(|source| DataError::Xml {
            path: "<inline>".into(),
            source,
        })?;
        Ok(Self {
            allowed: doc.skills.into_iter().map(|s| s.id).collect(),
        })
    }

    /// Builds the table from skill ids. For tests.
    pub fn from_ids(ids: impl IntoIterator<Item = i32>) -> Self {
        Self {
            allowed: ids.into_iter().collect(),
        }
    }

    /// Whether the skill may be sold.
    pub fn is_allowed(&self, skill_id: i32) -> bool {
        self.allowed.contains(&skill_id)
    }

    /// Number of allowed skills.
    pub fn len(&self) -> usize {
        self.allowed.len()
    }

    /// True when nothing was loaded.
    pub fn is_empty(&self) -> bool {
        self.allowed.is_empty()
    }
}

#[derive(Deserialize)]
struct RawList {
    #[serde(rename = "skill", default)]
    skills: Vec<RawSkill>,
}

#[derive(Deserialize)]
struct RawSkill {
    #[serde(rename = "@id")]
    id: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_file_loads() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/game");
        if !root.join("data/SellBuffData.xml").is_file() {
            eprintln!("skipped: no dist/game snapshot");
            return;
        }
        let data = SellBuffData::load(&root);
        assert!(data.is_allowed(1232) && data.is_allowed(1003));
    }

    #[test]
    fn the_allowed_ids() {
        let data =
            SellBuffData::from_xml(r#"<list><skill id="1232" /> <!-- x --><skill id="1003" /></list>"#)
                .unwrap();
        assert!(data.is_allowed(1232) && data.is_allowed(1003) && !data.is_allowed(1));
        assert_eq!(data.len(), 2);
    }
}
