//! Local admin endpoint: game status (online characters) and a clean shutdown for the
//! supervisor/GUI (D-30, D-38).

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use l2_admin_api::{
    AdminHandler, GameAction, GameInfo, GamePlayer, GameStatus, PlayerAction, Request, Response, Session,
    TOWNS, Token,
};
use l2_config::IniFile;
use l2_world::{AdminQuery, GuiReply, GuiRequest, OnlinePlayer, WorldCommand};
use tokio::net::TcpListener;
use tracing::{info, warn};

use crate::state::GameState;
use crate::{Server, StartupError};

/// File name inside the config directory.
pub const ADMIN_CONFIG_FILE: &str = "Admin.ini";
/// Default bind address (the login server uses 7901).
pub const DEFAULT_BIND: &str = "127.0.0.1:7902";
/// How long a status request waits for the world thread. The world ticks every 50 ms, so a
/// longer wait means it is stuck.
const WORLD_ANSWER_TIMEOUT: Duration = Duration::from_secs(5);

/// `config/Admin.ini`.
#[derive(Clone, PartialEq, Eq)]
pub struct AdminSettings {
    pub enabled: bool,
    pub bind: String,
    pub token: String,
}

impl std::fmt::Debug for AdminSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AdminSettings")
            .field("enabled", &self.enabled)
            .field("bind", &self.bind)
            .field("token", &"<redacted>")
            .finish()
    }
}

impl AdminSettings {
    /// Reads `Admin.ini` from `config_dir`, then the supervisor's token. Missing file -> disabled (no warning).
    pub fn load(config_dir: &Path) -> Self {
        Self::load_file(config_dir).with_supervisor_token(std::env::var(l2_admin_api::ADMIN_TOKEN_ENV).ok())
    }

    /// D-56: a supervisor hands its admin token to the servers it starts, so `Admin.ini` needs
    /// none. A token written in `Admin.ini` still wins (a hand-made setup).
    #[must_use]
    pub fn with_supervisor_token(mut self, token: Option<String>) -> Self {
        let token = token.map(|t| t.trim().to_owned()).filter(|t| !t.is_empty());
        if let Some(token) = token
            && self.token.is_empty()
        {
            self.enabled = true;
            self.token = token;
        }
        self
    }

    fn load_file(config_dir: &Path) -> Self {
        let path = config_dir.join(ADMIN_CONFIG_FILE);
        let disabled = Self {
            enabled: false,
            bind: DEFAULT_BIND.into(),
            token: String::new(),
        };
        if !path.exists() {
            return disabled;
        }
        match IniFile::load(&path) {
            Ok(ini) => Self {
                enabled: ini.get_bool("Enabled", false),
                bind: ini.get_string("Bind", DEFAULT_BIND).trim().to_owned(),
                token: ini.get_string("Token", "").trim().to_owned(),
            },
            Err(e) => {
                warn!("Cannot read {}: {e}; admin endpoint disabled.", path.display());
                disabled
            }
        }
    }
}

/// Starts the endpoint when `Admin.ini` enables it.
pub async fn start(server: &mut Server, config_dir: &Path) -> Result<(), StartupError> {
    let settings = AdminSettings::load(config_dir);
    if !settings.enabled {
        return Ok(());
    }
    if settings.token.is_empty() {
        warn!(
            "Admin endpoint enabled but Token is empty in {ADMIN_CONFIG_FILE}: it will refuse every connection."
        );
    }
    let listener = TcpListener::bind(&settings.bind)
        .await
        .map_err(|source| StartupError::Listen {
            what: "Admin",
            address: settings.bind.clone(),
            source,
        })?;
    server.admin_addr = listener.local_addr().ok();
    info!("Admin endpoint listening on {}.", settings.bind);
    let handler = Arc::new(GameAdmin {
        state: Arc::clone(server.state()),
    });
    server.spawn(l2_admin_api::serve(
        listener,
        "game".into(),
        Token::new(settings.token),
        handler,
    ));
    Ok(())
}

struct GameAdmin {
    state: Arc<GameState>,
}

