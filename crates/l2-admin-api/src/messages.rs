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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Response {
    Ok,
    Servers { servers: Vec<ServerStatus> },
    LoginStatus { status: LoginStatus },
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

#[cfg(test)]
mod tests {
    use super::*;

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
