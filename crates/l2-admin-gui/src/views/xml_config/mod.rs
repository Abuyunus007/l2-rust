//! XML configs as forms (card G-08): `AccessLevels.xml`, `AdminCommands.xml`, `ClassMaster.xml`,
//! `DynamicExpRates.xml`, `SiegeSchedule.xml`, `BotReportPunishments.xml`, `SecondaryAuth.xml`,
//! `Scripts.xml` (read-only) and `default-ipconfig.xml` / `ipconfig.xml` of `run/game/config`.
//!
//! Java: none (tooling); the element and attribute names are those of the Java loaders, which
//! each form names in its module comment.
//!
//! # Where it is shown
//! Inside the config editor (G-09), as the "XML configs" group at the end of its topic list:
//! [`XmlConfigView`] has no window of its own. The editor owns one, calls
//! [`XmlConfigView::topic_list`] for the group, [`XmlConfigView::form_ui`] for the chosen form and
//! [`XmlConfigView::actions_ui`] for the save and discard buttons, and passes its run folder on
//! with [`XmlConfigView::set_run_dir`].
//!
//! # How it saves
//! A form never writes XML. It compares what the user edited with the file and produces
//! [`l2_config::xml_edit::Edit`]s ("replace this byte range"), applied to the text that was
//! loaded. Comments, order, tabs and line endings stay; no change means the same bytes. If the
//! file changed on disk since it was loaded, saving is refused and the user reloads.
//!
//! # Adding a form (the pattern)
//! 1. A module with a struct that holds [`model::RowList`]s (or other state) and implements
//!    [`XmlForm`]: `load(text)`, `is_dirty`, `issues`, `edits`, `ui`. Tests in the module: an
//!    unchanged form gives the same text, one change touches one element, the validation rules.
//! 2. A [`TopicId`] variant with its file and title, and a [`Form`] variant.
//!
//! # Restart
//! The Rust game server reads these files once at start and has no `//reload` for them (see
//! `l2-world/src/admin/reload.rs`), so every save ends with "Restart the game server".

pub mod access_levels;
pub mod admin_commands;
pub mod bot_report;
pub mod class_master;
pub mod dynamic_exp;
pub mod ipconfig;
pub mod model;
pub mod scripts;
pub mod secondary_auth;
pub mod siege;
pub mod widgets;

use std::path::{Path, PathBuf};

use l2_config::ini_edit::write_atomic;
use l2_config::xml_edit::{Edit, XmlDoc, apply_edits};

use model::{Issue, has_errors};

/// The config folder inside the run folder.
const CONFIG_SUBDIR: &str = "game/config";

/// What every form does. No egui in the data rules, so they are tested without a window.
pub trait XmlForm {
    /// Did the user change anything?
    fn is_dirty(&self) -> bool;
    /// The problems of the current values. An error (not a warning) blocks saving.
    fn issues(&self) -> Vec<Issue>;
    /// The changes against `doc`, the file as it was loaded.
    fn edits(&self, doc: &XmlDoc) -> Vec<Edit>;
    /// Draws the form.
    fn ui(&mut self, ui: &mut egui::Ui);
}

/// The text of the file with the form's changes in it.
pub fn render(form: &dyn XmlForm, original: &str) -> Result<String, String> {
    let doc = XmlDoc::parse(original).map_err(|e| e.to_string())?;
    apply_edits(original, form.edits(&doc)).map_err(|e| e.to_string())
}

/// The heading of the group in the topic list.
pub const GROUP_TITLE: &str = "XML configs";

/// The files that have a form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TopicId {
    AccessLevels,
    AdminCommands,
    ClassMaster,
    DynamicExp,
    Siege,
    BotReport,
    SecondaryAuth,
    Scripts,
    IpTemplate,
    IpConfig,
}

impl TopicId {
    pub const ALL: [TopicId; 10] = [
        TopicId::AccessLevels,
        TopicId::AdminCommands,
        TopicId::ClassMaster,
        TopicId::DynamicExp,
        TopicId::Siege,
        TopicId::BotReport,
        TopicId::SecondaryAuth,
        TopicId::Scripts,
        TopicId::IpTemplate,
        TopicId::IpConfig,
    ];

