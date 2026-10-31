//! The minigame arenas: `data/UndergroundColiseum.xml` and `data/KrateisCube.xml` (M8-GAMES-01).
//!
//! Java: `managers/games/UndergroundColiseumManager.java` (`parseDocument`),
//! `managers/games/KrateisCubeManager.java` (`parseDocument`). The definitions are immutable; the
//! state of a match is the world's (`l2_world::minigame`). Doors are listed by id; the ones the
//! door table does not know are dropped by the world, like Java's `if (door != null)`.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use crate::loader::{DataError, load_file, log_loaded};

/// A place of an arena.
pub type Place = (i32, i32, i32);

/// One reward line (`UCReward`, `KrateisReward`). `id` -100 is PC cafe points, -200 clan reputation,
/// -300 fame, above 0 an item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArenaReward {
    pub id: i32,
    pub amount: i64,
    /// Java `useModifers` (sic).
    pub use_modifier: bool,
}

/// A tower of an Underground Coliseum team.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColiseumTower {
    pub npc_id: i32,
    pub place: Place,
}

/// A start point of an Underground Coliseum arena (`UCPoint`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColiseumPoint {
    pub doors: Vec<i32>,
    pub place: Place,
}

/// One `<arena>` of `UndergroundColiseum.xml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColiseumArenaDef {
    pub id: i32,
    pub min_level: i32,
    pub max_level: i32,
    /// `curator`: the NPC id of the arena's manager (Java looks up its spawn).
    pub curator: i32,
    /// The towers, in file order: index 0 is the blue team, 1 the red.
    pub towers: Vec<ColiseumTower>,
    pub points: Vec<ColiseumPoint>,
    pub rewards: Vec<ArenaReward>,
}

/// One `<arena>` of `KrateisCube.xml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KrateisArenaDef {
    pub id: i32,
    /// `manager`: the NPC id of the arena's match manager.
    pub manager: i32,
    pub min_level: i32,
    pub max_level: i32,
    pub min_players: i32,
    pub max_players: i32,
    pub doors_a: Vec<i32>,
    pub doors_b: Vec<i32>,
    pub wait_locations: Vec<Place>,
    pub battle_locations: Vec<Place>,
    pub watcher_locations: Vec<Place>,
    /// `<buffs>`: (skill id, level).
    pub buffs: Vec<(i32, i32)>,
    pub rewards: Vec<ArenaReward>,
    /// `<add_parameters>` (Java `StatSet`): the values are text, `getInt` parses them.
    pub params: BTreeMap<String, String>,
}

impl KrateisArenaDef {
    /// Java `StatSet.getInt(name)`: the parameter as a number. Java throws for a missing or bad
    /// value; the world treats that as 0 (and the shipped file has them all).
    pub fn param(&self, name: &str) -> i32 {
        self.params
            .get(name)
            .and_then(|v| v.trim().parse::<i32>().ok())
            .unwrap_or(0)
    }
}

/// The arenas of both games, by id.
#[derive(Debug, Default, Clone)]
pub struct MinigameData {
    coliseum: BTreeMap<i32, ColiseumArenaDef>,
    krateis: BTreeMap<i32, KrateisArenaDef>,
}

impl MinigameData {
    const UC_LOADER: &'static str = "UndergroundColiseumManager";
    const KRATEIS_LOADER: &'static str = "KrateisCubeManager";

    /// Loads both files under `<root>/data`. A missing file is Java's warning and no arenas.
    pub fn load(root: &Path) -> Self {
        let mut data = Self::default();
        load_file(
            &root.join("data/UndergroundColiseum.xml"),
            |doc: RawColiseumList| {
                data.add_coliseum(doc);
                Ok(())
            },
        );
        log_loaded(Self::UC_LOADER, data.coliseum.len(), "coliseum arenas");
        load_file(&root.join("data/KrateisCube.xml"), |doc: RawKrateisList| {
            data.add_krateis(doc);
            Ok(())
        });
        log_loaded(Self::KRATEIS_LOADER, data.krateis.len(), "arena templates");
        data
    }

