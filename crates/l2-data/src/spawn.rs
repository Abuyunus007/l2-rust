//! NPC spawns from `data/spawns`: fixed `<npc>` spawns, spawn groups and spawn territories.
//!
//! Java: `gameserver/data/xml/SpawnData.java` (the parser), `gameserver/data/SpawnTable.java`,
//! `gameserver/entity/spawns/{Spawn,SpawnGroup,SpawnGroupEntry,SpawnSelection}.java`,
//! `gameserver/entity/zone/type/NpcSpawnTerritory.java` and the zone forms it wraps
//! (`entity/zone/form/{ZoneNPoly,ZoneCuboid,ZoneCylinder}.java`).
//! Data: `data/spawns/**/*.xml` (`xsd/spawns.xsd`).
//!
//! # What is loaded
//! Only the description of the spawns. Nothing is put into the world here: M3-NPC-01 creates the
//! NPCs, rolls the random groups and picks territory points with the world RNG. Java does two
//! things at load time that are left to the world for that reason: the territory point of each
//! group slot (`setXYZ(getRandomPoint())`, which would consume RNG and needs geodata) and the
//! group rolls (`SpawnGroup.initialSpawn`).
//!
//! # How Java reads the files (and what is ported)
//! * Attributes are read with `Integer.decode` (`0x`, `#` and a leading `0` for octal), except
//!   `AIData` values and the cylinder `rad`, which use `Integer.parseInt`. A missing mandatory
//!   attribute is a `NullPointerException` in Java, a bad number a `NumberFormatException`; both
//!   abort the rest of the file, the entries parsed before it stay (see `loader::load_file`).
//! * `enabled` must be present and only `true` (any case) loads the list.
//! * Spawn territories are global by name and the first definition wins. A `<territory>` that
//!   Java rejects (too few points, unknown shape) is logged and not registered. One whose
//!   coordinates fail to parse is registered without a shape (Java's `catch` falls through), so
//!   its random point is unavailable.
//! * Child elements are walked in document order, because it matters: `AIData` applies only to the
//!   `<npc>` elements after it, and a banned area needs its territory defined before it.
//!
//! The Java count lines are logged at the end of [`SpawnData::load`]. Java prints the number of NPCs
//! it created at startup; this loader cannot know that (random groups roll later), so it prints
//! the fixed count and the number of random groups instead.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use l2_util::WorldRng;
use serde::Deserialize;
use tracing::{info, warn};

use crate::loader::{DataError, load_file};
use crate::npc::NpcTemplates;
use crate::player_template::Location;

const LOADER: &str = "SpawnData";

/// What the server config contributes to loading spawns. The defaults are Java's code defaults.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SpawnLoadOptions {
    /// `Custom/FakePlayers.ini` `EnableFakePlayers` (default false): fake player NPCs may spawn.
    pub fake_players_enabled: bool,
    /// `Development.ini` `NoSpawns` (default false): load nothing, log nothing.
    pub no_spawns: bool,
}

/// Java `SpawnSelection`: how a spawn group picks its entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnSelection {
    /// `all`, and the fallback for an unknown value: every entry spawns its whole quota.
    All,
    /// `random`: one entry is rolled and spawns its whole quota.
    Random,
    /// `randomFill`: `maximumNpc` rolls with replacement, one unit each.
    RandomFill,
}

/// Java `Spawn` `periodOfDay`: the spawn is only active at day or at night (`DayNightSpawnManager`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeriodOfDay {
    /// `day`.
    Day,
    /// `night`.
    Night,
}

/// Java `SpawnGroup`: a block with a `selection` attribute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnGroup {
    /// The spawn name, else the first territory, else the file name (Java's rule).
    pub name: String,
    /// How entries are chosen.
    pub selection: SpawnSelection,
    /// The `maximumNpc` cap (0 when absent or ignored).
    pub maximum_npc: i32,
    /// Entries in document order.
    pub entries: Vec<SpawnGroupEntry>,
}

/// Java `SpawnGroupEntry`: one `<npc>` of a group, with one slot per territory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnGroupEntry {
    /// The `count` of the `<npc>`: units this entry may spawn.
    pub total: i32,
    /// `respawnDelay` in seconds, as written.
    pub respawn_delay: i32,
    /// `respawnRandom` in seconds, as written.
    pub respawn_random: i32,
    /// Indices into [`SpawnData::spawns`], one per territory of the group.
    pub slots: Vec<usize>,
}

/// Java `Spawn`: one place where NPCs of one template are created.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NpcSpawn {
    /// NPC template id.
    pub npc_id: i32,
    /// Units this spawn creates: 1 for a fixed spawn (each `count` is a spawn of its own), the
    /// entry total for a group slot.
    pub amount: i32,
    /// Fixed location. For a spawn with a territory, Java replaces it by a random point of the
    /// territory when the NPC is created; for a group slot the point is picked by the world too.
    pub location: Location,
    /// Heading, `-1` for random (Java's default).
    pub heading: i32,
    /// Minimum respawn delay in milliseconds (`Spawn.setRespawnDelay`), 0 when none.
    pub respawn_min_ms: i32,
    /// Maximum respawn delay in milliseconds, 0 when none.
    pub respawn_max_ms: i32,
    /// Java's `_doRespawn` flag as the loader leaves it: a fixed spawn respawns when it has a delay,
    /// a group slot only in an `all` group with a delay. Day and night spawns keep Java's default.
    pub respawn_enabled: bool,
    /// Chase range (`chaseRange`, default 0).
    pub chase_range: i32,
    /// Spawn name, when non-empty.
    pub name: Option<String>,
    /// The territory the spawn is placed in, only when it exists (Java `getSpawnTerritory`
    /// returns null for an unknown name, and the spawn keeps its fixed location).
    pub territory: Option<String>,
    /// Day or night only, when `periodOfDay` is `day` or `night`.
    pub period_of_day: Option<PeriodOfDay>,
    /// Java `AIData` of the block, when one appeared before this `<npc>`.
    pub ai_data: Option<BTreeMap<String, i32>>,
    /// Index into [`SpawnData::groups`] for a group slot.
    pub group: Option<usize>,
    /// The datapack file, relative to the datapack root (Java `fileName`).
    pub source: String,
}

