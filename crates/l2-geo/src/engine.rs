//! `GeoEngine`: whole-world geodata, height, line of sight and movement checks.
//!
//! Java: `gameserver/geoengine/GeoEngine.java` (constructor, `getRegion`, `hasGeoPos`,
//! `checkNearestNswe*`, `getNearestZ`/`getNextLowerZ`/`getNextHigherZ`, `getHeight`,
//! `getSpawnHeight`, `canSeeTarget`, `getValidLocation`, `canMoveToTarget`, `isCompletelyBlocked`),
//! `config/GeoEngineConfig.java` and `util/GeoUtils.java` (`computeNswe`).
//!
//! # What is ported
//! Everything `GeoEngine` does on top of the M1 region model, except what changes geodata at
//! runtime: `setNearestNswe` / `unsetNearestNswe` / `reloadRegion` / `setRegion` (doors write NSWE
//! into the geodata in Java; here the engine is **immutable** after loading, so it is `Send + Sync`
//! and can be shared with worker threads, D-14). Methods that take a `WorldObject` in Java
//! (door / artefact shortcuts of `canSeeTarget`) are left to the caller; the engine takes plain
//! coordinates. Pathfinding is M3-GEO-02.
//!
//! # Loading and the per-region override (D-26)
//! [`GeoEngine::load`] walks the tiles `11..=26` x `10..=26` like Java and reads `X_Y.l2j` from the
//! main folder (`GeoDataPath`). A file that fails to parse is logged, reported in [`LoadReport`]
//! and the region stays a null region (same as Java's warning + skip).
//!
//! **Rust-only extension, not in Java:** a second folder can supply chosen regions, so the two
//! geodata sets can be mixed. Two keys, read by [`GeoOverride::from_ini`] from any [`IniFile`]
//! (D-36 says Rust-only settings must not go into the Java `GeoEngine.ini`; put them in a new
//! Rust-only file such as `config/GeoOverride.ini`, the file name is chosen by the code that
//! starts the server):
//!
//! ```ini
//! # Folder with the second geodata set. Relative paths are resolved against the datapack root
//! # (like GeoDataPath). Empty = no override (default).
//! GeoDataOverridePath = ./data/geodata-converted/
//! # Regions loaded from the override folder instead of GeoDataPath, "X_Y" separated by commas.
//! GeoDataOverrideRegions = 12_24,17_10
//! ```
//!
//! Entries that are not `X_Y` (decimal digits) or lie outside the tile range are logged and ignored.
//! If the override folder has no file (or an unreadable file) for a listed region, that is logged
//! and the region is loaded from the main folder instead.
//!
//! # Java quirks kept
//! * Geo coordinates are divided with truncation ([`crate::coords`]).
//! * `getRegion` computes `(geoX / 2048) * 32 + geoY / 2048` with truncating division and no
//!   wrapping of the Y part, so absurd coordinates alias to other regions exactly like Java. Where
//!   Java would throw `ArrayIndexOutOfBoundsException` (index outside 0..1024) this port answers
//!   like a null region.
//! * [`GeoEngine::check_nearest_nswe_anti_corner_cut`] asks the region of the *origin* cell about
//!   its neighbours too. A region only looks at the part of a coordinate inside itself, so at a
//!   region border the neighbour on the other side is read from the wrong side of the same region.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use l2_config::IniFile;
use tracing::{debug, info, warn};

use crate::cell::{
    NSWE_EAST, NSWE_NORTH, NSWE_NORTH_EAST, NSWE_NORTH_WEST, NSWE_SOUTH, NSWE_SOUTH_EAST, NSWE_SOUTH_WEST,
    NSWE_WEST,
};
use crate::coords::{Location, compute_nswe, geo_x, geo_y, world_x, world_y};
use crate::error::GeoError;
use crate::line::{GridLineIterator2D, GridLineIterator3D};
use crate::obstacles::{DynamicObstacles, NoObstacles};
use crate::region::{GEO_REGIONS_SIDE, GeoSet, REGION_CELLS_SIDE, Region};

/// `World.TILE_X_MIN`: first region column that has a geodata file.
pub const TILE_X_MIN: u32 = 11;
/// `World.TILE_X_MAX`.
pub const TILE_X_MAX: u32 = 26;
/// `World.TILE_Y_MIN`.
pub const TILE_Y_MIN: u32 = 10;
/// `World.TILE_Y_MAX`.
pub const TILE_Y_MAX: u32 = 26;

/// Config key (Rust only, D-26): folder with the second geodata set.
pub const OVERRIDE_PATH_KEY: &str = "GeoDataOverridePath";
/// Config key (Rust only, D-26): `X_Y,X_Y,...` regions to load from the override folder.
pub const OVERRIDE_REGIONS_KEY: &str = "GeoDataOverrideRegions";

const ELEVATED_SEE_OVER_DISTANCE: i32 = 2;
const MAX_SEE_OVER_HEIGHT: i32 = 48;
const SPAWN_Z_DELTA_LIMIT: i32 = 100;
const HEIGHT_INCREASE_LIMIT: i32 = 40;
const SPAWN_HEIGHT_OFFSET: i32 = 20;
/// When the height check fails on a single cell, accept the move if a neighbour of that cell has
/// a layer within this many units of the source Z (`PATH_CONTINUITY_TOLERANCE`).
const PATH_CONTINUITY_TOLERANCE: i32 = 16;

/// The `GeoEngineConfig` values the engine itself reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeoEngineOptions {
    /// `PathFinding`: 0 off, 1 path node files, 2 geodata cells. The engine only asks "is it at
    /// least 1" (`isCompletelyBlocked`).
    pub pathfinding: i32,
    /// `AvoidObstructedPathNodes`: `canMoveToTarget` also checks the plain NSWE of every step.
    pub avoid_obstructed_path_nodes: bool,
}

impl Default for GeoEngineOptions {
    /// The defaults of `GeoEngineConfig.load()` when a key is missing (not the shipped ini).
    fn default() -> Self {
        GeoEngineOptions {
            pathfinding: 0,
            avoid_obstructed_path_nodes: true,
        }
    }
}

impl GeoEngineOptions {
    /// The values of the shipped `dist/game/config/GeoEngine.ini`: `PathFinding = 2`,
    /// `AvoidObstructedPathNodes = True`.
    pub fn shipped() -> Self {
        GeoEngineOptions {
            pathfinding: 2,
            avoid_obstructed_path_nodes: true,
        }
    }
}

impl From<&l2_config::game::GeoEngineConfig> for GeoEngineOptions {
    fn from(config: &l2_config::game::GeoEngineConfig) -> Self {
        GeoEngineOptions {
            pathfinding: config.pathfinding,
            avoid_obstructed_path_nodes: config.avoid_obstructed_path_nodes,
        }
    }
}

/// Where the second geodata set is and which regions come from it (D-26). Rust-only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeoOverride {
    /// Folder with the `X_Y.l2j` files of the second set.
    pub path: PathBuf,
    /// Regions `(x, y)` to take from [`path`](Self::path), in the order given, without duplicates.
    pub regions: Vec<(u32, u32)>,
    /// Entries of the list that were not usable, as written (already logged).
    pub ignored: Vec<String>,
}

impl GeoOverride {
    /// Builds the override from a folder and the text of `GeoDataOverrideRegions`.
    pub fn new(path: impl Into<PathBuf>, regions: &str) -> Self {
        let (regions, ignored) = Self::parse_regions(regions);
        GeoOverride {
            path: path.into(),
            regions,
            ignored,
        }
    }

