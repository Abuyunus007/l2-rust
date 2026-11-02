//! Servers view: the supervisor's processes (state, PID, uptime, last exit code) with
//! Start/Stop/Restart, and the login server's game server table.
//!
//! Java: `loginserver/ui/Gui.java` (the game server columns of the login server's window).

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use egui::{Align2, Color32};
use l2_admin_api::{Event, LoginStatus, Request, Response, ServerState, ServerStatus};

use crate::app::AppCtx;
use crate::bridge::BridgeEvent;
use crate::views::View;

/// The supervisor's name for the login server (`Server.login.*` in `supervisor.ini`).
const LOGIN_SERVER: &str = "login";
/// After a failed `GetLoginStatus` (server not answering yet) ask again after this long.
const LOGIN_RETRY: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Start,
    Stop,
    Restart,
}

impl Action {
    fn verb(self) -> &'static str {
        match self {
            Action::Start => "Start",
            Action::Stop => "Stop",
            Action::Restart => "Restart",
        }
    }

    fn request(self, name: &str) -> Request {
        let name = name.to_string();
        match self {
            Action::Start => Request::StartServer { name },
            Action::Stop => Request::StopServer { name },
            Action::Restart => Request::RestartServer { name },
        }
    }

    /// Stop and restart cut the players off, so they ask first. Start is harmless.
    fn needs_confirm(self) -> bool {
        self != Action::Start
    }
}

/// Which action buttons are enabled. Pure, so the rules are tested without egui.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Enabled {
    start: bool,
    stop: bool,
    restart: bool,
}

fn enabled_actions(state: ServerState, connected: bool) -> Enabled {
    let (start, stop, restart) = match state {
        ServerState::Stopped | ServerState::Crashed => (true, false, false),
        // Stop also works while starting: it cancels a pending restart or stops the new process.
        ServerState::Starting => (false, true, false),
        ServerState::Running => (false, true, true),
        // The supervisor is already on its way down.
        ServerState::Stopping => (false, false, false),
    };
    Enabled {
        start: connected && start,
        stop: connected && stop,
        restart: connected && restart,
    }
}

/// The servers a "Start all" / "Stop all" touches, in the order it works through them:
/// Start goes in the `Servers` order of `supervisor.ini` (the order the supervisor lists them),
/// Stop goes in reverse, so the game server is down before the login server it registered with.
/// Servers for which the single button would be disabled are skipped.
fn plan_batch(action: Action, servers: &[ServerStatus]) -> Vec<String> {
    let wanted = |s: &&ServerStatus| {
        let enabled = enabled_actions(s.state, true);
        match action {
            Action::Start => enabled.start,
            Action::Stop => enabled.stop,
            Action::Restart => false,
        }
    };
    let names =
        |iter: &mut dyn Iterator<Item = &ServerStatus>| iter.filter(wanted).map(|s| s.name.clone()).collect();
    match action {
        Action::Stop => names(&mut servers.iter().rev()),
        _ => names(&mut servers.iter()),
    }
}

fn state_label(state: ServerState) -> &'static str {
    match state {
        ServerState::Stopped => "Stopped",
        ServerState::Starting => "Starting",
        ServerState::Running => "Running",
        ServerState::Stopping => "Stopping",
        ServerState::Crashed => "Crashed",
    }
}

/// Same colours as the connection indicator in the status bar.
fn state_color(state: ServerState) -> Color32 {
    match state {
        ServerState::Running => Color32::from_rgb(0x3c, 0xb3, 0x71),
        ServerState::Starting | ServerState::Stopping => Color32::from_rgb(0xe0, 0xa0, 0x30),
        ServerState::Crashed => Color32::from_rgb(0xd9, 0x4f, 0x4f),
        ServerState::Stopped => Color32::GRAY,
    }
}

/// `hh:mm:ss`, with a day count in front once it passes 24 h.
fn format_uptime(secs: u64) -> String {
    let (days, rest) = (secs / 86_400, secs % 86_400);
    let (h, m, s) = (rest / 3600, rest % 3600 / 60, rest % 60);
    if days > 0 {
        format!("{days}d {h:02}:{m:02}:{s:02}")
    } else {
        format!("{h:02}:{m:02}:{s:02}")
    }
}

fn or_dash<T: ToString>(value: Option<T>) -> String {
    value.map_or_else(|| "-".to_string(), |v| v.to_string())
}

