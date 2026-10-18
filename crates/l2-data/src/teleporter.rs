//! Teleporter (gatekeeper) lists: which NPC offers which destinations, at what fee, for whom.
//!
//! Java: `gameserver/data/xml/TeleporterData.java`, `gameserver/entity/teleporter/TeleportHolder.java`,
//! `gameserver/entity/teleporter/TeleportLocation.java`, `gameserver/entity/actor/enums/player/TeleportType.java`.
//! Data: `data/teleporters/**/*.xml` (`xsd/teleporterData.xsd`).
//!
//! # Data layout
//! `<list>` holds `<npc id="..">` elements. Each has `<teleport type="..." name="..">` children
//! (one list each, with `<location x y z feeId feeCount castleId name/>` entries) and an optional
//! `<npcs><npc id=".."/></npcs>` that gives other NPC ids the same lists. A teleport list without a
//! `name` is called after its type (`NORMAL`, `OTHER`, ...). The bypass a gatekeeper sends names the
//! list and a location index, so a location's index is its position in the list.
//!
//! # What is kept
//! Everything the teleporter needs as data: list name and type (the noble-only and fee rules
//! depend on the type), location coordinates, fee item and count, and the castle ids that block a
//! teleport during a siege. Fee and siege checks are done by the caller (they need the player).
//!
//! # Java behaviour kept on purpose
//! * `<npcs>` copies the lists that were parsed **before** it in the same `<npc>` element. Lists
//!   after it are not copied (Java walks the children in order). Copies are snapshots.
//! * A second list with the same name inside one `<npc>` is dropped with a warning; a later
//!   `<npc>` element (or file) for the same id merges into the existing lists, replacing lists with
//!   the same name.
//! * An NPC with an empty `<npc>` element still counts as an NPC (Java's `computeIfAbsent`).
//! * Like Java's `parseDatapackFile`, a bad value (no `id`, an unknown `type`, a non-number) stops the
//!   rest of that file, while entries parsed before it stay.
//!
//! NOTE(java-quirk): Java's `getLocation(index)` is `List.get`, so an index out of range throws
//! instead of returning `null`; the `null` check in `TeleportHolder.doTeleport` is never reached.
//! [`TeleportHolder::location`] returns `None`; the caller logs what Java logs for an unknown location.
//!
//! NOTE(java-quirk): element names are matched exactly as written. Java matches `list`, `npc` and
//! `location` without regard to case, but the shipped files use the lower-case names only.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use serde::Deserialize;
use tracing::warn;

use crate::item::ADENA_ID;
use crate::loader::{DataError, load_dir, log_loaded};

/// Java `TeleportType`: which rules apply to a list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TeleportType {
    /// `NORMAL`: the town gatekeepers (free below the free-teleport level, siege and karma rules).
    Normal,
    /// `NOBLES_TOKEN`: Noblesse only, paid with the noblesse token.
    NoblesToken,
    /// `NOBLES_ADENA`: Noblesse only, paid with adena.
    NoblesAdena,
    /// `OTHER`: every other teleporter (fee always applies).
    Other,
}

impl TeleportType {
    /// The Java constant name, e.g. `NOBLES_TOKEN`. Also the default list name.
    pub fn name(self) -> &'static str {
        match self {
            TeleportType::Normal => "NORMAL",
            TeleportType::NoblesToken => "NOBLES_TOKEN",
            TeleportType::NoblesAdena => "NOBLES_ADENA",
            TeleportType::Other => "OTHER",
        }
    }

    /// Java `TeleportType.valueOf(name)`: exact and case sensitive.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "NORMAL" => Some(TeleportType::Normal),
            "NOBLES_TOKEN" => Some(TeleportType::NoblesToken),
            "NOBLES_ADENA" => Some(TeleportType::NoblesAdena),
            "OTHER" => Some(TeleportType::Other),
            _ => None,
        }
    }
}

/// One destination of a teleport list. Java `TeleportLocation` (a `Location` plus fee data).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeleportLocation {
    /// Position in the list: the index a bypass carries.
    pub id: usize,
    /// Shown name, `None` when the attribute is missing (Java `null`).
    pub name: Option<String>,
    /// X coordinate.
    pub x: i32,
    /// Y coordinate.
    pub y: i32,
    /// Z coordinate.
    pub z: i32,
    /// Heading, 0 when missing.
    pub heading: i32,
    /// Instance id, 0 when missing.
    pub instance_id: i32,
    /// Item id of the fee, [`ADENA_ID`] when missing.
    pub fee_id: i32,
    /// Fee count, 0 when missing (a free destination).
    pub fee_count: i64,
    /// Castles whose siege blocks this destination. Empty when none.
    pub castle_ids: Vec<i32>,
}

