//! Where the commands go: the admin endpoint of the game server, or a supervisor that relays them.

use std::future::Future;

use l2_admin_api::{AdminClient, GameAction, Request, Response};
use tokio::sync::Mutex;
use tracing::info;

/// Runs a GM action on the game server. A trait so the tests can stand in for the server.
pub trait Backend: Send + Sync {
    fn game_action(&self, action: GameAction) -> impl Future<Output = Result<Response, String>> + Send;
}

/// The admin endpoint (`ws://host:port`) with its token. `server`: the game server's name when the
/// endpoint is a supervisor (`None` for the game server's own endpoint).
pub struct AdminBackend {
    url: String,
    token: String,
    server: Option<String>,
    client: Mutex<Option<AdminClient>>,
}

impl AdminBackend {
    pub fn new(url: String, token: String, server: Option<String>) -> Self {
        Self {
            url,
            token,
            server,
            client: Mutex::new(None),
        }
    }
}

impl Backend for AdminBackend {
    /// Connects on the first command and again after a lost connection (a restarted server), so
    /// the bridge can be started before the server is.
    async fn game_action(&self, action: GameAction) -> Result<Response, String> {
        let mut client = self.client.lock().await;
        let mut last_error = String::new();
        for _ in 0..2 {
            if client.is_none() {
                let (connected, _events) = AdminClient::connect(&self.url, &self.token)
                    .await
                    .map_err(|e| format!("cannot reach the admin endpoint {}: {e}", self.url))?;
                info!("Connected to the admin endpoint {}", connected.endpoint_name());
                *client = Some(connected);
            }
            let Some(connected) = client.as_ref() else {
                continue;
            };
            let request = Request::GameAdmin {
                server: self.server.clone(),
                action: action.clone(),
            };
            match connected.request(request).await {
                Ok(response) => return Ok(response),
                Err(error) => {
                    // A broken connection: once more with a new one.
                    last_error = error.to_string();
                    *client = None;
                }
            }
        }
        Err(format!("the admin endpoint did not answer: {last_error}"))
    }
}
