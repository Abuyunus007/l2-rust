//! Process control: one small actor task per configured server.
//!
//! Java: `dist/login/LoginServerTask.sh`, `dist/game/GameServer.sh`, `GameServer.vbs` (run the
//! command in its working directory, start it again on exit code 2, collect the output).
//!
//! # State machine of one server
//! ```text
//! Stopped/Crashed --Start--> Starting --spawned--> Running --exit code == restart code--> Starting
//!                                                    |  \--exit 0--> Stopped
//!                                                    |   \--any other exit--> Crashed
//!                                                    \--Stop/Restart--> Stopping --> Stopped (/ Starting again)
//! ```
//! The actor owns the child; the handle only sends commands and reads the published status.
//!
//! # Graceful stop
//! * Unix: `kill -TERM <pid>` (the `kill` tool; no `unsafe` / libc needed). The child runs in its
//!   own process group, so Ctrl+C in the supervisor's terminal doesn't hit it directly.
//! * Windows: `taskkill /PID <pid>` without `/F`. That only works for processes that own a window;
//!   a console process (the JVM, the Rust servers) refuses it, and Windows has no console signal
//!   that makes a JVM run its shutdown hooks. In that case the process is killed at once instead
//!   of waiting for the timeout. NOTE: servers should get an admin `shutdown` request later.
//! * After `StopTimeoutSecs` without an exit, the process is killed.

use std::future::Future;
use std::pin::Pin;
use std::process::Stdio;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use l2_admin_api::{Event, LogLevel, ServerState, ServerStatus};
use tokio::process::{Child, Command};
use tokio::sync::{broadcast, mpsc, oneshot};

use crate::config::{ServerConfig, SupervisorConfig};
use crate::logs::LogStore;

type Reply = oneshot::Sender<Result<(), String>>;

enum Cmd {
    Start(Reply),
    Stop(Reply),
    Restart(Reply),
}

#[derive(Debug)]
struct StatusInner {
    state: ServerState,
    pid: Option<u32>,
    started_at: Option<Instant>,
    last_exit_code: Option<i32>,
}

/// Handle to one server's actor. Cheap to clone.
#[derive(Clone)]
pub struct ServerHandle {
    name: Arc<str>,
    cmd_tx: mpsc::UnboundedSender<Cmd>,
    status: Arc<Mutex<StatusInner>>,
    graceful: GracefulSlot,
}

/// Asks a server to exit cleanly (e.g. an admin `shutdown` request). `true` = it accepted.
pub type GracefulStop = Arc<dyn Fn() -> Pin<Box<dyn Future<Output = bool> + Send>> + Send + Sync>;
type GracefulSlot = Arc<Mutex<Option<GracefulStop>>>;

impl ServerHandle {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn status(&self) -> ServerStatus {
        snapshot(&self.name, &lock(&self.status))
    }

    /// Starts the server. Errors when it is already running or cannot be spawned.
    pub async fn start(&self) -> Result<(), String> {
        self.send(Cmd::Start).await
    }

    /// Registers the graceful stop used before signals and kills (D-38).
    pub fn set_graceful_stop(&self, stop: GracefulStop) {
        *self.graceful.lock().unwrap_or_else(PoisonError::into_inner) = Some(stop);
    }

    /// Stops the server and waits until the process is gone. Stopping a stopped server is fine.
    pub async fn stop(&self) -> Result<(), String> {
        self.send(Cmd::Stop).await
    }

    /// Stops (if running) and starts again, without the restart delay.
    pub async fn restart(&self) -> Result<(), String> {
        self.send(Cmd::Restart).await
    }

    async fn send(&self, make: fn(Reply) -> Cmd) -> Result<(), String> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(make(tx))
            .map_err(|_| "supervisor is shutting down".to_string())?;
        rx.await.map_err(|_| "supervisor is shutting down".to_string())?
    }
}

fn lock(status: &Mutex<StatusInner>) -> MutexGuard<'_, StatusInner> {
    status.lock().unwrap_or_else(|e| e.into_inner())
}