/// Shape of a spawn territory: Java `ZoneCuboid`, `ZoneNPoly` or `ZoneCylinder`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerritoryShape {
    /// Java `ZoneCuboid`: `Rectangle` `x`, `y`, `width`, `height` and the normalised Z band.
    Cuboid {
        /// Left edge.
        x: i32,
        /// Top edge.
        y: i32,
        /// Width.
        width: i32,
        /// Height.
        height: i32,
        /// Lowest Z.
        z1: i32,
        /// Highest Z.
        z2: i32,
    },
    /// Java `ZoneNPoly`: polygon vertices and the normalised Z band.
    NPoly {
        /// Vertex X coordinates.
        xs: Vec<i32>,
        /// Vertex Y coordinates.
        ys: Vec<i32>,
        /// Lowest Z.
        z1: i32,
        /// Highest Z.
        z2: i32,
    },
    /// Java `ZoneCylinder`. Its Z band is kept as written (Java does not normalise it).
    Cylinder {
        /// Centre X.
        x: i32,
        /// Centre Y.
        y: i32,
        /// Lower Z bound as written.
        z1: i32,
        /// Upper Z bound as written.
        z2: i32,
        /// Radius (above 0).
        rad: i32,
    },
}

/// Java `NpcSpawnTerritory`: a named area for spawns, with areas banned from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnTerritory {
    name: String,
    /// `None` when Java registered the territory with a null shape (broken coordinates).
    shape: Option<TerritoryShape>,
    /// Banned areas in document order. `None` is a banned area whose shape failed to load; Java
    /// keeps the null and throws when a random point is tested against it.
    banned: Vec<Option<TerritoryShape>>,
}

impl SpawnTerritory {
    /// The territory name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The shape, `None` when it failed to load.
    pub fn shape(&self) -> Option<&TerritoryShape> {
        self.shape.as_ref()
    }

    /// The banned areas, in document order.
    pub fn banned(&self) -> &[Option<TerritoryShape>] {
        &self.banned
    }

    /// Java `NpcSpawnTerritory.getRandomPoint`: a random point of the territory that is not in a
    /// banned area and, when geodata is there, not above the territory's high Z.
    ///
    /// `height` is `GeoEngine.getHeight(x, y, z)`; the returned Z is the height at the point, as
    /// Java's zone forms return it. The world passes its geodata here; a loader-only caller can
    /// pass `&|_, _, z| z`. Random numbers come from `rng` with Java's `Rnd.get` calls, see
    /// [`TerritoryShape::random_point`].
    ///
    /// Returns `None` where Java throws: no shape, or a banned area without a shape.
    pub fn random_point(
        &self,
        rng: &mut WorldRng,
        height: &dyn Fn(i32, i32, i32) -> i32,
    ) -> Option<Location> {
        let shape = self.shape.as_ref()?;
        if !self.banned.is_empty() {
            // Java: 100 points outside banned areas and below the high Z, then 100 outside banned
            // areas only, then the common loop below.
            for _ in 0..100 {
                let point = shape.random_point(rng, height);
                if self.is_banned(point)? {
                    continue;
                }
                if height(point.x, point.y, point.z) > shape.high_z() {
                    continue;
                }
                return Some(point);
            }
            for _ in 0..100 {
                let point = shape.random_point(rng, height);
                if self.is_banned(point)? {
                    continue;
                }
                return Some(point);
            }
        }
        for _ in 0..100 {
            let point = shape.random_point(rng, height);
            if height(point.x, point.y, point.z) > shape.high_z() {
                continue;
            }
            return Some(point);
        }
        Some(shape.random_point(rng, height))
    }

    /// `Some(true)` when `point` is in a banned area, `None` when a banned area has no shape.
    fn is_banned(&self, point: Location) -> Option<bool> {
        for banned in &self.banned {
            let shape = banned.as_ref()?;
            if shape.contains(point.x, point.y, point.z) {
                return Some(true);
            }
        }
        Some(false)
    }
}

impl TerritoryShape {
    /// Java `isInsideZone(x, y, z)`.
    pub fn contains(&self, x: i32, y: i32, z: i32) -> bool {
        match self {
            Self::Cuboid {
                x: left,
                y: top,
                width,
                height,
                z1,
                z2,
            } => in_half_open(x, *left, *width) && in_half_open(y, *top, *height) && z >= *z1 && z <= *z2,
            Self::NPoly { xs, ys, z1, z2 } => polygon_contains(xs, ys, x, y) && z >= *z1 && z <= *z2,
            Self::Cylinder {
                x: cx,
                y: cy,
                z1,
                z2,
                rad,
            } => {
                let dx = f64::from(*cx) - f64::from(x);
                let dy = f64::from(*cy) - f64::from(y);
                let rad_sq = f64::from(*rad) * f64::from(*rad);
                dx * dx + dy * dy <= rad_sq && z >= *z1 && z <= *z2
            }
        }
    }

    /// The highest Z of the shape (Java `getHighZ`).
    pub fn high_z(&self) -> i32 {
        match self {
            Self::Cuboid { z2, .. } | Self::NPoly { z2, .. } | Self::Cylinder { z2, .. } => *z2,
        }
    }

    /// The lowest Z of the shape (Java `getLowZ`).
    pub fn low_z(&self) -> i32 {
        match self {
            Self::Cuboid { z1, .. } | Self::NPoly { z1, .. } | Self::Cylinder { z1, .. } => *z1,
        }
    }

