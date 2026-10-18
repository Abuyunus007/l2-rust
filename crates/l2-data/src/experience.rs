//! Experience needed per level, and the highest player and pet levels.
//!
//! Java: `gameserver/data/xml/ExperienceData.java` and the `MaximumPlayerLevel` part of
//! `config/PlayerConfig.java`.
//! Data: `data/stats/players/experience.xml` (`xsd/experience.xsd`).
//!
//! Java keeps one level more than the highest player level: `maxLevel="85"` gives a table up to
//! level 86, because the exp for level 86 is the end of level 85's progress bar. The Rust getters
//! return the highest level itself (Java's `getMaxLevel() - 1`), so `max_level()` is what
//! `PlayerTemplates::load` and the player stats need.

use std::path::Path;

use serde::Deserialize;
use tracing::info;

use crate::loader::{DataError, load_file, log_loaded};

/// Java `PlayerConfig.PLAYER_MAXIMUM_LEVEL` with the default `MaximumPlayerLevel` (85), which Java
/// increments by one. Levels above it use the exp of this level. A config override comes with the
/// config loader; until then this is the cap.
pub const PLAYER_MAXIMUM_LEVEL: usize = 86;

/// Java `ExperienceData`: exp needed to reach each level, plus the level limits.
#[derive(Debug, Default)]
pub struct ExperienceTable {
    /// `exp[level]` is the total exp needed to reach `level`. Index 0 is unused, like Java.
    exp: Vec<i64>,
    max_level: Option<usize>,
    max_pet_level: Option<usize>,
}

impl ExperienceTable {
    const LOADER: &'static str = "ExperienceData";

    /// Loads `<root>/data/stats/players/experience.xml` and logs Java's count and max level lines.
    pub fn load(root: &Path) -> Self {
        let mut table = Self::default();
        load_file(
            &root.join("data/stats/players/experience.xml"),
            |doc: RawTable| table.add(doc),
        );
        log_loaded(Self::LOADER, table.exp.len().saturating_sub(1), "levels");
        if let Some(max) = table.max_level {
            info!("{}: Max Player Level is {max}.", Self::LOADER);
        }
        if let Some(max) = table.max_pet_level {
            info!("{}: Max Pet Level is {max}.", Self::LOADER);
        }
        table
    }

    /// Builds a table from XML text. For tests and tools.
    pub fn from_xml(xml: &str) -> Result<Self, DataError> {
        let doc = quick_xml::de::from_str(xml).map_err(|source| DataError::Xml {
            path: "<inline>".into(),
            source,
        })?;
        let mut table = Self::default();
        table.add(doc)?;
        Ok(table)
    }

    fn add(&mut self, doc: RawTable) -> Result<(), DataError> {
        // Java: _maxLevel = maxLevel + 1, capped at PLAYER_MAXIMUM_LEVEL.
        let level_limit = usize::try_from(doc.max_level + 1)
            .map_err(|_| DataError::Invalid(format!("negative maxLevel {}", doc.max_level)))?
            .min(PLAYER_MAXIMUM_LEVEL);
        // Java: _maxPetLevel = maxPetLevel + 1, but never above _maxLevel + 1.
        let pet_limit = usize::try_from(doc.max_pet_level + 1)
            .map_err(|_| DataError::Invalid(format!("negative maxPetLevel {}", doc.max_pet_level)))?
            .min(level_limit + 1);

        self.exp = vec![0; level_limit + 1];
        self.max_level = Some(level_limit - 1);
        self.max_pet_level = Some(pet_limit.saturating_sub(1));

        for entry in doc.experience {
            let level = usize::try_from(entry.level)
                .map_err(|_| DataError::Invalid(format!("negative experience level {}", entry.level)))?;
            // Java ignores levels above the cap or above the table, without an error.
            if level > PLAYER_MAXIMUM_LEVEL || level > level_limit {
                continue;
            }
            self.exp[level] = entry.to_level;
        }
        Ok(())
    }

    /// Java `getExpForLevel(level)`. Levels above the cap give the cap's exp. `None` when the
    /// table does not reach that level (Java throws) or the level is negative.
    pub fn exp_for_level(&self, level: i32) -> Option<i64> {
        let level = usize::try_from(level).ok()?.min(PLAYER_MAXIMUM_LEVEL);
        self.exp.get(level).copied()
    }

