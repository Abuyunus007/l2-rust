//! Players view: the characters in the Rust game server's world, polled through the supervisor
//! (`GetGameStatus`, relayed to the game server's admin endpoint). Sortable by column, with a
//! text filter. G-03 adds the actions on the selected player (kick, jail, bans, teleport to a
//! town, private message), which run as the matching `//` commands on the game server.

use std::cmp::Ordering;
use std::net::IpAddr;
use std::time::{Duration, Instant};

use l2_admin_api::{GameAction, GamePlayer, PlayerAction, Request, Response, TOWNS};

use crate::app::AppCtx;
use crate::bridge::BridgeEvent;
use crate::views::View;
use crate::views::game_admin::{GAME_SERVER, game_request, reply_lines};
/// How often the list is asked for again while the view is open.
const REFRESH: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Column {
    Name,
    Account,
    Level,
    Class,
    Clan,
    Location,
    Ip,
    OnlineSince,
}

impl Column {
    pub const ALL: [Column; 8] = [
        Column::Name,
        Column::Account,
        Column::Level,
        Column::Class,
        Column::Clan,
        Column::Location,
        Column::Ip,
        Column::OnlineSince,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Column::Name => "Name",
            Column::Account => "Account",
            Column::Level => "Level",
            Column::Class => "Class",
            Column::Clan => "Clan",
            Column::Location => "Location",
            Column::Ip => "IP",
            Column::OnlineSince => "Online since",
        }
    }

    /// Ascending order of this column. Names compare case-insensitively, with the exact name as
    /// the tie-breaker so the order is stable. `None` IPs (not reported) sort first.
    fn compare(self, a: &GamePlayer, b: &GamePlayer) -> Ordering {
        match self {
            Column::Name => ci_cmp(&a.name, &b.name),
            Column::Account => ci_cmp(&a.account, &b.account),
            Column::Level => a.level.cmp(&b.level),
            Column::Class => a.class_id.cmp(&b.class_id),
            // No clan sorts first.
            Column::Clan => ci_cmp(a.clan.as_deref().unwrap_or(""), b.clan.as_deref().unwrap_or("")),
            Column::Location => (a.x, a.y, a.z).cmp(&(b.x, b.y, b.z)),
            Column::Ip => ip_key(a.ip.as_deref()).cmp(&ip_key(b.ip.as_deref())),
            Column::OnlineSince => a.online_since_ms.cmp(&b.online_since_ms),
        }
    }
}

/// Sort key of an IP: not reported first, then addresses by value (so `10.0.0.2` comes before
/// `10.0.0.10`), then anything that does not parse as text. A total order, which `sort_by` needs.
fn ip_key(ip: Option<&str>) -> (u8, Option<IpAddr>, &str) {
    match ip {
        None => (0, None, ""),
        Some(text) => match text.parse::<IpAddr>() {
            Ok(addr) => (1, Some(addr), text),
            Err(_) => (2, None, text),
        },
    }
}

fn ci_cmp(a: &str, b: &str) -> Ordering {
    a.to_lowercase().cmp(&b.to_lowercase()).then_with(|| a.cmp(b))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sort {
    pub column: Column,
    pub descending: bool,
}

impl Default for Sort {
    /// The server sends the list sorted by name, so the view starts that way too.
    fn default() -> Self {
        Sort {
            column: Column::Name,
            descending: false,
        }
    }
}

/// The table behind the view: the last list from the server, a sort and a text filter.
/// No egui here, so the rules are tested directly.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PlayerTable {
    players: Vec<GamePlayer>,
    sort: Sort,
    filter: String,
}

impl PlayerTable {
    pub fn set_players(&mut self, players: Vec<GamePlayer>) {
        self.players = players;
    }

    pub fn clear(&mut self) {
        self.players.clear();
    }

    /// Players in the world, before filtering.
    pub fn len(&self) -> usize {
        self.players.len()
    }

    pub fn is_empty(&self) -> bool {
        self.players.is_empty()
    }

    pub fn sort(&self) -> Sort {
        self.sort
    }

    pub fn filter_mut(&mut self) -> &mut String {
        &mut self.filter
    }

