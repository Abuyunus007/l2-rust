//! The eframe app: window layout (status bar, navigation, central view) and the glue between
//! the bridge, the settings and the views.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use egui::Color32;
use l2_admin_api::{Request, Response};

use crate::bridge::{Bridge, BridgeEvent, ConnState};
use crate::launch::{
    self, LaunchHandle, LaunchState, ResolveInput, Resolved, UiRequest, WaitingVerdict, judge_waiting,
};
use crate::settings::{Settings, Theme};
use crate::views::{ViewId, Views};

/// Everything a view may use besides its own state. Built fresh each frame.
pub struct AppCtx<'a> {
    pub settings: &'a mut Settings,
    pub conn: &'a ConnState,
    pub bridge: &'a Bridge,
    /// The supervisor process: state, run folder, and the requests to start/stop it (G-01).
    pub launch: &'a LaunchHandle,
}

impl AppCtx<'_> {
    /// Finds the run folder, starts the supervisor if nobody listens, and (re)connects.
    pub fn connect(&self) {
        self.launch.connect();
    }

    pub fn disconnect(&self) {
        self.bridge.disconnect();
    }
}

pub struct AdminApp {
    settings: Settings,
    bridge: Bridge,
    conn: ConnState,
    views: Views,
    selected: ViewId,
    applied_theme: Option<Theme>,
    egui_ctx: egui::Context,
    launch: LaunchHandle,
    ui_requests: mpsc::Receiver<UiRequest>,
    resolved_tx: mpsc::Sender<Resolved>,
    resolved_rx: mpsc::Receiver<Resolved>,
    /// The `StopSupervisor` request in flight.
    stop_request: Option<u64>,
    /// Set once per connection loss so a supervisor that cannot start is not relaunched forever.
    relaunched: bool,
}

impl AdminApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> std::io::Result<Self> {
        let settings = Settings::load(cc.storage);
        let bridge = Bridge::spawn(Some(cc.egui_ctx.clone()))?;
        let (launch, ui_requests) = LaunchHandle::new();
        let (resolved_tx, resolved_rx) = mpsc::channel();
        let mut app = Self {
            settings,
            bridge,
            conn: ConnState::default(),
            views: Views::default(),
            selected: ViewId::Connection,
            applied_theme: None,
            egui_ctx: cc.egui_ctx.clone(),
            launch,
            ui_requests,
            resolved_tx,
            resolved_rx,
            stop_request: None,
            relaunched: false,
        };
        if app.settings.should_auto_connect() {
            app.start_resolve();
        }
        Ok(app)
    }

    /// Looks for the run folder and starts the supervisor on a short-lived thread (the port probe
    /// and the spawn would stall the UI); the answer comes back through `resolved_rx`.
    fn start_resolve(&mut self) {
        self.launch.state = LaunchState::Resolving;
        let input = ResolveInput::from_settings(&self.settings);
        let (tx, ctx) = (self.resolved_tx.clone(), self.egui_ctx.clone());
        let spawned = std::thread::Builder::new()
            .name("launcher".into())
            .spawn(move || {
                let _ = tx.send(launch::resolve(&input));
                ctx.request_repaint();
            });
        if let Err(e) = spawned {
            tracing::warn!("cannot start the launcher thread: {e}");
            self.launch.state =
                LaunchState::Problem(l2_supervisor::launcher::Problem::SpawnFailed(e.to_string()));
        }
    }

    /// Stops the other folder's supervisor, then resolves (and starts ours) as usual, all on a
    /// short-lived thread: the other one may take a while to stop its servers.
    fn start_take_over(&mut self, listen: std::net::SocketAddr) {
        self.bridge.disconnect();
        self.launch.state = LaunchState::Resolving;
        let input = ResolveInput::from_settings(&self.settings);
        let (tx, ctx) = (self.resolved_tx.clone(), self.egui_ctx.clone());
        let spawned = std::thread::Builder::new()
            .name("take-over".into())
            .spawn(move || {
                let resolved = match l2_supervisor::takeover::take_over(listen) {
                    Ok(_) => launch::resolve(&input),
                    Err(why) => launch::Resolved {
                        run_folder: None,
                        target: None,
                        listen: None,
                        state: LaunchState::Problem(l2_supervisor::launcher::Problem::TakeOverFailed(why)),
                    },
                };
                let _ = tx.send(resolved);
                ctx.request_repaint();
            });
        if let Err(e) = spawned {
            tracing::warn!("cannot start the take-over thread: {e}");
        }
    }

    fn handle_ui_requests(&mut self) {
        while let Ok(request) = self.ui_requests.try_recv() {
            match request {
                UiRequest::Connect => self.start_resolve(),
                UiRequest::UseRunFolder(folder) => {
                    self.settings.run_folder = folder;
                    self.start_resolve();
                }
                UiRequest::TakeOver(listen) => self.start_take_over(listen),
                UiRequest::StopSupervisor => {
                    self.stop_request = Some(self.bridge.request(Request::StopSupervisor));
                }
            }
        }
        while let Ok(resolved) = self.resolved_rx.try_recv() {
            self.apply_resolved(resolved);
        }
    }

    fn apply_resolved(&mut self, resolved: Resolved) {
        if let Some(folder) = &resolved.run_folder {
            // Remembered for the next start (card: "remembers the folder it chose").
            self.settings.run_folder = folder.to_string_lossy().into_owned();
        }
        self.launch.run_folder = resolved.run_folder;
        self.launch.url = resolved.target.as_ref().map(|t| t.url.clone());
        self.launch.listen = resolved.listen;
        self.launch.state = resolved.state;
        match resolved.target {
            Some(target) => self.bridge.connect(target),
            None => self.bridge.disconnect(),
        }
    }
}

