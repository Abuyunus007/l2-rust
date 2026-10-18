//! Map regions: which town area a position belongs to, and where a player restarts.
//!
//! Java: `data/xml/MapRegionData.java`. Data: `data/mapregion/*.xml`, `xsd/MapRegion.xsd`.
//!
//! A map region is a named group of 32768-unit map tiles (`<map X Y/>`, tile = `(pos >> 15) +
//! offset`) with a town name, a client location id (`locId`, shown in the "you are in" text), a
//! castle id, a bulletin-board id and its restart points. [`MapRegions::region_at`] is Java's
//! `getMapRegion(x, y)`, the restart functions are `getNearestTownRespawn` and friends.
//!
//! # What is not here
//! `getTeleToLocation` (clan hall / castle / fort / siege flag / instance / faction targets)
//! needs player, clan and residence state; the world builds it from these pieces and the
//! `getNearest...` / `getRestart...` functions below. Random restart points need the world's
//! RNG, so every function that picks a point takes `random` (see [`crate::zone::first_spawn`]).
//!
//! # Races
//! Races are Java enum names as text (`"HUMAN"`, `"DARK_ELF"`, ...). Java's `Race.valueOf` throws
//! for an unknown name; such a region is skipped here.
//!
//! NOTE(java-quirk): two regions can claim the same tile (e.g. `darkelf_town` and `kamael_town`
//! both list 17,19). Java builds the tile index by walking its `HashMap<String, RegionData>`, so
//! the region that comes last in *hash order* wins. That order is reproduced
//! (`String.hashCode`, table growth, insertion order inside a bucket).

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;
use tracing::warn;

use crate::loader::{DataError, load_dir, log_loaded};
use crate::player_template::Location;
use crate::zone::{JAVA_RACES, Zones};

/// `MapRegionData.DEFAULT_RESPAWN`.
pub const DEFAULT_RESPAWN: &str = "talking_island_town";
/// `MapRegionData.FALLBACK_FLYING_RESPAWN`.
pub const FALLBACK_FLYING_RESPAWN: &str = "union_base_of_kserth";
/// `MapRegionData.DEFAULT_TOWN`: the town name when a position has no region.
pub const DEFAULT_TOWN: &str = "Aden Castle Town";
const MAP_REGION_X_OFFSET: i32 = 20;
const MAP_REGION_Y_OFFSET: i32 = 18;
const COORDINATE_SHIFT: i32 = 15;
const GRID_SIZE: usize = 32;
/// Guard for the "banned race redirects to another region" chain. Java recurses without limit
/// (a cycle would end in a `StackOverflowError`); here a cycle falls back to the default region.
const MAX_REDIRECTS: usize = 32;

/// One `<region>`. Java `MapRegionData.RegionData`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapRegion {
    /// `name`: the key restart points refer to.
    pub name: String,
    /// `town`: the town name (`getClosestTownName`).
    pub town: String,
    /// `locId`: client location id.
    pub loc_id: i32,
    /// `castle`: castle id of the area (`getAreaCastle`).
    pub castle: i32,
    /// `bbs`: bulletin board id.
    pub bbs: i32,
    /// `<map X Y/>` tiles.
    pub maps: Vec<(i32, i32)>,
    /// `<respawnPoint>` without a flag.
    pub spawn_locs: Vec<Location>,
    /// `<respawnPoint isOther="true">`.
    pub other_spawn_locs: Vec<Location>,
    /// `<respawnPoint isChaotic="true">` (karma players).
    pub chaotic_spawn_locs: Vec<Location>,
    /// `<respawnPoint isBanish="true">`.
    pub banish_spawn_locs: Vec<Location>,
    /// `<banned race point>`: players of this race restart at the named region instead. A later
    /// entry for the same race replaces the earlier one.
    pub banned_races: Vec<(String, String)>,
}

