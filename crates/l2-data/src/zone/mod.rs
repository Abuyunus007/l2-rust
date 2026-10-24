//! Zones: the shapes and types from `data/zones/*.xml`, and the world-region grid that finds the
//! zones at a point.
//!
//! Java: `managers/ZoneManager.java`, `entity/zone/{ZoneType,ZoneRespawn,ZoneRegion}.java`,
//! `entity/zone/type/{RespawnZone,NpcSpawnTerritory}.java`. The shapes are in [`shape`].
//! Data: `data/zones/*.xml` (documented in `data/zones/documentation.txt`), `xsd/zones.xsd`.
//!
//! # What is loaded
//! Every `<zone>` of every enabled file, with its type name (`PeaceZone`, `TownZone`, ...), its
//! shape, its `<stat name=".." val=".."/>` parameters **as raw text** (Java parses them in the
//! type classes, which are M4+ work), its `<spawn>` points (respawn-type zones only) and its
//! `<race>` restart points (`RespawnZone` only). The M3 world only needs the type name plus
//! shape queries; effects, scripts and per-type parameter parsing come later.
//!
//! # Java's rules, kept
//! * A file whose root has `enabled="false"` is skipped.
//! * A zone without `id` gets 300001, 300002, ... in load order (files in the order NTFS lists
//!   them, upper-cased name; the counter also advances for zones that are skipped afterwards).
//!   `NpcSpawnTerritory` gets 0.
//! * A zone whose `type` is not a Java class in `entity/zone/type` (`TaxZone`, `SSQZone`, ...) is
//!   skipped with a warning, so those zones do not exist in the table, same as in Java.
//! * Zones are stored per type name; a later zone with the same id and type replaces the earlier
//!   one. The per-cell grid replaces by id **across types** and only in the cells the new zone
//!   touches, so an old zone can survive in other cells (Java's `ZoneRegion` maps behave so).
//! * The grid has 17 x 18 cells of 32768 units. A zone is registered in every cell its shape
//!   `intersectsRectangle`s.
//!
//! NOTE(java-quirk): Java iterates the zones of a cell (and of a type) in `ConcurrentHashMap`
//! order, which decides which of two overlapping zones of one type `zone_at` finds first. That
//! order is reproduced by `IntMap` (bins, insertion order, resize reordering); tree bins (8+ ids
//! in one bin of a 64+ bin table) are not modelled.

pub mod shape;

use std::path::{Path, PathBuf};

use serde::Deserialize;
use tracing::{debug, info, warn};

use crate::loader::{DataError, load_file, log_loaded};
use crate::player_template::Location;
pub use shape::ZoneForm;

/// `ZoneManager.SHIFT_BY`: grid cells are 2^15 = 32768 units wide.
pub const SHIFT_BY: i32 = 15;
/// `ZoneManager.OFFSET_X` = `abs(World.WORLD_X_MIN >> 15)`.
pub const OFFSET_X: i32 = 9;
/// `ZoneManager.OFFSET_Y` = `abs(World.WORLD_Y_MIN >> 15)`.
pub const OFFSET_Y: i32 = 8;
/// Cells along X: `(WORLD_X_MAX >> 15) + OFFSET_X + 1`.
pub const GRID_WIDTH: usize = 17;
/// Cells along Y: `(WORLD_Y_MAX >> 15) + OFFSET_Y + 1`.
pub const GRID_HEIGHT: usize = 18;
/// First id given to zones without an `id` attribute is this plus one.
const FIRST_DYNAMIC_ID: i32 = 300_000;

/// The zone classes `ZoneManager.load()` pre-registers (the count in its log line starts here).
const REGISTERED_TYPES: [&str; 31] = [
    "ArenaZone",
    "BossZone",
    "CastleZone",
    "ClanHallZone",
    "ConditionZone",
    "DamageZone",
    "DerbyTrackZone",
    "EffectZone",
    "FishingZone",
    "FlagPvPZone",
    "FortZone",
    "HqZone",
    "JailZone",
    "LandingZone",
    "MotherTreeZone",
    "NoLandingZone",
    "NoRestartZone",
    "NoStoreZone",
    "NoSummonFriendZone",
    "OlympiadStadiumZone",
    "PeaceZone",
    "ResidenceHallTeleportZone",
    "ResidenceTeleportZone",
    "ResidenceZone",
    "RespawnZone",
    "ScriptZone",
    "SiegableHallZone",
    "SiegeZone",
    "SwampZone",
    "TownZone",
    "WaterZone",
];

/// Zone types Java can instantiate (`Class.forName("...zone.type." + name)` with a public
/// `(int)` constructor): `REGISTERED_TYPES` minus the abstract `ResidenceZone`, plus `NoPvPZone`.
fn is_instantiable_type(name: &str) -> bool {
    name == "NoPvPZone" || (name != "ResidenceZone" && REGISTERED_TYPES.contains(&name))
}

/// Zone types that extend `ZoneRespawn` and so accept `<spawn>` elements.
fn is_respawn_type(name: &str) -> bool {
    matches!(
        name,
        "CastleZone"
            | "ClanHallZone"
            | "FortZone"
            | "SiegableHallZone"
            | "OlympiadStadiumZone"
            | "ResidenceTeleportZone"
            | "ResidenceHallTeleportZone"
    )
}

/// Java `type.isInstance(zone)`: `actual` is the zone's own class, `wanted` the class asked for.
/// Only the real inheritance chains between zone classes are listed.
pub fn zone_type_is_a(actual: &str, wanted: &str) -> bool {
    if actual == wanted {
        return true;
    }
    let parents: &[&str] = match actual {
        "CastleZone" | "FortZone" => &["ResidenceZone", "ZoneRespawn"],
        "ClanHallZone" => &["ResidenceZone", "ZoneRespawn"],
        "SiegableHallZone" => &["ClanHallZone", "ResidenceZone", "ZoneRespawn"],
        "OlympiadStadiumZone" | "ResidenceTeleportZone" => &["ZoneRespawn"],
        "ResidenceHallTeleportZone" => &["ResidenceTeleportZone", "ZoneRespawn"],
        _ => &[],
    };
    parents.contains(&wanted)
}

/// `Race` names Java's `Race.valueOf` accepts (`entity/actor/enums/creature/Race.java`).
pub(crate) const JAVA_RACES: [&str; 24] = [
    "HUMAN",
    "ELF",
    "DARK_ELF",
    "ORC",
    "DWARF",
    "KAMAEL",
    "ANIMAL",
    "BEAST",
    "BUG",
    "CASTLE_GUARD",
    "CONSTRUCT",
    "DEMONIC",
    "DIVINE",
    "DRAGON",
    "ELEMENTAL",
    "ETC",
    "FAIRY",
    "GIANT",
    "HUMANOID",
    "MERCENARY",
    "NONE",
    "PLANT",
    "SIEGE_WEAPON",
    "UNDEAD",
];

