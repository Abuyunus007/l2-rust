//! Door templates: the geometry and defaults of every door in `data/Doors.xml`.
//!
//! Java: `gameserver/data/xml/DoorData.java` (`parseDocument`, `insertCollisionData`),
//! `entity/actor/templates/DoorTemplate.java`. Data: `data/Doors.xml`, `xsd/Doors.xsd`.
//!
//! Java reads every `<door>` into a `StatSet` (all attribute values are text) and `DoorTemplate`
//! then reads typed values back with its own defaults; the same flow is used here, so the quirks
//! come for free: `Integer.parseInt` does not trim, `Boolean.parseBoolean` is true only for
//! `true` in any case, a missing required attribute (`id`, `name`, `pos`, `height`, `nodeZ`,
//! `node1`..`node4`) fails the door.
//!
//! What the world needs from a door is in [`DoorTemplate`]: the four corner nodes
//! (`DynamicObstacles` / `checkIfDoorsBetween` use them), the z range, the default state, hp and
//! the packet fields. Only the door attributes the shipped file uses from Java's
//! `CreatureTemplate` are read (`baseHpMax`, `basePDef`, `baseMDef`); the others (speeds, attack
//! stats...) mean nothing for a door.
//!
//! A door id that appears twice is spawned twice by Java but only the last one is found by id
//! (`_doors.put`); [`DoorTemplates::iter`] yields every door in file order (what the world
//! spawns), [`DoorTemplates::get`] and [`DoorTemplates::len`] follow the map.
//!
//! NOTE(java-quirk): in Java any exception (a missing attribute, a non-numeric value) aborts the
//! *rest of the file*, silently losing every later door. Here only the broken door is skipped,
//! with a warning. The shipped data has no such door.

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;
use tracing::warn;

use crate::loader::{DataError, load_file, log_loaded};

/// `Door.OPEN_BY_CLICK`.
pub const OPEN_BY_CLICK: i32 = 1;
/// `Door.OPEN_BY_TIME`.
pub const OPEN_BY_TIME: i32 = 2;
/// `Door.OPEN_BY_ITEM`.
pub const OPEN_BY_ITEM: i32 = 4;
/// `Door.OPEN_BY_SKILL`.
pub const OPEN_BY_SKILL: i32 = 8;
/// `Door.OPEN_BY_CYCLE`.
pub const OPEN_BY_CYCLE: i32 = 16;

/// One `<door>`. Java `DoorTemplate` (plus the collision values `DoorData.insertCollisionData`
/// adds to its `StatSet`).
#[derive(Debug, Clone, PartialEq)]
pub struct DoorTemplate {
    /// `id`: `Door.getId()`.
    pub id: i32,
    pub name: String,
    /// `node1`..`node4`: the corners of the door's footprint, in order.
    pub node_x: [i32; 4],
    pub node_y: [i32; 4],
    /// `nodeZ`: the lowest z of the door (`Door.getZMin()`).
    pub node_z: i32,
    /// `height`: `getZMax() = nodeZ + height`.
    pub height: i32,
    /// `pos`: where the door is spawned. `pos_z` is `min(pos z, nodeZ)`.
    pub pos_x: i32,
    pub pos_y: i32,
    pub pos_z: i32,
    /// `emitter_id` (default 0): the client effect id of `OnEventTrigger`.
    pub emitter: i32,
    /// `child_id_event` (default -1): a door told about this door's open / close.
    pub child_door_id: i32,
    /// `group` (default none): doors that open and close together.
    pub group_name: Option<String>,
    /// `hp_showable` (default true).
    pub show_hp: bool,
    /// `is_wall` (default false).
    pub is_wall: bool,
    /// `master_close_event`: 1 `act_open`, -1 `act_close`, else 0.
    pub master_door_close: i8,
    /// `master_open_event`: 1 `act_open`, -1 `act_close`, else 0.
    pub master_door_open: i8,
    /// `targetable` (default true).
    pub targetable: bool,
    /// `default_status="open"` (default closed).
    pub open_by_default: bool,
    /// `open_time` (only read when [`OPEN_BY_TIME`] is set, else 0), seconds.
    pub open_time: i32,
    /// `random_time` (only read with [`OPEN_BY_TIME`], default -1, else 0), seconds.
    pub random_time: i32,
    /// `close_time` (default -1), seconds.
    pub close_time: i32,
    /// `level` (default 0).
    pub level: i32,
    /// `open_method` (default 0): the `OPEN_BY_*` bit mask.
    pub open_type: i32,
    /// `check_collision` (default true): false = the door never blocks movement or sight.
    pub check_collision: bool,
    /// `is_attackable` (default false).
    pub attackable: bool,
    /// `clanhall_id` (default 0).
    pub clanhall_id: i32,
    /// `stealth` (default false).
    pub stealth: bool,
    /// `baseHpMax` (a Java `float`; DoorData presets 1 so a door without hp is not born dead).
    pub base_hp_max: f32,
    /// `basePDef` (default 0).
    pub base_p_def: i32,
    /// `baseMDef` (default 0).
    pub base_m_def: i32,
    /// `collisionRadius` as `DoorData.insertCollisionData` computes it:
    /// `min(|node1.x - node2.x|, |node1.y - node2.y|)`, at least 20.
    pub collision_radius: i32,
    /// `collisionHeight`: the `height`.
    pub collision_height: i32,
}

