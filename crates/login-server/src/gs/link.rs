//! One game server connection as the rest of the login server sees it: the framing cipher and
//! the [`GsLink`] handle.
// Java: org/l2jmobius/loginserver/GameServerThread.java (`sendPacket`, `kickPlayer`,
//       `requestCharacters`, `changePasswordResponse`, the account set)

use std::collections::HashSet;
use std::net::IpAddr;
use std::sync::{Mutex, MutexGuard, PoisonError};

use l2_crypto::LinkCipher;
use l2_net::{CipherError, ConnectionHandle, Encode, FrameCipher};
use l2_protocol_login::link::{ChangePasswordResponse, KickPlayer, RequestCharacters};

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// `l2-crypto`'s [`LinkCipher`] as the framing layer's cipher: seal on send, open on receive.
///
/// The decrypted body keeps its padding and the checksum slot, as Java hands the whole buffer to
/// the packet readers.
pub struct LinkFrameCipher(LinkCipher);

impl LinkFrameCipher {
    pub fn new(cipher: LinkCipher) -> Self {
        Self(cipher)
    }
}

impl FrameCipher for LinkFrameCipher {
    fn encrypt(&mut self, payload: Vec<u8>) -> Vec<u8> {
        self.0.seal(&payload)
    }

    fn decrypt(&mut self, body: &mut Vec<u8>) -> Result<(), CipherError> {
        self.0
            .open(body.as_mut_slice())
            .map(|_| ())
            .map_err(|error| CipherError(error.to_string()))
    }
}

/// A live link to a game server. Shared (`Arc`) between the connection task and the
/// `GameServerInfo` of the server it belongs to; other tasks use it to send packets and to ask
/// which accounts are on that game server.
// Java: GameServerThread
#[derive(Debug)]
pub struct GsLink {
    /// Unique per connection, so a link can be recognised again (`PartialEq`).
    id: u64,
    connection: ConnectionHandle,
    peer_ip: IpAddr,
    /// Java `_accountsOnGameServer`.
    accounts: Mutex<HashSet<String>>,
}

impl PartialEq for GsLink {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for GsLink {}

impl GsLink {
    pub fn new(id: u64, connection: ConnectionHandle) -> Self {
        let peer_ip = connection.peer().ip();
        Self {
            id,
            connection,
            peer_ip,
            accounts: Mutex::new(HashSet::new()),
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    /// Java `getConnectionIpAddress`.
    pub fn peer_ip(&self) -> IpAddr {
        self.peer_ip
    }

    /// Queues a packet; it is encrypted with the key in use when it is written. False when the
    /// connection is gone (Java `sendPacket` returns silently then).
    pub fn send<P: Encode>(&self, packet: &P) -> bool {
        self.connection.send(packet.to_bytes())
    }

    /// Closes the link after the queued packets are written.
    pub fn close(&self) {
        self.connection.close();
    }

    pub fn is_closed(&self) -> bool {
        self.connection.is_closed()
    }

    // Java: GameServerThread.kickPlayer
    pub fn kick_player(&self, account: &str) {
        self.send(&KickPlayer {
            account: account.to_owned(),
        });
    }

    // Java: GameServerThread.requestCharacters
    pub fn request_characters(&self, account: &str) {
        self.send(&RequestCharacters {
            account: account.to_owned(),
        });
    }

    // Java: GameServerThread.changePasswordResponse
    pub fn change_password_response(&self, character_name: &str, message: &str) {
        self.send(&ChangePasswordResponse {
            character_name: character_name.to_owned(),
            message: message.to_owned(),
        });
    }

    // Java: GameServerThread.hasAccountOnGameServer
    pub fn has_account(&self, account: &str) -> bool {
        lock(&self.accounts).contains(account)
    }

    // Java: GameServerThread.getPlayerCount
    pub fn player_count(&self) -> usize {
        lock(&self.accounts).len()
    }

    /// Adds an account to the game server's online set.
    // Java: GameServerThread.addAccountOnGameServer
    // Java also drops the account from the login server's list of authed clients in there. The
    // link does not know that list, so `gs::handlers::add_account_on_game_server` does both.
    pub fn add_account(&self, account: &str) {
        lock(&self.accounts).insert(account.to_owned());
    }

    /// Removes an account from the online set.
    // Java: GameServerThread.removeAccountOnGameServer
    // See `add_account`: `gs::handlers::remove_account_on_game_server` does both steps.
    pub fn remove_account(&self, account: &str) {
        lock(&self.accounts).remove(account);
    }
}
