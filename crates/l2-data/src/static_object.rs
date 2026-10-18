//! Static objects: town maps, thrones and signboards from `data/StaticObjects.xml`.
//!
//! Java: `gameserver/data/xml/StaticObjectData.java`, `entity/actor/instance/StaticObject.java`.
//! Data: `data/StaticObjects.xml`, `xsd/StaticObjects.xsd`.
//!
//! `id`, `name`, `x`, `y` and `z` are required; `type` defaults to 0, `texture` to `none`,
//! `map_x` / `map_y` to 0. Java spawns one `StaticObject` per element (each gets its own object
//! id from the id manager, the XML `id` is the client-side static object id), so the count is the
//! number of elements.
//!
//! NOTE(java-quirk): as in `door.rs`, only the broken element is skipped here, not the rest of the
//! file.

use std::path::Path;

use serde::Deserialize;
use tracing::warn;

use crate::loader::{DataError, load_file, log_loaded};

/// One `<object>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticObjectTemplate {
    /// `id`: `StaticObject.getId()`, the id the client knows the object by.
    pub id: i32,
    pub name: String,
    /// `type`: 0 map signs, 1 thrones, 2 arena signs, 3 (unused by Java's code).
    pub kind: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// `texture` (default `none`): the town map texture, `ShowTownMap("town_map." + texture)`.
    pub texture: String,
    pub map_x: i32,
    pub map_y: i32,
}

/// All static objects. Immutable once loaded.
#[derive(Debug, Clone, Default)]
pub struct StaticObjectTemplates {
    objects: Vec<StaticObjectTemplate>,
}

impl StaticObjectTemplates {
    /// Loads `data/StaticObjects.xml` below `root`, logging Java's count line.
    pub fn load(root: &Path) -> Self {
        let mut table = Self::default();
        load_file::<RawFile, _>(&root.join("data").join("StaticObjects.xml"), |doc| {
            table.add_document(doc);
            Ok(())
        });
        log_loaded("StaticObjectData", table.len(), "static object templates");
        table
    }

    /// Parses one `StaticObjects.xml` text (tests, tools).
    pub fn from_xml(text: &str) -> Result<Self, DataError> {
        let doc = quick_xml::de::from_str(text).map_err(|source| DataError::Xml {
            path: "<inline>".into(),
            source,
        })?;
        let mut table = Self::default();
        table.add_document(doc);
        Ok(table)
    }

    fn add_document(&mut self, doc: RawFile) {
        for child in doc.children {
            let RawChild::Object(raw) = child else { continue };
            match build_object(&raw) {
                Ok(object) => self.objects.push(object),
                Err(error) => warn!("StaticObjectData: {error}"),
            }
        }
    }

    pub fn len(&self) -> usize {
        self.objects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    /// In file order, the order Java spawns them.
    pub fn iter(&self) -> impl Iterator<Item = &StaticObjectTemplate> {
        self.objects.iter()
    }
}

#[derive(Deserialize)]
struct RawFile {
    #[serde(rename = "$value", default)]
    children: Vec<RawChild>,
}

#[derive(Deserialize)]
enum RawChild {
    #[serde(rename = "object")]
    Object(Box<RawObject>),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct RawObject {
    #[serde(rename = "@id")]
    id: Option<String>,
    #[serde(rename = "@name")]
    name: Option<String>,
    #[serde(rename = "@type")]
    kind: Option<String>,
    #[serde(rename = "@x")]
    x: Option<String>,
    #[serde(rename = "@y")]
    y: Option<String>,
    #[serde(rename = "@z")]
    z: Option<String>,
    #[serde(rename = "@texture")]
    texture: Option<String>,
    #[serde(rename = "@map_x")]
    map_x: Option<String>,
    #[serde(rename = "@map_y")]
    map_y: Option<String>,
}

fn int(key: &str, value: &Option<String>, default: Option<i32>) -> Result<i32, String> {
    match (value, default) {
        (Some(text), _) => text
            .parse()
            .map_err(|_| format!("Integer value required, but found: {text}")),
        (None, Some(default)) => Ok(default),
        (None, None) => Err(format!("Integer value required, but not specified: {key}!")),
    }
}

// Java: StaticObjectData.parseDocument + addObject
fn build_object(raw: &RawObject) -> Result<StaticObjectTemplate, String> {
    let id = int("id", &raw.id, None)?;
    let kind = int("type", &raw.kind, Some(0))?;
    let name = raw
        .name
        .clone()
        .ok_or_else(|| format!("object {id}: String value required, but not specified"))?;
    let map_x = int("map_x", &raw.map_x, Some(0))?;
    let map_y = int("map_y", &raw.map_y, Some(0))?;
    let x = int("x", &raw.x, None)?;
    let y = int("y", &raw.y, None)?;
    let z = int("z", &raw.z, None)?;
    Ok(StaticObjectTemplate {
        id,
        name,
        kind,
        x,
        y,
        z,
        texture: raw.texture.clone().unwrap_or_else(|| "none".to_owned()),
        map_x,
        map_y,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults() {
        let t = StaticObjectTemplates::from_xml(
            r#"<list>
              <object id="17250001" name="map" type="0" x="1" y="2" z="3" texture="t" map_x="5" map_y="6"/>
              <object id="19210010" name="throne" x="-1" y="-2" z="-3"/>
              <object id="1" x="0" y="0" z="0"/>
            </list>"#,
        )
        .unwrap();
        assert_eq!(t.len(), 2, "an object without a name is skipped");
        let v: Vec<_> = t.iter().collect();
        assert_eq!((v[0].texture.as_str(), v[0].map_x, v[0].map_y), ("t", 5, 6));
        assert_eq!((v[1].kind, v[1].texture.as_str(), v[1].map_x), (0, "none", 0));
    }
}
