//! Local admin endpoint: game status (online characters) and a clean shutdown for the
//! supervisor/GUI (D-30, D-38).
//!
//! Java: `Shutdown.startShutdown` (the GM command) is the same action; the Java game server
//! has no remote endpoint. Settings come from the Rust-only `config/Admin.ini` (D-36); a missing
//! file means disabled. Same pattern as `login-server/src/admin.rs`.
//! The character list is read from the world thread (D-10): the endpoint sends an
//! `AdminQuery` and waits for the answer, it never touches the world's state.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use l2_admin_api::{AdminHandler, GamePlayer, GameStatus, Request, Response, Session, Token};
use l2_config::IniFile;
use l2_world::{AdminQuery, OnlinePlayer, WorldCommand};
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
    /// Reads `Admin.ini` from `config_dir`. Missing file -> disabled (no warning).
    pub fn load(config_dir: &Path) -> Self {
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
            _ => Response::Error {
                message: "not supported by the game server".into(),
            },
        }
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
                }]
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
