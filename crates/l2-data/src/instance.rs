//! Instance zones: the names of `data/InstanceNames.xml` and the templates of
//! `data/instances/**/*.xml`.
//!
//! Java: `gameserver/managers/InstanceManager.java` (`load`, `parseDocument`) and
//! `gameserver/entity/instancezone/Instance.java` (`loadInstanceTemplate`, `parseInstance`).
//! Data: `data/InstanceNames.xml`, `data/instances/**`, `xsd/instance.xsd`.
//!
//! # When Java parses a template
//! `InstanceManager.load` only remembers which file holds which template id; `Instance.
//! loadInstanceTemplate` parses that file every time an instance is created from it ("TODO: Cache
//! templates"). The files do not change while the server runs, so the template is parsed once
//! here and the world copies it into each new instance (`l2-world/src/instance/`).
//!
//! # Parsing like the DOM walk of `parseInstance`
//! Element names are compared lower case (`getNodeName().toLowerCase()`), the `<reset>`, `<door>`,
//! `<group>`, `<npc>` and `<skill>` names with `equalsIgnoreCase`, `<Location>` exactly. Numbers
//! are `Integer.parseInt` / `Long.parseLong`, booleans `Boolean.parseBoolean`.
//!
//! An exception in `parseInstance` ends the parse of the file, and what the instance got before
//! it stays (Java sets the fields on the `Instance` as it goes). [`InstanceTemplate::parse`] does
//! the same: it returns the template with everything read up to the error and the error text.

use std::collections::BTreeMap;
use std::path::Path;

use tracing::warn;

use crate::loader::{DataError, log_loaded};
use crate::stats::XmlNode;
use crate::stats::jnum::{number_format_message, parse_boolean, parse_int};
use crate::stats::node::parse_dom;

/// Java `InstanceReenterType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InstanceReenterType {
    #[default]
    None,
    OnEnter,
    OnFinish,
}

impl InstanceReenterType {
    /// Java `InstanceReenterType.valueOf(name)`: the exact constant name.
    fn value_of(name: &str) -> Option<Self> {
        match name {
            "NONE" => Some(Self::None),
            "ON_ENTER" => Some(Self::OnEnter),
            "ON_FINISH" => Some(Self::OnFinish),
            _ => None,
        }
    }
}

/// Java `InstanceRemoveBuffType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InstanceRemoveBuffType {
    #[default]
    None,
    All,
    Whitelist,
    Blacklist,
}

impl InstanceRemoveBuffType {
    /// Java `InstanceRemoveBuffType.valueOf(name.toUpperCase())`.
    fn value_of_upper(name: &str) -> Option<Self> {
        match name.to_uppercase().as_str() {
            "NONE" => Some(Self::None),
            "ALL" => Some(Self::All),
            "WHITELIST" => Some(Self::Whitelist),
            "BLACKLIST" => Some(Self::Blacklist),
            _ => None,
        }
    }
}

/// Java `java.time.DayOfWeek`; `value()` is Java's `getValue()` (Monday 1 to Sunday 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DayOfWeek {
    Monday = 1,
    Tuesday = 2,
    Wednesday = 3,
    Thursday = 4,
    Friday = 5,
    Saturday = 6,
    Sunday = 7,
}

impl DayOfWeek {
    /// Java `DayOfWeek.valueOf(name.toUpperCase())`.
    fn value_of_upper(name: &str) -> Option<Self> {
        Some(match name.to_uppercase().as_str() {
            "MONDAY" => Self::Monday,
            "TUESDAY" => Self::Tuesday,
            "WEDNESDAY" => Self::Wednesday,
            "THURSDAY" => Self::Thursday,
            "FRIDAY" => Self::Friday,
            "SATURDAY" => Self::Saturday,
            "SUNDAY" => Self::Sunday,
            _ => return None,
        })
    }

    /// Java `getValue()`.
    pub fn value(self) -> i32 {
        self as i32
    }
}

/// Java `InstanceReenterTimeHolder`: a fixed delay, or a day / hour / minute of the week.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceReenterTime {
    /// `new InstanceReenterTimeHolder(time)`: milliseconds after now.
    Delay(i64),
    /// `new InstanceReenterTimeHolder(day, hour, minute)`; `-1` for an hour or minute not given.
    At {
        day: Option<DayOfWeek>,
        hour: i32,
        minute: i32,
    },
}

