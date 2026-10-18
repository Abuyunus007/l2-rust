//! Logs view: a live tail of the servers' output, from the supervisor's `subscribe_logs`.
//!
//! Lines are kept per server (at most [`MAX_LINES_PER_SERVER`], oldest dropped first). The
//! filters and the pause only change what is drawn, never what is stored.
//!
//! Subscribing needs the bridge, which only [`View::ui`] gets, so the subscription starts the
//! first time the view is shown after each connection. From then on lines are collected in
//! [`View::on_event`] whether or not the view is visible. Before the first show, the
//! supervisor's own backlog covers the gap.

use std::collections::{BTreeSet, VecDeque};
use std::sync::Arc;

use egui::{Color32, RichText};
use l2_admin_api::{Event, LogLevel, LogLine, Request, Response};

use crate::app::AppCtx;
use crate::bridge::BridgeEvent;
use crate::views::View;

/// Lines kept per server. The oldest go first.
pub const MAX_LINES_PER_SERVER: usize = 20_000;

/// A stored line. `arrival` numbers all lines in the order they arrived, across servers, so the
/// merged view keeps the true order and a paused view can stop at a point in it.
#[derive(Clone, Debug)]
pub struct Entry {
    pub arrival: u64,
    pub line: Arc<LogLine>,
}

#[derive(Debug)]
struct ServerLog {
    name: String,
    lines: VecDeque<Entry>,
}

/// The lines of every server, newest at the back of each server's queue.
#[derive(Debug, Default)]
pub struct LogStore {
    servers: Vec<ServerLog>,
    next_arrival: u64,
}

impl LogStore {
    pub fn push(&mut self, line: LogLine) {
        let index = match self.servers.iter().position(|s| s.name == line.server) {
            Some(index) => index,
            None => {
                self.servers.push(ServerLog {
                    name: line.server.clone(),
                    lines: VecDeque::new(),
                });
                self.servers.len() - 1
            }
        };
        let server = &mut self.servers[index];
        server.lines.push_back(Entry {
            arrival: self.next_arrival,
            line: Arc::new(line),
        });
        self.next_arrival += 1;
        while server.lines.len() > MAX_LINES_PER_SERVER {
            server.lines.pop_front();
        }
    }

    /// Forgets every line. The arrival counter keeps counting, so an old pause point stays valid.
    pub fn clear(&mut self) {
        self.servers.clear();
    }

    /// Number of stored lines over all servers.
    pub fn count(&self) -> usize {
        self.servers.iter().map(|s| s.lines.len()).sum()
    }

    pub fn server_names(&self) -> impl Iterator<Item = &str> {
        self.servers.iter().map(|s| s.name.as_str())
    }

    /// The arrival number the next line gets. Every line numbered below it has arrived.
    pub fn next_arrival(&self) -> u64 {
        self.next_arrival
    }
}

/// What the view shows. Changing it never touches the stored lines.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Filter {
    /// `None` = all servers.
    pub server: Option<String>,
    pub error: bool,
    pub warn: bool,
    pub info: bool,
    pub debug: bool,
    /// Case-insensitive substring of the message or the target. Empty = no text filter.
    pub text: String,
}

impl Default for Filter {
    fn default() -> Self {
        Self {
            server: None,
            error: true,
            warn: true,
            info: true,
            debug: true,
            text: String::new(),
        }
    }
}

impl Filter {
    fn level_shown(&self, level: Option<LogLevel>) -> bool {
        match level {
            // Lines without a parsed level are never hidden by the level checkboxes.
            None => true,
            Some(LogLevel::Error) => self.error,
            Some(LogLevel::Warn) => self.warn,
            Some(LogLevel::Info) => self.info,
            // Trace has no checkbox of its own; it is shown together with debug.
            Some(LogLevel::Debug | LogLevel::Trace) => self.debug,
        }
    }

    /// `needle` is `self.text` lowercased. It is passed in so that it is lowercased once per
    /// frame, not once per line.
    fn accepts(&self, line: &LogLine, needle: &str) -> bool {
        if self.server.as_deref().is_some_and(|s| s != line.server.as_str()) {
            return false;
        }
        if !self.level_shown(line.level) {
            return false;
        }
        needle.is_empty()
            || line.message.to_lowercase().contains(needle)
            || line
                .target
                .as_deref()
                .is_some_and(|t| t.to_lowercase().contains(needle))
    }
}