    /// Java `getRandomPoint()` of the zone form. The random numbers are Java's `Rnd.get(min, max)`
    /// calls (inclusive ranges, [`WorldRng::get`]) in this order:
    /// * cuboid: one `x` in `[x, x + width]`, then one `y` in `[y, y + height]`, no containment test;
    /// * npoly: `x` in the polygon's bounds, `y` in its bounds, then up to 1000 new pairs while the
    ///   point is outside the polygon (`ZoneNPoly`'s `antiBlocker`), then the last pair is used
    ///   even if it is still outside;
    /// * cylinder: `x` in `[cx - rad, cx + rad]`, `y` likewise, repeated while outside the circle.
    ///
    /// The Z is `height(x, y, (z1 + z2) / 2)`.
    pub fn random_point(&self, rng: &mut WorldRng, height: &dyn Fn(i32, i32, i32) -> i32) -> Location {
        let (x, y) = match self {
            Self::Cuboid {
                x,
                y,
                width,
                height: h,
                ..
            } => (
                rng.get(*x, x.wrapping_add(*width)),
                rng.get(*y, y.wrapping_add(*h)),
            ),
            Self::NPoly { xs, ys, .. } => {
                let (min_x, max_x) = min_max(xs);
                let (min_y, max_y) = min_max(ys);
                let mut x = rng.get(min_x, max_x);
                let mut y = rng.get(min_y, max_y);
                let mut anti_blocker = 0;
                while !polygon_contains(xs, ys, x, y) && anti_blocker < 1000 {
                    anti_blocker += 1;
                    x = rng.get(min_x, max_x);
                    y = rng.get(min_y, max_y);
                }
                (x, y)
            }
            Self::Cylinder {
                x: cx, y: cy, rad, ..
            } => {
                // NOTE(java-quirk): ZoneCylinder measures the first candidate from the origin (0, 0),
                // not from the centre. A centre within `rad` of the origin therefore returns (0, 0)
                // without any random draw. Kept as is.
                let rad_sq = f64::from(*rad) * f64::from(*rad);
                let (mut x, mut y) = (0, 0);
                let mut dx = f64::from(*cx);
                let mut dy = f64::from(*cy);
                let (x2, y2) = (cx.wrapping_sub(*rad), cy.wrapping_sub(*rad));
                let (x3, y3) = (cx.wrapping_add(*rad), cy.wrapping_add(*rad));
                while dx * dx + dy * dy > rad_sq {
                    x = rng.get(x2, x3);
                    y = rng.get(y2, y3);
                    dx = f64::from(*cx) - f64::from(x);
                    dy = f64::from(*cy) - f64::from(y);
                }
                (x, y)
            }
        };
        let z = height(x, y, (self.low_z().wrapping_add(self.high_z())) / 2);
        Location { x, y, z }
    }
}

/// Java `Rectangle.contains` on one axis: `start <= v < start + len`.
fn in_half_open(v: i32, start: i32, len: i32) -> bool {
    let (v, start, end) = (i64::from(v), i64::from(start), i64::from(start) + i64::from(len));
    len > 0 && v >= start && v < end
}

fn min_max(values: &[i32]) -> (i32, i32) {
    let min = values.iter().copied().min().unwrap_or(0);
    let max = values.iter().copied().max().unwrap_or(0);
    (min, max)
}

/// Java `java.awt.Polygon.contains(x, y)`: the even-odd crossing test of JDK 8 and later, which
/// includes the left and top edges and excludes the right and bottom ones. The point must first be
/// inside the bounding box (`Rectangle2D` containment, also half-open).
// NOTE(java-quirk): edges on the right and bottom of the bounding box are outside, so a random
// point at the maximum X or Y is always rejected here and resampled.
fn polygon_contains(xs: &[i32], ys: &[i32], x: i32, y: i32) -> bool {
    let n = xs.len().min(ys.len());
    if n <= 2 {
        return false;
    }
    let (min_x, max_x) = min_max(&xs[..n]);
    let (min_y, max_y) = min_max(&ys[..n]);
    if !in_half_open(x, min_x, max_x.wrapping_sub(min_x))
        || !in_half_open(y, min_y, max_y.wrapping_sub(min_y))
    {
        return false;
    }
    let (px, py) = (f64::from(x), f64::from(y));
    let mut hits = 0_u32;
    let mut lastx = i64::from(xs[n - 1]);
    let mut lasty = i64::from(ys[n - 1]);
    for i in 0..n {
        let curx = i64::from(xs[i]);
        let cury = i64::from(ys[i]);
        if polygon_edge_hit(px, py, lastx, lasty, curx, cury) {
            hits += 1;
        }
        lastx = curx;
        lasty = cury;
    }
    (hits & 1) == 1
}

/// One edge of the JDK crossing test. `last` is the previous vertex, `cur` the current one.
fn polygon_edge_hit(px: f64, py: f64, lastx: i64, lasty: i64, curx: i64, cury: i64) -> bool {
    if cury == lasty {
        return false;
    }
    let leftx = if curx < lastx {
        if px >= lastx as f64 {
            return false;
        }
        curx
    } else {
        if px >= curx as f64 {
            return false;
        }
        lastx
    };
    let (test1, test2) = if cury < lasty {
        if py < cury as f64 || py >= lasty as f64 {
            return false;
        }
        if px < leftx as f64 {
            return true;
        }
        (px - curx as f64, py - cury as f64)
    } else {
        if py < lasty as f64 || py >= cury as f64 {
            return false;
        }
        if px < leftx as f64 {
            return true;
        }
        (px - lastx as f64, py - lasty as f64)
    };
    test1 < test2 / (lasty - cury) as f64 * (lastx - curx) as f64
}

// ---------------------------------------------------------------------------------------------
// Java number and string rules
// ---------------------------------------------------------------------------------------------

/// Java `Integer.decode`: optional sign, then `0x`/`0X`/`#` for hex, a leading `0` for octal.
fn java_decode(text: &str) -> Result<i32, DataError> {
    let (negative, unsigned) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    let (radix, digits) = if let Some(rest) = unsigned
        .strip_prefix("0x")
        .or_else(|| unsigned.strip_prefix("0X"))
    {
        (16, rest)
    } else if let Some(rest) = unsigned.strip_prefix('#') {
        (16, rest)
    } else if unsigned.len() > 1 && unsigned.starts_with('0') {
        (8, &unsigned[1..])
    } else {
        (10, unsigned)
    };
    parse_signed(negative, digits, radix, text)
}

/// Java `Integer.parseInt(text)`: decimal only, optional sign.
fn java_parse_int(text: &str) -> Result<i32, DataError> {
    let (negative, digits) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    parse_signed(negative, digits, 10, text)
}

fn parse_signed(negative: bool, digits: &str, radix: u32, text: &str) -> Result<i32, DataError> {
    let invalid = || DataError::Invalid(format!("For input string: \"{text}\""));
    if digits.is_empty() || digits.starts_with(['+', '-']) {
        return Err(invalid());
    }
    let magnitude = i64::from_str_radix(digits, radix).map_err(|_| invalid())?;
    let value = if negative { -magnitude } else { magnitude };
    i32::try_from(value).map_err(|_| invalid())
}

/// Java `Boolean.parseBoolean`: only `true`, in any case.
fn java_boolean(text: &str) -> bool {
    text.eq_ignore_ascii_case("true")
}

/// Java `String.split(";")`: trailing empty pieces are dropped, and an input without `;` is one piece.
fn java_split_semicolon(text: &str) -> Vec<String> {
    if !text.contains(';') {
        return vec![text.to_owned()];
    }
    let mut pieces: Vec<String> = text.split(';').map(str::to_owned).collect();
    while pieces.last().is_some_and(String::is_empty) {
        pieces.pop();
    }
    pieces
}