/// One `<npc>` of a `<spawnlist>` group: the `StatSet` `parseInstance` stores.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceSpawn {
    /// `<group name>`.
    pub group: String,
    pub npc_id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// `heading` (default 0).
    pub heading: i32,
    /// `respawn` in seconds (default 0: no respawn).
    pub respawn: i32,
    /// `respawnRandom` in seconds (default 0).
    pub respawn_random: i32,
    /// `onKillDelay` in milliseconds (default -1: the NPC keeps its own).
    pub on_kill_delay: i32,
    /// The value `Spawn.setRandomWalking` gets.
    ///
    /// NOTE(java-quirk): Java stores the *negation* of `allowRandomWalk` (of the `<npc>`, else of
    /// the root): `spawnSet.set("allowRandomWalk", !allowRandomWalk)`. So `allowRandomWalk="false"`
    /// makes the NPC walk, and the default (true) makes it stand.
    pub random_walk: bool,
}

/// One instance template: what `parseInstance` sets on a new `Instance`. The defaults are the
/// field initialisers of `Instance`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceTemplate {
    /// The root `id` (`_templateId`).
    pub id: i32,
    /// The file, relative to `data/instances/` (Java `getInstanceTemplateFileName`).
    pub file: String,
    /// `ejectTime` times 1000; `None` keeps `GeneralConfig.EJECT_DEAD_PLAYER_TIME`.
    pub eject_time_ms: Option<i32>,
    /// The root `allowRandomWalk` (default true).
    pub allow_random_walk: bool,
    /// `<activityTime val>` in minutes: the instance lasts this long (+15 s).
    pub activity_time_minutes: Option<i32>,
    /// `<allowSummon val>` (default true).
    pub allow_summon: bool,
    /// `<emptyDestroyTime val>` times 1000 (default -1: an empty instance stays).
    pub empty_destroy_time_ms: i64,
    /// `<showTimer val increase text>`.
    pub show_timer: bool,
    pub timer_increase: bool,
    pub timer_text: String,
    /// `<PvPInstance val>`.
    pub pvp: bool,
    /// `<doorlist>`: the door ids, in file order.
    pub doors: Vec<i32>,
    /// `<spawnlist>`: every `<npc>` of every group, in file order.
    pub spawns: Vec<InstanceSpawn>,
    /// `<exitPoint x y z>`.
    pub exit_point: Option<(i32, i32, i32)>,
    /// `<spawnPoints>`: `None` when the element is missing (Java keeps its empty set).
    pub enter_locations: Option<Vec<(i32, i32, i32)>>,
    /// `<reenter additionStyle>`.
    pub reenter_type: InstanceReenterType,
    /// The `<reset>` entries.
    pub reenter_data: Vec<InstanceReenterTime>,
    /// `<removeBuffs type>`.
    pub remove_buff_type: InstanceRemoveBuffType,
    /// The `<skill id>` of `<removeBuffs>`.
    pub buff_exceptions: Vec<i32>,
}

impl InstanceTemplate {
    fn new(file: &str) -> Self {
        Self {
            id: -1,
            file: file.to_owned(),
            eject_time_ms: None,
            allow_random_walk: true,
            activity_time_minutes: None,
            allow_summon: true,
            empty_destroy_time_ms: -1,
            show_timer: false,
            timer_increase: true,
            timer_text: String::new(),
            pvp: false,
            doors: Vec::new(),
            spawns: Vec::new(),
            exit_point: None,
            enter_locations: None,
            reenter_type: InstanceReenterType::None,
            reenter_data: Vec::new(),
            remove_buff_type: InstanceRemoveBuffType::None,
            buff_exceptions: Vec::new(),
        }
    }

    /// Java `Instance.loadInstanceTemplate` for one file's text: the template and, when the parse
    /// stopped early, the error Java would log ("Instance: error while loading ...").
    pub fn parse(xml: &str, file: &str) -> (Self, Option<String>) {
        let mut template = Self::new(file);
        let root = match parse_dom(xml) {
            Ok(root) => root,
            Err(error) => return (template, Some(error.to_string())),
        };
        if !root.name.eq_ignore_ascii_case("instance") {
            return (template, None);
        }
        let error = template.parse_instance(&root).err();
        (template, error)
    }