impl DoorTemplate {
    /// `isOpenableBySkill()` and friends.
    pub fn has_open_method(&self, method: i32) -> bool {
        (self.open_type & method) == method
    }
}

/// All doors of `Doors.xml`. Immutable once loaded.
#[derive(Debug, Clone, Default)]
pub struct DoorTemplates {
    /// In file order: what Java spawns, one `Door` per entry.
    doors: Vec<DoorTemplate>,
    /// Door id to its index; a repeated id points at the last one (`DoorData._doors.put`).
    by_id: HashMap<i32, usize>,
}

impl DoorTemplates {
    /// Loads `data/Doors.xml` below `root`, logging Java's count line.
    pub fn load(root: &Path) -> Self {
        let mut table = Self::default();
        load_file::<RawDoorFile, _>(&root.join("data").join("Doors.xml"), |doc| {
            table.add_document(doc);
            Ok(())
        });
        log_loaded("DoorData", table.len(), "doors");
        table
    }

    /// Parses one `Doors.xml` text (tests, tools).
    pub fn from_xml(text: &str) -> Result<Self, DataError> {
        let doc = quick_xml::de::from_str(text).map_err(|source| DataError::Xml {
            path: "<inline>".into(),
            source,
        })?;
        let mut table = Self::default();
        table.add_document(doc);
        Ok(table)
    }

    fn add_document(&mut self, doc: RawDoorFile) {
        for child in doc.children {
            let RawDoorChild::Door(raw) = child else { continue };
            match build_door(&raw) {
                Ok(door) => {
                    self.by_id.insert(door.id, self.doors.len());
                    self.doors.push(door);
                }
                Err(error) => warn!("DoorData: {error}"),
            }
        }
    }

    /// Java's "Loaded N doors": the number of distinct door ids.
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.doors.is_empty()
    }

    /// Every door in file order, repeated ids included.
    pub fn iter(&self) -> impl Iterator<Item = &DoorTemplate> {
        self.doors.iter()
    }

    /// `DoorData.getDoorTemplate(id)`.
    pub fn get(&self, id: i32) -> Option<&DoorTemplate> {
        self.by_id.get(&id).map(|&i| &self.doors[i])
    }
}

// Raw XML shape (xsd/Doors.xsd). Every attribute is text, like Java's StatSet.
#[derive(Deserialize)]
struct RawDoorFile {
    #[serde(rename = "$value", default)]
    children: Vec<RawDoorChild>,
}