impl AdminApp {
    fn pump_events(&mut self) {
        while let Some(event) = self.bridge.try_recv() {
            self.conn.apply(&event, Instant::now());
            self.track_supervisor(&event);
            if matches!(event, BridgeEvent::Connected { .. }) {
                // The supervisor only starts pushing events to a client after its first
                // request (ADM-02), so ask for the server list right away. The answer reaches
                // the views as an ordinary `BridgeEvent::Response`.
                self.bridge.request(Request::ListServers);
            }
            self.views.dispatch(&event);
        }
    }

    /// Keeps [`LaunchState`] in step with the connection.
    fn track_supervisor(&mut self, event: &BridgeEvent) {
        match event {
            BridgeEvent::Connected { .. } => {
                self.launch.state = LaunchState::Running;
                self.relaunched = false;
            }
            BridgeEvent::Waiting { failures, error, .. } => {
                if self.launch.state == LaunchState::Stopped {
                    // We stopped it on purpose: the dropped connection is expected, don't retry.
                    self.bridge.disconnect();
                    return;
                }
                let pid_alive = || {
                    self.launch.run_folder.as_ref().is_some_and(|dir| {
                        let ini = dir.join(l2_supervisor::launcher::INI_NAME);
                        l2_supervisor::pidfile::read(&l2_supervisor::pidfile::path_for(&ini))
                            .is_some_and(|info| info.is_alive())
                    })
                };
                match judge_waiting(
                    &self.launch.state,
                    *failures,
                    error,
                    self.launch.listen,
                    pid_alive,
                ) {
                    WaitingVerdict::Keep => {}
                    WaitingVerdict::Become(state) => self.launch.state = state,
                    WaitingVerdict::Relaunch if !self.relaunched => {
                        self.relaunched = true;
                        self.start_resolve();
                    }
                    WaitingVerdict::Relaunch => {}
                }
            }
            BridgeEvent::Response { id, result } if self.stop_request == Some(*id) => {
                self.stop_request = None;
                match result {
                    Ok(Response::Ok) => self.launch.state = LaunchState::Stopped,
                    Ok(Response::Error { message }) => {
                        self.launch.notice = Some(format!("Stop failed: {message}"));
                    }
                    Ok(_) => {}
                    Err(error) => self.launch.notice = Some(format!("Stop failed: {error}")),
                }
            }
            _ => {}
        }
    }

    fn status_bar(&self, ui: &mut egui::Ui) {
        // Address on the right; the state text gets the rest and is cut off (full text on hover)
        // so a long OS error message cannot push the address out of the window.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(self.launch.url.clone().unwrap_or_else(|| self.settings.ws_url()));
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                let (dot, color) = state_indicator(&self.conn);
                ui.colored_label(color, dot);
                let supervisor = launch::status_text(&self.launch.state, self.conn.is_connected());
                let text = if self.conn.is_connected() {
                    supervisor
                } else {
                    format!("{supervisor} | {}", describe_state(&self.conn, Instant::now()))
                };
                ui.add(egui::Label::new(&text).truncate()).on_hover_text(&text);
            });
        });
    }

    fn navigation(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        for id in ViewId::ALL {
            if id.is_available() {
                ui.selectable_value(&mut self.selected, id, id.label());
            } else {
                ui.add_enabled_ui(false, |ui| ui.selectable_label(false, id.label()))
                    .response
                    .on_disabled_hover_text(format!("Coming in {}", id.planned_in()));
            }
        }
    }
}

