//! Backups view: "Backup now", the backup files, how the last one ended and when the next one is
//! due. The supervisor makes the backups (`l2-supervisor/src/backup.rs`, settings in
//! `Backup.ini`); this view only asks for them (M9-H01).
//!
//! "Restore" never restores anything: it opens a dialog with the command to run by hand, because
//! a restore replaces the whole database and must not be one click away.

use std::time::{Duration, Instant};

use l2_admin_api::{BackupDatabase, BackupInfo, BackupStatus, Request, Response};

use crate::app::AppCtx;
use crate::bridge::BridgeEvent;
use crate::views::View;
use crate::views::players::format_utc;

/// How often the state and the list are asked for again.
const REFRESH: Duration = Duration::from_secs(10);
/// While a backup runs, ask more often so the result shows up quickly.
const REFRESH_RUNNING: Duration = Duration::from_secs(1);

/// The command lines of the restore dialog. Pure, so the text is tested without egui.
///
/// `-p` without a value makes `mysql` ask for the password: it never goes into a command line.
/// The dump has no `CREATE DATABASE` (it is a dump of one database), so the database must exist.
pub fn restore_commands(folder: &str, file: &str, db: &BackupDatabase) -> (String, String) {
    let path = format!("{}/{file}", folder.trim_end_matches(['/', '\\']));
    let mysql = format!(
        "mysql --host={} --port={} --user={} -p {}",
        db.host, db.port, db.user, db.database
    );
    let unix = format!("gunzip -c \"{path}\" | {mysql}");
    let sql = path.strip_suffix(".gz").unwrap_or(&path);
    let windows = format!("{mysql} < \"{}\"", sql.replace('/', "\\"));
    (unix, windows)
}

/// `1.5 MB`, `830 KB`, `12 B`.
pub fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[derive(Default)]
pub struct BackupsView {
    status: Option<BackupStatus>,
    backups: Vec<BackupInfo>,
    /// `BackupState` / `ListBackups` / `BackupNow` requests in flight.
    state_request: Option<u64>,
    list_request: Option<u64>,
    now_request: Option<u64>,
    /// No new refresh before this instant. `None` = due now.
    next_at: Option<Instant>,
    error: Option<String>,
    /// The file the restore dialog is open for.
    restore: Option<String>,
}

impl BackupsView {
    fn reset(&mut self) {
        *self = Self {
            restore: self.restore.take(),
            ..Self::default()
        };
    }

    fn refresh_due(&self, connected: bool, now: Instant) -> bool {
        connected
            && self.state_request.is_none()
            && self.list_request.is_none()
            && self.next_at.is_none_or(|at| now >= at)
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
                if self.state_request == Some(*id) {
                    self.state_request = None;
                    match result {
                        Ok(Response::BackupState { state }) => {
                            self.status = Some(BackupStatus::clone(state));
                            self.error = None;
                        }
                        Ok(Response::Error { message }) | Err(message) => self.error = Some(message.clone()),
                        Ok(other) => self.error = Some(format!("unexpected answer: {other:?}")),
                    }
                    // After the match: the pace depends on whether a backup runs now.
                    self.schedule(now);
                } else if self.list_request == Some(*id) {
                    self.list_request = None;
                    if let Ok(Response::Backups { backups }) = result {
                        self.backups.clone_from(backups);
                    }
                } else if self.now_request == Some(*id) {
                    self.now_request = None;
                    match result {
                        // Look at the state at once: it now says "running".
                        Ok(Response::Ok) => self.next_at = None,
                        Ok(Response::Error { message }) | Err(message) => self.error = Some(message.clone()),
                        Ok(other) => self.error = Some(format!("unexpected answer: {other:?}")),
                    }
                }
            }
        }
    }

    fn schedule(&mut self, now: Instant) {
        let running = self.status.as_ref().is_some_and(|s| s.running);
        self.next_at = Some(now + if running { REFRESH_RUNNING } else { REFRESH });
    }

    fn restore_dialog(&mut self, ctx: &egui::Context) {
        let Some(file) = self.restore.clone() else { return };
        let mut open = true;
        egui::Window::new("Restore a backup")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .show(ctx, |ui| {
                ui.label(format!("File: {file}"));
                ui.add_space(4.0);
                ui.colored_label(
                    ui.visuals().warn_fg_color,
                    "A restore replaces the data in the database. Stop the login and game server first. \
                     Nothing is restored from here: run the command yourself.",
                );
                ui.add_space(8.0);
                match self.status.as_ref().and_then(|s| s.database.as_ref().map(|d| (s, d))) {
                    Some((status, db)) => {
                        let (unix, windows) = restore_commands(&status.folder, &file, db);
                        ui.label("Linux, macOS, Git Bash (asks for the database password):");
                        ui.add(egui::TextEdit::multiline(&mut unix.as_str()).desired_rows(2).desired_width(f32::INFINITY));
                        ui.add_space(4.0);
                        ui.label("Windows: unpack the .gz first (7-Zip), then (asks for the password):");
                        ui.add(
                            egui::TextEdit::multiline(&mut windows.as_str())
                                .desired_rows(2)
                                .desired_width(f32::INFINITY),
                        );
                        ui.add_space(4.0);
                        ui.label(format!("The database `{}` must exist (it is empty after a reset).", db.database));
                    }
                    None => {
                        ui.label("The database settings could not be read (Database.ini), so there is no command to show.");
                    }
                }
            });
        if !open {
            self.restore = None;
        }
    }
}

