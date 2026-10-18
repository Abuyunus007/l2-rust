//! Starting and stopping the in-process proxy (D-35) and the control row above the packet list.
//!
//! ```text
//!   UI thread                              "packet-proxy" runtime (2 worker threads)
//!   ProxyController::start/stop  ──spawn──►  Proxy::start / Proxy::stop
//!   ProxyController::poll        ◄─ProxyMsg─  drain task: records in batches, state changes
//! ```
//!
//! The proxy gets its own small runtime instead of the admin bridge's one: the bridge runtime is
//! a single thread that is busy with the supervisor connection, and the proxy needs
//! `spawn_blocking` (RSA key generation) plus a task per connection. Nothing here blocks the UI:
//! every call returns at once and the results come back through [`ProxyController::poll`], which
//! the view calls once per frame. The drain task wakes the UI with `request_repaint`.

use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::mpsc;

use l2_proxy::{Proxy, ProxyConfig, ProxyError, Record};
use tokio::runtime::{Builder, Handle, Runtime};

use crate::settings::PacketSettings;

/// How many records one message carries at most.
const BATCH: usize = 512;

/// What the runtime tells the UI.
enum ProxyMsg {
    Started(Box<Proxy>),
    StartFailed(String),
    Stopped,
    Records(Vec<Record>),
}

/// What the proxy is doing, as the control row shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunState {
    Stopped,
    Starting,
    Running { local_addr: SocketAddr },
    Stopping,
}

/// Owns the proxy and the runtime it runs on. Lives on the UI thread.
pub struct ProxyController {
    runtime: Option<Runtime>,
    proxy: Option<Box<Proxy>>,
    state: RunState,
    /// The last start failure, until the next start.
    error: Option<String>,
    tx: mpsc::Sender<ProxyMsg>,
    rx: mpsc::Receiver<ProxyMsg>,
}

impl Default for ProxyController {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            runtime: None,
            proxy: None,
            state: RunState::Stopped,
            error: None,
            tx,
            rx,
        }
    }
}

impl Drop for ProxyController {
    fn drop(&mut self) {
        // The window is closing: end the proxy tasks without waiting for them.
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

impl ProxyController {
    pub fn state(&self) -> RunState {
        self.state
    }

    /// Why the last start failed, in words for the user.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// `(server id, local port, real address)` of the game relays opened so far.
    pub fn game_relays(&self) -> Vec<(u8, u16, SocketAddr)> {
        self.proxy.as_ref().map(|p| p.game_relays()).unwrap_or_default()
    }

    /// Starts the proxy. Does nothing unless it is stopped. `ctx` is repainted whenever the
    /// proxy has news (`None` in tests).
    pub fn start(&mut self, config: ProxyConfig, ctx: Option<egui::Context>) {
        if self.state != RunState::Stopped {
            return;
        }
        let runtime = match self.runtime_handle() {
            Ok(runtime) => runtime,
            Err(e) => {
                self.error = Some(format!("Cannot start the proxy's worker threads: {e}"));
                return;
            }
        };
        self.error = None;
        self.state = RunState::Starting;
        let tx = self.tx.clone();
        let listen = config.listen;
        runtime.spawn(async move {
            let notify = move || {
                if let Some(ctx) = &ctx {
                    ctx.request_repaint();
                }
            };
            let (records_tx, mut records_rx) = tokio::sync::mpsc::unbounded_channel();
            match Proxy::start(config, records_tx).await {
                Ok(proxy) => {
                    let _ = tx.send(ProxyMsg::Started(Box::new(proxy)));
                    notify();
                }
                Err(e) => {
                    let _ = tx.send(ProxyMsg::StartFailed(describe_start_error(listen, &e)));
                    notify();
                    return;
                }
            }
            // Ends when the proxy and every connection of it have dropped their senders.
            let mut batch = Vec::new();
            while records_rx.recv_many(&mut batch, BATCH).await > 0 {
                if tx.send(ProxyMsg::Records(std::mem::take(&mut batch))).is_err() {
                    return;
                }
                notify();
            }
        });
    }

    /// Stops the proxy. Does nothing unless it is running.
    pub fn stop(&mut self, ctx: Option<egui::Context>) {
        let (Some(proxy), Some(runtime)) = (self.proxy.take(), self.runtime.as_ref()) else {
            return;
        };
        self.state = RunState::Stopping;
        let tx = self.tx.clone();
        runtime.spawn(async move {
            proxy.stop().await;
            let _ = tx.send(ProxyMsg::Stopped);
            if let Some(ctx) = &ctx {
                ctx.request_repaint();
            }
        });
    }

    /// Takes in what the runtime has reported and returns the new records, oldest first.
    pub fn poll(&mut self) -> Vec<Record> {
        let mut records = Vec::new();
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                ProxyMsg::Started(proxy) => {
                    self.state = RunState::Running {
                        local_addr: proxy.local_addr(),
                    };
                    self.proxy = Some(proxy);
                }
                ProxyMsg::StartFailed(error) => {
                    self.state = RunState::Stopped;
                    self.error = Some(error);
                }
                ProxyMsg::Stopped => self.state = RunState::Stopped,
                ProxyMsg::Records(mut batch) => records.append(&mut batch),
            }
        }
        records
    }