fn required<'a>(value: Option<&'a str>, attribute: &str) -> Result<&'a str, DataError> {
    // Java: `getNamedItem(name)` returns null and the unboxing or `getNodeValue` throws NPE.
    value.ok_or_else(|| DataError::Invalid(format!("missing attribute {attribute}")))
}

fn optional_int(value: Option<&str>, default: i32) -> Result<i32, DataError> {
    value.map_or(Ok(default), java_decode)
}

// ---------------------------------------------------------------------------------------------
// Raw XML shape (xsd/spawns.xsd). Child order is kept with `$value`.
// ---------------------------------------------------------------------------------------------

#[derive(Deserialize)]
struct RawSpawnFile {
    #[serde(rename = "@enabled")]
    enabled: Option<String>,
    #[serde(rename = "spawn", default)]
    spawns: Vec<RawSpawnBlock>,
}

#[derive(Deserialize)]
struct RawSpawnBlock {
    #[serde(rename = "@name")]
    name: Option<String>,
    #[serde(rename = "@zone")]
    zone: Option<String>,
    #[serde(rename = "@zones")]
    zones: Option<String>,
    #[serde(rename = "@selection")]
    selection: Option<String>,
    #[serde(rename = "@maximumNpc")]
    maximum_npc: Option<String>,
    #[serde(rename = "$value", default)]
    children: Vec<RawSpawnChild>,
}

/// The children Java's walk of a `<spawn>` looks at. Other elements (and text) are ignored.
#[derive(Deserialize)]
enum RawSpawnChild {
    #[serde(rename = "npc")]
    Npc(Box<RawNpc>),
    #[serde(rename = "territory")]
    Territory(RawTerritory),
    #[serde(rename = "banned_territory")]
    BannedTerritory(RawTerritory),
    #[serde(rename = "AIData")]
    AiData(BTreeMap<String, String>),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct RawNpc {
    #[serde(rename = "@id")]
    id: Option<String>,
    #[serde(rename = "@x")]
    x: Option<String>,
    #[serde(rename = "@y")]
    y: Option<String>,
    #[serde(rename = "@z")]
    z: Option<String>,
    #[serde(rename = "@count")]
    count: Option<String>,
    #[serde(rename = "@heading")]
    heading: Option<String>,
    #[serde(rename = "@respawnDelay")]
    respawn_delay: Option<String>,
    #[serde(rename = "@respawnRandom")]
    respawn_random: Option<String>,
    #[serde(rename = "@chaseRange")]
    chase_range: Option<String>,
    #[serde(rename = "@periodOfDay")]
    period_of_day: Option<String>,
}

#[derive(Deserialize)]
struct RawTerritory {
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
    #[serde(rename = "node", default)]
    nodes: Vec<RawNode>,
}

#[derive(Deserialize)]
struct RawNode {
    #[serde(rename = "@x")]
    x: Option<String>,
    #[serde(rename = "@y")]
    y: Option<String>,
    #[serde(rename = "@rad")]
    rad: Option<String>,
}

// ---------------------------------------------------------------------------------------------
// The table
// ---------------------------------------------------------------------------------------------

/// All spawns, groups and territories from `data/spawns`. Java `SpawnData` + `SpawnTable`.
#[derive(Debug, Default)]
pub struct SpawnData {
    groups: Vec<SpawnGroup>,
    spawns: Vec<NpcSpawn>,
    territories: BTreeMap<String, SpawnTerritory>,
    parsed_files: usize,
}

/// The data of one `<spawn>` block while its children are walked (Java's block locals).
struct Block {
    source: String,
    file_name: String,
    /// Java `territoryName`: the `zone` attribute, or the first of `zones`.
    territory: Option<String>,
    /// Java `territoryNames`: the `zones` split, kept only for a group.
    territory_names: Option<Vec<String>>,
    group: Option<usize>,
    ai_data: Option<BTreeMap<String, i32>>,
}

/// The values of one `<npc>` that do not depend on the territory (Java's `spawnInfo`).
#[derive(Clone)]
struct NpcDraft {
    npc_id: i32,
    location: Location,
    heading: i32,
    respawn_delay: i32,
    respawn_random: i32,
    chase_range: i32,
    name: Option<String>,
    period_of_day: Option<PeriodOfDay>,
}

enum ShapeOutcome {
    Shape(TerritoryShape),
    /// Java logs a warning and `continue`s: nothing is registered.
    Rejected,
    /// Java's `catch` path: the territory is registered with a null shape.
    Broken,
}

impl SpawnData {
    /// Loads `<root>/data/spawns/**/*.xml` (subfolders included, in sorted path order) and logs
    /// Java's count lines. `npcs` decides which NPCs may spawn (Java `checkTemplate`).
    pub fn load(root: &Path, npcs: &NpcTemplates, options: &SpawnLoadOptions) -> Self {
        let mut table = Self::default();
        if options.no_spawns {
            return table;
        }
        info!("{LOADER}: Initializing spawns...");
        let dir = root.join("data/spawns");
        let mut files = Vec::new();
        if dir.is_dir() {
            collect_xml_files(&dir, &mut files);
        } else {
            warn!("Directory not found: {}", dir.display());
        }
        files.sort();
        for path in files {
            let source = source_name(root, &path);
            let file_name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            load_file(&path, |doc: RawSpawnFile| {
                table.add_file(doc, &source, &file_name, npcs, options)
            });
        }
        table.log_counts();
        table
    }

    /// Groups in creation order (Java `SpawnGroupTable`).
    pub fn groups(&self) -> &[SpawnGroup] {
        &self.groups
    }

    /// Every spawn of the table (Java `SpawnTable`): fixed spawns and group slots, in the order
    /// they were declared. Group slots have `group` set.
    pub fn spawns(&self) -> &[NpcSpawn] {
        &self.spawns
    }

    /// The spawns of one NPC template (Java `SpawnTable.getSpawns`).
    pub fn spawns_of(&self, npc_id: i32) -> impl Iterator<Item = &NpcSpawn> {
        self.spawns.iter().filter(move |spawn| spawn.npc_id == npc_id)
    }

    /// Territory by name (Java `ZoneManager.getSpawnTerritory`).
    pub fn territory(&self, name: &str) -> Option<&SpawnTerritory> {
        self.territories.get(name)
    }

    /// All territories, by name.
    pub fn territories(&self) -> impl Iterator<Item = &SpawnTerritory> {
        self.territories.values()
    }

    /// Number of spawn files parsed (Java `_parsedFiles`).
    pub fn parsed_files(&self) -> usize {
        self.parsed_files
    }

