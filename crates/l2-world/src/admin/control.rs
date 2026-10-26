//! Server control: the login server switches (`//server_gm_only`, ...) and the shutdown
//! countdown (`//server_shutdown`, `//server_restart`, `//server_abort`).
// Java: dist/game/data/scripts/handlers/chat/commands/admin/{AdminLogin,AdminShutdown}.java
// Java: gameserver/Shutdown.java, gameserver/LoginServerThread.java (setServerStatus,
// Java: setMaxPlayer, sendServerStatus, sendServerType, getStatusString)
//
// # What the world keeps
// The world mirrors the three values the Java `LoginServerThread` keeps for the pages (the list
// status, the maximum player count, the registered name) and tells the login server link about
// every change through the outbox (`LsEvent`). The name comes back from the link
// (`WorldCommand::LoginServerRegistered`).
//
// # The shutdown
// Kain only stops itself: when the countdown is over the world asks the process to stop
// (`ControlRequest::Shutdown`), which is the same graceful stop the supervisor's `shutdown`
// request uses. It never touches the Java game server (D-38, D-39).
//
// NOTE(java-quirk): `Shutdown` keeps `_secondsShut` and `_shutdownMode` in statics. A second
// `startShutdown` while a countdown runs makes Java run two counter threads on the same statics
// (they count down twice as fast and send every message twice, unless the old one happens to see
// `ABORT` in the instant between `abort()` and the constructor). Here a new countdown replaces
// the running one.
//
// Left out:
// - `PrecautionaryRestartManager.restartEnabled()/restartAborted()` (`PrecautionaryRestartEnabled`
//   is off by default and the manager is not ported).
// - Java's `SIGTERM` mode and the shutdown hook: the process stops through the server's own
//   signal handling.

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use tracing::warn;

use super::{AdminHandler, AdminHandlers, JavaException, Tokens, is_numeric, parse_int};
use crate::outbox::{ControlRequest, LsEvent};
use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::{World, game_time_minutes, unix_ms_now};

type Outcome = Result<(), JavaException>;

/// `ServerStatus` of the game server's login link: the list status values.
pub mod ls_status {
    pub const AUTO: i32 = 0x00;
    pub const GOOD: i32 = 0x01;
    pub const NORMAL: i32 = 0x02;
    pub const FULL: i32 = 0x03;
    pub const DOWN: i32 = 0x04;
    pub const GM_ONLY: i32 = 0x05;
    /// `ServerStatus.SERVER_LIST_STATUS`.
    pub const ATTRIBUTE_LIST_STATUS: i32 = 0x01;
    /// `ServerStatus.SERVER_TYPE`.
    pub const ATTRIBUTE_TYPE: i32 = 0x02;
    /// `ServerStatus.SERVER_AGE` of the game server's own table (the login server reads age as 6,
    /// see the quirk in `l2-protocol-login`).
    pub const ATTRIBUTE_AGE: i32 = 0x05;
}

/// `ServerStatus.STATUS_STRING`.
const STATUS_STRING: [&str; 6] = ["Auto", "Good", "Normal", "Full", "Down", "Gm Only"];

/// The configuration the admin commands read or change (Java `GeneralConfig`, `ServerConfig`,
/// `PlayerConfig`, `DevelopmentConfig`). `Default` is each key's Java default.
#[derive(Debug, Clone, PartialEq)]
pub struct AdminSettings {
    /// `GeneralConfig.GM_ANNOUNCER_NAME` (`GMShowAnnouncerName`).
    pub gm_announcer_name: bool,
    /// `GeneralConfig.GM_STARTUP_AUTO_LIST` (`GMStartupAutoList`).
    pub gm_startup_auto_list: bool,
    /// `GeneralConfig.SERVER_GMONLY` (`ServerGMOnly`), changed by `//server_gm_only`.
    pub server_gmonly: bool,
    /// `ServerConfig.SERVER_LIST_TYPE` as flags, changed by `//server_list_type`.
    pub server_list_type: i32,
    /// `ServerConfig.SERVER_LIST_AGE`.
    pub server_list_age: i32,
    /// `ServerConfig.SERVER_LIST_BRACKET`.
    pub server_list_brackets: bool,
    /// `ServerConfig.MAXIMUM_ONLINE_USERS`.
    pub maximum_online_users: i32,
    /// `PlayerConfig.PETITIONING_ALLOWED`.
    pub petitioning_allowed: bool,
    /// `PlayerConfig.MAX_PETITIONS_PENDING`.
    pub max_petitions_pending: i32,
    /// `PlayerConfig.MAX_PETITIONS_PER_PLAYER`.
    pub max_petitions_per_player: i32,
    /// `DevelopmentConfig.DEBUG_CLIENT_PACKETS` (what `//debug packets` toggles).
    pub debug_client_packets: bool,
    /// `GameServer.getStartTime()`: Unix milliseconds, 0 until the server sets it.
    pub server_start_ms: i64,
}

