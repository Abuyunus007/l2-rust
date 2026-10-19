//! Hit condition bonus: the percentage an attack gains or loses from height, side, night and rain.
//!
//! Java: `gameserver/data/xml/HitConditionBonusData.java`. The formula (`getConditionBonus`)
//! belongs to the combat formulas (M4), it is not here.
//! Data: `data/stats/hitConditionBonus.xml` (`xsd/hitConditionBonus.xsd`).

use std::path::Path;

use serde::Deserialize;
use tracing::info;

use crate::loader::{DataError, load_file};

/// The seven percentages. Every value is 0 when its element is missing (Java field defaults).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HitConditionBonus {
    /// `front`: attacking an enemy from the front.
    pub front: i32,
    /// `side`: attacking an enemy from the side.
    pub side: i32,
    /// `back`: attacking an enemy from behind.
    pub back: i32,
    /// `high`: attacking from a higher place.
    pub high: i32,
    /// `low`: attacking from a lower place.
    pub low: i32,
    /// `dark`: attacking at night.
    pub dark: i32,
    /// `rain`: attacking in the rain. Loaded but unused, like Java (it has no rain support).
    pub rain: i32,
}

impl HitConditionBonus {
    const LOADER: &'static str = "HitConditionBonusData";

    /// Loads `<root>/data/stats/hitConditionBonus.xml` and logs Java's line
    /// (`HitConditionBonusData: Loaded hit condition bonuses.`, no count).
    pub fn load(root: &Path) -> Self {
        let mut table = Self::default();
        load_file(
            &root.join("data/stats/hitConditionBonus.xml"),
            |doc: RawHitConditionBonus| {
                table.add(doc);
                Ok(())
            },
        );
        info!("{}: Loaded hit condition bonuses.", Self::LOADER);
        table
    }

    /// Builds the values from XML text. For tests and tools.
    pub fn from_xml(xml: &str) -> Result<Self, DataError> {
        let doc = quick_xml::de::from_str(xml).map_err(|source| DataError::Xml {
            path: "<inline>".into(),
            source,
        })?;
        let mut table = Self::default();
        table.add(doc);
        Ok(table)
    }

    fn add(&mut self, doc: RawHitConditionBonus) {
        // Java reads each element as it comes, so a repeated element keeps its last value.
        let last = |values: &[RawValue]| values.last().map(|v| v.val);
        if let Some(v) = last(&doc.front) {
            self.front = v;
        }
        if let Some(v) = last(&doc.side) {
            self.side = v;
        }
        if let Some(v) = last(&doc.back) {
            self.back = v;
        }
        if let Some(v) = last(&doc.high) {
            self.high = v;
        }
        if let Some(v) = last(&doc.low) {
            self.low = v;
        }
        if let Some(v) = last(&doc.dark) {
            self.dark = v;
        }
        if let Some(v) = last(&doc.rain) {
            self.rain = v;
        }
    }
}

// ---- Raw XML shape (xsd/hitConditionBonus.xsd) ----------------------------------------------

#[derive(Deserialize)]
struct RawHitConditionBonus {
    #[serde(default)]
    front: Vec<RawValue>,
    #[serde(default)]
    side: Vec<RawValue>,
    #[serde(default)]
    back: Vec<RawValue>,
    #[serde(default)]
    high: Vec<RawValue>,
    #[serde(default)]
    low: Vec<RawValue>,
    #[serde(default)]
    dark: Vec<RawValue>,
    #[serde(default)]
    rain: Vec<RawValue>,
}

#[derive(Deserialize)]
struct RawValue {
    #[serde(rename = "@val")]
    val: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_elements_are_zero() {
        let table = HitConditionBonus::from_xml(r#"<hitConditionBonus><back val="10"/></hitConditionBonus>"#)
            .unwrap();
        assert_eq!(table.back, 10);
        assert_eq!(table.front, 0);
        assert_eq!(table.rain, 0);
    }
}