/// Java `TeleportHolder`: one named list of destinations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeleportHolder {
    name: String,
    kind: TeleportType,
    locations: Vec<TeleportLocation>,
}

impl TeleportHolder {
    /// An empty list.
    pub fn new(name: impl Into<String>, kind: TeleportType) -> Self {
        Self {
            name: name.into(),
            kind,
            locations: Vec::new(),
        }
    }

    /// Java `registerLocation`: appends a destination; its id is the next index.
    pub fn register_location(&mut self, mut location: TeleportLocation) {
        location.id = self.locations.len();
        self.locations.push(location);
    }

    /// The list name (the `name` of the `<teleport>`, or its type name).
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The list type.
    pub fn kind(&self) -> TeleportType {
        self.kind
    }

    /// All destinations, in document order.
    pub fn locations(&self) -> &[TeleportLocation] {
        &self.locations
    }

    /// Java `getLocation(index)`: `None` for a negative or unknown index (see the module notes).
    pub fn location(&self, index: i32) -> Option<&TeleportLocation> {
        let index = usize::try_from(index).ok()?;
        self.locations.get(index)
    }

    /// Java `isNoblesse`: the list is for Noblesse only.
    pub fn is_noblesse(&self) -> bool {
        matches!(self.kind, TeleportType::NoblesAdena | TeleportType::NoblesToken)
    }
}

/// Java `TeleporterData`: the lists of every teleporter NPC, by NPC template id.
#[derive(Debug, Default, Clone)]
pub struct TeleporterData {
    lists: HashMap<i32, HashMap<String, Arc<TeleportHolder>>>,
}

impl TeleporterData {
    const LOADER: &'static str = "TeleporterData";

    /// Loads every file below `<root>/data/teleporters` and logs Java's count line.
    pub fn load(root: &Path) -> Self {
        let mut data = Self::default();
        load_dir(&root.join("data/teleporters"), true, |doc: RawList| {
            data.add_document(doc)
        });
        log_loaded(Self::LOADER, data.npc_count(), "npc teleporters");
        data
    }

    /// Builds the table from XML text. For tests and tools.
    pub fn from_xml(xml: &str) -> Result<Self, DataError> {
        let doc = quick_xml::de::from_str(xml).map_err(|source| DataError::Xml {
            path: "<inline>".into(),
            source,
        })?;
        let mut data = Self::default();
        data.add_document(doc)?;
        Ok(data)
    }

    /// Java `getHolder(npcId, listName)`: the list of an NPC, `None` when either is unknown.
    pub fn get_holder(&self, npc_id: i32, list_name: &str) -> Option<&TeleportHolder> {
        self.lists.get(&npc_id)?.get(list_name).map(Arc::as_ref)
    }

    /// Java `getTeleporterCount()`: the number of NPC ids with a (possibly empty) set of lists.
    pub fn npc_count(&self) -> usize {
        self.lists.len()
    }

    /// True when no NPC was loaded.
    pub fn is_empty(&self) -> bool {
        self.lists.is_empty()
    }

    /// Java `parseDocument`: one file. An error stops the file; what was registered stays.
    fn add_document(&mut self, doc: RawList) -> Result<(), DataError> {
        for npc in doc.npcs {
            let npc_id = decode_id(npc.id.as_deref())?;
            let mut tele_list: HashMap<String, Arc<TeleportHolder>> = HashMap::new();
            // Children are handled in document order: an `<npcs>` element copies what came before it.
            for child in npc.children {
                match child {
                    RawNpcChild::Teleport(raw) => {
                        let holder = build_holder(raw)?;
                        if tele_list.contains_key(holder.name()) {
                            warn!(
                                "Duplicate teleport list ({}) has been found for NPC: {npc_id}",
                                holder.name()
                            );
                        } else {
                            tele_list.insert(holder.name().to_string(), Arc::new(holder));
                        }
                    }
                    RawNpcChild::Npcs(alias_list) => {
                        for alias in alias_list.npcs {
                            let alias_id = decode_id(alias.id.as_deref())?;
                            self.register(alias_id, &tele_list);
                        }
                    }
                    RawNpcChild::Other => {}
                }
            }
            self.register(npc_id, &tele_list);
        }
        Ok(())
    }

