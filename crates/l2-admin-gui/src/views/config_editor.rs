//! Config editor: the `.ini` files of the run folder (`run/login/config`, `run/game/config`),
//! one editor per key, with the comment lines as help, the Java default and a reset button
//! (card G-05).
//!
//! Java: none (tooling). Keys, comments, defaults and the in-place writer are `l2_config`
//! (`ini_edit`, `defaults`); only the changed lines are rewritten, so comments, order and line
//! endings stay. [`FileState`] holds the rules and has no egui in it, so it is tested directly.
//! The view only draws it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use l2_config::defaults::{Side, default_for, java_defaults};
use l2_config::ini_edit::{
    GEO_OVERRIDE_TEMPLATE, IniDoc, IniEntry, apply_changes, is_secret, mask_value, write_atomic,
};

use crate::app::AppCtx;
use crate::views::View;

/// Folder of the servers' own copy of `dist/` (D-55), relative to the working directory.
pub const DEFAULT_RUN_DIR: &str = "run";
/// The Rust-only file that is not in `dist/`; offered as a template when it does not exist.
const GEO_OVERRIDE_FILE: &str = "GeoOverride.ini";

/// How the value of a key is edited.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueKind {
    Bool,
    Int,
    Float,
    Text,
}

fn is_bool_text(s: &str) -> bool {
    s.eq_ignore_ascii_case("true") || s.eq_ignore_ascii_case("false")
}

