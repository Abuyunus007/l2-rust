//! Setup wizard (G-02): turns a fresh checkout or package into a working install without editing a
//! file by hand. One scrolling page with five steps; every step can be repeated, and the page
//! shows the values that are in the files now.
//!
//! The decisions live in [`super::setup_core`] (tested there). The database work runs on the
//! installer's own library ([`l2_db_installer::background`]): nothing connects before the user
//! presses "Test connection" or "Install database". The password is held in this view only and
//! written to `Database.ini`; it is never logged and not part of [`crate::settings::Settings`].

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

use egui::{Color32, RichText};
use l2_admin_api::{Request, Response};
use l2_db_installer::background::{self, Event as InstallEvent, TestResult};
use l2_db_installer::installer::{self, DatabaseState, ErrorChoice, ExistsChoice, Level};

use super::View;
use super::setup_core::{
    self as core, Check, CopyPlan, DbTest, Form, Layout, LayoutKind, Status, count_geodata, execute_copy,
    plan_copy, run_checks,
};
use crate::app::AppCtx;
use crate::bridge::BridgeEvent;
use crate::launch::{self, LaunchState};

/// How often the checks are read again (they look at the disk).
const CHECK_INTERVAL: Duration = Duration::from_secs(2);
/// Servers "Start servers" starts, in this order. The Java game server is started by hand.
const START_ORDER: [&str; 2] = ["login", "game"];

#[derive(Default)]
pub struct SetupView {
    state: Option<State>,
}

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

struct Install {
    rx: Option<Receiver<InstallEvent>>,
    log: Vec<(Level, String)>,
    progress: f32,
    question: Option<Question>,
}

#[derive(Default)]
struct Starting {
    /// The user pressed "Start servers" and the supervisor is not connected yet.
    waiting_for_connection: bool,
    queue: VecDeque<&'static str>,
    inflight: Option<(u64, &'static str)>,
    messages: Vec<(bool, String)>,
}

struct State {
    layout: Option<Layout>,
    gui_dir: PathBuf,
    form: Form,
    plan: CopyPlan,
    overwrite: bool,
    copy_message: Option<(bool, String)>,
    apply_message: Option<(bool, String)>,
    show_password: bool,
    db: DbTest,
    test_rx: Option<Receiver<TestResult>>,
    install: Install,
    starting: Starting,
    checks: Vec<Check>,
    checked_at: Option<Instant>,
}

impl SetupView {
    /// A view for a known layout (tests). The real one detects it on the first frame.
    #[cfg(test)]
    fn with_layout(layout: Option<Layout>, gui_dir: PathBuf) -> Self {
        Self {
            state: Some(State::new(layout, gui_dir)),
        }
    }
}

impl State {
    fn detect() -> Self {
        let exe = std::env::current_exe().unwrap_or_default();
        let cwd = std::env::current_dir().unwrap_or_default();
        let gui_dir = exe.parent().map(PathBuf::from).unwrap_or_default();
        Self::new(Layout::detect(&exe, &cwd), gui_dir)
    }

    fn new(layout: Option<Layout>, gui_dir: PathBuf) -> Self {
        let (form, plan) = match &layout {
            Some(l) => (Form::load(l), plan_copy(l).unwrap_or_default()),
            None => (Form::default_without_layout(), CopyPlan::default()),
        };
        Self {
            layout,
            gui_dir,
            form,
            plan,
            overwrite: false,
            copy_message: None,
            apply_message: None,
            show_password: false,
            db: DbTest::NotTested,
            test_rx: None,
            install: Install {
                rx: None,
                log: Vec::new(),
                progress: 0.0,
                question: None,
            },
            starting: Starting::default(),
            checks: Vec::new(),
            checked_at: None,
        }
    }

    /// Reads the files again (the page shows what is in them now).
    fn reload(&mut self) {
        if let Some(layout) = &self.layout {
            // The password the user typed for a test stays if the file has none.
            let typed = std::mem::take(&mut self.form.db_password);
            self.form = Form::load(layout);
            if self.form.db_password.is_empty() {
                self.form.db_password = typed;
            }
            self.plan = plan_copy(layout).unwrap_or_default();
        }
        self.checked_at = None;
    }

