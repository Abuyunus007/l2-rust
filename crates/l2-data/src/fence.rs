//! Fences: the walls `data/FenceData.xml` puts into the world (arena fences).
//!
//! Java: `gameserver/data/xml/FenceData.java` (`parseDocument`, `spawnFence`),
//! `entity/actor/enums/creature/FenceState.java`. Data: `data/FenceData.xml`, `xsd/FenceData.xsd`.
//!
//! `x`, `y`, `z`, `name`, `width`, `length` and `height` are required (Java's `getInt` /
//! `getString` throw without them), `state` defaults to `CLOSED` and must be an exact enum name.
//!
//! NOTE(java-quirk): as in `door.rs`, Java aborts the rest of the file at the first bad fence;
//! here only the broken fence is skipped, with a warning.

use std::path::Path;

use serde::Deserialize;
use tracing::warn;

use crate::loader::{DataError, load_file, log_loaded};

/// Java `FenceState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FenceState {
    /// `HIDDEN(0)`
    Hidden,
    /// `OPENED(1)`
    Opened,
    /// `CLOSED(2)`
    Closed,
    /// `CLOSED_HIDDEN(0)`
    ClosedHidden,
}

impl FenceState {
    /// `Enum.valueOf(FenceState.class, name)`: exact and case sensitive.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "HIDDEN" => Some(Self::Hidden),
            "OPENED" => Some(Self::Opened),
            "CLOSED" => Some(Self::Closed),
            "CLOSED_HIDDEN" => Some(Self::ClosedHidden),
            _ => None,
        }
    }

    /// `getClientId()`: what `ExColosseumFenceInfo` carries.
    pub fn client_id(self) -> i32 {
        match self {
            Self::Hidden | Self::ClosedHidden => 0,
            Self::Opened => 1,
            Self::Closed => 2,
        }
    }

    /// `isGeodataEnabled()`: the fence blocks movement and sight.
    pub fn is_geodata_enabled(self) -> bool {
        matches!(self, Self::ClosedHidden | Self::Closed)
    }
}

/// One `<fence>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FenceTemplate {
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub width: i32,
    pub length: i32,
    /// How many stacked fence pieces the client shows (`Fence` allocates `height - 1` extra ids).
    pub height: i32,
    pub state: FenceState,
}

/// All fences of `FenceData.xml`. Immutable once loaded.
#[derive(Debug, Clone, Default)]
pub struct FenceTemplates {
    fences: Vec<FenceTemplate>,
}

impl FenceTemplates {
    /// Loads `data/FenceData.xml` below `root`, logging Java's count line.
    pub fn load(root: &Path) -> Self {
        let mut table = Self::default();
        load_file::<RawFenceFile, _>(&root.join("data").join("FenceData.xml"), |doc| {
            table.add_document(doc);
            Ok(())
        });
        log_loaded("FenceData", table.len(), "fences");
        table
    }

    /// Parses one `FenceData.xml` text (tests, tools).
    pub fn from_xml(text: &str) -> Result<Self, DataError> {
        let doc = quick_xml::de::from_str(text).map_err(|source| DataError::Xml {
            path: "<inline>".into(),
            source,
        })?;
        let mut table = Self::default();
        table.add_document(doc);
        Ok(table)
    }

    fn add_document(&mut self, doc: RawFenceFile) {
        for child in doc.children {
            let RawFenceChild::Fence(raw) = child else {
                continue;
            };
            match build_fence(&raw) {
                Ok(fence) => self.fences.push(fence),
                Err(error) => warn!("FenceData: {error}"),
            }
        }
    }

    /// Java's "Loaded N fences".
    pub fn len(&self) -> usize {
        self.fences.len()
    }

    pub fn is_empty(&self) -> bool {
        self.fences.is_empty()
    }

    /// In file order, the order Java spawns them.
    pub fn iter(&self) -> impl Iterator<Item = &FenceTemplate> {
        self.fences.iter()
    }
}

#[derive(Deserialize)]
struct RawFenceFile {
    #[serde(rename = "$value", default)]
    children: Vec<RawFenceChild>,
}

#[derive(Deserialize)]
enum RawFenceChild {
    #[serde(rename = "fence")]
    Fence(RawFence),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct RawFence {
    #[serde(rename = "@name")]
    name: Option<String>,
    #[serde(rename = "@x")]
    x: Option<String>,
    #[serde(rename = "@y")]
    y: Option<String>,
    #[serde(rename = "@z")]
    z: Option<String>,
    #[serde(rename = "@width")]
    width: Option<String>,
    #[serde(rename = "@length")]
    length: Option<String>,
    #[serde(rename = "@height")]
    height: Option<String>,
    #[serde(rename = "@state")]
    state: Option<String>,
}

fn req_int(key: &str, value: &Option<String>) -> Result<i32, String> {
    match value {
        None => Err(format!("Integer value required, but not specified: {key}!")),
        Some(text) => text
            .parse()
            .map_err(|_| format!("Integer value required, but found: {text}!")),
    }
}

// Java: FenceData.spawnFence(Node)
fn build_fence(raw: &RawFence) -> Result<FenceTemplate, String> {
    let x = req_int("x", &raw.x)?;
    let y = req_int("y", &raw.y)?;
    let z = req_int("z", &raw.z)?;
    let name = raw
        .name
        .clone()
        .ok_or_else(|| "String value required, but not specified".to_owned())?;
    let width = req_int("width", &raw.width)?;
    let length = req_int("length", &raw.length)?;
    let height = req_int("height", &raw.height)?;
    let state = match &raw.state {
        None => FenceState::Closed,
        Some(text) => FenceState::from_name(text)
            .ok_or_else(|| format!("Enum value of type FenceState required, but found: {text}"))?,
    };
    Ok(FenceTemplate {
        name,
        x,
        y,
        z,
        width,
        length,
        height,
        state,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_defaults_to_closed_and_bad_fences_are_skipped() {
        let t = FenceTemplates::from_xml(
            r#"<list>
              <fence name="a" x="1" y="2" z="3" width="100" length="200" height="3"/>
              <fence name="b" x="1" y="2" z="3" width="100" length="200" height="1" state="HIDDEN"/>
              <fence name="c" x="1" y="2" z="3" width="100" length="200" height="1" state="hidden"/>
              <fence name="d" x="1" y="2" z="3" width="100" length="200"/>
            </list>"#,
        )
        .unwrap();
        let v: Vec<_> = t.iter().collect();
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].state, FenceState::Closed);
        assert_eq!(v[1].state, FenceState::Hidden);
    }

    #[test]
    fn state_flags() {
        assert!(FenceState::Closed.is_geodata_enabled());
        assert!(FenceState::ClosedHidden.is_geodata_enabled());
        assert!(!FenceState::Opened.is_geodata_enabled());
        assert!(!FenceState::Hidden.is_geodata_enabled());
        assert_eq!(FenceState::Closed.client_id(), 2);
        assert_eq!(FenceState::ClosedHidden.client_id(), 0);
    }
}
