//! Bridge between the egui UI thread and the async admin client.
//!
//! ```text
//!   UI thread                         "admin-bridge" thread (tokio runtime)
//!   Bridge::connect/request  ──cmd──►  connect loop, backoff, one AdminClient
//!   Bridge::try_recv         ◄─event──  BridgeEvent  (+ ctx.request_repaint())
//! ```
//!
//! The UI never blocks: commands go into an unbounded channel, events come out of a std channel
//! that the UI drains once per frame ([`Bridge::try_recv`]). Every event wakes the UI with
//! `request_repaint`, so an idle window costs nothing.
//!
//! The bridge owns the reconnect policy: after a failed attempt or a lost connection it waits
//! [`backoff_delay`] and tries again, until the user disconnects or changes the target. A
//! rejection (wrong token, wrong protocol version) is not retried, because it cannot fix itself.
//!
//! The UI mirrors the connection in a [`ConnState`] by feeding it every [`BridgeEvent`].

use std::fmt;
use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc as std_mpsc;
use std::time::{Duration, Instant};

use l2_admin_api::{AdminClient, AdminError, Event, Request, Response};
use tokio::sync::mpsc;

/// Upper bound for one connection attempt (TCP connect + handshake).
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const BACKOFF_START: Duration = Duration::from_secs(1);
const BACKOFF_MAX: Duration = Duration::from_secs(30);

/// Delay before the next attempt after `failures` consecutive failures (1, 2, 4 ... 30 s).
pub fn backoff_delay(failures: u32) -> Duration {
    let doublings = failures.saturating_sub(1).min(16);
    BACKOFF_START.saturating_mul(1u32 << doublings).min(BACKOFF_MAX)
}

/// Where to connect.
#[derive(Clone, PartialEq, Eq)]
pub struct Target {
    pub url: String,
    pub token: String,
}

// Hand-written so the token never ends up in a log line.
impl fmt::Debug for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Target")
            .field("url", &self.url)
            .field("token", &"<redacted>")
            .finish()
    }
}

/// What the UI tells the bridge.
#[derive(Debug)]
enum Command {
    /// (Re)connect to this target, dropping any current connection.
    Connect(Target),
    /// Drop the connection and stop retrying.
    Disconnect,
    Request {
        id: u64,
        request: Request,
    },
}

/// What the bridge tells the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeEvent {
    /// An attempt is starting (`attempt` counts from 1 since the last good connection).
    Connecting { attempt: u32 },
    /// Authenticated; `name` is what the endpoint calls itself ("supervisor").
    Connected { name: String },
    /// The attempt failed or the connection was lost; the next attempt follows in `retry_in`.
    Waiting {
        failures: u32,
        error: String,
        retry_in: Duration,
    },
    /// The endpoint refused us (wrong token or version). No retry until the user acts.
    Rejected { reason: String },
    /// The user disconnected.
    Disconnected,
    /// Pushed by the endpoint (log lines, state changes).
    Event(Event),
    /// Answer to [`Bridge::request`] number `id`; `Err` = not connected / connection lost.
    Response {
        id: u64,
        result: Result<Response, String>,
    },
}

/// The connection as the UI shows it. Update it with [`ConnState::apply`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ConnState {
    #[default]
    Disconnected,
    Connecting {
        attempt: u32,
    },
    Connected {
        name: String,
    },
    Waiting {
        failures: u32,
        error: String,
        retry_at: Instant,
    },
    Rejected {
        reason: String,
    },
}

impl ConnState {
    /// Folds one bridge event into the state. `now` is passed in so tests don't need a clock.
    pub fn apply(&mut self, event: &BridgeEvent, now: Instant) {
        match event {
            BridgeEvent::Connecting { attempt } => *self = Self::Connecting { attempt: *attempt },
            BridgeEvent::Connected { name } => *self = Self::Connected { name: name.clone() },
            BridgeEvent::Waiting {
                failures,
                error,
                retry_in,
            } => {
                *self = Self::Waiting {
                    failures: *failures,
                    error: error.clone(),
                    retry_at: now + *retry_in,
                }
            }
            BridgeEvent::Rejected { reason } => {
                *self = Self::Rejected {
                    reason: reason.clone(),
                }
            }
            BridgeEvent::Disconnected => *self = Self::Disconnected,
            BridgeEvent::Event(_) | BridgeEvent::Response { .. } => {}
        }
    }

    #[allow(dead_code)] // for views that need a live connection
    pub fn is_connected(&self) -> bool {
        matches!(self, Self::Connected { .. })
    }

    /// True while the bridge is working on (or waiting to retry) a connection.
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            Self::Connecting { .. } | Self::Connected { .. } | Self::Waiting { .. }
        )
    }
}