fn disabled_reason(connected: bool, state: ServerState) -> String {
    if connected {
        format!("Not possible while {}", state_label(state).to_lowercase())
    } else {
        "Not connected to the supervisor".to_string()
    }
}

#[derive(Default)]
pub struct ServersView {
    servers: Vec<ServerStatus>,
    login: Option<LoginStatus>,
    login_error: Option<String>,
    /// The `GetLoginStatus` request in flight, if any.
    login_request: Option<u64>,
    /// No new `GetLoginStatus` before this instant (set after a failure).
    login_retry_at: Option<Instant>,
    /// Start/Stop/Restart requests in flight, by request id, with a label for error messages.
    actions: HashMap<u64, String>,
    /// Stop or Restart waiting for the user's confirmation.
    confirm: Option<(Action, String)>,
    /// "Stop all" waiting for the user's confirmation.
    confirm_stop_all: bool,
    /// Start all / Stop all: one request at a time, the next after the answer, so the order holds.
    batch: VecDeque<(Action, String)>,
    /// The request of the batch in flight.
    batch_inflight: Option<u64>,
    error: Option<String>,
}

impl ServersView {
    fn server(&self, name: &str) -> Option<&ServerStatus> {
        self.servers.iter().find(|s| s.name == name)
    }

    /// Folds one bridge event into the view. `now` is passed in so the retry timer is testable.
    fn apply(&mut self, event: &BridgeEvent, now: Instant) {
        match event {
            // A new attempt or session: requests of the old connection never get a useful answer.
            BridgeEvent::Connecting { .. } | BridgeEvent::Waiting { .. } | BridgeEvent::Connected { .. } => {
                self.clear_login()
            }
            BridgeEvent::Disconnected | BridgeEvent::Rejected { .. } => {
                self.clear_login();
                self.servers.clear();
                self.actions.clear();
                self.confirm = None;
                self.confirm_stop_all = false;
                self.batch.clear();
                self.batch_inflight = None;
                self.error = None;
            }
            BridgeEvent::Event(Event::ServerStateChanged { status }) => {
                // The login server's game server table is only valid while it runs.
                if status.name == LOGIN_SERVER && status.state != ServerState::Running {
                    self.clear_login();
                }
                match self.servers.iter_mut().find(|s| s.name == status.name) {
                    Some(slot) => *slot = status.clone(),
                    None => self.servers.push(status.clone()),
                }
            }
            BridgeEvent::Event(Event::LoginStatusChanged { server, status }) => {
                if server == LOGIN_SERVER {
                    self.login = Some(status.clone());
                    self.login_error = None;
                }
            }
            BridgeEvent::Event(Event::LogLine { .. }) => {}
            BridgeEvent::Response { id, result } => {
                if let Ok(Response::Servers { servers }) = result {
                    // Answer to ListServers (sent by the app on every connect).
                    self.servers = servers.clone();
                } else if self.login_request == Some(*id) {
                    self.login_request = None;
                    match result {
                        Ok(Response::LoginStatus { status }) => {
                            self.login = Some(status.clone());
                            self.login_error = None;
                        }
                        Ok(Response::Error { message }) => self.login_failed(message.clone(), now),
                        Ok(other) => self.login_failed(format!("unexpected answer: {other:?}"), now),
                        Err(error) => self.login_failed(error.clone(), now),
                    }
                } else if let Some(what) = self.actions.remove(id) {
                    let failure = match result {
                        Ok(Response::Error { message }) => Some(message.clone()),
                        Ok(_) => None,
                        Err(error) => Some(error.clone()),
                    };
                    if self.batch_inflight == Some(*id) {
                        self.batch_inflight = None;
                        // A failed step ends the batch: later servers may depend on this one.
                        if failure.is_some() {
                            self.batch.clear();
                        }
                    }
                    if let Some(failure) = failure {
                        self.error = Some(format!("{what} failed: {failure}"));
                    }
                }
            }
        }
    }

    fn clear_login(&mut self) {
        self.login = None;
        self.login_error = None;
        self.login_request = None;
        self.login_retry_at = None;
    }

    fn login_failed(&mut self, message: String, now: Instant) {
        self.login_error = Some(message);
        self.login_retry_at = Some(now + LOGIN_RETRY);
    }

    /// True when the login server runs, we have no status for it, and no request is pending.
    fn login_refresh_due(&self, connected: bool, now: Instant) -> bool {
        let running = self
            .server(LOGIN_SERVER)
            .is_some_and(|s| s.state == ServerState::Running);
        connected
            && running
            && self.login.is_none()
            && self.login_request.is_none()
            && self.login_retry_at.is_none_or(|at| now >= at)
    }