    /// Builds the coliseum arenas from XML text. For tests and tools.
    pub fn coliseum_from_xml(xml: &str) -> Result<Self, DataError> {
        let doc: RawColiseumList = quick_xml::de::from_str(xml).map_err(|source| DataError::Xml {
            path: "<inline>".into(),
            source,
        })?;
        let mut data = Self::default();
        data.add_coliseum(doc);
        Ok(data)
    }

    /// Builds the Kratei's Cube arenas from XML text. For tests and tools.
    pub fn krateis_from_xml(xml: &str) -> Result<Self, DataError> {
        let doc: RawKrateisList = quick_xml::de::from_str(xml).map_err(|source| DataError::Xml {
            path: "<inline>".into(),
            source,
        })?;
        let mut data = Self::default();
        data.add_krateis(doc);
        Ok(data)
    }

    fn add_coliseum(&mut self, doc: RawColiseumList) {
        for raw in doc.arenas {
            let def = ColiseumArenaDef {
                id: raw.id,
                min_level: raw.min_lvl,
                max_level: raw.max_lvl,
                curator: raw.curator,
                towers: raw
                    .towers
                    .iter()
                    .map(|t| ColiseumTower {
                        npc_id: t.id,
                        place: (t.x, t.y, t.z),
                    })
                    .collect(),
                points: raw
                    .spawns
                    .iter()
                    .map(|s| ColiseumPoint {
                        doors: parse_ids(s.doors.as_deref()),
                        place: (s.x, s.y, s.z),
                    })
                    .collect(),
                rewards: raw
                    .rewards
                    .into_iter()
                    .flat_map(|r| r.items)
                    .map(reward)
                    .collect(),
            };
            // `_arenas.put(arena.getId(), arena)`.
            self.coliseum.insert(def.id, def);
        }
    }

    fn add_krateis(&mut self, doc: RawKrateisList) {
        for raw in doc.arenas {
            let mut params = BTreeMap::new();
            for block in &raw.params {
                for set in &block.sets {
                    params.insert(set.name.clone(), set.value.clone());
                }
            }
            let places = |lists: &[RawPlaces]| -> Vec<Place> {
                lists
                    .iter()
                    .flat_map(|l| l.points.iter().map(|p| (p.x, p.y, p.z)))
                    .collect()
            };
            let doors = |lists: &[RawDoors]| -> Vec<i32> {
                lists.iter().flat_map(|l| parse_ids(l.list.as_deref())).collect()
            };
            let def = KrateisArenaDef {
                id: raw.id,
                manager: raw.manager,
                min_level: raw.min_level,
                max_level: raw.max_level,
                min_players: raw.min_players,
                max_players: raw.max_players,
                doors_a: doors(&raw.door_a),
                doors_b: doors(&raw.door_b),
                wait_locations: places(&raw.wait),
                battle_locations: places(&raw.battle),
                watcher_locations: places(&raw.watcher),
                buffs: raw
                    .buffs
                    .iter()
                    .flat_map(|b| b.skills.iter().map(|s| (s.id, s.level)))
                    .collect(),
                rewards: raw
                    .rewards
                    .into_iter()
                    .flat_map(|r| r.items)
                    .map(reward)
                    .collect(),
                params,
            };
            self.krateis.insert(def.id, def);
        }
    }

    /// Adds the arenas of `other` (for tests that build both games from text).
    pub fn merge(&mut self, other: Self) {
        self.coliseum.extend(other.coliseum);
        self.krateis.extend(other.krateis);
    }

    pub fn coliseum_arena(&self, id: i32) -> Option<&ColiseumArenaDef> {
        self.coliseum.get(&id)
    }

    /// The coliseum arenas by id.
    pub fn coliseum_arenas(&self) -> impl Iterator<Item = &ColiseumArenaDef> {
        self.coliseum.values()
    }

    pub fn krateis_arena(&self, id: i32) -> Option<&KrateisArenaDef> {
        self.krateis.get(&id)
    }

