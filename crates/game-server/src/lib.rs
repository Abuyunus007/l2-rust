//! The game server: startup, background tasks and shutdown.
// Java: org/l2jmobius/gameserver/GameServer.java (constructor = startup order)
// Java: org/l2jmobius/gameserver/Shutdown.java (what is stored and stopped on shutdown)
//
// Same layout as `login-server`:
// - `main.rs`    command line, logging, signals, exit code. Nothing else.
// - `lib.rs`     `Server::start` runs the startup steps in Java's order and returns a running
//                `Server`; `Server::run_until` waits for a signal or a shutdown request;
//                `Server::shutdown` stores the players and stops everything.
// - `state.rs`   the shared `Arc<GameState>` (config, pool, ids, datapack, world handle).
// - `runtime.rs` the world thread and its outbox seam.
// - `hexid.rs`, `admin.rs`, `client.rs`, `logging.rs`: one module per concern.
//
// Startup order (Java `GameServer` constructor): config, database, IdManager, data, World,
// ... client listener, login server thread. What exists so far:
//   1. config            `GameConfig::load`                              (M2-CFG-01)
//   2. database          `l2_db::connect`
//   3. IdManager         `IdFactory` from the Rust range (D-39)           (M2-IDF-01)
//   4. data              `Datapack::load`                                (M2-DATA-01)
//   5. geodata           `geo::load_engine` (before the world: it gets the engine) (M3-GEO-03)
//   6. world             the world thread + persistence task            (M2-WORLD-01, M2-PERSIST-01)
//   7. path workers      `geo::Geo::start` (need the world's command channel)   (M3-GEO-03)
//   8. hexid.txt         the saved login server registration
//   9. login server link (M2-GS-02), then the client listener and sessions (M2-GS-03)
//  10. admin endpoint
//   4. data              `Datapack::load_with` (options from the config) (M2-DATA-01, M3-CFG-02)
//   5. world             the world thread + persistence task            (M2-WORLD-01, M2-PERSIST-01)
//   6. hexid.txt         the saved login server registration
//   7. login server link (M2-GS-02), then the client listener and sessions (M2-GS-03)
//   8. admin endpoint
// The character screen (M2-CHAR-01) and entering the world (M2-ENTER-01) live in `client/`.

pub mod admin;
pub mod client;
pub mod geo;
pub mod hexid;
pub mod logging;
pub mod ls_link;
pub mod npcs;
pub mod persist;
pub mod runtime;
pub mod state;
pub mod world_data;

use std::net::SocketAddr;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use l2_config::{ConfigError, DatabaseConfig, GameConfig, IniFile, InterfaceConfig};
use l2_data::{DataError, Datapack, DatapackOptions};
use l2_db::{DbError, IdRepository, Pool};
use l2_net::CancellationToken;
use l2_util::WorldRng;
use l2_world::{IdError, IdFactory, RUST_FIRST_OBJECT_ID, RUST_LAST_OBJECT_ID, StaticData, WorldCommand};
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tracing::{info, warn};

use crate::persist::{DbStore, LsNotice, RetryPolicy};
use crate::runtime::WorldRuntime;
use crate::state::{ExitReason, GameState, ShutdownHandle};
use crate::world_data::DatapackData;

/// File names inside the config directory.
pub const DATABASE_CONFIG_FILE: &str = "Database.ini";
pub const INTERFACE_CONFIG_FILE: &str = "Interface.ini";
pub const SERVER_CONFIG_FILE: &str = "Server.ini";

/// Where the server reads and writes. Relative paths are relative to `root`, which plays the
/// role of Java's working directory (the `dist/game` folder).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    pub root: PathBuf,
    /// `*.ini` files and `hexid.txt` (Java `./config`).
    pub config_dir: PathBuf,
    /// Log files (Java `./log`).
    pub log_dir: PathBuf,
}

impl Paths {
    pub fn resolve(root: &Path, config_dir: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            config_dir: root.join(config_dir),
            log_dir: root.join(logging::LOG_DIR_NAME),
        }
    }
}