impl AdminHandler for GameAdmin {
    async fn handle(&self, request: Request, _session: &Session) -> Response {
        match request {
            Request::Shutdown { restart } => {
                info!(
                    "Shutdown requested through the admin endpoint{}.",
                    if restart { " (restart)" } else { "" }
                );
                self.state.request_shutdown(restart);
                Response::Ok
            }
            Request::GetGameStatus { .. } => match online_players(&self.state).await {
                Ok(players) => Response::GameStatus {
                    status: game_status(players),
                },
                Err(message) => Response::Error { message },
            },
            Request::GameAdmin { action, .. } => game_action(&self.state, action).await,
            _ => Response::Error {
                message: "not supported by the game server".into(),
            },
        }
    }
}

/// The longest reason, message or command the endpoint passes on.
const MAX_TEXT_CHARS: usize = 400;

/// Runs one GM action in the world and turns the answer into a response. Every action is a
/// `//command` of the virtual GM (see `l2_world::admin::console`), so it takes the same code
/// path as a GM's chat command.
async fn game_action(state: &GameState, action: GameAction) -> Response {
    let request = match build_request(state, action).await {
        Ok(request) => request,
        Err(message) => return Response::Error { message },
    };
    match gui_query(state, request).await {
        Ok(GuiReply::Lines(lines)) => Response::GameReply { lines },
        Ok(GuiReply::Info(info)) => Response::GameInfo {
            info: GameInfo {
                uptime_secs: info.uptime_secs,
                online: info.online,
                memory_bytes: info.memory_bytes,
                threads: thread_count(),
                ticks: info.ticks,
                tick_ms: info.tick_ms,
                tick_stats: l2_admin_api::TickStats {
                    avg_us: info.tick_avg_us,
                    p99_us: info.tick_p99_us,
                    samples: info.tick_samples,
                },
                shutdown_in_secs: info.shutdown_in_secs,
            },
        },
        Err(message) => Response::Error { message },
    }
}

/// One line of text: control characters become spaces, the ends are trimmed, the length is capped.
fn one_line(text: &str) -> String {
    let cleaned: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    cleaned.trim().chars().take(MAX_TEXT_CHARS).collect()
}

/// A character or account name as a command word: no spaces or control characters.
fn command_word(name: &str) -> Result<&str, String> {
    let name = name.trim();
    if name.is_empty() || name.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(format!("'{name}' is not a usable name"));
    }
    Ok(name)
}

fn gui_command(command: String, with_html: bool) -> GuiRequest {
    GuiRequest::Command { command, with_html }
}

async fn build_request(state: &GameState, action: GameAction) -> Result<GuiRequest, String> {
    match action {
        GameAction::Player { name, action } => {
            let name = command_word(&name)?.to_owned();
            match action {
                PlayerAction::Kick => Ok(gui_command(format!("kick {name}"), false)),
                PlayerAction::Jail { minutes, reason } => Ok(punishment(
                    &name,
                    "CHARACTER",
                    "JAIL",
                    minutes,
                    &reason,
                    "Jailed by admin",
                )),
                PlayerAction::BanChar { minutes, reason } => Ok(punishment(
                    &name,
                    "CHARACTER",
                    "BAN",
                    minutes,
                    &reason,
                    "Banned by admin",
                )),
                PlayerAction::ChatBan { minutes, reason } => Ok(punishment(
                    &name,
                    "CHARACTER",
                    "CHAT_BAN",
                    minutes,
                    &reason,
                    "Chat banned by admin",
                )),
                PlayerAction::BanAccount { minutes, reason } => {
                    // The account punishment is keyed by the account name, not the character's.
                    let players = online_players(state).await?;
                    let account = players
                        .iter()
                        .find(|p| p.name.eq_ignore_ascii_case(&name))
                        .map(|p| p.account.clone())
                        .ok_or_else(|| format!("{name} is not online."))?;
                    let account = command_word(&account)?;
                    Ok(punishment(
                        account,
                        "ACCOUNT",
                        "BAN",
                        minutes,
                        &reason,
                        "Banned by admin",
                    ))
                }
                PlayerAction::Teleport { town } => {
                    let (_, x, y, z) = TOWNS
                        .iter()
                        .find(|(n, ..)| n.eq_ignore_ascii_case(town.trim()))
                        .copied()
                        .ok_or_else(|| format!("unknown town '{town}'"))?;
                    Ok(GuiRequest::Teleport { name, x, y, z })
                }
                PlayerAction::Message { text } => {
                    let text = one_line(&text);
                    if text.is_empty() {
                        return Err("the message is empty".into());
                    }
                    Ok(GuiRequest::Message { name, text })
                }
            }
        }
        GameAction::Announce { text, critical } => {
            let text = one_line(&text);
            if text.is_empty() {
                return Err("the announcement is empty".into());
            }
            let word = if critical { "announce_crit" } else { "announce" };
            Ok(gui_command(format!("{word} {text}"), false))
        }
        GameAction::Command { command } => {
            let command = one_line(&command);
            if command.trim_start_matches('/').trim().is_empty() {
                return Err("the command is empty".into());
            }
            Ok(gui_command(command, true))
        }
        GameAction::Shutdown { seconds, restart } => {
            let word = if restart {
                "server_restart"
            } else {
                "server_shutdown"
            };
            Ok(gui_command(format!("{word} {seconds}"), false))
        }
        GameAction::AbortShutdown => Ok(gui_command("server_abort".into(), false)),
        GameAction::ServerInfo => Ok(GuiRequest::Info),
    }
}