/// The `<spawn>` points of a respawn-type zone (Java `ZoneRespawn` and two subclasses).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ZoneSpawns {
    /// Spawns without a `type` (or with an empty one).
    pub spawns: Vec<Location>,
    /// `type="other"`.
    pub other: Vec<Location>,
    /// `type="chaotic"`.
    pub chaotic: Vec<Location>,
    /// `type="banish"`.
    pub banish: Vec<Location>,
    /// `type="spectatorSpawn"` (`OlympiadStadiumZone` only).
    pub spectator: Vec<Location>,
    /// `type="challenger"` (`SiegableHallZone` only).
    pub challenger: Vec<Location>,
}

/// One zone. Java `ZoneType` and its subclasses, minus runtime state.
#[derive(Debug, Clone, PartialEq)]
pub struct Zone {
    /// The `id` attribute, or the generated 300001+ id.
    pub id: i32,
    /// The Java class name from the `type` attribute, e.g. `TownZone`.
    pub zone_type: String,
    /// The `name` attribute, else the `name` stat; `None` when neither is there.
    pub name: Option<String>,
    /// The shape.
    pub form: ZoneForm,
    /// The `<stat name val>` pairs in file order, unparsed. See [`Zone::param`].
    pub params: Vec<(String, String)>,
    /// `<spawn>` points; empty for zone types that do not take any.
    pub spawns: ZoneSpawns,
    /// `<race name point>` pairs of a `RespawnZone`: Java race name to restart-point (map region)
    /// name. A later entry for the same race replaces the earlier one.
    pub race_points: Vec<(String, String)>,
}

impl Zone {
    /// Java `isInsideZone(x, y, z)`.
    pub fn is_inside_zone(&self, x: i32, y: i32, z: i32) -> bool {
        self.form.is_inside_zone(x, y, z)
    }

    /// Java `isInsideZone(x, y)`: tests at the zone's highest Z.
    pub fn is_inside_zone_xy(&self, x: i32, y: i32) -> bool {
        self.form.is_inside_zone(x, y, self.form.high_z())
    }

    /// Java `getDistanceToZone(x, y)`.
    pub fn distance_to_zone(&self, x: i32, y: i32) -> f64 {
        self.form.distance_to_zone(x, y)
    }

    /// The value of a `<stat>`; the last one wins if a name repeats (Java overwrites).
    pub fn param(&self, name: &str) -> Option<&str> {
        self.params
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    /// [`param`](Self::param) read as an integer (`Integer.parseInt`).
    pub fn int_param(&self, name: &str) -> Option<i32> {
        self.param(name).and_then(|v| v.parse().ok())
    }

    /// Java `type.isInstance(zone)` for a zone class name, see [`zone_type_is_a`].
    pub fn is_a(&self, wanted: &str) -> bool {
        zone_type_is_a(&self.zone_type, wanted)
    }

    /// `RespawnZone.getRespawnPoint(player)`: the map region name for a Java race name.
    pub fn respawn_point(&self, race: &str) -> Option<&str> {
        self.race_points
            .iter()
            .find(|(r, _)| r == race)
            .map(|(_, p)| p.as_str())
    }

    /// `ZoneRespawn.getSpawnLoc()`. `random(len)` is `Rnd.get(len)` when
    /// `RandomRespawnInTownEnabled` is on; pass [`first_spawn`] when it is off. `None` where Java
    /// would throw (a zone with no spawn).
    pub fn spawn_loc(&self, random: &mut dyn FnMut(usize) -> usize) -> Option<Location> {
        pick(&self.spawns.spawns, random)
    }

    /// `ZoneRespawn.getOtherSpawnLoc()`: falls back to [`spawn_loc`](Self::spawn_loc).
    pub fn other_spawn_loc(&self, random: &mut dyn FnMut(usize) -> usize) -> Option<Location> {
        pick(&self.spawns.other, random).or_else(|| self.spawn_loc(random))
    }

    /// `ZoneRespawn.getChaoticSpawnLoc()`: falls back to [`spawn_loc`](Self::spawn_loc).
    pub fn chaotic_spawn_loc(&self, random: &mut dyn FnMut(usize) -> usize) -> Option<Location> {
        pick(&self.spawns.chaotic, random).or_else(|| self.spawn_loc(random))
    }

    /// `ZoneRespawn.getBanishSpawnLoc()`: falls back to [`spawn_loc`](Self::spawn_loc).
    pub fn banish_spawn_loc(&self, random: &mut dyn FnMut(usize) -> usize) -> Option<Location> {
        pick(&self.spawns.banish, random).or_else(|| self.spawn_loc(random))
    }
}

/// The `random` argument for "random respawn is off": always the first entry.
pub fn first_spawn(_len: usize) -> usize {
    0
}

fn pick(list: &[Location], random: &mut dyn FnMut(usize) -> usize) -> Option<Location> {
    if list.is_empty() {
        None
    } else {
        // The index is clamped so a bad random source cannot panic.
        list.get(random(list.len())).or_else(|| list.first()).copied()
    }
}

/// Java `NpcSpawnTerritory`: a named shape with optional banned shapes cut out of it. Only the
/// geometry lives here (the random point needs the geoengine).
#[derive(Debug, Clone, PartialEq)]
pub struct SpawnTerritory {
    /// The `name` attribute.
    pub name: String,
    /// The territory shape.
    pub form: ZoneForm,
    /// Shapes added with `addBannedTerritory`, excluded by [`is_inside_zone_xy`](Self::is_inside_zone_xy).
    pub banned: Vec<ZoneForm>,
}

impl SpawnTerritory {
    /// Java `new NpcSpawnTerritory(name, territory)`.
    pub fn new(name: String, form: ZoneForm) -> Self {
        Self {
            name,
            form,
            banned: Vec::new(),
        }
    }

    /// Java `isInsideZone(x, y, z)`: the territory shape only, banned shapes are not checked.
    pub fn is_inside_zone(&self, x: i32, y: i32, z: i32) -> bool {
        self.form.is_inside_zone(x, y, z)
    }

    /// Java `isInsideZone(x, y)`: tests at the territory's lowest Z and excludes banned shapes.
    pub fn is_inside_zone_xy(&self, x: i32, y: i32) -> bool {
        let z = self.form.low_z();
        self.form.is_inside_zone(x, y, z) && !self.banned.iter().any(|b| b.is_inside_zone(x, y, z))
    }

