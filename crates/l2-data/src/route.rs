//! The NPC walking routes: `data/Routes.xml` (M8-MISC-01).
//!
//! Java: `managers/WalkingManager.java` (`load`, `parseDocument`), `data/holders/NpcRoutesHolder.java`,
//! `entity/actor/holders/npc/{WalkRoute,WalkNode}.java`. The routes are immutable; which NPC
//! walks where, and how far it got, is the world's (`l2_world::walking`).

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;
use tracing::warn;

use crate::loader::{load_file, log_loaded};
use crate::npc::NpcTemplates;

/// `WalkingManager.NO_REPEAT`.
pub const NO_REPEAT: i8 = -1;
/// `WalkingManager.REPEAT_GO_BACK` (`repeatStyle="back"`).
pub const REPEAT_GO_BACK: i8 = 0;
/// `WalkingManager.REPEAT_GO_FIRST` (`cycle`).
pub const REPEAT_GO_FIRST: i8 = 1;
/// `WalkingManager.REPEAT_TELE_FIRST` (`conveyor`).
pub const REPEAT_TELE_FIRST: i8 = 2;
/// `WalkingManager.REPEAT_RANDOM` (`random`).
pub const REPEAT_RANDOM: i8 = 3;

/// What a node makes the NPC say (Java: `WalkNode._npcString` or `_chatText`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteSay {
    /// An `npcString="NAME"` attribute: the Java constant name of the `NpcStringId`.
    NpcStringName(String),
    /// An `npcStringId="12345"` attribute: the client id.
    NpcStringId(i32),
    /// A `string="text"` attribute.
    Text(String),
}

/// Java `WalkNode`: one point of a route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalkNode {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// `getDelay()`: seconds to wait at the point.
    pub delay: i32,
    /// `runToLocation()`.
    pub run: bool,
    /// The shout at the point, if any.
    pub say: Option<RouteSay>,
}

/// Java `WalkRoute`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalkRoute {
    pub name: String,
    pub nodes: Vec<WalkNode>,
    /// `repeatWalk()`: the `repeat` attribute, and a style that exists (`REPEAT_MIN..=REPEAT_MAX`,
    /// that is back, cycle and conveyor; the `random` style is 3 and is not "supported" here).
    // NOTE(java-quirk): `WalkRoute` only accepts the repeat types 0 to 2, so `repeat="true"` with
    // `repeatStyle="random"` gives `repeatWalk() == false`. The random style still picks a random
    // node at every step (`WalkInfo.calculateNextNode` tests the style before `repeatWalk`).
    pub repeat_walk: bool,
    /// `getRepeatType()`.
    pub repeat_type: i8,
}

/// `data/Routes.xml`: Java `WalkingManager._routes`, `_targetedNpcIds` and `_routesToAttach`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Routes {
    routes: HashMap<String, WalkRoute>,
    /// `_routesToAttach`: by NPC id, the route of each spawn location (`NpcRoutesHolder`).
    attach: HashMap<i32, HashMap<(i32, i32, i32), String>>,
}

impl Routes {
    const LOADER: &'static str = "WalkingManager";

    /// Loads `<root>/data/Routes.xml` and logs Java's count line. A target whose NPC template does
    /// not exist is ignored, as in Java.
    pub fn load(root: &Path, npcs: &NpcTemplates) -> Self {
        let mut table = Self::default();
        load_file(&root.join("data/Routes.xml"), |doc: RawRoutes| {
            table.add(doc, &|id| npcs.get(id).is_some());
            Ok(())
        });
        log_loaded(Self::LOADER, table.routes.len(), "walking routes");
        table
    }

    /// Builds the table from XML text; `npc_exists` is Java's `NpcData.getTemplate(id) != null`.
    pub fn from_xml(xml: &str, npc_exists: &dyn Fn(i32) -> bool) -> Result<Self, crate::DataError> {
        let doc: RawRoutes = quick_xml::de::from_str(xml).map_err(|source| crate::DataError::Xml {
            path: "<inline>".into(),
            source,
        })?;
        let mut table = Self::default();
        table.add(doc, npc_exists);
        Ok(table)
    }