impl Default for AdminSettings {
    fn default() -> Self {
        Self {
            gm_announcer_name: false,
            gm_startup_auto_list: false,
            server_gmonly: false,
            // `ServerListType` defaults to "Free".
            server_list_type: 0x40,
            server_list_age: 0,
            server_list_brackets: false,
            maximum_online_users: 2000,
            petitioning_allowed: true,
            max_petitions_pending: 25,
            max_petitions_per_player: 5,
            debug_client_packets: false,
            server_start_ms: 0,
        }
    }
}

/// What the `LoginServerThread` keeps for the pages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LsMirror {
    /// `_serverName`: `None` (Java `null`) until the login server registered the game server.
    pub(crate) server_name: Option<String>,
    /// `_status`. NOTE(java-quirk): Java never sets it at the registration, so it reads `Auto`
    /// even when the server starts GM only.
    pub(crate) status: i32,
    /// `_maxPlayer`.
    pub(crate) max_players: i32,
}

impl Default for LsMirror {
    fn default() -> Self {
        Self {
            server_name: None,
            status: ls_status::AUTO,
            max_players: 0,
        }
    }
}

/// Java `Shutdown` mode constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum ShutdownMode {
    #[default]
    Sigterm,
    GmShutdown,
    GmRestart,
    Abort,
}

impl ShutdownMode {
    /// `Shutdown.MODE_TEXT`.
    fn text(self) -> &'static str {
        match self {
            Self::Sigterm => "SIGTERM",
            Self::GmShutdown => "shutting down",
            Self::GmRestart => "restarting",
            Self::Abort => "aborting",
        }
    }
}

/// The statics of Java's `Shutdown`.
#[derive(Debug, Default)]
pub(crate) struct ShutdownState {
    /// `_shutdownMode`.
    pub(crate) mode: ShutdownMode,
    /// `_secondsShut`.
    pub(crate) seconds: i32,
    /// `_counterInstance != null`: true once any countdown was started.
    pub(crate) counter_exists: bool,
    /// `_countdownFinished`.
    pub(crate) finished: bool,
    /// The pending one second step of the countdown.
    pub(crate) timer: Option<TimerId>,
}

/// Everything the admin handlers of this module group keep in the world.
#[derive(Default)]
pub(crate) struct ControlWorld {
    pub(crate) settings: AdminSettings,
    pub(crate) ls: LsMirror,
    pub(crate) shutdown: ShutdownState,
    pub(crate) gm_list: super::gm_tools::GmList,
    pub(crate) snoop: super::gm_tools::SnoopTable,
    pub(crate) announcements: super::announce::AnnouncementTable,
    pub(crate) debug: super::debug::DebugState,
    pub(crate) petitions: crate::petition::PetitionWorld,
    /// `World.MAX_CONNECTED_COUNT`.
    pub(crate) max_connected: usize,
}

/// The timers of the admin handlers (one `TimerKind::Admin`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdminTimer {
    /// One second of the shutdown countdown.
    ShutdownStep,
    /// The next run of an auto announcement (`generation` ends the runs of a restarted one).
    AutoAnnouncement { id: i32, generation: u64 },
    /// The periodic check of `//debug doors`, `//debug geodata` or `//debug movement`.
    Debug(super::debug::DebugTimer),
}

/// Java `Shutdown` timing: the seconds at which the countdown says something.
fn is_announced_second(seconds: i32) -> bool {
    matches!(
        seconds,
        540 | 480 | 420 | 360 | 300 | 240 | 180 | 120 | 60 | 30 | 10 | 5 | 4 | 3 | 2 | 1
    )
}

/// Java `AdminLogin` / `AdminShutdown` registrations.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let login: AdminHandler = admin_login;
    for command in [
        "admin_server_gm_only",
        "admin_server_all",
        "admin_server_max_player",
        "admin_server_list_type",
        "admin_server_list_age",
        "admin_server_login",
    ] {
        handlers.register(command, login);
    }
    let shutdown: AdminHandler = admin_shutdown;
    for command in [
        "admin_server_shutdown",
        "admin_server_restart",
        "admin_server_abort",
    ] {
        handlers.register(command, shutdown);
    }
}