    /// Java `getHighZ()`.
    pub fn high_z(&self) -> i32 {
        self.form.high_z()
    }
}

/// All zones, with Java's lookups. Immutable once loaded.
#[derive(Debug, Clone)]
pub struct Zones {
    /// Every zone ever created. Zones replaced in a class map stay here because cells may still
    /// point at them.
    zones: Vec<Zone>,
    /// Class name to id to index into `zones`. Java `_classZones`.
    classes: Vec<(String, IntMap)>,
    /// Per grid cell `x * GRID_HEIGHT + y`: id to index.
    cells: Vec<IntMap>,
    spawn_territories: Vec<SpawnTerritory>,
}

impl Zones {
    /// Loads `data/zones/*.xml` below `root` (the folder that contains `data/`), logging Java's
    /// count lines.
    pub fn load(root: &Path) -> Self {
        info!("ZoneManager {GRID_WIDTH} by {GRID_HEIGHT} Zone Region Grid set up.");
        let mut builder = Builder::new();
        let dir = root.join("data").join("zones");
        if dir.is_dir() {
            for path in xml_files_in_ntfs_order(&dir) {
                let file = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                load_file::<RawZoneFile, _>(&path, |doc| {
                    builder.add_file(doc, &file);
                    Ok(())
                });
            }
        } else {
            warn!("Directory not found: {}", dir.display());
        }
        let zones = builder.finish();
        log_loaded(
            "ZoneManager",
            zones.class_count(),
            &format!("zone classes and {} zones", zones.len()),
        );
        if let Some(last) = zones.last_static_id() {
            info!("ZoneManager: Last static id {last}.");
        }
        zones
    }

    /// Number of zones (Java `getSize()`): the sum over the class maps.
    pub fn len(&self) -> usize {
        self.classes.iter().map(|(_, m)| m.len()).sum()
    }

    /// `true` when no zone is loaded.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Number of zone classes (the first number of Java's log line): the 31 pre-registered
    /// ones, plus any other class a loaded zone has.
    pub fn class_count(&self) -> usize {
        self.classes.len()
    }

    /// The biggest id below 300000 (Java's "Last static id" line), `None` when there is none.
    pub fn last_static_id(&self) -> Option<i32> {
        self.iter()
            .map(|z| z.id)
            .filter(|&id| id < FIRST_DYNAMIC_ID)
            .max()
    }

    /// Every registered zone, class by class.
    pub fn iter(&self) -> impl Iterator<Item = &Zone> {
        self.classes
            .iter()
            .flat_map(|(_, map)| map.iter().map(|(_, i)| &self.zones[i]))
    }

    /// Every zone in the order Java created them (files in load order, zones in file order),
    /// including zones a later one with the same id and type replaced in its class map. Java's
    /// `setParameter` side effects (a `ClanHallZone` registering with its clan hall) happen in
    /// this order (M7-RES-01).
    pub fn created(&self) -> impl Iterator<Item = &Zone> {
        self.zones.iter()
    }

    /// Java `getAllZones(Class)`: the zones of exactly this class (no subclasses, like the Java
    /// map lookup). Empty for an unknown class.
    pub fn all_of_type(&self, zone_type: &str) -> Vec<&Zone> {
        self.classes
            .iter()
            .find(|(t, _)| t == zone_type)
            .map(|(_, map)| map.iter().map(|(_, i)| &self.zones[i]).collect())
            .unwrap_or_default()
    }

    /// Java `getZoneById(id)`: the first class map that has the id.
    pub fn get(&self, id: i32) -> Option<&Zone> {
        self.classes
            .iter()
            .find_map(|(_, map)| map.get(id))
            .map(|i| &self.zones[i])
    }

    /// Java `getZoneById(id, Class)`.
    pub fn get_of_type(&self, id: i32, zone_type: &str) -> Option<&Zone> {
        self.classes
            .iter()
            .find(|(t, _)| t == zone_type)
            .and_then(|(_, map)| map.get(id))
            .map(|i| &self.zones[i])
    }

    /// Java `getZoneByName(name)`.
    pub fn get_by_name(&self, name: &str) -> Option<&Zone> {
        self.iter().find(|z| z.name.as_deref() == Some(name))
    }

    /// Java `getZoneByName(name, Class)`.
    pub fn get_by_name_and_type(&self, name: &str, zone_type: &str) -> Option<&Zone> {
        self.all_of_type(zone_type)
            .into_iter()
            .find(|z| z.name.as_deref() == Some(name))
    }

    /// The zones registered in the grid cell of `(x, y)`, i.e. Java `getRegion(x, y).getZones()`.
    /// Empty outside the grid.
    pub fn region_zones(&self, x: i32, y: i32) -> Vec<&Zone> {
        match cell_index(x, y) {
            Some(cell) => self.cells[cell].iter().map(|(_, i)| &self.zones[i]).collect(),
            None => Vec::new(),
        }
    }

    /// Java `getZones(x, y, z)`: every zone that contains the point.
    pub fn zones_at(&self, x: i32, y: i32, z: i32) -> Vec<&Zone> {
        self.region_zones(x, y)
            .into_iter()
            .filter(|zone| zone.is_inside_zone(x, y, z))
            .collect()
    }

    /// Java `getZones(x, y)`: ignores Z (tests each zone at its own highest Z).
    pub fn zones_at_xy(&self, x: i32, y: i32) -> Vec<&Zone> {
        self.region_zones(x, y)
            .into_iter()
            .filter(|zone| zone.is_inside_zone_xy(x, y))
            .collect()
    }

    /// Java `getZone(x, y, z, Class)`: the first zone at the point that is an instance of the
    /// class (see [`zone_type_is_a`]).
    pub fn zone_at(&self, x: i32, y: i32, z: i32, zone_type: &str) -> Option<&Zone> {
        self.region_zones(x, y)
            .into_iter()
            .find(|zone| zone.is_inside_zone(x, y, z) && zone.is_a(zone_type))
    }

    /// `true` when the point is inside any zone of the class (the check behind flags like
    /// "in peace zone": Java sets the flag while the creature is inside).
    pub fn is_inside_zone_type(&self, x: i32, y: i32, z: i32, zone_type: &str) -> bool {
        self.zone_at(x, y, z, zone_type).is_some()
    }

    /// Java `spawnTerritoryExists(name)` + `getSpawnTerritory(name)`.
    pub fn spawn_territory(&self, name: &str) -> Option<&SpawnTerritory> {
        self.spawn_territories.iter().find(|t| t.name == name)
    }