    /// Spawns that are created at startup without a roll: every fixed spawn, plus the entries of
    /// `all` groups that fit `maximumNpc` (Java `SpawnGroup.tryReserve`). Pets, decoys and traps,
    /// which Java counts without creating, are counted here as well.
    pub fn planned_fixed_spawns(&self) -> usize {
        let mut total = self.spawns.iter().filter(|spawn| spawn.group.is_none()).count();
        for group in &self.groups {
            if group.selection != SpawnSelection::All {
                continue;
            }
            let mut reserved = 0_i64;
            for entry in &group.entries {
                let count = i64::from(entry.total);
                if group.maximum_npc > 0 && reserved + count > i64::from(group.maximum_npc) {
                    continue;
                }
                reserved += count;
                total += usize::try_from(entry.total.max(0)).unwrap_or(0);
            }
        }
        total
    }

    /// Groups whose entries are rolled when the world starts (`random` and `randomFill`).
    pub fn random_groups(&self) -> usize {
        self.groups
            .iter()
            .filter(|group| group.selection != SpawnSelection::All && !group.entries.is_empty())
            .count()
    }

    /// Number of spawns in the table (fixed spawns and group slots).
    pub fn len(&self) -> usize {
        self.spawns.len()
    }

    /// True when no spawn was loaded.
    pub fn is_empty(&self) -> bool {
        self.spawns.is_empty()
    }

    fn log_counts(&self) {
        for group in &self.groups {
            if group.entries.is_empty() {
                warn!("SpawnGroup: {} has no valid entries to spawn.", group.name);
            }
        }
        if !self.groups.is_empty() {
            info!("{LOADER}: {} spawn groups initialized.", self.groups.len());
        }
        info!("{LOADER}: {} spawn files parsed.", self.parsed_files);
        info!(
            "{LOADER}: {} spawns declared, {} random spawn groups roll when the world starts.",
            self.planned_fixed_spawns(),
            self.random_groups()
        );
    }

    /// Java `parseDocument` for one file. Errors leave the entries parsed before them in place.
    fn add_file(
        &mut self,
        doc: RawSpawnFile,
        source: &str,
        file_name: &str,
        npcs: &NpcTemplates,
        options: &SpawnLoadOptions,
    ) -> Result<(), DataError> {
        // Java adds the file to `_parsedFiles` before it looks at the list.
        self.parsed_files += 1;
        let enabled = required(doc.enabled.as_deref(), "enabled")?;
        if !java_boolean(enabled) {
            return Ok(());
        }
        for raw in doc.spawns {
            self.add_block(raw, source, file_name, npcs, options)?;
        }
        Ok(())
    }

    /// Java's handling of one `<spawn>` block.
    fn add_block(
        &mut self,
        raw: RawSpawnBlock,
        source: &str,
        file_name: &str,
        npcs: &NpcTemplates,
        options: &SpawnLoadOptions,
    ) -> Result<(), DataError> {
        let mut block = Block {
            source: source.to_owned(),
            file_name: file_name.to_owned(),
            territory: raw.zone.clone(),
            territory_names: None,
            group: None,
            ai_data: None,
        };
        if let Some(zones) = raw.zones.as_deref() {
            if block.territory.is_some() {
                warn!("{LOADER}: zone and zones are exclusive, ignoring zones in {file_name}");
            } else {
                let names = java_split_semicolon(zones);
                // Java: `territoryNames[0]` throws for `zones=";"` (an empty split).
                let first = names
                    .first()
                    .cloned()
                    .ok_or_else(|| DataError::Invalid("zones has no territory name".into()))?;
                block.territory = Some(first);
                block.territory_names = Some(names);
            }
        }

        if let Some(selection_text) = raw.selection.as_deref() {
            let selection = if selection_text.eq_ignore_ascii_case("random") {
                SpawnSelection::Random
            } else if selection_text.eq_ignore_ascii_case("randomFill") {
                SpawnSelection::RandomFill
            } else {
                if !selection_text.eq_ignore_ascii_case("all") {
                    warn!("{LOADER}: Unknown selection \"{selection_text}\" in {file_name}, using all.");
                }
                SpawnSelection::All
            };
            let mut maximum_npc = optional_int(raw.maximum_npc.as_deref(), 0)?;
            if selection == SpawnSelection::Random && maximum_npc > 0 {
                // Retail random_spawn never reads the cap.
                warn!("{LOADER}: maximumNpc is not read by random selection, ignoring it in {file_name}");
                maximum_npc = 0;
            }
            if selection == SpawnSelection::RandomFill && maximum_npc <= 0 {
                warn!(
                    "{LOADER}: randomFill requires maximumNpc, the group in {file_name} will spawn nothing."
                );
            }
            let name = raw
                .name
                .clone()
                .or_else(|| block.territory.clone())
                .unwrap_or_else(|| file_name.to_owned());
            self.groups.push(SpawnGroup {
                name,
                selection,
                maximum_npc,
                entries: Vec::new(),
            });
            block.group = Some(self.groups.len() - 1);
        } else if block.territory_names.is_some() {
            warn!(
                "{LOADER}: zones requires an entry selection, using only the first territory in {file_name}"
            );
            block.territory_names = None;
        }

        let spawn_name = raw.name;
        for child in raw.children {
            match child {
                RawSpawnChild::AiData(values) => {
                    // Every child element of AIData is one `name` = value entry.
                    let map = block.ai_data.get_or_insert_with(BTreeMap::new);
                    for (key, text) in values {
                        let value = match key.as_str() {
                            "disableRandomAnimation" | "disableRandomWalk" => i32::from(java_boolean(&text)),
                            _ => java_parse_int(&text)?,
                        };
                        map.insert(key, value);
                    }
                }
                RawSpawnChild::Territory(raw_territory) => self.add_territory(raw_territory, &block)?,
                RawSpawnChild::BannedTerritory(raw_banned) => self.add_banned(raw_banned, &block)?,
                RawSpawnChild::Npc(raw_npc) => {
                    self.add_npc(*raw_npc, spawn_name.as_deref(), &block, npcs, options)?;
                }
                RawSpawnChild::Other => {}
            }
        }
        Ok(())
    }

