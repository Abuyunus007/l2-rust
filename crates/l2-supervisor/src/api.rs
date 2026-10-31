//! The supervisor's admin API: [`SupervisorHandler`] answers the GUI, [`spawn_admin_link`]
//! connects to a server's own admin endpoint and relays its status.
//!
//! Reference: `crates/l2-admin-api/src/transport.rs` (`serve`, `AdminHandler`, `AdminClient`).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use l2_admin_api::{AdminClient, AdminHandler, Event, Request, Response, Session};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::{Notify, broadcast};
use tokio::task::JoinHandle;

use crate::config::ServerConfig;
use crate::logs::LogStore;
use crate::process::Supervisor;

/// Pause between attempts to reach a server's admin endpoint.
const RECONNECT_DELAY: Duration = Duration::from_secs(2);
/// How often a log forwarder checks whether its client is still there (when no lines flow).
const CLOSED_CHECK_INTERVAL: Duration = Duration::from_secs(5);

/// The connection to one server's admin endpoint, shared with the handler.
#[derive(Clone, Default)]
pub struct AdminLink {
    client: Arc<Mutex<Option<Arc<AdminClient>>>>,
}

impl AdminLink {
    fn current(&self) -> Option<Arc<AdminClient>> {
        self.client.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn set(&self, client: Option<Arc<AdminClient>>) {
        *self.client.lock().unwrap_or_else(|e| e.into_inner()) = client;
    }

    /// Sends `shutdown` to the server's endpoint. `false` when there is no connection or the
    /// server refused, so the caller falls back to signals (D-38).
    pub async fn request_shutdown(&self) -> bool {
        match self.current() {
            Some(client) => matches!(
                client.request(Request::Shutdown { restart: false }).await,
                Ok(Response::Ok)
            ),
            None => false,
        }
    }
}

/// Keeps a connection to `server.admin_endpoint` alive (reconnecting forever) and turns the
/// server's `LoginStatusChanged` events into events for the GUI, with `server` set to our name.
///
/// Returns `None` when the server has no admin endpoint.
pub fn spawn_admin_link(
    server: &ServerConfig,
    events: broadcast::Sender<Event>,
) -> Option<(AdminLink, JoinHandle<()>)> {
    let endpoint = server.admin_endpoint.clone()?;
    let (name, token) = (server.name.clone(), server.admin_token.clone());
    let link = AdminLink::default();
    let shared = link.clone();
    let task = tokio::spawn(async move {
        let url = format!("ws://{endpoint}");
        loop {
            match AdminClient::connect(&url, &token).await {
                Ok((client, mut incoming)) => {
                    tracing::info!("{name}: admin endpoint {endpoint} connected");
                    let client = Arc::new(client);
                    shared.set(Some(Arc::clone(&client)));
                    // The server only pushes events to connections that have sent a request,
                    // and the GUI wants the current picture right away.
                    if let Ok(Response::LoginStatus { status }) =
                        client.request(Request::GetLoginStatus { server: None }).await
                    {
                        let _ = events.send(Event::LoginStatusChanged {
                            server: name.clone(),
                            status,
                        });
                    }
                    while let Some(event) = incoming.recv().await {
                        if let Event::LoginStatusChanged { status, .. } = event {
                            let _ = events.send(Event::LoginStatusChanged {
                                server: name.clone(),
                                status,
                            });
                        }
                    }
                    shared.set(None);
                    tracing::info!("{name}: admin endpoint {endpoint} disconnected");
                }
                Err(e) => tracing::debug!("{name}: admin endpoint {endpoint} not reachable: {e}"),
            }
            tokio::time::sleep(RECONNECT_DELAY).await;
        }
    });
    Some((link, task))
}

pub struct SupervisorHandler {
    supervisor: Arc<Supervisor>,
    logs: Arc<LogStore>,
    links: HashMap<String, AdminLink>,
    /// Clients that have made at least one request: the targets of broadcast events.
    sessions: Arc<Mutex<HashMap<u64, Session>>>,
    /// The running log forwarder of each session (one `SubscribeLogs` at a time).
    log_streams: Mutex<HashMap<u64, JoinHandle<()>>>,
    /// Raised once a `StopSupervisor` has stopped all servers: the binary then exits.
    stop_requested: Arc<Notify>,
}

impl SupervisorHandler {
    /// Creates the handler and the task that pushes state/status events to all clients.
    pub fn new(
        supervisor: Arc<Supervisor>,
        links: HashMap<String, AdminLink>,
        stop_requested: Arc<Notify>,
    ) -> Arc<Self> {
        let sessions: Arc<Mutex<HashMap<u64, Session>>> = Arc::default();
        tokio::spawn(broadcast_events(
            supervisor.subscribe_events(),
            Arc::clone(&sessions),
        ));
        Arc::new(Self {
            logs: Arc::clone(supervisor.logs()),
            supervisor,
            links,
            sessions,
            log_streams: Mutex::default(),
            stop_requested,
        })
    }