fn snapshot(name: &str, inner: &StatusInner) -> ServerStatus {
    ServerStatus {
        name: name.to_string(),
        state: inner.state,
        pid: inner.pid,
        uptime_secs: match (inner.state, inner.started_at) {
            (ServerState::Running | ServerState::Stopping, Some(at)) => Some(at.elapsed().as_secs()),
            _ => None,
        },
        last_exit_code: inner.last_exit_code,
    }
}

/// All supervised servers plus the shared log store and event stream.
pub struct Supervisor {
    servers: Vec<ServerHandle>,
    logs: Arc<LogStore>,
    events: broadcast::Sender<Event>,
}

impl Supervisor {
    /// Creates the actors (idle; nothing is started). Must run inside a tokio runtime.
    pub fn new(config: &SupervisorConfig, logs: Arc<LogStore>) -> Arc<Self> {
        let (events, _) = broadcast::channel(256);
        let servers = config
            .servers
            .iter()
            .map(|server| {
                let status = Arc::new(Mutex::new(StatusInner {
                    state: ServerState::Stopped,
                    pid: None,
                    started_at: None,
                    last_exit_code: None,
                }));
                let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
                let graceful: GracefulSlot = Arc::default();
                let actor = Actor {
                    graceful: Arc::clone(&graceful),
                    cfg: Arc::new(server.clone()),
                    stop_timeout: config.stop_timeout,
                    restart_delay: config.restart_delay,
                    status: Arc::clone(&status),
                    logs: Arc::clone(&logs),
                    events: events.clone(),
                };
                tokio::spawn(actor.run(cmd_rx));
                ServerHandle {
                    graceful,
                    name: server.name.as_str().into(),
                    cmd_tx,
                    status,
                }
            })
            .collect();
        Arc::new(Self {
            servers,
            logs,
            events,
        })
    }

    pub fn logs(&self) -> &Arc<LogStore> {
        &self.logs
    }

    /// `ServerStateChanged` events (and, via [`Self::event_sender`], relayed login status).
    pub fn subscribe_events(&self) -> broadcast::Receiver<Event> {
        self.events.subscribe()
    }

    pub fn event_sender(&self) -> broadcast::Sender<Event> {
        self.events.clone()
    }

    pub fn get(&self, name: &str) -> Option<&ServerHandle> {
        self.servers.iter().find(|s| s.name() == name)
    }

    /// Statuses in configuration order.
    pub fn list(&self) -> Vec<ServerStatus> {
        self.servers.iter().map(ServerHandle::status).collect()
    }

    /// Starts every server whose config says `Autostart = True`. Failures are logged.
    pub async fn autostart(&self, config: &SupervisorConfig) {
        for server in config.servers.iter().filter(|s| s.autostart) {
            if let Some(handle) = self.get(&server.name)
                && let Err(e) = handle.start().await
            {
                tracing::error!("autostart of {} failed: {e}", server.name);
            }
        }
    }

    /// Stops the servers one after the other in reverse configuration order (the game server
    /// before the login server it registered with). Used by the `StopSupervisor` request.
    pub async fn stop_all_reverse(&self) {
        for server in self.servers.iter().rev() {
            if let Err(e) = server.stop().await {
                tracing::warn!("stopping {} failed: {e}", server.name());
            }
        }
    }

    /// Stops every server (in parallel). Used when the supervisor exits.
    pub async fn shutdown(&self) {
        let stops = self.servers.iter().map(|s| {
            let s = s.clone();
            tokio::spawn(async move { s.stop().await })
        });
        for task in stops.collect::<Vec<_>>() {
            let _ = task.await;
        }
    }
}

struct Actor {
    graceful: GracefulSlot,
    cfg: Arc<ServerConfig>,
    stop_timeout: Duration,
    restart_delay: Duration,
    status: Arc<Mutex<StatusInner>>,
    logs: Arc<LogStore>,
    events: broadcast::Sender<Event>,
}

/// What ended one run of the child.
enum Outcome {
    Exited(Option<i32>),
    Stop(Reply),
    Restart(Reply),
}