    pub fn file(self) -> &'static str {
        match self {
            TopicId::AccessLevels => access_levels::FILE,
            TopicId::AdminCommands => admin_commands::FILE,
            TopicId::ClassMaster => class_master::FILE,
            TopicId::DynamicExp => dynamic_exp::FILE,
            TopicId::Siege => siege::FILE,
            TopicId::BotReport => bot_report::FILE,
            TopicId::SecondaryAuth => secondary_auth::FILE,
            TopicId::Scripts => scripts::FILE,
            TopicId::IpTemplate => ipconfig::TEMPLATE_FILE,
            TopicId::IpConfig => ipconfig::FILE,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            TopicId::AccessLevels => "Access levels",
            TopicId::AdminCommands => "Admin commands",
            TopicId::ClassMaster => "Class master",
            TopicId::DynamicExp => "Dynamic exp rates",
            TopicId::Siege => "Siege schedule",
            TopicId::BotReport => "Bot report punishments",
            TopicId::SecondaryAuth => "Secondary auth",
            TopicId::Scripts => "Scripts (read-only)",
            TopicId::IpTemplate => "IP config (template)",
            TopicId::IpConfig => "IP config (active)",
        }
    }

    /// Files that may be missing without being an error (the server has defaults).
    fn optional(self) -> bool {
        self == TopicId::IpConfig
    }
}

/// A form of any topic.
pub enum Form {
    AccessLevels(access_levels::AccessLevelsForm),
    AdminCommands(admin_commands::AdminCommandsForm),
    ClassMaster(class_master::ClassMasterForm),
    DynamicExp(dynamic_exp::DynamicExpForm),
    Siege(siege::SiegeForm),
    BotReport(bot_report::BotReportForm),
    SecondaryAuth(Box<secondary_auth::SecondaryAuthForm>),
    Scripts(scripts::ScriptsForm),
    IpConfig(ipconfig::IpConfigForm),
}

impl Form {
    pub fn load(topic: TopicId, text: &str) -> Result<Self, String> {
        Ok(match topic {
            TopicId::AccessLevels => Form::AccessLevels(access_levels::AccessLevelsForm::load(text)?),
            TopicId::AdminCommands => Form::AdminCommands(admin_commands::AdminCommandsForm::load(text)?),
            TopicId::ClassMaster => Form::ClassMaster(class_master::ClassMasterForm::load(text)?),
            TopicId::DynamicExp => Form::DynamicExp(dynamic_exp::DynamicExpForm::load(text)?),
            TopicId::Siege => Form::Siege(siege::SiegeForm::load(text)?),
            TopicId::BotReport => Form::BotReport(bot_report::BotReportForm::load(text)?),
            TopicId::SecondaryAuth => {
                Form::SecondaryAuth(Box::new(secondary_auth::SecondaryAuthForm::load(text)?))
            }
            TopicId::Scripts => Form::Scripts(scripts::ScriptsForm::load(text)?),
            TopicId::IpTemplate | TopicId::IpConfig => Form::IpConfig(ipconfig::IpConfigForm::load(text)?),
        })
    }

    pub fn as_form(&self) -> &dyn XmlForm {
        match self {
            Form::AccessLevels(f) => f,
            Form::AdminCommands(f) => f,
            Form::ClassMaster(f) => f,
            Form::DynamicExp(f) => f,
            Form::Siege(f) => f,
            Form::BotReport(f) => f,
            Form::SecondaryAuth(f) => f.as_ref(),
            Form::Scripts(f) => f,
            Form::IpConfig(f) => f,
        }
    }

    pub fn as_form_mut(&mut self) -> &mut dyn XmlForm {
        match self {
            Form::AccessLevels(f) => f,
            Form::AdminCommands(f) => f,
            Form::ClassMaster(f) => f,
            Form::DynamicExp(f) => f,
            Form::Siege(f) => f,
            Form::BotReport(f) => f,
            Form::SecondaryAuth(f) => f.as_mut(),
            Form::Scripts(f) => f,
            Form::IpConfig(f) => f,
        }
    }
}