    fn installing(&self) -> bool {
        self.install.rx.is_some()
    }

    fn poll(&mut self) {
        if let Some(rx) = &self.test_rx
            && let Ok(result) = rx.try_recv()
        {
            self.db = match result {
                Ok(DatabaseState::Exists) => DbTest::Reachable {
                    database_exists: true,
                },
                Ok(DatabaseState::Missing) => DbTest::Reachable {
                    database_exists: false,
                },
                Err(why) => DbTest::Failed(why),
            };
            self.test_rx = None;
            self.checked_at = None;
        }
        let mut events = Vec::new();
        if let Some(rx) = &self.install.rx {
            while let Ok(event) = rx.try_recv() {
                events.push(event);
            }
        }
        for event in events {
            match event {
                InstallEvent::Log(level, text) => self.install.log.push((level, text)),
                InstallEvent::Progress(p) => self.install.progress = p,
                InstallEvent::AskExists { db_name, reply } => {
                    self.install.question = Some(Question::Exists { db_name, reply });
                }
                InstallEvent::AskError { error, reply } => {
                    self.install.question = Some(Question::Error { error, reply });
                }
                InstallEvent::Done => {
                    self.install.rx = None;
                    // The database may exist now: the old test result is stale.
                    self.db = DbTest::NotTested;
                    self.checked_at = None;
                }
            }
        }
    }
}

impl Form {
    /// The form when no layout was found: nothing to read, nothing to write.
    fn default_without_layout() -> Self {
        Self::defaults(&Layout::repo(std::path::Path::new(".")))
    }
}

impl View for SetupView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut AppCtx<'_>) {
        let state = self.state.get_or_insert_with(State::detect);
        state.poll();
        state.pump_start(cx);

        ui.heading("Setup");
        ui.label("Gets the servers ready: run folder, database, ports, supervisor. Open it again any time; it shows what the files say now.");
        ui.horizontal(|ui| {
            if ui
                .button("Reload from files")
                .on_hover_text("Forget the unsaved changes on this page")
                .clicked()
            {
                state.reload();
            }
        });
        ui.separator();

        let Some(layout) = state.layout.clone() else {
            ui.colored_label(
                ui.visuals().error_fg_color,
                "No dist/ folder (repository) and no login/ + game/ folders (release package) found next to this program or in the current folder.",
            );
            return;
        };

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::CollapsingHeader::new("1. Run folder")
                    .default_open(true)
                    .show(ui, |ui| state.step_run_folder(ui, &layout));
                egui::CollapsingHeader::new("2. Database")
                    .default_open(true)
                    .show(ui, |ui| state.step_database(ui, &layout));
                egui::CollapsingHeader::new("3. Servers")
                    .default_open(true)
                    .show(ui, |ui| state.step_servers(ui));
                egui::CollapsingHeader::new("4. Admin endpoints and supervisor")
                    .default_open(true)
                    .show(ui, |ui| state.step_apply(ui, &layout));
                egui::CollapsingHeader::new("5. Check and start")
                    .default_open(true)
                    .show(ui, |ui| state.step_check(ui, &layout, cx));
            });
        state.question_window(ui.ctx());
    }

    fn on_event(&mut self, event: &BridgeEvent) {
        let Some(state) = &mut self.state else { return };
        if let BridgeEvent::Response { id, result } = event
            && let Some((inflight, name)) = state.starting.inflight
            && inflight == *id
        {
            state.starting.inflight = None;
            let (ok, text) = match result {
                Ok(Response::Error { message }) => (false, format!("{name}: {message}")),
                Ok(_) => (true, format!("{name}: start requested")),
                Err(why) => (false, format!("{name}: {why}")),
            };
            state.starting.messages.push((ok, text));
            // A failed start stops the chain: the next server depends on this one.
            if !ok {
                state.starting.queue.clear();
            }
        }
    }
}

