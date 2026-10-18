//! The login server: startup, background tasks and shutdown.
// Java: org/l2jmobius/loginserver/LoginServer.java
//
// REFERENCE LAYOUT for server binaries (the game server copies it):
// - `main.rs`   command line, logging, signals, exit code. Nothing else.
// - `lib.rs`    `Server::start` runs the startup steps in Java's order and returns a running
//               `Server`; `Server::run_until` waits for a signal or a shutdown request;
//               `Server::shutdown` stops the background tasks and closes the database.
// - `state.rs`  the shared `Arc<LoginState>` that connection tasks will use.
// - one module per concern (`bans`, `keys`, `server_names`, `logging`).
// Everything that can be tested without a socket or a database is a plain function with the
// clock or RNG passed in. The startup itself needs a database and is covered by
// `tests/bootstrap.rs` (gated on `L2_TEST_DATABASE_URL`).
//
// Startup order (Java `LoginServer` constructor): config, database, key pools
// (`LoginController`), game server table, IP bans, scheduled restart, listeners. The listeners
// are M1-LS-02 (clients) and M1-LS-04 (game servers) and slot in before `ready`.

pub mod admin;
pub mod bans;
pub mod client;
pub mod gs;
pub mod keys;
pub mod logging;
pub mod server_names;
pub mod state;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use l2_config::{ConfigError, LoginConfig, LoginConfigSet};
use l2_db::{DbError, LoginRepository, Pool};
use l2_net::CancellationToken;
use l2_util::WorldRng;
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tokio::time::{Instant, interval_at, sleep};
use tracing::{info, warn};

use crate::bans::{BanList, load_ban_file};
use crate::keys::{KeyError, generate_client_keys, generate_game_server_keys};
use crate::server_names::{SERVER_NAMES_FILE, load_server_names};
use crate::state::{ExitReason, GameServerTable, LoginState, ShutdownHandle};

/// Time before a client that did not finish logging in is kicked, and the period of the purge
/// task. Java: `LoginController.LOGIN_TIMEOUT`.
pub const LOGIN_TIMEOUT: Duration = Duration::from_secs(5 * 60);
/// Java: `MILLISECONDS_PER_HOUR`.
const MILLIS_PER_HOUR: i64 = 3_600_000;
/// Name of the ban file in the root folder. Java: `BANNED_IPS_CONFIGURATION_PATH`.
pub const BANNED_IP_FILE: &str = "banned_ip.cfg";

/// Where the server reads and writes. Relative paths are relative to `root`, which plays the
/// role of Java's working directory (the `dist/login` folder).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    pub root: PathBuf,
    /// `*.ini` files (Java `./config`).
    pub config_dir: PathBuf,
    /// `servername.xml` (Java `./data`).
    pub data_dir: PathBuf,
    /// Log files (Java `./log`).
    pub log_dir: PathBuf,
    /// `banned_ip.cfg`.
    pub banned_ip_file: PathBuf,
}

impl Paths {
    pub fn resolve(root: &Path, config_dir: &Path, data_dir: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            config_dir: root.join(config_dir),
            data_dir: root.join(data_dir),
            log_dir: root.join(logging::LOG_DIR_NAME),
            banned_ip_file: root.join(BANNED_IP_FILE),
        }
    }
}

