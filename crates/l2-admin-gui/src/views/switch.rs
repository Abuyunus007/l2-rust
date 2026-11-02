//! "Switch to Rust" assistant (M9-S01), a section of the Setup view. Three steps, each needs a
//! click: 1 prepare (backup through the supervisor, login server's server list), 2 switch (writes
//! the `.ini` changes), 3 check (the in-game list of `docs/runbooks/m9-switch.md`). Nothing is
//! written before the click on "Apply the switch"; "Back to side by side" reverses it.
//!
//! The decisions are in [`super::switch_core`] (tested there).

use std::time::{Instant, SystemTime, UNIX_EPOCH};

use egui::Color32;
use l2_admin_api::{LoginStatus, Request, Response};

use super::setup_core::Layout;
use super::switch_core::{self as core, BackupOutcome, Current, Gate, IdChoice};
use crate::app::AppCtx;
use crate::bridge::BridgeEvent;

const LOGIN_SERVER: &str = "login";
const POLL_EVERY: std::time::Duration = std::time::Duration::from_secs(1);

#[derive(Debug, Default)]
enum Backup {
    #[default]
    Idle,
    /// `BackupNow` sent; waiting for its answer.
    Requested {
        request: u64,
        started_ms: i64,
    },
    /// Asking `BackupState` until the run is over.
    Polling {
        started_ms: i64,
        request: Option<u64>,
        next: Option<Instant>,
    },
    Ok(String),
    Failed(String),
}

#[derive(Default)]
pub struct SwitchAssistant {
    /// 0 = prepare, 1 = switch, 2 = check. Moves on only with a click.
    step: u8,
    backup: Backup,
    login: Option<LoginStatus>,
    login_request: Option<u64>,
    login_error: Option<String>,
    take_java_id: Option<bool>,
    message: Option<(bool, String)>,
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn ok_color() -> Color32 {
    Color32::from_rgb(0x3c, 0xb3, 0x71)
}

impl SwitchAssistant {
    fn choice(&self) -> IdChoice {
        if self.take_java_id.unwrap_or(true) {
            IdChoice::TakeJava
        } else {
            IdChoice::Keep
        }
    }

    fn backup_ok(&self) -> bool {
        matches!(self.backup, Backup::Ok(_))
    }

    pub fn on_event(&mut self, event: &BridgeEvent) {
        match event {
            BridgeEvent::Disconnected | BridgeEvent::Rejected { .. } => {
                self.login_request = None;
                // A backup that was running goes on at the supervisor; ask again after reconnecting.
                if let Backup::Requested { started_ms, .. } | Backup::Polling { started_ms, .. } = self.backup
                {
                    self.backup = Backup::Polling {
                        started_ms,
                        request: None,
                        next: None,
                    };
                }
            }
            BridgeEvent::Response { id, result } => self.on_response(*id, result),
            _ => {}
        }
    }