impl Actor {
    async fn run(self, mut rx: mpsc::UnboundedReceiver<Cmd>) {
        while let Some(cmd) = rx.recv().await {
            match cmd {
                Cmd::Stop(reply) => {
                    let _ = reply.send(Ok(()));
                }
                Cmd::Start(reply) | Cmd::Restart(reply) => self.supervise(&mut rx, reply).await,
            }
        }
    }

    fn set_state(&self, state: ServerState, pid: Option<u32>) {
        let status = {
            let mut inner = lock(&self.status);
            inner.state = state;
            inner.pid = pid;
            match state {
                ServerState::Running => inner.started_at = Some(Instant::now()),
                ServerState::Stopping => {} // uptime keeps counting until the process is gone
                _ => inner.started_at = None,
            }
            snapshot(&self.cfg.name, &inner)
        };
        let _ = self.events.send(Event::ServerStateChanged { status });
    }

    fn set_exit_code(&self, code: Option<i32>) {
        lock(&self.status).last_exit_code = code;
    }

    fn note(&self, level: LogLevel, message: String) {
        self.logs.push_supervisor(&self.cfg.name, level, message);
    }

    /// Runs the server until it is stopped, finished or crashed. `reply` answers the first start.
    async fn supervise(&self, rx: &mut mpsc::UnboundedReceiver<Cmd>, first_reply: Reply) {
        let name = self.cfg.name.as_str();
        let mut reply = Some(first_reply);
        loop {
            self.set_state(ServerState::Starting, None);
            let mut child = match self.spawn() {
                Ok(child) => child,
                Err(e) => {
                    let message = format!("cannot start '{}': {e}", self.cfg.command);
                    tracing::error!("{name}: {message}");
                    self.note(LogLevel::Error, message.clone());
                    self.set_state(ServerState::Crashed, None);
                    if let Some(reply) = reply.take() {
                        let _ = reply.send(Err(message));
                    }
                    return;
                }
            };
            self.set_state(ServerState::Running, child.id());
            tracing::info!("{name}: started (pid {})", child.id().unwrap_or(0));
            self.note(
                LogLevel::Info,
                format!("started (pid {})", child.id().unwrap_or(0)),
            );
            if let Some(reply) = reply.take() {
                let _ = reply.send(Ok(()));
            }

            let outcome = loop {
                tokio::select! {
                    status = child.wait() => break Outcome::Exited(match status {
                        Ok(status) => status.code(),
                        Err(e) => {
                            tracing::error!("{name}: waiting for the process failed: {e}");
                            None
                        }
                    }),
                    cmd = rx.recv() => match cmd {
                        Some(Cmd::Start(r)) => { let _ = r.send(Err("already running".into())); }
                        Some(Cmd::Stop(r)) => break Outcome::Stop(r),
                        Some(Cmd::Restart(r)) => break Outcome::Restart(r),
                        // All handles dropped: the supervisor is going away.
                        None => break Outcome::Stop(oneshot::channel().0),
                    }
                }
            };

            match outcome {
                Outcome::Exited(code) => {
                    self.set_exit_code(code);
                    if code.is_some() && code == self.cfg.restart_on_exit_code {
                        let message = format!(
                            "exited with code {}, restarting in {} s",
                            code.unwrap_or_default(),
                            self.restart_delay.as_secs()
                        );
                        tracing::info!("{name}: {message}");
                        self.note(LogLevel::Info, message);
                        self.set_state(ServerState::Starting, None);
                        // Java scripts sleep before starting again. A command during the delay wins.
                        let sleep = tokio::time::sleep(self.restart_delay);
                        tokio::pin!(sleep);
                        loop {
                            tokio::select! {
                                () = &mut sleep => break,
                                cmd = rx.recv() => match cmd {
                                    Some(Cmd::Start(r)) => { let _ = r.send(Err("already starting".into())); }
                                    Some(Cmd::Restart(r)) => { reply = Some(r); break; }
                                    Some(Cmd::Stop(r)) => {
                                        self.set_state(ServerState::Stopped, None);
                                        let _ = r.send(Ok(()));
                                        return;
                                    }
                                    None => return,
                                }
                            }
                        }
                        continue;
                    }
                    if code == Some(0) {
                        self.note(LogLevel::Info, "exited normally".into());
                        self.set_state(ServerState::Stopped, None);
                    } else {
                        let message = match code {
                            Some(code) => format!("crashed, exit code {code}"),
                            None => "crashed (terminated by a signal)".to_string(),
                        };
                        tracing::warn!("{name}: {message}");
                        self.note(LogLevel::Error, message);
                        self.set_state(ServerState::Crashed, None);
                    }
                    return;
                }
                Outcome::Stop(r) => {
                    self.stop_child(&mut child).await;
                    self.set_state(ServerState::Stopped, None);
                    let _ = r.send(Ok(()));
                    return;
                }
                Outcome::Restart(r) => {
                    self.stop_child(&mut child).await;
                    reply = Some(r);
                }
            }
        }
    }