    /// Number of `NpcSpawnTerritory` entries in `data/zones`.
    pub fn spawn_territory_count(&self) -> usize {
        self.spawn_territories.len()
    }

    /// Java `getSpawnTerritories(object)`: the territories that contain the point.
    pub fn spawn_territories_at(&self, x: i32, y: i32, z: i32) -> Vec<&SpawnTerritory> {
        self.spawn_territories
            .iter()
            .filter(|t| t.is_inside_zone(x, y, z))
            .collect()
    }
}

/// Java `getRegion(x, y)` index, `None` when `ArrayIndexOutOfBoundsException` would be thrown.
fn cell_index(x: i32, y: i32) -> Option<usize> {
    let cx = (x >> SHIFT_BY) + OFFSET_X;
    let cy = (y >> SHIFT_BY) + OFFSET_Y;
    if cx < 0 || cy < 0 || cx >= GRID_WIDTH as i32 || cy >= GRID_HEIGHT as i32 {
        return None;
    }
    Some(cx as usize * GRID_HEIGHT + cy as usize)
}

/// The `.xml` files of `dir` in the order Java's `File.listFiles()` returns them on NTFS (Windows):
/// by upper-cased name, so `clanhall_teleport.xml` comes before `clan_hall.xml` (`_` sorts after
/// letters). The order decides the generated ids of zones without an `id`, so it is fixed here
/// instead of depending on the OS the way Java does (Linux lists in no particular order).
fn xml_files_in_ntfs_order(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = match std::fs::read_dir(dir) {
        Ok(entries) => entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file() && p.extension().is_some_and(|e| e.eq_ignore_ascii_case("xml")))
            .collect(),
        Err(error) => {
            warn!("Cannot list {}: {error}", dir.display());
            Vec::new()
        }
    };
    files.sort_by_cached_key(|p| {
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        (name.to_uppercase(), name)
    });
    files
}

/// A model of Java's `ConcurrentHashMap<Integer, _>` as far as iteration order goes: 16 bins to
/// start with, `put` appends to the end of the bin, the table doubles when the entry count
/// reaches 3/4 of its size, and a resize reverses the part of a bin before its last same-side run
/// (`transfer`). A bin with 8+ entries in a table under 64 bins makes Java pre-size the table
/// (`treeifyBin`), also modelled; real tree bins (8+ entries in a table of 64+) are not.
#[derive(Debug, Clone)]
struct IntMap {
    bins: Vec<Vec<(i32, usize)>>,
    count: usize,
    size_ctl: usize,
}

impl IntMap {
    fn new() -> Self {
        Self {
            bins: vec![Vec::new(); 16],
            count: 0,
            size_ctl: 12,
        }
    }

    fn spread(id: i32) -> usize {
        let h = id as u32;
        ((h ^ (h >> 16)) & 0x7fff_ffff) as usize
    }

    fn len(&self) -> usize {
        self.count
    }

    fn contains(&self, id: i32) -> bool {
        self.get(id).is_some()
    }

    fn get(&self, id: i32) -> Option<usize> {
        let bin = &self.bins[Self::spread(id) & (self.bins.len() - 1)];
        bin.iter().find(|e| e.0 == id).map(|e| e.1)
    }

    /// `put`: replaces the value of an existing id in place, else appends to the bin.
    fn put(&mut self, id: i32, value: usize) {
        let i = Self::spread(id) & (self.bins.len() - 1);
        if let Some(entry) = self.bins[i].iter_mut().find(|e| e.0 == id) {
            entry.1 = value;
            return;
        }
        let existing = self.bins[i].len();
        self.bins[i].push((id, value));
        self.count += 1;
        if existing >= 8 && self.bins.len() < 64 {
            // treeifyBin -> tryPresize(n << 1)
            let target = (self.bins.len() << 1) * 3 / 2 + 1;
            let target = target.next_power_of_two();
            while target > self.size_ctl {
                self.transfer();
            }
        }
        while self.count >= self.size_ctl {
            self.transfer();
        }
    }

    /// Doubles the table the way `ConcurrentHashMap.transfer` does.
    fn transfer(&mut self) {
        let n = self.bins.len();
        let mut next: Vec<Vec<(i32, usize)>> = vec![Vec::new(); n * 2];
        for (i, bin) in self.bins.iter().enumerate() {
            if bin.is_empty() {
                continue;
            }
            let side = |e: &(i32, usize)| Self::spread(e.0) & n;
            let mut run_bit = side(&bin[0]);
            let mut last_run = 0;
            for (k, e) in bin.iter().enumerate().skip(1) {
                let b = side(e);
                if b != run_bit {
                    run_bit = b;
                    last_run = k;
                }
            }
            let (mut ln, mut hn) = (Vec::new(), Vec::new());
            if run_bit == 0 {
                ln.extend_from_slice(&bin[last_run..]);
            } else {
                hn.extend_from_slice(&bin[last_run..]);
            }
            // The nodes before the last run are copied to the *front* one by one.
            for e in &bin[..last_run] {
                if side(e) == 0 {
                    ln.insert(0, *e);
                } else {
                    hn.insert(0, *e);
                }
            }
            next[i] = ln;
            next[i + n] = hn;
        }
        self.bins = next;
        self.size_ctl = (n * 2) - (n >> 1);
    }

    /// `(id, value)` in Java's iteration order.
    fn iter(&self) -> impl Iterator<Item = (i32, usize)> + '_ {
        self.bins.iter().flatten().copied()
    }
}

// Raw XML shape (xsd/zones.xsd). Attributes are text so that one broken zone is skipped alone.
#[derive(Deserialize)]
struct RawZoneFile {
    #[serde(rename = "@enabled")]
    enabled: Option<String>,
    #[serde(rename = "$value", default)]
    children: Vec<RawListChild>,
}

