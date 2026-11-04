//! Config editor: the `.ini` files of the run folder (`run/login/config`, `run/game/config`),
//! one control per key, with the comment lines as help, the Java default and a reset button
//! (cards G-05, G-07).
//!
//! (`ini_edit`, `defaults`); only the changed lines are rewritten, so comments, order and line
//! endings stay. [`FileState`] holds the rules and has no egui in it, so it is tested directly.
//! The view only draws it.
//!
//! - `mod.rs`: the file state and the window (search, save, the "Changed settings" list, the
//!   by-file layout of G-05).
//! - `topic_view.rs`: the by-topic layout (G-07), built on `l2_config::topics`, with the XML
//!   forms of `xml_config` (G-08) as the last group of the topic list (G-09).
//! - `controls.rs`: which control a key gets (switch, slider, drop-down, ...) and how it is drawn.

mod controls;
mod topic_view;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use l2_config::defaults::{Side, default_for, java_defaults};
use l2_config::ini_edit::{
    GEO_OVERRIDE_TEMPLATE, IniDoc, IniEntry, apply_changes, is_secret, mask_value, write_atomic,
};
use l2_config::ranges::is_list_key;

use self::controls::draw_entry;
use self::topic_view::{Bucket, TopicIndex};
use crate::app::AppCtx;
use crate::views::View;
use crate::views::xml_config::XmlConfigView;

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

/// One edited key, for the "Changed settings" list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangeLine {
    pub side: Side,
    pub rel: String,
    pub key: String,
    pub old: String,
    pub new: String,
    pub secret: bool,
}

impl ChangeLine {
    fn shown(&self, value: &str) -> String {
        let text = if self.secret {
            mask_value(value)
        } else {
            value.to_owned()
        };
        if text.is_empty() {
            "(empty)".to_owned()
        } else {
            text
        }
    }

    /// The old value as the list shows it: a secret is masked, an empty value says so.
    pub fn old_shown(&self) -> String {
        self.shown(&self.old)
    }

    pub fn new_shown(&self) -> String {
        self.shown(&self.new)
    }
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
        // `ServerRestartDays = 4` is a list that happens to hold one number; it must accept `4,5`.
        if is_list_key(&entry.key) {
            return ValueKind::Text;
        }
        detect_kind(&entry.value, self.default_of(entry).as_deref())
    }

    /// Old and new value of every edited key, for the "Changed settings" list.
    pub fn change_list(&self) -> Vec<ChangeLine> {
        self.entries
            .iter()
            .filter(|e| self.is_dirty(e))
            .filter_map(|e| {
                Some(ChangeLine {
                    side: self.side,
                    rel: self.rel.clone(),
                    key: e.key.clone(),
                    old: e.value.clone(),
                    new: self.buffers.get(&e.key)?.clone(),
                    secret: self.is_secret(&e.key),
                })
            })
            .collect()
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

/// How the settings are organised on screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Left: topics (a topic groups settings of several files). The default.
    Topics,
    /// Left: the files, as in G-05.
    Files,
}

pub struct ConfigEditorView {
    run_dir: String,
    files: Vec<FileState>,
    index: TopicIndex,
    mode: Mode,
    /// The file shown in [`Mode::Files`].
    selected: usize,
    /// The topic shown in [`Mode::Topics`]; `None` = the first one.
    topic: Option<Bucket>,
    search: String,
    loaded: bool,
    errors: Vec<String>,
    /// Result of the last save: restart notes, or the error.
    notes: Vec<String>,
    /// The "Changed settings" window is open (opened by the save button).
    review: bool,
    /// The XML configs (`AccessLevels.xml`, ...): the last group of the topic list.
    xml: XmlConfigView,
    /// [`Mode::Files`]: the XML group is chosen, not a file.
    files_xml: bool,
}

impl Default for ConfigEditorView {
    fn default() -> Self {
        Self::with_run_dir(DEFAULT_RUN_DIR)
    }
}

impl ConfigEditorView {
    pub fn with_run_dir(run_dir: &str) -> Self {
        Self {
            run_dir: run_dir.to_owned(),
            files: Vec::new(),
            index: TopicIndex::default(),
            mode: Mode::Topics,
            selected: 0,
            topic: None,
            search: String::new(),
            loaded: false,
            errors: Vec::new(),
            notes: Vec::new(),
            review: false,
            xml: XmlConfigView::with_run_dir(run_dir),
            files_xml: false,
        }
    }