    fn on_response(&mut self, id: u64, result: &Result<Response, String>) {
        if self.login_request == Some(id) {
            self.login_request = None;
            match result {
                Ok(Response::LoginStatus { status }) => {
                    self.login = Some(status.clone());
                    self.login_error = None;
                }
                Ok(Response::Error { message }) | Err(message) => {
                    self.login = None;
                    self.login_error = Some(message.clone());
                }
                Ok(other) => self.login_error = Some(format!("unexpected answer: {other:?}")),
            }
            return;
        }
        match &mut self.backup {
            Backup::Requested { request, started_ms } if *request == id => {
                let started_ms = *started_ms;
                self.backup = match result {
                    Ok(Response::Error { message }) | Err(message) => Backup::Failed(message.clone()),
                    Ok(_) => Backup::Polling {
                        started_ms,
                        request: None,
                        next: None,
                    },
                };
            }
            Backup::Polling {
                started_ms,
                request,
                next,
            } if *request == Some(id) => {
                *request = None;
                match result {
                    Ok(Response::BackupState { state }) => match core::backup_outcome(state, *started_ms) {
                        BackupOutcome::Running => *next = Some(Instant::now() + POLL_EVERY),
                        BackupOutcome::Ok(file) => self.backup = Backup::Ok(file),
                        BackupOutcome::Failed(why) => self.backup = Backup::Failed(why),
                    },
                    Ok(Response::Error { message }) | Err(message) => {
                        self.backup = Backup::Failed(message.clone())
                    }
                    Ok(_) => *next = Some(Instant::now() + POLL_EVERY),
                }
            }
            _ => {}
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, layout: &Layout, cx: &AppCtx<'_>) {
        let connected = cx.conn.is_connected();
        self.pump_backup(ui, cx, connected);
        ui.label(
            "Rust replaces Java: the Rust game server takes the Java server's port, and every SideBySide.ini switch goes to True. Read docs/runbooks/m9-switch.md first. Nothing changes without a click.",
        );
        let current = match core::read_current(layout) {
            Ok(c) => c,
            Err(e) => {
                ui.colored_label(
                    ui.visuals().error_fg_color,
                    format!("Cannot read the settings: {e}"),
                );
                return;
            }
        };
        ui.add_space(4.0);
        ui.strong(format!("Step {} of 3", self.step + 1));
        match self.step {
            0 => self.step_prepare(ui, cx, connected, &current),
            1 => self.step_switch(ui, layout, &current),
            _ => self.step_check(ui, layout),
        }
        ui.add_space(6.0);
        if let Some((ok, text)) = &self.message {
            let color = if *ok {
                ok_color()
            } else {
                ui.visuals().error_fg_color
            };
            ui.colored_label(color, text);
        }
        if self.step < 2 && ui.small_button("Start over").clicked() {
            *self = Self::default();
        }
    }

    fn pump_backup(&mut self, ui: &egui::Ui, cx: &AppCtx<'_>, connected: bool) {
        if let Backup::Polling { request, next, .. } = &mut self.backup
            && request.is_none()
            && connected
        {
            match next {
                Some(at) if *at > Instant::now() => ui.ctx().request_repaint_after(*at - Instant::now()),
                _ => {
                    *request = Some(cx.bridge.request(Request::BackupState));
                    *next = None;
                }
            }
        }
    }

    fn step_prepare(&mut self, ui: &mut egui::Ui, cx: &AppCtx<'_>, connected: bool, current: &Current) {
        ui.label("1. Prepare: back up the database, then stop the Java game server (Sieghardt).");
        if !connected {
            ui.colored_label(ui.visuals().warn_fg_color, "Not connected to the supervisor.");
        }
        ui.horizontal(|ui| {
            let idle = matches!(self.backup, Backup::Idle | Backup::Ok(_) | Backup::Failed(_));
            if ui
                .add_enabled(connected && idle, egui::Button::new("Back up the database now"))
                .clicked()
            {
                let started_ms = now_ms();
                self.backup = Backup::Requested {
                    request: cx.bridge.request(Request::BackupNow),
                    started_ms,
                };
                self.message = None;
            }
            match &self.backup {
                Backup::Idle => {
                    ui.label("No backup yet.");
                }
                Backup::Requested { .. } | Backup::Polling { .. } => {
                    ui.spinner();
                    ui.label("Backup running...");
                }
                Backup::Ok(file) => {
                    ui.colored_label(ok_color(), format!("Backup done: {file}"));
                }
                Backup::Failed(why) => {
                    ui.colored_label(ui.visuals().error_fg_color, format!("Backup failed: {why}"));
                }
            }
        });

        ui.horizontal(|ui| {
            let can = connected && self.login_request.is_none();
            if ui
                .add_enabled(can, egui::Button::new("Ask the login server"))
                .on_hover_text("Reads the list of registered game servers")
                .clicked()
            {
                self.login_request = Some(cx.bridge.request(Request::GetLoginStatus {
                    server: Some(LOGIN_SERVER.to_string()),
                }));
            }
            if let Some(error) = &self.login_error {
                ui.colored_label(ui.visuals().error_fg_color, format!("No answer: {error}"));
            }
        });
        if let Some(status) = &self.login {
            for s in &status.game_servers {
                let state = if s.authed { "registered" } else { "down" };
                ui.label(format!("  {} (id {}): {state}", s.name, s.id));
            }
        }
        let refusal = core::java_registered(self.login.as_ref(), current.server_id_number());
        match &refusal {
            Some(why) => {
                ui.colored_label(ui.visuals().warn_fg_color, why);
            }
            None => {
                ui.colored_label(ok_color(), "No Java game server is registered.");
            }
        }
        ui.label(format!(
            "Write down: Rust port {} and server id {} now; Java uses port {} and id {}.",
            current.game_port.trim(),
            current.server_id.trim(),
            core::JAVA_PORT,
            core::JAVA_SERVER_ID
        ));
        let ready = self.backup_ok() && refusal.is_none();
        if ui
            .add_enabled(ready, egui::Button::new("Continue to step 2"))
            .on_disabled_hover_text("Needs a finished backup and no registered Java game server")
            .clicked()
        {
            self.step = 1;
            self.message = None;
        }
    }

    fn step_switch(&mut self, ui: &mut egui::Ui, layout: &Layout, current: &Current) {
        ui.label("2. Switch: what will be written.");
        let mut take = self.take_java_id.unwrap_or(true);
        ui.radio_value(
            &mut take,
            true,
            format!("Take the Java server's id ({})", core::JAVA_SERVER_ID),
        );
        ui.radio_value(&mut take, false, "Keep the Rust server's own id");
        self.take_java_id = Some(take);
        ui.label("With id 2 the Rust server needs the hexid.txt of the Java server (see the runbook).");
        let plan = core::plan_forward(current, self.choice());
        if current.all_switches_on() {
            ui.label("All SideBySide.ini switches are True already.");
        }
        if plan.is_empty() {
            ui.label("Nothing to change: the settings are already switched.");
        }
        egui::Grid::new("switch_plan")
            .num_columns(3)
            .striped(true)
            .show(ui, |ui| {
                for (key, value) in &current.switches {
                    ui.label(*key);
                    ui.label(value.as_str());
                    ui.label(if plan.iter().any(|c| c.key == *key) {
                        "-> True"
                    } else {
                        "(stays)"
                    });
                    ui.end_row();
                }
                for (key, value) in [
                    (core::PORT_KEY, &current.game_port),
                    (core::ID_KEY, &current.server_id),
                ] {
                    ui.label(key);
                    ui.label(value.as_str());
                    match plan.iter().find(|c| c.key == key) {
                        Some(c) => ui.label(format!("-> {}", c.to)),
                        None => ui.label("(stays)"),
                    };
                    ui.end_row();
                }
            });
        ui.horizontal(|ui| {
            if ui.button("Apply the switch").clicked() {
                let gate = Gate {
                    backup_ok: self.backup_ok(),
                    login: self.login.as_ref(),
                };
                self.message = Some(match core::apply_forward(layout, self.choice(), gate) {
                    Ok(n) => {
                        self.step = 2;
                        (
                            true,
                            format!("{n} values written. Restart the Rust game server to use them."),
                        )
                    }
                    Err(e) => (false, format!("Not written: {e}")),
                });
            }
            if ui.button("Back to step 1").clicked() {
                self.step = 0;
                self.message = None;
            }
        });
    }

    fn step_check(&mut self, ui: &mut egui::Ui, layout: &Layout) {
        ui.label("3. Check: restart the Rust game server, then in the game:");
        for line in CHECKS {
            ui.label(format!("  - {line}"));
        }
        ui.label("The full list and the way back are in docs/runbooks/m9-switch.md.");
        if core::has_snapshot(layout) {
            ui.label("The values from before are kept in switch-before.ini (run folder).");
        }
        if ui
            .button("Back to side by side")
            .on_hover_text("Writes the old values back; restart the game server afterwards")
            .clicked()
        {
            self.message = Some(match core::apply_back(layout) {
                Ok(n) => {
                    self.step = 0;
                    self.backup = Backup::Idle;
                    (
                        true,
                        format!("{n} values restored. Restart the Rust game server."),
                    )
                }
                Err(e) => (false, format!("Not written: {e}")),
            });
        }
    }
}

/// Short form of the checks of `docs/runbooks/m9-switch.md`.
const CHECKS: [&str; 6] = [
    "The login server's list shows the Rust server up, on the Java port.",
    "Log in with a test account, pick a character, walk around, fight, loot.",
    "Mail and the clan warehouse show the same items as before.",
    "Siege, Olympiad and Seven Signs pages show the state the Java server left.",
    "The daily reset, clan maintenance and offline shops write to the database (log lines).",
    "Log out and in again: the character is where it was.",
];
