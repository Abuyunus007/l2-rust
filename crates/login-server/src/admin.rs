//! Local admin endpoint: status for the supervisor/GUI and a clean shutdown (D-30, D-38).
//!
//! Java: the Swing `loginserver/ui/Gui.java` showed the same game server table.
//! Settings come from the Rust-only `config/Admin.ini` (D-36); a missing file means disabled.

use std::collections::HashSet;
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use l2_admin_api::{AdminHandler, Event, GameServerStatus, LoginStatus, Request, Response, Session, Token};
use l2_config::IniFile;
use tokio::net::TcpListener;
use tracing::{info, warn};

use crate::accounts::{AccountStore, AccountsService};
use crate::state::{LoginState, ServerStatus};
use crate::{Server, StartupError};

/// File name inside the config directory.
pub const ADMIN_CONFIG_FILE: &str = "Admin.ini";
/// How often a connected client gets a `login_status_changed` when something changed.
const PUSH_INTERVAL: Duration = Duration::from_secs(2);

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
    /// Reads `Admin.ini` from `config_dir`, then the supervisor's token. Missing file → disabled (no warning).
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
            bind: "127.0.0.1:7901".into(),
            token: String::new(),
        };
        if !path.exists() {
            return disabled;
        }
        match IniFile::load(&path) {
            Ok(ini) => Self {
                enabled: ini.get_bool("Enabled", false),
                bind: ini.get_string("Bind", "127.0.0.1:7901").trim().to_owned(),
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
    info!("Admin endpoint listening on {}.", settings.bind);
    let handler = Arc::new(LoginAdmin {
        state: Arc::clone(server.state()),
        accounts: AccountsService::new(server.state().repository.clone()),
        pushing: Mutex::default(),
    });
    server.spawn(l2_admin_api::serve(
        listener,
        "login".into(),
        Token::new(settings.token),
        handler,
    ));
    Ok(())
}

struct LoginAdmin<S> {
    state: Arc<LoginState>,
    /// The Accounts view's actions (G-04).
    accounts: AccountsService<S>,
    /// Sessions that already have a status pusher.
    pushing: Mutex<HashSet<u64>>,
}

impl<S: AccountStore> LoginAdmin<S> {
    /// After a client's first status request, push changes to it until it disconnects.
    fn ensure_pusher(&self, session: &Session) {
        let first = self
            .pushing
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(session.id());
        if !first {
            return;
        }
        let (state, session) = (Arc::clone(&self.state), session.clone());
        tokio::spawn(async move {
            let mut last = login_status(&state);
            loop {
                tokio::time::sleep(PUSH_INTERVAL).await;
                let now = login_status(&state);
                if now != last {
                    let event = Event::LoginStatusChanged {
                        server: "login".into(),
                        status: now.clone(),
                    };
                    if !session.send_event(event) {
                        break;
                    }
                    last = now;
                } else if session.is_closed() {
                    break;
                }
            }
        });
    }
}

impl<S: AccountStore> AdminHandler for LoginAdmin<S> {
    async fn handle(&self, request: Request, session: &Session) -> Response {
        match request {
            Request::GetLoginStatus { .. } => {
                self.ensure_pusher(session);
                Response::LoginStatus {
                    status: login_status(&self.state),
                }
            }
            Request::Shutdown { restart } => {
                info!(
                    "Shutdown requested through the admin endpoint{}.",
                    if restart { " (restart)" } else { "" }
                );
                self.state.request_shutdown(restart);
                Response::Ok
            }
            // Nothing here logs the request: it holds passwords (its Debug hides them anyway).
            Request::Accounts { action, .. } => match self.accounts.run(action).await {
                Ok(result) => Response::Accounts { result },
                Err(e) => Response::Error {
                    message: e.to_string(),
                },
            },
            _ => Response::Error {
                message: "not supported by the login server".into(),
            },
        }
    }
}

/// Java `GameServerInfo.getStatusName`.
pub fn status_name(status: ServerStatus) -> &'static str {
    match status {
        ServerStatus::Auto => "Auto",
        ServerStatus::Good => "Good",
        ServerStatus::Normal => "Normal",
        ServerStatus::Full => "Full",
        ServerStatus::Down => "Down",
        ServerStatus::GmOnly => "GM Only",
    }
}