/// `Double.parseDouble`-like for the editor: plain numbers, optionally with a Java type suffix.
fn parse_float(s: &str) -> Option<f64> {
    let s = s.trim();
    let first = s.chars().next()?;
    if !(first.is_ascii_digit() || matches!(first, '+' | '-' | '.')) {
        return None; // Rust would accept "inf" and "nan" as numbers
    }
    let s = s.strip_suffix(['f', 'F', 'd', 'D']).unwrap_or(s);
    s.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// The editor for a value, from what the file has and the Java default.
pub fn detect_kind(value: &str, default: Option<&str>) -> ValueKind {
    let v = value.trim();
    let d = default.map(str::trim);
    if is_bool_text(v) || (v.is_empty() && d.is_some_and(is_bool_text)) {
        return ValueKind::Bool;
    }
    let default_is_float = d.is_some_and(|d| d.parse::<i64>().is_err() && parse_float(d).is_some());
    if v.parse::<i64>().is_ok() {
        return if default_is_float {
            ValueKind::Float
        } else {
            ValueKind::Int
        };
    }
    if parse_float(v).is_some() {
        return ValueKind::Float;
    }
    if v.is_empty() && d.is_some_and(|d| d.parse::<i64>().is_ok()) {
        return ValueKind::Int;
    }
    ValueKind::Text
}

/// Checks `text` for `kind`. `wide` allows the whole `i64` range for integers; otherwise they
/// must fit Java's `int` (what almost every key is read with).
pub fn validate(kind: ValueKind, text: &str, wide: bool) -> Result<(), String> {
    if text.contains(['\n', '\r']) {
        return Err("must be one line".into());
    }
    if text.starts_with([' ', '\t']) {
        return Err("must not start with a blank".into());
    }
    if text.chars().rev().take_while(|&c| c == '\\').count() % 2 == 1 {
        return Err("must not end with a single backslash".into());
    }
    match kind {
        ValueKind::Bool if !is_bool_text(text) => Err("must be True or False".into()),
        ValueKind::Int => match text.parse::<i64>() {
            Ok(v) if wide || i32::try_from(v).is_ok() => Ok(()),
            Ok(_) => Err(format!("out of range ({} to {})", i32::MIN, i32::MAX)),
            Err(_) => Err("must be a whole number".into()),
        },
        ValueKind::Float if parse_float(text).is_none() => Err("must be a number".into()),
        _ => Ok(()),
    }
}

/// Whether two values mean the same: booleans ignoring case, numbers by value (`1` = `1.0`).
pub fn values_equal(a: &str, b: &str) -> bool {
    let (a, b) = (a.trim(), b.trim());
    if a == b || (is_bool_text(a) && a.eq_ignore_ascii_case(b)) {
        return true;
    }
    matches!((parse_float(a), parse_float(b)), (Some(x), Some(y)) if x == y)
}

/// One `.ini` file, loaded for editing.
pub struct FileState {
    pub side: Side,
    /// Path below the config folder, `/` separated (`Custom/Banking.ini`).
    pub rel: String,
    pub path: PathBuf,
    /// The file does not exist yet; saving creates it from the template.
    pub is_new: bool,
    text: String,
    /// One per key, the last occurrence of a repeated key (the one the server uses).
    pub entries: Vec<IniEntry>,
    defaults: BTreeMap<String, String>,
    /// The text in the editors, by key.
    pub buffers: BTreeMap<String, String>,
}

impl FileState {
    /// Reads the file. A missing `GeoOverride.ini` becomes the template; any other missing file
    /// is an error.
    pub fn load(side: Side, rel: &str, path: &Path) -> Result<Self, String> {
        match std::fs::read(path) {
            Ok(bytes) => {
                let text = String::from_utf8(bytes).map_err(|_| format!("{rel}: not valid UTF-8"))?;
                Ok(Self::from_text(side, rel, path, text, false))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && rel == GEO_OVERRIDE_FILE => Ok(
                Self::from_text(side, rel, path, GEO_OVERRIDE_TEMPLATE.to_owned(), true),
            ),
            Err(e) => Err(format!("{rel}: {e}")),
        }
    }

    pub fn from_text(side: Side, rel: &str, path: &Path, text: String, is_new: bool) -> Self {
        let mut entries: Vec<IniEntry> = Vec::new();
        for entry in IniDoc::parse(&text).entries() {
            // Java keeps the last of a repeated key; show only that one.
            entries.retain(|e| e.key != entry.key);
            entries.push(entry);
        }
        let buffers = entries.iter().map(|e| (e.key.clone(), e.value.clone())).collect();
        Self {
            side,
            rel: rel.to_owned(),
            path: path.to_path_buf(),
            is_new,
            text,
            entries,
            defaults: java_defaults(side, rel),
            buffers,
        }
    }

    /// The file name without folders; the secret rules go by it.
    pub fn file_name(&self) -> &str {
        self.rel.rsplit('/').next().unwrap_or(&self.rel)
    }

    pub fn is_secret(&self, key: &str) -> bool {
        is_secret(self.file_name(), key)
    }

    /// The Java default of `key`, from the typed config structs or the file's own comment.
    pub fn default_of(&self, entry: &IniEntry) -> Option<String> {
        default_for(&self.defaults, &entry.key, entry.comment_default.as_deref())
    }

    pub fn kind_of(&self, entry: &IniEntry) -> ValueKind {
        detect_kind(&entry.value, self.default_of(entry).as_deref())
    }

    /// Integers may use the whole `i64` when the file already holds such a value.
    fn wide(&self, entry: &IniEntry) -> bool {
        let big = |s: &str| s.trim().parse::<i64>().is_ok_and(|v| i32::try_from(v).is_err());
        big(&entry.value) || self.default_of(entry).is_some_and(|d| big(&d))
    }

    pub fn is_dirty(&self, entry: &IniEntry) -> bool {
        self.buffers.get(&entry.key).is_some_and(|b| *b != entry.value)
    }

    pub fn differs_from_default(&self, entry: &IniEntry) -> bool {
        match (self.default_of(entry), self.buffers.get(&entry.key)) {
            (Some(d), Some(b)) => !values_equal(b, &d),
            _ => false,
        }
    }

    /// Why the edited text of `entry` cannot be saved, if it cannot.
    pub fn problem(&self, entry: &IniEntry) -> Option<String> {
        if !self.is_dirty(entry) {
            return None;
        }
        let text = self.buffers.get(&entry.key)?;
        validate(self.kind_of(entry), text, self.wide(entry)).err()
    }

    /// Puts the Java default into the editor of `entry`.
    pub fn reset_to_default(&mut self, entry: &IniEntry) {
        if let Some(d) = self.default_of(entry) {
            self.buffers.insert(entry.key.clone(), d.trim().to_owned());
        }
    }

    pub fn discard(&mut self) {
        self.buffers = self
            .entries
            .iter()
            .map(|e| (e.key.clone(), e.value.clone()))
            .collect();
    }

    /// (key, new text) of every edited key. A secret is only in here when it was changed.
    pub fn changes(&self) -> Vec<(String, String)> {
        self.entries
            .iter()
            .filter(|e| self.is_dirty(e))
            .filter_map(|e| Some((e.key.clone(), self.buffers.get(&e.key)?.clone())))
            .collect()
    }

    pub fn problems(&self) -> Vec<String> {
        self.entries
            .iter()
            .filter_map(|e| self.problem(e).map(|p| format!("{}: {} {p}", self.rel, e.key)))
            .collect()
    }

    /// Writes the changed values in place. Returns how many keys changed. A new file is written
    /// from the template with the changes applied.
    pub fn save(&self) -> Result<usize, String> {
        let changes = self.changes();
        if changes.is_empty() && !self.is_new {
            return Ok(0);
        }
        let (text, count) = apply_changes(&self.text, &changes).map_err(|e| format!("{}: {e}", self.rel))?;
        if self.is_new {
            write_atomic(&self.path, &text).map_err(|e| format!("{}: {e}", self.rel))?;
            return Ok(count);
        }
        if count > 0 {
            // Re-read right before writing: the file may have been edited by hand meanwhile, and
            // only our lines must change.
            let current = std::fs::read(&self.path).map_err(|e| format!("{}: {e}", self.rel))?;
            let current = String::from_utf8(current).map_err(|_| format!("{}: not valid UTF-8", self.rel))?;
            let (text, count) =
                apply_changes(&current, &changes).map_err(|e| format!("{}: {e}", self.rel))?;
            write_atomic(&self.path, &text).map_err(|e| format!("{}: {e}", self.rel))?;
            return Ok(count);
        }
        Ok(0)
    }

    /// Does `query` (lower case) match a key, a comment line or the group of `entry`?
    pub fn matches(&self, entry: &IniEntry, query: &str) -> bool {
        query.is_empty()
            || entry.key.to_lowercase().contains(query)
            || entry.group.to_lowercase().contains(query)
            || entry.help.iter().any(|h| h.to_lowercase().contains(query))
    }
}

/// The `.ini` files of a run folder: `login/config/*.ini` and `game/config/**.ini` (one level
/// of subfolders, `Custom/`), sorted. `GeoOverride.ini` is always in the game list.
pub fn scan_run_folder(run: &Path) -> Vec<(Side, String, PathBuf)> {
    let mut out = Vec::new();
    for (side, dir) in [(Side::Login, "login/config"), (Side::Game, "game/config")] {
        let base = run.join(dir);
        let mut rels = Vec::new();
        collect_ini(&base, "", 0, &mut rels);
        if side == Side::Game && !rels.iter().any(|r| r == GEO_OVERRIDE_FILE) {
            rels.push(GEO_OVERRIDE_FILE.to_owned());
        }
        rels.sort_by_key(|r| r.to_lowercase());
        out.extend(rels.into_iter().map(|rel| {
            let path = base.join(&rel);
            (side, rel, path)
        }));
    }
    out
}

fn collect_ini(dir: &Path, prefix: &str, depth: usize, out: &mut Vec<String>) {
    let Ok(read) = std::fs::read_dir(dir) else { return };
    for entry in read.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        if path.is_dir() {
            if depth < 1 {
                collect_ini(&path, &format!("{prefix}{name}/"), depth + 1, out);
            }
        } else if name.ends_with(".ini") {
            out.push(format!("{prefix}{name}"));
        }
    }
}

/// "Restart the login server to apply: ..." lines for the saved files.
pub fn restart_notes(saved: &[(Side, String)]) -> Vec<String> {
    let mut notes = Vec::new();
    for (side, name) in [(Side::Login, "login"), (Side::Game, "game")] {
        let files: Vec<&str> = saved
            .iter()
            .filter(|(s, _)| *s == side)
            .map(|(_, f)| f.as_str())
            .collect();
        if !files.is_empty() {
            notes.push(format!(
                "Restart the {name} server to apply: {}",
                files.join(", ")
            ));
        }
    }
    notes
}

pub struct ConfigEditorView {
    run_dir: String,
    files: Vec<FileState>,
    selected: usize,
    search: String,
    loaded: bool,
    errors: Vec<String>,
    /// Result of the last save: restart notes, or the error.
    notes: Vec<String>,
}

impl Default for ConfigEditorView {
    fn default() -> Self {
        Self {
            run_dir: DEFAULT_RUN_DIR.to_owned(),
            files: Vec::new(),
            selected: 0,
            search: String::new(),
            loaded: false,
            errors: Vec::new(),
            notes: Vec::new(),
        }
    }
}

impl ConfigEditorView {
    fn reload(&mut self) {
        let keep = self.files.get(self.selected).map(|f| (f.side, f.rel.clone()));
        self.files.clear();
        self.errors.clear();
        for (side, rel, path) in scan_run_folder(Path::new(self.run_dir.trim())) {
            match FileState::load(side, &rel, &path) {
                Ok(file) => self.files.push(file),
                Err(e) => self.errors.push(e),
            }
        }
        self.selected = keep
            .and_then(|(s, r)| self.files.iter().position(|f| f.side == s && f.rel == r))
            .unwrap_or(0);
        self.loaded = true;
    }

    fn dirty_files(&self) -> usize {
        self.files.iter().filter(|f| !f.changes().is_empty()).count()
    }

    fn save_all(&mut self) {
        let problems: Vec<String> = self.files.iter().flat_map(FileState::problems).collect();
        if !problems.is_empty() {
            self.notes = problems;
            return;
        }
        let mut saved = Vec::new();
        let mut notes = Vec::new();
        for file in &self.files {
            if file.changes().is_empty() {
                continue;
            }
            match file.save() {
                Ok(n) if n > 0 => saved.push((file.side, file.rel.clone())),
                Ok(_) => {}
                Err(e) => notes.push(format!("Not saved: {e}")),
            }
        }
        notes.extend(restart_notes(&saved));
        self.reload();
        self.notes = notes;
    }
}

fn server_name(side: Side) -> &'static str {
    match side {
        Side::Login => "Login server",
        Side::Game => "Game server",
    }
}