    /// Reads [`OVERRIDE_PATH_KEY`] and [`OVERRIDE_REGIONS_KEY`]. `None` when the path is empty or
    /// missing, or when no usable region is listed (default: no override). A relative path is
    /// joined to `base` (the datapack root); an absolute path stays as it is.
    pub fn from_ini(ini: &IniFile, base: &Path) -> Option<GeoOverride> {
        let path = ini
            .get_value(OVERRIDE_PATH_KEY)
            .map(str::trim)
            .filter(|p| !p.is_empty())?;
        let list = ini.get_value(OVERRIDE_REGIONS_KEY).unwrap_or("");
        let over = GeoOverride::new(base.join(path), list);
        if over.regions.is_empty() {
            info!(
                "GeoEngine: {OVERRIDE_PATH_KEY} is set but {OVERRIDE_REGIONS_KEY} lists no usable region, no override."
            );
            return None;
        }
        Some(over)
    }

    /// Splits `12_24, 17_10` into regions. Anything that is not `X_Y` (decimal digits) inside the
    /// tile range [`TILE_X_MIN`]..=[`TILE_X_MAX`] x [`TILE_Y_MIN`]..=[`TILE_Y_MAX`] is logged and
    /// returned in the second list. Empty entries (a trailing comma) are skipped silently.
    pub fn parse_regions(text: &str) -> (Vec<(u32, u32)>, Vec<String>) {
        let mut regions = Vec::new();
        let mut ignored = Vec::new();
        for entry in text.split(',').map(str::trim).filter(|e| !e.is_empty()) {
            match parse_region_key(entry) {
                Some((x, y))
                    if (TILE_X_MIN..=TILE_X_MAX).contains(&x) && (TILE_Y_MIN..=TILE_Y_MAX).contains(&y) =>
                {
                    if !regions.contains(&(x, y)) {
                        regions.push((x, y));
                    }
                }
                _ => {
                    warn!(
                        "GeoEngine: ignoring geodata override entry '{entry}' (expected X_Y with X in {TILE_X_MIN}..{TILE_X_MAX} and Y in {TILE_Y_MIN}..{TILE_Y_MAX})."
                    );
                    ignored.push(entry.to_owned());
                }
            }
        }
        (regions, ignored)
    }
}

/// `X_Y` with plain decimal digits.
fn parse_region_key(entry: &str) -> Option<(u32, u32)> {
    let (x, y) = entry.split_once('_')?;
    let digits = |s: &str| !s.is_empty() && s.len() <= 4 && s.bytes().all(|b| b.is_ascii_digit());
    if !digits(x) || !digits(y) {
        return None;
    }
    Some((x.parse().ok()?, y.parse().ok()?))
}

/// Everything [`GeoEngine::load`] needs.
#[derive(Clone, Debug)]
pub struct GeoLoadSpec {
    /// `GeoDataPath`, already resolved.
    pub geodata_path: PathBuf,
    /// Optional second folder for some regions (D-26).
    pub override_: Option<GeoOverride>,
}

impl GeoLoadSpec {
    /// Main folder only, no override.
    pub fn new(geodata_path: impl Into<PathBuf>) -> Self {
        GeoLoadSpec {
            geodata_path: geodata_path.into(),
            override_: None,
        }
    }

    pub fn with_override(mut self, over: Option<GeoOverride>) -> Self {
        self.override_ = over;
        self
    }
}

/// What happened while loading.
#[derive(Debug, Default)]
pub struct LoadReport {
    /// Regions now present (`Loaded N regions.` in Java).
    pub loaded: usize,
    /// Regions that came from the override folder.
    pub overridden: Vec<(u32, u32)>,
    /// Files that exist but could not be parsed; those regions are null regions (or, for an
    /// override file, were loaded from the main folder).
    pub failed: Vec<GeoError>,
    /// Listed override regions without a usable file in the override folder.
    pub override_missing: Vec<(u32, u32)>,
    /// True when no region loaded and `PathFinding` was above 0, so it was set to 0 like Java does.
    pub pathfinding_disabled: bool,
}

/// Upper bound for the loader threads.
const MAX_LOAD_THREADS: usize = 8;

/// One tile position to load.
struct RegionJob {
    x: u32,
    y: u32,
    file_name: String,
    /// The override folder when this region is on the override list.
    override_dir: Option<PathBuf>,
}

#[derive(Default)]
struct JobResult {
    region: Option<Region>,
    from_override: bool,
    override_missing: bool,
    failed: Vec<GeoError>,
}

impl RegionJob {
    /// Override file first (when listed), then the main folder. A missing file is silent (most
    /// tiles are sea), a broken one is logged and reported.
    fn run(&self, geodata_path: &Path) -> JobResult {
        let mut result = JobResult::default();
        if let Some(dir) = &self.override_dir {
            let path = dir.join(&self.file_name);
            if path.is_file() {
                match Region::load_file(&path) {
                    Ok(r) => {
                        result.region = Some(r);
                        result.from_override = true;
                    }
                    Err(e) => {
                        warn!("GeoEngine: Failed to load override {}: {e}", path.display());
                        result.failed.push(e);
                    }
                }
            }
            if result.region.is_none() {
                warn!(
                    "GeoEngine: no usable override file for region {}_{} in {}, using GeoDataPath.",
                    self.x,
                    self.y,
                    dir.display()
                );
                result.override_missing = true;
            }
        }
        if result.region.is_none() {
            let path = geodata_path.join(&self.file_name);
            if path.is_file() {
                match Region::load_file(&path) {
                    Ok(r) => result.region = Some(r),
                    Err(e) => {
                        warn!("GeoEngine: Failed to load {}: {e}", self.file_name);
                        result.failed.push(e);
                    }
                }
            }
        }
        result
    }
}

/// The loaded world and the queries on it. Immutable and `Send + Sync`.
pub struct GeoEngine {
    set: GeoSet,
    options: GeoEngineOptions,
    obstacles: Arc<dyn DynamicObstacles>,
}

impl std::fmt::Debug for GeoEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GeoEngine")
            .field("regions", &self.set.loaded_count())
            .field("options", &self.options)
            .finish_non_exhaustive()
    }
}

impl GeoEngine {
    /// Wraps an already built region table (tests, tools). Like the Java constructor, an engine
    /// without any region turns `pathfinding` off.
    pub fn new(set: GeoSet, mut options: GeoEngineOptions) -> GeoEngine {
        if set.loaded_count() == 0 && options.pathfinding > 0 {
            options.pathfinding = 0;
            info!("GeoEngine: Pathfinding is disabled.");
        }
        GeoEngine {
            set,
            options,
            obstacles: Arc::new(NoObstacles),
        }
    }