/// A startup failure. The process exits with code 1.
#[derive(Debug, thiserror::Error)]
pub enum StartupError {
    #[error("cannot load the configuration: {0}")]
    Config(#[from] ConfigError),
    #[error("database error: {0}")]
    Database(#[from] DbError),
    /// Java: `IdManager.getNextId` throws "Ran out of valid ids." The server must not start
    /// without ids to hand out (D-39).
    #[error("object ids: {0}")]
    Ids(#[from] IdError),
    #[error("cannot load the datapack: {0}")]
    Datapack(#[from] DataError),
    #[error("cannot start the world thread: {0}")]
    WorldThread(std::io::Error),
    /// A listener could not be opened.
    #[error("{what} listener could not be started on {address}: {source}")]
    Listen {
        what: &'static str,
        address: String,
        source: std::io::Error,
    },
    #[error("internal error: {0}")]
    Internal(String),
}

/// A running game server (initialized, background tasks started).
pub struct Server {
    state: Arc<GameState>,
    shutdown_requests: mpsc::UnboundedReceiver<ExitReason>,
    /// The world thread and the consumer of its outbox (the persistence task).
    world: WorldRuntime,
    /// What the login server link sends next: `PlayerInGame`, and `PlayerLogout` once the
    /// character is stored. Taken once by the link (M2-GS-02).
    ls_notices: Option<mpsc::UnboundedReceiver<LsNotice>>,
    /// Background and listener tasks. All are stopped on shutdown.
    tasks: JoinSet<()>,
    /// Cancelled on shutdown; listeners and their connections watch it (child tokens).
    cancel: CancellationToken,
    /// Where the client listener is bound (useful with port 0 in tests).
    client_addr: Option<SocketAddr>,
    /// Where the admin endpoint is bound, when enabled.
    admin_addr: Option<SocketAddr>,
    /// The link to the login server (M2-GS-02).
    ls_link: Option<ls_link::LsLink>,
}

impl Server {
    /// Runs the startup steps. On success the server is initialized and listening.
    pub async fn start(paths: &Paths) -> Result<Self, StartupError> {
        // Java: ConfigLoader.init (+ InterfaceConfig.load, DatabaseConfig)
        let config = GameConfig::load(&paths.config_dir)?;
        let database = DatabaseConfig::from_ini(&IniFile::load(paths.config_dir.join(DATABASE_CONFIG_FILE))?);
        let interface =
            InterfaceConfig::from_ini(&IniFile::load(paths.config_dir.join(INTERFACE_CONFIG_FILE))?);
        info!("Configuration loaded from {}.", paths.config_dir.display());
        if interface.enable_gui {
            // The Java default is true, so this is normal; it only tells that nothing opens.
            info!("EnableGUI is set, but the Rust game server is headless; it is ignored.");
        }
        if database.backup_database {
            warn!("BackupDatabase is set, but database backups on shutdown are not supported yet.");
        }

        // Java: DatabaseFactory.init
        let pool = l2_db::connect(&database).await?;
        Self::start_with(config, pool, paths).await
    }

    /// Everything after the database connection.
    async fn start_with(config: GameConfig, pool: Pool, paths: &Paths) -> Result<Self, StartupError> {
        // Java: IdManager.getInstance -> DatabaseIdManager. Only the read-only scan runs.
        //
        // D-39 (side by side with the Java GS on the same database): Java's startup also runs
        // `DatabaseIdManager.cleanDatabase` (orphan DELETEs), `cleanCharacterStatus`
        // (`UPDATE characters SET online = 0`) and `cleanTimestamps` (expired-row DELETEs).
        // They are NOT ported: they would modify rows the running Java GS owns, and resetting
        // `online` would mark its players offline. For the same reason this server never calls
        // `CharacterRepository::reset_all_online`.
        let used = IdRepository::new(pool.clone())
            .used_object_ids(RUST_FIRST_OBJECT_ID, RUST_LAST_OBJECT_ID)
            .await?;
        let ids = Arc::new(IdFactory::new(used.ids)?);
        info!(
            "IdManager: Free ObjectID's remaining: {} ({} ids of other ranges are left to the Java game server).",
            ids.available_count(),
            used.outside_range
        );

        // Java: ServerConfig.DATAPACK_ROOT, then the data loaders ("Data", "Characters", ...)
        let datapack_root = resolve_datapack_root(
            &paths.root,
            &IniFile::load(paths.config_dir.join(SERVER_CONFIG_FILE))?.get_string("DatapackRoot", "."),
        );
        let datapack = {
            let root = datapack_root.clone();
            let options = options_from_config(&config, &paths.config_dir);
            tokio::task::spawn_blocking(move || Datapack::load_with(&root, &options))
                .await
                .map_err(|e| StartupError::Internal(format!("datapack loader: {e}")))??
        };
        let datapack = Arc::new(datapack);
        // The stat engine tables (statBonus.xml and the datapack parts, skills included) and the
        // access levels (see `world_data`), and the config values the world reads.
        let static_data: Arc<dyn StaticData> = {
            let (datapack, config_dir) = (Arc::clone(&datapack), paths.config_dir.clone());
            Arc::new(
                tokio::task::spawn_blocking(move || DatapackData::load(datapack, &config_dir))
                    .await
                    .map_err(|e| StartupError::Internal(format!("data loader: {e}")))?,
            )
        };
        let world_settings = world_data::world_settings(&config, &paths.config_dir);
        let regen_settings = world_data::regen_settings(&config);

        // Java: GeoEngine.getInstance (section "Geodata"). Loaded before the world starts, so the
        // world thread gets the engine. Not fatal: Java runs without geodata too.
        let geo_settings = geo::GeoSettings::load(&config, &paths.config_dir, &datapack_root);
        let geo_engine = {
            let settings = geo_settings.clone();
            tokio::task::spawn_blocking(move || geo::load_engine(&settings))
                .await
                .map_err(|e| StartupError::Internal(format!("geodata loader: {e}")))?
        };

        // Java: ServerConfig.loadHexid (at the end of ServerConfig.load)
        let hexid = hexid::load(&paths.config_dir);

        // Java: World.init, PlayerAutoSaveTaskManager. The outbox consumer is the persistence
        // task; it stores in order and reports logouts (after the store) on `ls_notices`.
        let autosave = persist::autosave_interval(&paths.config_dir);
        let (notices_tx, ls_notices) = mpsc::unbounded_channel();
        let left_acks = persist::LeftAcks::default();
        let world = {
            let acks = left_acks.clone();
            let store = DbStore::new(pool.clone());
            let (data, settings) = (Arc::clone(&static_data), world_settings.clone());
            let geo_engine = Arc::clone(&geo_engine);
            let (zones, map_regions) = (Arc::clone(&datapack.zones), Arc::clone(&datapack.map_regions));
            let (html, teleporters) = (Arc::clone(&datapack.html), Arc::clone(&datapack.teleporters));
            let hit_condition_bonus = Arc::clone(&datapack.hit_condition_bonus);
            let exp_tables = (
                Arc::clone(&datapack.experience),
                Arc::clone(&datapack.experience_loss),
                Arc::clone(&datapack.karma_loss),
                Arc::clone(&datapack.skill_trees),
            );
            let npc_start = (
                Arc::clone(&datapack),
                Arc::clone(&ids),
                config.clone(),
                paths.config_dir.clone(),
            );
            WorldRuntime::start_configured(
                WorldRng::from_entropy(),
                autosave,
                move |world| {
                    world.set_static_data(Box::new(data));
                    world.set_settings(settings);
                    world.set_regen_settings(regen_settings);
                    world.set_geo(geo_engine);
                    // Zones (flags, compass icon) and the restart points of the map regions.
                    world.set_zone_data(zones, map_regions);
                    // The pages of NPC dialogs and the gatekeeper lists (always preloaded).
                    world.set_html_cache(html);
                    world.set_teleporter_data(teleporters);
                    // Java: HitConditionBonusData (the attack's height, side and night bonus).
                    world.set_hit_condition_bonus(hit_condition_bonus);
                    // Java: ExperienceData, ExperienceLossData, KarmaLossData, SkillTreeData.
                    let (experience, experience_loss, karma_loss, skill_trees) = exp_tables;
                    world.set_exp_data(experience, experience_loss, karma_loss, skill_trees);
                    // Java: GameServer, after the managers: SpawnData, then the day/night start.
                    let (datapack, ids, config, config_dir) = npc_start;
                    // Java: DoorData, FenceData, StaticObjectData (GameServer constructor), before the
                    // spawns.
                    if let Err(e) = world.spawn_door_data(&datapack, &ids) {
                        tracing::error!("Doors, fences and static objects could not all be spawned: {e}");
                    }
                    // Java: GameServer, after the managers: SpawnData, then the day/night start.
                    npcs::spawn_npcs(world, &datapack, ids, &config, &config_dir);
                },
                move |outbox| persist::spawn(store, outbox, notices_tx, RetryPolicy::default(), acks),
            )
            .map_err(StartupError::WorldThread)?
        };

        // D-14: the path workers answer with `WorldCommand::PathFound` on the world's channel.
        let geo = geo::Geo::start(
            geo_engine,
            &geo_settings,
            Arc::new(geo::WorldPathSink::new(world.handle())),
        );

        // The world asks the pool for paths through this (nothing to install when it is off: a
        // move then goes straight to its destination, like Java with `PathFinding = 0`).
        if geo.paths.is_enabled() {
            let requests = geo::WorldPathRequests::new(Arc::clone(&geo.paths));
            if !world
                .handle()
                .send(WorldCommand::SetPathRequests(Box::new(requests)))
            {
                warn!("The world stopped before it got the path service.");
            }
        }

        let (shutdown, shutdown_requests) = ShutdownHandle::channel();
        let max_players = config.server.maximum_online_users;
        let state = Arc::new(GameState {
            config: Arc::new(config),
            paths: paths.clone(),
            pool,
            ids,
            datapack,
            static_data,
            world_settings,
            geo,
            world: world.handle(),
            left_acks,
            hexid,
            shutdown,
        });

        let mut server = Self {
            state,
            shutdown_requests,
            world,
            ls_notices: Some(ls_notices),
            tasks: JoinSet::new(),
            cancel: CancellationToken::new(),
            client_addr: None,
            admin_addr: None,
            ls_link: None,
        };
        // The login server link first: the client sessions authenticate through it. (Java starts
        // `LoginServerThread` after the listener; nothing can log in before the link is up
        // either way.)
        let link = ls_link::start(&mut server);
        if let Some(mut notices) = server.take_ls_notices() {
            // PlayerInGame / PlayerLogout in the order the world and the persistence task
            // produced them; a logout only after the character is stored (D-15).
            server.spawn(async move {
                while let Some(notice) = notices.recv().await {
                    match notice {
                        LsNotice::PlayerInGame { account } => link.player_in_game(&account).await,
                        LsNotice::PlayerLogout { account } => link.player_logout(&account).await,
                    }
                }
            });
        }
        client::start_listener(&mut server).await?;
        admin::start(&mut server, &paths.config_dir).await?;

        info!("GameServer: Maximum number of connected players is {max_players}.");
        info!("Game server is ready.");
        Ok(server)
    }

    /// The login server notices in the order the world produced them. A `PlayerLogout` comes
    /// only after the character's final store (requirement of M2-PERSIST-01). `None` after the
    /// first call: the login server link takes it.
    pub fn take_ls_notices(&mut self) -> Option<mpsc::UnboundedReceiver<LsNotice>> {
        self.ls_notices.take()
    }

    /// The bound address of the client listener.
    pub fn client_addr(&self) -> Option<SocketAddr> {
        self.client_addr
    }

    /// The bound address of the admin endpoint (`None` when `Admin.ini` does not enable it).
    pub fn admin_addr(&self) -> Option<SocketAddr> {
        self.admin_addr
    }

    /// The link to the login server (`auth_player`, `player_logout`, ...).
    pub fn ls_link(&self) -> Option<&ls_link::LsLink> {
        self.ls_link.as_ref()
    }

    /// Runs `task` until shutdown. Listener and link cards use this.
    pub fn spawn<F>(&mut self, task: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.tasks.spawn(task);
    }

    /// A token that is cancelled when the server shuts down (for `l2_net::listen` and
    /// connections).
    pub fn cancel_token(&self) -> CancellationToken {
        self.cancel.child_token()
    }

    pub fn state(&self) -> &Arc<GameState> {
        &self.state
    }

    /// Waits until `stop` completes (a signal: exit code 0) or something requests a shutdown or
    /// restart through [`GameState::request_shutdown`], then shuts down.
    // Java: the shutdown hook (SIGTERM) and Shutdown.startShutdown
    pub async fn run_until(mut self, stop: impl Future<Output = ()>) -> ExitReason {
        let reason = tokio::select! {
            () = stop => {
                info!("Shutdown signal received.");
                ExitReason::Shutdown
            }
            request = self.shutdown_requests.recv() => {
                // `None` cannot happen while the state holds a sender; treat it as a shutdown.
                request.unwrap_or(ExitReason::Shutdown)
            }
        };
        self.finish(reason).await
    }

    /// Stops right away. `restart` gives exit code 2 instead of 0.
    pub async fn shutdown(self, restart: bool) -> ExitReason {
        let reason = if restart {
            ExitReason::Restart
        } else {
            ExitReason::Shutdown
        };
        self.finish(reason).await
    }

    // Java: Shutdown.startShutdownActions
    async fn finish(mut self, reason: ExitReason) -> ExitReason {
        info!(
            "Game server is shutting down{}.",
            if reason == ExitReason::Restart {
                " for a restart"
            } else {
                ""
            }
        );
        // 0. The path workers first: they send to the world's channel, and no search can start
        //    once the world no longer reads.
        let state = Arc::clone(&self.state);
        if tokio::task::spawn_blocking(move || state.geo.paths.shutdown())
            .await
            .is_err()
        {
            warn!("The path worker shutdown task failed.");
        }
        // 1. Store and disconnect every player, and wait until the outbox is drained. This
        //    comes first so the players get their goodbye packets while the connections are
        //    still open, and so the login server link (a task below) is still up for the
        //    `PlayerLogout` messages M2-GS-02 sends once a character is saved.
        self.world.shutdown().await;
        // Java: the server is reported down, then `LoginServerThread` is interrupted.
        if let Some(link) = &self.ls_link {
            link.shutdown().await;
        }
        // 2. Stop listeners and connections, then abort the remaining tasks and wait for them,
        //    so nothing touches the pool later.
        self.cancel.cancel();
        self.tasks.shutdown().await;
        // 3. Java: DatabaseFactory.close(), the very last thing.
        self.state.pool.close().await;
        info!("Game server stopped, exit code {}.", reason.code());
        reason
    }
}

/// Java: `new File(config.getString("DatapackRoot", ".").replace('\\', '/')).getCanonicalFile()`.
/// A relative path is relative to the server's root folder (Java: the working directory).
/// Symlinks are not resolved.
fn resolve_datapack_root(root: &Path, value: &str) -> PathBuf {
    let path = PathBuf::from(value.replace('\\', "/"));
    let joined = root.join(path);
    let absolute = std::path::absolute(&joined).unwrap_or(joined);
    let mut out = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// The datapack options the owner's Java game server gets from its config, so the Rust GS loads
/// the same custom items and NPCs (D-39, both servers share one database).
///
/// Each value is the key's getter result after Java's `load()`, with the same defaults, so an
/// empty config gives [`DatapackOptions::default`]. Keys: `General.ini` `CustomItemsLoad`,
/// `CustomNpcData` and `HtmCache`; `NPC.ini` `MaxAggroRange` and `DefaultCorpseTime`;
/// `Custom/FakePlayers.ini` `EnableFakePlayers`; `Development.ini` `NoSpawns`; `General.ini`
/// `CheckHtmlEncoding`.
// Java: GeneralConfig.java:258, 355, 358; NpcConfig.java:142, 145; FakePlayersConfig.java:50;
// the NoSpawns read in DevelopmentConfig; HtmCache.java:140 (CHECK_HTML_ENCODING).
fn options_from_config(config: &GameConfig, config_dir: &Path) -> DatapackOptions {
    DatapackOptions {
        skills: world_data::skill_load_options(config, config_dir),
        custom_items_load: config.general.custom_items_load,
        custom_npc_data: config.general.custom_npc_data,
        fake_players_enabled: config.fake_players.fake_players_enabled,
        no_spawns: config.development.no_spawns,
        max_aggro_range: config.npc.max_aggro_range,
        default_corpse_time: config.npc.default_corpse_time,
        htm_cache: config.general.htm_cache,
        check_html_encoding: config.general.check_html_encoding,
    }
}

/// Building blocks for unit tests that need a `Server` without a database.
#[cfg(test)]
pub(crate) mod test_support {
    use sqlx::mysql::MySqlPoolOptions;

    use super::*;
    use crate::state::WorldHandle;

    /// A pool that never connects, so shutdown paths run without a database.
    pub(crate) fn lazy_pool() -> Pool {
        MySqlPoolOptions::new()
            .connect_lazy("mysql://nobody@127.0.0.1:1/none")
            .unwrap()
    }

    /// A server with a running world, no listeners and a pool that is never used.
    pub(crate) fn idle_server() -> Server {
        let world = WorldRuntime::start(WorldRng::from_seed(1), crate::runtime::spawn_stub_consumer).unwrap();
        let (shutdown, shutdown_requests) = ShutdownHandle::channel();
        let handle: WorldHandle = world.handle();
        let state = Arc::new(GameState {
            config: Arc::new(GameConfig::default()),
            paths: Paths::resolve(Path::new("."), Path::new("config")),
            pool: lazy_pool(),
            ids: Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()),
            datapack: Arc::new(empty_datapack()),
            static_data: Arc::new(l2_world::NoData),
            world_settings: l2_world::WorldSettings::default(),
            geo: geo::Geo::empty(),
            world: handle,
            left_acks: persist::LeftAcks::default(),
            hexid: None,
            shutdown,
        });
        Server {
            state,
            shutdown_requests,
            world,
            ls_notices: None,
            tasks: JoinSet::new(),
            cancel: CancellationToken::new(),
            client_addr: None,
            admin_addr: None,
            ls_link: None,
        }
    }

    fn empty_datapack() -> Datapack {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("data")).unwrap();
        Datapack::load(dir.path()).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::test_support::idle_server;

    #[test]
    fn paths_follow_the_java_dist_layout() {
        let paths = Paths::resolve(Path::new("dist/game"), Path::new("./config"));
        assert_eq!(paths.config_dir, Path::new("dist/game").join("./config"));
        assert_eq!(paths.log_dir, Path::new("dist/game").join("log"));
    }

    #[test]
    fn absolute_config_directories_are_kept() {
        let absolute = std::env::temp_dir();
        let paths = Paths::resolve(Path::new("dist/game"), &absolute);
        assert_eq!(paths.config_dir, absolute);
    }

    #[test]
    fn datapack_root_is_relative_to_the_root_and_normalized() {
        let root = std::env::temp_dir().join("gs_root");
        assert_eq!(resolve_datapack_root(&root, "."), root);
        assert_eq!(resolve_datapack_root(&root, "./a/../b"), root.join("b"));
        // Backslashes are separators, like Java's `replace('\\', '/')`.
        assert_eq!(resolve_datapack_root(&root, "x\\y"), root.join("x").join("y"));
        // An absolute value ignores the root.
        let elsewhere = std::env::temp_dir().join("elsewhere");
        let value = elsewhere.to_string_lossy().replace('\\', "/");
        assert_eq!(resolve_datapack_root(&root, &value), elsewhere);
    }

    #[tokio::test]
    async fn shutdown_request_ends_run_until_with_its_exit_code() {
        let server = idle_server();
        server.state().request_shutdown(true);
        let reason = tokio::time::timeout(
            Duration::from_secs(10),
            server.run_until(std::future::pending::<()>()),
        )
        .await
        .unwrap();
        assert_eq!(reason, ExitReason::Restart);
        assert_eq!(reason.code(), 2);
    }

    #[tokio::test]
    async fn stop_signal_ends_run_until_with_exit_code_0() {
        let server = idle_server();
        let reason = tokio::time::timeout(Duration::from_secs(10), server.run_until(std::future::ready(())))
            .await
            .unwrap();
        assert_eq!(reason, ExitReason::Shutdown);
        assert_eq!(reason.code(), 0);
    }

    #[tokio::test]
    async fn direct_shutdown_stops_the_world_thread() {
        let server = idle_server();
        let state = Arc::clone(server.state());
        assert_eq!(server.shutdown(false).await, ExitReason::Shutdown);
        // The world is gone, so its channel refuses commands.
        assert!(!state.world.send(l2_world::WorldCommand::Disconnected {
            session: l2_world::SessionId(1)
        }));
    }

    #[test]
    fn fatal_has_exit_code_1() {
        assert_eq!(ExitReason::Fatal.code(), 1);
    }

    /// No config files at all: every key has Java's default, so the options are the defaults.
    #[test]
    fn options_from_an_empty_config_are_java_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let config = GameConfig::load(dir.path()).unwrap();
        assert_eq!(
            options_from_config(&config, dir.path()),
            DatapackOptions::default()
        );
        assert_eq!(
            options_from_config(&GameConfig::default(), dir.path()),
            DatapackOptions::default()
        );
    }

    /// The shipped `dist/game/config` turns the custom folders on and HtmCache off. Skips when
    /// the Java checkout is not there (CI).
    #[test]
    fn options_from_the_shipped_config_enable_custom_data() {
        let root = std::env::var("L2J_ROOT").unwrap_or_else(|_| r"C:\l2-rust\l2jmobius".to_string());
        let dir = Path::new(&root).join("dist").join("game").join("config");
        if !dir.is_dir() {
            eprintln!("skipped: no Java dist config at {}", dir.display());
            return;
        }
        let config = GameConfig::load(&dir).unwrap();
        let options = options_from_config(&config, &dir);
        assert!(options.custom_items_load);
        assert!(options.custom_npc_data);
        assert!(!options.htm_cache);
        assert!(options.check_html_encoding);
        assert!(!options.no_spawns);
        assert_eq!(options.max_aggro_range, 450);
        // Player.ini: EnableModifySkillDuration = True with the dance and song list.
        let skills = &options.skills;
        assert_eq!(
            (
                skills.min_abnormal_state_success_rate,
                skills.max_abnormal_state_success_rate
            ),
            (10, 90)
        );
        if skills.enable_modify_skill_duration {
            assert_eq!(skills.skill_duration_list.get(&264), Some(&3660));
        }
    }

    /// Each key reaches its option: EnableFakePlayers, NoSpawns, MaxAggroRange and
    /// DefaultCorpseTime from their own files, CustomItemsLoad from General.ini.
    #[test]
    fn each_config_key_reaches_its_option() {
        let dir = tempfile::tempdir().unwrap();
        let custom = dir.path().join("Custom");
        std::fs::create_dir_all(&custom).unwrap();
        std::fs::write(
            dir.path().join("General.ini"),
            "CustomItemsLoad = True\nHtmCache = False\nCheckHtmlEncoding = False\nCustomSkillsLoad = True\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("NPC.ini"),
            "MaxAggroRange = 900\nDefaultCorpseTime = 3\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("Development.ini"), "NoSpawns = True\n").unwrap();
        std::fs::write(custom.join("FakePlayers.ini"), "EnableFakePlayers = True\n").unwrap();
        std::fs::write(
            dir.path().join("Player.ini"),
            "MinAbnormalStateSuccessRate = 5\nMaxAbnormalStateSuccessRate = 95\n\
             EnableModifySkillDuration = True\nSkillDurationList = 264,3660;265,x;bad;\n\
             EnableModifySkillReuse = True\nSkillReuseList = 1,2\n",
        )
        .unwrap();

        let options = options_from_config(&GameConfig::load(dir.path()).unwrap(), dir.path());
        assert_eq!(
            options,
            DatapackOptions {
                custom_items_load: true,
                custom_npc_data: false,
                fake_players_enabled: true,
                no_spawns: true,
                max_aggro_range: 900,
                default_corpse_time: 3,
                htm_cache: false,
                check_html_encoding: false,
                skills: l2_data::SkillLoadOptions {
                    custom_skills_load: true,
                    min_abnormal_state_success_rate: 5,
                    max_abnormal_state_success_rate: 95,
                    enable_modify_skill_duration: true,
                    skill_duration_list: [(264, 3660)].into_iter().collect(),
                    enable_modify_skill_reuse: true,
                    skill_reuse_list: [(1, 2)].into_iter().collect(),
                },
            }
        );
    }
}