    fn streams(&self) -> MutexGuard<'_, HashMap<u64, JoinHandle<()>>> {
        self.log_streams.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn unknown(name: &str) -> Response {
        Response::Error {
            message: format!("unknown server '{name}'"),
        }
    }

    fn control(result: Result<(), String>) -> Response {
        match result {
            Ok(()) => Response::Ok,
            Err(message) => Response::Error { message },
        }
    }

    /// Starts the forwarder task of a `SubscribeLogs`: backlog first, then live lines.
    fn subscribe_logs(
        &self,
        server: Option<String>,
        since: Option<u64>,
        session: &Session,
    ) -> Result<JoinHandle<()>, Response> {
        if let Some(name) = &server
            && self.supervisor.get(name).is_none()
        {
            return Err(Self::unknown(name));
        }
        let (backlog, mut live) = self.logs.subscribe(server.as_deref(), since);
        let session = session.clone();
        Ok(tokio::spawn(async move {
            for line in backlog {
                if !session.send_event(Event::LogLine { line }) {
                    return;
                }
            }
            let mut check = tokio::time::interval(CLOSED_CHECK_INTERVAL);
            loop {
                tokio::select! {
                    line = live.recv() => match line {
                        Ok(line) => {
                            if server.as_deref().is_some_and(|s| s != line.server) {
                                continue;
                            }
                            if !session.send_event(Event::LogLine { line }) {
                                return;
                            }
                        }
                        Err(RecvError::Lagged(missed)) => {
                            tracing::warn!("log subscriber of {} fell behind, {missed} lines dropped", session.peer());
                        }
                        Err(RecvError::Closed) => return,
                    },
                    _ = check.tick() => if session.is_closed() { return },
                }
            }
        }))
    }
}

/// Pushes every event to every registered client; forgets clients that are gone.
async fn broadcast_events(
    mut events: broadcast::Receiver<Event>,
    sessions: Arc<Mutex<HashMap<u64, Session>>>,
) {
    loop {
        match events.recv().await {
            Ok(event) => {
                let mut sessions = sessions.lock().unwrap_or_else(|e| e.into_inner());
                sessions.retain(|_, session| session.send_event(event.clone()));
            }
            Err(RecvError::Lagged(missed)) => tracing::warn!("{missed} state events dropped"),
            Err(RecvError::Closed) => return,
        }
    }
}

impl AdminHandler for SupervisorHandler {
    async fn handle(&self, request: Request, session: &Session) -> Response {
        self.sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(session.id(), session.clone());
        match request {
            Request::ListServers => Response::Servers {
                servers: self.supervisor.list(),
            },
            Request::StartServer { name } => match self.supervisor.get(&name) {
                Some(server) => Self::control(server.start().await),
                None => Self::unknown(&name),
            },
            Request::StopServer { name } => match self.supervisor.get(&name) {
                Some(server) => Self::control(server.stop().await),
                None => Self::unknown(&name),
            },
            Request::RestartServer { name } => match self.supervisor.get(&name) {
                Some(server) => Self::control(server.restart().await),
                None => Self::unknown(&name),
            },
            Request::SubscribeLogs { server, since } => match self.subscribe_logs(server, since, session) {
                Ok(task) => {
                    let mut streams = self.streams();
                    streams.retain(|_, t| !t.is_finished());
                    if let Some(old) = streams.insert(session.id(), task) {
                        old.abort();
                    }
                    Response::Ok
                }
                Err(response) => response,
            },
            Request::UnsubscribeLogs => {
                if let Some(task) = self.streams().remove(&session.id()) {
                    task.abort();
                }
                Response::Ok
            }
            Request::StopSupervisor => {
                // Answer first: the servers can take up to StopTimeoutSecs each to stop.
                let (supervisor, stop_requested) =
                    (Arc::clone(&self.supervisor), Arc::clone(&self.stop_requested));
                tokio::spawn(async move {
                    supervisor.stop_all_reverse().await;
                    stop_requested.notify_one();
                });
                Response::Ok
            }
            // The GUI stops servers with StopServer; `shutdown` is for server endpoints.
            Request::Shutdown { .. } => Response::Error {
                message: "shutdown is sent to a server endpoint; use stop_server here".into(),
            },
            Request::GetLoginStatus { server: None } | Request::GetGameStatus { server: None } => {
                Response::Error {
                    message: "the supervisor needs a server name".into(),
                }
            }
            Request::GetLoginStatus { server: Some(name) } => {
                self.relay(&name, Request::GetLoginStatus { server: None }).await
            }
            Request::GetGameStatus { server: Some(name) } => {
                self.relay(&name, Request::GetGameStatus { server: None }).await
            }
            // Account management belongs to the login server (it owns the `accounts` table).
            Request::Accounts { server: None, .. } => Response::Error {
                message: "the supervisor needs a server name".into(),
            },
            Request::Accounts {
                server: Some(name),
                action,
            } => {
                self.relay(&name, Request::Accounts { server: None, action })
                    .await
            }
        }
    }
}

impl SupervisorHandler {
    /// Forwards a status request to the named server's own admin endpoint and returns its answer.
    /// `request` must carry `server: None`: the server answers for itself.
    async fn relay(&self, name: &str, request: Request) -> Response {
        if self.supervisor.get(name).is_none() {
            return Self::unknown(name);
        }
        let Some(client) = self.links.get(name).and_then(AdminLink::current) else {
            return Response::Error {
                message: format!("no admin connection to '{name}'"),
            };
        };
        match client.request(request).await {
            Ok(response) => response,
            Err(e) => Response::Error {
                message: format!("'{name}' did not answer: {e}"),
            },
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)] // test helpers may panic
mod tests {
    use std::sync::Arc;

