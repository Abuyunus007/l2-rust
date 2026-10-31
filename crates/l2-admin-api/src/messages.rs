//! Protocol messages. Every frame is one [`Message`] as JSON.
//!
//! Adding a feature = adding a variant to [`Request`], [`Response`] or [`Event`] and documenting
//! it in `docs/admin-api.md`. Field names are snake_case; enums are tagged with `"kind"`.

use serde::{Deserialize, Serialize};

/// Bumped on incompatible changes. Client and server must agree.
pub const PROTOCOL_VERSION: u32 = 1;

/// One WebSocket text frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    /// First message from the connecting side.
    Hello {
        version: u32,
        token: String,
    },
    /// The listener accepted the hello. `name` identifies the endpoint ("supervisor", "login").
    Welcome {
        version: u32,
        name: String,
    },
    /// The listener rejected the connection (wrong token or version). The connection closes.
    Rejected {
        reason: String,
    },
    Request {
        id: u64,
        request: Request,
    },
    Response {
        id: u64,
        response: Response,
    },
    Event {
        event: Event,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Request {
    /// Supervisor: all configured servers and their state.
    ListServers,
    StartServer {
        name: String,
    },
    StopServer {
        name: String,
    },
    RestartServer {
        name: String,
    },
    /// Supervisor: stream [`Event::LogLine`]s. `server: None` = all servers. `since` = the last
    /// `seq` the client already has; the backlog after it is sent first.
    SubscribeLogs {
        server: Option<String>,
        since: Option<u64>,
    },
    UnsubscribeLogs,
    /// Login server (relayed by the supervisor with `server` set): current status.
    GetLoginStatus {
        server: Option<String>,
    },
    /// Game server endpoint: the online characters. The supervisor relays it to the named server.
    GetGameStatus {
        server: Option<String>,
    },
    /// Login server (relayed by the supervisor with `server` set): account management (G-04).
    Accounts {
        server: Option<String>,
        action: AccountsAction,
    },
    /// Server endpoint: shut down cleanly (D-38). `restart: true` = exit code 2. The supervisor
    /// sends this before it falls back to a kill.
    Shutdown {
        restart: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Response {
    Ok,
    Servers { servers: Vec<ServerStatus> },
    LoginStatus { status: LoginStatus },
    GameStatus { status: GameStatus },
    Accounts { result: AccountsResult },
    Error { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    LogLine { line: LogLine },
    ServerStateChanged { status: ServerStatus },
    LoginStatusChanged { server: String, status: LoginStatus },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServerState {
    Stopped,
    Starting,
    Running,
    Stopping,
    /// Exited with an unexpected code and isn't being restarted.
    Crashed,
}

/// A supervised process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerStatus {
    pub name: String,
    pub state: ServerState,
    pub pid: Option<u32>,
    pub uptime_secs: Option<u64>,
    pub last_exit_code: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

/// One line of a server's output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogLine {
    pub server: String,
    /// Per-server sequence number, increasing. Used for `SubscribeLogs.since`.
    pub seq: u64,
    pub ts_ms: i64,
    /// Parsed from the line when its format is known.
    pub level: Option<LogLevel>,
    pub target: Option<String>,
    pub message: String,
}

/// Login server status (Java: what `loginserver/ui/Gui.java` shows).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoginStatus {
    pub game_servers: Vec<GameServerStatus>,
    /// Accounts authenticated on the LS that haven't entered a game server yet.
    pub authed_clients: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameServerStatus {
    pub id: u8,
    pub name: String,
    /// Connected and registered right now.
    pub authed: bool,
    pub online: u32,
    pub max_players: u32,
    /// Java `GameServerInfo.getStatusName()`: "Auto", "Good", "Normal", "Full", "Down", "GM Only".
    pub status: String,
}

/// Game server status: the characters in the world right now, sorted by name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameStatus {
    pub players: Vec<GamePlayer>,
}

/// One character in the world.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GamePlayer {
    pub name: String,
    pub account: String,
    pub level: i32,
    pub class_id: i32,
    /// The client's address. `None` when the session didn't report one.
    pub ip: Option<String>,
    /// Unix time in milliseconds at which the character entered the world.
    pub online_since_ms: i64,
}

/// Accounts per page of [`AccountsAction::Search`].
pub const ACCOUNTS_PAGE_SIZE: u32 = 50;

/// What the Accounts view asks the login server to do. Passwords travel in clear text over the
/// admin connection (token protected, D-30) and are never logged: `Debug` hides them.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum AccountsAction {
    /// Logins starting with `prefix` (empty = all), [`ACCOUNTS_PAGE_SIZE`] per page, `page` from 0.
    Search {
        prefix: String,
        page: u32,
    },
    /// The `lastIP` of one account (shown on demand).
    LastIp {
        login: String,
    },
    Create {
        login: String,
        password: String,
        access_level: i32,
    },
    SetPassword {
        login: String,
        password: String,
    },
    SetAccessLevel {
        login: String,
        access_level: i32,
    },
    /// Sets `accessLevel = -1` (what Java's AccountManager does).
    Ban {
        login: String,
    },
    /// Back to access level 0 (Java's AccountManager has no unban; it sets "0 - User").
    Unban {
        login: String,
    },
    /// Deletes the `accounts` row only, like Java's AccountManager.
    Delete {
        login: String,
    },
}

impl std::fmt::Debug for AccountsAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Search { prefix, page } => f
                .debug_struct("Search")
                .field("prefix", prefix)
                .field("page", page)
                .finish(),
            Self::LastIp { login } => f.debug_struct("LastIp").field("login", login).finish(),
            Self::Create {
                login, access_level, ..
            } => f
                .debug_struct("Create")
                .field("login", login)
                .field("password", &"<redacted>")
                .field("access_level", access_level)
                .finish(),
            Self::SetPassword { login, .. } => f
                .debug_struct("SetPassword")
                .field("login", login)
                .field("password", &"<redacted>")
                .finish(),
            Self::SetAccessLevel { login, access_level } => f
                .debug_struct("SetAccessLevel")
                .field("login", login)
                .field("access_level", access_level)
                .finish(),
            Self::Ban { login } => f.debug_struct("Ban").field("login", login).finish(),
            Self::Unban { login } => f.debug_struct("Unban").field("login", login).finish(),
            Self::Delete { login } => f.debug_struct("Delete").field("login", login).finish(),
        }
    }
}