/// The lines to draw, oldest first. `before` (a paused view) hides the lines that arrived at or
/// after that arrival number.
pub fn visible(store: &LogStore, filter: &Filter, before: Option<u64>) -> Vec<Entry> {
    let needle = filter.text.to_lowercase();
    let mut rows: Vec<Entry> = store
        .servers
        .iter()
        .filter(|s| filter.server.as_deref().is_none_or(|name| name == s.name))
        .flat_map(|s| s.lines.iter())
        .filter(|e| before.is_none_or(|b| e.arrival < b) && filter.accepts(&e.line, &needle))
        .cloned()
        .collect();
    rows.sort_unstable_by_key(|e| e.arrival);
    rows
}

/// The line as shown and copied: `HH:MM:SS.mmm [server] LEVEL target: message`.
pub fn format_line(line: &LogLine) -> String {
    let mut out = format!("{} [{}]", format_time(line.ts_ms), line.server);
    if let Some(level) = level_label(line.level) {
        out.push(' ');
        out.push_str(level);
    }
    if let Some(target) = &line.target {
        out.push(' ');
        out.push_str(target);
        out.push(':');
    }
    out.push(' ');
    out.push_str(&line.message);
    out
}

/// The selected rows as text, one line each, oldest first. Selected rows that the filter hides
/// are not in `rows`, so they are not copied.
pub fn selected_text(rows: &[Entry], selected: &BTreeSet<u64>) -> String {
    rows.iter()
        .filter(|e| selected.contains(&e.arrival))
        .map(|e| format_line(&e.line))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Time of day in UTC (the GUI has no time zone support; the status line says so).
fn format_time(ts_ms: i64) -> String {
    let ms_of_day = ts_ms.rem_euclid(86_400_000);
    let h = ms_of_day / 3_600_000;
    let m = ms_of_day / 60_000 % 60;
    let s = ms_of_day / 1_000 % 60;
    let ms = ms_of_day % 1_000;
    format!("{h:02}:{m:02}:{s:02}.{ms:03}")
}

fn level_label(level: Option<LogLevel>) -> Option<&'static str> {
    match level {
        None => None,
        Some(LogLevel::Error) => Some("ERROR"),
        Some(LogLevel::Warn) => Some("WARN"),
        Some(LogLevel::Info) => Some("INFO"),
        Some(LogLevel::Debug) => Some("DEBUG"),
        Some(LogLevel::Trace) => Some("TRACE"),
    }
}

/// Errors red, warnings yellow (dark amber on a light background, so it stays readable).
fn level_color(level: Option<LogLevel>, dark: bool) -> Option<Color32> {
    match level {
        Some(LogLevel::Error) => Some(Color32::from_rgb(0xe5, 0x4d, 0x4d)),
        Some(LogLevel::Warn) if dark => Some(Color32::from_rgb(0xf2, 0xc9, 0x4c)),
        Some(LogLevel::Warn) => Some(Color32::from_rgb(0x9a, 0x6a, 0x00)),
        _ => None,
    }
}

/// Plain click: select only this row (clicking the only selected row clears it).
/// Ctrl-click: add or remove this row.
fn toggle_selection(selected: &mut BTreeSet<u64>, arrival: u64, additive: bool) {
    if additive {
        if !selected.remove(&arrival) {
            selected.insert(arrival);
        }
    } else if selected.len() == 1 && selected.contains(&arrival) {
        selected.clear();
    } else {
        selected.clear();
        selected.insert(arrival);
    }
}

#[derive(Default)]
pub struct LogsView {
    store: LogStore,
    /// Names from the last server list. The selector offers them before they have logged.
    server_names: Vec<String>,
    filter: Filter,
    /// `Some(n)` while paused: only lines numbered below `n` are drawn.
    paused_at: Option<u64>,
    /// The rows on screen, rebuilt when `dirty` is set.
    rows: Vec<Entry>,
    dirty: bool,
    /// Arrival numbers of the selected rows.
    selected: BTreeSet<u64>,
    /// The subscription is active on the current connection.
    subscribed: bool,
    subscribe_id: Option<u64>,
    subscribe_error: Option<String>,
}