    /// A click on a column header: the same column flips the direction, another column sorts
    /// ascending.
    pub fn click(&mut self, column: Column) {
        if self.sort.column == column {
            self.sort.descending = !self.sort.descending;
        } else {
            self.sort = Sort {
                column,
                descending: false,
            };
        }
    }

    /// The rows to show: the players matching the filter, in sort order. Equal rows keep the
    /// order the server sent (by name).
    pub fn rows(&self) -> Vec<&GamePlayer> {
        let needle = self.filter.trim().to_lowercase();
        let mut rows: Vec<&GamePlayer> = self
            .players
            .iter()
            .filter(|p| needle.is_empty() || matches_filter(p, &needle))
            .collect();
        rows.sort_by(|a, b| {
            let order = self.sort.column.compare(a, b);
            if self.sort.descending {
                order.reverse()
            } else {
                order
            }
        });
        rows
    }
}

/// `needle` is already lower case. Matches name, account, clan, IP, level or class id as text.
fn matches_filter(p: &GamePlayer, needle: &str) -> bool {
    p.name.to_lowercase().contains(needle)
        || p.account.to_lowercase().contains(needle)
        || p.clan.as_deref().unwrap_or("").to_lowercase().contains(needle)
        || p.ip.as_deref().unwrap_or("").to_lowercase().contains(needle)
        || p.level.to_string().contains(needle)
        || p.class_id.to_string().contains(needle)
}

/// `YYYY-MM-DD HH:MM:SS UTC` for a Unix time in milliseconds. UTC, because the GUI has no time
/// zone library and no datapack-free way to ask for the local one.
pub fn format_utc(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02} UTC",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// Days since 1970-01-01 to (year, month, day). Howard Hinnant's `civil_from_days`.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097); // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11], March = 0
    let day = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let month = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let year = yoe + era * 400 + i64::from(month <= 2);
    (
        year,
        u32::try_from(month).unwrap_or(1),
        u32::try_from(day).unwrap_or(1),
    )
}

/// The actions of the player panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionKind {
    Kick,
    Jail,
    BanChar,
    BanAccount,
    ChatBan,
    Teleport,
    Message,
}

impl ActionKind {
    pub const ALL: [ActionKind; 7] = [
        ActionKind::Kick,
        ActionKind::Jail,
        ActionKind::BanChar,
        ActionKind::BanAccount,
        ActionKind::ChatBan,
        ActionKind::Teleport,
        ActionKind::Message,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ActionKind::Kick => "Kick",
            ActionKind::Jail => "Jail",
            ActionKind::BanChar => "Ban character",
            ActionKind::BanAccount => "Ban account",
            ActionKind::ChatBan => "Chat ban",
            ActionKind::Teleport => "Teleport to town",
            ActionKind::Message => "Send message",
        }
    }

    /// The actions that are asked about twice (a second click confirms).
    pub fn needs_confirm(self) -> bool {
        matches!(
            self,
            ActionKind::Kick | ActionKind::BanChar | ActionKind::BanAccount
        )
    }
}

/// The fields next to the buttons.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ActionForm {
    /// Minutes of a jail or ban; empty or 0 = no end.
    pub minutes: String,
    pub reason: String,
    pub message: String,
    /// Index into [`TOWNS`].
    pub town: usize,
}

/// The request for `kind` on the character `name`, or the reason it cannot be sent.
pub fn player_action_request(name: &str, kind: ActionKind, form: &ActionForm) -> Result<Request, String> {
    let minutes = || -> Result<u32, String> {
        let text = form.minutes.trim();
        if text.is_empty() {
            return Ok(0);
        }
        text.parse()
            .map_err(|_| format!("'{text}' is not a number of minutes"))
    };
    let reason = form.reason.trim().to_owned();
    let action = match kind {
        ActionKind::Kick => PlayerAction::Kick,
        ActionKind::Jail => PlayerAction::Jail {
            minutes: minutes()?,
            reason,
        },
        ActionKind::BanChar => PlayerAction::BanChar {
            minutes: minutes()?,
            reason,
        },
        ActionKind::BanAccount => PlayerAction::BanAccount {
            minutes: minutes()?,
            reason,
        },
        ActionKind::ChatBan => PlayerAction::ChatBan {
            minutes: minutes()?,
            reason,
        },
        ActionKind::Teleport => {
            let (town, ..) = TOWNS.get(form.town).ok_or("choose a town")?;
            PlayerAction::Teleport {
                town: (*town).to_owned(),
            }
        }
        ActionKind::Message => {
            let text = form.message.trim();
            if text.is_empty() {
                return Err("the message is empty".into());
            }
            PlayerAction::Message {
                text: text.to_owned(),
            }
        }
    };
    Ok(game_request(GameAction::Player {
        name: name.to_owned(),
        action,
    }))
}

