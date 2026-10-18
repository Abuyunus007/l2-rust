//! The eframe app: window layout (status bar, navigation, central view) and the glue between
//! the bridge, the settings and the views.

use std::time::{Duration, Instant};

use egui::Color32;
use l2_admin_api::Request;

use crate::bridge::{Bridge, BridgeEvent, ConnState, Target};
use crate::settings::{Settings, Theme};
use crate::views::{ViewId, Views};

/// Everything a view may use besides its own state. Built fresh each frame.
pub struct AppCtx<'a> {
    pub settings: &'a mut Settings,
    pub conn: &'a ConnState,
    pub bridge: &'a Bridge,
}

impl AppCtx<'_> {
    /// Connects to the address in the settings (reconnects if already connected).
    pub fn connect(&self) {
        self.bridge.connect(target_of(self.settings));
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
}

impl AdminApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> std::io::Result<Self> {
        let settings = Settings::load(cc.storage);
        let bridge = Bridge::spawn(Some(cc.egui_ctx.clone()))?;
        if settings.should_auto_connect() {
            bridge.connect(target_of(&settings));
        }
        Ok(Self {
            settings,
            bridge,
            conn: ConnState::default(),
            views: Views::default(),
            selected: ViewId::Connection,
            applied_theme: None,
        })
    }
}

fn target_of(settings: &Settings) -> Target {
    Target {
        url: settings.ws_url(),
        token: settings.token.clone(),
    }
}

impl AdminApp {
    fn pump_events(&mut self) {
        while let Some(event) = self.bridge.try_recv() {
            self.conn.apply(&event, Instant::now());
            if matches!(event, BridgeEvent::Connected { .. }) {
                // The supervisor only starts pushing events to a client after its first
                // request (ADM-02), so ask for the server list right away. The answer reaches
                // the views as an ordinary `BridgeEvent::Response`.
                self.bridge.request(Request::ListServers);
            }
            self.views.dispatch(&event);
        }
    }

    fn status_bar(&self, ui: &mut egui::Ui) {
        // Address on the right; the state text gets the rest and is cut off (full text on hover)
        // so a long OS error message cannot push the address out of the window.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(self.settings.ws_url());
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                let (dot, color) = state_indicator(&self.conn);
                ui.colored_label(color, dot);
                let text = describe_state(&self.conn, Instant::now());
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