/// One loaded file with its form.
pub struct XmlFile {
    pub topic: TopicId,
    pub path: PathBuf,
    /// The text as loaded; the form's edits are relative to it.
    pub original: String,
    pub form: Form,
}

impl XmlFile {
    pub fn load(topic: TopicId, path: &Path) -> Result<Self, String> {
        let name = topic.file();
        let bytes = std::fs::read(path).map_err(|e| format!("{name}: {e}"))?;
        let original = String::from_utf8(bytes).map_err(|_| format!("{name}: not valid UTF-8"))?;
        let form = Form::load(topic, &original).map_err(|e| format!("{name}: {e}"))?;
        Ok(Self {
            topic,
            path: path.to_path_buf(),
            original,
            form,
        })
    }

    pub fn is_dirty(&self) -> bool {
        self.form.as_form().is_dirty()
    }

    pub fn issues(&self) -> Vec<Issue> {
        self.form.as_form().issues()
    }

    /// Puts the form back to the file as loaded.
    pub fn discard(&mut self) {
        if let Ok(form) = Form::load(self.topic, &self.original) {
            self.form = form;
        }
    }

    /// Writes the changes. `Ok(false)` when there was nothing to write.
    pub fn save(&self) -> Result<bool, String> {
        let name = self.topic.file();
        let new_text = render(self.form.as_form(), &self.original).map_err(|e| format!("{name}: {e}"))?;
        if new_text == self.original {
            return Ok(false);
        }
        // The edits are byte ranges of the text we loaded: they are only valid if the file is
        // still that text.
        let on_disk = std::fs::read(&self.path).map_err(|e| format!("{name}: {e}"))?;
        if on_disk != self.original.as_bytes() {
            return Err(format!(
                "{name}: the file changed on disk since it was loaded; press Reload"
            ));
        }
        write_atomic(&self.path, &new_text).map_err(|e| format!("{name}: {e}"))?;
        Ok(true)
    }
}

pub struct XmlConfigView {
    run_dir: String,
    files: Vec<XmlFile>,
    selected: usize,
    errors: Vec<String>,
    notes: Vec<String>,
}

impl XmlConfigView {
    pub fn with_run_dir(run_dir: &str) -> Self {
        Self {
            run_dir: run_dir.to_owned(),
            files: Vec::new(),
            selected: 0,
            errors: Vec::new(),
            notes: Vec::new(),
        }
    }

    /// Follows the run folder of the config editor. The files are read again by [`Self::reload`].
    pub fn set_run_dir(&mut self, run_dir: &str) {
        self.run_dir = run_dir.to_owned();
    }

    fn config_dir(&self) -> PathBuf {
        Path::new(self.run_dir.trim()).join(CONFIG_SUBDIR)
    }

    /// Reads every topic's file again. Unsaved edits and the notes are dropped.
    pub fn reload(&mut self) {
        let keep = self.files.get(self.selected).map(|f| f.topic);
        self.files.clear();
        self.errors.clear();
        self.notes.clear();
        let dir = self.config_dir();
        for topic in TopicId::ALL {
            let path = dir.join(topic.file());
            if !path.is_file() {
                if !topic.optional() {
                    self.errors
                        .push(format!("{}: not found in {}", topic.file(), dir.display()));
                }
                continue;
            }
            match XmlFile::load(topic, &path) {
                Ok(file) => self.files.push(file),
                Err(e) => self.errors.push(e),
            }
        }
        self.selected = keep
            .and_then(|t| self.files.iter().position(|f| f.topic == t))
            .unwrap_or(0);
    }

    /// Has any XML file been found (so the group has something to show)?
    pub fn has_files(&self) -> bool {
        !self.files.is_empty()
    }

    #[cfg(test)]
    pub fn files(&self) -> &[XmlFile] {
        &self.files
    }

    #[cfg(test)]
    pub fn files_mut(&mut self) -> &mut [XmlFile] {
        &mut self.files
    }