/// Java `new StringTokenizer(text).countTokens()`.
pub(crate) fn count_tokens(text: &str) -> usize {
    let mut tokens = Tokens::new(text);
    let mut count = 0;
    while tokens.next_token().is_some() {
        count += 1;
    }
    count
}

impl World {
    /// Hands the world the configuration of the admin handlers. Call before players enter.
    pub fn set_admin_settings(&mut self, settings: AdminSettings) {
        self.admin.ctl.ls.max_players = settings.maximum_online_users;
        self.admin.ctl.settings = settings;
    }

    /// The login server registered the game server under this name (Java `_serverName`).
    pub(crate) fn on_login_server_registered(&mut self, name: String) {
        self.admin.ctl.ls.server_name = Some(name);
    }

    /// Java `LoginServerThread.setServerStatus(status)` for the six valid values.
    pub(crate) fn ls_set_server_status(&mut self, status: i32) {
        self.outbox.ls_event(LsEvent::SetServerStatus(status));
        self.admin.ctl.ls.status = status;
    }

    /// Java `LoginServerThread.setMaxPlayer(maxPlayer)`.
    pub(crate) fn ls_set_max_player(&mut self, max_players: i32) {
        self.outbox.ls_event(LsEvent::SetMaxPlayer(max_players));
        self.admin.ctl.ls.max_players = max_players;
    }

    /// Java `LoginServerThread.sendServerStatus(id, value)`.
    pub(crate) fn ls_send_server_status(&mut self, id: i32, value: i32) {
        self.outbox.ls_event(LsEvent::ServerStatus { id, value });
    }

    /// Java `LoginServerThread.sendServerType()`.
    fn ls_send_server_type(&mut self) {
        let value = self.admin.ctl.settings.server_list_type;
        self.ls_send_server_status(ls_status::ATTRIBUTE_TYPE, value);
    }
}

// ------------------------------------------------------------------------------- AdminLogin

/// Java `ServerConfig.getServerTypeId(String[])`: the flags of the words, unknown words count
/// for nothing.
fn server_type_id(words: &[&str]) -> i32 {
    words.iter().fold(0, |all, word| {
        all | match word.trim().to_lowercase().as_str() {
            "normal" => 0x01,
            "relax" => 0x02,
            "test" => 0x04,
            "nolabel" => 0x08,
            "restricted" => 0x10,
            "event" => 0x20,
            "free" => 0x40,
            _ => 0,
        }
    })
}

/// Java `AdminLogin.getServerTypeName`.
fn server_type_name(server_type: i32) -> String {
    let mut name = String::new();
    for i in 0..7 {
        let current = server_type & (1 << i);
        if current > 0 {
            if !name.is_empty() {
                name.push('+');
            }
            name.push_str(match current {
                0x01 => "Normal",
                0x02 => "Relax",
                0x04 => "Test",
                0x08 => "NoLabel",
                0x10 => "Restricted",
                0x20 => "Event",
                0x40 => "Free",
                _ => "",
            });
        }
    }
    name
}

