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
//   5. world             the world thread + outbox seam                  (M2-WORLD-01)
//   6. hexid.txt         the saved login server registration
//   7. client listener   placeholder until M2-GS-03
//   8. admin endpoint
// Still to come, each slotting into `start_with`: login server link (M2-GS-02, Java starts
// `LoginServerThread` last), the persistence task (M2-PERSIST-01), client sessions (M2-GS-03).

pub mod admin;
pub mod client;
pub mod hexid;
pub mod logging;
pub mod runtime;
pub mod state;

use std::net::SocketAddr;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use l2_config::{ConfigError, DatabaseConfig, GameConfig, IniFile, InterfaceConfig};
use l2_data::{DataError, Datapack};
use l2_db::{DbError, IdRepository, Pool};
use l2_net::CancellationToken;
use l2_util::WorldRng;
use l2_world::{IdError, IdFactory, RUST_FIRST_OBJECT_ID, RUST_LAST_OBJECT_ID};
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tracing::{info, warn};

use crate::runtime::{WorldRuntime, spawn_stub_consumer};
use crate::state::{ExitReason, GameState, ShutdownHandle};

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
    /// The world thread and the consumer of its outbox (persistence, later).
    world: WorldRuntime,
    /// Background and listener tasks. All are stopped on shutdown.
    tasks: JoinSet<()>,
    /// Cancelled on shutdown; listeners and their connections watch it (child tokens).
    cancel: CancellationToken,
    /// Where the client listener is bound (useful with port 0 in tests).
    client_addr: Option<SocketAddr>,
    /// Where the admin endpoint is bound, when enabled.
    admin_addr: Option<SocketAddr>,
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
            tokio::task::spawn_blocking(move || Datapack::load(&root))
                .await
                .map_err(|e| StartupError::Internal(format!("datapack loader: {e}")))??
        };
        let datapack = Arc::new(datapack);

        // Java: ServerConfig.loadHexid (at the end of ServerConfig.load)
        let hexid = hexid::load(&paths.config_dir);

        // Java: World.init. The outbox consumer is a stub until M2-PERSIST-01 replaces it.
        let world = WorldRuntime::start(WorldRng::from_entropy(), spawn_stub_consumer)
            .map_err(StartupError::WorldThread)?;

        let (shutdown, shutdown_requests) = ShutdownHandle::channel();
        let max_players = config.server.maximum_online_users;
        let state = Arc::new(GameState {
            config: Arc::new(config),
            paths: paths.clone(),
            pool,
            ids,
            datapack,
            world: world.handle(),
            hexid,
            shutdown,
        });

        let mut server = Self {
            state,
            shutdown_requests,
            world,
            tasks: JoinSet::new(),
            cancel: CancellationToken::new(),
            client_addr: None,
            admin_addr: None,
        };
        // Listeners and links, in Java's order. Each card adds one line here:
        // - client listener on GameserverPort (this card: placeholder, M2-GS-03: sessions)
        // - login server link (M2-GS-02; Java starts `LoginServerThread` after the listener)
        client::start_listener(&mut server).await?;
        admin::start(&mut server, &paths.config_dir).await?;

        info!("GameServer: Maximum number of connected players is {max_players}.");
        info!("Game server is ready.");
        Ok(server)
    }

    /// The bound address of the client listener.
    pub fn client_addr(&self) -> Option<SocketAddr> {
        self.client_addr
    }

    /// The bound address of the admin endpoint (`None` when `Admin.ini` does not enable it).
    pub fn admin_addr(&self) -> Option<SocketAddr> {
        self.admin_addr
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
        // 1. Store and disconnect every player, and wait until the outbox is drained. This
        //    comes first so the players get their goodbye packets while the connections are
        //    still open, and so the login server link (a task below) is still up for the
        //    `PlayerLogout` messages M2-GS-02 sends once a character is saved.
        self.world.shutdown().await;
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
        let world = WorldRuntime::start(WorldRng::from_seed(1), spawn_stub_consumer).unwrap();
        let (shutdown, shutdown_requests) = ShutdownHandle::channel();
        let handle: WorldHandle = world.handle();
        let state = Arc::new(GameState {
            config: Arc::new(GameConfig::default()),
            paths: Paths::resolve(Path::new("."), Path::new("config")),
            pool: lazy_pool(),
            ids: Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()),
            datapack: Arc::new(empty_datapack()),
            world: handle,
            hexid: None,
            shutdown,
        });
        Server {
            state,
            shutdown_requests,
            world,
            tasks: JoinSet::new(),
            cancel: CancellationToken::new(),
            client_addr: None,
            admin_addr: None,
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
}
