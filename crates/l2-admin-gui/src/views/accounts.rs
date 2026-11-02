//! Accounts view: search, create, change the password or access level, ban, unban and delete
//! accounts. The login server's admin endpoint does the work (it owns the `accounts` table); the
//! supervisor relays `Request::Accounts` to it (G-04).
//!
//! Java: `tools/AccountManager.java` is the spec for what each action writes. The game server's
//! `//ban_acc` is a different thing and is explained next to the ban button.
//!
//! Passwords: the form fields are moved into the request and replaced by empty strings, so the
//! view keeps nothing after sending. They are never shown again and never logged.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use l2_admin_api::{
    ACCOUNTS_PAGE_SIZE, AccountPage, AccountSummary, AccountsAction, AccountsResult, Request, Response,
};

use crate::app::AppCtx;
use crate::bridge::BridgeEvent;
use crate::views::View;
use crate::views::players::format_utc;

/// The supervisor's name for the login server (`Server.login.*` in `supervisor.ini`).
const LOGIN_SERVER: &str = "login";
/// Where the game server keeps its access level names, relative to the datapack folder.
const ACCESS_LEVELS_FILE: &str = "config/AccessLevels.xml";

/// Access level numbers with the names of the game server's `AccessLevels.xml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessLevelNames {
    entries: Vec<(i32, String)>,
}

impl Default for AccessLevelNames {
    /// The levels the shipped `AccessLevels.xml` defines (also the ones Java's AccountManager
    /// offers), for when the file is not at hand.
    fn default() -> Self {
        let entries = [
            (-1, "Banned"),
            (0, "User"),
            (10, "Chat Moderator"),
            (20, "Test GM"),
            (30, "General GM"),
            (40, "Support GM"),
            (50, "Event GM"),
            (60, "Head GM"),
            (70, "Admin"),
            (100, "Master"),
        ];
        Self {
            entries: entries.iter().map(|(l, n)| (*l, (*n).to_owned())).collect(),
        }
    }
}

impl AccessLevelNames {
    /// Reads the `<access level=".." name=".."/>` entries. `None` when there are none.
    pub fn parse(xml: &str) -> Option<Self> {
        let xml = strip_comments(xml);
        let mut entries = Vec::new();
        for tag in xml.split("<access ").skip(1) {
            let tag = tag.split('>').next().unwrap_or("");
            let level = attribute(tag, "level").and_then(|v| v.trim().parse::<i32>().ok());
            if let (Some(level), Some(name)) = (level, attribute(tag, "name")) {
                entries.push((level, name.to_owned()));
            }
        }
        (!entries.is_empty()).then_some(Self { entries })
    }

    /// `100 - Master`; a level without a name is just the number.
    pub fn label(&self, level: i32) -> String {
        match self.name(level) {
            Some(name) => format!("{level} - {name}"),
            None => level.to_string(),
        }
    }

    pub fn name(&self, level: i32) -> Option<&str> {
        self.entries
            .iter()
            .find(|(l, _)| *l == level)
            .map(|(_, n)| n.as_str())
    }

    pub fn entries(&self) -> &[(i32, String)] {
        &self.entries
    }
}

fn strip_comments(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len());
    let mut rest = xml;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        match rest[start..].find("-->") {
            Some(end) => rest = &rest[start + end + 3..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// Value of `name="..."` inside one tag.
fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let key = format!("{name}=\"");
    // Match whole attribute names only (`nameColor` must not answer for `name`).
    let mut from = 0;
    while let Some(pos) = tag[from..].find(&key) {
        let at = from + pos;
        let boundary = at == 0 || tag[..at].ends_with(char::is_whitespace);
        if boundary {
            let value = &tag[at + key.len()..];
            return value.split('"').next();
        }
        from = at + key.len();
    }
    None
}

/// `<datapack>/config/AccessLevels.xml` for the data browser's datapack folder setting.
pub fn access_levels_path(datapack_path: &str) -> Option<PathBuf> {
    let trimmed = datapack_path.trim().trim_matches('"').trim();
    (!trimmed.is_empty()).then(|| Path::new(trimmed).join(ACCESS_LEVELS_FILE))
}

/// What an in-flight request was for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pending {
    Search,
    LastIp(String),
    /// Changes something; the list is refreshed when it succeeds.
    Change,
}