#[derive(Deserialize)]
enum RawDoorChild {
    #[serde(rename = "door")]
    Door(Box<RawDoor>),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct RawDoor {
    #[serde(rename = "@id")]
    id: Option<String>,
    #[serde(rename = "@name")]
    name: Option<String>,
    #[serde(rename = "@pos")]
    pos: Option<String>,
    #[serde(rename = "@height")]
    height: Option<String>,
    #[serde(rename = "@nodeZ")]
    node_z: Option<String>,
    #[serde(rename = "@node1")]
    node1: Option<String>,
    #[serde(rename = "@node2")]
    node2: Option<String>,
    #[serde(rename = "@node3")]
    node3: Option<String>,
    #[serde(rename = "@node4")]
    node4: Option<String>,
    #[serde(rename = "@emitter_id")]
    emitter_id: Option<String>,
    #[serde(rename = "@hp_showable")]
    hp_showable: Option<String>,
    #[serde(rename = "@is_wall")]
    is_wall: Option<String>,
    #[serde(rename = "@group")]
    group: Option<String>,
    #[serde(rename = "@child_id_event")]
    child_id_event: Option<String>,
    #[serde(rename = "@master_close_event")]
    master_close_event: Option<String>,
    #[serde(rename = "@master_open_event")]
    master_open_event: Option<String>,
    #[serde(rename = "@targetable")]
    targetable: Option<String>,
    #[serde(rename = "@default_status")]
    default_status: Option<String>,
    #[serde(rename = "@close_time")]
    close_time: Option<String>,
    #[serde(rename = "@level")]
    level: Option<String>,
    #[serde(rename = "@open_method")]
    open_method: Option<String>,
    #[serde(rename = "@check_collision")]
    check_collision: Option<String>,
    #[serde(rename = "@open_time")]
    open_time: Option<String>,
    #[serde(rename = "@random_time")]
    random_time: Option<String>,
    #[serde(rename = "@is_attackable")]
    is_attackable: Option<String>,
    #[serde(rename = "@clanhall_id")]
    clanhall_id: Option<String>,
    #[serde(rename = "@stealth")]
    stealth: Option<String>,
    #[serde(rename = "@baseHpMax")]
    base_hp_max: Option<String>,
    #[serde(rename = "@basePDef")]
    base_p_def: Option<String>,
    #[serde(rename = "@baseMDef")]
    base_m_def: Option<String>,
}

/// `StatSet.getInt(key)`: required.
fn req_int(key: &str, value: &Option<String>) -> Result<i32, String> {
    match value {
        None => Err(format!("Integer value required, but not specified: {key}!")),
        Some(text) => text
            .parse()
            .map_err(|_| format!("Integer value required, but found: {text}!")),
    }
}

/// `StatSet.getInt(key, default)`.
fn opt_int(value: &Option<String>, default: i32) -> Result<i32, String> {
    match value {
        None => Ok(default),
        Some(text) => text
            .parse()
            .map_err(|_| format!("Integer value required, but found: {text}")),
    }
}

/// `StatSet.getBoolean(key, default)`: `Boolean.parseBoolean`.
fn opt_bool(value: &Option<String>, default: bool) -> bool {
    value
        .as_ref()
        .map_or(default, |text| text.eq_ignore_ascii_case("true"))
}

/// `"x,y".split(",")` read with `Integer.parseInt`, as `DoorTemplate` does for `node1`..`node4`.
fn node(key: &str, value: &Option<String>) -> Result<(i32, i32), String> {
    let text = value
        .as_ref()
        .ok_or_else(|| format!("String value required, but not specified: {key}"))?;
    let mut parts = text.split(',');
    let mut next = || -> Result<i32, String> {
        parts
            .next()
            .ok_or_else(|| format!("bad coordinates in {key}: {text}"))?
            .parse()
            .map_err(|_| format!("bad coordinates in {key}: {text}"))
    };
    let x = next()?;
    let y = next()?;
    Ok((x, y))
}

/// `master_close_event` / `master_open_event`.
fn master_event(value: &Option<String>) -> i8 {
    match value.as_deref() {
        Some("act_open") => 1,
        Some("act_close") => -1,
        _ => 0,
    }
}

// Java: DoorData.parseDocument + insertCollisionData + DoorTemplate constructor
fn build_door(raw: &RawDoor) -> Result<DoorTemplate, String> {
    let id = req_int("id", &raw.id)?;
    let name = raw
        .name
        .clone()
        .ok_or_else(|| "String value required, but not specified".to_owned())
        .map_err(|e| format!("door {id}: {e}"))?;
    build_inner(raw, id, name).map_err(|e| format!("door {id}: {e}"))
}

fn build_inner(raw: &RawDoor, id: i32, name: String) -> Result<DoorTemplate, String> {
    // insertCollisionData runs first in Java (node1, node2, height).
    let height = req_int("height", &raw.height)?;
    let (n1x, n1y) = node("node1", &raw.node1)?;
    let (n2x, n2y) = node("node2", &raw.node2)?;
    let collision_radius = (n1x.wrapping_sub(n2x))
        .wrapping_abs()
        .min(n1y.wrapping_sub(n2y).wrapping_abs())
        .max(20);

    // DoorTemplate constructor, in its order.
    let pos_text = raw
        .pos
        .as_ref()
        .ok_or_else(|| "String value required, but not specified".to_owned())?;
    let pos: Vec<&str> = pos_text.split(',').collect();
    let node_z = req_int("nodeZ", &raw.node_z)?;
    let (n3x, n3y) = node("node3", &raw.node3)?;
    let (n4x, n4y) = node("node4", &raw.node4)?;
    let coord = |i: usize| -> Result<i32, String> {
        pos.get(i)
            .ok_or_else(|| format!("bad pos: {pos_text}"))?
            .parse()
            .map_err(|_| format!("bad pos: {pos_text}"))
    };
    let (pos_x, pos_y, pos_z) = (coord(0)?, coord(1)?, coord(2)?);

    let open_type = opt_int(&raw.open_method, 0)?;
    let (open_time, random_time) = if open_type & OPEN_BY_TIME == OPEN_BY_TIME {
        (
            req_int("open_time", &raw.open_time)?,
            opt_int(&raw.random_time, -1)?,
        )
    } else {
        (0, 0)
    };
    let base_hp_max = match &raw.base_hp_max {
        // DoorData presets baseHpMax = 1 "to avoid doors created dead".
        None => 1.0,
        Some(text) => text
            .trim()
            .parse::<f32>()
            .map_err(|_| format!("Float value required, but found: {text}"))?,
    };

    Ok(DoorTemplate {
        id,
        name,
        node_x: [n1x, n2x, n3x, n4x],
        node_y: [n1y, n2y, n3y, n4y],
        node_z,
        height,
        pos_x,
        pos_y,
        pos_z: pos_z.min(node_z),
        emitter: opt_int(&raw.emitter_id, 0)?,
        child_door_id: opt_int(&raw.child_id_event, -1)?,
        group_name: raw.group.clone(),
        show_hp: opt_bool(&raw.hp_showable, true),
        is_wall: opt_bool(&raw.is_wall, false),
        master_door_close: master_event(&raw.master_close_event),
        master_door_open: master_event(&raw.master_open_event),
        targetable: opt_bool(&raw.targetable, true),
        open_by_default: raw.default_status.as_deref() == Some("open"),
        open_time,
        random_time,
        close_time: opt_int(&raw.close_time, -1)?,
        level: opt_int(&raw.level, 0)?,
        open_type,
        check_collision: opt_bool(&raw.check_collision, true),
        attackable: opt_bool(&raw.is_attackable, false),
        clanhall_id: opt_int(&raw.clanhall_id, 0)?,
        stealth: opt_bool(&raw.stealth, false),
        base_hp_max,
        base_p_def: opt_int(&raw.base_p_def, 0)?,
        base_m_def: opt_int(&raw.base_m_def, 0)?,
        collision_radius,
        collision_height: height,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOOR: &str = r#"<?xml version="1.0"?><list>
        <door id="1" name="a" open_method="2" height="150" baseHpMax="5000" basePDef="10"
              pos="100,200,-50" node1="90,190" node2="130,230" node3="130,210" node4="90,210"
              nodeZ="-60" default_status="open" open_time="30" />
        <door id="2" name="b" height="100" pos="0,0,0" node1="0,0" node2="100,5" node3="1,1"
              node4="2,2" nodeZ="10" group="g" hp_showable="TRUE" targetable="false" />
        <door id="3" name="broken" height="x" pos="0,0,0" node1="0,0" node2="1,1" node3="1,1"
              node4="2,2" nodeZ="10" />
        <door id="2" name="b2" group="g" targetable="false" height="100" pos="0,0,0" node1="0,0" node2="100,5" node3="1,1"
              node4="2,2" nodeZ="10" />
    </list>"#;

    #[test]
    fn java_defaults_and_derived_values() {
        let t = DoorTemplates::from_xml(DOOR).unwrap();
        let a = t.get(1).unwrap();
        assert_eq!(
            (a.pos_x, a.pos_y, a.pos_z),
            (100, 200, -60),
            "pos z is min(pos z, nodeZ)"
        );
        assert_eq!(a.node_x, [90, 130, 130, 90]);
        assert_eq!(a.node_y, [190, 230, 210, 210]);
        assert!(a.open_by_default && a.show_hp && a.targetable && a.check_collision);
        assert_eq!((a.open_time, a.random_time, a.close_time), (30, -1, -1));
        assert_eq!(a.collision_radius, 40);
        assert_eq!(a.collision_height, 150);
        assert_eq!(a.base_hp_max, 5000.0);
        let b = t.get(2).unwrap();
        assert_eq!(b.base_hp_max, 1.0, "DoorData presets baseHpMax = 1");
        assert_eq!(b.collision_radius, 20, "radius is at least 20");
        assert_eq!((b.open_time, b.random_time), (0, 0));
        assert_eq!(b.group_name.as_deref(), Some("g"));
        assert!(!b.targetable);
    }

    #[test]
    fn broken_door_is_skipped_and_repeated_ids_count_once() {
        let t = DoorTemplates::from_xml(DOOR).unwrap();
        assert!(t.get(3).is_none());
        assert_eq!(t.iter().count(), 3, "all good doors are spawned");
        assert_eq!(t.len(), 2, "Java counts distinct ids");
        assert_eq!(t.get(2).unwrap().name, "b2", "the last definition wins by id");
    }
}