    /// Loads the whole world (Java: the `GeoEngine` constructor), applying the override list.
    ///
    /// Never fails as a whole: a missing folder gives an engine with no regions, and a bad file
    /// leaves its region as a null region; both are logged and listed in the [`LoadReport`].
    ///
    /// The files are parsed on a few short-lived worker threads (startup only; nothing here runs
    /// in the world loop), and the report is assembled in region order, so it does not depend on
    /// thread timing.
    pub fn load(spec: &GeoLoadSpec, options: GeoEngineOptions) -> (GeoEngine, LoadReport) {
        let jobs: Vec<RegionJob> = (TILE_X_MIN..=TILE_X_MAX)
            .flat_map(|x| (TILE_Y_MIN..=TILE_Y_MAX).map(move |y| (x, y)))
            .map(|(x, y)| RegionJob {
                x,
                y,
                file_name: format!("{x}_{y}.l2j"),
                override_dir: spec
                    .override_
                    .as_ref()
                    .filter(|o| o.regions.contains(&(x, y)))
                    .map(|o| o.path.clone()),
            })
            .collect();

        let workers = std::thread::available_parallelism()
            .map_or(1, std::num::NonZero::get)
            .clamp(1, MAX_LOAD_THREADS);
        let next_job = AtomicUsize::new(0);
        let mut results: Vec<(usize, JobResult)> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..workers)
                .map(|_| {
                    scope.spawn(|| {
                        let mut done = Vec::new();
                        loop {
                            let index = next_job.fetch_add(1, Ordering::Relaxed);
                            let Some(job) = jobs.get(index) else { break };
                            done.push((index, job.run(&spec.geodata_path)));
                        }
                        done
                    })
                })
                .collect();
            handles
                .into_iter()
                // A panic in a worker is a bug in the parser: let it surface.
                .flat_map(|h| h.join().unwrap_or_else(|panic| std::panic::resume_unwind(panic)))
                .collect()
        });
        results.sort_by_key(|(index, _)| *index);

        let mut set = GeoSet::new();
        let mut report = LoadReport::default();
        for (index, result) in results {
            let job = &jobs[index];
            report.failed.extend(result.failed);
            if result.override_missing {
                report.override_missing.push((job.x, job.y));
            }
            if let Some(region) = result.region {
                if result.from_override {
                    report.overridden.push((job.x, job.y));
                }
                set.insert(job.x, job.y, region);
                report.loaded += 1;
                debug!("GeoEngine: loaded region {}_{}", job.x, job.y);
            }
        }
        info!("GeoEngine: Loaded {} regions.", report.loaded);
        let pathfinding_before = options.pathfinding;
        let engine = GeoEngine::new(set, options);
        report.pathfinding_disabled = pathfinding_before > 0 && engine.options.pathfinding == 0;
        (engine, report)
    }

    /// Replaces the door / fence hook (default: [`NoObstacles`]).
    pub fn with_obstacles(mut self, obstacles: Arc<dyn DynamicObstacles>) -> Self {
        self.obstacles = obstacles;
        self
    }

    /// Replaces the options (tests; the Java values are static and can change at runtime too).
    pub fn with_options(mut self, options: GeoEngineOptions) -> Self {
        self.options = options;
        self
    }

    pub fn options(&self) -> GeoEngineOptions {
        self.options
    }

    /// The region table.
    pub fn set(&self) -> &GeoSet {
        &self.set
    }

    /// Number of loaded regions.
    pub fn loaded_count(&self) -> usize {
        self.set.loaded_count()
    }

    /// Heap bytes used by all regions (see [`Region::heap_bytes`]).
    pub fn heap_bytes(&self) -> usize {
        (0..GEO_REGIONS_SIDE as u32)
            .flat_map(|x| (0..GEO_REGIONS_SIDE as u32).map(move |y| (x, y)))
            .filter_map(|(x, y)| self.set.region(x, y))
            .map(Region::heap_bytes)
            .sum()
    }

    // ------------------------------------------------------------------ geo-coordinate queries

    /// `GeoEngine.getRegion`: the region of a geo cell, `None` where Java answers `NullRegion`.
    ///
    /// NOTE(java-quirk): the index is `(geoX / 2048) * 32 + (geoY / 2048)` (truncating division, no
    /// range check on Y), so a Y beyond the 32 columns reads the next row. Java throws for an index
    /// outside 0..1024; this returns `None` then.
    pub fn region_at(&self, geo_x: i32, geo_y: i32) -> Option<&Region> {
        let index = (geo_x / REGION_CELLS_SIDE)
            .wrapping_mul(GEO_REGIONS_SIDE as i32)
            .wrapping_add(geo_y / REGION_CELLS_SIDE);
        if !(0..(GEO_REGIONS_SIDE * GEO_REGIONS_SIDE) as i32).contains(&index) {
            return None;
        }
        self.set.region(
            (index / GEO_REGIONS_SIDE as i32) as u32,
            (index % GEO_REGIONS_SIDE as i32) as u32,
        )
    }

    /// `GeoEngine.hasGeoPos`.
    pub fn has_geo_pos(&self, geo_x: i32, geo_y: i32) -> bool {
        self.region_at(geo_x, geo_y).is_some()
    }

    /// `GeoEngine.checkNearestNswe`: all bits of `nswe` open on the cell (nearest layer to `world_z`).
    pub fn check_nearest_nswe(&self, geo_x: i32, geo_y: i32, world_z: i32, nswe: u8) -> bool {
        match self.region_at(geo_x, geo_y) {
            Some(r) => r.check_nearest_nswe(geo_x, geo_y, world_z, nswe),
            None => true,
        }
    }

    /// `GeoEngine.checkNearestNsweAntiCornerCut`: like [`check_nearest_nswe`](Self::check_nearest_nswe),
    /// and for a diagonal step both orthogonal neighbours must allow their part of the move.
    pub fn check_nearest_nswe_anti_corner_cut(&self, geo_x: i32, geo_y: i32, world_z: i32, nswe: u8) -> bool {
        // NullRegion answers true to everything, so a cell without a region never blocks.
        let Some(region) = self.region_at(geo_x, geo_y) else {
            return true;
        };
        let mut can_move = true;
        if (nswe & NSWE_NORTH_EAST) == NSWE_NORTH_EAST {
            can_move = region.check_nearest_nswe(geo_x, geo_y.wrapping_sub(1), world_z, NSWE_EAST)
                && region.check_nearest_nswe(geo_x.wrapping_add(1), geo_y, world_z, NSWE_NORTH);
        }
        if can_move && (nswe & NSWE_NORTH_WEST) == NSWE_NORTH_WEST {
            can_move = region.check_nearest_nswe(geo_x, geo_y.wrapping_sub(1), world_z, NSWE_WEST)
                && region.check_nearest_nswe(geo_x.wrapping_sub(1), geo_y, world_z, NSWE_NORTH);
        }
        if can_move && (nswe & NSWE_SOUTH_EAST) == NSWE_SOUTH_EAST {
            can_move = region.check_nearest_nswe(geo_x, geo_y.wrapping_add(1), world_z, NSWE_EAST)
                && region.check_nearest_nswe(geo_x.wrapping_add(1), geo_y, world_z, NSWE_SOUTH);
        }
        if can_move && (nswe & NSWE_SOUTH_WEST) == NSWE_SOUTH_WEST {
            can_move = region.check_nearest_nswe(geo_x, geo_y.wrapping_add(1), world_z, NSWE_WEST)
                && region.check_nearest_nswe(geo_x.wrapping_sub(1), geo_y, world_z, NSWE_SOUTH);
        }
        can_move && region.check_nearest_nswe(geo_x, geo_y, world_z, nswe)
    }

    /// `GeoEngine.getNearestZ`: `world_z` itself where there is no geodata.
    pub fn nearest_z(&self, geo_x: i32, geo_y: i32, world_z: i32) -> i32 {
        match self.region_at(geo_x, geo_y) {
            Some(r) => r.nearest_z(geo_x, geo_y, world_z),
            None => world_z,
        }
    }

    /// `GeoEngine.getNextLowerZ`.
    pub fn next_lower_z(&self, geo_x: i32, geo_y: i32, world_z: i32) -> i32 {
        match self.region_at(geo_x, geo_y) {
            Some(r) => r.next_lower_z(geo_x, geo_y, world_z),
            None => world_z,
        }
    }

    /// `GeoEngine.getNextHigherZ`.
    pub fn next_higher_z(&self, geo_x: i32, geo_y: i32, world_z: i32) -> i32 {
        match self.region_at(geo_x, geo_y) {
            Some(r) => r.next_higher_z(geo_x, geo_y, world_z),
            None => world_z,
        }
    }

    /// `GeoEngine.isCompletelyBlocked`: no direction open on the cell. Always false while
    /// `PathFinding` is 0 and on cells without geodata.
    pub fn is_completely_blocked(&self, geo_x: i32, geo_y: i32, geo_z: i32) -> bool {
        if self.options.pathfinding < 1 {
            return false;
        }
        match self.region_at(geo_x, geo_y) {
            Some(r) => {
                !r.check_nearest_nswe(geo_x, geo_y, geo_z, NSWE_NORTH)
                    && !r.check_nearest_nswe(geo_x, geo_y, geo_z, NSWE_SOUTH)
                    && !r.check_nearest_nswe(geo_x, geo_y, geo_z, NSWE_EAST)
                    && !r.check_nearest_nswe(geo_x, geo_y, geo_z, NSWE_WEST)
            }
            None => false,
        }
    }

    // ------------------------------------------------------------------ world-coordinate queries

    /// `GeoEngine.hasGeo`: is there geodata under a world position?
    pub fn has_geo(&self, x: i32, y: i32) -> bool {
        self.has_geo_pos(geo_x(x), geo_y(y))
    }

    /// `GeoEngine.getHeight`: the ground Z under a world position, nearest to `z`.
    pub fn height(&self, x: i32, y: i32, z: i32) -> i32 {
        self.nearest_z(geo_x(x), geo_y(y), z)
    }

    /// `GeoEngine.getSpawnHeight`: the layer at or below `z + 20`, if it is within 100 of `z`,
    /// else `z`. `z` itself where there is no geodata.
    pub fn spawn_height(&self, x: i32, y: i32, z: i32) -> i32 {
        let gx = geo_x(x);
        let gy = geo_y(y);
        if !self.has_geo_pos(gx, gy) {
            return z;
        }
        let next_lower = self.next_lower_z(gx, gy, z.wrapping_add(SPAWN_HEIGHT_OFFSET));
        if next_lower.wrapping_sub(z).wrapping_abs() <= SPAWN_Z_DELTA_LIMIT {
            next_lower
        } else {
            z
        }
    }

    // ------------------------------------------------------------------ line of sight

    /// `canSeeTarget(x, y, z, instanceId, targetX, targetY, targetZ, targetInstanceId)`: no line of
    /// sight between different instances, else [`can_see_target_in_instance`](Self::can_see_target_in_instance).
    pub fn can_see_target_between_instances(
        &self,
        from: Location,
        instance_id: i32,
        to: Location,
        target_instance_id: i32,
    ) -> bool {
        instance_id == target_instance_id && self.can_see_target_in_instance(from, to, instance_id)
    }

    /// `canSeeTarget(x, y, z, targetX, targetY, targetZ, instanceId)`: doors and fences first (a
    /// door blocks sight when it is on the segment, checked from both faces), then the geodata.
    pub fn can_see_target_in_instance(&self, from: Location, to: Location, instance_id: i32) -> bool {
        if self.obstacles.doors_between(from, to, instance_id, true) {
            return false;
        }
        if self.obstacles.fence_between(from, to, instance_id) {
            return false;
        }
        self.can_see_target(from, to)
    }

    /// `getLosGeoZ`: the Z of the next cell on the line, given the step `nswe` (a single direction).
    fn los_geo_z(
        &self,
        previous_x: i32,
        previous_y: i32,
        previous_geo_z: i32,
        current_x: i32,
        current_y: i32,
        nswe: u8,
    ) -> i32 {
        // Java throws a RuntimeException when N+S or W+E are both set; every caller passes a
        // single direction or a computeNswe result, so it cannot happen here.
        if self.check_nearest_nswe_anti_corner_cut(previous_x, previous_y, previous_geo_z, nswe) {
            self.nearest_z(current_x, current_y, previous_geo_z)
        } else {
            self.next_higher_z(current_x, current_y, previous_geo_z)
        }
    }

    /// `canSeeTarget(x, y, z, targetX, targetY, targetZ)`: line of sight over the geodata only (no
    /// doors, no fences).
    ///
    /// The line is walked in 3D from the higher end to the lower end (the ends are swapped if the
    /// target is higher); a cell blocks when its floor is more than 48 units above the line (the
    /// first two cells are measured from the start height instead of the line).
    pub fn can_see_target(&self, from: Location, to: Location) -> bool {
        let mut geo_x_from = geo_x(from.x);
        let mut geo_y_from = geo_y(from.y);
        let mut target_geo_x = geo_x(to.x);
        let mut target_geo_y = geo_y(to.y);

        let mut nearest_from_z = self.nearest_z(geo_x_from, geo_y_from, from.z);
        let mut nearest_to_z = self.nearest_z(target_geo_x, target_geo_y, to.z);

        // Fastpath for same coordinates.
        if geo_x_from == target_geo_x && geo_y_from == target_geo_y {
            return !self.has_geo_pos(target_geo_x, target_geo_y) || nearest_from_z == nearest_to_z;
        }

        if nearest_to_z > nearest_from_z {
            std::mem::swap(&mut nearest_to_z, &mut nearest_from_z);
            std::mem::swap(&mut target_geo_x, &mut geo_x_from);
            std::mem::swap(&mut target_geo_y, &mut geo_y_from);
        }

        let mut points = GridLineIterator3D::new(
            geo_x_from,
            geo_y_from,
            nearest_from_z,
            target_geo_x,
            target_geo_y,
            nearest_to_z,
        );

        // First point is guaranteed to be available, skip it, we can always see our own position.
        points.advance();
        let mut previous_x = points.x();
        let mut previous_y = points.y();
        let mut previous_geo_z = points.z();
        let mut point_index = 0;
        while points.advance() {
            let current_x = points.x();
            let current_y = points.y();
            if current_x == previous_x && current_y == previous_y {
                continue;
            }

            let bee_current_z = points.z();
            let mut current_geo_z = previous_geo_z;

            // Check if the position has geodata.
            if self.has_geo_pos(current_x, current_y) {
                let nswe = compute_nswe(previous_x, previous_y, current_x, current_y);
                current_geo_z =
                    self.los_geo_z(previous_x, previous_y, previous_geo_z, current_x, current_y, nswe);
                let max_height = if point_index < ELEVATED_SEE_OVER_DISTANCE {
                    nearest_from_z.wrapping_add(MAX_SEE_OVER_HEIGHT)
                } else {
                    bee_current_z.wrapping_add(MAX_SEE_OVER_HEIGHT)
                };
                let mut can_see_through = false;
                if current_geo_z <= max_height {
                    can_see_through = match diagonal_parts(nswe) {
                        // Straight step: nothing more to check.
                        None => true,
                        // Diagonal step: both orthogonal neighbours must be low enough as well.
                        Some((vertical_dy, horizontal_dx, vertical_nswe, horizontal_nswe)) => {
                            // The vertical neighbour (previous_y +- 1) is entered moving along X
                            // (`horizontal_part`), the horizontal one (previous_x +- 1) moving along Y.
                            let vertical_y = previous_y.wrapping_add(vertical_dy);
                            let horizontal_x = previous_x.wrapping_add(horizontal_dx);
                            let vertical_geo_z = self.los_geo_z(
                                previous_x,
                                previous_y,
                                previous_geo_z,
                                previous_x,
                                vertical_y,
                                horizontal_nswe,
                            );
                            let horizontal_geo_z = self.los_geo_z(
                                previous_x,
                                previous_y,
                                previous_geo_z,
                                horizontal_x,
                                previous_y,
                                vertical_nswe,
                            );
                            vertical_geo_z <= max_height
                                && horizontal_geo_z <= max_height
                                && vertical_geo_z <= self.nearest_z(previous_x, vertical_y, bee_current_z)
                                && horizontal_geo_z <= self.nearest_z(horizontal_x, previous_y, bee_current_z)
                        }
                    };
                }

                if !can_see_through {
                    return false;
                }
            }

            previous_x = current_x;
            previous_y = current_y;
            previous_geo_z = current_geo_z;
            point_index += 1;
        }

        true
    }

    // ------------------------------------------------------------------ movement

    /// `hasNeighbourLayerNear`: does a neighbour (E, W, S, N) that has geodata have a layer within
    /// `tolerance` of `world_z`? Detects a one-cell gap in a continuous surface.
    fn has_neighbour_layer_near(&self, geo_x: i32, geo_y: i32, world_z: i32, tolerance: i32) -> bool {
        const DX: [i32; 4] = [1, -1, 0, 0];
        const DY: [i32; 4] = [0, 0, 1, -1];
        for i in 0..4 {
            let nx = geo_x.wrapping_add(DX[i]);
            let ny = geo_y.wrapping_add(DY[i]);
            if self.has_geo_pos(nx, ny)
                && self
                    .nearest_z(nx, ny, world_z)
                    .wrapping_sub(world_z)
                    .wrapping_abs()
                    <= tolerance
            {
                return true;
            }
        }
        false
    }

    /// `getValidLocation(x, y, z, targetX, targetY, targetZ, instanceId)`: walks the line cell by
    /// cell and returns the last position before an obstacle, or the target (with its ground Z) if
    /// the way is free. A closed door or fence on the segment returns the start on the ground.
    pub fn valid_location(&self, from: Location, to: Location, instance_id: i32) -> Location {
        let start_geo_x = geo_x(from.x);
        let start_geo_y = geo_y(from.y);
        let nearest_from_z = self.nearest_z(start_geo_x, start_geo_y, from.z);
        let target_geo_x = geo_x(to.x);
        let target_geo_y = geo_y(to.y);
        let nearest_to_z = self.nearest_z(target_geo_x, target_geo_y, to.z);

        // Door checks.
        if self.obstacles.doors_between(
            Location::new(from.x, from.y, nearest_from_z),
            Location::new(to.x, to.y, nearest_to_z),
            instance_id,
            false,
        ) {
            return Location::new(from.x, from.y, self.height(from.x, from.y, nearest_from_z));
        }

        // Fence checks.
        if self.obstacles.fence_between(
            Location::new(from.x, from.y, nearest_from_z),
            Location::new(to.x, to.y, nearest_to_z),
            instance_id,
        ) {
            return Location::new(from.x, from.y, self.height(from.x, from.y, nearest_from_z));
        }

        let mut points = GridLineIterator2D::new(start_geo_x, start_geo_y, target_geo_x, target_geo_y);

        // First point is guaranteed to be available.
        points.advance();
        let mut previous_x = points.x();
        let mut previous_y = points.y();
        let mut previous_z = nearest_from_z;

        while points.advance() {
            let current_x = points.x();
            let current_y = points.y();
            let raw_current_z = self.nearest_z(current_x, current_y, previous_z);
            let current_z = if raw_current_z.wrapping_sub(previous_z) > HEIGHT_INCREASE_LIMIT {
                // Sudden height increase. Single-cell layer-gap fallback: if a neighbour of this
                // cell has a layer near previous_z the surface is continuous through it, so step
                // over the gap as if the cell had a layer at previous_z.
                if !self.has_neighbour_layer_near(current_x, current_y, previous_z, PATH_CONTINUITY_TOLERANCE)
                {
                    // Genuine wall, return previous location.
                    return Location::new(world_x(previous_x), world_y(previous_y), previous_z);
                }
                previous_z
            } else {
                raw_current_z
            };

            if self.has_geo_pos(previous_x, previous_y) {
                if self.is_completely_blocked(current_x, current_y, current_z) {
                    // Can't move, return previous location.
                    return Location::new(world_x(previous_x), world_y(previous_y), previous_z);
                }

                if !self.check_nearest_nswe_anti_corner_cut(
                    previous_x,
                    previous_y,
                    previous_z,
                    compute_nswe(previous_x, previous_y, current_x, current_y),
                ) {
                    // Can't move, return previous location.
                    return Location::new(world_x(previous_x), world_y(previous_y), previous_z);
                }
            }

            previous_x = current_x;
            previous_y = current_y;
            previous_z = current_z;
        }

        if self.has_geo_pos(previous_x, previous_y) && previous_z != nearest_to_z {
            Location::new(from.x, from.y, nearest_from_z)
        } else {
            Location::new(to.x, to.y, nearest_to_z)
        }
    }

    /// `canMoveToTarget(fromX, fromY, fromZ, toX, toY, toZ, instanceId)`: can a creature walk the
    /// straight line? Same walk as [`valid_location`](Self::valid_location), but answers yes / no
    /// and, with `AvoidObstructedPathNodes`, also checks the plain NSWE of every step.
    pub fn can_move_to_target(&self, from: Location, to: Location, instance_id: i32) -> bool {
        let start_geo_x = geo_x(from.x);
        let start_geo_y = geo_y(from.y);
        let nearest_from_z = self.nearest_z(start_geo_x, start_geo_y, from.z);
        let target_geo_x = geo_x(to.x);
        let target_geo_y = geo_y(to.y);
        let nearest_to_z = self.nearest_z(target_geo_x, target_geo_y, to.z);

        // Door checks.
        if self.obstacles.doors_between(
            Location::new(from.x, from.y, nearest_from_z),
            Location::new(to.x, to.y, nearest_to_z),
            instance_id,
            false,
        ) {
            return false;
        }

        // Fence checks.
        if self.obstacles.fence_between(
            Location::new(from.x, from.y, nearest_from_z),
            Location::new(to.x, to.y, nearest_to_z),
            instance_id,
        ) {
            return false;
        }

        let mut points = GridLineIterator2D::new(start_geo_x, start_geo_y, target_geo_x, target_geo_y);

        // First point is guaranteed to be available.
        points.advance();
        let mut previous_x = points.x();
        let mut previous_y = points.y();
        let mut previous_z = nearest_from_z;

        while points.advance() {
            let current_x = points.x();
            let current_y = points.y();
            let raw_current_z = self.nearest_z(current_x, current_y, previous_z);
            let current_z = if raw_current_z.wrapping_sub(previous_z) > HEIGHT_INCREASE_LIMIT {
                // Single-cell layer-gap fallback (mirrors valid_location).
                if !self.has_neighbour_layer_near(current_x, current_y, previous_z, PATH_CONTINUITY_TOLERANCE)
                {
                    return false;
                }
                previous_z
            } else {
                raw_current_z
            };

            if self.has_geo_pos(previous_x, previous_y) {
                let nswe = compute_nswe(previous_x, previous_y, current_x, current_y);
                if self.options.avoid_obstructed_path_nodes
                    && !self.check_nearest_nswe(previous_x, previous_y, previous_z, nswe)
                {
                    return false;
                }

                if !self.check_nearest_nswe_anti_corner_cut(previous_x, previous_y, previous_z, nswe) {
                    return false;
                }
            }

            previous_x = current_x;
            previous_y = current_y;
            previous_z = current_z;
        }

        !self.has_geo_pos(previous_x, previous_y) || previous_z == nearest_to_z
    }
}

