//! Local admin endpoint: a clean shutdown for the supervisor/GUI (D-30, D-38).
//!
//! Java: `Shutdown.startShutdown` (the GM command) is the same action; the Java game server
//! has no remote endpoint. Settings come from the Rust-only `config/Admin.ini` (D-36); a missing
//! file means disabled. Same pattern as `login-server/src/admin.rs`.
//! Status and player queries join with the cards that have something to show.

use std::path::Path;
use std::sync::Arc;

use l2_admin_api::{AdminHandler, Request, Response, Session, Token};
use l2_config::IniFile;
use tokio::net::TcpListener;
use tracing::{info, warn};

use crate::state::GameState;
use crate::{Server, StartupError};

/// File name inside the config directory.
pub const ADMIN_CONFIG_FILE: &str = "Admin.ini";
/// Default bind address (the login server uses 7901).
pub const DEFAULT_BIND: &str = "127.0.0.1:7902";

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
            _ => Response::Error {
                message: "not supported by the game server".into(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
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
