//! Connection view: where the supervisor is (run folder), optional address and token overrides,
//! theme, connect/disconnect, and stopping the supervisor.
//!
//! The smallest complete [`View`]: it only reads and writes shared state through [`AppCtx`].

use egui::Align2;

use crate::app::{AppCtx, describe_state};
use crate::launch::{self, LaunchState};
use crate::settings::Theme;
use crate::views::View;
use l2_supervisor::launcher::Problem;

#[derive(Default)]
pub struct ConnectionView {
    show_token: bool,
    /// The text of the "Choose run folder" field; `None` = not edited yet (shows the current one).
    run_folder_input: Option<String>,
    confirm_stop: bool,
    /// The other supervisor the user is asked to replace.
    confirm_take_over: Option<std::net::SocketAddr>,
}

impl View for ConnectionView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut AppCtx<'_>) {
        ui.heading("Supervisor connection");
        ui.add_space(8.0);

        self.run_folder_row(ui, cx);
        ui.add_space(8.0);

        egui::Grid::new("connection_settings")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label("Address");
                ui.add(
                    egui::TextEdit::singleline(&mut cx.settings.url)
                        .hint_text("from supervisor.ini (Listen)")
                        .desired_width(320.0),
                );
                ui.end_row();

                ui.label("Token");
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut cx.settings.token)
                            .password(!self.show_token)
                            .hint_text("optional: from supervisor.ini")
                            .desired_width(240.0),
                    );
                    ui.checkbox(&mut self.show_token, "Show");
                });
                ui.end_row();

                ui.label("Theme");
                egui::ComboBox::from_id_salt("theme")
                    .selected_text(cx.settings.theme.label())
                    .show_ui(ui, |ui| {
                        for theme in Theme::ALL {
                            ui.selectable_value(&mut cx.settings.theme, theme, theme.label());
                        }
                    });
                ui.end_row();
            });

        ui.add_space(4.0);
        ui.label("Address and token are read from supervisor.ini. Fill them in only to override the file.");
        ui.checkbox(&mut cx.settings.auto_connect, "Connect on startup");
        if !cx.settings.token.is_empty() {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                "The token override is stored in plain text in this app's settings file on this PC.",
            );
        }

        ui.add_space(12.0);
        ui.horizontal(|ui| {
            let busy = cx.launch.state == LaunchState::Resolving;
            let label = if cx.conn.is_active() {
                "Reconnect"
            } else {
                "Connect"
            };
            if ui
                .add_enabled(!busy, egui::Button::new(label))
                .on_hover_text("Starts the supervisor first if nobody listens")
                .clicked()
            {
                cx.connect();
            }
            if ui
                .add_enabled(cx.conn.is_active(), egui::Button::new("Disconnect"))
                .clicked()
            {
                cx.disconnect();
            }
            if ui
                .add_enabled(cx.conn.is_connected(), egui::Button::new("Stop supervisor"))
                .on_hover_text("Stops all servers cleanly, then the supervisor exits")
                .clicked()
            {
                self.confirm_stop = true;
            }
        });

        ui.add_space(12.0);
        ui.label(launch::status_text(&cx.launch.state, cx.conn.is_connected()));
        ui.label(describe_state(cx.conn, std::time::Instant::now()));
        if launch::needs_setup(&cx.launch.state) {
            ui.colored_label(ui.visuals().warn_fg_color, launch::SETUP_HINT);
        }
        if let Some(notice) = &cx.launch.notice {
            ui.colored_label(ui.visuals().error_fg_color, notice);
        }
        if let LaunchState::Problem(Problem::OtherSupervisor(listen)) = cx.launch.state {
            ui.add_space(8.0);
            ui.colored_label(
                ui.visuals().warn_fg_color,
                "Another supervisor (an older package or a copy from another folder) is running on this port.",
            );
            if ui
                .button("Stop it and start this one")
                .on_hover_text(
                    "Stops the other supervisor and its servers, then starts the one of this folder",
                )
                .clicked()
            {
                self.confirm_take_over = Some(listen);
            }
        }

        self.confirm_stop_dialog(ui, cx);
        self.confirm_take_over_dialog(ui, cx);
    }
}

impl ConnectionView {
    fn run_folder_row(&mut self, ui: &mut egui::Ui, cx: &AppCtx<'_>) {
        let current = cx
            .launch
            .run_folder
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        let input = self.run_folder_input.get_or_insert_with(|| current.clone());
        ui.horizontal(|ui| {
            ui.label("Run folder");
            ui.add(
                egui::TextEdit::singleline(input)
                    .hint_text("the folder with supervisor.ini")
                    .desired_width(360.0),
            );
            if ui
                .button("Choose run folder")
                .on_hover_text("Use this folder and connect")
                .clicked()
                && !input.trim().is_empty()
            {
                cx.launch.use_run_folder(input.trim().to_string());
            }
        });
        // Follow the folder the launcher found, unless the user is typing another.
        if !current.is_empty() && self.run_folder_input.as_deref() == Some("") {
            self.run_folder_input = Some(current);
        }
    }

    fn confirm_take_over_dialog(&mut self, ui: &mut egui::Ui, cx: &AppCtx<'_>) {
        let Some(listen) = self.confirm_take_over else {
            return;
        };
        let mut open = true;
        let mut answer = None;
        egui::Window::new("Replace the other supervisor?")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ui.ctx(), |ui| {
                ui.label(format!(
                    "The supervisor on port {} is stopped together with its servers.",
                    listen.port()
                ));
                ui.label("Players on those servers are disconnected. Then this folder's supervisor starts.");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Replace it").clicked() {
                        answer = Some(true);
                    }
                    if ui.button("Cancel").clicked() {
                        answer = Some(false);
                    }
                });
            });
        match answer {
            Some(true) => {
                self.confirm_take_over = None;
                cx.launch.take_over(listen);
            }
            Some(false) => self.confirm_take_over = None,
            None if !open => self.confirm_take_over = None,
            None => {}
        }
    }

    fn confirm_stop_dialog(&mut self, ui: &mut egui::Ui, cx: &AppCtx<'_>) {
        if !self.confirm_stop {
            return;
        }
        let mut open = true;
        let mut answer = None;
        egui::Window::new("Stop the supervisor?")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ui.ctx(), |ui| {
                ui.label("All servers are stopped (in reverse order), then the supervisor exits.");
                ui.label("Players will be disconnected.");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Stop everything").clicked() {
                        answer = Some(true);
                    }
                    if ui.button("Cancel").clicked() {
                        answer = Some(false);
                    }
                });
            });
        match answer {
            Some(true) => {
                self.confirm_stop = false;
                cx.launch.stop_supervisor();
            }
            Some(false) => self.confirm_stop = false,
            None if !open => self.confirm_stop = false,
            None => {}
        }
    }
}