    /// Java `registerTeleportList`: merges the lists into the NPC's entry (`putAll`).
    fn register(&mut self, npc_id: i32, lists: &HashMap<String, Arc<TeleportHolder>>) {
        self.lists.entry(npc_id).or_default().extend(
            lists
                .iter()
                .map(|(name, holder)| (name.clone(), Arc::clone(holder))),
        );
    }
}

/// Java `parseDocument` for one `<teleport>`: type, name (default: the type's name) and locations.
fn build_holder(raw: RawTeleport) -> Result<TeleportHolder, DataError> {
    // Java: `parseEnum` gives `null` for a bad or missing type and the next line calls `type.name()`,
    // so the file stops there.
    let kind = raw
        .kind
        .as_deref()
        .and_then(TeleportType::from_name)
        .ok_or_else(|| {
            DataError::Invalid(format!(
                "teleport type {:?} is not a TeleportType",
                raw.kind.as_deref().unwrap_or("")
            ))
        })?;
    let name = raw.name.unwrap_or_else(|| kind.name().to_string());
    let mut holder = TeleportHolder::new(name, kind);
    for location in &raw.locations {
        holder.register_location(build_location(location)?);
    }
    Ok(holder)
}

/// Java `TeleportLocation(StatSet)`. The attributes are read the way `StatSet` reads them:
/// `Integer.parseInt` for ints, `Long.parseLong` for `feeCount`, and a missing value takes the default.
fn build_location(raw: &RawLocation) -> Result<TeleportLocation, DataError> {
    Ok(TeleportLocation {
        id: 0,
        name: raw.name.clone(),
        x: stat_int(raw.x.as_deref(), "x", 0)?,
        y: stat_int(raw.y.as_deref(), "y", 0)?,
        z: stat_int(raw.z.as_deref(), "z", 0)?,
        heading: stat_int(raw.heading.as_deref(), "heading", 0)?,
        instance_id: stat_int(raw.instance_id.as_deref(), "instanceId", 0)?,
        fee_id: stat_int(raw.fee_id.as_deref(), "feeId", ADENA_ID)?,
        fee_count: stat_long(raw.fee_count.as_deref(), "feeCount", 0)?,
        castle_ids: parse_castle_ids(raw.castle_id.as_deref().unwrap_or(""))?,
    })
}

/// `StatSet.getInt(key, default)`: `Integer.parseInt` of the text (decimal only, sign allowed).
fn stat_int(value: Option<&str>, key: &str, default: i32) -> Result<i32, DataError> {
    match value {
        None => Ok(default),
        Some(text) => text
            .parse()
            .map_err(|_| DataError::Invalid(format!("{key} \"{text}\" is not an integer"))),
    }
}

/// `StatSet.getLong(key, default)`: `Long.parseLong` of the text.
fn stat_long(value: Option<&str>, key: &str, default: i64) -> Result<i64, DataError> {
    match value {
        None => Ok(default),
        Some(text) => text
            .parse()
            .map_err(|_| DataError::Invalid(format!("{key} \"{text}\" is not a long"))),
    }
}

/// Java `TeleportLocation` castle list: `""` is none, `"4"` or `"4;5"`. Java's `split(";")` drops
/// trailing empty parts (`"4;"` is `[4]`), but an empty part in the middle is an error.
fn parse_castle_ids(text: &str) -> Result<Vec<i32>, DataError> {
    if text.is_empty() {
        return Ok(Vec::new());
    }
    if !text.contains(';') {
        return Ok(vec![stat_int(Some(text), "castleId", 0)?]);
    }
    let mut parts: Vec<&str> = text.split(';').collect();
    while parts.last() == Some(&"") {
        parts.pop();
    }
    parts
        .into_iter()
        .map(|part| stat_int(Some(part), "castleId", 0))
        .collect()
}

/// `IXmlReader.parseInteger` (Java `Integer.decode`): the ids of NPC elements may be written as
/// hex (`0x`, `#`) or octal (leading `0`). A missing id is an error (Java throws a NullPointerException).
///
/// Same rules as `npc/stat_set.rs` `decode_i32`, which is private to the NPC loader.
fn decode_id(text: Option<&str>) -> Result<i32, DataError> {
    let text = text.ok_or_else(|| DataError::Invalid("teleporter NPC without an id".into()))?;
    java_decode(text).ok_or_else(|| DataError::Invalid(format!("id \"{text}\" is not an integer")))
}