/// Java `AdminLogin.onCommand`.
fn admin_login(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    if command == "admin_server_gm_only" {
        // `gmOnly()`
        world.ls_set_server_status(ls_status::GM_ONLY);
        world.admin.ctl.settings.server_gmonly = true;
        world.send_sys_message(gm, "Server is now GM only");
        show_login_page(world, gm)?;
    } else if command == "admin_server_all" {
        // `allowToAll()`
        world.ls_set_server_status(ls_status::AUTO);
        world.admin.ctl.settings.server_gmonly = false;
        world.send_sys_message(gm, "Server is not GM only anymore");
        show_login_page(world, gm)?;
    } else if command.starts_with("admin_server_max_player") {
        let mut st = Tokens::new(command);
        if count_tokens(command) > 1 {
            st.next_token();
            let number = st.next_token().unwrap_or_default();
            match parse_int(number) {
                Ok(value) => {
                    world.ls_set_max_player(value);
                    world.send_sys_message(gm, &format!("maxPlayer set to {number}"));
                    show_login_page(world, gm)?;
                }
                Err(_) => world.send_sys_message(gm, "Max players must be a number."),
            }
        } else {
            world.send_sys_message(gm, "Format is server_max_player <max>");
        }
    } else if command.starts_with("admin_server_list_type") {
        let tokens = count_tokens(command);
        if tokens > 1 {
            let mut st = Tokens::new(command);
            st.next_token();
            let modes: Vec<&str> = std::iter::from_fn(|| st.next_token()).map(str::trim).collect();
            let new_type = match parse_int(modes[0]) {
                Ok(value) => value,
                Err(_) => server_type_id(&modes),
            };
            if world.admin.ctl.settings.server_list_type != new_type {
                world.admin.ctl.settings.server_list_type = new_type;
                world.ls_send_server_type();
                world.send_sys_message(
                    gm,
                    &format!("Server Type changed to {}", server_type_name(new_type)),
                );
            } else {
                world.send_sys_message(
                    gm,
                    &format!("Server Type is already {}", server_type_name(new_type)),
                );
            }
            show_login_page(world, gm)?;
        } else {
            world.send_sys_message(
                gm,
                "Format is server_list_type <normal/relax/test/nolabel/restricted/event/free>",
            );
        }
    } else if command.starts_with("admin_server_list_age") {
        if count_tokens(command) > 1 {
            let mut st = Tokens::new(command);
            st.next_token();
            let mode = st.next_token().unwrap_or_default();
            match parse_int(mode) {
                Ok(age) => {
                    if world.admin.ctl.settings.server_list_age != age {
                        // NOTE(java-quirk): Java stores the age into `SERVER_LIST_TYPE`, and
                        // never into `SERVER_LIST_AGE`, so the same age is "changed" again.
                        world.admin.ctl.settings.server_list_type = age;
                        world.ls_send_server_status(ls_status::ATTRIBUTE_AGE, age);
                        world.send_sys_message(gm, &format!("Server Age changed to {age}"));
                    } else {
                        world.send_sys_message(gm, &format!("Server Age is already {age}"));
                    }
                    show_login_page(world, gm)?;
                }
                Err(_) => world.send_sys_message(gm, "Age must be a number"),
            }
        } else {
            world.send_sys_message(gm, "Format is server_list_age <number>");
        }
    } else if command == "admin_server_login" {
        show_login_page(world, gm)?;
    }
    Ok(())
}

/// Java `AdminLogin.showMainPage`.
fn show_login_page(world: &mut World, gm: ObjectId) -> Outcome {
    let mut html = world.admin_html_file(gm, "data/html/admin/login.htm");
    // `replace(pattern, String)` with a null value throws a `NullPointerException`; the name is
    // null until the login server registered the game server.
    let Some(name) = world.admin.ctl.ls.server_name.clone() else {
        return Err(JavaException("java.lang.NullPointerException".to_owned()));
    };
    let ctl = &world.admin.ctl;
    let status = STATUS_STRING
        .get(usize::try_from(ctl.ls.status).unwrap_or(usize::MAX))
        .copied()
        .unwrap_or_default();
    html.replace("%server_name%", &name)
        .replace("%status%", status)
        .replace("%clock%", &server_type_name(ctl.settings.server_list_type))
        .replace("%brackets%", &ctl.settings.server_list_brackets.to_string())
        .replace("%max_players%", &ctl.ls.max_players.to_string());
    world.send_admin_html(gm, &html);
    Ok(())
}

// ----------------------------------------------------------------------------- AdminShutdown

/// Java `AdminShutdown.onCommand`.
fn admin_shutdown(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    if command.starts_with("admin_server_shutdown") {
        // `command.substring(22)`: a command without the space after it throws
        // `StringIndexOutOfBoundsException`, which shows the form.
        match command.get(22..) {
            Some(value) if command.len() >= 22 => {
                if is_numeric(value) {
                    world.start_shutdown(Some(gm), parse_int(value)?, false);
                } else {
                    world.send_sys_message(gm, "Usage: //server_shutdown <seconds>");
                    send_shutdown_form(world, gm);
                }
            }
            _ => send_shutdown_form(world, gm),
        }
    } else if command.starts_with("admin_server_restart") {
        match command.get(21..) {
            Some(value) if command.len() >= 21 => {
                if is_numeric(value) {
                    world.start_shutdown(Some(gm), parse_int(value)?, true);
                } else {
                    world.send_sys_message(gm, "Usage: //server_restart <seconds>");
                    send_shutdown_form(world, gm);
                }
            }
            _ => send_shutdown_form(world, gm),
        }
    } else if command.starts_with("admin_server_abort") {
        world.abort_shutdown(Some(gm));
    }
    Ok(())
}