/// Where the two-step delete is.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum DeleteStep {
    #[default]
    Idle,
    /// First click done, waiting for the second.
    Asked(String),
    /// Second click done, waiting for the final one.
    Sure(String),
}

/// A message under the form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub ok: bool,
    pub text: String,
}

#[derive(Default)]
pub struct AccountsView {
    // Search and list.
    prefix: String,
    page: u32,
    data: Option<AccountPage>,
    last_ips: HashMap<String, Option<String>>,
    selected: Option<String>,
    // Forms.
    create_login: String,
    create_password: String,
    create_repeat: String,
    create_level: i32,
    new_password: String,
    new_repeat: String,
    new_level: i32,
    delete: DeleteStep,
    // State.
    names: AccessLevelNames,
    names_path: String,
    names_source: Option<String>,
    names_loaded_for: Option<String>,
    pending: HashMap<u64, Pending>,
    refresh: bool,
    notice: Option<Notice>,
}

impl AccountsView {
    fn error(&mut self, text: impl Into<String>) {
        self.notice = Some(Notice {
            ok: false,
            text: text.into(),
        });
    }

    fn request(action: AccountsAction) -> Request {
        Request::Accounts {
            server: Some(LOGIN_SERVER.to_owned()),
            action,
        }
    }

    /// The search for the current prefix and page.
    pub fn search_request(&self) -> (Request, Pending) {
        let action = AccountsAction::Search {
            prefix: self.prefix.clone(),
            page: self.page,
        };
        (Self::request(action), Pending::Search)
    }

    /// Starts a new search from the first page.
    pub fn search_from_start(&mut self) {
        self.page = 0;
        self.refresh = true;
    }

    /// The create form as a request. Checks that both password fields match and moves the
    /// passwords out of the form.
    pub fn create_request(&mut self) -> Result<(Request, Pending), String> {
        let login = self.create_login.trim().to_owned();
        if login.is_empty() {
            return Err("Enter a login.".into());
        }
        if self.create_password.trim().is_empty() {
            return Err("Enter a password.".into());
        }
        if self.create_password != self.create_repeat {
            return Err("The two passwords differ.".into());
        }
        let password = std::mem::take(&mut self.create_password);
        self.create_repeat = String::new();
        let action = AccountsAction::Create {
            login,
            password,
            access_level: self.create_level,
        };
        Ok((Self::request(action), Pending::Change))
    }

    /// The change-password form for the selected account.
    pub fn password_request(&mut self) -> Result<(Request, Pending), String> {
        let Some(login) = self.selected.clone() else {
            return Err("Select an account first.".into());
        };
        if self.new_password.trim().is_empty() {
            return Err("Enter the new password.".into());
        }
        if self.new_password != self.new_repeat {
            return Err("The two passwords differ.".into());
        }
        let password = std::mem::take(&mut self.new_password);
        self.new_repeat = String::new();
        Ok((
            Self::request(AccountsAction::SetPassword { login, password }),
            Pending::Change,
        ))
    }

    pub fn level_request(&self) -> Result<(Request, Pending), String> {
        let Some(login) = self.selected.clone() else {
            return Err("Select an account first.".into());
        };
        let action = AccountsAction::SetAccessLevel {
            login,
            access_level: self.new_level,
        };
        Ok((Self::request(action), Pending::Change))
    }

    pub fn ban_request(&self, ban: bool) -> Result<(Request, Pending), String> {
        let Some(login) = self.selected.clone() else {
            return Err("Select an account first.".into());
        };
        let action = if ban {
            AccountsAction::Ban { login }
        } else {
            AccountsAction::Unban { login }
        };
        Ok((Self::request(action), Pending::Change))
    }

    pub fn last_ip_request(login: &str) -> (Request, Pending) {
        (
            Self::request(AccountsAction::LastIp {
                login: login.to_owned(),
            }),
            Pending::LastIp(login.to_owned()),
        )
    }