impl MapRegion {
    /// The region a banned race is redirected to, as a name.
    pub fn banned_point(&self, race: &str) -> Option<&str> {
        self.banned_races
            .iter()
            .find(|(r, _)| r == race)
            .map(|(_, p)| p.as_str())
    }
}

/// All map regions. Immutable once loaded.
#[derive(Debug, Clone)]
pub struct MapRegions {
    /// In first-insertion order of the name.
    regions: Vec<MapRegion>,
    by_name: HashMap<String, usize>,
    /// Java's `_spatialGrid[x][y]` as `x * GRID_SIZE + y`.
    grid: Vec<Option<usize>>,
    default_respawn: Option<usize>,
    fallback_flying: Option<usize>,
}

impl MapRegions {
    /// Loads `data/mapregion/*.xml` below `root`, logging Java's count line.
    pub fn load(root: &Path) -> Self {
        let mut regions: Vec<MapRegion> = Vec::new();
        let mut by_name: HashMap<String, usize> = HashMap::new();
        load_dir::<RawMapFile, _>(&root.join("data").join("mapregion"), false, |doc| {
            for child in doc.children {
                let RawMapChild::Region(raw) = child else { continue };
                match build_region(raw) {
                    // `_regions.put`: a repeated name replaces the region but keeps its position.
                    Ok(region) => match by_name.get(&region.name) {
                        Some(&i) => regions[i] = region,
                        None => {
                            by_name.insert(region.name.clone(), regions.len());
                            regions.push(region);
                        }
                    },
                    Err(error) => warn!("MapRegionData: {error}"),
                }
            }
            Ok(())
        });

        // buildSpatialIndex: walk the regions in HashMap order, the last one wins a tile.
        let names: Vec<&str> = regions.iter().map(|r| r.name.as_str()).collect();
        let mut grid = vec![None; GRID_SIZE * GRID_SIZE];
        for i in java_hashmap_order(&names) {
            for &(x, y) in &regions[i].maps {
                if (0..GRID_SIZE as i32).contains(&x) && (0..GRID_SIZE as i32).contains(&y) {
                    grid[x as usize * GRID_SIZE + y as usize] = Some(i);
                }
            }
        }
        let default_respawn = by_name.get(DEFAULT_RESPAWN).copied();
        let fallback_flying = by_name.get(FALLBACK_FLYING_RESPAWN).copied();
        log_loaded("MapRegionData", regions.len(), "map regions");
        Self {
            regions,
            by_name,
            grid,
            default_respawn,
            fallback_flying,
        }
    }

    /// Number of regions (Java `_regions.size()`).
    pub fn len(&self) -> usize {
        self.regions.len()
    }

    /// `true` when nothing is loaded.
    pub fn is_empty(&self) -> bool {
        self.regions.is_empty()
    }

    /// All regions in first-seen order.
    pub fn iter(&self) -> impl Iterator<Item = &MapRegion> {
        self.regions.iter()
    }

    /// A region by its `name`.
    pub fn get(&self, name: &str) -> Option<&MapRegion> {
        self.by_name.get(name).map(|&i| &self.regions[i])
    }

    /// Java `getMapRegionX(posX)`.
    pub fn map_region_x(pos_x: i32) -> i32 {
        (pos_x >> COORDINATE_SHIFT) + MAP_REGION_X_OFFSET
    }

    /// Java `getMapRegionY(posY)`.
    pub fn map_region_y(pos_y: i32) -> i32 {
        (pos_y >> COORDINATE_SHIFT) + MAP_REGION_Y_OFFSET
    }

    /// Java `getMapRegion(locX, locY)`: the region that owns the map tile, `None` outside.
    pub fn region_at(&self, x: i32, y: i32) -> Option<&MapRegion> {
        let (rx, ry) = (Self::map_region_x(x), Self::map_region_y(y));
        if (0..GRID_SIZE as i32).contains(&rx) && (0..GRID_SIZE as i32).contains(&ry) {
            self.grid[rx as usize * GRID_SIZE + ry as usize].map(|i| &self.regions[i])
        } else {
            None
        }
    }