/// The bytes of the memory the server uses: Java prints `totalMemory() - freeMemory()` of the
/// JVM heap. On Linux this is the resident set of the process, elsewhere 0 (the standard library
/// cannot read it).
pub(crate) fn used_memory_bytes() -> u64 {
    std::fs::read_to_string("/proc/self/statm")
        .ok()
        .and_then(|text| {
            text.split_whitespace()
                .nth(1)
                .and_then(|pages| pages.parse::<u64>().ok())
        })
        .map_or(0, |pages| pages.saturating_mul(4096))
}

/// `DateTimeFormatter.ofPattern("h:mm a")` of `LocalTime.of(minutes / 60, minutes % 60)`.
fn format_clock(minutes: i32) -> String {
    let (hour, minute) = (minutes / 60, minutes % 60);
    let twelve = if hour % 12 == 0 { 12 } else { hour % 12 };
    format!("{twelve}:{minute:02} {}", if hour < 12 { "AM" } else { "PM" })
}

/// Java `AdminShutdown.sendHtmlForm`.
fn send_shutdown_form(world: &mut World, gm: ObjectId) {
    let minutes = game_time_minutes(unix_ms_now());
    let mut html = world.admin_html_file(gm, "data/html/admin/shutdown.htm");
    html.replace("%count%", &world.players.len().to_string())
        .replace("%used%", &used_memory_bytes().to_string())
        .replace("%time%", &format_clock(minutes));
    world.send_admin_html(gm, &html);
}

impl World {
    /// Java `SystemMessage(THE_SERVER_WILL_BE_COMING_DOWN_IN_S1_SECOND_S...)` to every player.
    fn send_server_quit(&self, seconds: i32) {
        let message = SystemMessage::new(
            SystemMessageId::TheServerWillBeComingDownInS1SecondSPleaseFindASafePlaceToLogOut,
        )
        .with(SystemMessageParam::IntNumber(seconds));
        self.broadcast_to_all_online_players(&message);
    }

    /// Java `World.broadcastToAllOnlinePlayers(text, isCritical)`.
    pub(crate) fn broadcast_announcement(&self, text: &str, critical: bool) {
        use l2_protocol_game::server::{ChatType, CreatureSay, SayBody, SenderField};
        self.broadcast_to_all_online_players(&CreatureSay {
            sender_object_id: 0,
            chat_type: if critical {
                ChatType::CriticalAnnounce
            } else {
                ChatType::Announcement
            },
            sender: SenderField::Name(String::new()),
            message_id: -1,
            body: SayBody::Text(text.to_owned()),
        });
    }

    /// Java `Shutdown.startShutdown(player, seconds, restart)`.
    pub(crate) fn start_shutdown(&mut self, player: Option<ObjectId>, seconds: i32, restart: bool) {
        let state = &mut self.admin.ctl.shutdown;
        state.mode = if restart {
            ShutdownMode::GmRestart
        } else {
            ShutdownMode::GmShutdown
        };
        let mode_text = state.mode.text();
        match player {
            Some(player) => {
                let name = self.object_name(player).unwrap_or_default();
                warn!(
                    "GM: {name}({}) issued shutdown command. {mode_text} in {seconds} seconds!",
                    player.0
                );
            }
            None => warn!(
                "Server scheduled restart issued shutdown command. {} in {seconds} seconds!",
                if restart { "Restart" } else { "Shutdown" }
            ),
        }
        // `if (_shutdownMode > 0)`: always true here.
        if !is_announced_second(seconds) {
            self.send_server_quit(seconds);
        }
        // `_counterInstance.abort()` and the new counter thread (see the module quirk).
        if let Some(timer) = self.admin.ctl.shutdown.timer.take() {
            self.timers.cancel(timer);
        }
        let state = &mut self.admin.ctl.shutdown;
        state.counter_exists = true;
        state.seconds = seconds.max(0);
        state.mode = if restart {
            ShutdownMode::GmRestart
        } else {
            ShutdownMode::GmShutdown
        };
        // The counter thread starts at once.
        self.shutdown_step();
    }

    /// Java `Shutdown.abort(player)`.
    pub(crate) fn abort_shutdown(&mut self, player: Option<ObjectId>) {
        let who =
            player.map(|player| format!("{}({}) ", self.object_name(player).unwrap_or_default(), player.0));
        let who = who.unwrap_or_default();
        let state = &mut self.admin.ctl.shutdown;
        if state.finished {
            warn!("GM: {who}shutdown ABORT failed because countdown has finished.");
            return;
        }
        warn!(
            "GM: {who}issued shutdown ABORT. {} has been stopped!",
            state.mode.text()
        );
        if state.counter_exists {
            // `_counterInstance.abort()`: the counter notices at its next step.
            state.mode = ShutdownMode::Abort;
            // NOTE(java-quirk): the mode is already `ABORT` here, so the text says "aborting".
            let text = format!(
                "Server aborts {} and continues normal operation!",
                ShutdownMode::Abort.text()
            );
            self.broadcast_announcement(&text, false);
        }
    }