    /// Java's `territory` branch: registers a named territory unless the name is taken.
    fn add_territory(&mut self, raw: RawTerritory, block: &Block) -> Result<(), DataError> {
        // Java: a missing name falls back to the block's zone; a null name throws in
        // `spawnTerritoryExists`.
        let declared = match raw.name.clone() {
            Some(name) => name,
            None => block
                .territory
                .clone()
                .ok_or_else(|| DataError::Invalid("territory without a name".into()))?,
        };
        if self.territories.contains_key(&declared) {
            return Ok(());
        }
        let min_z = java_decode(required(raw.min_z.as_deref(), "minZ")?)?;
        let max_z = java_decode(required(raw.max_z.as_deref(), "maxZ")?)?;
        let shape_name = raw.shape.as_deref().unwrap_or("NPoly");
        let what = format!("territory {declared}");
        // NOTE(java-quirk): `attrs` still points at the last `<node>` when the cylinder radius is read,
        // so `rad` comes from that node if there is one.
        let rad = raw
            .nodes
            .last()
            .map_or(raw.rad.as_deref(), |node| node.rad.as_deref());
        let shape = match build_shape(
            shape_name,
            &raw.nodes,
            rad,
            (min_z, max_z),
            &what,
            &block.file_name,
        ) {
            ShapeOutcome::Shape(shape) => Some(shape),
            ShapeOutcome::Rejected => return Ok(()),
            ShapeOutcome::Broken => None,
        };
        self.territories.insert(
            declared.clone(),
            SpawnTerritory {
                name: declared,
                shape,
                banned: Vec::new(),
            },
        );
        Ok(())
    }

    /// Java's `banned_territory` branch: adds an area to the block's territory.
    fn add_banned(&mut self, raw: RawTerritory, block: &Block) -> Result<(), DataError> {
        let name = block
            .territory
            .clone()
            .ok_or_else(|| DataError::Invalid("banned_territory without a territory name".into()))?;
        if !self.territories.contains_key(&name) {
            warn!("{LOADER}: SpawnTable: banned_territory {name} must be set after normal territory.");
            return Ok(());
        }
        let min_z = java_decode(required(raw.min_z.as_deref(), "minZ")?)?;
        let max_z = java_decode(required(raw.max_z.as_deref(), "maxZ")?)?;
        let shape_name = raw.shape.as_deref().unwrap_or("NPoly");
        let what = format!("banned_territory {name}");
        let rad = raw
            .nodes
            .last()
            .map_or(raw.rad.as_deref(), |node| node.rad.as_deref());
        let outcome = match build_shape(
            shape_name,
            &raw.nodes,
            rad,
            (min_z, max_z),
            &what,
            &block.file_name,
        ) {
            ShapeOutcome::Shape(shape) => Some(Some(shape)),
            ShapeOutcome::Rejected => None,
            ShapeOutcome::Broken => Some(None),
        };
        if let (Some(entry), Some(territory)) = (outcome, self.territories.get_mut(&name)) {
            territory.banned.push(entry);
        }
        Ok(())
    }

    /// Java's `npc` branch: one or `count` spawns, or the slots of a group entry.
    fn add_npc(
        &mut self,
        raw: RawNpc,
        spawn_name: Option<&str>,
        block: &Block,
        npcs: &NpcTemplates,
        options: &SpawnLoadOptions,
    ) -> Result<(), DataError> {
        let npc_id = java_decode(required(raw.id.as_deref(), "id")?)?;
        if !check_template(npc_id, npcs, options) {
            return Ok(());
        }
        // Java reads x, y, z in turn and stops at the first missing one (the NPE is caught), so
        // a missing y keeps x and leaves y and z at 0.
        let (mut x, mut y, mut z) = (0, 0, 0);
        if let Some(text) = raw.x.as_deref() {
            x = java_decode(text)?;
            if let Some(text) = raw.y.as_deref() {
                y = java_decode(text)?;
                if let Some(text) = raw.z.as_deref() {
                    z = java_decode(text)?;
                }
            }
        }
        if x == 0 && y == 0 && block.territory.is_none() {
            warn!(
                "XML Spawnlist: Spawn could not be initialized, both coordinates and zone are unspecified for ID {npc_id}"
            );
            return Ok(());
        }
        let count = optional_int(raw.count.as_deref(), 1)?;
        let period_of_day = raw.period_of_day.as_deref().and_then(|text| {
            if text.eq_ignore_ascii_case("day") {
                Some(PeriodOfDay::Day)
            } else if text.eq_ignore_ascii_case("night") {
                Some(PeriodOfDay::Night)
            } else {
                None
            }
        });
        let draft = NpcDraft {
            npc_id,
            location: Location { x, y, z },
            heading: optional_int(raw.heading.as_deref(), -1)?,
            respawn_delay: optional_int(raw.respawn_delay.as_deref(), 0)?,
            respawn_random: optional_int(raw.respawn_random.as_deref(), 0)?,
            chase_range: optional_int(raw.chase_range.as_deref(), 0)?,
            name: spawn_name.filter(|name| !name.is_empty()).map(str::to_owned),
            period_of_day,
        };

        if let Some(group) = block.group {
            if period_of_day.is_some() {
                warn!(
                    "{LOADER}: periodOfDay cannot be combined with an entry selection, skipping NPC {npc_id} in {}",
                    block.file_name
                );
                return Ok(());
            }
            self.add_group_entry(group, &draft, count, block);
            return Ok(());
        }

        for _ in 0..count {
            let spawn = self.make_spawn(&draft, block, block.territory.clone(), 1, None);
            self.spawns.push(spawn);
        }
        Ok(())
    }

    /// Java `addGroupSpawn`: one slot per territory of the block, then the entry joins the group.
    fn add_group_entry(&mut self, group: usize, draft: &NpcDraft, total: i32, block: &Block) {
        let selection = self.groups[group].selection;
        let slot_territories: Vec<Option<String>> = match &block.territory_names {
            Some(names) => names.iter().map(|name| Some(name.clone())).collect(),
            None => vec![block.territory.clone()],
        };
        let mut slots = Vec::with_capacity(slot_territories.len());
        for territory in slot_territories {
            // Java: a rolling group, or an `all` group without a delay, stops the slot's own respawn.
            let respawn_enabled = selection == SpawnSelection::All && draft.respawn_delay > 0;
            let spawn = self.make_spawn(draft, block, territory, total, Some((group, respawn_enabled)));
            self.spawns.push(spawn);
            slots.push(self.spawns.len() - 1);
        }
        self.groups[group].entries.push(SpawnGroupEntry {
            total,
            respawn_delay: draft.respawn_delay,
            respawn_random: draft.respawn_random,
            slots,
        });
    }