fn java_decode(text: &str) -> Option<i32> {
    let (negative, rest) = match text.as_bytes().first()? {
        b'-' => (true, &text[1..]),
        b'+' => (false, &text[1..]),
        _ => (false, text),
    };
    let (radix, digits) = if let Some(hex) = rest.strip_prefix("0x").or_else(|| rest.strip_prefix("0X")) {
        (16, hex)
    } else if let Some(hex) = rest.strip_prefix('#') {
        (16, hex)
    } else if rest.len() > 1 && rest.starts_with('0') {
        (8, &rest[1..])
    } else {
        (10, rest)
    };
    if digits.is_empty() || !digits.chars().all(|c| c.is_digit(radix)) {
        return None;
    }
    let magnitude = i64::from_str_radix(digits, radix).ok()?;
    i32::try_from(if negative { -magnitude } else { magnitude }).ok()
}

// Raw XML shape (xsd/teleporterData.xsd). Every value is read as text and parsed like Java does.

#[derive(Deserialize)]
struct RawList {
    #[serde(rename = "npc", default)]
    npcs: Vec<RawNpc>,
}

#[derive(Deserialize)]
struct RawNpc {
    #[serde(rename = "@id", default)]
    id: Option<String>,
    #[serde(rename = "$value", default)]
    children: Vec<RawNpcChild>,
}

#[derive(Deserialize)]
enum RawNpcChild {
    #[serde(rename = "teleport")]
    Teleport(RawTeleport),
    #[serde(rename = "npcs")]
    Npcs(RawAliases),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct RawAliases {
    #[serde(rename = "npc", default)]
    npcs: Vec<RawAlias>,
}

#[derive(Deserialize)]
struct RawAlias {
    #[serde(rename = "@id", default)]
    id: Option<String>,
}

#[derive(Deserialize)]
struct RawTeleport {
    #[serde(rename = "@name", default)]
    name: Option<String>,
    #[serde(rename = "@type", default)]
    kind: Option<String>,
    #[serde(rename = "location", default)]
    locations: Vec<RawLocation>,
}

#[derive(Deserialize)]
struct RawLocation {
    #[serde(rename = "@name", default)]
    name: Option<String>,
    #[serde(rename = "@x", default)]
    x: Option<String>,
    #[serde(rename = "@y", default)]
    y: Option<String>,
    #[serde(rename = "@z", default)]
    z: Option<String>,
    #[serde(rename = "@heading", default)]
    heading: Option<String>,
    #[serde(rename = "@instanceId", default)]
    instance_id: Option<String>,
    #[serde(rename = "@feeId", default)]
    fee_id: Option<String>,
    #[serde(rename = "@feeCount", default)]
    fee_count: Option<String>,
    #[serde(rename = "@castleId", default)]
    castle_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<list>
	<npc id="100">
		<teleport type="NORMAL">
			<location name="Town A" x="-10" y="20" z="-30" feeCount="500" />
			<location name="Town B" x="+1" y="2" z="3" heading="90" instanceId="7" feeId="13722" feeCount="1" castleId="4;5;" />
		</teleport>
		<teleport type="NOBLES_TOKEN">
			<location x="5" y="6" z="7" />
		</teleport>
		<npcs>
			<npc id="0x65"/>
		</npcs>
	</npc>
	<npc id="200">
		<npcs>
			<npc id="201"/>
		</npcs>
		<teleport name="late" type="OTHER">
			<location name="Late" x="1" y="1" z="1" />
		</teleport>
	</npc>
</list>"#;

    #[test]
    fn lists_locations_and_defaults_are_read() {
        let data = TeleporterData::from_xml(XML).unwrap();
        // 100, 101 (alias 0x65 is 101), 200, 201 (alias with no lists).
        assert_eq!(data.npc_count(), 4);
        let normal = data.get_holder(100, "NORMAL").unwrap();
        assert_eq!(normal.kind(), TeleportType::Normal);
        assert!(!normal.is_noblesse());
        assert_eq!(normal.locations().len(), 2);
        let first = normal.location(0).unwrap();
        assert_eq!(first.name.as_deref(), Some("Town A"));
        assert_eq!((first.x, first.y, first.z), (-10, 20, -30));
        assert_eq!((first.heading, first.instance_id), (0, 0));
        assert_eq!((first.fee_id, first.fee_count), (ADENA_ID, 500));
        assert!(first.castle_ids.is_empty());
        let second = normal.location(1).unwrap();
        assert_eq!(second.id, 1);
        assert_eq!((second.x, second.heading, second.instance_id), (1, 90, 7));
        assert_eq!((second.fee_id, second.fee_count), (13722, 1));
        assert_eq!(second.castle_ids, vec![4, 5]);
        assert!(normal.location(2).is_none());
        assert!(normal.location(-1).is_none());
    }