impl LogsView {
    /// Server names for the selector: the server list first, then any others seen in the logs.
    fn server_options(&self) -> Vec<String> {
        let mut names = self.server_names.clone();
        for name in self.store.server_names() {
            if !names.iter().any(|n| n == name) {
                names.push(name.to_string());
            }
        }
        names
    }

    fn controls(&mut self, ui: &mut egui::Ui) {
        let before = self.filter.clone();
        let options = self.server_options();
        let current = self
            .filter
            .server
            .clone()
            .unwrap_or_else(|| "All servers".to_string());

        ui.horizontal_wrapped(|ui| {
            ui.label("Server");
            egui::ComboBox::from_id_salt("logs_server")
                .selected_text(current)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.filter.server, None, "All servers");
                    for name in &options {
                        ui.selectable_value(&mut self.filter.server, Some(name.clone()), name.as_str());
                    }
                });

            ui.checkbox(&mut self.filter.error, "Error");
            ui.checkbox(&mut self.filter.warn, "Warn");
            ui.checkbox(&mut self.filter.info, "Info");
            ui.checkbox(&mut self.filter.debug, "Debug");

            ui.add(
                egui::TextEdit::singleline(&mut self.filter.text)
                    .hint_text("Filter text")
                    .desired_width(180.0),
            );

            let pause_label = if self.paused_at.is_some() {
                "Resume"
            } else {
                "Pause"
            };
            if ui.button(pause_label).clicked() {
                self.paused_at = match self.paused_at {
                    Some(_) => None,
                    None => Some(self.store.next_arrival()),
                };
                self.dirty = true;
            }

            let n = self.selected.len();
            if ui
                .add_enabled(n > 0, egui::Button::new(format!("Copy selected ({n})")))
                .clicked()
            {
                ui.ctx().copy_text(selected_text(&self.rows, &self.selected));
            }
        });

        if self.filter != before {
            self.dirty = true;
        }
    }

    fn status(&self, ui: &mut egui::Ui, connected: bool) {
        ui.horizontal(|ui| {
            if connected {
                let paused = if self.paused_at.is_some() { " (paused)" } else { "" };
                ui.label(format!(
                    "{} of {} lines, times in UTC{paused}",
                    self.rows.len(),
                    self.store.count()
                ));
            } else {
                ui.label("Not connected: log lines appear once connected.");
            }
            if let Some(error) = &self.subscribe_error {
                ui.colored_label(
                    ui.visuals().error_fg_color,
                    format!("Subscribing failed: {error}"),
                );
            }
        });
    }

    fn list(&mut self, ui: &mut egui::Ui) {
        let dark = ui.visuals().dark_mode;
        let row_height = ui.spacing().interact_size.y;
        let rows = &self.rows;
        let selected = &mut self.selected;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            // Follows the newest line until the user scrolls up; following resumes when they
            // scroll back to the bottom.
            .stick_to_bottom(true)
            .show_rows(ui, row_height, rows.len(), |ui, range| {
                for entry in &rows[range] {
                    let is_selected = selected.contains(&entry.arrival);
                    let text = RichText::new(format_line(&entry.line)).monospace();
                    let text = match level_color(entry.line.level, dark) {
                        Some(color) => text.color(color),
                        None => text,
                    };
                    let response = ui.add_sized(
                        [ui.available_width(), row_height],
                        egui::Button::selectable(is_selected, text).truncate(),
                    );
                    if response.clicked() {
                        let additive = ui.input(|i| i.modifiers.command);
                        toggle_selection(selected, entry.arrival, additive);
                    }
                }
            });
    }
}

