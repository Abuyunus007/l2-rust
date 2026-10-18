//! Which connection belongs to which account: for `KickPlayer` from the login server and for the
//! admin view. Network-side bookkeeping only; the world keeps its own player list (D-10).
// Java: LoginServerThread._accountsInGameServer (account -> GameClient)

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use l2_net::{ConnectionHandle, Encode};
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;
use l2_world::{SessionId, WorldCommand};
use tracing::info;

use crate::state::WorldHandle;

/// One authenticated connection.
#[derive(Debug, Clone)]
pub struct SessionEntry {
    pub session: SessionId,
    pub handle: ConnectionHandle,
    /// Set while the account's character is in the world (ENTER-01 sets it). Shared with the
    /// session.
    pub in_world: Arc<AtomicBool>,
}

/// What [`SessionRegistry::kick`] did, so the caller knows whether it still has to tell the
/// login server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KickOutcome {
    /// The character is in the world: the world stores it and the persistence task reports the
    /// logout afterwards (D-15).
    WorldKick,
    /// The client was at the character screen: it is closed and its session reports the logout
    /// when the connection ends.
    Closed,
    /// No connection of this account: the caller reports the logout itself (Java does the same in
    /// `doKickPlayer` when the client is gone).
    NotConnected,
}

#[derive(Debug, Default)]
pub struct SessionRegistry {
    sessions: Mutex<HashMap<String, SessionEntry>>,
}

impl SessionRegistry {
    fn lock(&self) -> MutexGuard<'_, HashMap<String, SessionEntry>> {
        self.sessions.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Registers an authenticated connection. A previous entry of the same account is replaced
    /// (the login server only lets one through; this can only be a stale entry).
    pub fn insert(&self, account: &str, entry: SessionEntry) {
        self.lock().insert(account.to_string(), entry);
    }

    /// Removes the entry only if it still belongs to `session`, so a newer connection of the same
    /// account is not dropped.
    pub fn remove_if_session(&self, account: &str, session: SessionId) -> bool {
        let mut sessions = self.lock();
        if sessions.get(account).is_some_and(|e| e.session == session) {
            sessions.remove(account);
            true
        } else {
            false
        }
    }

    pub fn get(&self, account: &str) -> Option<SessionEntry> {
        self.lock().get(account).cloned()
    }

    pub fn len(&self) -> usize {
        self.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.lock().is_empty()
    }

    /// The login server wants `account` off this server.
    // Java: LoginServerThread.doKickPlayer
    pub fn kick(&self, account: &str, world: &WorldHandle) -> KickOutcome {
        let Some(entry) = self.get(account) else {
            return KickOutcome::NotConnected;
        };
        info!("Kicked by login, account '{account}'.");
        if entry.in_world.load(Ordering::Acquire) {
            // Java: Disconnection.of(client).storeAndDeleteWith(msg). The world sends the message,
            // stores the character and closes the connection through its packet sink.
            if world.send(WorldCommand::Kick {
                account: account.to_string(),
            }) {
                return KickOutcome::WorldKick;
            }
        }
        // Java: client.close(msg) — the message, then the connection is closed.
        entry.handle.send_and_close(
            SystemMessage::new(SystemMessageId::AnotherPersonHasLoggedInWithTheSameAccount).to_bytes(),
        );
        KickOutcome::Closed
    }
}