/// For a diagonal step: `(dy of the vertical neighbour, dx of the horizontal neighbour, the
/// vertical NSWE part, the horizontal NSWE part)`; `None` for a straight step.
///
/// The Java code spells the four cases out (`NSWE_NORTH_EAST` first, then NORTH_WEST, SOUTH_EAST,
/// SOUTH_WEST); this table is the same four branches. North is `y - 1`, east is `x + 1`.
fn diagonal_parts(nswe: u8) -> Option<(i32, i32, u8, u8)> {
    if (nswe & NSWE_NORTH_EAST) == NSWE_NORTH_EAST {
        Some((-1, 1, NSWE_NORTH, NSWE_EAST))
    } else if (nswe & NSWE_NORTH_WEST) == NSWE_NORTH_WEST {
        Some((-1, -1, NSWE_NORTH, NSWE_WEST))
    } else if (nswe & NSWE_SOUTH_EAST) == NSWE_SOUTH_EAST {
        Some((1, 1, NSWE_SOUTH, NSWE_EAST))
    } else if (nswe & NSWE_SOUTH_WEST) == NSWE_SOUTH_WEST {
        Some((1, -1, NSWE_SOUTH, NSWE_WEST))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::sync::Mutex;

    use super::*;
    use crate::cell::NSWE_ALL;
    use crate::region::REGION_BLOCKS;

    /// World position of the middle of a geo cell.
    fn at(geo_x: i32, geo_y: i32, z: i32) -> Location {
        Location::new(world_x(geo_x), world_y(geo_y), z)
    }

    /// Cell word: height must be a multiple of 8 (the file format stores `height * 2` in bits 4..15).
    fn word(height: i32, nswe: u8) -> u16 {
        (((height * 2) as i16) as u16 & 0xFFF0) | u16::from(nswe)
    }

    fn flat_bytes(base: i16) -> Vec<u8> {
        let mut out = Vec::with_capacity(REGION_BLOCKS * 3);
        for _ in 0..REGION_BLOCKS {
            out.push(0);
            out.extend_from_slice(&base.to_le_bytes());
        }
        out
    }

    /// A flat region at `base` with complex blocks at the given block indexes (`bx * 256 + by`).
    fn region_bytes(base: i16, blocks: &[(usize, [u16; 64])]) -> Vec<u8> {
        let mut out = Vec::new();
        for block in 0..REGION_BLOCKS {
            if let Some((_, cells)) = blocks.iter().find(|(i, _)| *i == block) {
                out.push(1);
                for c in cells {
                    out.extend_from_slice(&c.to_le_bytes());
                }
            } else {
                out.push(0);
                out.extend_from_slice(&base.to_le_bytes());
            }
        }
        out
    }

    /// Region 20_18 starts at geo (40960, 36864). Block (2, 3) covers x 40976..40984, y 36888..36896.
    const RX: i32 = 20 * 2048;
    const RY: i32 = 18 * 2048;
    const BLOCK_2_3: usize = 2 * 256 + 3;

    fn engine_with(bytes: &[u8], options: GeoEngineOptions) -> GeoEngine {
        let mut set = GeoSet::new();
        assert!(set.insert(20, 18, Region::parse(bytes).unwrap()));
        GeoEngine::new(set, options)
    }

    /// Block (2, 3) as a complex block: all cells at height 0 open in all directions, except those
    /// `f` changes. Index is `cx * 8 + cy`.
    fn block_with(f: impl Fn(usize, usize) -> u16) -> [u16; 64] {
        let mut cells = [word(0, NSWE_ALL); 64];
        for cx in 0..8 {
            for cy in 0..8 {
                cells[cx * 8 + cy] = f(cx, cy);
            }
        }
        cells
    }

    #[test]
    fn engine_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<GeoEngine>();
        assert_send_sync::<Arc<GeoEngine>>();
    }

    #[test]
    fn no_geodata_behaves_like_null_region() {
        let engine = GeoEngine::new(GeoSet::new(), GeoEngineOptions::shipped());
        // No region loaded: pathfinding is switched off like in Java.
        assert_eq!(engine.options().pathfinding, 0);
        assert!(!engine.has_geo(0, 0));
        assert_eq!(engine.height(0, 0, 123), 123);
        assert_eq!(engine.spawn_height(0, 0, 77), 77);
        let a = Location::new(0, 0, 10);
        let b = Location::new(900, 400, 500);
        assert!(engine.can_see_target(a, b));
        assert!(engine.can_move_to_target(a, b, 0));
        assert_eq!(engine.valid_location(a, b, 0), b);
        assert!(!engine.is_completely_blocked(40960, 36864, 0));
        assert!(engine.check_nearest_nswe_anti_corner_cut(40960, 36864, 0, NSWE_NORTH_EAST));
    }

    #[test]
    fn flat_region_is_open() {
        let engine = engine_with(&flat_bytes(-8), GeoEngineOptions::shipped());
        assert_eq!(engine.options().pathfinding, 2);
        assert!(engine.has_geo(0, 0));
        assert_eq!(engine.height(0, 0, 5000), -8);
        let a = at(RX + 100, RY + 100, -8);
        let b = at(RX + 160, RY + 70, -8);
        assert!(engine.can_see_target(a, b));
        assert!(engine.can_move_to_target(a, b, 0));
        assert_eq!(engine.valid_location(a, b, 0), Location::new(b.x, b.y, -8));
        // Same cell, same height: visible; same cell, other layer height: not.
        assert!(engine.can_see_target(Location::new(0, 0, -8), Location::new(3, 3, -8)));
        // The target's Z is snapped to the ground of its cell.
        assert_eq!(engine.valid_location(a, Location::new(b.x, b.y, 9999), 0).z, -8);
    }

    #[test]
    fn high_wall_blocks_sight_and_walking() {
        // A column of cells (cx = 4) 400 units high across the whole block: a wall.
        let wall = block_with(|cx, _| {
            if cx == 4 {
                word(400, NSWE_ALL)
            } else {
                word(0, NSWE_ALL)
            }
        });
        let engine = engine_with(
            &region_bytes(0, &[(BLOCK_2_3, wall)]),
            GeoEngineOptions::shipped(),
        );
        let base_x = RX + 2 * 8; // first cell of block (2, 3)
        let y = RY + 3 * 8 + 3;
        let from = at(base_x + 1, y, 0);
        let to = at(base_x + 6, y, 0);
        assert!(!engine.can_see_target(from, to));
        // NOTE(java-quirk): the "sudden height increase" check of the walk is defeated by the
        // single-cell gap fallback whenever the step is orthogonal: the cell we come from is a
        // neighbour of the wall cell and has a layer near our Z. Only NSWE (or a diagonal step)
        // stops a walk, which is why real walls carry NSWE bits.
        assert!(engine.can_move_to_target(from, to, 0));
        assert_eq!(engine.valid_location(from, to, 0), Location::new(to.x, to.y, 0));
        // Both sides of the wall are fine on their own.
        assert!(engine.can_see_target(from, at(base_x + 3, y, 0)));
        assert!(engine.can_move_to_target(from, at(base_x + 3, y, 0), 0));
        // The start Z is snapped to the ground of its cell first, so a high start Z changes nothing.
        assert!(!engine.can_see_target(at(base_x + 1, y, 1000), at(base_x + 6, y, 0)));
        // Standing on top of the wall (target Z above the start) swaps the ends: the line runs from the
        // wall top down to the floor, and the floor cells never rise above it.
        assert!(engine.can_see_target(at(base_x + 6, y, 0), at(base_x + 4, y, 400)));
    }

    #[test]
    fn nswe_wall_stops_walking_but_not_sight() {
        // No EAST out of cx = 3 and no WEST out of cx = 4: a thin wall between them at height 0.
        let wall = block_with(|cx, _| match cx {
            3 => word(0, NSWE_ALL & !NSWE_EAST),
            4 => word(0, NSWE_ALL & !NSWE_WEST),
            _ => word(0, NSWE_ALL),
        });
        let engine = engine_with(
            &region_bytes(0, &[(BLOCK_2_3, wall)]),
            GeoEngineOptions::shipped(),
        );
        let base_x = RX + 2 * 8;
        let y = RY + 3 * 8 + 3;
        let from = at(base_x + 1, y, 0);
        let to = at(base_x + 6, y, 0);
        assert!(!engine.can_move_to_target(from, to, 0));
        assert_eq!(
            engine.valid_location(from, to, 0),
            Location::new(world_x(base_x + 3), world_y(y), 0)
        );
        // Line of sight only looks at heights.
        assert!(engine.can_see_target(from, to));
        // With AvoidObstructedPathNodes off the plain NSWE check is skipped, but the anti-corner-cut
        // check still looks at the same bit for a straight step: still blocked.
        let lax = engine_with(
            &region_bytes(
                0,
                &[(
                    BLOCK_2_3,
                    block_with(|cx, _| {
                        if cx == 3 {
                            word(0, NSWE_ALL & !NSWE_EAST)
                        } else {
                            word(0, NSWE_ALL)
                        }
                    }),
                )],
            ),
            GeoEngineOptions {
                pathfinding: 2,
                avoid_obstructed_path_nodes: false,
            },
        );
        assert!(!lax.can_move_to_target(from, to, 0));
        // A cell closed on all sides blocks a walk into it only while pathfinding is on.
        let closed = block_with(|cx, cy| {
            if (cx, cy) == (4, 3) {
                word(0, 0)
            } else {
                word(0, NSWE_ALL)
            }
        });
        let bytes = region_bytes(0, &[(BLOCK_2_3, closed)]);
        let on = engine_with(&bytes, GeoEngineOptions::shipped());
        let off = engine_with(&bytes, GeoEngineOptions::default());
        assert!(on.is_completely_blocked(base_x + 4, y, 0));
        assert!(!off.is_completely_blocked(base_x + 4, y, 0));
        assert_eq!(
            on.valid_location(from, to, 0),
            Location::new(world_x(base_x + 3), world_y(y), 0)
        );
    }

    #[test]
    fn region_lookup_follows_java_index_formula() {
        let mut set = GeoSet::new();
        set.insert(21, 10, Region::parse(&flat_bytes(0)).unwrap());
        set.insert(22, 10, Region::parse(&flat_bytes(0)).unwrap());
        let engine = GeoEngine::new(set, GeoEngineOptions::default());
        assert!(engine.has_geo_pos(21 * 2048, 10 * 2048));
        assert!(engine.has_geo_pos(21 * 2048 + 2047, 10 * 2048 + 2047));
        assert!(!engine.has_geo_pos(21 * 2048, 11 * 2048));
        // NOTE(java-quirk): a Y of 42 regions reads index 21 * 32 + 42 = 22 * 32 + 10, i.e. region 22_10.
        assert!(engine.has_geo_pos(21 * 2048, 42 * 2048));
        // Negative cells just left of the table (-1 / 2048 == 0) read region 0_0: none loaded.
        assert!(!engine.has_geo_pos(-1, -1));
        // Index outside 0..1024: Java throws, this answers "no geodata".
        assert!(!engine.has_geo_pos(40 * 2048, 0));
        assert!(!engine.has_geo_pos(-5000, 0));
        assert!(!engine.has_geo_pos(i32::MAX, i32::MAX));
    }

    #[test]
    fn spawn_height_rules() {
        let engine = engine_with(&flat_bytes(100), GeoEngineOptions::default());
        // Layer 100, z + 20 >= 100: the layer (within 100 of z).
        assert_eq!(engine.spawn_height(0, 0, 90), 100);
        assert_eq!(engine.spawn_height(0, 0, 200), 100);
        // Too far below the layer: unchanged.
        assert_eq!(engine.spawn_height(0, 0, 400), 400);
        // z + 20 below the layer: no layer at or below, next_lower returns z + 20, within limit.
        assert_eq!(engine.spawn_height(0, 0, 50), 70);
    }

    /// Records every call and answers with fixed results.
    #[derive(Default)]
    struct Recorder {
        calls: Mutex<Vec<String>>,
        door: bool,
        fence: bool,
    }

    impl DynamicObstacles for Recorder {
        fn doors_between(&self, from: Location, to: Location, instance: i32, double: bool) -> bool {
            self.calls.lock().unwrap().push(format!(
                "door {} {} {} {} {} {} {instance} {double}",
                from.x, from.y, from.z, to.x, to.y, to.z
            ));
            self.door
        }

        fn fence_between(&self, from: Location, to: Location, instance: i32) -> bool {
            self.calls.lock().unwrap().push(format!(
                "fence {} {} {} {} {} {} {instance}",
                from.x, from.y, from.z, to.x, to.y, to.z
            ));
            self.fence
        }
    }

    #[test]
    fn obstacle_hook_is_called_where_java_calls_door_and_fence_data() {
        let bytes = flat_bytes(40);
        let from = Location::new(-100, 200, 7);
        let to = Location::new(300, 500, 9);

        let rec = Arc::new(Recorder::default());
        let engine = engine_with(&bytes, GeoEngineOptions::default()).with_obstacles(rec.clone());
        // Doors and fences are asked first (both answer no), then the flat geodata lets sight pass.
        assert!(engine.can_see_target_in_instance(from, to, 5));
        let calls = std::mem::take(&mut *rec.calls.lock().unwrap());
        assert_eq!(
            calls,
            vec!["door -100 200 7 300 500 9 5 true", "fence -100 200 7 300 500 9 5"]
        );

        // Movement checks use the ground Z of both ends and no double face check.
        let _ = engine.can_move_to_target(from, to, 3);
        let calls = std::mem::take(&mut *rec.calls.lock().unwrap());
        let ground = engine.height(from.x, from.y, from.z);
        let target_ground = engine.height(to.x, to.y, to.z);
        assert_eq!(
            calls,
            vec![
                format!("door -100 200 {ground} 300 500 {target_ground} 3 false"),
                format!("fence -100 200 {ground} 300 500 {target_ground} 3"),
            ]
        );
        let _ = engine.valid_location(from, to, 3);
        assert_eq!(std::mem::take(&mut *rec.calls.lock().unwrap()), calls);

        // A door answers true: sight and movement fail, valid_location returns the start on the ground.
        let door = Arc::new(Recorder {
            door: true,
            ..Recorder::default()
        });
        let engine = engine_with(&bytes, GeoEngineOptions::default()).with_obstacles(door.clone());
        assert!(!engine.can_see_target_in_instance(from, to, 0));
        assert!(!engine.can_move_to_target(from, to, 0));
        assert_eq!(
            engine.valid_location(from, to, 0),
            Location::new(from.x, from.y, ground)
        );
        // The door check short-circuits the fence check.
        assert!(door.calls.lock().unwrap().iter().all(|c| c.starts_with("door")));

        // A fence alone behaves the same.
        let fence = Arc::new(Recorder {
            fence: true,
            ..Recorder::default()
        });
        let engine = engine_with(&bytes, GeoEngineOptions::default()).with_obstacles(fence);
        assert!(!engine.can_see_target_in_instance(from, to, 0));
        assert!(!engine.can_move_to_target(from, to, 0));
        // Plain geodata sight ignores the hook.
        assert!(engine.can_see_target(from, to));
    }

    #[test]
    fn different_instances_never_see_each_other() {
        let engine = GeoEngine::new(GeoSet::new(), GeoEngineOptions::default());
        let a = Location::new(0, 0, 0);
        let b = Location::new(10, 10, 0);
        assert!(engine.can_see_target_between_instances(a, 0, b, 0));
        assert!(engine.can_see_target_between_instances(a, 7, b, 7));
        assert!(!engine.can_see_target_between_instances(a, 0, b, 7));
    }

    #[test]
    fn override_regions_are_parsed_leniently() {
        let (regions, ignored) = GeoOverride::parse_regions(
            " 12_24 , 17_10,12_24,abc,12-24,99_99,12_,_5,10_10,27_26,-1_10,,26_26 ,",
        );
        assert_eq!(regions, vec![(12, 24), (17, 10), (26, 26)]);
        assert_eq!(
            ignored,
            vec!["abc", "12-24", "99_99", "12_", "_5", "10_10", "27_26", "-1_10"]
        );
        assert_eq!(GeoOverride::parse_regions(""), (vec![], vec![]));
    }

    #[test]
    fn override_from_ini() {
        let ini = IniFile::parse(
            "GeoOverride.ini",
            "GeoDataOverridePath = alt/geo\nGeoDataOverrideRegions = 20_18, bad\n",
        )
        .unwrap();
        let over = GeoOverride::from_ini(&ini, Path::new("root")).unwrap();
        assert_eq!(over.path, Path::new("root").join("alt/geo"));
        assert_eq!(over.regions, vec![(20, 18)]);
        assert_eq!(over.ignored, vec!["bad"]);

        // Default: nothing configured, nothing overridden.
        assert_eq!(
            GeoOverride::from_ini(&IniFile::empty("GeoOverride.ini"), Path::new("root")),
            None
        );
        let no_regions = IniFile::parse(
            "x",
            "GeoDataOverridePath = alt\nGeoDataOverrideRegions = nonsense\n",
        )
        .unwrap();
        assert_eq!(GeoOverride::from_ini(&no_regions, Path::new("root")), None);
        let no_path = IniFile::parse("x", "GeoDataOverrideRegions = 20_18\n").unwrap();
        assert_eq!(GeoOverride::from_ini(&no_path, Path::new("root")), None);
        // An absolute path is not joined.
        let abs_dir = std::env::temp_dir();
        let text = format!(
            "GeoDataOverridePath = {}\nGeoDataOverrideRegions = 20_18\n",
            abs_dir.display().to_string().replace('\\', "/")
        );
        let abs = IniFile::parse("x", &text).unwrap();
        assert_eq!(
            GeoOverride::from_ini(&abs, Path::new("root")).unwrap().path,
            abs_dir
        );
    }

    fn write(dir: &Path, name: &str, bytes: &[u8]) {
        std::fs::write(dir.join(name), bytes).unwrap();
    }

    #[test]
    fn load_mixes_two_folders_by_region_list() {
        let main = tempfile::tempdir().unwrap();
        let alt = tempfile::tempdir().unwrap();
        write(main.path(), "20_18.l2j", &flat_bytes(8));
        write(main.path(), "20_19.l2j", &flat_bytes(16));
        write(main.path(), "11_10.l2j", &flat_bytes(24));
        write(main.path(), "not_a_region.l2j", b"junk");
        write(main.path(), "99_99.l2j", b"junk");
        // The override has 20_18 and an unrelated 21_18 that is not listed; 20_19 is listed but absent;
        // 11_10 is listed and its override file is corrupt.
        write(alt.path(), "20_18.l2j", &flat_bytes(80));
        write(alt.path(), "21_18.l2j", &flat_bytes(88));
        write(alt.path(), "11_10.l2j", b"corrupt");

        let spec = GeoLoadSpec::new(main.path())
            .with_override(Some(GeoOverride::new(alt.path(), "20_18,20_19,11_10,bogus")));
        let (engine, report) = GeoEngine::load(&spec, GeoEngineOptions::shipped());
        assert_eq!(report.loaded, 3);
        assert_eq!(report.overridden, vec![(20, 18)]);
        assert_eq!(report.override_missing, vec![(11, 10), (20, 19)]);
        assert_eq!(report.failed.len(), 1);
        assert!(!report.pathfinding_disabled);
        assert_eq!(engine.loaded_count(), 3);
        // Override wins for 20_18; the others come from the main folder; unlisted override files are ignored.
        assert_eq!(engine.height(0, 0, 0), 80);
        assert_eq!(engine.nearest_z(20 * 2048, 19 * 2048, 0), 16);
        assert_eq!(engine.nearest_z(11 * 2048, 10 * 2048, 0), 24);
        assert!(!engine.has_geo_pos(21 * 2048, 18 * 2048));
        assert!(engine.heap_bytes() > 0);
    }

    #[test]
    fn load_without_override_and_with_bad_or_missing_files() {
        let main = tempfile::tempdir().unwrap();
        write(main.path(), "20_18.l2j", &flat_bytes(8));
        write(main.path(), "20_19.l2j", b"corrupt");
        let (engine, report) = GeoEngine::load(&GeoLoadSpec::new(main.path()), GeoEngineOptions::shipped());
        assert_eq!(report.loaded, 1);
        assert_eq!(report.failed.len(), 1);
        assert!(report.overridden.is_empty());
        assert!(!engine.has_geo_pos(20 * 2048, 19 * 2048));
        assert_eq!(engine.options().pathfinding, 2);

        // Nothing loadable: pathfinding goes off.
        let empty = tempfile::tempdir().unwrap();
        let (engine, report) = GeoEngine::load(&GeoLoadSpec::new(empty.path()), GeoEngineOptions::shipped());
        assert_eq!(report.loaded, 0);
        assert!(report.pathfinding_disabled);
        assert_eq!(engine.options().pathfinding, 0);
        let (engine, report) = GeoEngine::load(
            &GeoLoadSpec::new(empty.path().join("missing")),
            GeoEngineOptions::default(),
        );
        assert_eq!(report.loaded, 0);
        assert!(!report.pathfinding_disabled);
        assert_eq!(engine.loaded_count(), 0);
    }
}
