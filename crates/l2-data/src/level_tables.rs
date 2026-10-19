//! Two per-level tables of the player stats: the exp lost at death and the karma modifier.
//!
//! Java: `gameserver/data/xml/ExperienceLossData.java` and `gameserver/data/xml/KarmaLossData.java`.
//! Data: `data/stats/players/experienceLoss.xml` (`xsd/experienceLoss.xsd`) and
//! `data/stats/players/karmaLoss.xml` (`xsd/karmaLoss.xsd`).
//!
//! Both are an array of `PLAYER_MAXIMUM_LEVEL` doubles (86 with the default `MaximumPlayerLevel`,
//! see [`crate::experience::PLAYER_MAXIMUM_LEVEL`]) filled with 1.0, then overwritten by the file.
//! A level at or above the length is ignored by the loader and answered with the last entry by the
//! getter.

use std::path::Path;

use serde::Deserialize;
use tracing::warn;

use crate::experience::PLAYER_MAXIMUM_LEVEL;
use crate::loader::{DataError, load_file, log_loaded};

/// One of the two tables: a value per level, 1.0 where the file says nothing.
#[derive(Debug, Clone, PartialEq)]
struct LevelValues(Vec<f64>);

impl Default for LevelValues {
    fn default() -> Self {
        // Java `Arrays.fill(array, 1d)` (before the file is read).
        Self(vec![1.0; PLAYER_MAXIMUM_LEVEL])
    }
}

impl LevelValues {
    fn set(&mut self, level: i32, value: f64) {
        // Java: `if (level >= PLAYER_MAXIMUM_LEVEL) return;`. A negative level would throw there.
        if let Ok(level) = usize::try_from(level)
            && let Some(slot) = self.0.get_mut(level)
        {
            *slot = value;
        }
    }

    fn get(&self, level: i32, what: &str) -> f64 {
        let last = self.0.len() - 1;
        let Ok(index) = usize::try_from(level) else {
            // Java throws `ArrayIndexOutOfBoundsException`; a level below 0 does not exist.
            return self.0[0];
        };
        if index >= self.0.len() {
            warn!("{what}: Requested too high level ({level}).");
            return self.0[last];
        }
        self.0[index]
    }
}

/// Java `ExperienceLossData`: the percentage of a level's exp a death costs.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ExperienceLoss(LevelValues);

impl ExperienceLoss {
    const LOADER: &'static str = "ExperienceLossData";

    /// Loads `<root>/data/stats/players/experienceLoss.xml` and logs Java's count line.
    pub fn load(root: &Path) -> Self {
        let mut table = Self::default();
        load_file(
            &root.join("data/stats/players/experienceLoss.xml"),
            |doc: RawExperienceLoss| {
                table.add(doc);
                Ok(())
            },
        );
        log_loaded(Self::LOADER, table.0.0.len() - 1, "levels");
        table
    }

    /// Builds the table from XML text. For tests and tools.
    pub fn from_xml(xml: &str) -> Result<Self, DataError> {
        let doc = quick_xml::de::from_str(xml).map_err(|source| DataError::Xml {
            path: "<inline>".into(),
            source,
        })?;
        let mut table = Self::default();
        table.add(doc);
        Ok(table)
    }

    fn add(&mut self, doc: RawExperienceLoss) {
        for entry in doc.entries {
            self.0.set(entry.level, entry.val);
        }
    }

    /// Java `getPercentLost(level)`. A level at or above the table length gives the last entry.
    pub fn percent_lost(&self, level: i32) -> f64 {
        self.0.get(level, Self::LOADER)
    }
}

/// Java `KarmaLossData`: the divisor of the karma a player loses for exp.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct KarmaLoss(LevelValues);

impl KarmaLoss {
    const LOADER: &'static str = "KarmaLossData";

    /// Loads `<root>/data/stats/players/karmaLoss.xml` and logs Java's count line.
    pub fn load(root: &Path) -> Self {
        let mut table = Self::default();
        load_file(
            &root.join("data/stats/players/karmaLoss.xml"),
            |doc: RawKarmaLoss| {
                table.add(doc);
                Ok(())
            },
        );
        log_loaded(Self::LOADER, table.0.0.len() - 1, "modifiers");
        table
    }

    /// Builds the table from XML text. For tests and tools.
    pub fn from_xml(xml: &str) -> Result<Self, DataError> {
        let doc = quick_xml::de::from_str(xml).map_err(|source| DataError::Xml {
            path: "<inline>".into(),
            source,
        })?;
        let mut table = Self::default();
        table.add(doc);
        Ok(table)
    }

    fn add(&mut self, doc: RawKarmaLoss) {
        for entry in doc.entries {
            self.0.set(entry.level, entry.val);
        }
    }

    /// Java `getModifier(level)`. A level at or above the table length gives the last entry.
    pub fn modifier(&self, level: i32) -> f64 {
        self.0.get(level, Self::LOADER)
    }
}

// Raw XML shape (xsd/experienceLoss.xsd, xsd/karmaLoss.xsd).
#[derive(Deserialize)]
struct RawExperienceLoss {
    #[serde(rename = "experience", default)]
    entries: Vec<RawEntry>,
}

#[derive(Deserialize)]
struct RawKarmaLoss {
    #[serde(rename = "modifier", default)]
    entries: Vec<RawEntry>,
}

#[derive(Deserialize)]
struct RawEntry {
    #[serde(rename = "@level")]
    level: i32,
    #[serde(rename = "@val")]
    val: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOSS: &str = r#"<?xml version="1.0"?><list>
        <experience level="1" val="10.0" />
        <experience level="2" val="9.875" />
        <experience level="85" val="1.0" />
        <experience level="86" val="7.0" />
    </list>"#;

    #[test]
    fn experience_loss_reads_levels_and_defaults_to_one() {
        let table = ExperienceLoss::from_xml(LOSS).unwrap();
        assert_eq!(table.percent_lost(1), 10.0);
        assert_eq!(table.percent_lost(2), 9.875);
        // Not listed: Java's fill value.
        assert_eq!(table.percent_lost(3), 1.0);
        assert_eq!(table.percent_lost(0), 1.0);
    }

    #[test]
    fn levels_at_the_table_length_are_ignored_and_answered_with_the_last_entry() {
        let table = ExperienceLoss::from_xml(LOSS).unwrap();
        // The table has 86 entries (0..=85): level 86 is not stored, a lookup gives level 85.
        assert_eq!(table.percent_lost(86), table.percent_lost(85));
        assert_eq!(table.percent_lost(120), 1.0);
    }

    #[test]
    fn karma_loss_reads_modifiers() {
        let table = KarmaLoss::from_xml(
            r#"<list><modifier level="1" val="0.772184315" /><modifier level="85" val="35.25374225" /></list>"#,
        )
        .unwrap();
        assert_eq!(table.modifier(1), 0.772184315);
        assert_eq!(table.modifier(85), 35.25374225);
        assert_eq!(table.modifier(50), 1.0);
        assert_eq!(table.modifier(99), 35.25374225);
    }
}