impl View for ConfigEditorView {
    fn ui(&mut self, ui: &mut egui::Ui, _cx: &mut AppCtx<'_>) {
        if !self.loaded {
            self.reload();
        }
        ui.heading("Config editor");
        ui.horizontal(|ui| {
            ui.label("Run folder:");
            ui.add(egui::TextEdit::singleline(&mut self.run_dir).desired_width(260.0));
            if ui
                .button("Reload")
                .on_hover_text("Discards unsaved edits")
                .clicked()
            {
                self.reload();
                self.notes.clear();
            }
            ui.label("Search:");
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text("key or comment, in all files")
                    .desired_width(240.0),
            );
        });
        let dirty = self.dirty_files();
        ui.horizontal(|ui| {
            if ui
                .add_enabled(dirty > 0, egui::Button::new(format!("Save ({dirty} files)")))
                .clicked()
            {
                self.save_all();
            }
            if ui
                .add_enabled(dirty > 0, egui::Button::new("Discard edits"))
                .clicked()
            {
                self.files.iter_mut().for_each(FileState::discard);
            }
        });
        for note in &self.notes {
            ui.colored_label(ui.visuals().warn_fg_color, note.as_str());
        }
        for error in &self.errors {
            ui.colored_label(ui.visuals().error_fg_color, error.as_str());
        }
        ui.separator();
        if self.files.is_empty() {
            ui.label("No config files found. Set the run folder (it holds login/config and game/config).");
            return;
        }