    #[cfg(test)]
    pub fn errors(&self) -> &[String] {
        &self.errors
    }

    #[cfg(test)]
    pub fn notes(&self) -> &[String] {
        &self.notes
    }

    /// The topic whose form [`Self::form_ui`] shows.
    #[cfg(test)]
    pub fn selected_topic(&self) -> Option<TopicId> {
        self.files.get(self.selected).map(|f| f.topic)
    }

    /// Chooses the topic whose form [`Self::form_ui`] shows; false when its file was not read.
    #[cfg(test)]
    pub fn select(&mut self, topic: TopicId) -> bool {
        match self.files.iter().position(|f| f.topic == topic) {
            Some(i) => {
                self.selected = i;
                true
            }
            None => false,
        }
    }

    pub fn dirty_files(&self) -> usize {
        self.files.iter().filter(|f| f.is_dirty()).count()
    }

    /// Hands the access levels (as edited) to the admin commands form, for its drop-down.
    fn sync_levels(&mut self) {
        let levels = self.files.iter().find_map(|f| match &f.form {
            Form::AccessLevels(a) => Some(a.level_list()),
            _ => None,
        });
        if let Some(levels) = levels {
            for file in &mut self.files {
                if let Form::AdminCommands(a) = &mut file.form {
                    a.set_levels(levels.clone());
                }
            }
        }
    }

    /// Saves every changed file when none of them has an error. Sets the notes.
    pub fn save_all(&mut self) {
        self.sync_levels();
        let mut problems = Vec::new();
        for file in self.files.iter().filter(|f| f.is_dirty()) {
            let issues = file.issues();
            if has_errors(&issues) {
                problems.extend(
                    issues
                        .iter()
                        .filter(|i| !i.warning)
                        .map(|i| format!("{}: {} {}", file.topic.file(), i.field, i.message)),
                );
            }
        }
        if !problems.is_empty() {
            self.notes = problems;
            return;
        }
        let mut saved = Vec::new();
        let mut notes = Vec::new();
        for (i, file) in self.files.iter().enumerate().filter(|(_, f)| f.is_dirty()) {
            match file.save() {
                Ok(true) => saved.push(i),
                Ok(false) => {}
                Err(e) => notes.push(format!("Not saved: {e}")),
            }
        }
        // Read the saved files again (the edits are relative to the text that was loaded). A file
        // that failed to save keeps its edits.
        let mut names = Vec::new();
        for i in saved {
            let (topic, path) = (self.files[i].topic, self.files[i].path.clone());
            names.push(topic.file());
            match XmlFile::load(topic, &path) {
                Ok(file) => self.files[i] = file,
                Err(e) => notes.push(format!("Saved, but cannot read it again: {e}")),
            }
        }
        if !names.is_empty() {
            notes.push(restart_note(&names));
        }
        self.notes = notes;
    }

    /// Copies the saved `default-ipconfig.xml` to `ipconfig.xml`, which makes the server use it.
    pub fn create_ipconfig(&mut self) {
        let dir = self.config_dir();
        let (from, to) = (dir.join(ipconfig::TEMPLATE_FILE), dir.join(ipconfig::FILE));
        let note = if to.exists() {
            format!("{} exists already", ipconfig::FILE)
        } else {
            match std::fs::copy(&from, &to) {
                Ok(_) => format!("Created {}. Restart the game server to use it.", ipconfig::FILE),
                Err(e) => format!("Could not create {}: {e}", ipconfig::FILE),
            }
        };
        self.reload();
        self.notes = vec![note];
    }

    /// Does the search (`query`, lower case) find the topic? By its title or its file name; an
    /// empty query and the group's own name find every topic.
    fn topic_matches(topic: TopicId, query: &str) -> bool {
        query.is_empty()
            || GROUP_TITLE.to_lowercase().contains(query)
            || topic.title().to_lowercase().contains(query)
            || topic.file().to_lowercase().contains(query)
    }

    /// How many topics the search finds.
    pub fn hits(&self, query: &str) -> usize {
        self.files
            .iter()
            .filter(|f| Self::topic_matches(f.topic, query))
            .count()
    }

