//! The egui window: connection fields, the two schema check boxes, Install, progress and log.
// The installation runs on its own thread with a small tokio runtime. It talks to the window
// through a channel; its questions (database exists, statement failed) are events with a reply
// channel, and the worker blocks until the window answers.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};

use egui::{Color32, RichText};

use crate::background::{self, Event};
use crate::console::{DEFAULT_DB, DEFAULT_HOST, DEFAULT_PORT, DEFAULT_USER};
use crate::installer::{self, ErrorChoice, ExistsChoice, Level, Params};

enum Question {
    Exists {
        db_name: String,
        reply: Sender<ExistsChoice>,
    },
    Error {
        error: String,
        reply: Sender<ErrorChoice>,
    },
}

pub struct InstallerApp {
    sql_dir: PathBuf,
    dark: bool,
    host: String,
    port: String,
    user: String,
    password: String,
    db_name: String,
    login: bool,
    game: bool,
    log: Vec<(Level, String)>,
    progress: f32,
    running: bool,
    events: Option<Receiver<Event>>,
    question: Option<Question>,
}

impl InstallerApp {
    pub fn new(sql_dir: PathBuf, dark: bool) -> Self {
        Self {
            sql_dir,
            dark,
            host: DEFAULT_HOST.to_owned(),
            port: DEFAULT_PORT.to_owned(),
            user: DEFAULT_USER.to_owned(),
            password: String::new(),
            db_name: DEFAULT_DB.to_owned(),
            login: true,
            game: true,
            log: Vec::new(),
            progress: 0.0,
            running: false,
            events: None,
            question: None,
        }
    }

    fn push_log(&mut self, level: Level, text: impl Into<String>) {
        self.log.push((level, text.into()));
    }

    /// Starts the worker. Nothing touches a database before this is called.
    fn start(&mut self, ctx: &egui::Context) {
        let Ok(port) = self.port.trim().parse::<u16>() else {
            self.push_log(Level::Error, format!("Invalid port: {}", self.port.trim()));
            return;
        };
        let params = Params {
            options: installer::connect_options(
                self.host.trim(),
                port,
                self.user.trim(),
                self.password.trim(),
            ),
            db_name: self.db_name.trim().to_owned(),
            sql_dir: self.sql_dir.clone(),
            login: self.login,
            game: self.game,
        };
        match background::spawn_install(params, ctx.clone()) {
            Ok(rx) => {
                self.events = Some(rx);
                self.running = true;
                self.progress = 0.0;
            }
            Err(e) => self.push_log(Level::Error, format!("Cannot start the installer: {e}")),
        }
    }

    fn pump(&mut self) {
        let mut events = Vec::new();
        if let Some(rx) = &self.events {
            while let Ok(event) = rx.try_recv() {
                events.push(event);
            }
        }
        for event in events {
            match event {
                Event::Log(level, text) => self.push_log(level, text),
                Event::Progress(p) => self.progress = p,
                Event::AskExists { db_name, reply } => {
                    self.question = Some(Question::Exists { db_name, reply });
                }
                Event::AskError { error, reply } => {
                    self.question = Some(Question::Error { error, reply });
                }
                Event::Done => {
                    self.running = false;
                    self.events = None;
                }
            }
        }
    }

    fn question_window(&mut self, ctx: &egui::Context) {
        let mut answered = false;
        match &self.question {
            Some(Question::Exists { db_name, reply }) => {
                egui::Window::new("Database Exists")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        ui.label(format!("Database '{db_name}' already exists."));
                        ui.label("Do you want to reset it? This deletes ALL data in that database.");
                        ui.horizontal(|ui| {
                            if ui
                                .button(RichText::new("Delete and Recreate").color(Color32::LIGHT_RED))
                                .clicked()
                            {
                                let _ = reply.send(ExistsChoice::Reset);
                                answered = true;
                            }
                            if ui.button("Install on Existing Database").clicked() {
                                let _ = reply.send(ExistsChoice::Keep);
                                answered = true;
                            }
                            if ui.button("Cancel").clicked() {
                                let _ = reply.send(ExistsChoice::Cancel);
                                answered = true;
                            }
                        });
                    });
            }
            Some(Question::Error { error, reply }) => {
                egui::Window::new("Script Error")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        ui.label(format!("MySQL Error: {error}"));
                        ui.horizontal(|ui| {
                            if ui.button("Continue").clicked() {
                                let _ = reply.send(ErrorChoice::Continue);
                                answered = true;
                            }
                            if ui.button("Abort").clicked() {
                                let _ = reply.send(ErrorChoice::Abort);
                                answered = true;
                            }
                        });
                    });
            }
            None => {}
        }
        if answered {
            self.question = None;
        }
    }

    fn form(&mut self, ui: &mut egui::Ui) {
        let editable = !self.running;
        ui.add_enabled_ui(editable, |ui| {
            ui.label("Host:");
            ui.text_edit_singleline(&mut self.host);
            ui.label("Port:");
            ui.text_edit_singleline(&mut self.port);
            ui.label("Username:");
            ui.text_edit_singleline(&mut self.user);
            ui.label("Password:");
            ui.add(egui::TextEdit::singleline(&mut self.password).password(true));
            ui.label("Database:");
            ui.text_edit_singleline(&mut self.db_name);
            ui.add_space(6.0);
            ui.checkbox(&mut self.login, "Install Login");
            ui.checkbox(&mut self.game, "Install Game");
        });
        ui.add_space(6.0);
        let can_install = !self.running && (self.login || self.game);
        if ui
            .add_enabled(can_install, egui::Button::new("Install Database"))
            .clicked()
        {
            let ctx = ui.ctx().clone();
            self.start(&ctx);
        }
    }

    fn log_view(&self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .stick_to_bottom(true)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (level, text) in &self.log {
                    let color = match level {
                        Level::Info => ui.visuals().text_color(),
                        Level::Success => Color32::from_rgb(0x3c, 0xb3, 0x71),
                        Level::Error => Color32::from_rgb(0xd9, 0x4f, 0x4f),
                    };
                    ui.label(RichText::new(text).color(color).monospace());
                }
            });
    }
}

impl eframe::App for InstallerApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_theme(if self.dark {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        });
        self.pump();
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::left("form")
            .resizable(false)
            .exact_size(210.0)
            .show(ui, |ui| self.form(ui));
        egui::Panel::bottom("progress").show(ui, |ui| {
            ui.add(egui::ProgressBar::new(self.progress).show_percentage());
        });
        egui::CentralPanel::default().show(ui, |ui| self.log_view(ui));
        let ctx = ui.ctx().clone();
        self.question_window(&ctx);
    }
}

/// Opens the window. Returns an error when no window can be created (no display), so the caller
/// can fall back to the console.
pub fn run(sql_dir: PathBuf, dark: bool) -> Result<(), String> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Mobius - Database Installer")
            .with_inner_size([720.0, 460.0])
            .with_min_inner_size([620.0, 400.0]),
        ..Default::default()
    };
    eframe::run_native(
        "l2-db-installer",
        options,
        Box::new(move |_cc| Ok(Box::new(InstallerApp::new(sql_dir, dark)))),
    )
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_the_java_ones() {
        let app = InstallerApp::new(PathBuf::from("sql"), true);
        assert_eq!(app.host, "localhost");
        assert_eq!(app.port, "3306");
        assert_eq!(app.user, "root");
        assert_eq!(app.db_name, "l2jmobiush5");
        assert!(app.login && app.game);
        assert!(!app.running && app.events.is_none());
    }
}