        let query = self.search.trim().to_lowercase();
        let height = ui.available_height();
        ui.horizontal_top(|ui| {
            ui.allocate_ui(egui::vec2(230.0, height), |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("cfg_files")
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.file_list(ui, &query));
            });
            ui.separator();
            egui::ScrollArea::vertical()
                .id_salt("cfg_keys")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if query.is_empty() {
                        if let Some(file) = self.files.get_mut(self.selected) {
                            draw_file(ui, file, &query, false);
                        }
                    } else {
                        for file in &mut self.files {
                            draw_file(ui, file, &query, true);
                        }
                    }
                });
        });
    }
}

impl ConfigEditorView {
    fn file_list(&mut self, ui: &mut egui::Ui, query: &str) {
        let mut last_side = None;
        for (i, file) in self.files.iter().enumerate() {
            if last_side != Some(file.side) {
                ui.strong(server_name(file.side));
                last_side = Some(file.side);
            }
            let hits = file.entries.iter().filter(|e| file.matches(e, query)).count();
            if !query.is_empty() && hits == 0 {
                continue;
            }
            let mut label = file.rel.clone();
            if !file.changes().is_empty() {
                label.push_str(" *");
            }
            if file.is_new {
                label.push_str(" (new)");
            }
            if !query.is_empty() {
                label.push_str(&format!("  [{hits}]"));
            }
            ui.selectable_value(&mut self.selected, i, label);
        }
    }
}