    /// One click on the delete button. Asks twice before it returns the request.
    pub fn delete_click(&mut self) -> Option<(Request, Pending)> {
        let login = self.selected.clone()?;
        match std::mem::take(&mut self.delete) {
            DeleteStep::Idle => {
                self.delete = DeleteStep::Asked(login);
                None
            }
            DeleteStep::Asked(asked) if asked == login => {
                self.delete = DeleteStep::Sure(login);
                None
            }
            DeleteStep::Sure(sure) if sure == login => {
                Some((Self::request(AccountsAction::Delete { login }), Pending::Change))
            }
            // The selection moved since the first click: start over.
            _ => {
                self.delete = DeleteStep::Asked(login);
                None
            }
        }
    }

    pub fn select(&mut self, login: Option<String>) {
        if self.selected != login {
            self.delete = DeleteStep::Idle;
            self.new_password = String::new();
            self.new_repeat = String::new();
            if let Some(account) = login.as_deref().and_then(|l| self.account(l)) {
                self.new_level = account.access_level;
            }
            self.selected = login;
        }
    }

    fn account(&self, login: &str) -> Option<&AccountSummary> {
        self.data.as_ref()?.accounts.iter().find(|a| a.login == login)
    }

    pub fn page_count(&self) -> u32 {
        let total = self.data.as_ref().map_or(0, |d| d.total);
        let per_page = u64::from(ACCOUNTS_PAGE_SIZE);
        u32::try_from(total.div_ceil(per_page)).unwrap_or(u32::MAX).max(1)
    }

    fn track(&mut self, cx: &AppCtx<'_>, (request, kind): (Request, Pending)) {
        let id = cx.bridge.request(request);
        self.pending.insert(id, kind);
    }

    fn reset(&mut self) {
        self.pending.clear();
        self.data = None;
        self.last_ips.clear();
        self.delete = DeleteStep::Idle;
    }

    /// Folds one bridge event into the view.
    pub fn apply(&mut self, event: &BridgeEvent) {
        match event {
            BridgeEvent::Connecting { .. }
            | BridgeEvent::Waiting { .. }
            | BridgeEvent::Disconnected
            | BridgeEvent::Rejected { .. } => self.reset(),
            BridgeEvent::Connected { .. } => {
                self.reset();
                self.refresh = true;
            }
            BridgeEvent::Event(_) => {}
            BridgeEvent::Response { id, result } => {
                let Some(kind) = self.pending.remove(id) else {
                    return;
                };
                match result {
                    Ok(Response::Accounts { result }) => self.apply_result(kind, result),
                    Ok(Response::Error { message }) => self.error(message.clone()),
                    Ok(other) => self.error(format!("unexpected answer: {other:?}")),
                    Err(error) => self.error(error.clone()),
                }
            }
        }
    }

    fn apply_result(&mut self, kind: Pending, result: &AccountsResult) {
        match result {
            AccountsResult::Page(page) => {
                self.page = page.page;
                self.data = Some(page.clone());
                self.last_ips.clear();
                // A deleted account stays selected otherwise.
                if let Some(login) = self.selected.clone()
                    && self.account(&login).is_none()
                {
                    self.selected = None;
                    self.delete = DeleteStep::Idle;
                }
            }
            AccountsResult::LastIp { login, ip } => {
                self.last_ips.insert(login.clone(), ip.clone());
            }
            AccountsResult::Done { login, access_level } => {
                let text = match access_level {
                    Some(level) => format!("Done for '{login}': access level {}.", self.names.label(*level)),
                    None => format!("Done for '{login}'."),
                };
                self.notice = Some(Notice { ok: true, text });
                self.delete = DeleteStep::Idle;
                if kind == Pending::Change {
                    self.refresh = true;
                }
            }
        }
    }

    /// Reads the names file once per path.
    fn load_names(&mut self, datapack_path: &str) {
        let path = if self.names_path.trim().is_empty() {
            access_levels_path(datapack_path)
        } else {
            Some(PathBuf::from(self.names_path.trim().trim_matches('"')))
        };
        let key = path.as_ref().map(|p| p.display().to_string());
        if key == self.names_loaded_for {
            return;
        }
        self.names_loaded_for = key;
        let Some(path) = path else {
            self.names = AccessLevelNames::default();
            self.names_source = None;
            return;
        };
        match std::fs::read_to_string(&path)
            .ok()
            .and_then(|xml| AccessLevelNames::parse(&xml))
        {
            Some(names) => {
                self.names = names;
                self.names_source = Some(path.display().to_string());
            }
            None => {
                self.names = AccessLevelNames::default();
                self.names_source = None;
            }
        }
    }