/// UI-side handle. Cheap calls only; nothing here blocks.
pub struct Bridge {
    cmd_tx: mpsc::UnboundedSender<Command>,
    events: std_mpsc::Receiver<BridgeEvent>,
    next_request_id: AtomicU64,
}

impl Bridge {
    /// Starts the background thread. `ctx` is repainted whenever an event is queued
    /// (`None` in tests). The thread ends when the `Bridge` is dropped.
    pub fn spawn(ctx: Option<egui::Context>) -> io::Result<Self> {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (event_tx, events) = std_mpsc::channel();
        let emit = Emitter { tx: event_tx, ctx };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        std::thread::Builder::new()
            .name("admin-bridge".into())
            .spawn(move || runtime.block_on(drive(cmd_rx, emit)))?;
        Ok(Self {
            cmd_tx,
            events,
            next_request_id: AtomicU64::new(1),
        })
    }

    /// Connects (or reconnects) and keeps the connection up with backoff.
    pub fn connect(&self, target: Target) {
        let _ = self.cmd_tx.send(Command::Connect(target));
    }

    /// Drops the connection and stops retrying.
    pub fn disconnect(&self) {
        let _ = self.cmd_tx.send(Command::Disconnect);
    }

    /// Sends a request; the answer arrives later as [`BridgeEvent::Response`] with the returned id.
    pub fn request(&self, request: Request) -> u64 {
        let id = self.next_request_id.fetch_add(1, Ordering::Relaxed);
        let _ = self.cmd_tx.send(Command::Request { id, request });
        id
    }

    /// Next queued event, if any. Call in a loop once per frame.
    pub fn try_recv(&self) -> Option<BridgeEvent> {
        self.events.try_recv().ok()
    }
}

#[derive(Clone)]
struct Emitter {
    tx: std_mpsc::Sender<BridgeEvent>,
    ctx: Option<egui::Context>,
}

impl Emitter {
    fn emit(&self, event: BridgeEvent) {
        // A closed channel means the UI is gone; nothing left to tell.
        let _ = self.tx.send(event);
        if let Some(ctx) = &self.ctx {
            ctx.request_repaint();
        }
    }
}

/// What the connect loop does next.
enum Cycle {
    /// Start over with a new target.
    Next(Target),
    /// Stay disconnected until the next `Connect`.
    Idle,
    /// The UI side is gone; end the thread.
    Quit,
}

/// Top level: idle until a `Connect` arrives, then run the connect loop for it.
async fn drive(mut cmd_rx: mpsc::UnboundedReceiver<Command>, emit: Emitter) {
    let mut cycle = Cycle::Idle;
    loop {
        cycle = match cycle {
            Cycle::Quit => return,
            Cycle::Next(target) => connect_loop(target, &mut cmd_rx, &emit).await,
            Cycle::Idle => interrupted(&mut cmd_rx, &emit).await,
        };
    }
}

/// Waits for a command that ends the current wait (`Connect`, `Disconnect`, channel closed).
/// Requests that arrive meanwhile are answered with "not connected". Cancel-safe.
async fn interrupted(cmd_rx: &mut mpsc::UnboundedReceiver<Command>, emit: &Emitter) -> Cycle {
    loop {
        match cmd_rx.recv().await {
            None => return Cycle::Quit,
            Some(Command::Connect(target)) => return Cycle::Next(target),
            Some(Command::Disconnect) => {
                emit.emit(BridgeEvent::Disconnected);
                return Cycle::Idle;
            }
            Some(Command::Request { id, .. }) => emit.emit(BridgeEvent::Response {
                id,
                result: Err("not connected".into()),
            }),
        }
    }
}

/// Connect, serve the session, and on failure wait and try again. Returns when the user (or
/// the closing UI) interrupts, or when the endpoint rejects us.
async fn connect_loop(
    target: Target,
    cmd_rx: &mut mpsc::UnboundedReceiver<Command>,
    emit: &Emitter,
) -> Cycle {
    let mut failures: u32 = 0;
    loop {
        emit.emit(BridgeEvent::Connecting {
            attempt: failures + 1,
        });
        let attempt = tokio::time::timeout(CONNECT_TIMEOUT, AdminClient::connect(&target.url, &target.token));
        let outcome = tokio::select! {
            outcome = attempt => outcome,
            cycle = interrupted(cmd_rx, emit) => return cycle,
        };
        let error = match outcome {
            Ok(Ok((client, events))) => {
                emit.emit(BridgeEvent::Connected {
                    name: client.endpoint_name().to_string(),
                });
                failures = 0;
                match session(client, events, cmd_rx, emit).await {
                    Ended::Lost(error) => error,
                    Ended::Cycle(cycle) => return cycle,
                }
            }
            Ok(Err(AdminError::Rejected(reason))) => {
                emit.emit(BridgeEvent::Rejected { reason });
                return Cycle::Idle;
            }
            Ok(Err(e)) => e.to_string(),
            Err(_elapsed) => "connection attempt timed out".to_string(),
        };
        failures += 1;
        let retry_in = backoff_delay(failures);
        emit.emit(BridgeEvent::Waiting {
            failures,
            error,
            retry_in,
        });
        tokio::select! {
            () = tokio::time::sleep(retry_in) => {}
            cycle = interrupted(cmd_rx, emit) => return cycle,
        }
    }
}