    fn runtime_handle(&mut self) -> io::Result<Handle> {
        if self.runtime.is_none() {
            self.runtime = Some(
                Builder::new_multi_thread()
                    .worker_threads(2)
                    .thread_name("packet-proxy")
                    .enable_all()
                    .build()?,
            );
        }
        match &self.runtime {
            Some(runtime) => Ok(runtime.handle().clone()),
            None => Err(io::Error::other("runtime missing")),
        }
    }
}

/// The proxy configuration for what the user typed.
pub fn parse_config(settings: &PacketSettings) -> Result<ProxyConfig, String> {
    let listen: SocketAddr = settings.listen.trim().parse().map_err(|_| {
        format!(
            "Listen address {:?} must look like 127.0.0.1:2108",
            settings.listen.trim()
        )
    })?;
    let upstream: SocketAddr = settings.upstream.trim().parse().map_err(|_| {
        format!(
            "Login server address {:?} must look like 127.0.0.1:2106",
            settings.upstream.trim()
        )
    })?;
    let mut config = ProxyConfig::new(listen, upstream);
    let advertise = settings.advertise_ip.trim();
    if !advertise.is_empty() {
        config.advertise_ip = Some(
            advertise
                .parse::<Ipv4Addr>()
                .map_err(|_| format!("Advertise IP {advertise:?} must be an IPv4 address"))?,
        );
    }
    Ok(config)
}

/// A start failure in words for the user. Binding is the usual one: the default login port is
/// taken by the real login server.
pub fn describe_start_error(listen: SocketAddr, error: &ProxyError) -> String {
    match error {
        ProxyError::Io(e) if e.kind() == io::ErrorKind::AddrInUse => format!(
            "Cannot listen on {listen}: the port is already in use. Another proxy or the login \
             server may be running there; choose another listen port."
        ),
        ProxyError::Io(e) if e.kind() == io::ErrorKind::PermissionDenied => format!(
            "Cannot listen on {listen}: not allowed (the port may be reserved by Windows or need \
             more rights). Choose another listen port."
        ),
        ProxyError::Io(e) => format!("Cannot listen on {listen}: {e}"),
        ProxyError::Config(message) => format!("Invalid proxy settings: {message}"),
        other => format!("Cannot start the proxy: {other}"),
    }
}

