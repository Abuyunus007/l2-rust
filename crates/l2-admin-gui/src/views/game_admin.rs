//! Game admin view (G-03): announcements, a console that runs any admin command, shutdown or
//! restart with a countdown, and the numbers `//serverinfo` shows. Everything goes to the Rust
//! game server through the supervisor (`Request::GameAdmin`), where it runs as the matching `//`
//! command of a virtual GM.
//!
//! Java: `AdminAnnouncements`, `AdminShutdown`, `AdminServerInfo` (chat commands). The request
//! building and the answer parsing are plain functions so they are tested without egui.

use std::time::{Duration, Instant};

use l2_admin_api::{GameAction, GameInfo, Request, Response};

use crate::app::AppCtx;
use crate::bridge::BridgeEvent;
use crate::views::View;

/// The supervisor's name for the game server (`Server.game.*` in `supervisor.ini`).
pub const GAME_SERVER: &str = "game";
/// How often the server info is asked for again while the view is open.
const INFO_REFRESH: Duration = Duration::from_secs(5);
/// The console keeps this many output lines.
const CONSOLE_LINES: usize = 500;

/// A request to the game server through the supervisor.
pub fn game_request(action: GameAction) -> Request {
    Request::GameAdmin {
        server: Some(GAME_SERVER.to_string()),
        action,
    }
}

/// What the answer to a [`GameAction`] says: the lines of a reply, or the error.
pub fn reply_lines(result: &Result<Response, String>) -> Result<Vec<String>, String> {
    match result {
        Ok(Response::GameReply { lines }) => Ok(lines.clone()),
        Ok(Response::Error { message }) => Err(message.clone()),
        Ok(other) => Err(format!("unexpected answer: {other:?}")),
        Err(error) => Err(error.clone()),
    }
}

/// The announcement request, or `None` for an empty text.
pub fn announce_request(text: &str, critical: bool) -> Option<Request> {
    let text = text.trim();
    (!text.is_empty()).then(|| {
        game_request(GameAction::Announce {
            text: text.to_owned(),
            critical,
        })
    })
}

/// The console request: `//kick Amy` and `kick Amy` are the same. `None` for an empty line.
pub fn console_request(line: &str) -> Option<Request> {
    let command = line.trim().trim_start_matches('/').trim();
    (!command.is_empty()).then(|| {
        game_request(GameAction::Command {
            command: command.to_owned(),
        })
    })
}

/// The shutdown or restart request for the text of the seconds field.
pub fn shutdown_request(seconds: &str, restart: bool) -> Result<Request, String> {
    let seconds: u32 = seconds
        .trim()
        .parse()
        .map_err(|_| format!("'{}' is not a number of seconds", seconds.trim()))?;
    Ok(game_request(GameAction::Shutdown { seconds, restart }))
}

/// The info lines of the panel.
pub fn info_lines(info: &GameInfo) -> Vec<String> {
    let mut lines = vec![
        format!("Uptime: {}", format_duration(info.uptime_secs)),
        format!("Online: {}", info.online),
        format!(
            "Memory: {}",
            if info.memory_bytes == 0 {
                "unknown".to_owned()
            } else {
                format!("{} MB", info.memory_bytes / 0x10_0000)
            }
        ),
        format!(
            "Threads: {}",
            if info.threads == 0 {
                "unknown".to_owned()
            } else {
                info.threads.to_string()
            }
        ),
        format!("Ticks: {} ({} ms each)", info.ticks, info.tick_ms),
    ];
    if let Some(left) = info.shutdown_in_secs {
        lines.push(format!("Shutdown countdown: {left} s left"));
    }
    lines
}

/// `1d 02:03:04` style uptime.
pub fn format_duration(secs: u64) -> String {
    let (days, rest) = (secs / 86_400, secs % 86_400);
    let clock = format!("{:02}:{:02}:{:02}", rest / 3600, rest % 3600 / 60, rest % 60);
    if days > 0 {
        format!("{days}d {clock}")
    } else {
        clock
    }
}

#[derive(Default)]
pub struct GameAdminView {
    announce_text: String,
    announce_critical: bool,
    console_input: String,
    console_output: Vec<String>,
    seconds: String,
    /// The last result of an announce or shutdown button.
    status: Option<Result<String, String>>,
    info: Option<GameInfo>,
    info_error: Option<String>,
    info_request: Option<u64>,
    info_next_at: Option<Instant>,
    /// Requests whose answer goes to the status line / the console.
    status_request: Option<u64>,
    console_request: Option<u64>,
}

impl GameAdminView {
    fn reset(&mut self) {
        self.info = None;
        self.info_error = None;
        self.info_request = None;
        self.info_next_at = None;
        self.status_request = None;
        self.console_request = None;
    }

    fn push_console(&mut self, line: String) {
        self.console_output.push(line);
        let extra = self.console_output.len().saturating_sub(CONSOLE_LINES);
        self.console_output.drain(..extra);
    }