    /// Reads the `.ini` files and the XML configs again; unsaved edits are dropped.
    fn reload(&mut self) {
        self.xml.set_run_dir(&self.run_dir);
        self.xml.reload();
        self.reload_ini();
    }

    fn reload_ini(&mut self) {
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
        self.index = TopicIndex::build(&self.files);
        if self.topic.as_ref().is_some_and(|t| !self.has_topic(t)) {
            self.topic = None;
        }
        self.loaded = true;
    }

    fn dirty_files(&self) -> usize {
        self.files.iter().filter(|f| !f.changes().is_empty()).count()
    }

    fn all_changes(&self) -> Vec<ChangeLine> {
        self.files.iter().flat_map(FileState::change_list).collect()
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
        // Only the `.ini` files: unsaved XML edits stay.
        self.reload_ini();
        self.notes = notes;
    }

    /// The "Changed settings" window: old and new value of every edited key, before anything is
    /// written. Nothing is saved until the button in here is pressed.
    fn review_window(&mut self, ctx: &egui::Context) {
        if !self.review {
            return;
        }
        let lines = self.all_changes();
        let problems: Vec<String> = self.files.iter().flat_map(FileState::problems).collect();
        let mut open = true;
        let (mut save, mut cancel) = (false, false);
        egui::Window::new("Changed settings")
            .id(egui::Id::unique("cfg_review"))
            .collapsible(false)
            .default_width(620.0)
            .open(&mut open)
            .show(ctx, |ui| {
                if lines.is_empty() {
                    ui.label("Nothing is changed.");
                } else {
                    egui::ScrollArea::vertical()
                        .id_salt("cfg_review_scroll")
                        .max_height(380.0)
                        .show(ui, |ui| {
                            egui::Grid::new("cfg_review_grid").striped(true).show(ui, |ui| {
                                ui.strong("File");
                                ui.strong("Setting");
                                ui.strong("Before");
                                ui.strong("After");
                                ui.end_row();
                                for line in &lines {
                                    ui.weak(format!("{} / {}", server_word(line.side), line.rel));
                                    ui.label(&line.key);
                                    ui.label(line.old_shown());
                                    ui.label(line.new_shown());
                                    ui.end_row();
                                }
                            });
                        });
                }
                for problem in &problems {
                    ui.colored_label(ui.visuals().error_fg_color, problem.as_str());
                }
                ui.separator();
                ui.horizontal(|ui| {
                    let label = format!("Save {} change(s)", lines.len());
                    if ui
                        .add_enabled(!lines.is_empty() && problems.is_empty(), egui::Button::new(label))
                        .clicked()
                    {
                        save = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
        if save {
            self.save_all();
        }
        if save || cancel || !open {
            self.review = false;
        }
    }
}

pub(super) fn server_name(side: Side) -> &'static str {
    match side {
        Side::Login => "Login server",
        Side::Game => "Game server",
    }
}

/// `login` / `game`, for the small print next to a setting.
pub(super) fn server_word(side: Side) -> &'static str {
    match side {
        Side::Login => "login",
        Side::Game => "game",
    }
}

impl View for ConfigEditorView {
    fn ui(&mut self, ui: &mut egui::Ui, _cx: &mut AppCtx<'_>) {
        if !self.loaded {
            self.reload();
        }
        // Ctrl+F (Cmd+F on a Mac) jumps to the search box.
        let ctrl_f = ui
            .ctx()
            .input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::F));
        ui.heading("Config editor");
        ui.horizontal_wrapped(|ui| {
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
            ui.separator();
            ui.label("Show:");
            ui.selectable_value(&mut self.mode, Mode::Topics, "By topic");
            ui.selectable_value(&mut self.mode, Mode::Files, "By file");
        });
        let dirty = self.dirty_files();
        ui.horizontal_wrapped(|ui| {
            ui.label("Search:");
            let search = ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text("key, comment, file or topic (Ctrl+F)")
                    .desired_width(300.0),
            );
            if ctrl_f {
                search.request_focus();
            }
            if !self.search.is_empty() && ui.small_button("Clear").clicked() {
                self.search.clear();
            }
            ui.separator();
            if ui
                .add_enabled(
                    dirty > 0,
                    egui::Button::new(format!("Review and save ({dirty} files)")),
                )
                .clicked()
            {
                self.review = true;
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
        self.xml.actions_ui(ui);
        let ctx = ui.ctx().clone();
        self.review_window(&ctx);
        ui.separator();
        if self.files.is_empty() && !self.xml.has_files() {
            ui.label("No config files found. Set the run folder (it holds login/config and game/config).");
            return;
        }

        let query = self.search.trim().to_lowercase();
        match self.mode {
            Mode::Topics => self.topics_body(ui, &query),
            Mode::Files => self.files_body(ui, &query),
        }
    }
}

impl ConfigEditorView {
    /// The G-05 layout: files on the left, the keys of the selected file (or of all files that
    /// match the search) on the right.
    fn files_body(&mut self, ui: &mut egui::Ui, query: &str) {
        let height = ui.available_height();
        ui.horizontal_top(|ui| {
            ui.allocate_ui(egui::vec2(230.0, height), |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("cfg_files")
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.file_list(ui, query));
            });
            ui.separator();
            if self.files_xml && self.xml.hits(query) > 0 {
                ui.allocate_ui(egui::vec2(ui.available_width(), height), |ui| {
                    self.xml.form_ui(ui)
                });
                return;
            }
            egui::ScrollArea::vertical()
                .id_salt("cfg_keys")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if query.is_empty() {
                        if let Some(file) = self.files.get_mut(self.selected) {
                            draw_file(ui, file, query, false);
                        }
                    } else {
                        for file in &mut self.files {
                            draw_file(ui, file, query, true);
                        }
                    }
                });
        });
    }

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
            if ui
                .selectable_label(!self.files_xml && self.selected == i, label)
                .clicked()
            {
                self.selected = i;
                self.files_xml = false;
            }
        }
        // The XML configs have no `.ini` file of their own: they close the list.
        ui.add_space(8.0);
        if self.xml.topic_list(ui, self.files_xml, query) {
            self.files_xml = true;
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
        draw_entry(ui, file, entry, false);
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

    #[test]
    fn list_keys_stay_text_even_with_one_number_in_them() {
        let f = FileState::from_text(
            Side::Game,
            "Server.ini",
            Path::new("x.ini"),
            "ServerRestartDays = 4\nLogItemsIdsList = 4356\nCharMaxNumber = 7\n".to_owned(),
            false,
        );
        assert_eq!(f.kind_of(&f.entries[0]), ValueKind::Text);
        assert_eq!(f.kind_of(&f.entries[1]), ValueKind::Text);
        assert_eq!(f.kind_of(&f.entries[2]), ValueKind::Int);
        // So `4,5` is accepted for the list but not for the number.
        let mut f = f;
        f.buffers.insert("ServerRestartDays".into(), "4,5".into());
        f.buffers.insert("CharMaxNumber".into(), "7,8".into());
        assert_eq!(f.problems().len(), 1);
    }

    #[test]
    fn the_change_list_shows_old_and_new_and_masks_secrets() {
        let mut f = FileState::from_text(
            Side::Login,
            "Database.ini",
            Path::new("x.ini"),
            TEXT.to_owned(),
            false,
        );
        assert!(f.change_list().is_empty());
        f.buffers.insert("RateXp".into(), "5".into());
        f.buffers.insert("Password".into(), "swordfish".into());
        f.buffers.insert("Token".into(), "abc".into());
        f.buffers.insert("LoginserverPort".into(), "2107".into());
        let list = f.change_list();
        let by_key = |k: &str| list.iter().find(|l| l.key == k).unwrap();
        assert_eq!(list.len(), 4);
        let xp = by_key("RateXp");
        assert_eq!((xp.old_shown().as_str(), xp.new_shown().as_str()), ("2", "5"));
        assert_eq!(by_key("LoginserverPort").old_shown(), "2106");
        // Secrets: never the value, and an empty value says so.
        let pw = by_key("Password");
        assert!(!pw.old_shown().contains("hunter2") && !pw.new_shown().contains("swordfish"));
        let token = by_key("Token");
        assert_eq!(token.old_shown(), "(empty)");
        assert_ne!(token.new_shown(), "abc");
        // Reset to the file's own values: nothing left to list.
        f.discard();
        assert!(f.change_list().is_empty());
    }

    // ---- against a copy of the repo's dist/ ----

    fn copy_dir(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for entry in std::fs::read_dir(from).unwrap().flatten() {
            let (src, dst) = (entry.path(), to.join(entry.file_name()));
            if src.is_dir() {
                copy_dir(&src, &dst);
            } else {
                std::fs::copy(&src, &dst).unwrap();
            }
        }
    }

    /// A run folder made of the `.ini` files of `dist/` (never the real `run/` folder). `None`
    /// when `dist/` is not there.
    fn run_folder_copy() -> Option<tempfile::TempDir> {
        let dist = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist");
        if !dist.join("game/config").is_dir() {
            eprintln!("skipped: no dist/");
            return None;
        }
        let dir = tempfile::tempdir().unwrap();
        for side in ["login", "game"] {
            copy_dir(
                &dist.join(side).join("config"),
                &dir.path().join(side).join("config"),
            );
        }
        Some(dir)
    }

    fn read_all(dir: &Path) -> BTreeMap<String, Vec<u8>> {
        scan_run_folder(dir)
            .into_iter()
            .filter_map(|(side, rel, path)| {
                Some((format!("{}/{rel}", server_word(side)), std::fs::read(path).ok()?))
            })
            .collect()
    }

    #[test]
    fn saving_without_a_change_leaves_every_file_byte_identical() {
        let Some(dir) = run_folder_copy() else { return };
        let before = read_all(dir.path());
        assert!(before.len() > 70);
        let mut view = ConfigEditorView::with_run_dir(&dir.path().to_string_lossy());
        view.reload();
        assert!(view.errors.is_empty(), "{:?}", view.errors);
        assert_eq!(view.dirty_files(), 0);
        view.save_all();
        assert_eq!(read_all(dir.path()), before);
        // The list in the review window is empty too.
        assert!(view.all_changes().is_empty());
    }

    #[test]
    fn one_change_through_a_control_touches_one_line() {
        let Some(dir) = run_folder_copy() else { return };
        let path = dir.path().join("game/config/Rates.ini");
        let before = std::fs::read_to_string(&path).unwrap();
        let mut view = ConfigEditorView::with_run_dir(&dir.path().to_string_lossy());
        view.reload();
        let file = view
            .files
            .iter_mut()
            .find(|f| f.side == Side::Game && f.rel == "Rates.ini")
            .unwrap();
        // What a slider writes for RateXp.
        file.buffers.insert("RateXp".into(), "7.5".into());
        assert_eq!(view.all_changes().len(), 1);
        view.save_all();
        let after = std::fs::read_to_string(&path).unwrap();
        let changed: Vec<(&str, &str)> = before
            .lines()
            .zip(after.lines())
            .filter(|(a, b)| a != b)
            .collect();
        assert_eq!(before.lines().count(), after.lines().count());
        assert_eq!(changed, [("RateXp = 1", "RateXp = 7.5")]);
        assert!(view.notes.iter().any(|n| n.contains("Restart the game server")));
    }

    fn draw(view: &mut ConfigEditorView, frames: usize, size: egui::Vec2) {
        use crate::bridge::{Bridge, ConnState};
        use crate::settings::Settings;

        let mut settings = Settings::default();
        let bridge = Bridge::spawn(None).unwrap();
        let conn = ConnState::default();
        let launch = crate::launch::LaunchHandle::detached();
        let ctx = egui::Context::default();
        for _ in 0..frames {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
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
    fn every_topic_draws_headless_on_a_narrow_and_a_wide_window() {
        let Some(dir) = run_folder_copy() else { return };
        let mut view = ConfigEditorView::with_run_dir(&dir.path().to_string_lossy());
        view.reload();
        let buckets: Vec<Bucket> = view.index.buckets().iter().map(|(b, _)| b.clone()).collect();
        // 17 fixed topics + the Custom files; nothing falls into "Other" with dist/ alone.
        assert!(buckets.len() >= 17 + 40, "{}", buckets.len());
        assert!(!buckets.contains(&Bucket::Other));
        for bucket in buckets {
            view.topic = Some(bucket.clone());
            draw(&mut view, 2, egui::vec2(900.0, 700.0));
            draw(&mut view, 2, egui::vec2(1500.0, 800.0));
            assert_eq!(view.current_topic(), Some(bucket));
        }
    }

    #[test]
    fn the_by_file_view_the_search_and_the_review_window_draw() {
        let Some(dir) = run_folder_copy() else { return };
        let mut view = ConfigEditorView::with_run_dir(&dir.path().to_string_lossy());
        view.reload();
        view.mode = Mode::Files;
        draw(&mut view, 2, egui::vec2(1100.0, 800.0));
        view.search = "pathfinding".into();
        draw(&mut view, 2, egui::vec2(1100.0, 800.0));
        view.mode = Mode::Topics;
        view.search = "rate".into();
        draw(&mut view, 2, egui::vec2(1100.0, 800.0));
        view.search = "no such setting anywhere".into();
        draw(&mut view, 2, egui::vec2(1100.0, 800.0));
        // The review window with an edit, a secret and a bad value.
        view.search.clear();
        let file = view
            .files
            .iter_mut()
            .find(|f| f.side == Side::Game && f.rel == "Database.ini")
            .unwrap();
        file.buffers.insert("Password".into(), "swordfish".into());
        view.review = true;
        draw(&mut view, 2, egui::vec2(1100.0, 800.0));
        assert!(view.review);
        assert_eq!(view.all_changes().len(), 1);
    }

    // ---- the XML configs group (G-09) ----

    use crate::views::xml_config::TopicId;

    #[test]
    fn the_xml_configs_are_the_last_group_of_the_topic_list() {
        let Some(dir) = run_folder_copy() else { return };
        let mut view = ConfigEditorView::with_run_dir(&dir.path().to_string_lossy());
        view.reload();
        assert!(view.xml.has_files(), "{:?}", view.xml.errors());
        assert!(view.has_topic(&Bucket::Xml));
        // The group is not an `.ini` topic: no index entry, and every setting is still in one bucket.
        assert!(!view.index.has(&Bucket::Xml));
        // Every XML topic draws, narrow and wide, as the chosen topic.
        let topics: Vec<TopicId> = view.xml.files().iter().map(|f| f.topic).collect();
        assert!(topics.len() >= 9);
        for topic in topics {
            assert!(view.xml.select(topic));
            view.topic = Some(Bucket::Xml);
            draw(&mut view, 2, egui::vec2(900.0, 700.0));
            draw(&mut view, 2, egui::vec2(1500.0, 800.0));
            assert_eq!(view.current_topic(), Some(Bucket::Xml));
            assert_eq!(view.xml.selected_topic(), Some(topic));
        }
    }

    #[test]
    fn the_search_finds_xml_topics_by_title_and_by_file_name() {
        let Some(dir) = run_folder_copy() else { return };
        let mut view = ConfigEditorView::with_run_dir(&dir.path().to_string_lossy());
        view.reload();
        let total = view.xml.hits("");
        assert!(total >= 9);
        // The file name.
        assert_eq!(view.xml.hits("dynamicexprates.xml"), 1);
        assert_eq!(view.xml.hits("classmaster"), 1);
        // The title.
        assert_eq!(view.xml.hits("dynamic exp"), 1);
        assert_eq!(view.xml.hits("siege schedule"), 1);
        // The name of the group finds all of them.
        assert_eq!(view.xml.hits("xml configs"), total);
        assert_eq!(view.xml.hits("no such topic"), 0);
        // Drawn in a search, with and without a hit among the settings.
        for query in ["dynamicexprates.xml", "ban", "no such topic anywhere"] {
            view.search = query.into();
            view.topic = Some(Bucket::Xml);
            draw(&mut view, 2, egui::vec2(1100.0, 800.0));
            view.mode = Mode::Files;
            draw(&mut view, 2, egui::vec2(1100.0, 800.0));
            view.mode = Mode::Topics;
        }
    }

    #[test]
    fn the_by_file_layout_has_the_xml_group_too() {
        let Some(dir) = run_folder_copy() else { return };
        let mut view = ConfigEditorView::with_run_dir(&dir.path().to_string_lossy());
        view.reload();
        view.mode = Mode::Files;
        view.files_xml = true;
        draw(&mut view, 2, egui::vec2(1100.0, 800.0));
        view.files_xml = false;
        draw(&mut view, 2, egui::vec2(1100.0, 800.0));
    }

    #[test]
    fn xml_edits_are_saved_by_their_own_button_and_leave_the_ini_edits_alone() {
        let Some(dir) = run_folder_copy() else { return };
        let mut view = ConfigEditorView::with_run_dir(&dir.path().to_string_lossy());
        view.reload();
        let path = dir.path().join("game/config/ClassMaster.xml");
        let before = std::fs::read_to_string(&path).unwrap();
        // An edit of an `.ini` file stays unsaved while the XML files are saved.
        let file = view
            .files
            .iter_mut()
            .find(|f| f.side == Side::Game && f.rel == "Rates.ini")
            .unwrap();
        file.buffers.insert("RateXp".into(), "7.5".into());
        assert!(view.xml.select(TopicId::ClassMaster));
        let file = view
            .xml
            .files_mut()
            .iter_mut()
            .find(|f| f.topic == TopicId::ClassMaster)
            .unwrap();
        let crate::views::xml_config::Form::ClassMaster(form) = &mut file.form else {
            unreachable!()
        };
        // The switches of the card: noblesse on the first option.
        form.options[0].rewards.set_flag("setNoble", true);
        assert_eq!(view.xml.dirty_files(), 1);
        view.topic = Some(Bucket::Xml);
        draw(&mut view, 2, egui::vec2(1100.0, 800.0));
        view.xml.save_all();
        let after = std::fs::read_to_string(&path).unwrap();
        assert_eq!(after.matches("<setNoble />").count(), 1, "{after}");
        assert_eq!(
            after.replace("<setNoble />", "").lines().count(),
            before.lines().count() + 3
        );
        assert_eq!(view.dirty_files(), 1, "the .ini edit is still there");
        assert_eq!(view.xml.dirty_files(), 0);
        assert!(view.xml.notes().iter().any(|n| n.contains("ClassMaster.xml")));
        // Reload drops what is left.
        view.reload();
        assert_eq!(view.dirty_files(), 0);
        assert!(view.xml.notes().is_empty());
    }

    #[test]
    fn the_run_folder_without_xml_files_still_shows_the_settings() {
        let Some(dir) = run_folder_copy() else { return };
        for entry in std::fs::read_dir(dir.path().join("game/config"))
            .unwrap()
            .flatten()
        {
            if entry.path().extension().is_some_and(|e| e == "xml") {
                std::fs::remove_file(entry.path()).unwrap();
            }
        }
        let mut view = ConfigEditorView::with_run_dir(&dir.path().to_string_lossy());
        view.reload();
        assert!(!view.xml.has_files());
        view.topic = Some(Bucket::Xml);
        assert_ne!(view.current_topic(), Some(Bucket::Xml));
        draw(&mut view, 2, egui::vec2(1100.0, 800.0));
    }

    #[test]
    fn the_controls_of_every_dist_key_draw_with_every_kind_of_edit() {
        // One frame over a topic with switches, sliders, drop-downs and colours, with an edit in
        // each so the "changed" and "differs" markers are drawn too.
        let Some(dir) = run_folder_copy() else { return };
        let mut view = ConfigEditorView::with_run_dir(&dir.path().to_string_lossy());
        view.reload();
        for file in &mut view.files {
            let keys: Vec<String> = file.entries.iter().map(|e| e.key.clone()).collect();
            for key in keys {
                let Some(entry) = file.entries.iter().find(|e| e.key == key).cloned() else {
                    continue;
                };
                let edited = match file.kind_of(&entry) {
                    ValueKind::Bool => "True".to_owned(),
                    ValueKind::Int | ValueKind::Float => "3".to_owned(),
                    ValueKind::Text => format!("{}x", entry.value),
                };
                file.buffers.insert(key, edited);
            }
        }
        let buckets: Vec<Bucket> = view.index.buckets().iter().map(|(b, _)| b.clone()).collect();
        for bucket in buckets.into_iter().take(20) {
            view.topic = Some(bucket);
            draw(&mut view, 1, egui::vec2(1400.0, 900.0));
        }
    }
}