enum Ended {
    /// The connection dropped; the string says why.
    Lost(String),
    Cycle(Cycle),
}

/// Runs while connected: forwards pushed events to the UI and requests to the endpoint.
async fn session(
    client: AdminClient,
    mut events: mpsc::UnboundedReceiver<Event>,
    cmd_rx: &mut mpsc::UnboundedReceiver<Command>,
    emit: &Emitter,
) -> Ended {
    let client = Arc::new(client);
    loop {
        tokio::select! {
            event = events.recv() => match event {
                Some(event) => emit.emit(BridgeEvent::Event(event)),
                // The reader task ended: the other side closed the connection.
                None => return Ended::Lost("connection closed".into()),
            },
            command = cmd_rx.recv() => match command {
                None => return Ended::Cycle(Cycle::Quit),
                Some(Command::Connect(target)) => return Ended::Cycle(Cycle::Next(target)),
                Some(Command::Disconnect) => {
                    emit.emit(BridgeEvent::Disconnected);
                    return Ended::Cycle(Cycle::Idle);
                }
                Some(Command::Request { id, request }) => {
                    // Requests are answered out of order, so each gets its own task.
                    let (client, emit) = (Arc::clone(&client), emit.clone());
                    tokio::spawn(async move {
                        let result = client.request(request).await.map_err(|e| e.to_string());
                        emit.emit(BridgeEvent::Response { id, result });
                    });
                }
            },
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)] // test helpers may panic
mod tests {
    use l2_admin_api::{AdminHandler, LogLevel, LogLine, Session, Token, serve};
    use tokio::net::TcpListener;

    use super::*;

    #[test]
    fn backoff_doubles_up_to_the_cap() {
        let secs: Vec<u64> = (1..=8).map(|n| backoff_delay(n).as_secs()).collect();
        assert_eq!(secs, [1, 2, 4, 8, 16, 30, 30, 30]);
        assert_eq!(backoff_delay(0), Duration::from_secs(1));
        assert_eq!(backoff_delay(u32::MAX), Duration::from_secs(30));
    }

    #[test]
    fn conn_state_follows_events() {
        let now = Instant::now();
        let mut state = ConnState::default();
        assert_eq!(state, ConnState::Disconnected);
        assert!(!state.is_active());

        state.apply(&BridgeEvent::Connecting { attempt: 1 }, now);
        assert_eq!(state, ConnState::Connecting { attempt: 1 });
        assert!(state.is_active() && !state.is_connected());

        state.apply(
            &BridgeEvent::Connected {
                name: "supervisor".into(),
            },
            now,
        );
        assert!(state.is_connected());

        state.apply(
            &BridgeEvent::Waiting {
                failures: 1,
                error: "connection closed".into(),
                retry_in: Duration::from_secs(2),
            },
            now,
        );
        assert_eq!(
            state,
            ConnState::Waiting {
                failures: 1,
                error: "connection closed".into(),
                retry_at: now + Duration::from_secs(2),
            }
        );
        assert!(state.is_active());

        state.apply(
            &BridgeEvent::Rejected {
                reason: "invalid token".into(),
            },
            now,
        );
        assert!(!state.is_active() && !state.is_connected());

        // Data events don't touch the connection state.
        let before = state.clone();
        state.apply(
            &BridgeEvent::Response {
                id: 1,
                result: Ok(Response::Ok),
            },
            now,
        );
        assert_eq!(state, before);

        state.apply(&BridgeEvent::Disconnected, now);
        assert_eq!(state, ConnState::Disconnected);
    }

    #[test]
    fn target_debug_hides_the_token() {
        let target = Target {
            url: "ws://x".into(),
            token: "hunter2".into(),
        };
        assert!(!format!("{target:?}").contains("hunter2"));
    }

    struct TestHandler;

    impl AdminHandler for TestHandler {
        async fn handle(&self, request: Request, session: &Session) -> Response {
            match request {
                Request::SubscribeLogs { .. } => {
                    session.send_event(Event::LogLine {
                        line: LogLine {
                            server: "login".into(),
                            seq: 1,
                            ts_ms: 0,
                            level: Some(LogLevel::Info),
                            target: None,
                            message: "hello log".into(),
                        },
                    });
                    Response::Ok
                }
                _ => Response::Servers { servers: vec![] },
            }
        }
    }

    fn target(addr: std::net::SocketAddr, token: &str) -> Target {
        Target {
            url: format!("ws://{addr}"),
            token: token.into(),
        }
    }

    async fn start_server(listener: TcpListener) {
        tokio::spawn(serve(
            listener,
            "test".into(),
            Token::new("secret"),
            Arc::new(TestHandler),
        ));
    }

    /// An address nothing listens on (bind, remember, drop).
    async fn closed_addr() -> std::net::SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        listener.local_addr().unwrap()
    }

    /// Polls the bridge until `pick` accepts an event; fails after 10 s.
    async fn wait_for<T>(bridge: &Bridge, mut pick: impl FnMut(&BridgeEvent) -> Option<T>) -> T {
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            while let Some(event) = bridge.try_recv() {
                if let Some(found) = pick(&event) {
                    return found;
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("timed out waiting for a bridge event");
    }

    #[tokio::test]
    async fn connects_requests_and_receives_events() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        start_server(listener).await;

        let bridge = Bridge::spawn(None).unwrap();
        bridge.connect(target(addr, "secret"));
        let name = wait_for(&bridge, |e| match e {
            BridgeEvent::Connected { name } => Some(name.clone()),
            _ => None,
        })
        .await;
        assert_eq!(name, "test");

        let id = bridge.request(Request::ListServers);
        let result = wait_for(&bridge, |e| match e {
            BridgeEvent::Response { id: got, result } if *got == id => Some(result.clone()),
            _ => None,
        })
        .await;
        assert_eq!(result, Ok(Response::Servers { servers: vec![] }));

        bridge.request(Request::SubscribeLogs {
            server: None,
            since: None,
        });
        let line = wait_for(&bridge, |e| match e {
            BridgeEvent::Event(Event::LogLine { line }) => Some(line.message.clone()),
            _ => None,
        })
        .await;
        assert_eq!(line, "hello log");

        bridge.disconnect();
        wait_for(&bridge, |e| (*e == BridgeEvent::Disconnected).then_some(())).await;
    }

    #[tokio::test]
    async fn wrong_token_is_rejected_without_retrying() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        start_server(listener).await;

        let bridge = Bridge::spawn(None).unwrap();
        bridge.connect(target(addr, "nope"));
        let reason = wait_for(&bridge, |e| match e {
            BridgeEvent::Rejected { reason } => Some(reason.clone()),
            _ => None,
        })
        .await;
        assert_eq!(reason, "invalid token");

        // No further attempt may follow.
        tokio::time::sleep(Duration::from_millis(1500)).await;
        assert_eq!(bridge.try_recv(), None);
    }

    #[tokio::test]
    async fn retries_with_backoff_until_the_server_appears() {
        let addr = closed_addr().await;
        let bridge = Bridge::spawn(None).unwrap();
        bridge.connect(target(addr, "secret"));

        let (failures, retry_in) = wait_for(&bridge, |e| match e {
            BridgeEvent::Waiting {
                failures, retry_in, ..
            } => Some((*failures, *retry_in)),
            _ => None,
        })
        .await;
        assert_eq!((failures, retry_in), (1, Duration::from_secs(1)));

        // Now the supervisor comes up on that address; the next attempt must find it.
        let listener = TcpListener::bind(addr).await.unwrap();
        start_server(listener).await;
        wait_for(&bridge, |e| {
            matches!(e, BridgeEvent::Connected { .. }).then_some(())
        })
        .await;
    }

    #[tokio::test]
    async fn disconnect_stops_retrying_and_requests_fail_when_offline() {
        let addr = closed_addr().await;
        let bridge = Bridge::spawn(None).unwrap();
        bridge.connect(target(addr, "secret"));
        wait_for(&bridge, |e| {
            matches!(e, BridgeEvent::Waiting { .. }).then_some(())
        })
        .await;

        bridge.disconnect();
        wait_for(&bridge, |e| (*e == BridgeEvent::Disconnected).then_some(())).await;

        let id = bridge.request(Request::ListServers);
        let result = wait_for(&bridge, |e| match e {
            BridgeEvent::Response { id: got, result } if *got == id => Some(result.clone()),
            _ => None,
        })
        .await;
        assert_eq!(result, Err("not connected".to_string()));

        // Idle: the backoff timer must not fire a new attempt.
        tokio::time::sleep(Duration::from_millis(1500)).await;
        assert_eq!(bridge.try_recv(), None);
    }
}