    fn apply(&mut self, event: &BridgeEvent, now: Instant) {
        match event {
            BridgeEvent::Connecting { .. }
            | BridgeEvent::Waiting { .. }
            | BridgeEvent::Connected { .. }
            | BridgeEvent::Disconnected
            | BridgeEvent::Rejected { .. } => self.reset(),
            BridgeEvent::Event(_) => {}
            BridgeEvent::Response { id, result } => {
                if self.info_request == Some(*id) {
                    self.info_request = None;
                    self.info_next_at = Some(now + INFO_REFRESH);
                    match result {
                        Ok(Response::GameInfo { info }) => {
                            self.info = Some(info.clone());
                            self.info_error = None;
                        }
                        Ok(Response::Error { message }) => self.info_failed(message.clone()),
                        Ok(other) => self.info_failed(format!("unexpected answer: {other:?}")),
                        Err(error) => self.info_failed(error.clone()),
                    }
                } else if self.status_request == Some(*id) {
                    self.status_request = None;
                    self.status = Some(reply_lines(result).map(|lines| {
                        if lines.is_empty() {
                            "Done.".to_owned()
                        } else {
                            lines.join(" | ")
                        }
                    }));
                } else if self.console_request == Some(*id) {
                    self.console_request = None;
                    match reply_lines(result) {
                        Ok(lines) if lines.is_empty() => self.push_console("(no output)".into()),
                        Ok(lines) => {
                            for line in lines {
                                self.push_console(line);
                            }
                        }
                        Err(error) => self.push_console(format!("Error: {error}")),
                    }
                }
            }
        }
    }

    fn info_failed(&mut self, message: String) {
        self.info = None;
        self.info_error = Some(message);
    }

    fn info_due(&self, connected: bool, now: Instant) -> bool {
        connected && self.info_request.is_none() && self.info_next_at.is_none_or(|at| now >= at)
    }
}