    fn add(&mut self, doc: RawRoutes, npc_exists: &dyn Fn(i32) -> bool) {
        for raw in doc.routes {
            let style = raw.repeat_style.trim().to_lowercase();
            let repeat_type = match style.as_str() {
                "back" => REPEAT_GO_BACK,
                "cycle" => REPEAT_GO_FIRST,
                "conveyor" => REPEAT_TELE_FIRST,
                "random" => REPEAT_RANDOM,
                _ => NO_REPEAT,
            };
            let repeat = raw.repeat.as_deref().is_some_and(java_boolean);
            let mut nodes = Vec::new();
            for child in raw.children {
                match child {
                    RawChild::Point(point) => match point.parse() {
                        Ok(node) => nodes.push(node),
                        Err(e) => warn!("{}: route '{}': {e}", Self::LOADER, raw.name),
                    },
                    RawChild::Target(target) => {
                        let Some((npc_id, x, y, z)) = target.parse() else {
                            warn!(
                                "{}: Error in target definition for route '{}'",
                                Self::LOADER,
                                raw.name
                            );
                            continue;
                        };
                        if npc_exists(npc_id) {
                            // `NpcRoutesHolder.addRoute`: the later route of a location wins.
                            self.attach
                                .entry(npc_id)
                                .or_default()
                                .insert((x, y, z), raw.name.clone());
                        }
                    }
                }
            }
            if nodes.is_empty() {
                // NOTE(java-quirk): `new WalkRoute` throws for an empty node list and Java loses
                // the rest of the file; here only this route is skipped.
                warn!("{}: route '{}' has no points; skipped.", Self::LOADER, raw.name);
                continue;
            }
            let supported = (REPEAT_GO_BACK..=REPEAT_TELE_FIRST).contains(&repeat_type);
            self.routes.insert(
                raw.name.clone(),
                WalkRoute {
                    name: raw.name,
                    nodes,
                    repeat_walk: repeat && supported,
                    repeat_type,
                },
            );
        }
    }

    /// Every route, in no particular order (Java keeps a `HashMap`).
    pub fn iter(&self) -> impl Iterator<Item = &WalkRoute> {
        self.routes.values()
    }

    /// Java `getRoute(name)`.
    pub fn get(&self, name: &str) -> Option<&WalkRoute> {
        self.routes.get(name)
    }

    /// The number of routes.
    pub fn len(&self) -> usize {
        self.routes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.routes.is_empty()
    }

    /// Java `isTargeted(npc)`: some route names this NPC id as a target.
    pub fn is_targeted(&self, npc_id: i32) -> bool {
        self.attach.contains_key(&npc_id)
    }

    /// Java `NpcRoutesHolder.getRouteName(npc)`: the route attached to the NPC id at its spawn
    /// location, or `None` (Java's empty string).
    pub fn route_at(&self, npc_id: i32, spawn: (i32, i32, i32)) -> Option<&str> {
        self.attach
            .get(&npc_id)
            .and_then(|by_location| by_location.get(&spawn))
            .map(String::as_str)
    }
}

/// `Boolean.valueOf(text)`: only "true" (any case) is true.
fn java_boolean(text: &str) -> bool {
    text.eq_ignore_ascii_case("true")
}

#[derive(Debug, Deserialize)]
struct RawRoutes {
    #[serde(rename = "route", default)]
    routes: Vec<RawRoute>,
}

#[derive(Debug, Deserialize)]
struct RawRoute {
    #[serde(rename = "@name")]
    name: String,
    #[serde(rename = "@repeat")]
    repeat: Option<String>,
    #[serde(rename = "@repeatStyle", default)]
    repeat_style: String,
    #[serde(rename = "$value", default)]
    children: Vec<RawChild>,
}

#[derive(Debug, Deserialize)]
enum RawChild {
    #[serde(rename = "point")]
    Point(RawPoint),
    #[serde(rename = "target")]
    Target(RawTarget),
}

#[derive(Debug, Deserialize)]
struct RawPoint {
    #[serde(rename = "@X")]
    x: Option<String>,
    #[serde(rename = "@Y")]
    y: Option<String>,
    #[serde(rename = "@Z")]
    z: Option<String>,
    #[serde(rename = "@delay")]
    delay: Option<String>,
    #[serde(rename = "@run")]
    run: Option<String>,
    #[serde(rename = "@string")]
    string: Option<String>,
    #[serde(rename = "@npcString")]
    npc_string: Option<String>,
    #[serde(rename = "@npcStringId")]
    npc_string_id: Option<String>,
}