    /// The group in the topic list (left side), under the heading [`GROUP_TITLE`]: the topics that
    /// match `query` (lower case). `active` = the form of this group is on screen, so the chosen
    /// topic is drawn selected. Returns true when the user clicked a topic.
    pub fn topic_list(&mut self, ui: &mut egui::Ui, active: bool, query: &str) -> bool {
        if self.hits(query) == 0 {
            return false;
        }
        ui.strong(GROUP_TITLE);
        let mut clicked = false;
        for (i, file) in self.files.iter().enumerate() {
            if !Self::topic_matches(file.topic, query) {
                continue;
            }
            let mut label = file.topic.title().to_owned();
            if file.is_dirty() {
                label.push_str(" *");
            }
            if has_errors(&file.issues()) {
                label.push_str(" !");
            }
            let response = ui
                .selectable_label(active && self.selected == i, label)
                .on_hover_text(file.topic.file());
            if response.clicked() {
                self.selected = i;
                clicked = true;
            }
        }
        clicked
    }

    /// The form of the selected topic (right side).
    pub fn form_ui(&mut self, ui: &mut egui::Ui) {
        self.sync_levels();
        let mut create = false;
        let ipconfig_exists = self.files.iter().any(|f| f.topic == TopicId::IpConfig);
        if let Some(file) = self.files.get_mut(self.selected) {
            ui.heading(file.topic.title());
            ui.weak(format!(
                "{} in {}",
                file.topic.file(),
                file.path
                    .parent()
                    .map_or_else(String::new, |p| p.display().to_string())
            ));
            if file.topic == TopicId::IpTemplate {
                ui.horizontal(|ui| {
                    ui.label("The servers only read ipconfig.xml; this file is the template.");
                    if !ipconfig_exists && ui.button("Create ipconfig.xml from the saved file").clicked() {
                        create = true;
                    }
                });
            }
            ui.separator();
            file.form.as_form_mut().ui(ui);
        }
        if create {
            self.create_ipconfig();
        }
    }

    /// The save and discard buttons of the XML files, the notes of the last save and the files that
    /// could not be read. Draws nothing when no XML file was found.
    pub fn actions_ui(&mut self, ui: &mut egui::Ui) {
        if !self.has_files() && self.errors.is_empty() {
            return;
        }
        let dirty = self.dirty_files();
        ui.horizontal_wrapped(|ui| {
            ui.label(GROUP_TITLE);
            if ui
                .add_enabled(
                    dirty > 0,
                    egui::Button::new(format!("Save XML configs ({dirty} files)")),
                )
                .clicked()
            {
                self.save_all();
            }
            if ui
                .add_enabled(dirty > 0, egui::Button::new("Discard XML edits"))
                .clicked()
            {
                self.files.iter_mut().for_each(XmlFile::discard);
            }
        });
        for note in &self.notes {
            ui.colored_label(ui.visuals().warn_fg_color, note.as_str());
        }
        for error in &self.errors {
            ui.colored_label(ui.visuals().error_fg_color, error.as_str());
        }
    }

    /// The old stand-alone page, for the tests: the buttons, the list and the form side by side.
    #[cfg(test)]
    pub fn show(&mut self, ui: &mut egui::Ui) {
        if self.files.is_empty() && self.errors.is_empty() {
            self.reload();
        }
        self.actions_ui(ui);
        ui.separator();
        if self.files.is_empty() {
            return;
        }
        let height = ui.available_height();
        ui.horizontal_top(|ui| {
            ui.allocate_ui(egui::vec2(210.0, height), |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("xml_topics")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        self.topic_list(ui, true, "");
                    });
            });
            ui.separator();
            ui.allocate_ui(egui::vec2(ui.available_width(), height), |ui| self.form_ui(ui));
        });
    }
}

/// "Restart the game server to apply: ..." for the saved files.
pub fn restart_note(saved: &[&str]) -> String {
    format!("Restart the game server to apply: {}", saved.join(", "))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