    /// Java `getMapRegionLocId(x, y)`: 0 when there is no region.
    pub fn map_region_loc_id(&self, x: i32, y: i32) -> i32 {
        self.region_at(x, y).map_or(0, |r| r.loc_id)
    }

    /// Java `getClosestTownName`: the region's town, or `"Aden Castle Town"` outside any region.
    pub fn closest_town_name(&self, x: i32, y: i32) -> &str {
        self.region_at(x, y).map_or(DEFAULT_TOWN, |r| r.town.as_str())
    }

    /// Java `getAreaCastle`: 0 when there is no region.
    pub fn area_castle(&self, x: i32, y: i32) -> i32 {
        self.region_at(x, y).map_or(0, |r| r.castle)
    }

    /// Java `getBBs(loc)`: the region's bulletin board id, else the default region's, else 0.
    pub fn bbs(&self, x: i32, y: i32) -> i32 {
        match self.region_at(x, y) {
            Some(region) => region.bbs,
            None => self.default_region().map_or(0, |r| r.bbs),
        }
    }

    /// The region named [`DEFAULT_RESPAWN`], if loaded.
    pub fn default_region(&self) -> Option<&MapRegion> {
        self.default_respawn.map(|i| &self.regions[i])
    }

    /// Java `getMapRegionTown(regionName)`.
    pub fn region_town(&self, name: &str) -> Option<&str> {
        self.get(name).map(|r| r.town.as_str())
    }

    /// Java `getSpawnLocByRegionName`.
    pub fn spawn_loc_by_region_name(
        &self,
        name: &str,
        random: &mut dyn FnMut(usize) -> usize,
    ) -> Option<Location> {
        self.get(name).and_then(|r| spawn_loc(r, random))
    }

    /// Java `getNearestTownRespawn(creature)`. `race` is the Java race name of a player, `None`
    /// for any other creature. A `RespawnZone` at the position wins over the map region.
    /// `None` is Java's `null` (no restart point at all).
    pub fn nearest_town_respawn(
        &self,
        zones: &Zones,
        pos: Location,
        race: Option<&str>,
        random: &mut dyn FnMut(usize) -> usize,
    ) -> Option<Location> {
        let Location { x, y, z } = pos;
        if let Some(zone) = zones.zone_at(x, y, z, "RespawnZone") {
            return match race {
                Some(race) => self.restart_region_spawn_loc(race, zone.respawn_point(race), random),
                // `creature.asPlayer()` is null for non-players: Java's NPE lands in the catch
                // and uses the default region.
                None => self.default_region().and_then(|r| spawn_loc(r, random)),
            };
        }
        if let Some(region) = self.region_at(x, y) {
            if let Some(race) = race
                && let Some(point) = region.banned_point(race)
                && let Some(alternate) = self.get(point)
            {
                return spawn_loc(alternate, random);
            }
            return spawn_loc(region, random);
        }
        self.default_region().and_then(|r| spawn_loc(r, random))
    }

    /// Java `getNearestKarmaRespawn(player)`: like [`nearest_town_respawn`](Self::nearest_town_respawn)
    /// with the chaotic restart points, for players with karma. `flying_mounted` selects the
    /// flying fallback region when the position has neither a zone nor a region.
    pub fn nearest_karma_respawn(
        &self,
        zones: &Zones,
        pos: Location,
        race: &str,
        flying_mounted: bool,
        random: &mut dyn FnMut(usize) -> usize,
    ) -> Option<Location> {
        let Location { x, y, z } = pos;
        if let Some(zone) = zones.zone_at(x, y, z, "RespawnZone") {
            return self.restart_region_chaotic_spawn_loc(race, zone.respawn_point(race), random);
        }
        if let Some(region) = self.region_at(x, y) {
            if let Some(point) = region.banned_point(race)
                && let Some(alternate) = self.get(point)
            {
                return chaotic_spawn_loc(alternate, random);
            }
            return chaotic_spawn_loc(region, random);
        }
        if flying_mounted && let Some(i) = self.fallback_flying {
            return chaotic_spawn_loc(&self.regions[i], random);
        }
        self.default_region().and_then(|r| chaotic_spawn_loc(r, random))
    }