/// `//punishment_add <key> <affect> <type> <minutes> <reason>`: what the `//jail` and `//ban_*`
/// shortcuts call, with minutes 0 and a fixed reason. 0 minutes = forever.
fn punishment(
    key: &str,
    affect: &str,
    kind: &str,
    minutes: u32,
    reason: &str,
    default_reason: &str,
) -> GuiRequest {
    let reason = one_line(reason);
    let reason = if reason.is_empty() {
        default_reason
    } else {
        &reason
    };
    gui_command(
        format!("punishment_add {key} {affect} {kind} {minutes} {reason}"),
        false,
    )
}

/// The threads of this process (Linux reads `/proc`; elsewhere the standard library cannot tell,
/// so 0).
fn thread_count() -> u32 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|text| {
            text.lines()
                .find_map(|line| line.strip_prefix("Threads:"))
                .and_then(|n| n.trim().parse().ok())
        })
        .unwrap_or(0)
}

/// Sends a GUI request to the world and waits for the answer (same pattern as
/// [`online_players`]).
async fn gui_query(state: &GameState, request: GuiRequest) -> Result<GuiReply, String> {
    let (reply, answer) = tokio::sync::oneshot::channel();
    let query = AdminQuery::Gui(
        request,
        Box::new(move |answer| {
            let _ = reply.send(answer);
        }),
    );
    if !state.world.send(WorldCommand::Admin(query)) {
        return Err("the world is not running".into());
    }
    match tokio::time::timeout(WORLD_ANSWER_TIMEOUT, answer).await {
        Ok(Ok(answer)) => Ok(answer),
        Ok(Err(_)) => Err("the world is shutting down".into()),
        Err(_) => Err("the world did not answer in time".into()),
    }
}

/// Asks the world for the online characters. The world calls the callback on its own thread
/// with the answer, so the callback only hands the value over a oneshot channel.
async fn online_players(state: &GameState) -> Result<Vec<OnlinePlayer>, String> {
    let (reply, answer) = tokio::sync::oneshot::channel();
    let query = AdminQuery::OnlinePlayers(Box::new(move |list| {
        // The asker may have given up already; the list is then not needed.
        let _ = reply.send(list);
    }));
    if !state.world.send(WorldCommand::Admin(query)) {
        return Err("the world is not running".into());
    }
    match tokio::time::timeout(WORLD_ANSWER_TIMEOUT, answer).await {
        Ok(Ok(list)) => Ok(list),
        // The world drops queued queries when it stops, so the callback never runs.
        Ok(Err(_)) => Err("the world is shutting down".into()),
        Err(_) => Err("the world did not answer in time".into()),
    }
}