/// The current picture: every registered game server and the waiting clients.
pub fn login_status(state: &LoginState) -> LoginStatus {
    let tables = &state.game_servers;
    let game_servers = tables
        .registered_ids()
        .into_iter()
        .filter_map(|id| tables.registered(id))
        .map(|info| GameServerStatus {
            id: u8::try_from(info.id).unwrap_or(0),
            name: tables.server_name(info.id).unwrap_or_default().to_owned(),
            authed: info.authed,
            online: info
                .link
                .as_ref()
                .map_or(0, |link| u32::try_from(link.player_count()).unwrap_or(u32::MAX)),
            max_players: u32::try_from(info.max_players).unwrap_or(0),
            status: status_name(info.status).to_owned(),
        })
        .collect();
    LoginStatus {
        game_servers,
        authed_clients: u32::try_from(state.clients.len()).unwrap_or(u32::MAX),
    }
}

#[cfg(test)]
mod tests {
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
        assert_eq!(s.bind, "127.0.0.1:7901");
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
        use crate::accounts::memory::MemoryAccountStore;
        use crate::state::ExitReason;
        use crate::state::test_support::{default_config, state_with};
        use l2_admin_api::AdminClient;

        let (state, mut requests) = state_with(default_config());
        let handler = Arc::new(LoginAdmin {
            state: Arc::new(state),
            accounts: AccountsService::new(MemoryAccountStore::default()),
            pushing: Mutex::default(),
        });
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(l2_admin_api::serve(
            listener,
            "login".into(),
            Token::new("t"),
            handler,
        ));

        let (client, _events) = AdminClient::connect(&format!("ws://{addr}"), "t").await.unwrap();
        let status = client
            .request(Request::GetLoginStatus { server: None })
            .await
            .unwrap();
        assert!(matches!(status, Response::LoginStatus { .. }));
        let reply = client.request(Request::Shutdown { restart: true }).await.unwrap();
        assert_eq!(reply, Response::Ok);
        assert_eq!(requests.recv().await, Some(ExitReason::Restart));
    }

    #[tokio::test]
    async fn accounts_round_trip_over_the_protocol() {
        use crate::accounts::memory::MemoryAccountStore;
        use crate::state::test_support::{default_config, state_with};
        use l2_admin_api::{AccountsAction, AccountsResult, AdminClient};

        let (state, _requests) = state_with(default_config());
        let handler = Arc::new(LoginAdmin {
            state: Arc::new(state),
            accounts: AccountsService::new(MemoryAccountStore::default()),
            pushing: Mutex::default(),
        });
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(l2_admin_api::serve(
            listener,
            "login".into(),
            Token::new("t"),
            handler,
        ));
        let (client, _events) = AdminClient::connect(&format!("ws://{addr}"), "t").await.unwrap();
        let ask = |action| client.request(Request::Accounts { server: None, action });

        let created = ask(AccountsAction::Create {
            login: "alice".into(),
            password: "pw".into(),
            access_level: 0,
        })
        .await
        .unwrap();
        assert_eq!(
            created,
            Response::Accounts {
                result: AccountsResult::Done {
                    login: "alice".into(),
                    access_level: Some(0)
                }
            }
        );
        let banned = ask(AccountsAction::Ban {
            login: "alice".into(),
        })
        .await
        .unwrap();
        assert!(matches!(banned, Response::Accounts { .. }));
        let listed = ask(AccountsAction::Search {
            prefix: "al".into(),
            page: 0,
        })
        .await
        .unwrap();
        let Response::Accounts {
            result: AccountsResult::Page(page),
        } = listed
        else {
            panic!("{listed:?}")
        };
        assert_eq!(page.total, 1);
        assert!(page.accounts[0].banned);

        let again = ask(AccountsAction::Create {
            login: "alice".into(),
            password: "pw".into(),
            access_level: 0,
        })
        .await
        .unwrap();
        assert_eq!(
            again,
            Response::Error {
                message: "account 'alice' already exists".into()
            }
        );
    }

    #[test]
    fn status_names_like_java() {
        assert_eq!(status_name(ServerStatus::GmOnly), "GM Only");
        assert_eq!(status_name(ServerStatus::Down), "Down");
    }
}