/// Answer to [`Request::Accounts`]. Failures come back as [`Response::Error`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AccountsResult {
    Page(AccountPage),
    /// `ip: None` = the account never logged in.
    LastIp {
        login: String,
        ip: Option<String>,
    },
    /// The action worked. `access_level` is the new level after ban, unban and set-level.
    Done {
        login: String,
        access_level: Option<i32>,
    },
}

/// One page of the account list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountPage {
    pub page: u32,
    pub page_size: u32,
    /// Accounts matching the prefix, over all pages.
    pub total: u64,
    pub accounts: Vec<AccountSummary>,
}

/// A row of the account list. Never carries the password hash or the IP.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountSummary {
    pub login: String,
    pub access_level: i32,
    /// Unix time in milliseconds of the last login, 0 = never.
    pub last_active_ms: i64,
    pub last_server: Option<i32>,
    /// `access_level < 0`.
    pub banned: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_never_shows_passwords() {
        let create = AccountsAction::Create {
            login: "bob".into(),
            password: "hunter2".into(),
            access_level: 0,
        };
        let set = AccountsAction::SetPassword {
            login: "bob".into(),
            password: "hunter2".into(),
        };
        let request = Request::Accounts {
            server: None,
            action: create.clone(),
        };
        for text in [format!("{create:?}"), format!("{set:?}"), format!("{request:?}")] {
            assert!(!text.contains("hunter2"), "{text}");
            assert!(text.contains("bob"));
        }
    }

    /// Snapshot of the JSON for every message shape. A change here is a protocol change:
    /// update `docs/admin-api.md` and consider bumping [`PROTOCOL_VERSION`].
    #[test]
    fn json_snapshots() {
        let status = ServerStatus {
            name: "login".into(),
            state: ServerState::Running,
            pid: Some(4242),
            uptime_secs: Some(60),
            last_exit_code: None,
        };
        let login = LoginStatus {
            game_servers: vec![GameServerStatus {
                id: 1,
                name: "Bartz".into(),
                authed: true,
                online: 3,
                max_players: 2000,
                status: "Normal".into(),
            }],
            authed_clients: 1,
        };
        let game = GameStatus {
            players: vec![GamePlayer {
                name: "Amy".into(),
                account: "acc1".into(),
                level: 80,
                class_id: 88,
                ip: Some("127.0.0.1".into()),
                online_since_ms: 1_700_000_000_000,
            }],
        };
        let line = LogLine {
            server: "login".into(),
            seq: 7,
            ts_ms: 1_700_000_000_000,
            level: Some(LogLevel::Info),
            target: None,
            message: "Loaded 0 IP Bans.".into(),
        };
        let cases: Vec<(Message, &str)> = vec![
            (
                Message::Hello {
                    version: 1,
                    token: "t".into(),
                },
                r#"{"type":"hello","version":1,"token":"t"}"#,
            ),
            (
                Message::Welcome {
                    version: 1,
                    name: "supervisor".into(),
                },
                r#"{"type":"welcome","version":1,"name":"supervisor"}"#,
            ),
            (
                Message::Rejected {
                    reason: "bad token".into(),
                },
                r#"{"type":"rejected","reason":"bad token"}"#,
            ),
            (
                Message::Request {
                    id: 1,
                    request: Request::ListServers,
                },
                r#"{"type":"request","id":1,"request":{"kind":"list_servers"}}"#,
            ),
            (
                Message::Request {
                    id: 2,
                    request: Request::StartServer { name: "login".into() },
                },
                r#"{"type":"request","id":2,"request":{"kind":"start_server","name":"login"}}"#,
            ),
            (
                Message::Request {
                    id: 3,
                    request: Request::SubscribeLogs {
                        server: None,
                        since: Some(5),
                    },
                },
                r#"{"type":"request","id":3,"request":{"kind":"subscribe_logs","server":null,"since":5}}"#,
            ),
            (
                Message::Request {
                    id: 6,
                    request: Request::Shutdown { restart: false },
                },
                r#"{"type":"request","id":6,"request":{"kind":"shutdown","restart":false}}"#,
            ),
            (
                Message::Response {
                    id: 1,
                    response: Response::Servers {
                        servers: vec![status.clone()],
                    },
                },
                r#"{"type":"response","id":1,"response":{"kind":"servers","servers":[{"name":"login","state":"running","pid":4242,"uptime_secs":60,"last_exit_code":null}]}}"#,
            ),
            (
                Message::Response {
                    id: 4,
                    response: Response::Error {
                        message: "unknown server".into(),
                    },
                },
                r#"{"type":"response","id":4,"response":{"kind":"error","message":"unknown server"}}"#,
            ),
            (
                Message::Response {
                    id: 5,
                    response: Response::LoginStatus {
                        status: login.clone(),
                    },
                },
                r#"{"type":"response","id":5,"response":{"kind":"login_status","status":{"game_servers":[{"id":1,"name":"Bartz","authed":true,"online":3,"max_players":2000,"status":"Normal"}],"authed_clients":1}}}"#,
            ),
            (
                Message::Request {
                    id: 7,
                    request: Request::GetGameStatus { server: None },
                },
                r#"{"type":"request","id":7,"request":{"kind":"get_game_status","server":null}}"#,
            ),
            (
                Message::Response {
                    id: 7,
                    response: Response::GameStatus { status: game },
                },
                r#"{"type":"response","id":7,"response":{"kind":"game_status","status":{"players":[{"name":"Amy","account":"acc1","level":80,"class_id":88,"ip":"127.0.0.1","online_since_ms":1700000000000}]}}}"#,
            ),
            (
                Message::Request {
                    id: 8,
                    request: Request::Accounts {
                        server: Some("login".into()),
                        action: AccountsAction::Search {
                            prefix: "al".into(),
                            page: 0,
                        },
                    },
                },
                r#"{"type":"request","id":8,"request":{"kind":"accounts","server":"login","action":{"action":"search","prefix":"al","page":0}}}"#,
            ),
            (
                Message::Request {
                    id: 9,
                    request: Request::Accounts {
                        server: None,
                        action: AccountsAction::Create {
                            login: "bob".into(),
                            password: "pw".into(),
                            access_level: 0,
                        },
                    },
                },
                r#"{"type":"request","id":9,"request":{"kind":"accounts","server":null,"action":{"action":"create","login":"bob","password":"pw","access_level":0}}}"#,
            ),
            (
                Message::Response {
                    id: 8,
                    response: Response::Accounts {
                        result: AccountsResult::Page(AccountPage {
                            page: 0,
                            page_size: 50,
                            total: 1,
                            accounts: vec![AccountSummary {
                                login: "alice".into(),
                                access_level: -1,
                                last_active_ms: 5,
                                last_server: Some(1),
                                banned: true,
                            }],
                        }),
                    },
                },
                r#"{"type":"response","id":8,"response":{"kind":"accounts","result":{"kind":"page","page":0,"page_size":50,"total":1,"accounts":[{"login":"alice","access_level":-1,"last_active_ms":5,"last_server":1,"banned":true}]}}}"#,
            ),
            (
                Message::Response {
                    id: 10,
                    response: Response::Accounts {
                        result: AccountsResult::Done {
                            login: "alice".into(),
                            access_level: Some(0),
                        },
                    },
                },
                r#"{"type":"response","id":10,"response":{"kind":"accounts","result":{"kind":"done","login":"alice","access_level":0}}}"#,
            ),
            (
                Message::Event {
                    event: Event::LogLine { line },
                },
                r#"{"type":"event","event":{"kind":"log_line","line":{"server":"login","seq":7,"ts_ms":1700000000000,"level":"info","target":null,"message":"Loaded 0 IP Bans."}}}"#,
            ),
            (
                Message::Event {
                    event: Event::ServerStateChanged { status },
                },
                r#"{"type":"event","event":{"kind":"server_state_changed","status":{"name":"login","state":"running","pid":4242,"uptime_secs":60,"last_exit_code":null}}}"#,
            ),
        ];
        for (msg, json) in cases {
            assert_eq!(serde_json::to_string(&msg).unwrap(), json);
            let back: Message = serde_json::from_str(json).unwrap();
            assert_eq!(back, msg);
        }
    }
}