    // Java: Instance.parseInstance
    fn parse_instance(&mut self, node: &XmlNode) -> Result<(), String> {
        self.id = int(required(node, "id")?)?;
        if let Some(a) = node.attr("ejectTime") {
            self.eject_time_ms = Some(int(a)?.wrapping_mul(1000));
        }
        if let Some(a) = node.attr("allowRandomWalk") {
            self.allow_random_walk = parse_boolean(a);
        }
        for n in &node.children {
            match n.name.to_lowercase().as_str() {
                "activitytime" => {
                    if let Some(a) = n.attr("val") {
                        self.activity_time_minutes = Some(int(a)?);
                    }
                }
                "allowsummon" => {
                    if let Some(a) = n.attr("val") {
                        self.allow_summon = parse_boolean(a);
                    }
                }
                "emptydestroytime" => {
                    if let Some(a) = n.attr("val") {
                        self.empty_destroy_time_ms = long(a)?.wrapping_mul(1000);
                    }
                }
                "showtimer" => {
                    if let Some(a) = n.attr("val") {
                        self.show_timer = parse_boolean(a);
                    }
                    if let Some(a) = n.attr("increase") {
                        self.timer_increase = parse_boolean(a);
                    }
                    if let Some(a) = n.attr("text") {
                        self.timer_text = a.to_owned();
                    }
                }
                "pvpinstance" => {
                    if let Some(a) = n.attr("val") {
                        self.pvp = parse_boolean(a);
                    }
                }
                "doorlist" => {
                    for d in n.children.iter().filter(|d| d.name.eq_ignore_ascii_case("door")) {
                        // The `<set>` children go into the door's `StatSet`, which `spawnDoors`
                        // never reads (it builds the door from `DoorData` by id).
                        self.doors.push(int(required(d, "id")?)?);
                    }
                }
                "spawnlist" => self.parse_spawn_list(n)?,
                "exitpoint" => {
                    let x = int(required(n, "x")?)?;
                    let y = int(required(n, "y")?)?;
                    let z = int(required(n, "z")?)?;
                    self.exit_point = Some((x, y, z));
                }
                "spawnpoints" => {
                    let mut locations = Vec::new();
                    for location in n.children.iter().filter(|l| l.name == "Location") {
                        // Java catches a bad location, logs it and goes on with the next.
                        match point(location) {
                            Ok(p) => locations.push(p),
                            Err(error) => warn!("Error parsing instance xml: {error}"),
                        }
                    }
                    self.enter_locations = Some(locations);
                }
                "reenter" => self.parse_reenter(n)?,
                "removebuffs" => {
                    if let Some(a) = n.attr("type") {
                        self.remove_buff_type = InstanceRemoveBuffType::value_of_upper(a)
                            .ok_or_else(|| enum_error("InstanceRemoveBuffType", a))?;
                    }
                    for d in n.children.iter().filter(|d| d.name.eq_ignore_ascii_case("skill")) {
                        if let Some(a) = d.attr("id") {
                            self.buff_exceptions.push(int(a)?);
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn parse_spawn_list(&mut self, n: &XmlNode) -> Result<(), String> {
        for group in n.children.iter().filter(|g| g.name.eq_ignore_ascii_case("group")) {
            let spawn_group = required(group, "name")?.to_owned();
            for d in group
                .children
                .iter()
                .filter(|d| d.name.eq_ignore_ascii_case("npc"))
            {
                let npc_id = int(required(d, "id")?)?;
                let x = int(required(d, "x")?)?;
                let y = int(required(d, "y")?)?;
                let z = int(required(d, "z")?)?;
                let optional = |name: &str, default: i32| d.attr(name).map_or(Ok(default), int);
                let heading = optional("heading", 0)?;
                let respawn = optional("respawn", 0)?;
                let on_kill_delay = optional("onKillDelay", -1)?;
                let respawn_random = optional("respawnRandom", 0)?;
                let allow_random_walk = d.attr("allowRandomWalk").map(parse_boolean);
                self.spawns.push(InstanceSpawn {
                    group: spawn_group.clone(),
                    npc_id,
                    x,
                    y,
                    z,
                    heading,
                    respawn,
                    respawn_random,
                    on_kill_delay,
                    random_walk: !allow_random_walk.unwrap_or(self.allow_random_walk),
                });
            }
        }
        Ok(())
    }

    fn parse_reenter(&mut self, n: &XmlNode) -> Result<(), String> {
        if let Some(a) = n.attr("additionStyle") {
            self.reenter_type =
                InstanceReenterType::value_of(a).ok_or_else(|| enum_error("InstanceReenterType", a))?;
        }
        for d in n.children.iter().filter(|d| d.name.eq_ignore_ascii_case("reset")) {
            match d.attr("time") {
                Some(a) => {
                    let time = long(a)?;
                    if time > 0 {
                        self.reenter_data.push(InstanceReenterTime::Delay(time));
                        // NOTE(java-quirk): this `break` leaves the loop over the `<reset>`
                        // elements: the resets after the first fixed delay are never read. A
                        // `time` of 0 or less adds nothing.
                        break;
                    }
                }
                None => {
                    let day = match d.attr("day") {
                        Some(a) => {
                            Some(DayOfWeek::value_of_upper(a).ok_or_else(|| enum_error("DayOfWeek", a))?)
                        }
                        None => None,
                    };
                    let hour = d.attr("hour").map_or(Ok(-1), int)?;
                    let minute = d.attr("minute").map_or(Ok(-1), int)?;
                    self.reenter_data
                        .push(InstanceReenterTime::At { day, hour, minute });
                }
            }
        }
        Ok(())
    }
}

fn required<'a>(node: &'a XmlNode, name: &str) -> Result<&'a str, String> {
    // Java: `getNamedItem(name).getNodeValue()` on a missing attribute.
    node.attr(name)
        .ok_or_else(|| format!("java.lang.NullPointerException: <{}> has no {name}", node.name))
}

fn int(text: &str) -> Result<i32, String> {
    parse_int(text).ok_or_else(|| format!("java.lang.NumberFormatException: {}", number_format_message(text)))
}

fn long(text: &str) -> Result<i64, String> {
    text.parse::<i64>()
        .map_err(|_| format!("java.lang.NumberFormatException: {}", number_format_message(text)))
}

fn point(node: &XmlNode) -> Result<(i32, i32, i32), String> {
    Ok((
        int(required(node, "x")?)?,
        int(required(node, "y")?)?,
        int(required(node, "z")?)?,
    ))
}

fn enum_error(name: &str, value: &str) -> String {
    format!("java.lang.IllegalArgumentException: No enum constant {name}.{value}")
}

/// The instance names and templates (Java `InstanceManager`'s tables). Immutable once loaded.
#[derive(Debug, Clone, Default)]
pub struct InstanceTemplates {
    /// `_instanceIdNames`.
    names: BTreeMap<i32, String>,
    /// `_instanceTemplateNames` (id, name) in file order, for `//instancezones`.
    name_list: Vec<(i32, String)>,
    /// `_instanceTemplates` with the parsed file.
    templates: BTreeMap<i32, InstanceTemplate>,
}

impl InstanceTemplates {
    /// Java `InstanceManager.load`: `data/InstanceNames.xml`, then every file of `data/instances`
    /// (recursive), logging Java's two count lines.
    pub fn load(root: &Path) -> Self {
        let mut table = Self::default();
        let names = root.join("data").join("InstanceNames.xml");
        match std::fs::read_to_string(&names) {
            Ok(text) => {
                if let Err(error) = table.add_names(&text) {
                    warn!("Error parsing InstanceNames.xml: {error}");
                }
            }
            Err(error) => warn!("Cannot parse InstanceNames.xml: {error}"),
        }
        log_loaded("InstanceManager", table.names.len(), "instance names");
        let dir = root.join("data").join("instances");
        let mut files = Vec::new();
        collect(&dir, &mut files);
        files.sort();
        for path in files {
            let relative = path
                .strip_prefix(&dir)
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            match std::fs::read_to_string(&path) {
                Ok(text) => table.add_template(&text, &relative),
                Err(error) => warn!("Cannot read {}: {error}", path.display()),
            }
        }
        log_loaded("InstanceManager", table.templates.len(), "instance templates");
        table
    }

    /// Parses an `InstanceNames.xml` text (Java `parseDocument`, the `list` case).
    pub fn add_names(&mut self, xml: &str) -> Result<(), DataError> {
        let root = parse_dom(xml).map_err(|e| DataError::Invalid(e.to_string()))?;
        if root.name != "list" {
            return Ok(());
        }
        for d in root.children.iter().filter(|d| d.name == "instance") {
            let (Some(id), Some(name)) = (d.attr("id").and_then(parse_int), d.attr("name")) else {
                return Err(DataError::Invalid(format!(
                    "bad instance name entry {:?}",
                    d.attrs
                )));
            };
            self.names.insert(id, name.to_owned());
            self.name_list.push((id, name.to_owned()));
        }
        Ok(())
    }

    /// Registers one template file (Java `parseDocument`, the `instance` case, plus the parse
    /// `loadInstanceTemplate` does later). A file whose root has no valid `id` is skipped.
    pub fn add_template(&mut self, xml: &str, file: &str) {
        let (template, error) = InstanceTemplate::parse(xml, file);
        if let Some(error) = &error {
            warn!("Instance: error while loading {file} ! {error}");
        }
        if template.id < 0 && error.is_some() {
            return;
        }
        if template.id >= 0 {
            self.templates.insert(template.id, template);
        }
    }

    /// Java `getInstanceIdName(id)`.
    pub fn name(&self, id: i32) -> &str {
        self.names.get(&id).map_or("UnknownInstance", String::as_str)
    }

    /// Java `getInstanceTemplateNames()`: (id, name) in file order.
    pub fn name_list(&self) -> &[(i32, String)] {
        &self.name_list
    }

    /// The template of `id` (Java: `getInstanceTemplateFileName(id)` and the parse of that file).
    pub fn get(&self, id: i32) -> Option<&InstanceTemplate> {
        self.templates.get(&id)
    }

    pub fn len(&self) -> usize {
        self.templates.len()
    }

    pub fn is_empty(&self) -> bool {
        self.templates.is_empty()
    }
}

fn collect(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        warn!("Directory not found: {}", dir.display());
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, out);
        } else if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("xml")) {
            out.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ARENA: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!-- Grassy Arena -->
<instance id="147" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
	<PvPInstance val="true" />
	<doorlist>
		<door id="17100001" />
		<door id="17100002" />
	</doorlist>
	<spawnlist>
		<group name="general">
			<npc id="36402" x="-89400" y="-252843" z="-3341" heading="0" respawn="60" />
			<npc id="36402" x="-86763" y="-252843" z="-3341" heading="0" respawn="60" />
		</group>
	</spawnlist>
</instance>"#;

    #[test]
    fn an_olympiad_arena() {
        let (t, error) = InstanceTemplate::parse(ARENA, "Olympiad/OlympiadGrassyArena.xml");
        assert_eq!(error, None);
        assert_eq!(t.id, 147);
        assert!(t.pvp);
        assert_eq!(t.doors, [17_100_001, 17_100_002]);
        assert_eq!(t.spawns.len(), 2);
        let s = &t.spawns[0];
        assert_eq!(
            (s.npc_id, s.x, s.y, s.z, s.heading),
            (36402, -89400, -252_843, -3341, 0)
        );
        assert_eq!((s.respawn, s.respawn_random, s.on_kill_delay), (60, 0, -1));
        // The default allowRandomWalk is true, stored negated.
        assert!(!s.random_walk);
        assert_eq!(s.group, "general");
        assert!(t.allow_summon);
        assert_eq!(t.empty_destroy_time_ms, -1);
        assert_eq!(t.enter_locations, None);
    }

    #[test]
    fn every_element_of_the_schema() {
        let xml = r#"<instance id="10" ejectTime="120" allowRandomWalk="false">
            <activityTime val="60"/><allowSummon val="false"/><emptyDestroyTime val="300"/>
            <showTimer val="true" increase="false" text="Time left"/>
            <spawnPoints><Location x="1" y="2" z="3"/><location x="9" y="9" z="9"/><Location x="4" y="5" z="6"/></spawnPoints>
            <exitPoint x="7" y="8" z="9"/>
            <reenter additionStyle="ON_FINISH"><reset day="Wednesday" hour="6" minute="30"/><reset hour="6"/></reenter>
            <removeBuffs type="whitelist"><skill id="1"/><skill id="2"/></removeBuffs>
            <spawnlist><group name="a"><npc id="5" x="1" y="1" z="1" allowRandomWalk="true"/><npc id="6" x="1" y="1" z="1"/></group></spawnlist>
        </instance>"#;
        let (t, error) = InstanceTemplate::parse(xml, "x.xml");
        assert_eq!(error, None);
        assert_eq!(t.eject_time_ms, Some(120_000));
        assert!(!t.allow_random_walk);
        assert_eq!(t.activity_time_minutes, Some(60));
        assert!(!t.allow_summon);
        assert_eq!(t.empty_destroy_time_ms, 300_000);
        assert!(t.show_timer && !t.timer_increase);
        assert_eq!(t.timer_text, "Time left");
        // `equals("Location")` is case-sensitive.
        assert_eq!(t.enter_locations, Some(vec![(1, 2, 3), (4, 5, 6)]));
        assert_eq!(t.exit_point, Some((7, 8, 9)));
        assert_eq!(t.reenter_type, InstanceReenterType::OnFinish);
        assert_eq!(
            t.reenter_data,
            [
                InstanceReenterTime::At {
                    day: Some(DayOfWeek::Wednesday),
                    hour: 6,
                    minute: 30
                },
                InstanceReenterTime::At {
                    day: None,
                    hour: 6,
                    minute: -1
                }
            ]
        );
        assert_eq!(t.remove_buff_type, InstanceRemoveBuffType::Whitelist);
        assert_eq!(t.buff_exceptions, [1, 2]);
        // `allowRandomWalk="true"` on the npc: false; none: the root's false, negated.
        assert!(!t.spawns[0].random_walk);
        assert!(t.spawns[1].random_walk);
    }

    #[test]
    fn the_first_fixed_reset_ends_the_reset_list() {
        let xml = r#"<instance id="1"><reenter additionStyle="ON_ENTER">
            <reset time="0"/><reset time="3600000"/><reset hour="6" minute="30"/></reenter></instance>"#;
        let (t, _) = InstanceTemplate::parse(xml, "x.xml");
        assert_eq!(t.reenter_type, InstanceReenterType::OnEnter);
        assert_eq!(t.reenter_data, [InstanceReenterTime::Delay(3_600_000)]);
    }

    #[test]
    fn an_error_keeps_what_was_read_before_it() {
        let xml = r#"<instance id="3"><PvPInstance val="true"/><exitPoint x="1" y="oops" z="3"/>
            <allowSummon val="false"/></instance>"#;
        let (t, error) = InstanceTemplate::parse(xml, "x.xml");
        assert!(error.unwrap().contains("For input string: \"oops\""));
        assert!(t.pvp);
        assert_eq!(t.exit_point, None);
        assert!(t.allow_summon, "not reached");
    }

    #[test]
    fn names_and_the_table() {
        let mut table = InstanceTemplates::default();
        table
            .add_names(
                r#"<list><instance id="1" name="Party Duel"/><instance id="147" name="Grassy"/></list>"#,
            )
            .unwrap();
        table.add_template(ARENA, "Olympiad/OlympiadGrassyArena.xml");
        table.add_template("<instance/>", "bad.xml");
        assert_eq!(table.len(), 1);
        assert_eq!(table.name(1), "Party Duel");
        assert_eq!(table.name(5), "UnknownInstance");
        assert_eq!(table.name_list()[1], (147, "Grassy".to_owned()));
        assert_eq!(table.get(147).unwrap().file, "Olympiad/OlympiadGrassyArena.xml");
    }

    /// The shipped datapack: Java's count lines say 160 names (the file) and one template per
    /// file id.
    #[test]
    fn the_shipped_instances_load() {
        let root = Path::new(r"C:\l2-rust\l2jmobius\dist\game");
        if !root.join("data").join("instances").is_dir() {
            eprintln!("skipped: the reference datapack is not on this machine");
            return;
        }
        let table = InstanceTemplates::load(root);
        assert_eq!(table.name_list().len(), 160);
        for id in [147, 148, 149, 150] {
            let arena = table.get(id).unwrap();
            assert!(arena.pvp);
            assert_eq!(arena.spawns.len(), 2, "{id}");
            assert_eq!(arena.doors.len(), 2, "{id}");
        }
        assert!(table.len() > 100);
    }
}