    fn send(&mut self, cx: &AppCtx<'_>, action: Action, name: String) {
        self.error = None;
        let id = cx.bridge.request(action.request(&name));
        self.actions.insert(id, format!("{} {name}", action.verb()));
    }

    /// Queues "Start all" / "Stop all" (see [`plan_batch`]).
    fn begin_batch(&mut self, action: Action) {
        self.error = None;
        self.batch = plan_batch(action, &self.servers)
            .into_iter()
            .map(|name| (action, name))
            .collect();
    }

    /// Sends the next step of the batch once the previous one was answered.
    fn pump_batch(&mut self, cx: &AppCtx<'_>) {
        if self.batch_inflight.is_some() {
            return;
        }
        if let Some((action, name)) = self.batch.pop_front() {
            let id = cx.bridge.request(action.request(&name));
            self.actions.insert(id, format!("{} {name}", action.verb()));
            self.batch_inflight = Some(id);
        }
    }

    fn all_buttons(&mut self, ui: &mut egui::Ui, connected: bool) {
        let idle = self.batch.is_empty() && self.batch_inflight.is_none();
        let can_start = connected && idle && !plan_batch(Action::Start, &self.servers).is_empty();
        let can_stop = connected && idle && !plan_batch(Action::Stop, &self.servers).is_empty();
        ui.horizontal(|ui| {
            if ui
                .add_enabled(can_start, egui::Button::new("Start all"))
                .on_hover_text("Starts the stopped servers in the order of supervisor.ini")
                .on_disabled_hover_text("Nothing to start, or not connected")
                .clicked()
            {
                self.begin_batch(Action::Start);
            }
            if ui
                .add_enabled(can_stop, egui::Button::new("Stop all"))
                .on_hover_text("Stops the running servers in reverse order")
                .on_disabled_hover_text("Nothing to stop, or not connected")
                .clicked()
            {
                self.confirm_stop_all = true;
            }
            if !idle {
                ui.label("Working...");
            }
        });
    }