    use l2_admin_api::{
        AdminClient, AdminHandler, GamePlayer, GameStatus, Request, Response, Session, Token, serve,
    };
    use tokio::net::TcpListener;

    use crate::Running;
    use crate::config::{ServerConfig, SupervisorConfig};

    /// A stand-in for a game server's admin endpoint.
    struct FakeGameServer;

    impl AdminHandler for FakeGameServer {
        async fn handle(&self, request: Request, _session: &Session) -> Response {
            match request {
                Request::GetGameStatus { server: None } => Response::GameStatus {
                    status: GameStatus {
                        players: vec![GamePlayer {
                            name: "Amy".into(),
                            account: "acc1".into(),
                            level: 80,
                            class_id: 88,
                            ip: None,
                            online_since_ms: 1_700_000_000_000,
                        }],
                    },
                },
                _ => Response::Error {
                    message: "unsupported".into(),
                },
            }
        }
    }

    /// A stand-in for the login server: answers every account action with `Done`.
    struct FakeLoginServer;

    impl AdminHandler for FakeLoginServer {
        async fn handle(&self, request: Request, _session: &Session) -> Response {
            match request {
                Request::Accounts {
                    server: None,
                    action: l2_admin_api::AccountsAction::Ban { login },
                } => Response::Accounts {
                    result: l2_admin_api::AccountsResult::Done {
                        login,
                        access_level: Some(-1),
                    },
                },
                _ => Response::Error {
                    message: "unsupported".into(),
                },
            }
        }
    }