    /// Java `getRestartRegionSpawnLoc(creature, point)`: the region named `point` (the default
    /// region when `point` is `None` or unknown), following banned-race redirects.
    pub fn restart_region_spawn_loc(
        &self,
        race: &str,
        point: Option<&str>,
        random: &mut dyn FnMut(usize) -> usize,
    ) -> Option<Location> {
        let mut point = point;
        for _ in 0..MAX_REDIRECTS {
            let region = point
                .and_then(|p| self.get(p))
                .or_else(|| self.default_region())?;
            match region.banned_point(race) {
                Some(next) => point = Some(next),
                None => return spawn_loc(region, random),
            }
        }
        self.default_region().and_then(|r| spawn_loc(r, random))
    }

    /// Java `getRestartRegionChaoticSpawnLoc(creature, point)`.
    pub fn restart_region_chaotic_spawn_loc(
        &self,
        race: &str,
        point: Option<&str>,
        random: &mut dyn FnMut(usize) -> usize,
    ) -> Option<Location> {
        let mut point = point;
        for _ in 0..MAX_REDIRECTS {
            let region = point
                .and_then(|p| self.get(p))
                .or_else(|| self.default_region())?;
            match region.banned_point(race) {
                Some(next) => point = Some(next),
                None => return chaotic_spawn_loc(region, random),
            }
        }
        self.default_region().and_then(|r| chaotic_spawn_loc(r, random))
    }

    /// Java `getRestartRegionLocId(creature, point)`: the `locId` of the restart region, 0 when
    /// there is none. An unknown `point` gives the default region's id (Java: before checking
    /// the banned races).
    pub fn restart_region_loc_id(&self, race: &str, point: Option<&str>) -> i32 {
        let default_id = self.default_region().map_or(0, |r| r.loc_id);
        let mut point = point;
        for _ in 0..MAX_REDIRECTS {
            let Some(region) = point.and_then(|p| self.get(p)) else {
                return default_id;
            };
            match region.banned_point(race) {
                Some(next) => point = Some(next),
                None => return region.loc_id,
            }
        }
        default_id
    }
}

/// Java `getSpawnLoc(region)`: `None` when the region has no normal restart point.
fn spawn_loc(region: &MapRegion, random: &mut dyn FnMut(usize) -> usize) -> Option<Location> {
    pick(&region.spawn_locs, random)
}

/// Java `getChaoticSpawnLoc(region)`: falls back to the normal restart points.
fn chaotic_spawn_loc(region: &MapRegion, random: &mut dyn FnMut(usize) -> usize) -> Option<Location> {
    if region.chaotic_spawn_locs.is_empty() {
        spawn_loc(region, random)
    } else {
        pick(&region.chaotic_spawn_locs, random)
    }
}

fn pick(list: &[Location], random: &mut dyn FnMut(usize) -> usize) -> Option<Location> {
    if list.is_empty() {
        None
    } else {
        // The index is clamped so a bad random source cannot panic.
        list.get(random(list.len())).or_else(|| list.first()).copied()
    }
}

/// Java `String.hashCode` (over UTF-16 code units).
fn java_string_hash(s: &str) -> i32 {
    s.encode_utf16()
        .fold(0i32, |h, c| h.wrapping_mul(31).wrapping_add(i32::from(c)))
}