    fn level_picker(ui: &mut egui::Ui, id: &str, names: &AccessLevelNames, level: &mut i32) {
        egui::ComboBox::from_id_salt(id)
            .selected_text(names.label(*level))
            .show_ui(ui, |ui| {
                for (value, _) in names.entries() {
                    ui.selectable_value(level, *value, names.label(*value));
                }
            });
        ui.add(egui::DragValue::new(level).range(-128..=127));
    }
}

impl View for AccountsView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut AppCtx<'_>) {
        let connected = cx.conn.is_connected();
        self.load_names(&cx.settings.datapack_path);
        if connected && self.refresh {
            self.refresh = false;
            let search = self.search_request();
            self.track(cx, search);
        }

        ui.heading("Accounts");
        ui.add_space(4.0);
        if !connected {
            ui.colored_label(ui.visuals().warn_fg_color, "Not connected to the supervisor.");
        }
        if let Some(notice) = &self.notice {
            let color = if notice.ok {
                ui.visuals().text_color()
            } else {
                ui.visuals().error_fg_color
            };
            ui.colored_label(color, &notice.text);
        }
        ui.add_space(4.0);

        let mut sends: Vec<(Request, Pending)> = Vec::new();
        let mut failure: Option<String> = None;

        // Search.
        ui.horizontal(|ui| {
            ui.label("Login starts with:");
            let edit = ui.add(egui::TextEdit::singleline(&mut self.prefix).desired_width(180.0));
            let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if (ui.add_enabled(connected, egui::Button::new("Search")).clicked() || enter) && connected {
                self.search_from_start();
            }
        });
        ui.collapsing("Access level names", |ui| {
            ui.horizontal(|ui| {
                ui.label("AccessLevels.xml:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.names_path)
                        .hint_text("empty = <datapack>/config/AccessLevels.xml")
                        .desired_width(380.0),
                );
            });
            match &self.names_source {
                Some(path) => ui.label(format!("Names from {path}")),
                None => ui.label("File not found: the built-in names of the shipped file are used."),
            };
        });
        ui.add_space(6.0);

        // List.
        let mut clicked: Option<String> = None;
        let mut want_ip: Option<String> = None;
        let mut goto: Option<u32> = None;
        match &self.data {
            None => {
                ui.label(if connected {
                    "Waiting for the login server..."
                } else {
                    "No account list."
                });
            }
            Some(data) if data.accounts.is_empty() => {
                ui.label("No account matches.");
            }
            Some(data) => {
                ui.label(format!(
                    "{} account(s), page {} of {}",
                    data.total,
                    data.page + 1,
                    self.page_count()
                ));
                egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                    egui::Grid::new("accounts_table")
                        .striped(true)
                        .spacing([16.0, 6.0])
                        .show(ui, |ui| {
                            for header in [
                                "Login",
                                "Access level",
                                "Last active",
                                "Last server",
                                "Banned",
                                "Last IP",
                            ] {
                                ui.strong(header);
                            }
                            ui.end_row();
                            for account in &data.accounts {
                                let selected = self.selected.as_deref() == Some(account.login.as_str());
                                if ui.selectable_label(selected, account.login.as_str()).clicked() {
                                    clicked = Some(account.login.clone());
                                }
                                ui.label(self.names.label(account.access_level));
                                ui.label(if account.last_active_ms == 0 {
                                    "never".to_owned()
                                } else {
                                    format_utc(account.last_active_ms)
                                });
                                ui.label(account.last_server.map_or("-".to_owned(), |s| s.to_string()));
                                ui.label(if account.banned { "yes" } else { "no" });
                                match self.last_ips.get(&account.login) {
                                    Some(Some(ip)) => {
                                        ui.label(ip.as_str());
                                    }
                                    Some(None) => {
                                        ui.label("never logged in");
                                    }
                                    None => {
                                        if ui.add_enabled(connected, egui::Button::new("Show")).clicked() {
                                            want_ip = Some(account.login.clone());
                                        }
                                    }
                                }
                                ui.end_row();
                            }
                        });
                });
                ui.horizontal(|ui| {
                    let last = self.page_count() - 1;
                    if ui
                        .add_enabled(connected && data.page > 0, egui::Button::new("Previous"))
                        .clicked()
                    {
                        goto = Some(data.page - 1);
                    }
                    if ui
                        .add_enabled(connected && data.page < last, egui::Button::new("Next"))
                        .clicked()
                    {
                        goto = Some(data.page + 1);
                    }
                });
            }
        }
        if let Some(login) = clicked {
            self.select(Some(login));
        }
        if let Some(login) = want_ip {
            sends.push(Self::last_ip_request(&login));
        }
        if let Some(page) = goto {
            self.page = page;
            self.refresh = true;
        }

        ui.separator();

        // Actions on the selected account.
        match self.selected.clone() {
            None => {
                ui.label("Select an account in the list to change, ban or delete it.");
            }
            Some(login) => {
                ui.strong(format!("Selected: {login}"));
                let names = self.names.clone();
                egui::Grid::new("account_actions")
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("New password:");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.new_password)
                                .password(true)
                                .desired_width(160.0),
                        );
                        ui.end_row();
                        ui.label("Repeat:");
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut self.new_repeat)
                                    .password(true)
                                    .desired_width(160.0),
                            );
                            if ui
                                .add_enabled(connected, egui::Button::new("Change password"))
                                .clicked()
                            {
                                match self.password_request() {
                                    Ok(send) => sends.push(send),
                                    Err(e) => failure = Some(e),
                                }
                            }
                        });
                        ui.end_row();
                        ui.label("Access level:");
                        ui.horizontal(|ui| {
                            Self::level_picker(ui, "set_level", &names, &mut self.new_level);
                            if ui.add_enabled(connected, egui::Button::new("Set")).clicked() {
                                match self.level_request() {
                                    Ok(send) => sends.push(send),
                                    Err(e) => failure = Some(e),
                                }
                            }
                        });
                        ui.end_row();
                    });
                ui.horizontal(|ui| {
                    if ui.add_enabled(connected, egui::Button::new("Ban")).clicked()
                        && let Ok(send) = self.ban_request(true)
                    {
                        sends.push(send);
                    }
                    if ui.add_enabled(connected, egui::Button::new("Unban")).clicked()
                        && let Ok(send) = self.ban_request(false)
                    {
                        sends.push(send);
                    }
                    let label = match &self.delete {
                        DeleteStep::Idle => "Delete...",
                        DeleteStep::Asked(_) => "Really delete? (click again)",
                        DeleteStep::Sure(_) => "Yes, delete this account for good",
                    };
                    if ui.add_enabled(connected, egui::Button::new(label)).clicked()
                        && let Some(send) = self.delete_click()
                    {
                        sends.push(send);
                    }
                    if self.delete != DeleteStep::Idle && ui.button("Cancel").clicked() {
                        self.delete = DeleteStep::Idle;
                    }
                });
                ui.small(
                    "Ban here sets the account's access level to -1 on the login server, like Java's \
                     AccountManager. Unban sets it back to 0 (User) and only works on banned accounts. \
                     Delete removes only the account row, like AccountManager: its characters stay in \
                     the game database.",
                );
                ui.small(
                    "//ban_acc <account> in the game is different: it adds a row to the game \
                     database table `punishments` (affect ACCOUNT, type BAN, expiration 0 = permanent, \
                     reason \"Banned by admin\") and leaves accounts.accessLevel alone. //unban_acc removes that row.",
                );
            }
        }

        ui.separator();

        // Create.
        ui.strong("Create an account");
        let names = self.names.clone();
        egui::Grid::new("account_create")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label("Login:");
                ui.add(egui::TextEdit::singleline(&mut self.create_login).desired_width(180.0));
                ui.end_row();
                ui.label("Password:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.create_password)
                        .password(true)
                        .desired_width(180.0),
                );
                ui.end_row();
                ui.label("Repeat:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.create_repeat)
                        .password(true)
                        .desired_width(180.0),
                );
                ui.end_row();
                ui.label("Access level:");
                ui.horizontal(|ui| Self::level_picker(ui, "create_level", &names, &mut self.create_level));
                ui.end_row();
            });
        if ui.add_enabled(connected, egui::Button::new("Create")).clicked() {
            match self.create_request() {
                Ok(send) => sends.push(send),
                Err(e) => failure = Some(e),
            }
        }

        if let Some(text) = failure {
            self.error(text);
        }
        for send in sends {
            self.track(cx, send);
        }
    }

    fn on_event(&mut self, event: &BridgeEvent) {
        self.apply(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view_with(accounts: &[(&str, i32)]) -> AccountsView {
        let page = AccountPage {
            page: 0,
            page_size: 50,
            total: accounts.len() as u64,
            accounts: accounts
                .iter()
                .map(|(login, level)| AccountSummary {
                    login: (*login).into(),
                    access_level: *level,
                    last_active_ms: 0,
                    last_server: Some(1),
                    banned: *level < 0,
                })
                .collect(),
        };
        AccountsView {
            data: Some(page),
            ..Default::default()
        }
    }

    fn action(request: Request) -> AccountsAction {
        match request {
            Request::Accounts { server, action } => {
                assert_eq!(server.as_deref(), Some("login"));
                action
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn parses_the_shipped_file_and_labels_levels() {
        let xml = r#"<?xml version="1.0"?><list>
            <!-- <access level="999" name="Hidden" /> -->
            <access level="-1" name="Banned" nameColor="FFFFFF" />
            <access level="100" nameColor="FFFFFF" name="Master" />
        </list>"#;
        let names = AccessLevelNames::parse(xml).unwrap();
        assert_eq!(names.entries().len(), 2);
        assert_eq!(names.label(100), "100 - Master");
        assert_eq!(names.label(-1), "-1 - Banned");
        assert_eq!(names.label(5), "5");
        assert!(AccessLevelNames::parse("<list/>").is_none());
    }

    #[test]
    fn the_built_in_names_match_the_dist_file() {
        // The dist snapshot is committed, so this runs everywhere.
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/game/config/AccessLevels.xml");
        let xml = std::fs::read_to_string(path).unwrap();
        assert_eq!(
            AccessLevelNames::parse(&xml).unwrap(),
            AccessLevelNames::default()
        );
    }

    #[test]
    fn the_names_file_sits_in_the_datapack_config_folder() {
        assert_eq!(access_levels_path("  "), None);
        assert_eq!(
            access_levels_path("\"some/game\""),
            Some(Path::new("some/game").join("config/AccessLevels.xml"))
        );
    }

    #[test]
    fn create_checks_the_form_and_empties_the_passwords() {
        let mut view = AccountsView::default();
        assert!(view.create_request().is_err());
        view.create_login = " alice ".into();
        assert!(view.create_request().is_err(), "no password");
        view.create_password = "secret1".into();
        view.create_repeat = "secret2".into();
        assert_eq!(view.create_request().unwrap_err(), "The two passwords differ.");
        assert_eq!(
            view.create_password, "secret1",
            "a refused form keeps what was typed"
        );
        view.create_repeat = "secret1".into();
        view.create_level = 10;
        let (request, pending) = view.create_request().unwrap();
        assert_eq!(pending, Pending::Change);
        assert_eq!(
            action(request),
            AccountsAction::Create {
                login: "alice".into(),
                password: "secret1".into(),
                access_level: 10
            }
        );
        assert!(view.create_password.is_empty() && view.create_repeat.is_empty());
    }

    #[test]
    fn password_change_needs_a_selection_and_matching_fields() {
        let mut view = view_with(&[("alice", 0)]);
        assert!(view.password_request().is_err());
        view.select(Some("alice".into()));
        view.new_password = "a".into();
        view.new_repeat = "b".into();
        assert!(view.password_request().is_err());
        view.new_repeat = "a".into();
        let (request, _) = view.password_request().unwrap();
        assert_eq!(
            action(request),
            AccountsAction::SetPassword {
                login: "alice".into(),
                password: "a".into()
            }
        );
        assert!(view.new_password.is_empty() && view.new_repeat.is_empty());
    }

    #[test]
    fn level_ban_and_unban_target_the_selected_account() {
        let mut view = view_with(&[("alice", 30)]);
        assert!(view.ban_request(true).is_err());
        view.select(Some("alice".into()));
        assert_eq!(view.new_level, 30, "the picker starts at the current level");
        view.new_level = 70;
        assert_eq!(
            action(view.level_request().unwrap().0),
            AccountsAction::SetAccessLevel {
                login: "alice".into(),
                access_level: 70
            }
        );
        assert_eq!(
            action(view.ban_request(true).unwrap().0),
            AccountsAction::Ban {
                login: "alice".into()
            }
        );
        assert_eq!(
            action(view.ban_request(false).unwrap().0),
            AccountsAction::Unban {
                login: "alice".into()
            }
        );
    }

    #[test]
    fn delete_asks_twice_and_resets_when_the_selection_moves() {
        let mut view = view_with(&[("alice", 0), ("bob", 0)]);
        assert!(view.delete_click().is_none(), "nothing selected");
        view.select(Some("alice".into()));
        assert!(view.delete_click().is_none());
        assert_eq!(view.delete, DeleteStep::Asked("alice".into()));
        assert!(view.delete_click().is_none());
        assert_eq!(view.delete, DeleteStep::Sure("alice".into()));
        view.select(Some("bob".into()));
        assert_eq!(view.delete, DeleteStep::Idle);
        assert!(view.delete_click().is_none());
        assert!(view.delete_click().is_none());
        let (request, pending) = view.delete_click().expect("third click deletes");
        assert_eq!(pending, Pending::Change);
        assert_eq!(action(request), AccountsAction::Delete { login: "bob".into() });
        assert_eq!(view.delete, DeleteStep::Idle);
    }

    fn respond(view: &mut AccountsView, id: u64, response: Response) {
        view.apply(&BridgeEvent::Response {
            id,
            result: Ok(response),
        });
    }

    #[test]
    fn answers_update_the_list_the_notice_and_ask_for_a_refresh() {
        let mut view = AccountsView::default();
        view.pending.insert(1, Pending::Search);
        let page = AccountPage {
            page: 0,
            page_size: 50,
            total: 120,
            accounts: view_with(&[("alice", 0)]).data.unwrap().accounts,
        };
        respond(
            &mut view,
            1,
            Response::Accounts {
                result: AccountsResult::Page(page),
            },
        );
        assert_eq!(view.page_count(), 3);
        assert_eq!(view.account("alice").unwrap().access_level, 0);

        view.pending.insert(2, Pending::LastIp("alice".into()));
        respond(
            &mut view,
            2,
            Response::Accounts {
                result: AccountsResult::LastIp {
                    login: "alice".into(),
                    ip: Some("10.0.0.1".into()),
                },
            },
        );
        assert_eq!(view.last_ips["alice"].as_deref(), Some("10.0.0.1"));

        view.refresh = false;
        view.pending.insert(3, Pending::Change);
        respond(
            &mut view,
            3,
            Response::Accounts {
                result: AccountsResult::Done {
                    login: "alice".into(),
                    access_level: Some(-1),
                },
            },
        );
        assert!(view.refresh);
        let notice = view.notice.clone().unwrap();
        assert!(notice.ok && notice.text.contains("-1 - Banned"), "{notice:?}");

        // Errors become a red notice; answers nobody asked for are ignored.
        view.pending.insert(4, Pending::Change);
        respond(
            &mut view,
            4,
            Response::Error {
                message: "no account 'x'".into(),
            },
        );
        assert_eq!(
            view.notice,
            Some(Notice {
                ok: false,
                text: "no account 'x'".into()
            })
        );
        respond(&mut view, 99, Response::Ok);
    }

    #[test]
    fn a_deleted_account_leaves_the_selection() {
        let mut view = view_with(&[("alice", 0)]);
        view.select(Some("alice".into()));
        view.pending.insert(1, Pending::Search);
        respond(
            &mut view,
            1,
            Response::Accounts {
                result: AccountsResult::Page(AccountPage {
                    page: 0,
                    page_size: 50,
                    total: 0,
                    accounts: vec![],
                }),
            },
        );
        assert_eq!(view.selected, None);
    }

    #[test]
    fn a_new_connection_clears_the_list_and_asks_again() {
        let mut view = view_with(&[("alice", 0)]);
        view.apply(&BridgeEvent::Disconnected);
        assert!(view.data.is_none());
        view.apply(&BridgeEvent::Connected {
            name: "supervisor".into(),
        });
        assert!(view.refresh);
        assert_eq!(view.search_request().1, Pending::Search);
    }
}