    /// The Kratei's Cube arenas by id.
    pub fn krateis_arenas(&self) -> impl Iterator<Item = &KrateisArenaDef> {
        self.krateis.values()
    }
}

fn reward(raw: RawItem) -> ArenaReward {
    ArenaReward {
        id: raw.id,
        amount: raw.amount,
        use_modifier: raw
            .use_modifiers
            .as_deref()
            .is_some_and(|v| v.trim().eq_ignore_ascii_case("true")),
    }
}

/// `doorList.split(",")`; an id that is not a number is skipped (Java would throw).
fn parse_ids(list: Option<&str>) -> Vec<i32> {
    list.unwrap_or("")
        .split(',')
        .filter_map(|id| id.trim().parse::<i32>().ok())
        .collect()
}

#[derive(Debug, Deserialize)]
struct RawColiseumList {
    #[serde(rename = "arena", default)]
    arenas: Vec<RawColiseumArena>,
}

#[derive(Debug, Deserialize)]
struct RawColiseumArena {
    #[serde(rename = "@id")]
    id: i32,
    #[serde(rename = "@minLvl")]
    min_lvl: i32,
    #[serde(rename = "@maxLvl")]
    max_lvl: i32,
    #[serde(rename = "@curator")]
    curator: i32,
    #[serde(rename = "tower", default)]
    towers: Vec<RawTower>,
    #[serde(rename = "spawn", default)]
    spawns: Vec<RawSpawn>,
    #[serde(rename = "rewards", default)]
    rewards: Vec<RawRewards>,
}

#[derive(Debug, Deserialize)]
struct RawTower {
    #[serde(rename = "@id")]
    id: i32,
    #[serde(rename = "@x")]
    x: i32,
    #[serde(rename = "@y")]
    y: i32,
    #[serde(rename = "@z")]
    z: i32,
}

#[derive(Debug, Deserialize)]
struct RawSpawn {
    #[serde(rename = "@doors")]
    doors: Option<String>,
    #[serde(rename = "@x")]
    x: i32,
    #[serde(rename = "@y")]
    y: i32,
    #[serde(rename = "@z")]
    z: i32,
}

#[derive(Debug, Deserialize)]
struct RawRewards {
    #[serde(rename = "item", default)]
    items: Vec<RawItem>,
}

