//! Geodata loading and the path worker pool of the game server.
// Java: org/l2jmobius/gameserver/GameServer.java (`GeoEngine.getInstance()`, section "Geodata")
// Java: org/l2jmobius/gameserver/config/GeoEngineConfig.java (`GeoDataPath`, `PathFinding`)
// Java: org/l2jmobius/gameserver/geoengine/GeoEngine.java (constructor: what is logged)
//
// Two steps, because the world thread needs the engine before it starts and the pool needs the
// world's command channel, which exists only after:
//   1. [`load_engine`]   blocking; run it on `spawn_blocking`. Never fails: Java runs without
//                        geodata too (every region is a null region), so a missing folder or a
//                        broken file is a warning.
//   2. [`Geo::start`]    starts the worker threads (D-14) when `PathFinding` is above 0. A
//                        finished search becomes `WorldCommand::PathFound` on the world channel.
//
// Rust-only settings live in `config/GeoOverride.ini` (D-26, D-36); `GeoEngine.ini` is read as
// Java reads it and never extended:
//
//   GeoDataOverridePath    = <folder>     second geodata set, relative to `DatapackRoot`
//   GeoDataOverrideRegions = 20_18,21_18  regions (X_Y) taken from that folder
//   PathWorkerThreads      = 0            path worker threads; 0 or less = CPUs / 2, at least 1
//
// The file is optional. A file that cannot be read is logged and treated as absent.

use std::path::{Path, PathBuf};
use std::sync::{Arc, PoisonError, RwLock};

use l2_config::game::GeoEngineConfig;
use l2_config::{GameConfig, IniFile};
use l2_geo::{
    GeoEngine, GeoEngineOptions, GeoLoadSpec, GeoOverride, GeoSet, PathOptions, PathPool, PathPoolConfig,
    PathPoolError, PathRequest, PathResult, PathSink, PoolStats,
};
use l2_world::WorldCommand;
use tracing::{debug, info, warn};

use crate::state::WorldHandle;

/// Rust-only settings file in the config directory (D-36).
pub const GEO_OVERRIDE_FILE: &str = "GeoOverride.ini";
/// Key of `GeoEngine.ini`: the geodata folder, relative to `DatapackRoot`.
pub const GEODATA_PATH_KEY: &str = "GeoDataPath";
/// Java default of `GeoDataPath`.
pub const GEODATA_PATH_DEFAULT: &str = "geodata";
/// Key of `GeoEngine.ini`: where `//geosave` writes regions, relative to `DatapackRoot`.
pub const GEOEDIT_PATH_KEY: &str = "GeoEditPath";
/// Java default of `GeoEditPath`.
pub const GEOEDIT_PATH_DEFAULT: &str = "saves";
/// Key of `GeoOverride.ini`: number of path worker threads.
pub const PATH_WORKER_THREADS_KEY: &str = "PathWorkerThreads";
/// Upper bound for `PathWorkerThreads`: more threads than this only compete for the same buffers.
pub const MAX_PATH_WORKER_THREADS: usize = 64;

/// Everything the geodata startup reads from the configuration.
#[derive(Debug, Clone)]
pub struct GeoSettings {
    /// `GeoDataPath` resolved against `DatapackRoot`.
    pub geodata_path: PathBuf,
    /// `GeoEditPath` resolved against `DatapackRoot` (`//geosave`, `//geosaveall`).
    pub geoedit_path: PathBuf,
    /// The second geodata set of `GeoOverride.ini` (D-26), if configured.
    pub override_: Option<GeoOverride>,
    /// What the engine itself reads (`PathFinding`, `AvoidObstructedPathNodes`).
    pub engine: GeoEngineOptions,
    /// Buffers and weights of the path searches.
    pub path: PathOptions,
    /// Worker threads of the pool (at least 1).
    pub path_threads: usize,
}