    #[test]
    fn noblesse_lists_and_aliases() {
        let data = TeleporterData::from_xml(XML).unwrap();
        let noble = data.get_holder(101, "NOBLES_TOKEN").unwrap();
        assert!(noble.is_noblesse());
        assert_eq!(noble.location(0).unwrap().name, None);
        // The alias of 100 holds the same lists (snapshot taken when `<npcs>` was read).
        assert_eq!(data.get_holder(101, "NORMAL").unwrap().locations().len(), 2);
    }

    #[test]
    fn alias_before_a_list_does_not_copy_it() {
        let data = TeleporterData::from_xml(XML).unwrap();
        assert!(data.get_holder(201, "OTHER").is_none());
        assert!(data.get_holder(200, "late").is_some());
        assert_eq!(data.get_holder(200, "late").unwrap().name(), "late");
    }

    #[test]
    fn unknown_list_and_npc_give_none() {
        let data = TeleporterData::from_xml(XML).unwrap();
        assert!(data.get_holder(100, "HUNTING").is_none());
        assert!(data.get_holder(999, "NORMAL").is_none());
    }

    #[test]
    fn duplicate_list_name_keeps_the_first() {
        let xml = r#"<list><npc id="1">
			<teleport type="OTHER"><location name="first" x="1" y="1" z="1"/></teleport>
			<teleport type="OTHER"><location name="second" x="2" y="2" z="2"/></teleport>
		</npc></list>"#;
        let data = TeleporterData::from_xml(xml).unwrap();
        let holder = data.get_holder(1, "OTHER").unwrap();
        assert_eq!(holder.locations()[0].name.as_deref(), Some("first"));
    }

    #[test]
    fn bad_type_stops_the_file_but_keeps_earlier_npcs() {
        let xml = r#"<list>
			<npc id="1"><teleport type="OTHER"><location x="1" y="1" z="1"/></teleport></npc>
			<npc id="2"><teleport type="HUNTING"><location x="1" y="1" z="1"/></teleport></npc>
			<npc id="3"><teleport type="OTHER"><location x="1" y="1" z="1"/></teleport></npc>
		</list>"#;
        let mut data = TeleporterData::default();
        let doc: RawList = quick_xml::de::from_str(xml).unwrap();
        assert!(data.add_document(doc).is_err());
        assert!(data.get_holder(1, "OTHER").is_some());
        assert!(data.get_holder(2, "OTHER").is_none());
        assert!(data.get_holder(3, "OTHER").is_none());
    }

    #[test]
    fn castle_list_rules_follow_java_split() {
        assert_eq!(parse_castle_ids("").unwrap(), Vec::<i32>::new());
        assert_eq!(parse_castle_ids("9").unwrap(), vec![9]);
        assert_eq!(parse_castle_ids("4;5;").unwrap(), vec![4, 5]);
        assert_eq!(parse_castle_ids(";").unwrap(), Vec::<i32>::new());
        assert!(parse_castle_ids("4;;5").is_err());
        assert!(parse_castle_ids(" 4").is_err());
    }

    #[test]
    fn npc_id_decodes_like_java_integer_decode() {
        assert_eq!(decode_id(Some("35100")).unwrap(), 35100);
        assert_eq!(decode_id(Some("0x10")).unwrap(), 16);
        assert_eq!(decode_id(Some("010")).unwrap(), 8);
        assert_eq!(decode_id(Some("-3")).unwrap(), -3);
        assert!(decode_id(None).is_err());
        assert!(decode_id(Some("12a")).is_err());
    }

    #[test]
    fn type_names_round_trip() {
        for kind in [
            TeleportType::Normal,
            TeleportType::NoblesToken,
            TeleportType::NoblesAdena,
            TeleportType::Other,
        ] {
            assert_eq!(TeleportType::from_name(kind.name()), Some(kind));
        }
        assert_eq!(TeleportType::from_name("HUNTING"), None);
    }
}