/// The admin view of the world's online list (the world already sorts it by name).
pub fn game_status(players: Vec<OnlinePlayer>) -> GameStatus {
    GameStatus {
        players: players
            .into_iter()
            .map(|p| GamePlayer {
                name: p.name,
                account: p.account,
                level: p.level,
                class_id: p.class_id,
                ip: p.ip.map(|ip| ip.to_string()),
                online_since_ms: p.online_since_unix_ms,
                clan: p.clan,
                x: p.x,
                y: p.y,
                z: p.z,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use std::net::IpAddr;

    use l2_world::{LeaveReason, ObjectId, PacketSink, PlayerData, SessionId};

    use super::*;

    #[test]
    fn the_supervisor_token_turns_the_endpoint_on_unless_admin_ini_has_one() {
        let dir = tempfile::tempdir().unwrap();
        let from_env = AdminSettings::load_file(dir.path()).with_supervisor_token(Some(" abc ".into()));
        assert!(from_env.enabled);
        assert_eq!(from_env.token, "abc");

        let blank = AdminSettings::load_file(dir.path()).with_supervisor_token(Some("  ".into()));
        assert!(!blank.enabled);

        std::fs::write(
            dir.path().join(ADMIN_CONFIG_FILE),
            "Enabled = True
Token = own
",
        )
        .unwrap();
        let own = AdminSettings::load_file(dir.path()).with_supervisor_token(Some("abc".into()));
        assert_eq!(own.token, "own");
    }

    #[test]
    fn missing_file_means_disabled() {
        let dir = tempfile::tempdir().unwrap();
        let s = AdminSettings::load(dir.path());
        assert!(!s.enabled);
        assert_eq!(s.bind, DEFAULT_BIND);
    }

    #[test]
    fn reads_settings_and_hides_token() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(ADMIN_CONFIG_FILE),
            "Enabled = True\nBind = 127.0.0.1:0\nToken = s3cret\n",
        )
        .unwrap();
        let s = AdminSettings::load(dir.path());
        assert!(s.enabled);
        assert_eq!(s.bind, "127.0.0.1:0");
        assert_eq!(s.token, "s3cret");
        assert!(!format!("{s:?}").contains("s3cret"));
    }

    #[tokio::test]
    async fn shutdown_request_over_the_protocol_stops_the_server() {
        use crate::state::ExitReason;
        use crate::test_support::idle_server;
        use l2_admin_api::AdminClient;

        let config = tempfile::tempdir().unwrap();
        std::fs::write(
            config.path().join(ADMIN_CONFIG_FILE),
            "Enabled = True
Bind = 127.0.0.1:0
Token = t
",
        )
        .unwrap();
        let mut server = idle_server();
        start(&mut server, config.path()).await.unwrap();
        let addr = server.admin_addr().unwrap();

        let (client, _events) = AdminClient::connect(&format!("ws://{addr}"), "t").await.unwrap();
        // Requests for other servers are refused, not ignored.
        let reply = client
            .request(Request::GetLoginStatus { server: None })
            .await
            .unwrap();
        assert!(matches!(reply, Response::Error { .. }));
        let reply = client.request(Request::Shutdown { restart: true }).await.unwrap();
        assert_eq!(reply, Response::Ok);
        let reason = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            server.run_until(std::future::pending::<()>()),
        )
        .await
        .unwrap();
        assert_eq!(reason, ExitReason::Restart);
    }

    struct NullSink;

    impl PacketSink for NullSink {
        fn send(&self, _bytes: Vec<u8>) {}
        fn left_world(&self, _reason: LeaveReason) {}
    }

    fn player_data(id: i32, name: &str) -> PlayerData {
        PlayerData {
            object_id: ObjectId(id),
            account: "acc".into(),
            name: name.into(),
            title: String::new(),
            class_id: 88,
            base_class: 88,
            race: 0,
            sex: 0,
            face: 0,
            hair_style: 0,
            hair_color: 0,
            level: 1,
            exp: 0,
            sp: 0,
            karma: 0,
            pvp_kills: 0,
            pk_kills: 0,
            access_level: 0,
            x: 0,
            y: 0,
            z: 0,
            heading: 0,
            cur_hp: 100.0,
            max_hp: 100.0,
            cur_mp: 50.0,
            max_mp: 50.0,
            cur_cp: 0.0,
            max_cp: 50.0,
            online_time: 0,
            items: vec![],
            skills: vec![],
            shortcuts: vec![],
            profile: Default::default(),
        }
    }

    #[tokio::test]
    async fn game_status_lists_the_characters_over_the_protocol() {
        use crate::test_support::idle_server;
        use l2_admin_api::AdminClient;

        let config = tempfile::tempdir().unwrap();
        std::fs::write(
            config.path().join(ADMIN_CONFIG_FILE),
            "Enabled = True\nBind = 127.0.0.1:0\nToken = t\n",
        )
        .unwrap();
        let mut server = idle_server();
        start(&mut server, config.path()).await.unwrap();
        let addr = server.admin_addr().unwrap();
        let (client, _events) = AdminClient::connect(&format!("ws://{addr}"), "t").await.unwrap();

        let empty = client
            .request(Request::GetGameStatus { server: None })
            .await
            .unwrap();
        assert_eq!(
            empty,
            Response::GameStatus {
                status: GameStatus { players: vec![] }
            }
        );

        // Sent before the query on the same channel, so the world enters them first.
        for (session, id, name, ip) in [
            (1, 0x6000_0001, "Zed", Some(IpAddr::from([127, 0, 0, 1]))),
            (2, 0x6000_0002, "Amy", None),
        ] {
            assert!(server.state().world.send(WorldCommand::EnterWorld {
                session: SessionId(session),
                player: Box::new(player_data(id, name)),
                sink: Box::new(NullSink),
                ip,
            }));
        }

        let reply = client
            .request(Request::GetGameStatus { server: None })
            .await
            .unwrap();
        let Response::GameStatus { status } = reply else {
            panic!("expected a game status, got {reply:?}");
        };
        let names: Vec<&str> = status.players.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["Amy", "Zed"]);
        assert_eq!(status.players[0].ip, None);
        assert_eq!(status.players[1].ip.as_deref(), Some("127.0.0.1"));
        assert_eq!(status.players[1].account, "acc");
        assert_eq!(status.players[1].class_id, 88);
        assert!(status.players[1].online_since_ms > 0);
        server.shutdown(false).await;
    }

    #[test]
    fn game_status_maps_the_world_list() {
        let status = game_status(vec![OnlinePlayer {
            object_id: ObjectId(0x6000_0001),
            account: "acc".into(),
            name: "Amy".into(),
            level: 80,
            class_id: 88,
            online_secs: 5,
            online_since_unix_ms: 1_700_000_000_000,
            ip: Some(IpAddr::from([10, 0, 0, 7])),
            clan: Some("Bears".into()),
            x: 1,
            y: 2,
            z: 3,
        }]);
        assert_eq!(
            status,
            GameStatus {
                players: vec![GamePlayer {
                    name: "Amy".into(),
                    account: "acc".into(),
                    level: 80,
                    class_id: 88,
                    ip: Some("10.0.0.7".into()),
                    online_since_ms: 1_700_000_000_000,
                    clan: Some("Bears".into()),
                    x: 1,
                    y: 2,
                    z: 3,
                }]
            }
        );
    }

    /// Static data whose master level may use the commands the GUI sends.
    struct GmData {
        rights: l2_world::AdminRights,
    }

    impl l2_world::StaticData for GmData {
        fn item(&self, _item_id: i32) -> Option<l2_world::ItemStatic> {
            None
        }
        fn skill_is_passive(&self, _skill_id: i32) -> bool {
            false
        }
        fn exp_percent(&self, _exp: i64, _level: i32) -> f64 {
            0.0
        }
        fn access_level(&self, level: i32) -> l2_world::AccessLevelStatic {
            if level >= 100 {
                l2_world::AccessLevelStatic {
                    name_color: 0,
                    title_color: 0,
                    is_gm: true,
                }
            } else {
                l2_world::AccessLevelStatic::USER
            }
        }
        fn admin_rights(&self) -> Option<&l2_world::AdminRights> {
            Some(&self.rights)
        }
        fn master_access_level(&self) -> i32 {
            100
        }
    }

    fn gm_data() -> GmData {
        let mut rights = l2_world::AdminRights::default();
        rights.add_level(0, 0);
        rights.add_level(100, 0);
        for command in [
            "kick",
            "announce",
            "announce_crit",
            "server_shutdown",
            "server_restart",
            "server_abort",
            "punishment_add",
            "teleport_character",
        ] {
            rights.add_command(command, 100, false);
        }
        GmData { rights }
    }

    /// An endpoint on port 0 over a world that knows GM rights, with two characters in it.
    async fn gm_endpoint() -> (crate::Server, l2_admin_api::AdminClient, tempfile::TempDir) {
        use crate::test_support::idle_server_with;
        use l2_admin_api::AdminClient;

        let config = tempfile::tempdir().unwrap();
        std::fs::write(
            config.path().join(ADMIN_CONFIG_FILE),
            "Enabled = True\nBind = 127.0.0.1:0\nToken = t\n",
        )
        .unwrap();
        let mut server = idle_server_with(|world| world.set_static_data(Box::new(gm_data())));
        start(&mut server, config.path()).await.unwrap();
        let addr = server.admin_addr().unwrap();
        let (client, _events) = AdminClient::connect(&format!("ws://{addr}"), "t").await.unwrap();
        for (session, id, name) in [(1, 0x6000_0001, "Zed"), (2, 0x6000_0002, "Amy")] {
            assert!(server.state().world.send(WorldCommand::EnterWorld {
                session: SessionId(session),
                player: Box::new(player_data(id, name)),
                sink: Box::new(NullSink),
                ip: None,
            }));
        }
        (server, client, config)
    }

    async fn game_request(client: &l2_admin_api::AdminClient, action: GameAction) -> Response {
        client
            .request(Request::GameAdmin { server: None, action })
            .await
            .unwrap()
    }

    async fn names(client: &l2_admin_api::AdminClient) -> Vec<String> {
        let Response::GameStatus { status } = client
            .request(Request::GetGameStatus { server: None })
            .await
            .unwrap()
        else {
            panic!("expected a game status");
        };
        status.players.into_iter().map(|p| p.name).collect()
    }

    fn lines(response: Response) -> Vec<String> {
        match response {
            Response::GameReply { lines } => lines,
            other => panic!("expected a reply, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn the_endpoint_kicks_bans_and_jails_through_the_world() {
        let (server, client, _config) = gm_endpoint().await;
        let kick = PlayerAction::Kick;
        let reply = game_request(
            &client,
            GameAction::Player {
                name: "Zed".into(),
                action: kick,
            },
        )
        .await;
        assert_eq!(lines(reply), ["You kicked Zed from the game."]);
        assert_eq!(names(&client).await, ["Amy"]);

        let jail = PlayerAction::Jail {
            minutes: 30,
            reason: "botting".into(),
        };
        let reply = game_request(
            &client,
            GameAction::Player {
                name: "Amy".into(),
                action: jail,
            },
        )
        .await;
        assert_eq!(
            lines(reply),
            ["Punishment JAIL have been applied to: CHARACTER Amy!"]
        );

        // A ban kicks the character, as `//ban_char` does.
        let ban = PlayerAction::BanChar {
            minutes: 0,
            reason: String::new(),
        };
        let reply = game_request(
            &client,
            GameAction::Player {
                name: "Amy".into(),
                action: ban,
            },
        )
        .await;
        assert_eq!(
            lines(reply),
            ["Punishment BAN have been applied to: CHARACTER Amy!"]
        );
        assert!(names(&client).await.is_empty());
        server.shutdown(false).await;
    }

    #[tokio::test]
    async fn the_endpoint_announces_and_runs_console_commands() {
        let (server, client, _config) = gm_endpoint().await;
        let reply = game_request(
            &client,
            GameAction::Announce {
                text: "Maintenance soon".into(),
                critical: true,
            },
        )
        .await;
        // The virtual GM is online while the command runs, so it hears the announcement too.
        assert_eq!(lines(reply), ["[critical announcement] Maintenance soon"]);
        let reply = game_request(
            &client,
            GameAction::Command {
                command: "nonsense".into(),
            },
        )
        .await;
        assert_eq!(lines(reply), ["The command 'nonsense' does not exist!"]);

        // Bad input is refused before it reaches the world.
        for action in [
            GameAction::Announce {
                text: "  ".into(),
                critical: false,
            },
            GameAction::Command { command: "//".into() },
            GameAction::Player {
                name: "two words".into(),
                action: PlayerAction::Kick,
            },
            GameAction::Player {
                name: "Amy".into(),
                action: PlayerAction::Teleport {
                    town: "Atlantis".into(),
                },
            },
        ] {
            assert!(matches!(
                game_request(&client, action).await,
                Response::Error { .. }
            ));
        }
        server.shutdown(false).await;
    }

    #[tokio::test]
    async fn the_endpoint_counts_down_and_aborts() {
        let (server, client, _config) = gm_endpoint().await;
        let reply = game_request(
            &client,
            GameAction::Shutdown {
                seconds: 120,
                restart: false,
            },
        )
        .await;
        assert_eq!(
            lines(reply),
            ["The server will be coming down in 120 second(s). Please find a safe place to log out."]
        );
        let Response::GameInfo { info } = game_request(&client, GameAction::ServerInfo).await else {
            panic!("expected the server info");
        };
        assert_eq!(info.online, 2);
        assert!(info.shutdown_in_secs.is_some());
        assert_eq!(info.tick_ms, 50);

        let reply = game_request(&client, GameAction::AbortShutdown).await;
        assert_eq!(
            lines(reply),
            ["[announcement] Server aborts aborting and continues normal operation!"]
        );
        server.shutdown(false).await;
    }

    #[tokio::test]
    async fn the_endpoint_teleports_to_a_town_and_whispers() {
        let (server, client, _config) = gm_endpoint().await;
        let reply = game_request(
            &client,
            GameAction::Player {
                name: "amy".into(),
                action: PlayerAction::Teleport { town: "Giran".into() },
            },
        )
        .await;
        assert_eq!(lines(reply), Vec::<String>::new());
        let reply = game_request(
            &client,
            GameAction::Player {
                name: "Amy".into(),
                action: PlayerAction::Message { text: "hi".into() },
            },
        )
        .await;
        assert_eq!(lines(reply), ["Message sent to Amy."]);
        server.shutdown(false).await;
    }

    #[test]
    fn requests_are_built_from_one_clean_line() {
        assert_eq!(one_line("a\nb\tc  "), "a b c");
        assert_eq!(one_line(&"x".repeat(500)).chars().count(), MAX_TEXT_CHARS);
        assert!(command_word("Amy").is_ok());
        assert!(command_word("A my").is_err());
        assert!(command_word("").is_err());
        assert_eq!(
            punishment(
                "Amy",
                "CHARACTER",
                "JAIL",
                30,
                " caught\nbotting ",
                "Jailed by admin"
            ),
            GuiRequest::Command {
                command: "punishment_add Amy CHARACTER JAIL 30 caught botting".into(),
                with_html: false
            }
        );
        // The shortcuts' own reason when the GM gave none.
        assert_eq!(
            punishment("acc", "ACCOUNT", "BAN", 0, "", "Banned by admin"),
            GuiRequest::Command {
                command: "punishment_add acc ACCOUNT BAN 0 Banned by admin".into(),
                with_html: false
            }
        );
    }

    #[tokio::test]
    async fn disabled_endpoint_binds_nothing() {
        use crate::test_support::idle_server;

        let config = tempfile::tempdir().unwrap();
        let mut server = idle_server();
        start(&mut server, config.path()).await.unwrap();
        assert_eq!(server.admin_addr(), None);
        server.shutdown(false).await;
    }
}