impl RawPoint {
    fn parse(self) -> Result<WalkNode, String> {
        let int = |value: &Option<String>, name: &str| {
            value
                .as_deref()
                .and_then(|v| v.trim().parse::<i32>().ok())
                .ok_or_else(|| format!("integer value required for {name}"))
        };
        // `string`, else `npcString`, else `npcStringId` (the first one present).
        let say = if let Some(text) = self.string {
            Some(RouteSay::Text(text))
        } else if let Some(name) = self.npc_string {
            Some(RouteSay::NpcStringName(name))
        } else if let Some(id) = self.npc_string_id {
            Some(RouteSay::NpcStringId(
                id.trim().parse().map_err(|_| format!("bad npcStringId '{id}'"))?,
            ))
        } else {
            None
        };
        Ok(WalkNode {
            x: int(&self.x, "X")?,
            y: int(&self.y, "Y")?,
            z: int(&self.z, "Z")?,
            delay: int(&self.delay, "delay")?,
            run: self.run.as_deref().is_some_and(java_boolean),
            say,
        })
    }
}

#[derive(Debug, Deserialize)]
struct RawTarget {
    #[serde(rename = "@id")]
    id: Option<String>,
    #[serde(rename = "@spawnX")]
    spawn_x: Option<String>,
    #[serde(rename = "@spawnY")]
    spawn_y: Option<String>,
    #[serde(rename = "@spawnZ")]
    spawn_z: Option<String>,
}

impl RawTarget {
    fn parse(self) -> Option<(i32, i32, i32, i32)> {
        let int = |value: &Option<String>| value.as_deref()?.trim().parse::<i32>().ok();
        Some((
            int(&self.id)?,
            int(&self.spawn_x)?,
            int(&self.spawn_y)?,
            int(&self.spawn_z)?,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version="1.0"?>
<routes>
  <!-- comment -->
  <route name="walk" repeat="true" repeatStyle="back">
    <target id="100" spawnX="1" spawnY="2" spawnZ="3" />
    <target id="999" spawnX="1" spawnY="2" spawnZ="3" />
    <point X="1" Y="2" Z="3" delay="5" run="true" string="hello" />
    <point X="10" Y="20" Z="30" delay="0" run="false" npcStringId="1010205" />
    <point X="11" Y="21" Z="31" delay="0" run="TRUE" npcString="SOME_NAME" />
  </route>
  <route name="rand" repeat="true" repeatStyle="random">
    <point X="1" Y="1" Z="1" delay="0" run="true" />
  </route>
  <route name="once" repeat="false" repeatStyle="cycle">
    <point X="1" Y="1" Z="1" delay="0" run="true" />
  </route>
  <route name="empty" repeat="false" repeatStyle="cycle" />
</routes>"#;

    fn parsed() -> Routes {
        Routes::from_xml(XML, &|id| id == 100).unwrap()
    }

    #[test]
    fn points_keep_their_order_and_their_words() {
        let routes = parsed();
        let walk = routes.get("walk").unwrap();
        assert_eq!(walk.nodes.len(), 3);
        assert_eq!(walk.repeat_type, REPEAT_GO_BACK);
        assert!(walk.repeat_walk);
        assert_eq!(walk.nodes[0].say, Some(RouteSay::Text("hello".into())));
        assert!(walk.nodes[0].run && walk.nodes[0].delay == 5);
        assert_eq!(walk.nodes[1].say, Some(RouteSay::NpcStringId(1_010_205)));
        assert!(!walk.nodes[1].run);
        assert_eq!(
            walk.nodes[2].say,
            Some(RouteSay::NpcStringName("SOME_NAME".into()))
        );
        // `Boolean.valueOf("TRUE")`.
        assert!(walk.nodes[2].run);
    }

    #[test]
    fn the_random_style_is_never_repeat_walk_and_an_empty_route_is_skipped() {
        let routes = parsed();
        let rand = routes.get("rand").unwrap();
        assert_eq!(rand.repeat_type, REPEAT_RANDOM);
        assert!(!rand.repeat_walk);
        assert!(!routes.get("once").unwrap().repeat_walk);
        assert!(routes.get("empty").is_none());
        assert_eq!(routes.len(), 3);
    }

    #[test]
    fn targets_attach_by_npc_and_spawn_location() {
        let routes = parsed();
        assert!(routes.is_targeted(100));
        // An NPC id without a template is not attached.
        assert!(!routes.is_targeted(999));
        assert_eq!(routes.route_at(100, (1, 2, 3)), Some("walk"));
        assert_eq!(routes.route_at(100, (1, 2, 4)), None);
        assert_eq!(routes.route_at(101, (1, 2, 3)), None);
    }
}