    #[tokio::test]
    async fn account_actions_are_relayed_to_the_named_server() {
        use l2_admin_api::{AccountsAction, AccountsResult};

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let fake_addr = listener.local_addr().unwrap();
        tokio::spawn(serve(
            listener,
            "login".into(),
            Token::new("ls-secret"),
            Arc::new(FakeLoginServer),
        ));
        let mut login = server("login", Some(fake_addr.to_string()));
        login.admin_token = "ls-secret".into();
        let running = Running::start(SupervisorConfig {
            listen: "127.0.0.1:0".parse().unwrap(),
            token: "secret".into(),
            log_buffer_lines: 100,
            stop_timeout: std::time::Duration::from_secs(2),
            restart_delay: std::time::Duration::ZERO,
            servers: vec![login],
        })
        .await
        .unwrap();
        let (client, _events) = AdminClient::connect(&format!("ws://{}", running.local_addr()), "secret")
            .await
            .unwrap();
        let ban = |server: Option<&str>| Request::Accounts {
            server: server.map(Into::into),
            action: AccountsAction::Ban {
                login: "alice".into(),
            },
        };

        let mut answer = None;
        for _ in 0..200 {
            let response = client.request(ban(Some("login"))).await.unwrap();
            if matches!(response, Response::Accounts { .. }) {
                answer = Some(response);
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        assert_eq!(
            answer,
            Some(Response::Accounts {
                result: AccountsResult::Done {
                    login: "alice".into(),
                    access_level: Some(-1)
                }
            })
        );
        assert_eq!(
            client.request(ban(None)).await.unwrap(),
            Response::Error {
                message: "the supervisor needs a server name".into()
            }
        );
        running.shutdown().await;
    }

    fn server(name: &str, admin_endpoint: Option<String>) -> ServerConfig {
        ServerConfig {
            name: name.into(),
            // Never started by this test.
            command: "unused".into(),
            args: vec![],
            working_dir: None,
            admin_endpoint,
            admin_token: "gs-secret".into(),
            autostart: false,
            restart_on_exit_code: None,
        }
    }

    #[tokio::test]
    async fn game_status_is_relayed_to_the_named_server() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let fake_addr = listener.local_addr().unwrap();
        tokio::spawn(serve(
            listener,
            "game".into(),
            Token::new("gs-secret"),
            Arc::new(FakeGameServer),
        ));

        let running = Running::start(SupervisorConfig {
            listen: "127.0.0.1:0".parse().unwrap(),
            token: "secret".into(),
            log_buffer_lines: 100,
            stop_timeout: std::time::Duration::from_secs(2),
            restart_delay: std::time::Duration::ZERO,
            servers: vec![server("game", Some(fake_addr.to_string())), server("plain", None)],
        })
        .await
        .unwrap();
        let (client, _events) = AdminClient::connect(&format!("ws://{}", running.local_addr()), "secret")
            .await
            .unwrap();

        // The link connects in the background; ask until it answers.
        let mut answer = None;
        for _ in 0..200 {
            let response = client
                .request(Request::GetGameStatus {
                    server: Some("game".into()),
                })
                .await
                .unwrap();
            if let Response::GameStatus { status } = response {
                answer = Some(status);
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        assert_eq!(answer.unwrap().players[0].name, "Amy");

        // Errors: no endpoint, unknown server, missing name.
        for (server, expected) in [
            (Some("plain"), "no admin connection to 'plain'"),
            (Some("zzz"), "unknown server 'zzz'"),
            (None, "the supervisor needs a server name"),
        ] {
            let response = client
                .request(Request::GetGameStatus {
                    server: server.map(Into::into),
                })
                .await
                .unwrap();
            assert_eq!(
                response,
                Response::Error {
                    message: expected.into()
                }
            );
        }
        running.shutdown().await;
    }
}