#[derive(Deserialize)]
enum RawListChild {
    #[serde(rename = "zone")]
    Zone(RawZone),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct RawZone {
    #[serde(rename = "@type")]
    zone_type: Option<String>,
    #[serde(rename = "@id")]
    id: Option<String>,
    #[serde(rename = "@name")]
    name: Option<String>,
    #[serde(rename = "@minZ")]
    min_z: Option<String>,
    #[serde(rename = "@maxZ")]
    max_z: Option<String>,
    #[serde(rename = "@shape")]
    shape: Option<String>,
    #[serde(rename = "@rad")]
    rad: Option<String>,
    // Document order matters for stats and spawns, hence one enum list.
    #[serde(rename = "$value", default)]
    children: Vec<RawZoneChild>,
}

#[derive(Deserialize)]
enum RawZoneChild {
    #[serde(rename = "node")]
    Node(RawNode),
    #[serde(rename = "stat")]
    Stat(RawStat),
    #[serde(rename = "spawn")]
    Spawn(RawSpawn),
    #[serde(rename = "race")]
    Race(RawRace),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct RawNode {
    #[serde(rename = "@X")]
    x: Option<String>,
    #[serde(rename = "@Y")]
    y: Option<String>,
}

#[derive(Deserialize)]
struct RawStat {
    #[serde(rename = "@name")]
    name: Option<String>,
    #[serde(rename = "@val")]
    val: Option<String>,
}

#[derive(Deserialize)]
struct RawSpawn {
    #[serde(rename = "@X")]
    x: Option<String>,
    #[serde(rename = "@Y")]
    y: Option<String>,
    #[serde(rename = "@Z")]
    z: Option<String>,
    #[serde(rename = "@type")]
    spawn_type: Option<String>,
}

#[derive(Deserialize)]
struct RawRace {
    #[serde(rename = "@name")]
    name: Option<String>,
    #[serde(rename = "@point")]
    point: Option<String>,
}

/// Java `Integer.parseInt` on an attribute; a missing attribute or a bad number is an error.
fn int_attr(value: &Option<String>, what: &str) -> Result<i32, DataError> {
    let text = value
        .as_deref()
        .ok_or_else(|| DataError::Invalid(format!("missing {what}")))?;
    text.parse()
        .map_err(|_| DataError::Invalid(format!("{what} is not an integer: {text}")))
}

struct Builder {
    zones: Vec<Zone>,
    classes: Vec<(String, IntMap)>,
    cells: Vec<IntMap>,
    spawn_territories: Vec<SpawnTerritory>,
    last_dynamic_id: i32,
}

impl Builder {
    fn new() -> Self {
        Self {
            zones: Vec::new(),
            classes: REGISTERED_TYPES
                .iter()
                .map(|name| ((*name).to_owned(), IntMap::new()))
                .collect(),
            cells: vec![IntMap::new(); GRID_WIDTH * GRID_HEIGHT],
            spawn_territories: Vec::new(),
            last_dynamic_id: FIRST_DYNAMIC_ID,
        }
    }

    fn add_file(&mut self, doc: RawZoneFile, file: &str) {
        // Java: `enabled` present and not "true" (Boolean.parseBoolean) skips the file.
        if let Some(enabled) = &doc.enabled
            && !enabled.eq_ignore_ascii_case("true")
        {
            return;
        }
        for child in doc.children {
            if let RawListChild::Zone(raw) = child
                && let Err(error) = self.add_zone(raw)
            {
                warn!("ZoneManager: {error} (file {file})");
            }
        }
    }

    fn add_zone(&mut self, raw: RawZone) -> Result<(), DataError> {
        let Some(zone_type) = raw.zone_type.clone() else {
            return Err(DataError::Invalid("ZoneData: Missing type for zone".to_owned()));
        };
        let is_territory = zone_type.eq_ignore_ascii_case("NpcSpawnTerritory");
        // The dynamic counter advances before the zone is validated, like Java's.
        let id = match &raw.id {
            Some(_) => int_attr(&raw.id, "zone id")?,
            None if is_territory => 0,
            None => {
                self.last_dynamic_id += 1;
                self.last_dynamic_id
            }
        };
        let attr_name = raw.name.clone();
        if is_territory {
            match &attr_name {
                None => {
                    return Err(DataError::Invalid(
                        "ZoneData: Missing name for NpcSpawnTerritory, skipping zone".to_owned(),
                    ));
                }
                Some(name) if self.spawn_territories.iter().any(|t| &t.name == name) => {
                    return Err(DataError::Invalid(format!(
                        "ZoneData: Name {name} already used for another zone, skipping zone"
                    )));
                }
                Some(_) => {}
            }
        }

        let form = build_form(id, &raw)?;

        if is_territory {
            if let Some(name) = attr_name {
                self.spawn_territories.push(SpawnTerritory::new(name, form));
            }
            return Ok(());
        }

        if !is_instantiable_type(&zone_type) {
            return Err(DataError::Invalid(format!(
                "ZoneData: No such zone type: {zone_type} (zone {id})"
            )));
        }

        let mut params = Vec::new();
        let mut spawns = ZoneSpawns::default();
        let mut race_points: Vec<(String, String)> = Vec::new();
        for child in &raw.children {
            match child {
                RawZoneChild::Stat(stat) => {
                    let (Some(name), Some(val)) = (&stat.name, &stat.val) else {
                        return Err(DataError::Invalid(format!(
                            "ZoneData: stat without name/val (zone {id})"
                        )));
                    };
                    params.push((name.clone(), val.clone()));
                }
                RawZoneChild::Spawn(spawn) if is_respawn_type(&zone_type) => {
                    let x = int_attr(&spawn.x, "spawn X")?;
                    let y = int_attr(&spawn.y, "spawn Y")?;
                    let z = int_attr(&spawn.z, "spawn Z")?;
                    add_spawn(
                        &zone_type,
                        &mut spawns,
                        Location { x, y, z },
                        spawn.spawn_type.as_deref(),
                    );
                }
                RawZoneChild::Race(race) if zone_type == "RespawnZone" => {
                    let (Some(name), Some(point)) = (&race.name, &race.point) else {
                        return Err(DataError::Invalid(format!(
                            "ZoneData: race without name/point (zone {id})"
                        )));
                    };
                    if !JAVA_RACES.contains(&name.as_str()) {
                        return Err(DataError::Invalid(format!(
                            "ZoneData: unknown race {name} (zone {id})"
                        )));
                    }
                    match race_points.iter_mut().find(|(r, _)| r == name) {
                        Some(entry) => entry.1 = point.clone(),
                        None => race_points.push((name.clone(), point.clone())),
                    }
                }
                _ => {}
            }
        }

        if self.classes.iter().any(|(_, m)| m.contains(id)) {
            debug!("ZoneManager: Caution: Zone ({id}) overrides previous definition.");
        }

        // `setParameter("name")` sets the name; a non-empty `name` attribute is applied after it.
        let stat_name = params
            .iter()
            .rev()
            .find(|(n, _)| n == "name")
            .map(|(_, v)| v.clone());
        let name = match attr_name {
            Some(name) if !name.is_empty() => Some(name),
            _ => stat_name,
        };

        let index = self.zones.len();
        let (min_x, max_x, min_y, max_y) = form.bounds();
        self.zones.push(Zone {
            id,
            zone_type: zone_type.clone(),
            name,
            form,
            params,
            spawns,
            race_points,
        });

        // Java `addZone`: the class map, created on first use for classes that were not
        // pre-registered (NoPvPZone).
        let class_index = match self.classes.iter().position(|(t, _)| *t == zone_type) {
            Some(i) => i,
            None => {
                self.classes.push((zone_type, IntMap::new()));
                self.classes.len() - 1
            }
        };
        self.classes[class_index].1.put(id, index);

        // Register in every cell the shape intersects. Only cells around the shape's bounding box
        // can intersect (every `intersectsRectangle` needs an overlap), the exact test decides.
        let first_x = ((min_x >> SHIFT_BY) + OFFSET_X - 1).max(0);
        let last_x = ((max_x >> SHIFT_BY) + OFFSET_X + 1).min(GRID_WIDTH as i32 - 1);
        let first_y = ((min_y >> SHIFT_BY) + OFFSET_Y - 1).max(0);
        let last_y = ((max_y >> SHIFT_BY) + OFFSET_Y + 1).min(GRID_HEIGHT as i32 - 1);
        for x in first_x..=last_x {
            for y in first_y..=last_y {
                let ax = (x - OFFSET_X) << SHIFT_BY;
                let bx = ((x + 1) - OFFSET_X) << SHIFT_BY;
                let ay = (y - OFFSET_Y) << SHIFT_BY;
                let by = ((y + 1) - OFFSET_Y) << SHIFT_BY;
                if self.zones[index].form.intersects_rectangle(ax, bx, ay, by) {
                    self.cells[x as usize * GRID_HEIGHT + y as usize].put(id, index);
                }
            }
        }
        Ok(())
    }

