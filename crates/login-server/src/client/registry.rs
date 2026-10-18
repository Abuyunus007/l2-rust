//! Accounts that are logged in on the login server (not yet on a game server).
//!
//! Java: `LoginController._loginServerClients` (account → `LoginClient`).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use l2_net::{ConnectionHandle, Encode};
use l2_protocol_login::enums::LoginFailReason;
use l2_protocol_login::server::LoginFail;

use super::server_list::CharacterReplies;
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
    /// Set when the client got `PlayOk` (Java `LoginClient._joinedGS`). The purge leaves such
    /// clients alone until the game server reports them (`PlayerInGame`).
    pub joined_gs: Arc<AtomicBool>,
    /// Where the `ReplyCharacters` of the game servers go (Java keeps them on the client).
    pub characters: Arc<CharacterReplies>,
}

impl AuthedClient {
    pub fn has_joined_gs(&self) -> bool {
        self.joined_gs.load(Ordering::Relaxed)
    }
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

    /// Gives the characters one game server reported to the account's waiting client. `false`
    /// when the account is not logged in here.
    // Java: LoginController.setCharactersOnServer
    pub fn set_characters_on_server(
        &self,
        account: &str,
        server_id: i32,
        characters: i32,
        delete_times: &[i64],
    ) -> bool {
        let Some(client) = self.get(account) else {
            return false;
        };
        client
            .characters
            .record_reply(server_id, characters, delete_times);
        true
    }

    /// The periodic cleanup: drops clients that did not join a game server within `timeout_ms`
    /// of connecting, and clients whose connection is gone. Returns how many were dropped.
    // Java: LoginController.purge
    pub fn purge(&self, now_ms: i64, timeout_ms: i64) -> usize {
        let removed = self.take_where(|client| {
            let timed_out =
                !client.has_joined_gs() && client.connected_at_ms.saturating_add(timeout_ms) <= now_ms;
            timed_out || client.handle.is_closed()
        });
        for (_, client) in &removed {
            // NOTE(java-quirk): Java's `close(LoginFailReason)` only sends the packet and relies
            // on the client to disconnect. The card asks for a close, so the connection ends
            // here after the packet.
            client.handle.send_and_close(
                LoginFail {
                    reason: LoginFailReason::AccessFailed,
                }
                .to_bytes(),
            );
        }
        removed.len()
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use std::time::Duration;

    use l2_net::{CancellationToken, ConnectionConfig, Control, FrameHandler, NoCipher, spawn_connection};
    use tokio::io::AsyncReadExt;
    use tokio::net::{TcpListener, TcpStream};

    use super::*;

    struct Idle;

    impl FrameHandler for Idle {
        async fn on_frame(&mut self, _body: Vec<u8>, _ctl: &mut Control<'_>) {}
    }

    /// A live server-side connection (no encryption) and the socket of its far end.
    async fn connection() -> (ConnectionHandle, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap()).await.unwrap();
        let (server, _) = listener.accept().await.unwrap();
        let config = Arc::new(ConnectionConfig {
            drop_packets: false,
            drop_packet_threshold: 250,
            tcp_nodelay: true,
        });
        let handle =
            spawn_connection(server, config, Box::new(NoCipher), Idle, CancellationToken::new()).unwrap();
        (handle, client)
    }

    fn entry(handle: ConnectionHandle, id: u64, connected_at_ms: i64, joined: bool) -> AuthedClient {
        AuthedClient {
            session_key: SessionKey {
                login_ok1: 1,
                login_ok2: 2,
                play_ok1: 3,
                play_ok2: 4,
            },
            handle,
            connection_id: id,
            connected_at_ms,
            access_level: 0,
            last_server: 0,
            joined_gs: Arc::new(AtomicBool::new(joined)),
            characters: Arc::default(),
        }
    }

    #[tokio::test]
    async fn purge_drops_stale_and_disconnected_clients() {
        let timeout = 300_000;
        let registry = ClientRegistry::default();
        let (stale, mut stale_socket) = connection().await;
        let (joined, _joined_socket) = connection().await;
        let (fresh, _fresh_socket) = connection().await;
        let (gone, gone_socket) = connection().await;
        registry.insert("stale", entry(stale, 1, 1_000, false));
        // Old, but already got PlayOk: waits for the game server.
        registry.insert("joined", entry(joined, 2, 1_000, true));
        registry.insert("fresh", entry(fresh, 3, 400_000, false));
        // Young, but the connection is gone.
        registry.insert("gone", entry(gone.clone(), 4, 400_000, false));
        drop(gone_socket);
        for _ in 0..200 {
            if gone.is_closed() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(gone.is_closed());

        // 1_000 + 300_000 <= 301_000: exactly at the limit counts as timed out.
        assert_eq!(registry.purge(301_000, timeout), 2);
        assert!(registry.get("stale").is_none());
        assert!(registry.get("gone").is_none());
        assert!(registry.get("joined").is_some());
        assert!(registry.get("fresh").is_some());

        // The stale client is told LoginFail(ACCESS_FAILED), then closed.
        let mut received = Vec::new();
        tokio::time::timeout(Duration::from_secs(10), stale_socket.read_to_end(&mut received))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(received, [4, 0, 0x01, 0x15]);

        // Nothing else is due yet.
        assert_eq!(registry.purge(301_000, timeout), 0);
        assert_eq!(registry.purge(700_001, timeout), 1, "fresh times out later");
        assert!(registry.get("joined").is_some());
    }

    #[tokio::test]
    async fn characters_reach_the_waiting_client() {
        let registry = ClientRegistry::default();
        let (handle, _socket) = connection().await;
        let client = entry(handle, 1, 0, false);
        let replies = Arc::clone(&client.characters);
        registry.insert("hero", client);
        assert!(registry.set_characters_on_server("hero", 2, 3, &[5_000]));
        assert!(!registry.set_characters_on_server("nobody", 2, 3, &[]));
        let state = replies.snapshot();
        assert_eq!(state.chars().get(&2), Some(&3));
        assert_eq!(state.delete_times().get(&2), Some(&vec![5_000]));
    }
}