    fn spawn(&self) -> std::io::Result<Child> {
        let mut command = Command::new(&self.cfg.command);
        command
            .args(&self.cfg.args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // If the supervisor goes away abnormally its children must not linger as orphans.
            .kill_on_drop(true);
        if let Some(dir) = &self.cfg.working_dir {
            command.current_dir(dir);
        }
        // The server uses it when its Admin.ini has no token (D-56).
        if self.cfg.admin_endpoint.is_some() && !self.cfg.admin_token.is_empty() {
            command.env(crate::secret::ADMIN_TOKEN_ENV, &self.cfg.admin_token);
        }
        #[cfg(unix)]
        command.process_group(0);
        #[cfg(windows)]
        command.creation_flags(CREATE_NEW_PROCESS_GROUP);

        let mut child = command.spawn()?;
        if let Some(stdout) = child.stdout.take() {
            tokio::spawn(Arc::clone(&self.logs).capture(self.cfg.name.clone(), stdout));
        }
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(Arc::clone(&self.logs).capture(self.cfg.name.clone(), stderr));
        }
        Ok(child)
    }

    /// Asks the process to exit, kills it after the timeout, and records the exit code.
    async fn stop_child(&self, child: &mut Child) {
        let name = self.cfg.name.as_str();
        self.set_state(ServerState::Stopping, child.id());
        let graceful = self
            .graceful
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        let asked = match (child.id(), graceful) {
            (None, _) => false, // already exited
            (Some(pid), Some(stop)) => {
                if stop().await {
                    self.note(
                        LogLevel::Info,
                        "asked to shut down through its admin endpoint".into(),
                    );
                    true
                } else {
                    request_exit(pid).await
                }
            }
            (Some(pid), None) => request_exit(pid).await,
        };
        let status = if asked {
            match tokio::time::timeout(self.stop_timeout, child.wait()).await {
                Ok(status) => status,
                Err(_) => {
                    let message = format!(
                        "did not stop within {} s, killing it",
                        self.stop_timeout.as_secs()
                    );
                    tracing::warn!("{name}: {message}");
                    self.note(LogLevel::Warn, message);
                    kill(child).await
                }
            }
        } else {
            tracing::info!("{name}: no graceful stop possible, killing it");
            kill(child).await
        };
        match status {
            Ok(status) => {
                self.set_exit_code(status.code());
                self.note(LogLevel::Info, format!("stopped (exit code {:?})", status.code()));
            }
            Err(e) => tracing::warn!("{name}: stopping failed: {e}"),
        }
    }
}

async fn kill(child: &mut Child) -> std::io::Result<std::process::ExitStatus> {
    child.start_kill()?;
    child.wait().await
}

#[cfg(windows)]
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;

/// Sends the "please exit" request. `true` = delivered (the process may still take a while).
#[cfg(unix)]
async fn request_exit(pid: u32) -> bool {
    run_quiet(Command::new("kill").arg("-TERM").arg(pid.to_string())).await
}

#[cfg(windows)]
async fn request_exit(pid: u32) -> bool {
    run_quiet(Command::new("taskkill").arg("/PID").arg(pid.to_string())).await
}

async fn run_quiet(command: &mut Command) -> bool {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .is_ok_and(|status| status.success())
}