    fn finish(self) -> Zones {
        Zones {
            zones: self.zones,
            classes: self.classes,
            cells: self.cells,
            spawn_territories: self.spawn_territories,
        }
    }
}

/// Java `ZoneRespawn.parseLoc`, with the two overrides (`OlympiadStadiumZone`, `SiegableHallZone`).
fn add_spawn(zone_type: &str, spawns: &mut ZoneSpawns, loc: Location, spawn_type: Option<&str>) {
    match spawn_type {
        None | Some("") => spawns.spawns.push(loc),
        Some("other") => spawns.other.push(loc),
        Some("chaotic") => spawns.chaotic.push(loc),
        Some("banish") => spawns.banish.push(loc),
        Some("spectatorSpawn") if zone_type == "OlympiadStadiumZone" => spawns.spectator.push(loc),
        Some("challenger") if zone_type == "SiegableHallZone" => spawns.challenger.push(loc),
        Some(other) => warn!("{zone_type}: Unknown location type: {other}"),
    }
}

/// The shape part of `ZoneManager.parseDocument`.
fn build_form(id: i32, raw: &RawZone) -> Result<ZoneForm, DataError> {
    let min_z = int_attr(&raw.min_z, "minZ")?;
    let max_z = int_attr(&raw.max_z, "maxZ")?;
    let mut coords: Vec<(i32, i32)> = Vec::new();
    for child in &raw.children {
        if let RawZoneChild::Node(node) = child {
            coords.push((int_attr(&node.x, "node X")?, int_attr(&node.y, "node Y")?));
        }
    }
    if coords.is_empty() {
        return Err(DataError::Invalid(format!(
            "ZoneData: missing data for zone: {id}"
        )));
    }
    let shape = raw
        .shape
        .as_deref()
        .ok_or_else(|| DataError::Invalid(format!("ZoneData: missing shape for zone: {id}")))?;
    if shape.eq_ignore_ascii_case("Cuboid") {
        // A cuboid needs exactly 2 points.
        if coords.len() == 2 {
            Ok(ZoneForm::cuboid(
                coords[0].0,
                coords[1].0,
                coords[0].1,
                coords[1].1,
                min_z,
                max_z,
            ))
        } else {
            Err(DataError::Invalid(format!(
                "ZoneData: Missing cuboid vertex data for zone: {id}"
            )))
        }
    } else if shape.eq_ignore_ascii_case("NPoly") {
        // A polygon needs at least 3 vertices.
        if coords.len() > 2 {
            let (xs, ys): (Vec<i32>, Vec<i32>) = coords.into_iter().unzip();
            Ok(ZoneForm::npoly(xs, ys, min_z, max_z))
        } else {
            Err(DataError::Invalid(format!("ZoneData: Bad data for zone: {id}")))
        }
    } else if shape.eq_ignore_ascii_case("Cylinder") {
        // A cylinder needs a centre point and a radius above 0.
        let rad = int_attr(&raw.rad, "rad")?;
        if coords.len() == 1 && rad > 0 {
            Ok(ZoneForm::cylinder(coords[0].0, coords[0].1, min_z, max_z, rad))
        } else {
            Err(DataError::Invalid(format!("ZoneData: Bad data for zone: {id}")))
        }
    } else {
        Err(DataError::Invalid(format!(
            "ZoneData: Unknown shape: \"{shape}\" for zone: {id}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_text(files: &[(&str, &str)]) -> Zones {
        let dir = tempfile::tempdir().unwrap();
        let zones_dir = dir.path().join("data").join("zones");
        std::fs::create_dir_all(&zones_dir).unwrap();
        for (name, text) in files {
            std::fs::write(zones_dir.join(name), text).unwrap();
        }
        Zones::load(dir.path())
    }

    const TOWN: &str = r#"<list enabled="true">
        <zone id="5" name="town" type="TownZone" shape="Cuboid" minZ="-100" maxZ="100">
            <stat name="townId" val="9" />
            <node X="0" Y="0" /><node X="1000" Y="2000" />
        </zone>
        <zone type="PeaceZone" shape="NPoly" minZ="0" maxZ="50">
            <node X="0" Y="0" /><node X="500" Y="0" /><node X="0" Y="500" />
        </zone>
        <zone id="7" type="WaterZone" shape="Cylinder" rad="100" minZ="0" maxZ="50">
            <node X="-5000" Y="-5000" />
        </zone>
    </list>"#;

    #[test]
    fn loads_the_three_shapes_and_finds_them() {
        let zones = load_text(&[("a.xml", TOWN)]);
        assert_eq!(zones.len(), 3);
        assert_eq!(zones.class_count(), 31);
        let town = zones.get(5).unwrap();
        assert_eq!(town.zone_type, "TownZone");
        assert_eq!(town.name.as_deref(), Some("town"));
        assert_eq!(town.int_param("townId"), Some(9));
        assert!(zones.zone_at(10, 10, 0, "TownZone").is_some());
        assert!(
            zones.zone_at(10, 10, 101, "TownZone").is_none(),
            "above the z band"
        );
        assert!(
            zones.zone_at(1000, 10, 0, "TownZone").is_none(),
            "upper edge is outside"
        );
        // The generated id is 300001; the polygon is a triangle.
        let peace = zones.get(300_001).unwrap();
        assert_eq!(peace.zone_type, "PeaceZone");
        assert!(peace.is_inside_zone(100, 100, 10));
        assert!(!peace.is_inside_zone(400, 400, 10));
        assert!(zones.zone_at(-5000, -5000, 10, "WaterZone").is_some());
        // Both the town cuboid and the peace triangle contain (10, 10, 10).
        let names: Vec<_> = zones.zones_at(10, 10, 10).iter().map(|z| z.id).collect();
        assert_eq!(names.len(), 2);
        assert!(zones.last_static_id() == Some(7));
        assert_eq!(zones.get_by_name("town").unwrap().id, 5);
    }

    #[test]
    fn points_outside_the_grid_have_no_zones() {
        let zones = load_text(&[("a.xml", TOWN)]);
        assert!(zones.zones_at(i32::MAX, 0, 0).is_empty());
        assert!(zones.zones_at(0, i32::MIN, 0).is_empty());
    }

    #[test]
    fn disabled_files_bad_zones_and_unknown_types_are_skipped() {
        let zones = load_text(&[
            (
                "a.xml",
                r#"<list enabled="false"><zone id="1" type="PeaceZone" shape="Cuboid" minZ="0" maxZ="1">
                    <node X="0" Y="0"/><node X="5" Y="5"/></zone></list>"#,
            ),
            (
                "b.xml",
                r#"<list>
                <zone id="2" type="PeaceZone" shape="Cuboid" minZ="0" maxZ="1"><node X="0" Y="0"/></zone>
                <zone id="3" type="PeaceZone" shape="NPoly" minZ="0" maxZ="1"><node X="0" Y="0"/><node X="5" Y="5"/></zone>
                <zone id="4" type="PeaceZone" shape="Cylinder" rad="0" minZ="0" maxZ="1"><node X="0" Y="0"/></zone>
                <zone id="5" type="PeaceZone" shape="Circle" minZ="0" maxZ="1"><node X="0" Y="0"/></zone>
                <zone id="6" type="PeaceZone" shape="Cuboid" minZ="0" maxZ="1"/>
                <zone id="7" type="TaxZone" shape="Cuboid" minZ="0" maxZ="1"><node X="0" Y="0"/><node X="5" Y="5"/></zone>
                <zone id="8" type="ResidenceZone" shape="Cuboid" minZ="0" maxZ="1"><node X="0" Y="0"/><node X="5" Y="5"/></zone>
                <zone id="x" type="PeaceZone" shape="Cuboid" minZ="0" maxZ="1"><node X="0" Y="0"/><node X="5" Y="5"/></zone>
                <zone shape="Cuboid" minZ="0" maxZ="1"><node X="0" Y="0"/><node X="5" Y="5"/></zone>
                <zone id="9" type="PeaceZone" shape="Cuboid" minZ="0" maxZ="1"><node X="0" Y="0"/><node X="5" Y="5"/></zone>
                </list>"#,
            ),
        ]);
        assert_eq!(zones.len(), 1);
        assert!(zones.get(9).is_some());
    }

    #[test]
    fn dynamic_ids_advance_for_skipped_zones_too() {
        let zones = load_text(&[(
            "a.xml",
            r#"<list>
            <zone type="TaxZone" shape="Cuboid" minZ="0" maxZ="1"><node X="0" Y="0"/><node X="5" Y="5"/></zone>
            <zone type="PeaceZone" shape="Cuboid" minZ="0" maxZ="1"><node X="0" Y="0"/><node X="5" Y="5"/></zone>
            </list>"#,
        )]);
        assert!(zones.get(300_001).is_none());
        assert!(zones.get(300_002).is_some());
    }

    #[test]
    fn same_id_replaces_in_its_class_but_not_across_classes() {
        let zones = load_text(&[(
            "a.xml",
            r#"<list>
            <zone id="1" name="first" type="PeaceZone" shape="Cuboid" minZ="0" maxZ="1"><node X="0" Y="0"/><node X="5" Y="5"/></zone>
            <zone id="1" name="second" type="PeaceZone" shape="Cuboid" minZ="0" maxZ="1"><node X="0" Y="0"/><node X="9" Y="9"/></zone>
            <zone id="1" name="other" type="TownZone" shape="Cuboid" minZ="0" maxZ="1"><node X="0" Y="0"/><node X="5" Y="5"/></zone>
            </list>"#,
        )]);
        // Two class maps hold id 1; the grid cell keeps only the last one.
        assert_eq!(zones.len(), 2);
        assert_eq!(
            zones.get_of_type(1, "PeaceZone").unwrap().name.as_deref(),
            Some("second")
        );
        assert_eq!(zones.region_zones(1, 1).len(), 1);
        assert_eq!(zones.region_zones(1, 1)[0].name.as_deref(), Some("other"));
    }

    #[test]
    fn name_comes_from_the_attribute_or_the_name_stat() {
        let zones = load_text(&[(
            "a.xml",
            r#"<list>
            <zone id="1" type="PeaceZone" shape="Cuboid" minZ="0" maxZ="1"><stat name="name" val="from_stat"/><node X="0" Y="0"/><node X="5" Y="5"/></zone>
            <zone id="2" name="attr" type="PeaceZone" shape="Cuboid" minZ="0" maxZ="1"><stat name="name" val="from_stat"/><node X="0" Y="0"/><node X="5" Y="5"/></zone>
            <zone id="3" name="" type="PeaceZone" shape="Cuboid" minZ="0" maxZ="1"><stat name="name" val="from_stat"/><node X="0" Y="0"/><node X="5" Y="5"/></zone>
            </list>"#,
        )]);
        assert_eq!(zones.get(1).unwrap().name.as_deref(), Some("from_stat"));
        assert_eq!(zones.get(2).unwrap().name.as_deref(), Some("attr"));
        assert_eq!(zones.get(3).unwrap().name.as_deref(), Some("from_stat"));
    }

    #[test]
    fn respawn_zone_races_and_spawn_points() {
        let zones = load_text(&[(
            "a.xml",
            r#"<list>
            <zone id="1" type="RespawnZone" shape="Cuboid" minZ="0" maxZ="1">
                <node X="0" Y="0"/><node X="5" Y="5"/>
                <race name="HUMAN" point="a"/><race name="HUMAN" point="b"/><race name="ELF" point="c"/>
            </zone>
            <zone id="2" type="RespawnZone" shape="Cuboid" minZ="0" maxZ="1">
                <node X="0" Y="0"/><node X="5" Y="5"/><race name="GOBLIN" point="a"/>
            </zone>
            <zone id="3" type="CastleZone" shape="Cuboid" minZ="0" maxZ="1">
                <node X="0" Y="0"/><node X="5" Y="5"/>
                <spawn X="1" Y="2" Z="3"/><spawn X="4" Y="5" Z="6" type="chaotic"/><spawn X="7" Y="8" Z="9" type="bogus"/>
            </zone>
            <zone id="4" type="PeaceZone" shape="Cuboid" minZ="0" maxZ="1">
                <node X="0" Y="0"/><node X="5" Y="5"/><spawn X="1" Y="2" Z="3"/>
            </zone>
            </list>"#,
        )]);
        let respawn = zones.get(1).unwrap();
        assert_eq!(
            respawn.respawn_point("HUMAN"),
            Some("b"),
            "a later race entry replaces"
        );
        assert_eq!(respawn.respawn_point("ELF"), Some("c"));
        assert_eq!(respawn.respawn_point("ORC"), None);
        assert!(zones.get(2).is_none(), "Race.valueOf fails for GOBLIN");
        let castle = zones.get(3).unwrap();
        assert_eq!(castle.spawns.spawns.len(), 1);
        assert_eq!(castle.spawns.chaotic.len(), 1);
        assert_eq!(
            castle.chaotic_spawn_loc(&mut first_spawn),
            Some(Location { x: 4, y: 5, z: 6 })
        );
        assert_eq!(
            castle.banish_spawn_loc(&mut first_spawn),
            Some(Location { x: 1, y: 2, z: 3 }),
            "no banish spawn: falls back to the normal one"
        );
        assert!(
            zones.get(4).unwrap().spawns.spawns.is_empty(),
            "PeaceZone takes no spawns"
        );
    }

    #[test]
    fn inheritance_chains_for_zone_at() {
        let zones = load_text(&[(
            "a.xml",
            r#"<list><zone id="1" type="SiegableHallZone" shape="Cuboid" minZ="0" maxZ="1">
                <node X="0" Y="0"/><node X="5" Y="5"/></zone></list>"#,
        )]);
        assert!(zones.zone_at(1, 1, 0, "SiegableHallZone").is_some());
        assert!(zones.zone_at(1, 1, 0, "ClanHallZone").is_some());
        assert!(zones.zone_at(1, 1, 0, "ResidenceZone").is_some());
        assert!(zones.zone_at(1, 1, 0, "PeaceZone").is_none());
        assert!(
            zones.all_of_type("ClanHallZone").is_empty(),
            "getAllZones is exact"
        );
    }

    #[test]
    fn npc_spawn_territories_are_kept_apart() {
        let zones = load_text(&[(
            "a.xml",
            r#"<list>
            <zone name="t1" type="NpcSpawnTerritory" shape="Cuboid" minZ="0" maxZ="10"><node X="0" Y="0"/><node X="9" Y="9"/></zone>
            <zone name="t1" type="NpcSpawnTerritory" shape="Cuboid" minZ="0" maxZ="10"><node X="0" Y="0"/><node X="99" Y="99"/></zone>
            <zone type="NpcSpawnTerritory" shape="Cuboid" minZ="0" maxZ="10"><node X="0" Y="0"/><node X="9" Y="9"/></zone>
            </list>"#,
        )]);
        assert!(zones.is_empty());
        assert_eq!(zones.spawn_territory_count(), 1);
        let t = zones.spawn_territory("t1").unwrap();
        assert!(t.is_inside_zone_xy(5, 5));
        assert!(!t.is_inside_zone_xy(50, 50), "the duplicate name was skipped");
        assert_eq!(zones.spawn_territories_at(5, 5, 5).len(), 1);
    }

    #[test]
    fn grid_registration_follows_intersects_rectangle() {
        // A big cuboid over four cells around the origin: x in [-100, 40000), y in [-100, 40000).
        let zones = load_text(&[(
            "a.xml",
            r#"<list><zone id="1" type="PeaceZone" shape="Cuboid" minZ="0" maxZ="1">
                <node X="-100" Y="-100"/><node X="40000" Y="40000"/></zone></list>"#,
        )]);
        for (x, y) in [(-1, -1), (0, 0), (32768, 0), (0, 32768), (32768, 32768)] {
            assert_eq!(zones.region_zones(x, y).len(), 1, "cell of {x},{y}");
        }
        assert!(zones.region_zones(-32769, 0).is_empty());
        assert!(zones.region_zones(70000, 0).is_empty());
    }

    #[test]
    fn chm_iteration_order() {
        let ids = |m: &IntMap| m.iter().map(|e| e.0).collect::<Vec<_>>();
        // Ids below the table size iterate in id order, whatever the insertion order.
        let mut m = IntMap::new();
        for id in [9, 2, 15, 1] {
            m.put(id, 0);
        }
        assert_eq!(ids(&m), vec![1, 2, 9, 15]);
        // 17 and 1 share bin 1 of a 16-bin table: insertion order stays.
        let mut m = IntMap::new();
        m.put(17, 0);
        m.put(1, 0);
        assert_eq!(ids(&m), vec![17, 1]);
        // The 12th entry makes the table grow to 32 bins.
        let mut m = IntMap::new();
        for i in 0..12 {
            m.put(31 - i, 0);
        }
        assert_eq!(ids(&m).first(), Some(&20));
        assert_eq!(m.len(), 12);
        // Putting an existing id replaces the value and keeps the position.
        m.put(25, 7);
        assert_eq!(m.get(25), Some(7));
        assert_eq!(m.len(), 12);
    }

    #[test]
    fn chm_resize_reverses_the_part_before_the_last_run() {
        // 1, 33 and 17 share bin 1 of 16 bins, in this order. When the table doubles, 1 and 33
        // stay in bin 1 and 17 goes to bin 17. Java keeps the trailing run (17) as it is and
        // copies the nodes before it to the front one by one, so 1 and 33 swap places.
        let mut m = IntMap::new();
        for id in [1, 33, 17] {
            m.put(id, 0);
        }
        for id in 100..109 {
            m.put(id, 0); // brings the count to 12 and triggers the resize
        }
        let ids: Vec<i32> = m.iter().map(|e| e.0).collect();
        let pos = |id| ids.iter().position(|&x| x == id).unwrap();
        assert_eq!(pos(1), pos(33) + 1, "{ids:?}");
        assert!(pos(33) < pos(17));
    }
}