impl eframe::App for AdminApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_ui_requests();
        self.pump_events();
        if self.applied_theme != Some(self.settings.theme) {
            ctx.set_theme(self.settings.theme.preference());
            self.applied_theme = Some(self.settings.theme);
        }
        // The retry countdown in the status bar needs a repaint without any event.
        if matches!(self.conn, ConnState::Waiting { .. }) {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("status_bar").show(ui, |ui| self.status_bar(ui));
        egui::Panel::left("navigation")
            .resizable(false)
            .exact_size(140.0)
            .show(ui, |ui| self.navigation(ui));
        egui::CentralPanel::default().show(ui, |ui| {
            let mut cx = AppCtx {
                settings: &mut self.settings,
                conn: &self.conn,
                bridge: &self.bridge,
                launch: &self.launch,
            };
            match self.views.get_mut(self.selected) {
                Some(view) => view.ui(ui, &mut cx),
                None => {
                    ui.label("Not available yet.");
                }
            }
        });
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.settings.save(storage);
    }
}

/// One-line description of the connection for the status bar and the connection view.
pub fn describe_state(conn: &ConnState, now: Instant) -> String {
    match conn {
        ConnState::Disconnected => "Disconnected".to_string(),
        ConnState::Connecting { attempt: 1 } => "Connecting...".to_string(),
        ConnState::Connecting { attempt } => format!("Connecting... (attempt {attempt})"),
        ConnState::Connected { name } => format!("Connected to {name}"),
        ConnState::Waiting {
            failures,
            error,
            retry_at,
        } => {
            // Round up so the countdown never shows 0 s while still waiting.
            let secs = retry_at.saturating_duration_since(now).as_secs_f32().ceil() as u64;
            format!(
                "Disconnected ({error}). Retrying in {secs} s (attempt {})",
                failures + 1
            )
        }
        ConnState::Rejected { reason } => format!("Rejected: {reason}"),
    }
}

fn state_indicator(conn: &ConnState) -> (&'static str, Color32) {
    match conn {
        ConnState::Connected { .. } => ("●", Color32::from_rgb(0x3c, 0xb3, 0x71)),
        ConnState::Connecting { .. } | ConnState::Waiting { .. } => {
            ("●", Color32::from_rgb(0xe0, 0xa0, 0x30))
        }
        ConnState::Rejected { .. } => ("●", Color32::from_rgb(0xd9, 0x4f, 0x4f)),
        ConnState::Disconnected => ("●", Color32::GRAY),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describes_every_state() {
        let now = Instant::now();
        assert_eq!(describe_state(&ConnState::Disconnected, now), "Disconnected");
        assert_eq!(
            describe_state(&ConnState::Connecting { attempt: 1 }, now),
            "Connecting..."
        );
        assert_eq!(
            describe_state(&ConnState::Connecting { attempt: 3 }, now),
            "Connecting... (attempt 3)"
        );
        assert_eq!(
            describe_state(
                &ConnState::Connected {
                    name: "supervisor".into()
                },
                now
            ),
            "Connected to supervisor"
        );
        assert_eq!(
            describe_state(
                &ConnState::Rejected {
                    reason: "invalid token".into()
                },
                now
            ),
            "Rejected: invalid token"
        );
    }

    #[test]
    fn waiting_shows_a_rounded_up_countdown() {
        let now = Instant::now();
        let waiting = ConnState::Waiting {
            failures: 2,
            error: "connection closed".into(),
            retry_at: now + Duration::from_millis(3200),
        };
        assert_eq!(
            describe_state(&waiting, now),
            "Disconnected (connection closed). Retrying in 4 s (attempt 3)"
        );
        // Past the deadline: never negative.
        let later = now + Duration::from_secs(10);
        assert!(describe_state(&waiting, later).contains("Retrying in 0 s"));
    }
}