#[derive(Debug, Deserialize)]
struct RawItem {
    #[serde(rename = "@id")]
    id: i32,
    #[serde(rename = "@amount")]
    amount: i64,
    #[serde(rename = "@useModifers")]
    use_modifiers: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawKrateisList {
    #[serde(rename = "arena", default)]
    arenas: Vec<RawKrateisArena>,
}

#[derive(Debug, Deserialize)]
struct RawKrateisArena {
    #[serde(rename = "@id")]
    id: i32,
    #[serde(rename = "@manager")]
    manager: i32,
    #[serde(rename = "@minLevel")]
    min_level: i32,
    #[serde(rename = "@maxLevel")]
    max_level: i32,
    #[serde(rename = "@minPlayers")]
    min_players: i32,
    #[serde(rename = "@maxPlayers")]
    max_players: i32,
    #[serde(rename = "doorA", default)]
    door_a: Vec<RawDoors>,
    #[serde(rename = "doorB", default)]
    door_b: Vec<RawDoors>,
    #[serde(rename = "waitLocations", default)]
    wait: Vec<RawPlaces>,
    #[serde(rename = "battleLocations", default)]
    battle: Vec<RawPlaces>,
    #[serde(rename = "watcherLocations", default)]
    watcher: Vec<RawPlaces>,
    #[serde(rename = "buffs", default)]
    buffs: Vec<RawBuffs>,
    #[serde(rename = "rewards", default)]
    rewards: Vec<RawRewards>,
    #[serde(rename = "add_parameters", default)]
    params: Vec<RawParams>,
}

#[derive(Debug, Deserialize)]
struct RawDoors {
    #[serde(rename = "@list")]
    list: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawPlaces {
    #[serde(rename = "point", default)]
    points: Vec<RawTower2>,
}

#[derive(Debug, Deserialize)]
struct RawTower2 {
    #[serde(rename = "@x")]
    x: i32,
    #[serde(rename = "@y")]
    y: i32,
    #[serde(rename = "@z")]
    z: i32,
}

#[derive(Debug, Deserialize)]
struct RawBuffs {
    #[serde(rename = "skill", default)]
    skills: Vec<RawSkill>,
}

#[derive(Debug, Deserialize)]
struct RawSkill {
    #[serde(rename = "@id")]
    id: i32,
    #[serde(rename = "@level")]
    level: i32,
}

#[derive(Debug, Deserialize)]
struct RawParams {
    #[serde(rename = "set", default)]
    sets: Vec<RawSet>,
}

#[derive(Debug, Deserialize)]
struct RawSet {
    #[serde(rename = "@name")]
    name: String,
    #[serde(rename = "@value")]
    value: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coliseum_arenas_are_read() {
        let xml = r#"<list>
            <arena id="1" minLvl="40" maxLvl="49" curator="32513">
                <tower id="18539" x="1" y="2" z="3" />
                <tower id="18539" x="4" y="5" z="6" />
                <spawn doors="17160037,17160038" x="7" y="8" z="9" />
                <spawn x="10" y="11" z="12" />
                <rewards><item id="-300" amount="20" useModifers="true" /><item id="57" amount="5" /></rewards>
            </arena>
        </list>"#;
        let data = MinigameData::coliseum_from_xml(xml).unwrap();
        let arena = data.coliseum_arena(1).unwrap();
        assert_eq!((arena.min_level, arena.max_level, arena.curator), (40, 49, 32513));
        assert_eq!(arena.towers[1].place, (4, 5, 6));
        assert_eq!(arena.points[0].doors, vec![17160037, 17160038]);
        assert!(arena.points[1].doors.is_empty());
        assert_eq!(
            arena.rewards,
            vec![
                ArenaReward {
                    id: -300,
                    amount: 20,
                    use_modifier: true
                },
                ArenaReward {
                    id: 57,
                    amount: 5,
                    use_modifier: false
                }
            ]
        );
    }

    #[test]
    fn krateis_arenas_are_read() {
        let xml = r#"<list>
            <arena id="1" manager="32504" minLevel="70" maxLevel="75" minPlayers="2" maxPlayers="25">
                <doorA list="1,2" /><doorB list="3" />
                <waitLocations><point x="1" y="2" z="3" /></waitLocations>
                <battleLocations><point x="4" y="5" z="6" /><point x="7" y="8" z="9" /></battleLocations>
                <buffs><skill id="1086" level="2" /></buffs>
                <rewards><item id="13067" amount="1" useModifers="true" /></rewards>
                <add_parameters><set name="minPoints" value="10" /></add_parameters>
            </arena>
        </list>"#;
        let data = MinigameData::krateis_from_xml(xml).unwrap();
        let arena = data.krateis_arena(1).unwrap();
        assert_eq!(arena.doors_a, vec![1, 2]);
        assert_eq!(arena.battle_locations.len(), 2);
        assert!(arena.watcher_locations.is_empty());
        assert_eq!(arena.buffs, vec![(1086, 2)]);
        assert_eq!(arena.param("minPoints"), 10);
        assert_eq!(arena.param("missing"), 0);
    }

    #[test]
    fn the_shipped_files_load_when_present() {
        let root = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game"));
        if !root.join("data/UndergroundColiseum.xml").exists() {
            eprintln!("skipped: the datapack is not here");
            return;
        }
        let data = MinigameData::load(root);
        assert_eq!(data.coliseum_arenas().count(), 5);
        assert_eq!(data.coliseum_arena(1).unwrap().points.len(), 4);
        assert_eq!(data.krateis_arenas().count(), 3);
        assert_eq!(data.krateis_arena(1).unwrap().param("pointPerPlayer"), 3);
    }
}
