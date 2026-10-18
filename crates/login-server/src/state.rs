//! `LoginState`: everything the login server shares between its tasks.
// Java: org/l2jmobius/loginserver/controller/LoginController.java (the singleton's fields)
// Java: org/l2jmobius/loginserver/data/GameServerTable.java
// Java: org/l2jmobius/loginserver/LoginServer.java (`_loginStatus`, `shutdown`)
//
// REFERENCE PATTERN for server-process state (the game server's connection-layer state copies
// it): one struct behind an `Arc`, built once at startup from already-loaded parts, read-only
// pools as plain fields, mutable parts behind the smallest lock that fits (`DashMap` for the
// ban list, a `Mutex` for short counters, atomics for flags). This is a server process, not the
// game world, so D-10 does not apply here. Nothing in this struct is ever held across an
// `.await`.

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

use l2_config::LoginConfigSet;
use l2_crypto::RsaKeyPair;
use l2_db::{LoginRepository, Pool, string_to_hex};
use l2_util::WorldRng;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use crate::bans::BanList;
use crate::keys::{ClientKeys, GameServerKeys, ScrambledKeyPair};
use crate::server_names::ServerNames;

/// Locks a mutex and keeps going if another task panicked while holding it: the data behind
/// these locks (counters, a map) cannot be left half-updated in a way that matters.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Status of the login server, and of a game server as it reports it (`ServerStatus`).
// Java: org/l2jmobius/loginserver/network/gameserverpackets/receive/ServerStatus.java
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerStatus {
    Auto = 0,
    Good = 1,
    Normal = 2,
    Full = 3,
    Down = 4,
    GmOnly = 5,
}

impl ServerStatus {
    pub fn from_i32(value: i32) -> Option<Self> {
        Some(match value {
            0 => Self::Auto,
            1 => Self::Good,
            2 => Self::Normal,
            3 => Self::Full,
            4 => Self::Down,
            5 => Self::GmOnly,
            _ => return None,
        })
    }

    pub fn as_i32(self) -> i32 {
        self as i32
    }

    fn from_u8(value: u8) -> Self {
        Self::from_i32(i32::from(value)).unwrap_or(Self::Normal)
    }
}

/// Why and how the process should end. The exit code is what the start scripts and the
/// supervisor look at.
// Java: LoginServer.EXIT_CODE_*
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitReason {
    /// Normal shutdown, exit code 0.
    Shutdown,
    /// Fatal startup error, exit code 1.
    Fatal,
    /// Restart requested, exit code 2 (the wrapper starts the server again).
    Restart,
}

impl ExitReason {
    pub fn code(self) -> u8 {
        match self {
            Self::Shutdown => 0,
            Self::Fatal => 1,
            Self::Restart => 2,
        }
    }
}

/// Cloneable way to ask the running server to stop (`LoginServer.shutdown(restart)`): the
/// restart timer, the admin endpoint and later the listeners hold one.
#[derive(Debug, Clone)]
pub struct ShutdownHandle {
    sender: mpsc::UnboundedSender<ExitReason>,
}

impl ShutdownHandle {
    /// A handle and the receiver `Server::run_until` waits on.
    pub fn channel() -> (Self, mpsc::UnboundedReceiver<ExitReason>) {
        let (sender, receiver) = mpsc::unbounded_channel();
        (Self { sender }, receiver)
    }

    /// Asks for a shutdown, or a restart (exit code 2). A request after the server already
    /// stopped is ignored.
    pub fn request(&self, restart: bool) {
        let reason = if restart {
            ExitReason::Restart
        } else {
            ExitReason::Shutdown
        };
        let _ = self.sender.send(reason);
    }
}

/// One row of the registered game server table.
// Java: GameServerTable.GameServerInfo (table model only; registration is M1-LS-04)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameServerInfo {
    pub id: i32,
    /// Bytes of the hex id (Java `BigInteger.toByteArray`), compared when a game server
    /// authenticates.
    pub hex_id: Vec<u8>,
    pub authed: bool,
    pub status: ServerStatus,
}

impl GameServerInfo {
    /// A registered server that is not connected: status DOWN, not authed.
    pub fn new(id: i32, hex_id: Vec<u8>) -> Self {
        Self {
            id,
            hex_id,
            authed: false,
            status: ServerStatus::Down,
        }
    }
}