impl GeoSettings {
    /// Reads `GeoEngine.ini` (again, for the path: the typed config does not keep it) and the
    /// optional `GeoOverride.ini` from `config_dir`.
    pub fn load(config: &GameConfig, config_dir: &Path, datapack_root: &Path) -> Self {
        let geo_ini = match IniFile::load(config_dir.join(GeoEngineConfig::FILE)) {
            Ok(ini) => ini,
            Err(e) => {
                warn!(
                    "GeoEngine: cannot read {} ({e}); the default {GEODATA_PATH_DEFAULT} folder is used.",
                    GeoEngineConfig::FILE
                );
                IniFile::empty(GeoEngineConfig::FILE)
            }
        };
        let over_path = config_dir.join(GEO_OVERRIDE_FILE);
        let over_ini = if over_path.is_file() {
            match IniFile::load(&over_path) {
                Ok(ini) => Some(ini),
                Err(e) => {
                    warn!("GeoEngine: cannot read {GEO_OVERRIDE_FILE} ({e}); it is ignored.");
                    None
                }
            }
        } else {
            None
        };
        let cpus = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
        Self::resolve(config, &geo_ini, over_ini.as_ref(), datapack_root, cpus)
    }

    /// The pure part of [`load`](Self::load).
    pub fn resolve(
        config: &GameConfig,
        geo_ini: &IniFile,
        over_ini: Option<&IniFile>,
        datapack_root: &Path,
        cpus: usize,
    ) -> Self {
        let geo = &config.geo_engine;
        let path = match PathOptions::try_from(geo) {
            Ok(options) => options,
            Err(e) => {
                // Java would throw while building the buffers; the Rust server keeps the
                // pathfinding it can offer instead of refusing to start.
                warn!("GeoEngine: invalid PathFindBuffers ({e}); the Java defaults are used.");
                PathOptions::default()
            }
        };
        Self {
            geodata_path: resolve_geodata_path(
                datapack_root,
                &geo_ini.get_string(GEODATA_PATH_KEY, GEODATA_PATH_DEFAULT),
            ),
            geoedit_path: resolve_geodata_path(
                datapack_root,
                &geo_ini.get_string(GEOEDIT_PATH_KEY, GEOEDIT_PATH_DEFAULT),
            ),
            override_: over_ini.and_then(|ini| GeoOverride::from_ini(ini, datapack_root)),
            engine: GeoEngineOptions::from(geo),
            path,
            path_threads: path_threads(over_ini, cpus),
        }
    }
}

/// Java: `Paths.get(DATAPACK_ROOT.getPath() + "/" + config.getString("GeoDataPath", "geodata"))`.
/// Rust extension: an absolute value is used as it is (Java would glue it onto the root).
pub fn resolve_geodata_path(datapack_root: &Path, value: &str) -> PathBuf {
    datapack_root.join(value.trim().replace('\\', "/"))
}

/// `PathWorkerThreads` of `GeoOverride.ini`; the default is half of the CPUs, at least 1.
fn path_threads(over_ini: Option<&IniFile>, cpus: usize) -> usize {
    let default = (cpus / 2).max(1);
    let configured = over_ini.map_or(0, |ini| ini.get_int(PATH_WORKER_THREADS_KEY, 0));
    match usize::try_from(configured) {
        Ok(n) if n > 0 => n.min(MAX_PATH_WORKER_THREADS),
        _ => default,
    }
}

/// Loads the geodata (blocking: about a second in release, 1 to 1.6 GB of memory). Never fails.
// Java: GeoEngine constructor
pub fn load_engine(settings: &GeoSettings) -> Arc<GeoEngine> {
    let spec = GeoLoadSpec::new(&settings.geodata_path)
        .with_override(settings.override_.clone())
        .with_geoedit_path(&settings.geoedit_path);
    let (engine, report) = GeoEngine::load(&spec, settings.engine);
    for error in &report.failed {
        warn!("GeoEngine: {error}");
    }
    for (x, y) in &report.override_missing {
        warn!(
            "GeoEngine: region {x}_{y} is on the override list but the override folder has no usable file for it."
        );
    }
    if !report.overridden.is_empty() {
        info!(
            "GeoEngine: {} region(s) come from the override folder.",
            report.overridden.len()
        );
    }
    if report.loaded == 0 {
        warn!(
            "GeoEngine: no geodata found in {}; the server runs without geodata.",
            settings.geodata_path.display()
        );
    }
    if report.pathfinding_disabled {
        // Java: "Pathfinding is disabled." (PathFinding is set to 0 when no region loaded)
        info!("GeoEngine: Pathfinding is disabled.");
    }
    debug!(
        "GeoEngine: about {} MiB of geodata in memory.",
        engine.heap_bytes() >> 20
    );
    Arc::new(engine)
}