    /// Java `createSpawn` (plus the respawn flag Java sets afterwards).
    fn make_spawn(
        &self,
        draft: &NpcDraft,
        block: &Block,
        territory: Option<String>,
        amount: i32,
        group: Option<(usize, bool)>,
    ) -> NpcSpawn {
        let (respawn_min_ms, respawn_max_ms) = respawn_range(draft.respawn_delay, draft.respawn_random);
        if draft.respawn_delay < 0 {
            warn!(
                "respawn delay is negative for spawn: NPC {} in {}",
                draft.npc_id, block.file_name
            );
        }
        let territory = territory
            .filter(|name| !name.is_empty())
            .filter(|name| self.territories.contains_key(name));
        let respawn_enabled = match group {
            Some((_, enabled)) => enabled,
            None if draft.period_of_day.is_some() => true,
            None => respawn_min_ms > 0,
        };
        NpcSpawn {
            npc_id: draft.npc_id,
            amount,
            location: draft.location,
            heading: draft.heading,
            respawn_min_ms,
            respawn_max_ms,
            respawn_enabled,
            chase_range: draft.chase_range,
            name: draft.name.clone(),
            territory,
            period_of_day: draft.period_of_day,
            ai_data: block.ai_data.clone(),
            group: group.map(|(index, _)| index),
            source: block.source.clone(),
        }
    }
}

/// Java `Spawn.setRespawnDelay(delay, randomInterval)`: milliseconds, `(0, 0)` for no delay.
fn respawn_range(delay: i32, random: i32) -> (i32, i32) {
    if delay == 0 {
        return (0, 0);
    }
    let min = delay.wrapping_sub(random).max(1).wrapping_mul(1000);
    let max = delay.wrapping_add(random).max(1).wrapping_mul(1000);
    (min, max)
}

/// Java `SpawnData.checkTemplate`: the NPC exists and may be spawned.
fn check_template(npc_id: i32, npcs: &NpcTemplates, options: &SpawnLoadOptions) -> bool {
    let Some(template) = npcs.get(npc_id) else {
        // NOTE(java-quirk): Java's range test `(npcId < 80000) && (npcId > 89999)` is never true, so
        // Java warns only when fake players are enabled. Kept as is.
        if options.fake_players_enabled {
            warn!("{LOADER}: Requested spawn for non-existing NPC: {npc_id}.");
        }
        return false;
    };
    if template.npc_type.eq_ignore_ascii_case("SiegeGuard")
        || template.npc_type.eq_ignore_ascii_case("RaidBoss")
    {
        return false;
    }
    !(!options.fake_players_enabled && template.fake_player)
}

/// Java's shape code shared by `territory` and `banned_territory`. `rad` is the radius attribute
/// Java reads for a cylinder. `min_max_z` are the Z bounds as written.
fn build_shape(
    shape_name: &str,
    nodes: &[RawNode],
    rad: Option<&str>,
    min_max_z: (i32, i32),
    what: &str,
    file_name: &str,
) -> ShapeOutcome {
    let (min_z, max_z) = min_max_z;
    let mut points = Vec::with_capacity(nodes.len());
    for node in nodes {
        let point = required(node.x.as_deref(), "x")
            .and_then(java_decode)
            .and_then(|x| {
                required(node.y.as_deref(), "y")
                    .and_then(java_decode)
                    .map(|y| (x, y))
            });
        match point {
            Ok(point) => points.push(point),
            Err(error) => {
                warn!("{LOADER}: SpawnTable: Failed to load {what} coordinates: {error}");
                return ShapeOutcome::Broken;
            }
        }
    }
    if points.is_empty() {
        warn!("{LOADER}: SpawnTable: missing data for spawn {what} XML file: {file_name}");
        return ShapeOutcome::Rejected;
    }
    let z1 = min_z.min(max_z);
    let z2 = min_z.max(max_z);
    if shape_name.eq_ignore_ascii_case("Cuboid") {
        if points.len() != 2 {
            warn!("{LOADER}: SpawnTable: Missing cuboid vertex data for {what} in file: {file_name}");
            return ShapeOutcome::Rejected;
        }
        let (x1, x2) = (points[0].0, points[1].0);
        let (y1, y2) = (points[0].1, points[1].1);
        let (left, right) = (x1.min(x2), x1.max(x2));
        let (top, bottom) = (y1.min(y2), y1.max(y2));
        return ShapeOutcome::Shape(TerritoryShape::Cuboid {
            x: left,
            y: top,
            width: right.wrapping_sub(left),
            height: bottom.wrapping_sub(top),
            z1,
            z2,
        });
    }
    if shape_name.eq_ignore_ascii_case("NPoly") {
        // NOTE(java-quirk): a polygon needs more than two vertices; a two-point "polygon" is rejected.
        if points.len() <= 2 {
            warn!("{LOADER}: SpawnTable: Bad data for {what} in file: {file_name}");
            return ShapeOutcome::Rejected;
        }
        return ShapeOutcome::Shape(TerritoryShape::NPoly {
            xs: points.iter().map(|point| point.0).collect(),
            ys: points.iter().map(|point| point.1).collect(),
            z1,
            z2,
        });
    }
    if shape_name.eq_ignore_ascii_case("Cylinder") {
        let rad = match rad.map(java_parse_int) {
            Some(Ok(rad)) => rad,
            Some(Err(error)) => {
                warn!("{LOADER}: SpawnTable: Failed to load {what} coordinates: {error}");
                return ShapeOutcome::Broken;
            }
            None => {
                warn!("{LOADER}: SpawnTable: Failed to load {what} coordinates: missing attribute rad");
                return ShapeOutcome::Broken;
            }
        };
        if points.len() != 1 || rad <= 0 {
            warn!("{LOADER}: SpawnTable: Bad data for {what} in file: {file_name}");
            return ShapeOutcome::Rejected;
        }
        // The cylinder keeps its Z bounds as written (Java does not normalise them).
        return ShapeOutcome::Shape(TerritoryShape::Cylinder {
            x: points[0].0,
            y: points[0].1,
            z1: min_z,
            z2: max_z,
            rad,
        });
    }
    warn!("{LOADER}: SpawnTable: Unknown shape: \"{shape_name}\"  for {what} in file: {file_name}");
    ShapeOutcome::Rejected
}

/// Lists `.xml` files below `dir`, recursively (Java `parseDatapackDirectory(path, true)`).
fn collect_xml_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) => {
            warn!("Cannot list {}: {error}", dir.display());
            return;
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_xml_files(&path, out);
        } else if path.is_file()
            && path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("xml"))
        {
            out.push(path);
        }
    }
}