    /// Java `getExpForLevel(level + 1) - getExpForLevel(level)` progress, as the exp bar shows it
    /// (`UserInfo` writes it as a `float` widened to a double). `None` when the table has no entry.
    pub fn exp_percent(&self, exp: i64, level: i32) -> Option<f64> {
        let current = self.exp_for_level(level)?;
        let next = self.exp_for_level(level.saturating_add(1))?;
        Some(((exp - current) as f32 / (next - current) as f32) as f64)
    }

    /// The highest player level (Java `getMaxLevel() - 1`). `None` when no table was loaded.
    pub fn max_level(&self) -> Option<usize> {
        self.max_level
    }

    /// The highest pet level (Java `getMaxPetLevel() - 1`). `None` when no table was loaded.
    pub fn max_pet_level(&self) -> Option<usize> {
        self.max_pet_level
    }

    /// True when no table was loaded.
    pub fn is_empty(&self) -> bool {
        self.exp.is_empty()
    }
}

// Raw XML shape (xsd/experience.xsd).
#[derive(Deserialize)]
struct RawTable {
    #[serde(rename = "@maxLevel")]
    max_level: i32,
    #[serde(rename = "@maxPetLevel")]
    max_pet_level: i32,
    #[serde(rename = "experience", default)]
    experience: Vec<RawExperience>,
}

#[derive(Deserialize)]
struct RawExperience {
    #[serde(rename = "@level")]
    level: i32,
    #[serde(rename = "@tolevel")]
    to_level: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<table maxLevel="3" maxPetLevel="2">
	<experience level="1" tolevel="0" />
	<experience level="2" tolevel="68" />
	<experience level="3" tolevel="363" />
	<experience level="4" tolevel="1168" />
	<experience level="5" tolevel="2884" />
</table>"#;

    #[test]
    fn levels_above_the_table_are_ignored() {
        let table = ExperienceTable::from_xml(XML).unwrap();
        // maxLevel 3 gives a table up to level 4 (Java _maxLevel = 4).
        assert_eq!(table.max_level(), Some(3));
        assert_eq!(table.exp_for_level(4), Some(1168));
        assert_eq!(table.exp_for_level(5), None);
        assert_eq!(table.exp_for_level(-1), None);
    }

    #[test]
    fn pet_level_is_capped_by_the_player_level() {
        // Java: _maxPetLevel = 51, capped at _maxLevel + 1 = 5, so the highest pet level is 4.
        let xml = XML.replace("maxPetLevel=\"2\"", "maxPetLevel=\"50\"");
        let table = ExperienceTable::from_xml(&xml).unwrap();
        assert_eq!(table.max_pet_level(), Some(4));
    }

    #[test]
    fn max_level_is_capped_like_java() {
        let xml = XML.replace("maxLevel=\"3\"", "maxLevel=\"200\"");
        let table = ExperienceTable::from_xml(&xml).unwrap();
        assert_eq!(table.max_level(), Some(PLAYER_MAXIMUM_LEVEL - 1));
        // Above the cap, Java uses the exp of the cap.
        assert_eq!(table.exp_for_level(1000), table.exp_for_level(86));
    }

    #[test]
    fn percent_is_progress_through_the_level() {
        let table = ExperienceTable::from_xml(XML).unwrap();
        assert_eq!(table.exp_percent(68, 2), Some(0.0));
        // Level 1 runs from 0 to 68 exp.
        assert_eq!(table.exp_percent(34, 1), Some(0.5));
    }

    #[test]
    fn negative_level_is_an_error() {
        let xml = XML.replace("level=\"3\"", "level=\"-3\"");
        assert!(matches!(
            ExperienceTable::from_xml(&xml),
            Err(DataError::Invalid(_))
        ));
    }

    #[test]
    fn missing_max_level_is_an_error() {
        let xml = r#"<table maxPetLevel="2"><experience level="1" tolevel="0"/></table>"#;
        assert!(matches!(
            ExperienceTable::from_xml(xml),
            Err(DataError::Xml { .. })
        ));
    }
}