impl View for BackupsView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut AppCtx<'_>) {
        let connected = cx.conn.is_connected();
        let now = Instant::now();
        if self.refresh_due(connected, now) {
            self.state_request = Some(cx.bridge.request(Request::BackupState));
            self.list_request = Some(cx.bridge.request(Request::ListBackups));
        } else if let Some(at) = self.next_at.filter(|at| *at > now) {
            ui.ctx().request_repaint_after(at - now);
        }

        ui.heading("Backups");
        ui.add_space(8.0);
        if !connected {
            ui.colored_label(ui.visuals().warn_fg_color, "Not connected to the supervisor.");
        }
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }

        let running = self.status.as_ref().is_some_and(|s| s.running);
        ui.horizontal(|ui| {
            let can = connected && !running && self.now_request.is_none();
            if ui
                .add_enabled(can, egui::Button::new("Backup now"))
                .on_disabled_hover_text(if running {
                    "A backup is running"
                } else {
                    "Not connected to the supervisor"
                })
                .clicked()
            {
                self.error = None;
                self.now_request = Some(cx.bridge.request(Request::BackupNow));
            }
            if running {
                ui.spinner();
                ui.label("Backup running...");
            }
        });
        ui.add_space(8.0);

        if let Some(status) = &self.status {
            egui::Grid::new("backup_state").num_columns(2).show(ui, |ui| {
                ui.label("Last backup:");
                match &status.last {
                    Some(last) if last.ok => {
                        ui.label(format!("{} ({})", last.message, format_utc(last.finished_ms)))
                    }
                    Some(last) => ui.colored_label(
                        ui.visuals().error_fg_color,
                        format!("failed {}: {}", format_utc(last.finished_ms), last.message),
                    ),
                    None => ui.label("none since the supervisor started"),
                };
                ui.end_row();
                ui.label("Next backup:");
                match (status.enabled, status.next_run_ms) {
                    (true, Some(ms)) => ui.label(format_utc(ms)),
                    (true, None) => ui.label("being planned"),
                    (false, _) => ui.label("schedule off (Enabled = False in Backup.ini)"),
                };
                ui.end_row();
                ui.label("Schedule:");
                ui.label(format!(
                    "every {} h, keeping the newest {}",
                    status.interval_hours, status.keep
                ));
                ui.end_row();
                ui.label("Folder:");
                ui.label(&status.folder);
                ui.end_row();
            });
            ui.add_space(8.0);
        }

        if self.backups.is_empty() {
            ui.label("No backup files yet.");
        } else {
            let mut restore = None;
            egui::ScrollArea::vertical().show(ui, |ui| {
                egui::Grid::new("backup_files")
                    .striped(true)
                    .spacing([16.0, 8.0])
                    .show(ui, |ui| {
                        for header in ["File", "Size", "Time", ""] {
                            ui.strong(header);
                        }
                        ui.end_row();
                        for backup in &self.backups {
                            ui.label(&backup.name);
                            ui.label(format_size(backup.size));
                            ui.label(format_utc(backup.modified_ms));
                            if ui.button("Restore...").clicked() {
                                restore = Some(backup.name.clone());
                            }
                            ui.end_row();
                        }
                    });
            });
            if restore.is_some() {
                self.restore = restore;
            }
        }
        self.restore_dialog(ui.ctx());
    }

    fn on_event(&mut self, event: &BridgeEvent) {
        self.apply(event, Instant::now());
    }
}