/// Indexes into `keys` (given in first-insertion order) in the order a Java `HashMap<String, _>`
/// iterates them: bucket order, then insertion order inside a bucket. Table size: 16, doubled
/// whenever the entry count exceeds 3/4 of it. (Treeified buckets are not modelled.)
fn java_hashmap_order(keys: &[&str]) -> Vec<usize> {
    let mut table = 16usize;
    while keys.len() > table - table / 4 {
        table *= 2;
    }
    let mut order: Vec<usize> = (0..keys.len()).collect();
    // Stable sort keeps the insertion order within one bucket.
    order.sort_by_key(|&i| {
        let h = java_string_hash(keys[i]) as u32;
        ((h ^ (h >> 16)) as usize) & (table - 1)
    });
    order
}

// Raw XML shape (xsd/MapRegion.xsd). Attributes are text so one broken region is skipped alone.
#[derive(Deserialize)]
struct RawMapFile {
    #[serde(rename = "$value", default)]
    children: Vec<RawMapChild>,
}

#[derive(Deserialize)]
enum RawMapChild {
    #[serde(rename = "region")]
    Region(RawRegion),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct RawRegion {
    #[serde(rename = "@name")]
    name: Option<String>,
    #[serde(rename = "@town")]
    town: Option<String>,
    #[serde(rename = "@locId")]
    loc_id: Option<String>,
    #[serde(rename = "@castle")]
    castle: Option<String>,
    #[serde(rename = "@bbs")]
    bbs: Option<String>,
    #[serde(rename = "$value", default)]
    children: Vec<RawRegionChild>,
}

#[derive(Deserialize)]
enum RawRegionChild {
    #[serde(rename = "respawnPoint")]
    RespawnPoint(RawPoint),
    #[serde(rename = "map")]
    Map(RawTile),
    #[serde(rename = "banned")]
    Banned(RawBanned),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct RawPoint {
    #[serde(rename = "@X")]
    x: Option<String>,
    #[serde(rename = "@Y")]
    y: Option<String>,
    #[serde(rename = "@Z")]
    z: Option<String>,
    #[serde(rename = "@isOther")]
    is_other: Option<String>,
    #[serde(rename = "@isChaotic")]
    is_chaotic: Option<String>,
    #[serde(rename = "@isBanish")]
    is_banish: Option<String>,
}

#[derive(Deserialize)]
struct RawTile {
    #[serde(rename = "@X")]
    x: Option<String>,
    #[serde(rename = "@Y")]
    y: Option<String>,
}

#[derive(Deserialize)]
struct RawBanned {
    #[serde(rename = "@race")]
    race: Option<String>,
    #[serde(rename = "@point")]
    point: Option<String>,
}

fn int_attr(value: &Option<String>, what: &str) -> Result<i32, DataError> {
    let text = value
        .as_deref()
        .ok_or_else(|| DataError::Invalid(format!("missing {what}")))?;
    text.parse()
        .map_err(|_| DataError::Invalid(format!("{what} is not an integer: {text}")))
}

/// `IXmlReader.parseBoolean(attrs, name, false)`: only "true" (any case) is true.
fn bool_attr(value: &Option<String>) -> bool {
    value.as_deref().is_some_and(|v| v.eq_ignore_ascii_case("true"))
}

fn build_region(raw: RawRegion) -> Result<MapRegion, DataError> {
    let name = raw
        .name
        .clone()
        .ok_or_else(|| DataError::Invalid("region without name".to_owned()))?;
    let town = raw
        .town
        .clone()
        .ok_or_else(|| DataError::Invalid(format!("region {name} without town")))?;
    let mut region = MapRegion {
        loc_id: int_attr(&raw.loc_id, "locId").map_err(|e| in_region(&name, e))?,
        castle: int_attr(&raw.castle, "castle").map_err(|e| in_region(&name, e))?,
        bbs: int_attr(&raw.bbs, "bbs").map_err(|e| in_region(&name, e))?,
        name,
        town,
        maps: Vec::new(),
        spawn_locs: Vec::new(),
        other_spawn_locs: Vec::new(),
        chaotic_spawn_locs: Vec::new(),
        banish_spawn_locs: Vec::new(),
        banned_races: Vec::new(),
    };
    for child in &raw.children {
        let name = region.name.clone();
        match child {
            RawRegionChild::RespawnPoint(p) => {
                let loc = Location {
                    x: int_attr(&p.x, "respawnPoint X").map_err(|e| in_region(&name, e))?,
                    y: int_attr(&p.y, "respawnPoint Y").map_err(|e| in_region(&name, e))?,
                    z: int_attr(&p.z, "respawnPoint Z").map_err(|e| in_region(&name, e))?,
                };
                // The flags are exclusive in this order (if / else if).
                if bool_attr(&p.is_other) {
                    region.other_spawn_locs.push(loc);
                } else if bool_attr(&p.is_chaotic) {
                    region.chaotic_spawn_locs.push(loc);
                } else if bool_attr(&p.is_banish) {
                    region.banish_spawn_locs.push(loc);
                } else {
                    region.spawn_locs.push(loc);
                }
            }
            RawRegionChild::Map(m) => region.maps.push((
                int_attr(&m.x, "map X").map_err(|e| in_region(&name, e))?,
                int_attr(&m.y, "map Y").map_err(|e| in_region(&name, e))?,
            )),
            RawRegionChild::Banned(b) => {
                let (Some(race), Some(point)) = (&b.race, &b.point) else {
                    return Err(DataError::Invalid(format!(
                        "region {name}: banned without race/point"
                    )));
                };
                if !JAVA_RACES.contains(&race.as_str()) {
                    return Err(DataError::Invalid(format!("region {name}: unknown race {race}")));
                }
                match region.banned_races.iter_mut().find(|(r, _)| r == race) {
                    Some(entry) => entry.1 = point.clone(),
                    None => region.banned_races.push((race.clone(), point.clone())),
                }
            }
            RawRegionChild::Other => {}
        }
    }
    Ok(region)
}

fn in_region(region: &str, error: DataError) -> DataError {
    DataError::Invalid(format!("region {region}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zone::first_spawn;

    fn load_text(zones_xml: Option<&str>, regions_xml: &str) -> (MapRegions, Zones) {
        let dir = tempfile::tempdir().unwrap();
        let map_dir = dir.path().join("data").join("mapregion");
        let zone_dir = dir.path().join("data").join("zones");
        std::fs::create_dir_all(&map_dir).unwrap();
        std::fs::create_dir_all(&zone_dir).unwrap();
        std::fs::write(map_dir.join("a.xml"), regions_xml).unwrap();
        if let Some(text) = zones_xml {
            std::fs::write(zone_dir.join("z.xml"), text).unwrap();
        }
        (MapRegions::load(dir.path()), Zones::load(dir.path()))
    }

    fn loc(x: i32, y: i32, z: i32) -> Location {
        Location { x, y, z }
    }

    const REGIONS: &str = r#"<list>
        <region name="talking_island_town" town="Talking Island Town" castle="1" locId="910" bbs="1">
            <respawnPoint X="1" Y="2" Z="3" />
            <respawnPoint X="4" Y="5" Z="6" />
            <respawnPoint X="7" Y="8" Z="9" isChaotic="true" />
            <map X="15" Y="23" />
        </region>
        <region name="elf_town" town="Elven Village" castle="2" locId="911" bbs="2">
            <respawnPoint X="10" Y="20" Z="30" />
            <map X="21" Y="19" />
            <banned race="DARK_ELF" point="darkelf_town" />
        </region>
        <region name="darkelf_town" town="Dark Elven Village" castle="2" locId="912" bbs="3">
            <respawnPoint X="40" Y="50" Z="60" />
            <banned race="HUMAN" point="loop_a" />
        </region>
        <region name="loop_a" town="A" castle="0" locId="1" bbs="0"><respawnPoint X="1" Y="1" Z="1" /><banned race="ORC" point="loop_b" /></region>
        <region name="loop_b" town="B" castle="0" locId="2" bbs="0"><respawnPoint X="2" Y="2" Z="2" /><banned race="ORC" point="loop_a" /></region>
        <region name="bad" town="Bad" castle="0" locId="x" bbs="0" />
        <region name="badrace" town="Bad" castle="0" locId="0" bbs="0"><banned race="GOBLIN" point="elf_town" /></region>
    </list>"#;

    // Tile 15,23 covers x in [-163840, -131072) and y in [163840, 196608).
    const TALKING: (i32, i32) = (-84_000, 243_000);

    #[test]
    fn tile_maths_and_lookups() {
        let (regions, _) = load_text(None, REGIONS);
        assert_eq!(regions.len(), 5, "bad locId and unknown race skip the region");
        assert_eq!(MapRegions::map_region_x(TALKING.0), 17);
        assert_eq!(MapRegions::map_region_y(TALKING.1), 25);
        // The test region owns tile 15,23: x in [-163840, -131072), y in [163840, 196608).
        assert_eq!(regions.map_region_loc_id(-150_000, 170_000), 910);
        assert_eq!(
            regions.closest_town_name(-150_000, 170_000),
            "Talking Island Town"
        );
        assert_eq!(regions.area_castle(-150_000, 170_000), 1);
        assert_eq!(regions.bbs(-150_000, 170_000), 1);
        // Outside every region.
        assert_eq!(regions.map_region_loc_id(0, 0), 0);
        assert_eq!(regions.closest_town_name(0, 0), "Aden Castle Town");
        assert_eq!(regions.area_castle(0, 0), 0);
        assert_eq!(regions.bbs(0, 0), 1, "the default region's bbs");
        assert_eq!(regions.region_town("elf_town"), Some("Elven Village"));
        assert_eq!(regions.region_town("nope"), None);
    }

    #[test]
    fn restart_points() {
        let (regions, zones) = load_text(None, REGIONS);
        let mut first = first_spawn;
        // In the region: first normal point, or the chaotic one for karma players.
        assert_eq!(
            regions.nearest_town_respawn(
                &zones,
                Location {
                    x: -150_000,
                    y: 170_000,
                    z: 0
                },
                Some("HUMAN"),
                &mut first
            ),
            Some(loc(1, 2, 3))
        );
        assert_eq!(
            regions.nearest_karma_respawn(
                &zones,
                Location {
                    x: -150_000,
                    y: 170_000,
                    z: 0
                },
                "HUMAN",
                false,
                &mut first
            ),
            Some(loc(7, 8, 9))
        );
        // A random source picks by index.
        assert_eq!(
            regions.nearest_town_respawn(
                &zones,
                Location {
                    x: -150_000,
                    y: 170_000,
                    z: 0
                },
                Some("HUMAN"),
                &mut |len| len - 1
            ),
            Some(loc(4, 5, 6))
        );
        // Elf town: dark elves are sent to the dark elf town.
        let elf_pos = 32768 + 100; // tile x 21
        let elf_y = 32768 + 100; // tile y 19
        assert_eq!(
            regions.nearest_town_respawn(
                &zones,
                Location {
                    x: elf_pos,
                    y: elf_y,
                    z: 0
                },
                Some("ELF"),
                &mut first
            ),
            Some(loc(10, 20, 30))
        );
        assert_eq!(
            regions.nearest_town_respawn(
                &zones,
                Location {
                    x: elf_pos,
                    y: elf_y,
                    z: 0
                },
                Some("DARK_ELF"),
                &mut first
            ),
            Some(loc(40, 50, 60))
        );
        // No region: the default region. A non-player creature gets the same.
        assert_eq!(
            regions.nearest_town_respawn(&zones, Location { x: 0, y: 0, z: 0 }, None, &mut first),
            Some(loc(1, 2, 3))
        );
        assert_eq!(
            regions.spawn_loc_by_region_name("elf_town", &mut first),
            Some(loc(10, 20, 30))
        );
        assert_eq!(regions.spawn_loc_by_region_name("nope", &mut first), None);
    }

    #[test]
    fn restart_region_redirects_and_cycles() {
        let (regions, _) = load_text(None, REGIONS);
        let mut first = first_spawn;
        // Redirect chain: elf_town -> (dark elf) darkelf_town -> (human) loop_a.
        assert_eq!(
            regions.restart_region_spawn_loc("DARK_ELF", Some("elf_town"), &mut first),
            Some(loc(40, 50, 60))
        );
        assert_eq!(
            regions.restart_region_spawn_loc("HUMAN", Some("darkelf_town"), &mut first),
            Some(loc(1, 1, 1))
        );
        // No point / unknown point: the default region.
        assert_eq!(
            regions.restart_region_spawn_loc("HUMAN", None, &mut first),
            Some(loc(1, 2, 3))
        );
        assert_eq!(
            regions.restart_region_spawn_loc("HUMAN", Some("nope"), &mut first),
            Some(loc(1, 2, 3))
        );
        // A banned-race cycle ends in the default region instead of overflowing the stack.
        assert_eq!(
            regions.restart_region_spawn_loc("ORC", Some("loop_a"), &mut first),
            Some(loc(1, 2, 3))
        );
        assert_eq!(regions.restart_region_loc_id("DARK_ELF", Some("elf_town")), 912);
        assert_eq!(regions.restart_region_loc_id("ELF", Some("elf_town")), 911);
        assert_eq!(regions.restart_region_loc_id("ELF", Some("nope")), 910);
        assert_eq!(regions.restart_region_loc_id("ELF", None), 910);
    }

    #[test]
    fn a_respawn_zone_overrides_the_region() {
        let zones_xml = r#"<list><zone id="1" type="RespawnZone" shape="Cuboid" minZ="-10" maxZ="10">
            <node X="-150100" Y="169900"/><node X="-149900" Y="170100"/>
            <race name="HUMAN" point="elf_town"/></zone></list>"#;
        let (regions, zones) = load_text(Some(zones_xml), REGIONS);
        let mut first = first_spawn;
        assert_eq!(
            regions.nearest_town_respawn(
                &zones,
                Location {
                    x: -150_000,
                    y: 170_000,
                    z: 0
                },
                Some("HUMAN"),
                &mut first
            ),
            Some(loc(10, 20, 30))
        );
        // A race the zone has no entry for goes to the default region.
        assert_eq!(
            regions.nearest_town_respawn(
                &zones,
                Location {
                    x: -150_000,
                    y: 170_000,
                    z: 0
                },
                Some("ORC"),
                &mut first
            ),
            Some(loc(1, 2, 3))
        );
        // Outside the zone's Z band the region applies again.
        assert_eq!(
            regions.nearest_town_respawn(
                &zones,
                Location {
                    x: -150_000,
                    y: 170_000,
                    z: 50
                },
                Some("HUMAN"),
                &mut first
            ),
            Some(loc(1, 2, 3))
        );
    }

    #[test]
    fn java_string_hash_matches_java() {
        assert_eq!(java_string_hash(""), 0);
        assert_eq!(java_string_hash("a"), 97);
        assert_eq!(java_string_hash("hello"), 99_162_322);
    }

    #[test]
    fn hashmap_order_is_by_bucket_then_insertion() {
        // "a" = 97 -> bucket 1, "q" = 113 -> bucket 1 (113 & 15), "b" = 98 -> bucket 2.
        let order = java_hashmap_order(&["q", "b", "a"]);
        assert_eq!(order, vec![0, 2, 1]);
    }
}