/// Server names, registered game servers and the RSA-512 pairs of the game server links.
// Java: GameServerTable
pub struct GameServerTable {
    names: ServerNames,
    registered: Mutex<BTreeMap<i32, GameServerInfo>>,
    keys: GameServerKeys,
}

impl GameServerTable {
    pub fn new(names: ServerNames, registered: BTreeMap<i32, GameServerInfo>, keys: GameServerKeys) -> Self {
        Self {
            names,
            registered: Mutex::new(registered),
            keys,
        }
    }

    /// Reads the `gameservers` table. Like Java, a database error is logged and leaves the
    /// table empty, and a row whose hexid cannot be parsed ends the loading there.
    // Java: GameServerTable.loadRegisteredGameServers
    pub async fn load_registered(repository: &LoginRepository) -> BTreeMap<i32, GameServerInfo> {
        let mut registered = BTreeMap::new();
        match repository.game_servers().await {
            Ok(rows) => {
                for row in rows {
                    match string_to_hex(&row.hexid) {
                        Ok(hex_id) => {
                            registered.insert(row.server_id, GameServerInfo::new(row.server_id, hex_id));
                        }
                        Err(error) => {
                            error!("Error loading registered game servers! {error}");
                            break;
                        }
                    }
                }
            }
            Err(error) => error!("Error loading registered game servers! {error}"),
        }
        info!("Loaded {} registered Game Servers.", registered.len());
        registered
    }

    pub fn server_name(&self, id: i32) -> Option<&str> {
        self.names.get(&id).map(String::as_str)
    }

    pub fn server_names(&self) -> &ServerNames {
        &self.names
    }

    pub fn registered_count(&self) -> usize {
        lock(&self.registered).len()
    }

    pub fn has_registered(&self, id: i32) -> bool {
        lock(&self.registered).contains_key(&id)
    }

    /// A copy of the registered server `id`.
    pub fn registered(&self, id: i32) -> Option<GameServerInfo> {
        lock(&self.registered).get(&id).cloned()
    }

    /// The ids of all registered servers, ascending.
    pub fn registered_ids(&self) -> Vec<i32> {
        lock(&self.registered).keys().copied().collect()
    }

    pub fn key_pairs(&self) -> &[RsaKeyPair] {
        &self.keys.pairs
    }
}

/// Shared state of the login server. Build it once, wrap it in an `Arc`, hand clones to tasks.
pub struct LoginState {
    pub config: LoginConfigSet,
    pub pool: Pool,
    pub repository: LoginRepository,
    pub client_keys: ClientKeys,
    pub game_servers: GameServerTable,
    pub bans: BanList,
    /// Failed login attempts per client address (Java `_failedLoginAttemps`).
    failed_logins: Mutex<HashMap<String, i32>>,
    status: AtomicU8,
    /// Picks keys at random. Seedable so tests are reproducible (D-13 spirit); production seeds
    /// it from the OS.
    rng: Mutex<WorldRng>,
    shutdown: ShutdownHandle,
}

impl LoginState {
    pub fn new(
        config: LoginConfigSet,
        pool: Pool,
        client_keys: ClientKeys,
        game_servers: GameServerTable,
        bans: BanList,
        rng: WorldRng,
        shutdown: ShutdownHandle,
    ) -> Self {
        Self {
            repository: LoginRepository::new(pool.clone()),
            config,
            pool,
            client_keys,
            game_servers,
            bans,
            failed_logins: Mutex::new(HashMap::new()),
            status: AtomicU8::new(ServerStatus::Normal as u8),
            rng: Mutex::new(rng),
            shutdown,
        }
    }

    /// Index in `0..len` (0 for an empty pool).
    fn random_index(&self, len: usize) -> usize {
        let bound = i32::try_from(len).unwrap_or(i32::MAX);
        usize::try_from(lock(&self.rng).below(bound)).unwrap_or(0)
    }

    /// A random client RSA pair (Java `LoginClient` constructor).
    pub fn random_client_key_pair(&self) -> Option<&ScrambledKeyPair> {
        self.client_keys
            .pairs
            .get(self.random_index(self.client_keys.pairs.len()))
    }

    /// A random client Blowfish key (Java `LoginController.getBlowfishKey`).
    pub fn random_blowfish_key(&self) -> Option<&[u8; 16]> {
        self.client_keys
            .blowfish
            .get(self.random_index(self.client_keys.blowfish.len()))
    }