/// A startup failure. The process exits with code 1 (Java `System.exit(1)` on fatal errors).
#[derive(Debug, thiserror::Error)]
pub enum StartupError {
    #[error("cannot load the configuration: {0}")]
    Config(#[from] ConfigError),
    #[error("cannot connect to the database: {0}")]
    Database(#[from] DbError),
    #[error("cannot create the key pools: {0}")]
    Keys(#[from] KeyError),
    /// A listener could not be opened. Java: "FATAL: ... listener could not be started on ...".
    #[error("{what} listener could not be started on {address}: {source}")]
    Listen {
        what: &'static str,
        address: String,
        source: std::io::Error,
    },
}

/// A running login server (initialized, background tasks started).
pub struct Server {
    state: Arc<LoginState>,
    shutdown_requests: mpsc::UnboundedReceiver<ExitReason>,
    /// Background and listener tasks. All are stopped on shutdown.
    tasks: JoinSet<()>,
    /// Cancelled on shutdown; listeners and their connections watch it (child tokens).
    cancel: CancellationToken,
    /// Where the client listener is bound (useful with port 0 in tests).
    client_addr: Option<std::net::SocketAddr>,
}

impl Server {
    /// Runs the startup steps. On success the server is initialized and has no listeners yet.
    pub async fn start(paths: &Paths) -> Result<Self, StartupError> {
        let config = LoginConfig::load(&paths.config_dir)?;
        info!("Configuration loaded from {}.", paths.config_dir.display());
        if config.interface.enable_gui {
            // The Java default is true, so this is normal; it only tells that nothing opens.
            info!("EnableGUI is set, but the Rust login server is headless; it is ignored.");
        }
        if config.database.backup_database {
            warn!("BackupDatabase is set, but database backups on shutdown are not supported yet.");
        }
        let pool = l2_db::connect(&config.database).await?;
        Self::start_with(config, pool, paths).await
    }

    /// Everything after the database connection.
    async fn start_with(config: LoginConfigSet, pool: Pool, paths: &Paths) -> Result<Self, StartupError> {
        // Java: LoginController.load
        let client_keys = generate_client_keys().await?;

        // Java: GameServerTable constructor
        let names = load_server_names(&paths.data_dir.join(SERVER_NAMES_FILE));
        let registered = GameServerTable::load_registered(&LoginRepository::new(pool.clone())).await;
        let game_server_keys = generate_game_server_keys().await?;
        let game_servers = GameServerTable::new(names, registered, game_server_keys);

        // Java: LoginServer.loadBanFile
        let bans = BanList::new();
        load_ban_file(&paths.banned_ip_file, &bans);

        let (shutdown, shutdown_requests) = ShutdownHandle::channel();
        let restart_delay = restart_delay(&config.login);
        let state = Arc::new(LoginState::new(
            config,
            pool,
            client_keys,
            game_servers,
            bans,
            WorldRng::from_entropy(),
            shutdown.clone(),
        ));

        let mut tasks = JoinSet::new();
        spawn_purge_task(&mut tasks, Arc::clone(&state));
        if let Some(delay) = restart_delay {
            spawn_restart_timer(&mut tasks, delay, shutdown);
        }

        let mut server = Self {
            state,
            shutdown_requests,
            tasks,
            cancel: CancellationToken::new(),
            client_addr: None,
        };
        // Listeners. Each card adds one line here:
        // - client listener on LoginserverPort (M1-LS-02)
        // - game server listener on LoginPort (M1-LS-04)
        server.start_listeners().await?;
        admin::start(&mut server, &paths.config_dir).await?;
        info!("Login server is ready.");
        Ok(server)
    }

    /// Starts the network listeners (see the list in `start_with`).
    async fn start_listeners(&mut self) -> Result<(), StartupError> {
        // Java order: game server listener first, then clients.
        gs::start(self).await?;
        client::start_listener(self).await?;
        Ok(())
    }

    /// The bound address of the client listener.
    pub fn client_addr(&self) -> Option<std::net::SocketAddr> {
        self.client_addr
    }

    /// Runs `task` until shutdown. Listener cards use this.
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

    pub fn state(&self) -> &Arc<LoginState> {
        &self.state
    }

    /// Waits until `stop` completes (a signal: exit code 0) or something requests a shutdown or
    /// restart through [`LoginState::request_shutdown`], then shuts down.
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
    // Java: LoginServer.shutdown(restart)
    pub async fn shutdown(self, restart: bool) -> ExitReason {
        let reason = if restart {
            ExitReason::Restart
        } else {
            ExitReason::Shutdown
        };
        self.finish(reason).await
    }

    async fn finish(mut self, reason: ExitReason) -> ExitReason {
        info!(
            "Login server is shutting down{}.",
            if reason == ExitReason::Restart {
                " for a restart"
            } else {
                ""
            }
        );
        // Stop listeners and connections first, then abort the remaining tasks and wait for
        // them, so nothing touches the pool later.
        self.cancel.cancel();
        self.tasks.shutdown().await;
        self.state.pool.close().await;
        info!("Login server stopped, exit code {}.", reason.code());
        reason
    }
}

/// The delay of the scheduled restart, or `None` when it is off or the configured time is not
/// positive.
// Java: LoginServer.configureScheduledRestart
pub fn restart_delay(config: &LoginConfig) -> Option<Duration> {
    if !config.login_server_schedule_restart {
        return None;
    }
    let hours = config.login_server_schedule_restart_time;
    // NOTE(java-quirk): `hours * MILLISECONDS_PER_HOUR` is a long multiplication in Java and
    // wraps on overflow; the wrapped value is used as is.
    let millis = hours.wrapping_mul(MILLIS_PER_HOUR);
    let Some(delay_ms) = u64::try_from(millis).ok().filter(|ms| *ms > 0) else {
        warn!(
            "Login server restart scheduling is enabled but the computed delay is invalid. Configured hours: {hours}."
        );
        return None;
    };
    info!("Login server restart scheduled in {hours} hour(s). Computed delay: {delay_ms} ms.");
    Some(Duration::from_millis(delay_ms))
}

/// After `delay`, asks for a restart (exit code 2).
fn spawn_restart_timer(tasks: &mut JoinSet<()>, delay: Duration, shutdown: ShutdownHandle) {
    tasks.spawn(async move {
        sleep(delay).await;
        info!("Scheduled restart time reached.");
        shutdown.request(true);
    });
}

/// Calls [`LoginState::purge`] every [`LOGIN_TIMEOUT`], first after one full period.
// Java: ThreadPool.scheduleAtFixedRate(this::purge, LOGIN_TIMEOUT, LOGIN_TIMEOUT)
fn spawn_purge_task(tasks: &mut JoinSet<()>, state: Arc<LoginState>) {
    tasks.spawn(async move {
        let mut ticker = interval_at(Instant::now() + LOGIN_TIMEOUT, LOGIN_TIMEOUT);
        loop {
            ticker.tick().await;
            state.purge();
        }
    });
}

#[cfg(test)]
mod tests {
    use l2_config::IniFile;

    use super::*;
    use crate::state::test_support::{default_config, state_with};

    fn login_config(text: &str) -> LoginConfig {
        LoginConfig::from_ini(&IniFile::parse("Server.ini", text).unwrap())
    }

    #[test]
    fn paths_follow_the_java_dist_layout() {
        let paths = Paths::resolve(
            Path::new("dist/login"),
            Path::new("./config"),
            Path::new("./data"),
        );
        assert_eq!(paths.config_dir, Path::new("dist/login").join("./config"));
        assert_eq!(paths.data_dir, Path::new("dist/login").join("./data"));
        assert_eq!(paths.log_dir, Path::new("dist/login").join("log"));
        assert_eq!(
            paths.banned_ip_file,
            Path::new("dist/login").join("banned_ip.cfg")
        );
    }

    #[test]
    fn absolute_directories_are_kept() {
        let absolute = std::env::temp_dir();
        let paths = Paths::resolve(Path::new("dist/login"), &absolute, Path::new("data"));
        assert_eq!(paths.config_dir, absolute);
    }

    #[test]
    fn restart_is_off_by_default() {
        assert_eq!(restart_delay(&login_config("")), None);
        // The time alone does nothing without LoginRestartSchedule.
        assert_eq!(restart_delay(&login_config("LoginRestartTime = 5\n")), None);
    }

    #[test]
    fn restart_delay_is_in_hours() {
        let config = login_config("LoginRestartSchedule = True\n");
        assert_eq!(restart_delay(&config), Some(Duration::from_secs(24 * 3600)));
        let config = login_config("LoginRestartSchedule = True\nLoginRestartTime = 6\n");
        assert_eq!(restart_delay(&config), Some(Duration::from_millis(6 * 3_600_000)));
    }

    #[test]
    fn non_positive_restart_time_is_ignored() {
        assert_eq!(
            restart_delay(&login_config(
                "LoginRestartSchedule = True\nLoginRestartTime = 0\n"
            )),
            None
        );
        assert_eq!(
            restart_delay(&login_config(
                "LoginRestartSchedule = True\nLoginRestartTime = -3\n"
            )),
            None
        );
    }

    #[tokio::test]
    async fn restart_timer_requests_exit_code_2() {
        let (handle, mut requests) = ShutdownHandle::channel();
        let mut tasks = JoinSet::new();
        spawn_restart_timer(&mut tasks, Duration::from_millis(20), handle);
        let reason = tokio::time::timeout(Duration::from_secs(5), requests.recv())
            .await
            .unwrap();
        assert_eq!(reason, Some(ExitReason::Restart));
        assert_eq!(ExitReason::Restart.code(), 2);
    }

    #[tokio::test]
    async fn shutdown_request_ends_run_until_with_its_exit_code() {
        let (state, requests) = state_with(default_config());
        let state = Arc::new(state);
        let server = Server {
            state: Arc::clone(&state),
            shutdown_requests: requests,
            tasks: JoinSet::new(),
            cancel: CancellationToken::new(),
            client_addr: None,
        };
        state.request_shutdown(true);
        let reason = server.run_until(std::future::pending::<()>()).await;
        assert_eq!(reason, ExitReason::Restart);
    }

    #[tokio::test]
    async fn stop_signal_ends_run_until_with_exit_code_0() {
        let (state, requests) = state_with(default_config());
        let mut tasks = JoinSet::new();
        let state = Arc::new(state);
        spawn_purge_task(&mut tasks, Arc::clone(&state));
        let server = Server {
            state,
            shutdown_requests: requests,
            tasks,
            cancel: CancellationToken::new(),
            client_addr: None,
        };
        let reason = server.run_until(std::future::ready(())).await;
        assert_eq!(reason, ExitReason::Shutdown);
        assert_eq!(reason.code(), 0);
    }
}