#[derive(Default)]
pub struct PlayersView {
    table: PlayerTable,
    /// The character the actions apply to.
    selected: Option<String>,
    form: ActionForm,
    /// The action waiting for its second click.
    armed: Option<ActionKind>,
    /// The action request in flight, and what it answered.
    action_request: Option<u64>,
    action_status: Option<Result<String, String>>,
    /// Characters whose IP the GM asked to see.
    revealed: Vec<String>,
    /// The `GetGameStatus` request in flight, if any.
    request: Option<u64>,
    /// No new request before this instant. `None` = due now.
    next_at: Option<Instant>,
    /// The last answer had no list (game server down, no connection to it, ...).
    error: Option<String>,
    /// At least one list has arrived since the last reset.
    loaded: bool,
}

impl PlayersView {
    /// Drops the list and any pending request: a new connection gets a fresh picture.
    fn reset(&mut self) {
        self.table.clear();
        self.request = None;
        self.next_at = None;
        self.error = None;
        self.loaded = false;
        self.selected = None;
        self.armed = None;
        self.action_request = None;
        self.revealed.clear();
    }

    /// The request for `kind` on the selected character. A confirm-first action only arms on the
    /// first click.
    fn trigger(&mut self, kind: ActionKind) -> Option<Request> {
        let name = self.selected.clone()?;
        if kind.needs_confirm() && self.armed != Some(kind) {
            self.armed = Some(kind);
            return None;
        }
        self.armed = None;
        match player_action_request(&name, kind, &self.form) {
            Ok(request) => Some(request),
            Err(error) => {
                self.action_status = Some(Err(error));
                None
            }
        }
    }

    /// True when connected, no request is pending, and the refresh time has come.
    fn refresh_due(&self, connected: bool, now: Instant) -> bool {
        connected && self.request.is_none() && self.next_at.is_none_or(|at| now >= at)
    }

    /// Folds one bridge event into the view. `now` is passed in so the timer is testable.
    fn apply(&mut self, event: &BridgeEvent, now: Instant) {
        match event {
            // A new attempt or session: requests of the old connection never get a useful answer.
            BridgeEvent::Connecting { .. }
            | BridgeEvent::Waiting { .. }
            | BridgeEvent::Connected { .. }
            | BridgeEvent::Disconnected
            | BridgeEvent::Rejected { .. } => self.reset(),
            BridgeEvent::Event(_) => {}
            BridgeEvent::Response { id, result } => {
                if self.action_request == Some(*id) {
                    self.action_request = None;
                    self.action_status = Some(reply_lines(result).map(|lines| {
                        if lines.is_empty() {
                            "Done.".to_owned()
                        } else {
                            lines.join(" | ")
                        }
                    }));
                    // The list changes after a kick or a ban: ask again now.
                    self.next_at = None;
                    return;
                }
                if self.request != Some(*id) {
                    return;
                }
                self.request = None;
                self.next_at = Some(now + REFRESH);
                match result {
                    Ok(Response::GameStatus { status }) => {
                        self.table.set_players(status.players.clone());
                        self.error = None;
                        self.loaded = true;
                    }
                    Ok(Response::Error { message }) => self.failed(message.clone()),
                    Ok(other) => self.failed(format!("unexpected answer: {other:?}")),
                    Err(error) => self.failed(error.clone()),
                }
            }
        }
    }

    fn failed(&mut self, message: String) {
        self.table.clear();
        self.error = Some(message);
        self.loaded = false;
    }
}