    fn confirm_stop_all_dialog(&mut self, ui: &mut egui::Ui, connected: bool) {
        if !self.confirm_stop_all {
            return;
        }
        let mut open = true;
        let mut answer: Option<bool> = None;
        egui::Window::new("Stop all servers?")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ui.ctx(), |ui| {
                ui.label("Players will be disconnected. The servers are stopped one after the other, last one first.");
                if cfg!(windows) {
                    ui.label("On Windows a process without admin endpoint is killed: players may not be saved.");
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.add_enabled(connected, egui::Button::new("Stop all")).clicked() {
                        answer = Some(true);
                    }
                    if ui.button("Cancel").clicked() {
                        answer = Some(false);
                    }
                });
            });
        match answer {
            Some(true) => {
                self.confirm_stop_all = false;
                self.begin_batch(Action::Stop);
            }
            Some(false) => self.confirm_stop_all = false,
            None if !open => self.confirm_stop_all = false,
            None => {}
        }
    }

    fn servers_table(&mut self, ui: &mut egui::Ui, cx: &AppCtx<'_>, connected: bool) {
        if self.servers.is_empty() {
            let text = if connected {
                "No servers configured in supervisor.ini."
            } else {
                "No server list yet."
            };
            ui.label(text);
            return;
        }
        let mut clicked: Option<(Action, String)> = None;
        egui::Grid::new("servers_table")
            .striped(true)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                for head in ["Name", "State", "PID", "Uptime", "Last exit", "Actions"] {
                    ui.strong(head);
                }
                ui.end_row();
                for server in &self.servers {
                    let enabled = enabled_actions(server.state, connected);
                    ui.label(server.name.as_str());
                    ui.colored_label(state_color(server.state), state_label(server.state));
                    ui.label(or_dash(server.pid));
                    ui.label(server.uptime_secs.map_or_else(|| "-".to_string(), format_uptime));
                    ui.label(or_dash(server.last_exit_code));
                    ui.horizontal(|ui| {
                        let buttons = [
                            (Action::Start, enabled.start),
                            (Action::Stop, enabled.stop),
                            (Action::Restart, enabled.restart),
                        ];
                        for (action, on) in buttons {
                            let response = ui
                                .add_enabled(on, egui::Button::new(action.verb()))
                                .on_disabled_hover_text(disabled_reason(connected, server.state));
                            if response.clicked() {
                                clicked = Some((action, server.name.clone()));
                            }
                        }
                    });
                    ui.end_row();
                }
            });

        if let Some((action, name)) = clicked {
            if action.needs_confirm() {
                self.confirm = Some((action, name));
            } else {
                self.send(cx, action, name);
            }
        }
    }

    fn login_panel(&self, ui: &mut egui::Ui) {
        ui.heading("Login server");
        ui.add_space(4.0);
        let Some(login_server) = self.server(LOGIN_SERVER) else {
            return;
        };
        if login_server.state != ServerState::Running {
            ui.label("Not running. Its game servers show up once it is running.");
            return;
        }
        let Some(status) = &self.login else {
            match &self.login_error {
                Some(error) => {
                    ui.colored_label(
                        ui.visuals().error_fg_color,
                        format!("Status unavailable: {error}"),
                    );
                }
                None => {
                    ui.label("Waiting for the login server's status...");
                }
            }
            return;
        };

        ui.label(format!(
            "Clients authed but not in a game server yet: {}",
            status.authed_clients
        ));
        ui.add_space(8.0);
        if status.game_servers.is_empty() {
            ui.label("No game server has registered.");
            return;
        }
        egui::Grid::new("game_servers_table")
            .striped(true)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                for head in ["ID", "Name", "Authed", "Online", "Status"] {
                    ui.strong(head);
                }
                ui.end_row();
                for gs in &status.game_servers {
                    ui.label(gs.id.to_string());
                    ui.label(gs.name.as_str());
                    ui.label(if gs.authed { "Yes" } else { "No" });
                    ui.label(format!("{} / {}", gs.online, gs.max_players));
                    ui.label(gs.status.as_str());
                    ui.end_row();
                }
            });
    }

    fn confirm_dialog(&mut self, ui: &mut egui::Ui, cx: &AppCtx<'_>, connected: bool) {
        let Some((action, name)) = self.confirm.clone() else {
            return;
        };
        let mut open = true;
        let mut answer: Option<bool> = None;
        egui::Window::new(format!("{} {name}?", action.verb()))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ui.ctx(), |ui| {
                ui.label("Players on this server will be disconnected.");
                if cfg!(windows) {
                    ui.label("On Windows the process is killed: shutdown hooks do not run and players may not be saved.");
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(connected, egui::Button::new(action.verb()))
                        .clicked()
                    {
                        answer = Some(true);
                    }
                    if ui.button("Cancel").clicked() {
                        answer = Some(false);
                    }
                });
            });
        match answer {
            Some(true) => {
                self.confirm = None;
                self.send(cx, action, name);
            }
            Some(false) => self.confirm = None,
            None if !open => self.confirm = None,
            None => {}
        }
    }
}