/// Draws the keys of one file (those matching `query`); `with_title` adds the file name on top.
fn draw_file(ui: &mut egui::Ui, file: &mut FileState, query: &str, with_title: bool) {
    let entries: Vec<IniEntry> = file
        .entries
        .iter()
        .filter(|e| file.matches(e, query))
        .cloned()
        .collect();
    if entries.is_empty() {
        return;
    }
    if with_title {
        ui.heading(format!("{} / {}", server_name(file.side), file.rel));
    }
    let mut group: Option<&str> = None;
    for entry in &entries {
        if group != Some(entry.group.as_str()) {
            if !entry.group.is_empty() {
                ui.add_space(6.0);
                ui.strong(egui::RichText::new(&entry.group).size(15.0));
            }
            group = Some(entry.group.as_str());
        }
        draw_entry(ui, file, entry);
    }
}

fn draw_entry(ui: &mut egui::Ui, file: &mut FileState, entry: &IniEntry) {
    let default = file.default_of(entry);
    let kind = file.kind_of(entry);
    let secret = file.is_secret(&entry.key);
    let differs = file.differs_from_default(entry);
    let problem = file.problem(entry);
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let mut name = egui::RichText::new(&entry.key).strong();
        if file.is_dirty(entry) {
            name = name.italics();
        }
        ui.label(name);
        if file.is_dirty(entry) {
            ui.label("*").on_hover_text("Changed, not saved yet");
        }
        if differs {
            let shown = match (&default, secret) {
                (Some(d), false) => format!("Differs from the default ({d})"),
                _ => "Differs from the default".to_owned(),
            };
            ui.colored_label(egui::Color32::from_rgb(0xe0, 0xa0, 0x30), "\u{25cf}")
                .on_hover_text(shown);
        }
        if default.is_some() && differs && ui.small_button("Reset to default").clicked() {
            file.reset_to_default(entry);
        }
    });
    let Some(buffer) = file.buffers.get_mut(&entry.key) else {
        return;
    };
    ui.horizontal(|ui| {
        let editable = entry.editable;
        match kind {
            _ if secret => {
                ui.add_enabled(
                    editable,
                    egui::TextEdit::singleline(buffer)
                        .password(true)
                        .desired_width(320.0),
                )
                .on_hover_text(format!(
                    "Hidden ({}). Only written when changed.",
                    mask_value(&entry.value)
                ));
            }
            ValueKind::Bool => {
                let mut on = buffer.trim().eq_ignore_ascii_case("true");
                if ui
                    .add_enabled(editable, egui::Checkbox::without_text(&mut on))
                    .changed()
                {
                    // Keep the file's spelling (`True` / `true`).
                    let capital = entry.value.chars().next().is_none_or(char::is_uppercase);
                    *buffer = match (on, capital) {
                        (true, true) => "True",
                        (true, false) => "true",
                        (false, true) => "False",
                        (false, false) => "false",
                    }
                    .to_owned();
                }
                ui.label(buffer.as_str());
            }
            _ => {
                let width = if kind == ValueKind::Text { 420.0 } else { 140.0 };
                let mut edit = egui::TextEdit::singleline(buffer).desired_width(width);
                if problem.is_some() {
                    edit = edit.text_color(egui::Color32::from_rgb(0xd9, 0x4f, 0x4f));
                }
                ui.add_enabled(editable, edit);
            }
        }
        if let (Some(d), false) = (&default, secret) {
            ui.weak(format!("default: {d}"));
        }
    });
    if let Some(problem) = problem {
        ui.colored_label(ui.visuals().error_fg_color, problem);
    }
    for line in &entry.help {
        ui.add(egui::Label::new(egui::RichText::new(line).weak().small()).wrap());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = "# ---\n# Main\n# ---\n\n# Port to listen on.\n# Default: 2106\nLoginserverPort = 2106\n\n# Flag.\n# Default: True\nShowLicence = False\n# Rate\nRateXp = 2\r\nLogin = root\nPassword = hunter2\nToken =\n";

    fn state(rel: &str) -> FileState {
        FileState::from_text(Side::Login, rel, Path::new("x.ini"), TEXT.to_owned(), false)
    }

    #[test]
    fn kinds_follow_value_and_default() {
        assert_eq!(detect_kind("True", None), ValueKind::Bool);
        assert_eq!(detect_kind("", Some("false")), ValueKind::Bool);
        assert_eq!(detect_kind("12", Some("5")), ValueKind::Int);
        assert_eq!(detect_kind("1", Some("1.0")), ValueKind::Float);
        assert_eq!(detect_kind("1.5", None), ValueKind::Float);
        assert_eq!(detect_kind("", Some("7")), ValueKind::Int);
        assert_eq!(detect_kind("inf", None), ValueKind::Text);
        assert_eq!(detect_kind("abc", Some("1")), ValueKind::Text);
    }

    #[test]
    fn numbers_get_a_range_check() {
        assert!(validate(ValueKind::Int, "2147483647", false).is_ok());
        assert!(validate(ValueKind::Int, "2147483648", false).is_err());
        assert!(validate(ValueKind::Int, "2147483648", true).is_ok());
        assert!(validate(ValueKind::Int, "1.5", true).is_err());
        assert!(validate(ValueKind::Float, "1.", true).is_ok());
        assert!(validate(ValueKind::Float, "1.5f", true).is_ok());
        assert!(validate(ValueKind::Float, "NaN", true).is_err());
        assert!(validate(ValueKind::Bool, "yes", true).is_err());
        assert!(validate(ValueKind::Text, "a\\", true).is_err());
        assert!(validate(ValueKind::Text, " a", true).is_err());
        assert!(validate(ValueKind::Text, "", true).is_ok());
    }

    #[test]
    fn defaults_and_the_marker() {
        let mut f = state("Server.ini");
        let port = f.entries[0].clone();
        assert_eq!(f.default_of(&port).as_deref(), Some("2106"));
        assert!(!f.differs_from_default(&port));
        f.buffers.insert("LoginserverPort".into(), "2107".into());
        assert!(f.differs_from_default(&port));
        assert!(f.is_dirty(&port));
        f.reset_to_default(&port);
        assert!(!f.differs_from_default(&port));
        assert!(!f.is_dirty(&port));
        // "ShowLicence = False" differs from the typed default (true).
        let licence = f.entries[1].clone();
        assert!(f.differs_from_default(&licence));
        // `2` equals the default `1.0`-style float of another key: numeric compare.
        assert!(values_equal("1", "1.0"));
        assert!(values_equal("True", "true"));
        assert!(!values_equal("a", "b"));
    }

    #[test]
    fn help_comes_with_the_entry() {
        let f = state("Server.ini");
        assert_eq!(f.entries[0].help, ["Port to listen on.", "Default: 2106"]);
        assert_eq!(f.entries[0].group, "Main");
        assert!(f.matches(&f.entries[0], "listen on"));
        assert!(f.matches(&f.entries[0], "loginserverport"));
        assert!(!f.matches(&f.entries[0], "nothing"));
    }

    #[test]
    fn secrets_are_masked_and_only_written_when_changed() {
        let mut f = FileState::from_text(
            Side::Login,
            "Database.ini",
            Path::new("x.ini"),
            TEXT.to_owned(),
            false,
        );
        assert!(f.is_secret("Password") && f.is_secret("Token") && f.is_secret("Login"));
        assert!(!f.is_secret("RateXp"));
        assert!(f.changes().is_empty());
        f.buffers.insert("Password".into(), "new".into());
        assert_eq!(f.changes(), [("Password".to_owned(), "new".to_owned())]);
        // The summary and the tooltip use the mask, never the value.
        assert!(!mask_value("hunter2").contains("hunter2"));
    }

    #[test]
    fn save_changes_only_the_edited_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Server.ini");
        std::fs::write(&path, TEXT).unwrap();
        let mut f = FileState::load(Side::Login, "Server.ini", &path).unwrap();
        f.buffers.insert("RateXp".into(), "5".into());
        f.buffers.insert("Token".into(), "abc".into());
        assert_eq!(f.save().unwrap(), 2);
        let saved = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            saved,
            TEXT.replace("RateXp = 2", "RateXp = 5")
                .replace("Token =", "Token = abc")
        );
        assert!(saved.contains("RateXp = 5\r\n"));
    }

    #[test]
    fn invalid_edits_are_reported_and_not_saved() {
        let mut f = state("Server.ini");
        f.buffers.insert("LoginserverPort".into(), "99999999999".into());
        f.buffers.insert("ShowLicence".into(), "maybe".into());
        assert_eq!(f.problems().len(), 2);
    }

    #[test]
    fn missing_geo_override_is_a_template_and_other_missing_files_are_errors() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("GeoOverride.ini");
        let mut f = FileState::load(Side::Game, "GeoOverride.ini", &path).unwrap();
        assert!(f.is_new);
        assert_eq!(f.entries.len(), 3);
        f.buffers.insert("PathWorkerThreads".into(), "4".into());
        assert_eq!(f.save().unwrap(), 1);
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .contains("PathWorkerThreads = 4")
        );
        assert!(FileState::load(Side::Game, "Nope.ini", &dir.path().join("Nope.ini")).is_err());
    }

    #[test]
    fn scans_both_servers_and_adds_the_geo_override() {
        let dir = tempfile::tempdir().unwrap();
        let login = dir.path().join("login/config");
        let custom = dir.path().join("game/config/Custom");
        std::fs::create_dir_all(&login).unwrap();
        std::fs::create_dir_all(&custom).unwrap();
        std::fs::write(login.join("Server.ini"), "").unwrap();
        std::fs::write(login.join("note.txt"), "").unwrap();
        std::fs::write(dir.path().join("game/config/General.ini"), "").unwrap();
        std::fs::write(custom.join("Banking.ini"), "").unwrap();
        let found: Vec<(Side, String)> = scan_run_folder(dir.path())
            .into_iter()
            .map(|(s, r, _)| (s, r))
            .collect();
        assert_eq!(
            found,
            [
                (Side::Login, "Server.ini".to_owned()),
                (Side::Game, "Custom/Banking.ini".to_owned()),
                (Side::Game, "General.ini".to_owned()),
                (Side::Game, "GeoOverride.ini".to_owned()),
            ]
        );
    }

    #[test]
    fn restart_notes_group_by_server() {
        let notes = restart_notes(&[
            (Side::Game, "Rates.ini".into()),
            (Side::Login, "Server.ini".into()),
            (Side::Game, "General.ini".into()),
        ]);
        assert_eq!(
            notes,
            [
                "Restart the login server to apply: Server.ini",
                "Restart the game server to apply: Rates.ini, General.ini"
            ]
        );
        assert!(restart_notes(&[]).is_empty());
    }

    #[test]
    fn every_dist_file_loads_with_help() {
        let dist = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist");
        if !dist.is_dir() {
            eprintln!("skipped: no dist/");
            return;
        }
        let mut keys = 0;
        for (side, dir) in [(Side::Login, "login/config"), (Side::Game, "game/config")] {
            let base = dist.join(dir);
            let mut rels = Vec::new();
            collect_ini(&base, "", 0, &mut rels);
            for rel in rels {
                let f = FileState::load(side, &rel, &base.join(&rel)).unwrap();
                assert!(f.changes().is_empty(), "{rel}");
                keys += f.entries.len();
            }
        }
        assert!(keys > 1000, "{keys}");
    }
}