/// The control row: addresses, start/stop, and the failure text. Returns what the user asked for.
pub fn controls_ui(ui: &mut egui::Ui, settings: &mut PacketSettings, state: RunState) -> ControlAction {
    let mut action = ControlAction::None;
    let editable = state == RunState::Stopped;
    ui.horizontal_wrapped(|ui| {
        ui.add_enabled_ui(editable, |ui| {
            ui.label("Client connects to");
            ui.add(egui::TextEdit::singleline(&mut settings.listen).desired_width(130.0))
                .on_hover_text("Point the game client's login address at this ip:port");
            ui.label("Login server");
            ui.add(egui::TextEdit::singleline(&mut settings.upstream).desired_width(130.0))
                .on_hover_text("The real login server (Java or Rust) the proxy forwards to");
            ui.label("Advertise IP");
            ui.add(
                egui::TextEdit::singleline(&mut settings.advertise_ip)
                    .hint_text("auto")
                    .desired_width(100.0),
            )
            .on_hover_text(
                "IPv4 address the client uses to reach the proxy's game ports. Empty = the IP \
                 above; needed when listening on 0.0.0.0",
            );
        });
        match state {
            RunState::Stopped => {
                if ui.button("Start proxy").clicked() {
                    action = ControlAction::Start;
                }
            }
            RunState::Starting => {
                ui.add_enabled(false, egui::Button::new("Starting..."));
            }
            RunState::Running { .. } => {
                if ui.button("Stop proxy").clicked() {
                    action = ControlAction::Stop;
                }
            }
            RunState::Stopping => {
                ui.add_enabled(false, egui::Button::new("Stopping..."));
            }
        }
    });
    action
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlAction {
    None,
    Start,
    Stop,
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;
    use std::time::{Duration, Instant};

    use super::*;

    fn poll_until(controller: &mut ProxyController, done: impl Fn(&ProxyController) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(60);
        while !done(controller) {
            assert!(
                Instant::now() < deadline,
                "timed out in state {:?}",
                controller.state()
            );
            let _ = controller.poll();
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Port 0 on loopback: never one of the real ports. The upstream is never contacted
    /// because no client connects.
    fn test_config() -> ProxyConfig {
        ProxyConfig::new("127.0.0.1:0".parse().unwrap(), "127.0.0.1:9".parse().unwrap())
    }

    #[test]
    fn starts_and_stops_the_proxy() {
        let mut controller = ProxyController::default();
        assert_eq!(controller.state(), RunState::Stopped);
        controller.start(test_config(), None);
        assert_eq!(controller.state(), RunState::Starting);
        poll_until(&mut controller, |c| matches!(c.state(), RunState::Running { .. }));
        let RunState::Running { local_addr } = controller.state() else {
            unreachable!()
        };
        assert!(local_addr.port() != 0, "a real port was assigned");
        assert!(controller.error().is_none());

        // A second start while running changes nothing.
        controller.start(test_config(), None);
        assert!(matches!(controller.state(), RunState::Running { .. }));

        controller.stop(None);
        assert_eq!(controller.state(), RunState::Stopping);
        poll_until(&mut controller, |c| c.state() == RunState::Stopped);
        assert!(controller.game_relays().is_empty());

        // And it can start again.
        controller.start(test_config(), None);
        poll_until(&mut controller, |c| matches!(c.state(), RunState::Running { .. }));
        controller.stop(None);
        poll_until(&mut controller, |c| c.state() == RunState::Stopped);
    }

    #[test]
    fn a_taken_port_gives_a_clear_error_and_the_proxy_stays_stopped() {
        // Occupy an ephemeral port, then ask the proxy for exactly that one.
        let taken = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut config = test_config();
        config.listen = taken.local_addr().unwrap();
        let mut controller = ProxyController::default();
        controller.start(config, None);
        poll_until(&mut controller, |c| c.error().is_some());
        assert_eq!(controller.state(), RunState::Stopped);
        let error = controller.error().unwrap();
        assert!(error.contains("already in use"), "{error}");
        assert!(
            error.contains(&taken.local_addr().unwrap().to_string()),
            "{error}"
        );

        // Freeing the port and starting again clears the error.
        drop(taken);
        controller.start(test_config(), None);
        assert!(controller.error().is_none());
        poll_until(&mut controller, |c| matches!(c.state(), RunState::Running { .. }));
        controller.stop(None);
        poll_until(&mut controller, |c| c.state() == RunState::Stopped);
    }

    #[test]
    fn stop_does_nothing_when_not_running() {
        let mut controller = ProxyController::default();
        controller.stop(None);
        assert_eq!(controller.state(), RunState::Stopped);
    }

    #[test]
    fn parses_what_the_user_typed() {
        let config = parse_config(&PacketSettings::default()).unwrap();
        assert_eq!(config.listen, "127.0.0.1:2108".parse().unwrap());
        assert_eq!(config.upstream, "127.0.0.1:2106".parse().unwrap());
        assert_eq!(config.advertise_ip, None);

        let settings = PacketSettings {
            listen: " 0.0.0.0:2108 ".into(),
            advertise_ip: " 192.168.1.20 ".into(),
            ..PacketSettings::default()
        };
        let config = parse_config(&settings).unwrap();
        assert_eq!(config.listen, "0.0.0.0:2108".parse().unwrap());
        assert_eq!(config.advertise_ip, Some(Ipv4Addr::new(192, 168, 1, 20)));
    }

    #[test]
    fn bad_input_names_the_field() {
        let bad = |listen: &str, upstream: &str, advertise: &str| {
            parse_config(&PacketSettings {
                listen: listen.into(),
                upstream: upstream.into(),
                advertise_ip: advertise.into(),
                ..PacketSettings::default()
            })
            .unwrap_err()
        };
        assert!(bad("localhost", "127.0.0.1:2106", "").contains("Listen address"));
        assert!(bad("127.0.0.1:2108", "", "").contains("Login server"));
        assert!(bad("127.0.0.1:2108", "127.0.0.1:2106", "::1").contains("Advertise IP"));
    }

    #[test]
    fn start_errors_are_worded_for_the_user() {
        let listen: SocketAddr = "127.0.0.1:2108".parse().unwrap();
        let in_use = ProxyError::Io(io::Error::from(io::ErrorKind::AddrInUse));
        assert!(describe_start_error(listen, &in_use).contains("already in use"));
        let denied = ProxyError::Io(io::Error::from(io::ErrorKind::PermissionDenied));
        assert!(describe_start_error(listen, &denied).contains("not allowed"));
        let config = ProxyError::Config("set advertise_ip".into());
        assert!(describe_start_error(listen, &config).contains("set advertise_ip"));
    }
}