    /// A random game server link RSA pair (Java `GameServerTable.getKeyPair`).
    pub fn random_game_server_key_pair(&self) -> Option<&RsaKeyPair> {
        let pairs = self.game_servers.key_pairs();
        pairs.get(self.random_index(pairs.len()))
    }

    // Java: LoginServer.getStatus / setStatus
    pub fn status(&self) -> ServerStatus {
        ServerStatus::from_u8(self.status.load(Ordering::Relaxed))
    }

    pub fn set_status(&self, status: ServerStatus) {
        self.status.store(status as u8, Ordering::Relaxed);
    }

    /// Counts a failed login of `address`. At `LoginTryBeforeBan` failures the address is banned
    /// for `LoginBlockAfterBan` seconds and its counter starts again. True when it got banned.
    // Java: LoginController.recordFailedLoginAttemp
    pub fn record_failed_login(&self, address: &str) -> bool {
        let attempts = {
            let mut failed = lock(&self.failed_logins);
            let count = failed.entry(address.to_owned()).or_insert(0);
            *count = count.saturating_add(1);
            *count
        };
        if attempts < self.config.login.login_try_before_ban {
            return false;
        }
        // NOTE(java-quirk): `LOGIN_BLOCK_AFTER_BAN * 1000` is an int multiplication in Java and
        // overflows (wraps) for block times above about 24 days.
        let block_ms = i64::from(self.config.login.login_block_after_ban.wrapping_mul(1000));
        self.bans.add(address, block_ms);
        // The counter starts again, so after the ban the client gets a fresh set of attempts.
        self.clear_failed_logins(address);
        warn!("Added banned address {address}! Too many login attempts.");
        true
    }

    // Java: LoginController.clearFailedLoginAttemps
    pub fn clear_failed_logins(&self, address: &str) {
        lock(&self.failed_logins).remove(address);
    }

    /// Failed attempts currently counted for `address`.
    pub fn failed_logins(&self, address: &str) -> i32 {
        lock(&self.failed_logins).get(address).copied().unwrap_or(0)
    }

    /// Periodic cleanup of clients that never finished logging in. Called every
    /// [`LOGIN_TIMEOUT`](crate::LOGIN_TIMEOUT).
    // Java: LoginController.purge (scheduled in the constructor)
    pub fn purge(&self) {
        // TODO(M1-LS-03): kick clients that did not join a game server within LOGIN_TIMEOUT and
        // drop disconnected ones. There is no client registry yet.
    }

    // Java: LoginServer.shutdown(restart)
    pub fn request_shutdown(&self, restart: bool) {
        self.shutdown.request(restart);
    }