#[cfg(test)]
mod tests {
    use l2_admin_api::BackupResult;

    use super::*;

    fn db() -> BackupDatabase {
        BackupDatabase {
            host: "localhost".into(),
            port: 3306,
            user: "root".into(),
            database: "l2jmobiush5".into(),
        }
    }

    fn status(running: bool) -> BackupStatus {
        BackupStatus {
            enabled: true,
            running,
            interval_hours: 24,
            keep: 7,
            folder: "C:\\l2\\backups".into(),
            database: Some(db()),
            last: Some(BackupResult {
                ok: true,
                message: "x.sql.gz".into(),
                finished_ms: 0,
            }),
            next_run_ms: Some(1),
        }
    }

    fn answer(id: u64, response: Response) -> BridgeEvent {
        BridgeEvent::Response {
            id,
            result: Ok(response),
        }
    }

    #[test]
    fn restore_commands_never_hold_a_password() {
        let (unix, windows) =
            restore_commands("C:\\l2\\backups\\", "l2jmobiush5-20261102-030000.sql.gz", &db());
        assert_eq!(
            unix,
            "gunzip -c \"C:\\l2\\backups/l2jmobiush5-20261102-030000.sql.gz\" | mysql --host=localhost --port=3306 --user=root -p l2jmobiush5"
        );
        assert_eq!(
            windows,
            "mysql --host=localhost --port=3306 --user=root -p l2jmobiush5 < \"C:\\l2\\backups\\l2jmobiush5-20261102-030000.sql\""
        );
    }

    #[test]
    fn sizes_are_readable() {
        assert_eq!(format_size(12), "12 B");
        assert_eq!(format_size(1536), "1.5 KB");
        assert_eq!(format_size(5 * 1024 * 1024), "5.0 MB");
    }

    #[test]
    fn answers_fill_the_view_and_pace_the_refresh() {
        let now = Instant::now();
        let mut view = BackupsView::default();
        assert!(view.refresh_due(true, now));
        assert!(!view.refresh_due(false, now));
        view.state_request = Some(1);
        view.list_request = Some(2);
        assert!(!view.refresh_due(true, now));

        view.apply(
            &answer(
                1,
                Response::BackupState {
                    state: Box::new(status(true)),
                },
            ),
            now,
        );
        view.apply(
            &answer(
                2,
                Response::Backups {
                    backups: vec![BackupInfo {
                        name: "a.sql.gz".into(),
                        size: 1,
                        modified_ms: 2,
                    }],
                },
            ),
            now,
        );
        assert!(view.status.as_ref().unwrap().running);
        assert_eq!(view.backups.len(), 1);
        // A running backup is watched closely.
        assert!(view.refresh_due(true, now + REFRESH_RUNNING));
        assert!(!view.refresh_due(true, now));

        view.now_request = Some(3);
        view.apply(&answer(3, Response::Ok), now);
        assert_eq!(view.next_at, None);
    }

    #[test]
    fn errors_show_and_a_new_connection_starts_fresh() {
        let now = Instant::now();
        let mut view = BackupsView {
            now_request: Some(7),
            ..BackupsView::default()
        };
        view.apply(
            &answer(
                7,
                Response::Error {
                    message: "a backup is already running".into(),
                },
            ),
            now,
        );
        assert_eq!(view.error.as_deref(), Some("a backup is already running"));
        view.status = Some(status(false));
        view.restore = Some("a.sql.gz".into());
        view.apply(
            &BridgeEvent::Connected {
                name: "supervisor".into(),
            },
            now,
        );
        assert!(view.status.is_none() && view.error.is_none());
        // The open dialog survives a reconnect.
        assert_eq!(view.restore.as_deref(), Some("a.sql.gz"));
    }
}