impl PlayersView {
    /// The buttons for the selected character.
    fn action_panel(&mut self, ui: &mut egui::Ui, cx: &mut AppCtx<'_>, connected: bool) {
        ui.add_space(12.0);
        ui.separator();
        let Some(name) = self.selected.clone() else {
            ui.label("Click a name to act on that character.");
            return;
        };
        ui.strong(format!("Selected: {name}"));
        egui::Grid::new("player_action_form")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("Minutes (jail, bans; empty = no end):");
                ui.add(egui::TextEdit::singleline(&mut self.form.minutes).desired_width(80.0));
                ui.end_row();
                ui.label("Reason:");
                ui.add(egui::TextEdit::singleline(&mut self.form.reason).desired_width(300.0));
                ui.end_row();
                ui.label("Message:");
                ui.add(egui::TextEdit::singleline(&mut self.form.message).desired_width(300.0));
                ui.end_row();
                ui.label("Town:");
                let current = TOWNS.get(self.form.town).map_or("?", |t| t.0);
                egui::ComboBox::from_id_salt("player_action_town")
                    .selected_text(current)
                    .show_ui(ui, |ui| {
                        for (index, (town, ..)) in TOWNS.iter().enumerate() {
                            ui.selectable_value(&mut self.form.town, index, *town);
                        }
                    });
                ui.end_row();
            });
        ui.horizontal_wrapped(|ui| {
            for kind in ActionKind::ALL {
                let label = if self.armed == Some(kind) {
                    format!("Really {}?", kind.label().to_lowercase())
                } else {
                    kind.label().to_owned()
                };
                if ui.add_enabled(connected, egui::Button::new(label)).clicked()
                    && let Some(request) = self.trigger(kind)
                {
                    self.action_request = Some(cx.bridge.request(request));
                }
            }
        });
        match &self.action_status {
            Some(Ok(text)) => {
                ui.label(text.as_str());
            }
            Some(Err(error)) => {
                ui.colored_label(ui.visuals().error_fg_color, error.as_str());
            }
            None => {}
        }
    }
}