impl View for ServersView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut AppCtx<'_>) {
        let connected = cx.conn.is_connected();
        let now = Instant::now();

        // The login server's game server table is not pushed until someone asks once.
        if self.login_refresh_due(connected, now) {
            self.login_request = Some(cx.bridge.request(Request::GetLoginStatus {
                server: Some(LOGIN_SERVER.to_string()),
            }));
        } else if let Some(at) = self.login_retry_at.filter(|at| *at > now) {
            ui.ctx().request_repaint_after(at - now);
        }

        ui.heading("Servers");
        ui.add_space(8.0);
        if !connected {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                "Not connected to the supervisor. Actions are disabled.",
            );
        }
        if let Some(error) = self.error.clone() {
            ui.horizontal(|ui| {
                ui.colored_label(ui.visuals().error_fg_color, error);
                if ui.small_button("Dismiss").clicked() {
                    self.error = None;
                }
            });
        }
        ui.add_space(8.0);

        self.pump_batch(cx);
        self.all_buttons(ui, connected);
        ui.add_space(8.0);
        self.servers_table(ui, cx, connected);
        if self.server(LOGIN_SERVER).is_some() {
            ui.add_space(16.0);
            self.login_panel(ui);
        }
        self.confirm_dialog(ui, cx, connected);
        self.confirm_stop_all_dialog(ui, connected);
    }

    fn on_event(&mut self, event: &BridgeEvent) {
        self.apply(event, Instant::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use l2_admin_api::GameServerStatus;

    fn status(name: &str, state: ServerState) -> ServerStatus {
        ServerStatus {
            name: name.into(),
            state,
            pid: None,
            uptime_secs: None,
            last_exit_code: None,
        }
    }

    fn list(servers: Vec<ServerStatus>) -> BridgeEvent {
        BridgeEvent::Response {
            id: 1,
            result: Ok(Response::Servers { servers }),
        }
    }

    #[test]
    fn buttons_follow_the_state() {
        let want = |start, stop, restart| Enabled { start, stop, restart };
        use ServerState::{Crashed, Running, Starting, Stopped, Stopping};
        assert_eq!(enabled_actions(Stopped, true), want(true, false, false));
        assert_eq!(enabled_actions(Crashed, true), want(true, false, false));
        assert_eq!(enabled_actions(Starting, true), want(false, true, false));
        assert_eq!(enabled_actions(Running, true), want(false, true, true));
        assert_eq!(enabled_actions(Stopping, true), want(false, false, false));
        for state in [Stopped, Starting, Running, Stopping, Crashed] {
            assert_eq!(
                enabled_actions(state, false),
                want(false, false, false),
                "{state:?}"
            );
        }
    }

    #[test]
    fn start_all_goes_forward_and_stop_all_goes_backward() {
        use ServerState::{Crashed, Running, Stopped, Stopping};
        let servers = vec![
            status("login", Stopped),
            status("game", Crashed),
            status("extra", Running),
            status("busy", Stopping),
        ];
        // Only servers whose single Start button is enabled, in list order.
        assert_eq!(plan_batch(Action::Start, &servers), ["login", "game"]);
        // Only servers that can be stopped, last one first.
        assert_eq!(plan_batch(Action::Stop, &servers), ["extra"]);

        let all_running = vec![status("login", Running), status("game", Running)];
        assert_eq!(plan_batch(Action::Stop, &all_running), ["game", "login"]);
        assert!(plan_batch(Action::Start, &all_running).is_empty());
        assert!(plan_batch(Action::Restart, &all_running).is_empty());
    }

    #[test]
    fn a_batch_continues_after_each_answer_and_stops_at_the_first_failure() {
        let now = Instant::now();
        let mut view = ServersView::default();
        view.begin_batch_for_test(Action::Stop, &["game", "login"]);

        // Pretend the first step was sent as request 5.
        let (action, name) = view.batch.pop_front().unwrap();
        view.actions.insert(5, format!("{} {name}", action.verb()));
        view.batch_inflight = Some(5);
        view.apply(
            &BridgeEvent::Response {
                id: 5,
                result: Ok(Response::Ok),
            },
            now,
        );
        assert_eq!(view.batch_inflight, None);
        assert_eq!(view.batch.len(), 1, "the next step is still queued");
        assert_eq!(view.error, None);

        // The second step fails: nothing else is queued, the error is shown.
        view.batch.push_back((Action::Stop, "third".into()));
        view.actions.insert(6, "Stop login".into());
        view.batch_inflight = Some(6);
        view.apply(
            &BridgeEvent::Response {
                id: 6,
                result: Ok(Response::Error {
                    message: "boom".into(),
                }),
            },
            now,
        );
        assert!(view.batch.is_empty());
        assert_eq!(view.error.as_deref(), Some("Stop login failed: boom"));
    }

    #[test]
    fn losing_the_connection_drops_the_batch() {
        let mut view = ServersView::default();
        view.begin_batch_for_test(Action::Start, &["login"]);
        view.batch_inflight = Some(1);
        view.confirm_stop_all = true;
        view.apply(&BridgeEvent::Disconnected, Instant::now());
        assert!(view.batch.is_empty() && view.batch_inflight.is_none() && !view.confirm_stop_all);
    }

    impl ServersView {
        fn begin_batch_for_test(&mut self, action: Action, names: &[&str]) {
            self.batch = names.iter().map(|n| (action, (*n).to_string())).collect();
        }
    }

    #[test]
    fn uptime_and_missing_values_are_readable() {
        assert_eq!(format_uptime(0), "00:00:00");
        assert_eq!(format_uptime(3723), "01:02:03");
        assert_eq!(format_uptime(90_061), "1d 01:01:01");
        assert_eq!(or_dash::<i32>(None), "-");
        assert_eq!(or_dash(Some(2)), "2");
    }

    #[test]
    fn list_and_state_events_update_the_table() {
        let now = Instant::now();
        let mut view = ServersView::default();
        view.apply(
            &list(vec![
                status("login", ServerState::Stopped),
                status("game", ServerState::Stopped),
            ]),
            now,
        );
        assert_eq!(view.servers.len(), 2);

        view.apply(
            &BridgeEvent::Event(Event::ServerStateChanged {
                status: status("game", ServerState::Running),
            }),
            now,
        );
        assert_eq!(view.server("game").map(|s| s.state), Some(ServerState::Running));
        assert_eq!(view.servers.len(), 2);

        // A server the list did not have yet is added.
        view.apply(
            &BridgeEvent::Event(Event::ServerStateChanged {
                status: status("extra", ServerState::Crashed),
            }),
            now,
        );
        assert_eq!(view.servers.len(), 3);
    }

    #[test]
    fn login_status_is_kept_while_the_login_server_runs() {
        let now = Instant::now();
        let mut view = ServersView::default();
        view.apply(&list(vec![status("login", ServerState::Running)]), now);
        assert!(view.login_refresh_due(true, now));
        assert!(!view.login_refresh_due(false, now));

        view.login_request = Some(7);
        assert!(!view.login_refresh_due(true, now), "request in flight");

        let login = LoginStatus {
            game_servers: vec![GameServerStatus {
                id: 1,
                name: "Bartz".into(),
                authed: true,
                online: 3,
                max_players: 2000,
                status: "Normal".into(),
            }],
            authed_clients: 2,
        };
        view.apply(
            &BridgeEvent::Response {
                id: 7,
                result: Ok(Response::LoginStatus {
                    status: login.clone(),
                }),
            },
            now,
        );
        assert_eq!(view.login, Some(login.clone()));
        assert_eq!(view.login_request, None);
        assert!(!view.login_refresh_due(true, now), "already have it");

        // Pushed updates replace it; another server's update is ignored.
        let other = LoginStatus {
            game_servers: vec![],
            authed_clients: 0,
        };
        view.apply(
            &BridgeEvent::Event(Event::LoginStatusChanged {
                server: "game".into(),
                status: other.clone(),
            }),
            now,
        );
        assert_eq!(view.login, Some(login));
        view.apply(
            &BridgeEvent::Event(Event::LoginStatusChanged {
                server: LOGIN_SERVER.into(),
                status: other.clone(),
            }),
            now,
        );
        assert_eq!(view.login, Some(other));

        // The login server stops: its table goes away.
        view.apply(
            &BridgeEvent::Event(Event::ServerStateChanged {
                status: status("login", ServerState::Stopped),
            }),
            now,
        );
        assert_eq!(view.login, None);
    }

    #[test]
    fn failed_login_request_is_shown_and_retried_later() {
        let now = Instant::now();
        let mut view = ServersView::default();
        view.apply(&list(vec![status("login", ServerState::Running)]), now);
        view.login_request = Some(3);
        view.apply(
            &BridgeEvent::Response {
                id: 3,
                result: Err("connection closed".into()),
            },
            now,
        );
        assert_eq!(view.login_error.as_deref(), Some("connection closed"));
        assert!(!view.login_refresh_due(true, now));
        assert!(view.login_refresh_due(true, now + LOGIN_RETRY));
    }

    #[test]
    fn action_errors_are_reported() {
        let now = Instant::now();
        let mut view = ServersView::default();
        view.actions.insert(9, "Stop login".into());
        view.apply(
            &BridgeEvent::Response {
                id: 9,
                result: Ok(Response::Error {
                    message: "unknown server".into(),
                }),
            },
            now,
        );
        assert_eq!(view.error.as_deref(), Some("Stop login failed: unknown server"));

        view.actions.insert(10, "Start login".into());
        view.apply(
            &BridgeEvent::Response {
                id: 10,
                result: Ok(Response::Ok),
            },
            now,
        );
        assert!(!view.actions.contains_key(&10));
    }

    #[test]
    fn disconnect_clears_the_session() {
        let now = Instant::now();
        let mut view = ServersView::default();
        view.apply(&list(vec![status("login", ServerState::Running)]), now);
        view.error = Some("old".into());
        view.confirm = Some((Action::Stop, "login".into()));
        view.apply(&BridgeEvent::Disconnected, now);
        assert!(view.servers.is_empty());
        assert!(view.error.is_none() && view.confirm.is_none());
    }

    #[test]
    fn only_stop_and_restart_need_confirmation() {
        assert!(!Action::Start.needs_confirm());
        assert!(Action::Stop.needs_confirm());
        assert!(Action::Restart.needs_confirm());
        assert_eq!(
            Action::Restart.request("login"),
            Request::RestartServer { name: "login".into() }
        );
    }
}