/// Turns finished searches into `WorldCommand::PathFound`.
pub struct WorldPathSink {
    world: WorldHandle,
}

impl WorldPathSink {
    pub fn new(world: WorldHandle) -> Self {
        Self { world }
    }
}

impl PathSink for WorldPathSink {
    fn deliver(&self, result: PathResult) {
        let (requester, request_id) = (result.requester, result.id);
        let command = WorldCommand::PathFound {
            requester,
            request_id,
            result,
        };
        if !self.world.send(command) {
            // The world thread is gone (shutdown); nobody waits for the path any more.
            debug!("path {request_id} of {requester} dropped, the world is gone");
        }
    }
}

/// The path worker pool behind a lock, so that it can be shared (`Arc<GameState>`) and still be
/// stopped through `&self` on shutdown (`PathPool::shutdown` needs `&mut`).
pub struct PathService {
    pool: RwLock<Option<PathPool>>,
}

impl PathService {
    /// No pool (`PathFinding = 0`).
    pub fn disabled() -> Self {
        Self {
            pool: RwLock::new(None),
        }
    }

    fn with_pool(pool: PathPool) -> Self {
        Self {
            pool: RwLock::new(Some(pool)),
        }
    }

    /// Whether workers are running.
    pub fn is_enabled(&self) -> bool {
        self.pool.read().unwrap_or_else(PoisonError::into_inner).is_some()
    }

    /// Queues a search. [`PathPoolError::ShutDown`] when there is no pool (`PathFinding = 0`
    /// or after shutdown); callers then fall back to a straight move, like Java without
    /// pathfinding.
    pub fn request(&self, request: PathRequest) -> Result<(), PathPoolError> {
        match &*self.pool.read().unwrap_or_else(PoisonError::into_inner) {
            Some(pool) => pool.request(request),
            None => Err(PathPoolError::ShutDown),
        }
    }

    /// Drops the live request of `requester`; `false` when there was none.
    pub fn cancel(&self, requester: u64) -> bool {
        self.pool
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .is_some_and(|pool| pool.cancel(requester))
    }

    pub fn stats(&self) -> Option<PoolStats> {
        self.pool
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .map(PathPool::stats)
    }

    /// Stops the workers and joins them. Idempotent. Blocks until running searches finish
    /// (milliseconds), so call it from `spawn_blocking` in async code.
    pub fn shutdown(&self) {
        let pool = self.pool.write().unwrap_or_else(PoisonError::into_inner).take();
        if let Some(mut pool) = pool {
            pool.shutdown();
            info!("GeoEngine: path workers stopped.");
        }
    }
}

/// The world's view of the pool (D-14): the movement code asks through `PathRequests`; answers
/// come back as `WorldCommand::PathFound` ([`WorldPathSink`]).
pub struct WorldPathRequests {
    service: Arc<PathService>,
}

impl WorldPathRequests {
    pub fn new(service: Arc<PathService>) -> Self {
        Self { service }
    }
}

impl l2_world::PathRequests for WorldPathRequests {
    fn request(&mut self, request: PathRequest) -> bool {
        match self.service.request(request) {
            Ok(()) => true,
            Err(e) => {
                debug!("path request refused: {e}");
                false
            }
        }
    }

    fn cancel(&mut self, requester: u64) {
        self.service.cancel(requester);
    }
}

/// The geodata and the path pool, shared through `GameState`.
pub struct Geo {
    /// Always present; has no regions when no geodata was found (null regions, like Java).
    pub engine: Arc<GeoEngine>,
    pub paths: Arc<PathService>,
}

impl Geo {
    /// An engine without regions and no pool (tests, and the fallback).
    pub fn empty() -> Self {
        Self {
            engine: Arc::new(GeoEngine::new(GeoSet::new(), GeoEngineOptions::default())),
            paths: Arc::new(PathService::disabled()),
        }
    }

