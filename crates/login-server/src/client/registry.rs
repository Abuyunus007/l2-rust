//! Accounts that are logged in on the login server (not yet on a game server).
//!
//! Java: `LoginController._loginServerClients` (account → `LoginClient`).

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use l2_net::ConnectionHandle;

use super::session_key::SessionKey;

/// What the rest of the server needs to know about an authenticated client.
#[derive(Debug, Clone)]
pub struct AuthedClient {
    pub session_key: SessionKey,
    /// To send to or close the client from another task (kick, purge).
    pub handle: ConnectionHandle,
    /// Unique per connection, so a disconnect only removes its own entry.
    pub connection_id: u64,
    /// When the connection was accepted (Java `getConnectionStartTime`), for the purge.
    pub connected_at_ms: i64,
    pub access_level: i32,
    pub last_server: i32,
}

/// Account name (lower case) → authenticated client.
#[derive(Debug, Default)]
pub struct ClientRegistry {
    clients: Mutex<HashMap<String, AuthedClient>>,
}

impl ClientRegistry {
    fn lock(&self) -> MutexGuard<'_, HashMap<String, AuthedClient>> {
        self.clients.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Java `putIfAbsent`: `false` when the account already has a client.
    pub fn try_insert(&self, account: &str, client: AuthedClient) -> bool {
        let mut clients = self.lock();
        if clients.contains_key(account) {
            return false;
        }
        clients.insert(account.to_owned(), client);
        true
    }

    /// Java `put` (`assignSessionKeyToClient`): replaces any previous entry.
    pub fn insert(&self, account: &str, client: AuthedClient) {
        self.lock().insert(account.to_owned(), client);
    }

    // Java: LoginController.removeAuthedLoginClient
    pub fn remove(&self, account: &str) -> Option<AuthedClient> {
        self.lock().remove(account)
    }

    /// Removes the entry only if it belongs to `connection_id` (a newer login of the same
    /// account stays).
    pub fn remove_if_connection(&self, account: &str, connection_id: u64) -> bool {
        let mut clients = self.lock();
        if clients
            .get(account)
            .is_some_and(|c| c.connection_id == connection_id)
        {
            clients.remove(account);
            return true;
        }
        false
    }

    // Java: LoginController.getAuthedClient
    pub fn get(&self, account: &str) -> Option<AuthedClient> {
        self.lock().get(account).cloned()
    }

    // Java: LoginController.getKeyForAccount
    pub fn session_key(&self, account: &str) -> Option<SessionKey> {
        self.lock().get(account).map(|c| c.session_key)
    }

    pub fn len(&self) -> usize {
        self.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.lock().is_empty()
    }

    /// Removes and returns every entry for which `expired` is true (the purge).
    pub fn take_where(&self, mut expired: impl FnMut(&AuthedClient) -> bool) -> Vec<(String, AuthedClient)> {
        let mut clients = self.lock();
        let keys: Vec<String> = clients
            .iter()
            .filter(|(_, c)| expired(c))
            .map(|(k, _)| k.clone())
            .collect();
        keys.into_iter()
            .filter_map(|k| clients.remove(&k).map(|c| (k, c)))
            .collect()
    }
}