impl View for PlayersView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut AppCtx<'_>) {
        let connected = cx.conn.is_connected();
        let now = Instant::now();

        if self.refresh_due(connected, now) {
            self.request = Some(cx.bridge.request(Request::GetGameStatus {
                server: Some(GAME_SERVER.to_string()),
            }));
        } else if let Some(at) = self.next_at.filter(|at| *at > now) {
            ui.ctx().request_repaint_after(at - now);
        }

        ui.heading("Players");
        ui.add_space(8.0);
        if !connected {
            ui.colored_label(ui.visuals().warn_fg_color, "Not connected to the supervisor.");
        }
        if let Some(error) = &self.error {
            ui.colored_label(
                ui.visuals().error_fg_color,
                format!("Players unavailable: {error}"),
            );
        }
        ui.horizontal(|ui| {
            ui.label("Filter:");
            ui.add(
                egui::TextEdit::singleline(self.table.filter_mut())
                    .hint_text("name, account, IP, level or class")
                    .desired_width(260.0),
            );
        });
        ui.add_space(8.0);

        if self.table.is_empty() {
            let text = if self.error.is_some() {
                "No player list."
            } else if self.loaded {
                "No players in the world."
            } else {
                "Waiting for the game server's player list..."
            };
            ui.label(text);
            return;
        }

        let sort = self.table.sort();
        let total = self.table.len();
        let rows = self.table.rows();
        let mut clicked: Option<Column> = None;
        let mut picked: Option<String> = None;
        let mut reveal: Option<String> = None;
        ui.label(if rows.len() == total {
            format!("{total} online")
        } else {
            format!("{} of {total} online shown", rows.len())
        });
        ui.add_space(4.0);
        egui::ScrollArea::horizontal().show(ui, |ui| {
            egui::Grid::new("players_table")
                .striped(true)
                .spacing([16.0, 8.0])
                .show(ui, |ui| {
                    for column in Column::ALL {
                        let active = sort.column == column;
                        let label = match (active, sort.descending) {
                            (true, false) => format!("{} (asc)", column.label()),
                            (true, true) => format!("{} (desc)", column.label()),
                            (false, _) => column.label().to_string(),
                        };
                        if ui.selectable_label(active, label).clicked() {
                            clicked = Some(column);
                        }
                    }
                    ui.end_row();
                    for player in &rows {
                        let is_selected = self.selected.as_deref() == Some(player.name.as_str());
                        if ui.selectable_label(is_selected, player.name.as_str()).clicked() {
                            picked = Some(player.name.clone());
                        }
                        ui.label(player.account.as_str());
                        ui.label(player.level.to_string());
                        ui.label(player.class_id.to_string());
                        ui.label(player.clan.as_deref().unwrap_or("-"));
                        ui.label(format!("{} {} {}", player.x, player.y, player.z));
                        // The IP only on demand: a click on the dots shows it.
                        let shown = self.revealed.contains(&player.name);
                        let text = match (&player.ip, shown) {
                            (None, _) => "-".to_owned(),
                            (Some(ip), true) => ip.clone(),
                            (Some(_), false) => "show".to_owned(),
                        };
                        if ui.selectable_label(shown, text).clicked() {
                            reveal = Some(player.name.clone());
                        }
                        ui.label(format_utc(player.online_since_ms));
                        ui.end_row();
                    }
                });
        });
        if rows.is_empty() {
            ui.label("No player matches the filter.");
        }
        if let Some(column) = clicked {
            self.table.click(column);
        }
        if let Some(name) = picked {
            if self.selected.as_deref() != Some(name.as_str()) {
                self.armed = None;
            }
            self.selected = Some(name);
        }
        if let Some(name) = reveal {
            match self.revealed.iter().position(|n| *n == name) {
                Some(index) => {
                    self.revealed.remove(index);
                }
                None => self.revealed.push(name),
            }
        }
        self.action_panel(ui, cx, connected);
    }

    fn on_event(&mut self, event: &BridgeEvent) {
        self.apply(event, Instant::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use l2_admin_api::GameStatus;

    fn player(
        name: &str,
        account: &str,
        level: i32,
        class_id: i32,
        ip: Option<&str>,
        since: i64,
    ) -> GamePlayer {
        GamePlayer {
            name: name.into(),
            account: account.into(),
            level,
            class_id,
            ip: ip.map(Into::into),
            online_since_ms: since,
            clan: None,
            x: 0,
            y: 0,
            z: 0,
        }
    }

    fn table() -> PlayerTable {
        let mut table = PlayerTable::default();
        table.set_players(vec![
            player("bob", "acc2", 20, 10, Some("10.0.0.2"), 3_000),
            player("Alice", "acc1", 80, 88, Some("10.0.0.10"), 2_000),
            player("carol", "acc3", 45, 53, None, 1_000),
        ]);
        table
    }

    fn names(table: &PlayerTable) -> Vec<&str> {
        table.rows().iter().map(|p| p.name.as_str()).collect()
    }

    #[test]
    fn starts_sorted_by_name_ignoring_case() {
        assert_eq!(names(&table()), ["Alice", "bob", "carol"]);
    }

    #[test]
    fn clicking_a_column_sorts_then_flips() {
        let mut t = table();
        t.click(Column::Level);
        assert_eq!(names(&t), ["bob", "carol", "Alice"]);
        t.click(Column::Level);
        assert_eq!(names(&t), ["Alice", "carol", "bob"]);
        // A different column starts ascending again.
        t.click(Column::OnlineSince);
        assert_eq!(names(&t), ["carol", "Alice", "bob"]);
        assert_eq!(
            t.sort(),
            Sort {
                column: Column::OnlineSince,
                descending: false
            }
        );
    }

    #[test]
    fn missing_ip_sorts_first_ascending() {
        let mut t = table();
        t.click(Column::Ip);
        assert_eq!(names(&t), ["carol", "bob", "Alice"], "10.0.0.2 before 10.0.0.10");
    }

    #[test]
    fn class_and_account_sort_by_their_own_values() {
        let mut t = table();
        t.click(Column::Class);
        assert_eq!(names(&t), ["bob", "carol", "Alice"]);
        t.click(Column::Account);
        assert_eq!(names(&t), ["Alice", "bob", "carol"]);
    }

    #[test]
    fn filter_matches_any_text_column_ignoring_case() {
        let mut t = table();
        *t.filter_mut() = "ALI".into();
        assert_eq!(names(&t), ["Alice"]);
        *t.filter_mut() = "acc2".into();
        assert_eq!(names(&t), ["bob"]);
        *t.filter_mut() = "10.0.0.1".into();
        assert_eq!(names(&t), ["Alice"]);
        *t.filter_mut() = "88".into();
        assert_eq!(names(&t), ["Alice"]);
        *t.filter_mut() = "  ".into();
        assert_eq!(names(&t).len(), 3, "blank filter shows everyone");
        *t.filter_mut() = "nobody".into();
        assert!(names(&t).is_empty());
        // Total stays the same while filtering.
        assert_eq!(t.len(), 3);
    }

    #[test]
    fn clan_and_location_sort_and_filter() {
        let mut t = PlayerTable::default();
        let mut a = player("Amy", "acc1", 1, 0, None, 0);
        a.clan = Some("Bears".into());
        a.x = 100;
        let mut b = player("Bo", "acc2", 1, 0, None, 0);
        b.x = -5;
        t.set_players(vec![a, b]);
        t.click(Column::Clan);
        assert_eq!(names(&t), ["Bo", "Amy"], "no clan first");
        t.click(Column::Location);
        assert_eq!(names(&t), ["Bo", "Amy"]);
        *t.filter_mut() = "bear".into();
        assert_eq!(names(&t), ["Amy"]);
    }

    fn form() -> ActionForm {
        ActionForm {
            minutes: " 30 ".into(),
            reason: " botting ".into(),
            message: "hello".into(),
            town: 9,
        }
    }

    fn player_action(request: Request) -> (String, PlayerAction) {
        match request {
            Request::GameAdmin {
                server,
                action: GameAction::Player { name, action },
            } => {
                assert_eq!(server.as_deref(), Some("game"));
                (name, action)
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn actions_become_requests() {
        let f = form();
        let build = |kind| player_action(player_action_request("Amy", kind, &f).unwrap()).1;
        assert_eq!(build(ActionKind::Kick), PlayerAction::Kick);
        assert_eq!(
            build(ActionKind::Jail),
            PlayerAction::Jail {
                minutes: 30,
                reason: "botting".into()
            }
        );
        assert_eq!(
            build(ActionKind::BanAccount),
            PlayerAction::BanAccount {
                minutes: 30,
                reason: "botting".into()
            }
        );
        assert_eq!(
            build(ActionKind::ChatBan),
            PlayerAction::ChatBan {
                minutes: 30,
                reason: "botting".into()
            }
        );
        assert_eq!(
            build(ActionKind::Teleport),
            PlayerAction::Teleport { town: "Giran".into() }
        );
        assert_eq!(
            build(ActionKind::Message),
            PlayerAction::Message { text: "hello".into() }
        );
        // Empty minutes = no end.
        let empty = ActionForm::default();
        assert_eq!(
            player_action(player_action_request("Amy", ActionKind::BanChar, &empty).unwrap()).1,
            PlayerAction::BanChar {
                minutes: 0,
                reason: String::new()
            }
        );
    }

    #[test]
    fn bad_forms_are_refused() {
        let mut f = form();
        f.minutes = "soon".into();
        assert_eq!(
            player_action_request("Amy", ActionKind::Jail, &f).unwrap_err(),
            "'soon' is not a number of minutes"
        );
        f.message = "  ".into();
        assert!(player_action_request("Amy", ActionKind::Message, &f).is_err());
        f.town = 999;
        assert!(player_action_request("Amy", ActionKind::Teleport, &f).is_err());
    }

    #[test]
    fn destructive_actions_need_a_second_click() {
        let mut view = PlayersView::default();
        assert!(view.trigger(ActionKind::Kick).is_none(), "nobody selected");
        view.selected = Some("Amy".into());
        assert!(view.trigger(ActionKind::Kick).is_none());
        assert_eq!(view.armed, Some(ActionKind::Kick));
        // Another action does not confirm the armed one.
        assert!(view.trigger(ActionKind::BanChar).is_none());
        assert_eq!(view.armed, Some(ActionKind::BanChar));
        assert!(view.trigger(ActionKind::BanChar).is_some());
        assert_eq!(view.armed, None);
        // Jail and the others go at once.
        assert!(view.trigger(ActionKind::Jail).is_some());
        // A form error is shown, nothing is sent.
        view.form.minutes = "x".into();
        assert!(view.trigger(ActionKind::Jail).is_none());
        assert!(matches!(view.action_status, Some(Err(_))));
    }

    #[test]
    fn the_answer_of_an_action_sets_the_status_and_refreshes_the_list() {
        let now = Instant::now();
        let mut view = PlayersView {
            action_request: Some(7),
            next_at: Some(now + REFRESH),
            ..PlayersView::default()
        };
        view.apply(
            &BridgeEvent::Response {
                id: 7,
                result: Ok(Response::GameReply {
                    lines: vec!["You kicked Amy from the game.".into()],
                }),
            },
            now,
        );
        assert_eq!(
            view.action_status,
            Some(Ok("You kicked Amy from the game.".into()))
        );
        assert!(view.refresh_due(true, now));
    }

    #[test]
    fn clear_empties_the_table() {
        let mut t = table();
        t.clear();
        assert!(t.is_empty());
        assert!(t.rows().is_empty());
    }

    #[test]
    fn utc_times_are_readable() {
        assert_eq!(format_utc(0), "1970-01-01 00:00:00 UTC");
        assert_eq!(format_utc(1_700_000_000_000), "2023-11-14 22:13:20 UTC");
        // 2024-02-29 is a leap day, 2024-03-01 the day after.
        assert_eq!(format_utc(1_709_164_800_000), "2024-02-29 00:00:00 UTC");
        assert_eq!(format_utc(1_709_251_200_000), "2024-03-01 00:00:00 UTC");
    }

    fn status_answer(id: u64, players: Vec<GamePlayer>) -> BridgeEvent {
        BridgeEvent::Response {
            id,
            result: Ok(Response::GameStatus {
                status: GameStatus { players },
            }),
        }
    }

    #[test]
    fn answer_fills_the_table_and_schedules_the_next_refresh() {
        let now = Instant::now();
        let mut view = PlayersView::default();
        assert!(view.refresh_due(true, now));
        assert!(!view.refresh_due(false, now), "not connected");

        view.request = Some(4);
        assert!(!view.refresh_due(true, now), "request in flight");
        view.apply(
            &status_answer(4, vec![player("Amy", "acc1", 80, 88, None, 0)]),
            now,
        );
        assert_eq!(view.table.len(), 1);
        assert!(view.loaded && view.error.is_none());
        assert_eq!(view.request, None);
        assert!(!view.refresh_due(true, now), "wait before asking again");
        assert!(view.refresh_due(true, now + REFRESH));
    }

    #[test]
    fn answers_to_other_requests_are_ignored() {
        let now = Instant::now();
        let mut view = PlayersView {
            request: Some(4),
            ..PlayersView::default()
        };
        view.apply(
            &status_answer(9, vec![player("Amy", "acc1", 80, 88, None, 0)]),
            now,
        );
        assert!(view.table.is_empty());
        assert_eq!(view.request, Some(4));
    }

    #[test]
    fn failed_request_shows_the_error_and_retries_later() {
        let now = Instant::now();
        let mut view = PlayersView::default();
        view.table.set_players(vec![player("old", "acc1", 1, 0, None, 0)]);
        view.request = Some(5);
        view.apply(
            &BridgeEvent::Response {
                id: 5,
                result: Ok(Response::Error {
                    message: "no admin connection to 'game'".into(),
                }),
            },
            now,
        );
        assert!(view.table.is_empty(), "no stale list");
        assert_eq!(view.error.as_deref(), Some("no admin connection to 'game'"));
        assert!(!view.refresh_due(true, now));
        assert!(view.refresh_due(true, now + REFRESH));

        view.request = Some(6);
        view.apply(
            &BridgeEvent::Response {
                id: 6,
                result: Err("connection closed".into()),
            },
            now,
        );
        assert_eq!(view.error.as_deref(), Some("connection closed"));
    }

    #[test]
    fn disconnect_resets_the_view() {
        let now = Instant::now();
        let mut view = PlayersView {
            request: Some(1),
            ..PlayersView::default()
        };
        view.apply(
            &status_answer(1, vec![player("Amy", "acc1", 80, 88, None, 0)]),
            now,
        );
        view.request = Some(2);
        view.apply(&BridgeEvent::Disconnected, now);
        assert!(view.table.is_empty());
        assert!(!view.loaded);
        assert_eq!(view.request, None);
        assert!(view.refresh_due(true, now));
    }
}