    pub fn shutdown_handle(&self) -> ShutdownHandle {
        self.shutdown.clone()
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    //! A `LoginState` without a database, for unit tests of code that only uses the state.

    use std::path::Path;

    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;
    use sqlx::mysql::{MySqlConnectOptions, MySqlPoolOptions};

    use super::*;

    /// All-defaults config, as Java gets it from missing `.ini` files.
    pub fn default_config() -> LoginConfigSet {
        l2_config::LoginConfig::load_with_processors(Path::new("this-directory-does-not-exist"), 4).unwrap()
    }

    /// The pool never connects (nothing calls it, and the address is a closed port), so no test
    /// built on this can reach a database.
    pub fn lazy_pool() -> Pool {
        let options = MySqlConnectOptions::new().host("127.0.0.1").port(1);
        MySqlPoolOptions::new()
            .max_connections(1)
            .connect_lazy_with(options)
    }

    pub fn state_with(config: LoginConfigSet) -> (LoginState, mpsc::UnboundedReceiver<ExitReason>) {
        let mut rng = ChaCha8Rng::seed_from_u64(5);
        let client_keys = ClientKeys::generate(&mut rng, 2, 4).unwrap();
        let gs_keys = GameServerKeys::generate(&mut rng, 2).unwrap();
        let names = ServerNames::from([(1, "Bartz".to_owned()), (2, "Sieghardt".to_owned())]);
        let registered = BTreeMap::from([(1, GameServerInfo::new(1, vec![0x12, 0x34]))]);
        let (handle, receiver) = ShutdownHandle::channel();
        let state = LoginState::new(
            config,
            lazy_pool(),
            client_keys,
            GameServerTable::new(names, registered, gs_keys),
            BanList::new(),
            WorldRng::from_seed(11),
            handle,
        );
        (state, receiver)
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::*;
    use super::*;

    #[tokio::test]
    async fn status_defaults_to_normal_and_can_change() {
        let (state, _rx) = state_with(default_config());
        assert_eq!(state.status(), ServerStatus::Normal);
        for status in [
            ServerStatus::Auto,
            ServerStatus::Good,
            ServerStatus::Full,
            ServerStatus::Down,
            ServerStatus::GmOnly,
        ] {
            state.set_status(status);
            assert_eq!(state.status(), status);
        }
    }

    #[test]
    fn status_numbers_match_java() {
        assert_eq!(ServerStatus::from_i32(4), Some(ServerStatus::Down));
        assert_eq!(ServerStatus::from_i32(5), Some(ServerStatus::GmOnly));
        assert_eq!(ServerStatus::from_i32(6), None);
        assert_eq!(ServerStatus::GmOnly.as_i32(), 5);
    }

    #[test]
    fn exit_codes_match_java() {
        assert_eq!(ExitReason::Shutdown.code(), 0);
        assert_eq!(ExitReason::Fatal.code(), 1);
        assert_eq!(ExitReason::Restart.code(), 2);
    }

    #[tokio::test]
    async fn failed_logins_ban_at_the_limit_and_reset() {
        let (state, _rx) = state_with(default_config());
        let limit = state.config.login.login_try_before_ban;
        assert_eq!(limit, 5);
        for attempt in 1..limit {
            assert!(!state.record_failed_login("10.0.0.9"));
            assert_eq!(state.failed_logins("10.0.0.9"), attempt);
            assert!(!state.bans.is_banned("10.0.0.9"));
        }
        assert!(state.record_failed_login("10.0.0.9"), "the fifth failure bans");
        assert!(state.bans.is_banned("10.0.0.9"));
        assert_eq!(
            state.failed_logins("10.0.0.9"),
            0,
            "counter restarts after the ban"
        );
        // The ban lasts LoginBlockAfterBan seconds (900 by default).
        let expiry = state.bans.expiry("10.0.0.9").unwrap();
        let remaining = expiry - l2_util::time::now_millis();
        assert!((899_000..=900_000).contains(&remaining), "remaining {remaining}");
        // Other addresses are not affected.
        assert_eq!(state.failed_logins("10.0.0.10"), 0);
    }

    #[tokio::test]
    async fn clear_failed_logins_forgets_the_address() {
        let (state, _rx) = state_with(default_config());
        state.record_failed_login("10.0.0.9");
        state.clear_failed_logins("10.0.0.9");
        assert_eq!(state.failed_logins("10.0.0.9"), 0);
    }

    #[tokio::test]
    async fn random_picks_come_from_the_pools() {
        let (state, _rx) = state_with(default_config());
        let mut seen_pairs = std::collections::HashSet::new();
        for _ in 0..64 {
            let pair = state.random_client_key_pair().unwrap();
            seen_pairs.insert(pair.scrambled_modulus);
            assert!(
                state
                    .client_keys
                    .blowfish
                    .contains(state.random_blowfish_key().unwrap())
            );
            assert!(state.random_game_server_key_pair().is_some());
        }
        assert_eq!(seen_pairs.len(), 2, "both pairs get picked eventually");
    }

    #[tokio::test]
    async fn game_server_table_accessors() {
        let (state, _rx) = state_with(default_config());
        let table = &state.game_servers;
        assert_eq!(table.server_name(1), Some("Bartz"));
        assert_eq!(table.server_name(99), None);
        assert_eq!(table.registered_count(), 1);
        assert!(table.has_registered(1));
        assert!(!table.has_registered(2));
        let info = table.registered(1).unwrap();
        assert_eq!(info.status, ServerStatus::Down);
        assert!(!info.authed);
        assert_eq!(table.registered_ids(), [1]);
    }

    #[tokio::test]
    async fn shutdown_requests_reach_the_receiver() {
        let (state, mut rx) = state_with(default_config());
        state.request_shutdown(false);
        state.shutdown_handle().request(true);
        assert_eq!(rx.recv().await, Some(ExitReason::Shutdown));
        assert_eq!(rx.recv().await, Some(ExitReason::Restart));
    }
}