    /// One step of `Shutdown.countdown()`: the body of the loop, then what `run()` does when the
    /// loop is over. Schedules the next second while there is one.
    fn shutdown_step(&mut self) {
        self.admin.ctl.shutdown.timer = None;
        // `if (_countdownFinished) return;` at the top of `run()`.
        if self.admin.ctl.shutdown.finished {
            return;
        }
        if self.admin.ctl.shutdown.seconds > 0 {
            // Rehabilitate previous server status if shutdown is aborted.
            if self.admin.ctl.shutdown.mode == ShutdownMode::Abort {
                if self.admin.ctl.ls.status == ls_status::DOWN {
                    let status = if self.admin.ctl.settings.server_gmonly {
                        ls_status::GM_ONLY
                    } else {
                        ls_status::AUTO
                    };
                    self.ls_set_server_status(status);
                }
                self.shutdown_countdown_over();
                return;
            }
            let seconds = self.admin.ctl.shutdown.seconds;
            if is_announced_second(seconds) {
                self.send_server_quit(seconds);
            }
            // Prevent players from logging in.
            if seconds <= 60 && self.admin.ctl.ls.status != ls_status::DOWN {
                self.ls_set_server_status(ls_status::DOWN);
            }
            self.admin.ctl.shutdown.seconds -= 1;
            let due = self.tick + TimerWheel::ms_to_ticks(1000);
            let timer = self
                .timers
                .schedule_at(due, TimerKind::Admin(AdminTimer::ShutdownStep));
            self.admin.ctl.shutdown.timer = Some(timer);
        } else {
            self.shutdown_countdown_over();
        }
    }

    /// The end of `Shutdown.run()` for a countdown thread.
    fn shutdown_countdown_over(&mut self) {
        let mode = self.admin.ctl.shutdown.mode;
        // Last point where logging is operational.
        warn!("GM shutdown countdown is over. {} NOW!", mode.text());
        match mode {
            ShutdownMode::GmShutdown | ShutdownMode::GmRestart => {
                // `startShutdownActions()` and `System.exit(0)` / `System.exit(2)`.
                self.admin.ctl.shutdown.finished = true;
                self.outbox.control(ControlRequest::Shutdown {
                    restart: mode == ShutdownMode::GmRestart,
                });
            }
            ShutdownMode::Abort => self.ls_set_server_status(ls_status::AUTO),
            ShutdownMode::Sigterm => {}
        }
    }

    /// Dispatch of `TimerKind::Admin`.
    pub(crate) fn on_admin_timer(&mut self, timer: AdminTimer) {
        match timer {
            AdminTimer::ShutdownStep => self.shutdown_step(),
            AdminTimer::AutoAnnouncement { id, generation } => {
                self.on_auto_announcement_timer(id, generation)
            }
            AdminTimer::Debug(timer) => self.on_debug_timer(timer),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_type_names_join_the_flags() {
        assert_eq!(server_type_name(0x40), "Free");
        assert_eq!(server_type_name(0x01 | 0x04), "Normal+Test");
        assert_eq!(server_type_name(0), "");
        assert_eq!(
            server_type_name(0x7F),
            "Normal+Relax+Test+NoLabel+Restricted+Event+Free"
        );
    }

    #[test]
    fn server_type_ids_read_the_words() {
        assert_eq!(server_type_id(&["normal", "RELAX", "nonsense"]), 0x03);
        assert_eq!(server_type_id(&[]), 0);
    }

    #[test]
    fn the_clock_is_twelve_hours() {
        assert_eq!(format_clock(0), "12:00 AM");
        assert_eq!(format_clock(13 * 60 + 5), "1:05 PM");
        assert_eq!(format_clock(12 * 60), "12:00 PM");
        assert_eq!(format_clock(23 * 60 + 59), "11:59 PM");
    }

    #[test]
    fn the_announced_seconds_are_javas() {
        assert!(is_announced_second(540));
        assert!(is_announced_second(1));
        assert!(!is_announced_second(45));
        assert!(!is_announced_second(0));
    }
}
