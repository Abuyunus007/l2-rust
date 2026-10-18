//! Connection view: supervisor address, token, theme, connect/disconnect.
//!
//! The smallest complete [`View`]: it only reads and writes shared state through [`AppCtx`].

use crate::app::{AppCtx, describe_state};
use crate::settings::Theme;
use crate::views::View;

#[derive(Default)]
pub struct ConnectionView {
    show_token: bool,
}

impl View for ConnectionView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut AppCtx<'_>) {
        ui.heading("Supervisor connection");
        ui.add_space(8.0);

        egui::Grid::new("connection_settings")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label("Address");
                ui.add(
                    egui::TextEdit::singleline(&mut cx.settings.url)
                        .hint_text("ws://127.0.0.1:7900")
                        .desired_width(320.0),
                );
                ui.end_row();

                ui.label("Token");
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut cx.settings.token)
                            .password(!self.show_token)
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
        ui.checkbox(&mut cx.settings.auto_connect, "Connect on startup");
        ui.colored_label(
            ui.visuals().warn_fg_color,
            "The token is stored in plain text in this app's settings file on this PC.",
        );

        ui.add_space(12.0);
        ui.horizontal(|ui| {
            let can_connect = cx.settings.can_connect();
            let label = if cx.conn.is_active() {
                "Reconnect"
            } else {
                "Connect"
            };
            if ui
                .add_enabled(can_connect, egui::Button::new(label))
                .on_disabled_hover_text("Enter an address and a token first")
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
        });

        ui.add_space(12.0);
        ui.label(describe_state(cx.conn, std::time::Instant::now()));
    }
}