fn message(ui: &mut egui::Ui, message: &Option<(bool, String)>) {
    if let Some((ok, text)) = message {
        let color = if *ok {
            Color32::from_rgb(0x3c, 0xb3, 0x71)
        } else {
            ui.visuals().error_fg_color
        };
        ui.colored_label(color, text);
    }
}

fn field(ui: &mut egui::Ui, label: &str, value: &mut String, width: f32) {
    ui.label(label);
    ui.add(egui::TextEdit::singleline(value).desired_width(width));
    ui.end_row();
}

impl State {
    fn step_run_folder(&mut self, ui: &mut egui::Ui, layout: &Layout) {
        let kind = match layout.kind {
            LayoutKind::Repo => "repository",
            LayoutKind::Package => "release package",
        };
        ui.label(format!("Layout: {kind}"));
        ui.label(format!("Run folder: {}", core::fwd(&layout.run_folder)));
        if layout.dist.is_none() {
            ui.label("The settings are already in place (login/ and game/ of the package).");
            return;
        }
        ui.label("Copies dist/login to run/login and dist/game/config to run/game/config. The datapack stays in dist/game/data.");
        ui.label(format!(
            "{} files to copy, {} already exist.",
            self.plan.create.len(),
            self.plan.existing.len()
        ));
        if !self.plan.existing.is_empty() {
            egui::CollapsingHeader::new(format!("Files that already exist ({})", self.plan.existing.len()))
                .id_salt("existing_files")
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
                        for item in &self.plan.existing {
                            ui.label(RichText::new(core::fwd(&item.to)).monospace());
                        }
                    });
                });
            ui.checkbox(
                &mut self.overwrite,
                "Overwrite the existing files (what you changed in them is lost)",
            );
        }
        let can_copy = !self.plan.create.is_empty() || (self.overwrite && !self.plan.existing.is_empty());
        if ui
            .add_enabled(can_copy, egui::Button::new("Copy files"))
            .on_disabled_hover_text("Nothing to copy")
            .clicked()
        {
            self.copy_message = Some(match execute_copy(&self.plan, self.overwrite) {
                Ok(r) => (
                    true,
                    format!("{} files copied, {} left as they were.", r.copied, r.skipped),
                ),
                Err(e) => (false, format!("Copy failed: {e}")),
            });
            self.plan = plan_copy(layout).unwrap_or_default();
            self.overwrite = false;
            if self.form.switches.is_empty() {
                self.form.switches = core::load_switches(&layout.game_config("SideBySide.ini"));
            }
            self.checked_at = None;
        }
        message(ui, &self.copy_message);
    }

    fn step_database(&mut self, ui: &mut egui::Ui, layout: &Layout) {
        let busy = self.installing() || self.test_rx.is_some();
        ui.add_enabled_ui(!self.installing(), |ui| {
            egui::Grid::new("db_fields")
                .num_columns(2)
                .spacing([12.0, 6.0])
                .show(ui, |ui| {
                    field(ui, "Host", &mut self.form.db_host, 220.0);
                    field(ui, "Port", &mut self.form.db_port, 80.0);
                    field(ui, "User", &mut self.form.db_user, 220.0);
                    ui.label("Password");
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.form.db_password)
                                .password(!self.show_password)
                                .desired_width(220.0),
                        );
                        ui.checkbox(&mut self.show_password, "Show");
                    });
                    ui.end_row();
                    field(ui, "Database name", &mut self.form.db_name, 220.0);
                });
        });
        ui.label("The password is written to the Database.ini files of the run folder only. Nothing connects until you press one of the buttons.");

        ui.horizontal(|ui| {
            if ui
                .add_enabled(!busy, egui::Button::new("Test connection"))
                .clicked()
            {
                self.start_test(ui.ctx());
            }
            if ui
                .add_enabled(!busy, egui::Button::new("Install database"))
                .on_hover_text("Creates the database and runs the login and game SQL scripts. Asks before it deletes anything.")
                .clicked()
            {
                self.start_install(ui.ctx(), layout);
            }
        });
        match &self.db {
            DbTest::NotTested => {}
            DbTest::Testing => {
                ui.label("Testing...");
            }
            DbTest::Reachable {
                database_exists: true,
            } => {
                ui.colored_label(
                    Color32::from_rgb(0x3c, 0xb3, 0x71),
                    "Connected. The database exists.",
                );
            }
            DbTest::Reachable {
                database_exists: false,
            } => {
                ui.colored_label(
                    Color32::from_rgb(0xe0, 0xa0, 0x30),
                    "Connected. The database does not exist yet: press \"Install database\".",
                );
            }
            DbTest::Failed(why) => {
                ui.colored_label(ui.visuals().error_fg_color, format!("Failed: {why}"));
            }
        }
        if !self.install.log.is_empty() || self.installing() {
            ui.add(egui::ProgressBar::new(self.install.progress).show_percentage());
            egui::ScrollArea::vertical()
                .id_salt("install_log")
                .max_height(180.0)
                .stick_to_bottom(true)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for (level, text) in &self.install.log {
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

    fn start_test(&mut self, ctx: &egui::Context) {
        let Some((options, name)) = self.connection(None) else {
            return;
        };
        match background::spawn_test(options, name, ctx.clone()) {
            Ok(rx) => {
                self.test_rx = Some(rx);
                self.db = DbTest::Testing;
            }
            Err(e) => self.db = DbTest::Failed(format!("cannot start the test: {e}")),
        }
    }

    fn start_install(&mut self, ctx: &egui::Context, layout: &Layout) {
        let Some((options, db_name)) = self.connection(Some(layout)) else {
            return;
        };
        let params = installer::Params {
            options,
            db_name,
            sql_dir: layout.sql_dir.clone(),
            login: true,
            game: true,
        };
        self.install.log.clear();
        self.install.progress = 0.0;
        match background::spawn_install(params, ctx.clone()) {
            Ok(rx) => self.install.rx = Some(rx),
            Err(e) => self
                .install
                .log
                .push((Level::Error, format!("Cannot start the installer: {e}"))),
        }
    }

    /// The connection options of the form, or `None` (with the reason on screen) for a bad port
    /// or name.
    fn connection(&mut self, install_in: Option<&Layout>) -> Option<(installer::DbOptions, String)> {
        let name = self.form.db_name.trim().to_owned();
        let port = self.form.db_port.trim().parse::<u16>();
        let problem = match (&port, name.is_empty(), self.form.db_host.trim().is_empty()) {
            (Err(_), _, _) => Some(format!("'{}' is not a port", self.form.db_port.trim())),
            (_, true, _) => Some("The database name is empty".to_owned()),
            (_, _, true) => Some("The host is empty".to_owned()),
            _ => None,
        };
        if let Some(problem) = problem {
            if install_in.is_some() {
                self.install.log.push((Level::Error, problem));
            } else {
                self.db = DbTest::Failed(problem);
            }
            return None;
        }
        let port = port.ok()?;
        Some((
            installer::connect_options(
                self.form.db_host.trim(),
                port,
                self.form.db_user.trim(),
                &self.form.db_password,
            ),
            name,
        ))
    }

    fn step_servers(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("server_fields")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                field(
                    ui,
                    "Login port for game clients (LoginserverPort)",
                    &mut self.form.login_client_port,
                    80.0,
                );
                field(
                    ui,
                    "Login port for game servers (LoginPort)",
                    &mut self.form.login_server_port,
                    80.0,
                );
                field(
                    ui,
                    "Game server port (GameserverPort)",
                    &mut self.form.game_port,
                    80.0,
                );
                field(
                    ui,
                    "Game server id (RequestServerID)",
                    &mut self.form.server_id,
                    80.0,
                );
                field(
                    ui,
                    "Datapack folder (DatapackRoot)",
                    &mut self.form.datapack_root,
                    420.0,
                );
                field(ui, "Geodata folder", &mut self.form.geodata_dir, 420.0);
            });
        if let Some(layout) = &self.layout {
            let found = count_geodata(&core::geodata_path(layout, &self.form));
            if found == 0 {
                ui.colored_label(
                    Color32::from_rgb(0xe0, 0xa0, 0x30),
                    "No .l2j geodata files in this folder. The game server starts without usable geodata.",
                );
            } else {
                ui.label(format!("{found} geodata region files found."));
            }
        }

        ui.add_space(6.0);
        ui.checkbox(
            &mut self.form.java_enabled,
            "A Java game server runs next to this one (same database)",
        );
        if self.form.java_enabled {
            ui.label("Use another game port and server id than the Java server (for example 7778 and 3 when Java has 7777 and 1). The Java entry is added to supervisor.ini; start it by hand.");
            egui::Grid::new("java_fields")
                .num_columns(2)
                .spacing([12.0, 6.0])
                .show(ui, |ui| {
                    field(ui, "Java game server folder", &mut self.form.java_folder, 420.0);
                });
            ui.label("The folder with java.cfg (the libs/ folder is next to it).");
            if self.form.switches.is_empty() {
                ui.label("SideBySide.ini is not in the run folder yet (step 1).");
            }
            ui.label("SideBySide.ini switches. Leave them off while Java runs; turn one on only when this server is the only one on the database:");
            for switch in &mut self.form.switches {
                ui.checkbox(&mut switch.value, &switch.key);
                ui.label(RichText::new(&switch.help).weak().small());
            }
        }
    }

    fn step_apply(&mut self, ui: &mut egui::Ui, layout: &Layout) {
        ui.label("Writes the values of steps 2 and 3 into the .ini files (comments and order stay), switches Enabled = True in both Admin.ini files, and writes supervisor.ini.");
        ui.label("No token is needed: the supervisor gives each server its admin token itself.");
        let exists = layout.supervisor_ini().is_file();
        if exists {
            ui.checkbox(
                &mut self.form.rewrite_supervisor,
                "Replace the existing supervisor.ini (the old one is kept as supervisor.ini.bak)",
            );
        }
        if ui.button("Apply settings").clicked() {
            self.apply_message = Some(match core::apply(layout, &self.form) {
                Ok(report) => {
                    let mut text = format!(
                        "Done. {} files changed, {} already had the values.",
                        report.changed.len(),
                        report.unchanged
                    );
                    if report.supervisor.is_some() {
                        text.push_str(" supervisor.ini written.");
                    }
                    text.push_str(" A running supervisor reads supervisor.ini only at its start: stop it in the Connection view to use the new file.");
                    (true, text)
                }
                Err(e) => (false, format!("Not written: {e}")),
            });
            self.form.rewrite_supervisor = false;
            self.checked_at = None;
        }
        message(ui, &self.apply_message);
    }

    fn step_check(&mut self, ui: &mut egui::Ui, layout: &Layout, cx: &AppCtx<'_>) {
        let due = self.checked_at.is_none_or(|at| at.elapsed() >= CHECK_INTERVAL);
        if due {
            self.checks = run_checks(layout, &self.gui_dir, &self.form, &self.db);
            self.checked_at = Some(Instant::now());
        }
        ui.ctx().request_repaint_after(CHECK_INTERVAL);
        egui::Grid::new("checks")
            .num_columns(3)
            .spacing([12.0, 4.0])
            .show(ui, |ui| {
                for check in &self.checks {
                    let (mark, color) = match check.status {
                        Status::Ok => ("OK", Color32::from_rgb(0x3c, 0xb3, 0x71)),
                        Status::Warn => ("!", Color32::from_rgb(0xe0, 0xa0, 0x30)),
                        Status::Missing => ("MISSING", Color32::from_rgb(0xd9, 0x4f, 0x4f)),
                    };
                    ui.colored_label(color, mark);
                    ui.label(check.label);
                    ui.label(RichText::new(&check.detail).weak());
                    ui.end_row();
                }
            });

        let ready = core::ready_to_start(&self.checks);
        let idle = self.starting.queue.is_empty()
            && self.starting.inflight.is_none()
            && !self.starting.waiting_for_connection;
        if ui
            .add_enabled(ready && idle, egui::Button::new("Start servers"))
            .on_hover_text("Starts the supervisor if needed, then the login server and the game server")
            .on_disabled_hover_text("Something is missing, or a start is in progress")
            .clicked()
        {
            self.starting.messages.clear();
            if cx.conn.is_connected() {
                self.starting.queue = START_ORDER.into();
            } else {
                self.starting.waiting_for_connection = true;
                cx.launch
                    .use_run_folder(layout.run_folder.to_string_lossy().into_owned());
            }
        }
        for (ok, text) in &self.starting.messages {
            let color = if *ok {
                Color32::from_rgb(0x3c, 0xb3, 0x71)
            } else {
                ui.visuals().error_fg_color
            };
            ui.colored_label(color, text);
        }
        if self.starting.waiting_for_connection {
            ui.label("Waiting for the supervisor...");
        }
    }

    /// Sends the next start request once the supervisor is connected and the last one answered.
    fn pump_start(&mut self, cx: &AppCtx<'_>) {
        let s = &mut self.starting;
        if s.waiting_for_connection && cx.conn.is_connected() {
            s.waiting_for_connection = false;
            s.queue = START_ORDER.into();
        } else if s.waiting_for_connection && matches!(cx.launch.state, LaunchState::Problem(_)) {
            s.waiting_for_connection = false;
            s.messages
                .push((false, launch::status_text(&cx.launch.state, false)));
        }
        if s.inflight.is_none()
            && let Some(name) = s.queue.pop_front()
        {
            let id = cx.bridge.request(Request::StartServer {
                name: name.to_owned(),
            });
            s.inflight = Some((id, name));
        }
    }

    fn question_window(&mut self, ctx: &egui::Context) {
        let mut answered = false;
        match &self.install.question {
            Some(Question::Exists { db_name, reply }) => {
                egui::Window::new("Database exists")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        ui.label(format!("Database '{db_name}' already exists."));
                        ui.label("Reset it? That deletes ALL data in it. \"Keep\" runs the scripts on top of it (they replace the tables they name).");
                        ui.horizontal(|ui| {
                            if ui
                                .button(RichText::new("Delete and recreate").color(Color32::LIGHT_RED))
                                .clicked()
                            {
                                let _ = reply.send(ExistsChoice::Reset);
                                answered = true;
                            }
                            if ui.button("Keep and install on it").clicked() {
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
                egui::Window::new("Script error")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        ui.label(format!("MySQL error: {error}"));
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
            self.install.question = None;
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::bridge::{Bridge, ConnState};
    use crate::settings::Settings;

    fn run_frames(view: &mut SetupView, frames: usize) {
        let mut settings = Settings::default();
        let bridge = Bridge::spawn(None).unwrap();
        let conn = ConnState::default();
        let launch = crate::launch::LaunchHandle::detached();
        let ctx = egui::Context::default();
        for _ in 0..frames {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000.0, 900.0),
                )),
                ..egui::RawInput::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                let mut cx = AppCtx {
                    settings: &mut settings,
                    conn: &conn,
                    bridge: &bridge,
                    launch: &launch,
                };
                view.ui(ui, &mut cx);
            });
            output.textures_delta.clear();
        }
    }

    #[test]
    fn draws_without_a_layout() {
        let mut view = SetupView::with_layout(None, PathBuf::new());
        run_frames(&mut view, 2);
    }

    #[test]
    fn draws_for_a_package_layout_and_makes_no_database_connection() {
        let dir = tempfile::tempdir().unwrap();
        let layout = Layout::package(dir.path(), &dir.path().join("bin"));
        let mut view = SetupView::with_layout(Some(layout), dir.path().join("bin"));
        run_frames(&mut view, 3);
        let state = view.state.as_ref().unwrap();
        assert_eq!(state.db, DbTest::NotTested);
        assert!(state.test_rx.is_none() && state.install.rx.is_none());
        // The check page exists and says the database is not tested.
        assert!(
            state
                .checks
                .iter()
                .any(|c| c.label == "Database" && c.status == Status::Warn)
        );
    }
}