/// `data/spawns/...` with forward slashes, as Java's `fileName` reads on every platform.
fn source_name(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rng() -> WorldRng {
        WorldRng::from_seed(7)
    }

    fn no_geo(_x: i32, _y: i32, z: i32) -> i32 {
        z
    }

    fn square(x: i32, y: i32, size: i32) -> TerritoryShape {
        TerritoryShape::NPoly {
            xs: vec![x, x + size, x + size, x],
            ys: vec![y, y, y + size, y + size],
            z1: -100,
            z2: 100,
        }
    }

    #[test]
    fn java_decode_follows_integer_decode() {
        assert_eq!(java_decode("-95152").unwrap(), -95152);
        assert_eq!(java_decode("0x10").unwrap(), 16);
        assert_eq!(java_decode("#ff").unwrap(), 255);
        assert_eq!(java_decode("010").unwrap(), 8, "a leading zero is octal");
        assert_eq!(java_decode("0").unwrap(), 0);
        assert_eq!(java_decode("+7").unwrap(), 7);
        assert!(java_decode("08").is_err());
        assert!(java_decode("--5").is_err());
        assert!(java_decode("").is_err());
        assert!(java_decode("1 ").is_err());
    }

    #[test]
    fn java_parse_int_is_decimal_only() {
        assert_eq!(java_parse_int("010").unwrap(), 10);
        assert!(java_parse_int("0x10").is_err());
        assert!(java_parse_int("+-1").is_err());
        assert!(java_parse_int("2147483648").is_err());
    }

    #[test]
    fn split_matches_java_string_split() {
        assert_eq!(java_split_semicolon("a;b;"), ["a", "b"]);
        assert_eq!(java_split_semicolon(";a"), ["", "a"]);
        assert_eq!(java_split_semicolon(""), [""]);
        assert!(java_split_semicolon(";").is_empty());
    }

    #[test]
    fn polygon_contains_is_half_open_on_the_bounding_box() {
        let shape = square(0, 0, 10);
        assert!(shape.contains(0, 0, 0));
        assert!(shape.contains(9, 9, 0));
        assert!(
            !shape.contains(10, 5, 0),
            "right edge is outside, as in Polygon.contains"
        );
        assert!(!shape.contains(5, 10, 0), "bottom edge is outside");
        assert!(!shape.contains(5, 5, 101), "above the Z band");
        assert!(!shape.contains(-1, 5, 0));
    }

    #[test]
    fn concave_polygon_excludes_its_notch() {
        // A square with a notch cut from the top middle.
        let shape = TerritoryShape::NPoly {
            xs: vec![0, 10, 10, 6, 5, 4, 0],
            ys: vec![0, 0, 10, 10, 4, 10, 10],
            z1: 0,
            z2: 0,
        };
        assert!(shape.contains(2, 8, 0));
        assert!(!shape.contains(5, 8, 0), "inside the notch");
        assert!(shape.contains(8, 2, 0));
    }

    #[test]
    fn cuboid_is_a_half_open_rectangle() {
        let shape = TerritoryShape::Cuboid {
            x: 0,
            y: 0,
            width: 4,
            height: 4,
            z1: 0,
            z2: 10,
        };
        assert!(shape.contains(0, 0, 10));
        assert!(!shape.contains(4, 0, 5));
        assert!(!shape.contains(0, 0, 11));
    }

    #[test]
    fn npoly_random_points_stay_inside_with_a_fixed_seed() {
        let shape = square(100, 200, 50);
        let territory = SpawnTerritory {
            name: "t".into(),
            shape: Some(shape.clone()),
            banned: Vec::new(),
        };
        let mut first = rng();
        let mut second = rng();
        for _ in 0..200 {
            let a = territory.random_point(&mut first, &no_geo).unwrap();
            let b = territory.random_point(&mut second, &no_geo).unwrap();
            assert_eq!(a, b, "the same seed gives the same points");
            assert!(shape.contains(a.x, a.y, a.z), "{a:?} is inside the square");
        }
    }

    #[test]
    fn banned_areas_are_avoided() {
        let territory = SpawnTerritory {
            name: "t".into(),
            shape: Some(square(0, 0, 100)),
            banned: vec![Some(square(0, 0, 90))],
        };
        let mut rng = rng();
        for _ in 0..200 {
            let point = territory.random_point(&mut rng, &no_geo).unwrap();
            assert!(!square(0, 0, 90).contains(point.x, point.y, point.z));
        }
    }

    #[test]
    fn points_above_the_ground_are_rejected_when_geodata_says_so() {
        let territory = SpawnTerritory {
            name: "t".into(),
            shape: Some(square(0, 0, 100)),
            banned: Vec::new(),
        };
        // Ground is 500 everywhere, above the territory high Z (100): all 100 tries fail the
        // check and the last sample is returned unchecked, as Java does.
        let high_ground = |_x: i32, _y: i32, _z: i32| 500;
        let point = territory.random_point(&mut rng(), &high_ground).unwrap();
        assert_eq!(point.z, 500);
        // Ground below the high Z is accepted on the first try.
        let low_ground = |_x: i32, _y: i32, _z: i32| 0;
        let point = territory.random_point(&mut rng(), &low_ground).unwrap();
        assert_eq!(point.z, 0);
    }

    #[test]
    fn cylinder_measures_its_first_candidate_from_the_origin() {
        // Centre within `rad` of (0, 0): Java returns (0, 0) without drawing a number.
        let near_origin = TerritoryShape::Cylinder {
            x: 1,
            y: 1,
            z1: 0,
            z2: 0,
            rad: 5,
        };
        let mut used = rng();
        let mut untouched = rng();
        let point = near_origin.random_point(&mut used, &no_geo);
        assert_eq!((point.x, point.y), (0, 0));
        assert_eq!(
            used.get(0, 1_000_000),
            untouched.get(0, 1_000_000),
            "no draw was used"
        );

        let far = TerritoryShape::Cylinder {
            x: 100_000,
            y: 100_000,
            z1: 0,
            z2: 0,
            rad: 300,
        };
        let mut rng = rng();
        for _ in 0..50 {
            let point = far.random_point(&mut rng, &no_geo);
            assert!(far.contains(point.x, point.y, point.z));
        }
    }

    #[test]
    fn respawn_range_follows_set_respawn_delay() {
        assert_eq!(respawn_range(0, 5), (0, 0));
        assert_eq!(respawn_range(60, 20), (40_000, 80_000));
        assert_eq!(
            respawn_range(10, 20),
            (1_000, 30_000),
            "minimum delay is one second"
        );
    }
}