impl View for GameAdminView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut AppCtx<'_>) {
        let connected = cx.conn.is_connected();
        let now = Instant::now();
        if self.info_due(connected, now) {
            self.info_request = Some(cx.bridge.request(game_request(GameAction::ServerInfo)));
        } else if let Some(at) = self.info_next_at.filter(|at| *at > now) {
            ui.ctx().request_repaint_after(at - now);
        }

        ui.heading("Game admin");
        ui.add_space(8.0);
        if !connected {
            ui.colored_label(ui.visuals().warn_fg_color, "Not connected to the supervisor.");
        }

        ui.strong("Server info");
        match (&self.info, &self.info_error) {
            (Some(info), _) => {
                for line in info_lines(info) {
                    ui.label(line);
                }
            }
            (None, Some(error)) => {
                ui.colored_label(ui.visuals().error_fg_color, format!("Unavailable: {error}"));
            }
            (None, None) => {
                ui.label("Waiting for the game server...");
            }
        }
        ui.separator();

        ui.strong("Announcement");
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.announce_text)
                    .hint_text("text for all players")
                    .desired_width(360.0),
            );
            ui.checkbox(&mut self.announce_critical, "Critical");
            if ui.add_enabled(connected, egui::Button::new("Announce")).clicked()
                && let Some(request) = announce_request(&self.announce_text, self.announce_critical)
            {
                self.status_request = Some(cx.bridge.request(request));
                self.announce_text.clear();
            }
        });
        ui.separator();

        ui.strong("Shutdown or restart");
        ui.horizontal(|ui| {
            ui.label("Seconds:");
            ui.add(egui::TextEdit::singleline(&mut self.seconds).desired_width(60.0));
            for (label, restart) in [("Shutdown", false), ("Restart", true)] {
                if ui.add_enabled(connected, egui::Button::new(label)).clicked() {
                    match shutdown_request(&self.seconds, restart) {
                        Ok(request) => self.status_request = Some(cx.bridge.request(request)),
                        Err(error) => self.status = Some(Err(error)),
                    }
                }
            }
            if ui.add_enabled(connected, egui::Button::new("Abort")).clicked() {
                self.status_request = Some(cx.bridge.request(game_request(GameAction::AbortShutdown)));
            }
        });
        match &self.status {
            Some(Ok(text)) => {
                ui.label(text.as_str());
            }
            Some(Err(error)) => {
                ui.colored_label(ui.visuals().error_fg_color, error.as_str());
            }
            None => {}
        }
        ui.separator();

        ui.strong("Console (runs as a GM with the highest access level)");
        let response = ui.add(
            egui::TextEdit::singleline(&mut self.console_input)
                .hint_text("admin command, e.g. server_abort")
                .desired_width(f32::INFINITY),
        );
        let submit = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if submit
            && connected
            && let Some(request) = console_request(&self.console_input)
        {
            let echo = format!("> {}", self.console_input.trim());
            self.push_console(echo);
            self.console_request = Some(cx.bridge.request(request));
            self.console_input.clear();
            response.request_focus();
        }
        egui::ScrollArea::vertical()
            .stick_to_bottom(true)
            .max_height(260.0)
            .show(ui, |ui| {
                for line in &self.console_output {
                    ui.label(egui::RichText::new(line.as_str()).monospace());
                }
            });
    }

    fn on_event(&mut self, event: &BridgeEvent) {
        self.apply(event, Instant::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action_of(request: Request) -> GameAction {
        match request {
            Request::GameAdmin { server, action } => {
                assert_eq!(server.as_deref(), Some("game"));
                action
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn announcements_are_trimmed_and_not_empty() {
        assert!(announce_request("   ", false).is_none());
        assert_eq!(
            action_of(announce_request("  Hello ", true).unwrap()),
            GameAction::Announce {
                text: "Hello".into(),
                critical: true
            }
        );
    }

    #[test]
    fn the_console_takes_commands_with_or_without_slashes() {
        assert!(console_request("  // ").is_none());
        for line in ["//kick Amy", "kick Amy", " /kick Amy "] {
            assert_eq!(
                action_of(console_request(line).unwrap()),
                GameAction::Command {
                    command: "kick Amy".into()
                }
            );
        }
    }

    #[test]
    fn shutdown_needs_a_number() {
        assert_eq!(
            action_of(shutdown_request(" 60 ", true).unwrap()),
            GameAction::Shutdown {
                seconds: 60,
                restart: true
            }
        );
        assert_eq!(
            shutdown_request("soon", false).unwrap_err(),
            "'soon' is not a number of seconds"
        );
        assert!(shutdown_request("-5", false).is_err());
    }

    #[test]
    fn replies_become_lines_or_errors() {
        let ok = Ok(Response::GameReply {
            lines: vec!["a".into(), "b".into()],
        });
        assert_eq!(reply_lines(&ok).unwrap(), ["a", "b"]);
        let err = Ok(Response::Error {
            message: "no admin connection to 'game'".into(),
        });
        assert_eq!(reply_lines(&err).unwrap_err(), "no admin connection to 'game'");
        assert_eq!(reply_lines(&Err("closed".into())).unwrap_err(), "closed");
        assert!(reply_lines(&Ok(Response::Ok)).is_err());
    }

    #[test]
    fn info_is_readable() {
        let info = GameInfo {
            uptime_secs: 90_061,
            online: 3,
            memory_bytes: 512 * 0x10_0000,
            threads: 0,
            ticks: 100,
            tick_ms: 50,
            tick_stats: Default::default(),
            shutdown_in_secs: Some(30),
        };
        assert_eq!(
            info_lines(&info),
            [
                "Uptime: 1d 01:01:01",
                "Online: 3",
                "Memory: 512 MB",
                "Threads: unknown",
                "Ticks: 100 (50 ms each)",
                "Shutdown countdown: 30 s left"
            ]
        );
        assert_eq!(format_duration(59), "00:00:59");
    }

    #[test]
    fn answers_go_to_the_request_that_asked() {
        let now = Instant::now();
        let mut view = GameAdminView {
            info_request: Some(1),
            status_request: Some(2),
            console_request: Some(3),
            ..GameAdminView::default()
        };
        let info = GameInfo {
            uptime_secs: 1,
            online: 2,
            memory_bytes: 0,
            threads: 0,
            ticks: 0,
            tick_ms: 50,
            tick_stats: Default::default(),
            shutdown_in_secs: None,
        };
        view.apply(
            &BridgeEvent::Response {
                id: 1,
                result: Ok(Response::GameInfo { info: info.clone() }),
            },
            now,
        );
        assert_eq!(view.info, Some(info));
        assert!(!view.info_due(true, now));
        assert!(view.info_due(true, now + INFO_REFRESH));

        view.apply(
            &BridgeEvent::Response {
                id: 2,
                result: Ok(Response::GameReply { lines: vec![] }),
            },
            now,
        );
        assert_eq!(view.status, Some(Ok("Done.".into())));

        view.apply(
            &BridgeEvent::Response {
                id: 3,
                result: Ok(Response::GameReply {
                    lines: vec!["one".into(), "two".into()],
                }),
            },
            now,
        );
        assert_eq!(view.console_output, ["one", "two"]);

        // An error of the console shows in the console, not on the status line.
        view.console_request = Some(4);
        view.apply(
            &BridgeEvent::Response {
                id: 4,
                result: Err("closed".into()),
            },
            now,
        );
        assert_eq!(view.console_output.last().unwrap(), "Error: closed");

        view.apply(&BridgeEvent::Disconnected, now);
        assert!(view.info.is_none() && view.info_request.is_none());
    }

    #[test]
    fn the_console_keeps_the_last_lines() {
        let mut view = GameAdminView::default();
        for i in 0..CONSOLE_LINES + 10 {
            view.push_console(format!("line {i}"));
        }
        assert_eq!(view.console_output.len(), CONSOLE_LINES);
        assert_eq!(view.console_output[0], "line 10");
    }
}