    /// Starts the pool when the engine still has `PathFinding` above 0 (it is 0 after a load
    /// without regions). A pool that cannot start is logged and the server runs without paths.
    pub fn start(engine: Arc<GeoEngine>, settings: &GeoSettings, sink: Arc<dyn PathSink>) -> Self {
        let mode = engine.options().pathfinding;
        let paths = if mode > 0 {
            let config = PathPoolConfig {
                options: settings.path.clone(),
                ..PathPoolConfig::new(settings.path_threads)
            };
            match PathPool::with_config(config, Arc::clone(&engine), sink) {
                Ok(pool) => {
                    info!(
                        "GeoEngine: Path finding is on (PathFinding = {mode}, {} worker thread(s)).",
                        pool.threads()
                    );
                    PathService::with_pool(pool)
                }
                Err(e) => {
                    warn!("GeoEngine: cannot start the path workers ({e}); path finding is off.");
                    PathService::disabled()
                }
            }
        } else {
            info!("GeoEngine: Path finding is off (PathFinding = {mode}).");
            PathService::disabled()
        };
        Self {
            engine,
            paths: Arc::new(paths),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::Duration;

    use l2_geo::coords::{world_x, world_y};
    use l2_geo::region::REGION_BLOCKS;
    use l2_geo::{Location, NoObstacles, PathFailure};

    use super::*;

    fn ini(text: &str) -> IniFile {
        IniFile::parse("test.ini", text).unwrap()
    }

    fn config_with(geo_ini: &str) -> GameConfig {
        GameConfig {
            geo_engine: GeoEngineConfig::from_ini(&ini(geo_ini)),
            ..GameConfig::default()
        }
    }

    fn sink() -> (Arc<WorldPathSink>, mpsc::Receiver<WorldCommand>) {
        let (tx, rx) = mpsc::channel();
        (Arc::new(WorldPathSink::new(WorldHandle::new(tx))), rx)
    }

    #[test]
    fn geodata_path_is_relative_to_the_datapack_root() {
        let root = std::env::temp_dir().join("gs_dp");
        let config = config_with("PathFinding = 2");
        // Missing key: Java default "geodata".
        let s = GeoSettings::resolve(&config, &ini(""), None, &root, 4);
        assert_eq!(s.geodata_path, root.join("geodata"));
        // `GeoEditPath` (L-07): Java default "saves", same resolution.
        assert_eq!(s.geoedit_path, root.join("saves"));
        let s = GeoSettings::resolve(&config, &ini("GeoEditPath = ./saves/"), None, &root, 4);
        assert_eq!(s.geoedit_path, root.join("./saves/"));
        let s = GeoSettings::resolve(&config, &ini("GeoDataPath = data/geo"), None, &root, 4);
        assert_eq!(s.geodata_path, root.join("data").join("geo"));
        // Backslashes are separators, like in DatapackRoot.
        let s = GeoSettings::resolve(&config, &ini("GeoDataPath = data\\\\geo"), None, &root, 4);
        assert_eq!(s.geodata_path, root.join("data").join("geo"));
        // An absolute path is used as it is.
        let abs = std::env::temp_dir().join("elsewhere");
        let text = format!("GeoDataPath = {}", abs.to_string_lossy().replace('\\', "/"));
        let s = GeoSettings::resolve(&config, &ini(&text), None, &root, 4);
        assert_eq!(s.geodata_path, abs);
        assert_eq!(s.engine.pathfinding, 2);
    }

    #[test]
    fn override_file_present_absent_and_odd() {
        let root = std::env::temp_dir().join("gs_dp");
        let config = config_with("");
        let geo_ini = ini("");
        // Absent.
        let s = GeoSettings::resolve(&config, &geo_ini, None, &root, 8);
        assert!(s.override_.is_none());
        assert_eq!(s.path_threads, 4);
        // Present: the folder is relative to the datapack root, bad entries are dropped.
        let over = ini(
            "GeoDataOverridePath = geo2\nGeoDataOverrideRegions = 20_18, 21_18, nope\nPathWorkerThreads = 3\n",
        );
        let s = GeoSettings::resolve(&config, &geo_ini, Some(&over), &root, 8);
        let o = s.override_.unwrap();
        assert_eq!(o.path, root.join("geo2"));
        assert_eq!(o.regions, vec![(20, 18), (21, 18)]);
        assert_eq!(s.path_threads, 3);
        // A path without usable regions is no override.
        let over = ini("GeoDataOverridePath = geo2\nGeoDataOverrideRegions = nope\n");
        assert!(
            GeoSettings::resolve(&config, &geo_ini, Some(&over), &root, 8)
                .override_
                .is_none()
        );
    }

    #[test]
    fn worker_thread_count_rules() {
        assert_eq!(path_threads(None, 1), 1);
        assert_eq!(path_threads(None, 2), 1);
        assert_eq!(path_threads(None, 16), 8);
        assert_eq!(path_threads(Some(&ini("PathWorkerThreads = 0")), 6), 3);
        assert_eq!(path_threads(Some(&ini("PathWorkerThreads = -2")), 6), 3);
        assert_eq!(path_threads(Some(&ini("PathWorkerThreads = x")), 6), 3);
        assert_eq!(path_threads(Some(&ini("PathWorkerThreads = 5")), 2), 5);
        assert_eq!(
            path_threads(Some(&ini("PathWorkerThreads = 100000")), 2),
            MAX_PATH_WORKER_THREADS
        );
    }

    #[test]
    fn config_files_in_a_directory_are_read() {
        let dir = tempfile::tempdir().unwrap();
        let config_dir = dir.path().join("config");
        std::fs::create_dir(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("GeoEngine.ini"),
            "GeoDataPath = g\nPathFinding = 2\n",
        )
        .unwrap();
        let config = config_with("PathFinding = 2");
        let s = GeoSettings::load(&config, &config_dir, dir.path());
        assert_eq!(s.geodata_path, dir.path().join("g"));
        assert!(s.override_.is_none());

        // Malformed override file (bad unicode escape): logged, treated as absent.
        std::fs::write(config_dir.join(GEO_OVERRIDE_FILE), "PathWorkerThreads = \\u12\n").unwrap();
        let s = GeoSettings::load(&config, &config_dir, dir.path());
        assert!(s.override_.is_none());

        std::fs::write(
            config_dir.join(GEO_OVERRIDE_FILE),
            "GeoDataOverridePath = alt\nGeoDataOverrideRegions = 20_18\nPathWorkerThreads = 2\n",
        )
        .unwrap();
        let s = GeoSettings::load(&config, &config_dir, dir.path());
        assert_eq!(s.override_.unwrap().path, dir.path().join("alt"));
        assert_eq!(s.path_threads, 2);

        // No GeoEngine.ini at all: the default folder.
        let empty = tempfile::tempdir().unwrap();
        let s = GeoSettings::load(&config, empty.path(), dir.path());
        assert_eq!(s.geodata_path, dir.path().join("geodata"));
    }

    #[test]
    fn startup_without_a_geodata_folder_is_not_fatal() {
        let dir = tempfile::tempdir().unwrap();
        let config = config_with("PathFinding = 2");
        let settings = GeoSettings::resolve(&config, &ini(""), None, dir.path(), 4);
        assert!(!settings.geodata_path.exists());
        let engine = load_engine(&settings);
        assert_eq!(engine.loaded_count(), 0);
        // Java: PathFinding drops to 0 when no region loaded, so no pool starts.
        assert_eq!(engine.options().pathfinding, 0);
        let (sink, _rx) = sink();
        let geo = Geo::start(engine, &settings, sink);
        assert!(!geo.paths.is_enabled());
        assert!(geo.paths.stats().is_none());
        let request = PathRequest::new(
            1,
            1,
            Location::new(0, 0, 0),
            Location::new(1, 1, 0),
            Arc::new(NoObstacles),
        );
        assert!(matches!(geo.paths.request(request), Err(PathPoolError::ShutDown)));
        geo.paths.shutdown();
    }

    fn flat_region() -> Vec<u8> {
        let mut out = Vec::with_capacity(REGION_BLOCKS * 3);
        for _ in 0..REGION_BLOCKS {
            out.push(0);
            out.extend_from_slice(&0i16.to_le_bytes());
        }
        out
    }

    /// Settings that read the single flat region 20_18 written into `dir`.
    fn flat_settings(dir: &Path, geo_ini: &str, threads: usize) -> GeoSettings {
        std::fs::write(dir.join("20_18.l2j"), flat_region()).unwrap();
        let mut settings = GeoSettings::resolve(&config_with(geo_ini), &ini(""), None, dir, 4);
        settings.geodata_path = dir.to_path_buf();
        settings.path_threads = threads;
        settings
    }

    #[test]
    fn the_world_requests_paths_through_the_pool_and_a_stopped_pool_refuses() {
        use l2_world::PathRequests;

        let dir = tempfile::tempdir().unwrap();
        let settings = flat_settings(dir.path(), "PathFinding = 2", 1);
        let engine = load_engine(&settings);
        let (sink, rx) = sink();
        let geo = Geo::start(engine, &settings, sink);
        let mut requests = WorldPathRequests::new(Arc::clone(&geo.paths));

        let cell = |gx: i32, gy: i32| Location::new(world_x(20 * 2048 + gx), world_y(18 * 2048 + gy), 0);
        let request = PathRequest::new(5, 1, cell(10, 10), cell(40, 30), Arc::new(NoObstacles));
        assert!(requests.request(request));
        assert!(matches!(
            rx.recv_timeout(Duration::from_secs(20)).unwrap(),
            WorldCommand::PathFound {
                requester: 5,
                request_id: 1,
                ..
            }
        ));
        requests.cancel(5);

        geo.paths.shutdown();
        let late = PathRequest::new(5, 2, cell(1, 1), cell(2, 2), Arc::new(NoObstacles));
        assert!(!requests.request(late), "refused: the move then walks straight");
    }

    #[test]
    fn pool_results_reach_the_world_channel_and_shutdown_is_clean() {
        let dir = tempfile::tempdir().unwrap();
        let settings = flat_settings(dir.path(), "PathFinding = 2", 2);
        let engine = load_engine(&settings);
        assert_eq!(engine.loaded_count(), 1);
        assert_eq!(engine.options().pathfinding, 2);

        let (sink, rx) = sink();
        let geo = Geo::start(engine, &settings, sink);
        assert!(geo.paths.is_enabled());

        let cell = |gx: i32, gy: i32| Location::new(world_x(20 * 2048 + gx), world_y(18 * 2048 + gy), 0);
        let request =
            PathRequest::new(42, 7, cell(10, 10), cell(40, 30), Arc::new(NoObstacles)).with_playable(true);
        geo.paths.request(request).unwrap();
        match rx.recv_timeout(Duration::from_secs(20)).unwrap() {
            WorldCommand::PathFound {
                requester,
                request_id,
                result,
            } => {
                assert_eq!((requester, request_id), (42, 7));
                assert_eq!((result.requester, result.id), (42, 7));
                // On open flat ground there is a path (its nodes are the l2-geo tests' business).
                assert!(result.path.is_ok(), "{:?}", result.path);
            }
            other => panic!("unexpected command {other:?}"),
        }
        assert_eq!(geo.paths.stats().unwrap().delivered, 1);

        geo.paths.shutdown();
        assert!(!geo.paths.is_enabled());
        let late = PathRequest::new(42, 8, cell(1, 1), cell(2, 2), Arc::new(NoObstacles));
        assert!(matches!(geo.paths.request(late), Err(PathPoolError::ShutDown)));
        // Idempotent.
        geo.paths.shutdown();
    }

    #[test]
    fn pathfinding_off_starts_no_pool() {
        let dir = tempfile::tempdir().unwrap();
        let settings = flat_settings(dir.path(), "PathFinding = 0", 2);
        let engine = load_engine(&settings);
        assert_eq!(engine.loaded_count(), 1);
        let (sink, _rx) = sink();
        let geo = Geo::start(engine, &settings, sink);
        assert!(!geo.paths.is_enabled());
    }

    #[test]
    fn a_gone_world_does_not_break_the_sink() {
        let (tx, rx) = mpsc::channel::<WorldCommand>();
        drop(rx);
        let sink = WorldPathSink::new(WorldHandle::new(tx));
        sink.deliver(PathResult {
            requester: 1,
            id: 1,
            path: Err(PathFailure::NoPath),
            elapsed: Duration::ZERO,
        });
    }

    /// Loads the owner's converted geodata: about 5 s and 1.6 GB in a debug build, so it only
    /// runs on request (`cargo test -p game-server -- --ignored real_geodata`).
    #[test]
    #[ignore = "loads about 1.6 GB of real geodata"]
    fn real_geodata_loads_when_the_reference_folder_exists() {
        let converted = Path::new(r"C:\l2-rust\geodata-converted");
        if !converted.is_dir() {
            eprintln!("skipped: {} is missing", converted.display());
            return;
        }
        let mut settings =
            GeoSettings::resolve(&config_with("PathFinding = 2"), &ini(""), None, Path::new("."), 4);
        settings.geodata_path = converted.to_path_buf();
        let engine = load_engine(&settings);
        eprintln!("loaded {} regions", engine.loaded_count());
        assert!(engine.loaded_count() > 0);
    }
}