impl View for LogsView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut AppCtx<'_>) {
        if cx.conn.is_connected() && !self.subscribed {
            self.subscribe_id = Some(cx.bridge.request(Request::SubscribeLogs {
                server: None,
                since: None,
            }));
            self.subscribed = true;
        }

        self.controls(ui);
        if self.dirty {
            self.rows = visible(&self.store, &self.filter, self.paused_at);
            self.dirty = false;
        }
        self.status(ui, cx.conn.is_connected());
        ui.add_space(4.0);
        self.list(ui);
    }

    fn on_event(&mut self, event: &BridgeEvent) {
        match event {
            BridgeEvent::Connected { .. } => {
                // The supervisor sends its backlog again with the new subscription, so start
                // from an empty store. A pause does not survive a reconnect.
                self.store.clear();
                self.paused_at = None;
                self.subscribed = false;
                self.subscribe_id = None;
                self.subscribe_error = None;
                self.dirty = true;
            }
            BridgeEvent::Connecting { .. }
            | BridgeEvent::Waiting { .. }
            | BridgeEvent::Rejected { .. }
            | BridgeEvent::Disconnected => self.subscribed = false,
            BridgeEvent::Event(Event::LogLine { line }) => {
                self.store.push(line.clone());
                self.dirty = true;
            }
            BridgeEvent::Response { id, result } if Some(*id) == self.subscribe_id => {
                self.subscribe_error = match result {
                    Ok(Response::Ok) => None,
                    Ok(Response::Error { message }) => Some(message.clone()),
                    Ok(other) => Some(format!("unexpected answer: {other:?}")),
                    Err(error) => Some(error.clone()),
                };
            }
            BridgeEvent::Response {
                result: Ok(Response::Servers { servers }),
                ..
            } => {
                self.server_names = servers.iter().map(|s| s.name.clone()).collect();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(server: &str, level: Option<LogLevel>, message: &str) -> LogLine {
        LogLine {
            server: server.into(),
            seq: 0,
            ts_ms: 1_700_000_000_000,
            level,
            target: None,
            message: message.into(),
        }
    }

    fn messages(rows: &[Entry]) -> Vec<&str> {
        rows.iter().map(|e| e.line.message.as_str()).collect()
    }

    #[test]
    fn keeps_at_most_the_cap_per_server_and_drops_the_oldest() {
        let mut store = LogStore::default();
        store.push(line("game", None, "game first"));
        for n in 0..=MAX_LINES_PER_SERVER {
            store.push(line("login", None, &format!("login {n}")));
        }
        let rows = visible(&store, &Filter::default(), None);
        // The game server's line is kept: the cap is per server.
        assert_eq!(rows.len(), MAX_LINES_PER_SERVER + 1);
        assert_eq!(store.count(), MAX_LINES_PER_SERVER + 1);
        assert_eq!(rows[0].line.message, "game first");
        assert_eq!(rows[1].line.message, "login 1", "login 0 was the oldest");
        assert_eq!(
            rows.last().unwrap().line.message,
            format!("login {MAX_LINES_PER_SERVER}")
        );
    }

    #[test]
    fn level_checkboxes_hide_their_level_only() {
        let mut store = LogStore::default();
        store.push(line("login", Some(LogLevel::Error), "e"));
        store.push(line("login", Some(LogLevel::Warn), "w"));
        store.push(line("login", Some(LogLevel::Info), "i"));
        store.push(line("login", Some(LogLevel::Debug), "d"));
        store.push(line("login", Some(LogLevel::Trace), "t"));
        store.push(line("login", None, "unparsed"));

        let filter = Filter {
            debug: false,
            ..Filter::default()
        };
        // Trace follows the debug checkbox; unparsed lines are never hidden by levels.
        assert_eq!(
            messages(&visible(&store, &filter, None)),
            ["e", "w", "i", "unparsed"]
        );

        let filter = Filter {
            error: false,
            info: false,
            ..Filter::default()
        };
        assert_eq!(
            messages(&visible(&store, &filter, None)),
            ["w", "d", "t", "unparsed"]
        );
    }

    #[test]
    fn text_filter_is_case_insensitive_and_checks_the_target() {
        let mut store = LogStore::default();
        store.push(line("login", Some(LogLevel::Info), "Loaded 0 IP Bans"));
        let mut with_target = line("game", Some(LogLevel::Info), "player entered");
        with_target.target = Some("GameClient".into());
        store.push(with_target);
        store.push(line("game", Some(LogLevel::Info), "unrelated"));

        let filter = |text: &str| Filter {
            text: text.into(),
            ..Filter::default()
        };
        assert_eq!(
            messages(&visible(&store, &filter("ip bans"), None)),
            ["Loaded 0 IP Bans"]
        );
        assert_eq!(
            messages(&visible(&store, &filter("GAMECLIENT"), None)),
            ["player entered"]
        );
        assert_eq!(visible(&store, &filter(""), None).len(), 3);
        assert!(visible(&store, &filter("nothing matches"), None).is_empty());
    }

    #[test]
    fn server_selector_limits_to_one_server() {
        let mut store = LogStore::default();
        store.push(line("login", None, "a"));
        store.push(line("game", None, "b"));
        store.push(line("login", None, "c"));
        let filter = Filter {
            server: Some("login".into()),
            ..Filter::default()
        };
        assert_eq!(messages(&visible(&store, &filter, None)), ["a", "c"]);
    }

    #[test]
    fn merged_view_keeps_arrival_order_across_servers() {
        let mut store = LogStore::default();
        store.push(line("login", None, "1"));
        store.push(line("game", None, "2"));
        store.push(line("login", None, "3"));
        assert_eq!(
            messages(&visible(&store, &Filter::default(), None)),
            ["1", "2", "3"]
        );
    }

    #[test]
    fn pause_point_hides_lines_that_arrived_later() {
        let mut store = LogStore::default();
        store.push(line("login", None, "before"));
        let paused_at = store.next_arrival();
        store.push(line("login", None, "after"));
        assert_eq!(
            messages(&visible(&store, &Filter::default(), Some(paused_at))),
            ["before"]
        );
        assert_eq!(visible(&store, &Filter::default(), None).len(), 2);
    }

    #[test]
    fn clear_keeps_the_arrival_counter() {
        let mut store = LogStore::default();
        store.push(line("login", None, "old"));
        let before = store.next_arrival();
        store.clear();
        assert_eq!(store.count(), 0);
        store.push(line("login", None, "new"));
        assert_eq!(store.next_arrival(), before + 1);
    }

    #[test]
    fn formats_time_of_day_in_utc() {
        // 1_700_000_000 s = 2023-11-14 22:13:20 UTC.
        assert_eq!(format_time(1_700_000_000_000), "22:13:20.000");
        assert_eq!(format_time(1_700_000_000_123), "22:13:20.123");
        assert_eq!(format_time(-1), "23:59:59.999");
    }

    #[test]
    fn formats_a_line_for_display_and_copy() {
        let mut l = line("login", Some(LogLevel::Warn), "low memory");
        assert_eq!(format_line(&l), "22:13:20.000 [login] WARN low memory");
        l.target = Some("Shutdown".into());
        l.level = None;
        assert_eq!(format_line(&l), "22:13:20.000 [login] Shutdown: low memory");
    }

    #[test]
    fn copy_takes_only_selected_visible_rows_in_order() {
        let mut store = LogStore::default();
        store.push(line("login", None, "a"));
        store.push(line("login", None, "b"));
        store.push(line("login", None, "c"));
        let rows = visible(&store, &Filter::default(), None);
        let selected: BTreeSet<u64> = [2, 0].into_iter().collect();
        assert_eq!(
            selected_text(&rows, &selected),
            "22:13:20.000 [login] a\n22:13:20.000 [login] c"
        );
    }

    #[test]
    fn selection_click_rules() {
        let mut selected = BTreeSet::new();
        toggle_selection(&mut selected, 1, false);
        assert_eq!(selected, BTreeSet::from([1]));
        toggle_selection(&mut selected, 2, false);
        assert_eq!(selected, BTreeSet::from([2]), "plain click replaces");
        toggle_selection(&mut selected, 2, false);
        assert!(selected.is_empty(), "plain click on the only selected row clears");
        toggle_selection(&mut selected, 3, true);
        toggle_selection(&mut selected, 4, true);
        assert_eq!(selected, BTreeSet::from([3, 4]), "ctrl-click adds");
        toggle_selection(&mut selected, 3, true);
        assert_eq!(selected, BTreeSet::from([4]), "ctrl-click removes");
    }

    #[test]
    fn server_options_list_configured_names_first() {
        let mut view = LogsView {
            server_names: vec!["login".into(), "game".into()],
            ..LogsView::default()
        };
        view.store.push(line("game", None, "x"));
        view.store.push(line("extra", None, "y"));
        assert_eq!(view.server_options(), ["login", "game", "extra"]);
    }
}
