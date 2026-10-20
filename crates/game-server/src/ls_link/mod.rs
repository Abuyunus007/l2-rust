//! The game server's side of the link to the login server (port `LoginPort`, default 9014).
// Java: org/l2jmobius/gameserver/LoginServerThread.java (all of it)
// Java: org/l2jmobius/gameserver/network/loginserverpackets/** (the packets live in
//       `l2-protocol-login::link`)
// Java: org/l2jmobius/gameserver/config/ServerConfig.java (`getServerTypeId`, `IPConfigData`)
//
// One task (the "link task") owns everything that belongs to the link: the connection, the
// accounts that are on this game server, the clients waiting for `PlayerAuthResponse`. Other
// tasks talk to it through [`LsLink`], a cheap handle that only sends commands over a channel.
// That replaces the `synchronized` sets and `ConcurrentHashMap`s of the Java thread without any
// lock around shared state.
//
// What the link task does, like the Java thread:
// - connect to `LoginHost:LoginPort`, wait for `InitLS`, send `BlowFishKey` (40 random bytes under
//   the login server's RSA key), switch to that key and send `AuthRequest`;
// - on `AuthResponse` write `hexid.txt`, send `ServerStatus` and the accounts that are online;
// - `auth_player` sends `PlayerAuthRequest` and waits for the answer; on success `PlayerInGame`,
//   on failure `PlayerLogout` goes back (Java `LoginServerThread` case 0x03);
// - `RequestCharacters` is answered from the database (Java `getCharsOnServer`);
// - `KickPlayer` goes to the session registry (M2-GS-03) through [`LsLink::take_kicks`];
// - when the link breaks it waits 5 seconds and connects again.
//
// NOTE(java-quirk): after a reconnect the request id stays what it was at start (`RequestServerID`
// or the id of `hexid.txt`), even when the login server gave another id the first time.

pub mod ipconfig;

use std::collections::{BTreeSet, HashMap};
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use l2_config::{GameConfig, IniFile};
use l2_crypto::LinkCipher;
use l2_crypto::rsa::encrypt_raw;
use l2_db::{CharacterRepository, Pool, hex_to_string};
use l2_net::{
    CancellationToken, CipherError, ConnectionConfig, ConnectionHandle, Control, Encode, FrameCipher,
    FrameHandler, PacketReader, spawn_connection,
};
use l2_protocol_login::link::{
    AuthResponse, BlowFishKey, ChangeAccessLevel, GameServerAuth, InitLS, LsPacket, PROTOCOL_REV,
    PlayerAuthRequest, PlayerAuthResponse, PlayerInGame, PlayerLogout, ReplyCharacters, ServerStatus,
    decode_from_ls,
};
use rand::Rng;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinSet;
use tracing::{debug, info, warn};

pub use ipconfig::IpConfig;

use crate::hexid::{self, HexId};
use crate::{SERVER_CONFIG_FILE, Server};

/// Java `RECONNECT_DELAY`.
pub const RECONNECT_DELAY: Duration = Duration::from_secs(5);
/// Java `BLOWFISH_KEY_SIZE`.
const BLOWFISH_KEY_SIZE: usize = 40;
/// Java `HEX_ID_SIZE`.
const HEX_ID_SIZE: usize = 16;
/// Java `RSAKeyGenParameterSpec.F4`.
const RSA_PUBLIC_EXPONENT: [u8; 3] = [0x01, 0x00, 0x01];
/// How long [`LsLink::shutdown`] waits for the last packets to be written.
const SHUTDOWN_WAIT: Duration = Duration::from_secs(5);

/// The four numbers the client sent in `AuthLogin`, which the login server compares with the
/// ones it gave out.
// Java: LoginServerThread.SessionKey
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionKey {
    pub login_ok1: i32,
    pub login_ok2: i32,
    pub play_ok1: i32,
    pub play_ok2: i32,
}

/// The login server wants this account off the game server (another login with the same
/// account). The session registry kicks the client, waits until the character is stored, and
/// then calls [`LsLink::player_logout`].
// Java: LoginServerThread.doKickPlayer
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KickRequest {
    pub account: String,
}

/// Where the number of characters of an account comes from (`RequestCharacters`).
pub trait CharacterCounts: Send + Sync + 'static {
    /// The `deletetime` of every character of the account, 0 meaning "not being deleted".
    fn delete_times<'a>(
        &'a self,
        account: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u64>, String>> + Send + 'a>>;
}

/// [`CharacterCounts`] from the `characters` table.
// Java: LoginServerThread.getCharsOnServer
pub struct DbCharacterCounts {
    characters: CharacterRepository,
}

impl DbCharacterCounts {
    pub fn new(pool: Pool) -> Self {
        Self {
            characters: CharacterRepository::new(pool),
        }
    }
}

impl CharacterCounts for DbCharacterCounts {
    fn delete_times<'a>(
        &'a self,
        account: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u64>, String>> + Send + 'a>> {
        Box::pin(async move {
            self.characters
                .delete_times_of_account(account)
                .await
                .map_err(|e| e.to_string())
        })
    }
}

/// Everything the link needs from the configuration, fixed at start.
#[derive(Clone)]
pub struct LsLinkParams {
    /// `LoginHost`.
    pub host: String,
    /// `LoginPort`.
    pub port: u16,
    /// `GameserverPort`: the port clients connect to, announced in `AuthRequest`.
    pub game_port: u16,
    /// `AcceptAlternateID`.
    pub accept_alternate: bool,
    /// Java `RESERVE_HOST_ON_LOGIN`, which is never set and so always false.
    pub reserve_host: bool,
    /// `GAME_SERVER_SUBNETS` and `GAME_SERVER_HOSTS`, see [`IpConfig`].
    pub subnets: Vec<String>,
    pub hosts: Vec<String>,
    /// `MaximumOnlineUsers`.
    pub max_players: i32,
    /// The id asked for: the `ServerID` of `hexid.txt`, else `RequestServerID`.
    pub request_id: i32,
    /// The secret of the registration: from `hexid.txt`, else newly generated (16 bytes).
    pub hex_id: Vec<u8>,
    /// Where `hexid.txt` is written after the registration.
    pub config_dir: PathBuf,
    /// `ServerListType` as flags (Java `getServerTypeId`).
    pub server_list_type: i32,
    /// `ServerListAge`.
    pub server_list_age: i32,
    /// `ServerListBrackets`.
    pub server_list_brackets: bool,
    /// `ServerGMOnly`.
    pub gm_only: bool,
    /// Wait between two connection attempts; [`RECONNECT_DELAY`] unless a test shortens it.
    pub reconnect_delay: Duration,
}

// The hex id is a secret, so it stays out of logs.
impl std::fmt::Debug for LsLinkParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LsLinkParams")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("game_port", &self.game_port)
            .field("request_id", &self.request_id)
            .field("hex_id", &"<redacted>")
            .finish_non_exhaustive()
    }
}

impl LsLinkParams {
    /// Reads the settings like the `LoginServerThread` constructor. With a `hexid.txt` its id and
    /// secret are used; without, the id comes from `RequestServerID` and a new secret is made.
    // Java: LoginServerThread constructor
    pub fn from_config(
        config: &GameConfig,
        server_list_type: i32,
        ipconfig: IpConfig,
        hexid: Option<&HexId>,
        config_dir: PathBuf,
    ) -> Self {
        let server = &config.server;
        let (request_id, hex_id) = match hexid {
            Some(hexid) => (hexid.server_id, hexid.hex_id.clone()),
            None => (server.request_id, random_nonzero_bytes(HEX_ID_SIZE)),
        };
        Self {
            host: server.game_server_login_host.clone(),
            port: port_of("LoginPort", server.game_server_login_port),
            game_port: port_of("GameserverPort", server.port_game),
            accept_alternate: server.accept_alternate_id,
            reserve_host: false,
            subnets: ipconfig.subnets,
            hosts: ipconfig.hosts,
            max_players: server.maximum_online_users,
            request_id,
            hex_id,
            config_dir,
            server_list_type,
            server_list_age: server.server_list_age,
            server_list_brackets: server.server_list_bracket,
            gm_only: config.general.server_gmonly,
            reconnect_delay: RECONNECT_DELAY,
        }
    }

    /// The `AuthRequest` for this registration.
    // Java: new AuthRequest(_requestID, _acceptAlternate, _hexID, _gamePort, _reserveHost, _maxPlayer, _subnets, _hosts)
    fn auth_request(&self, max_players: i32) -> GameServerAuth {
        GameServerAuth {
            // Java `writeByte(int)` and `writeShort(int)` keep the low bits.
            desired_id: (self.request_id & 0xFF) as u8,
            accept_alternate: self.accept_alternate,
            reserve_host: self.reserve_host,
            port: self.game_port,
            max_players,
            hex_id: self.hex_id.clone(),
            subnets: self.subnets.clone(),
            hosts: self.hosts.clone(),
        }
    }

    /// The `ServerStatus` sent right after the registration.
    // Java: LoginServerThread case 0x02
    fn registration_status(&self) -> ServerStatus {
        let age = match self.server_list_age {
            15 => ServerStatus::SERVER_AGE_15,
            18 => ServerStatus::SERVER_AGE_18,
            _ => ServerStatus::SERVER_AGE_ALL,
        };
        ServerStatus {
            attributes: vec![
                (
                    ServerStatus::SERVER_LIST_SQUARE_BRACKET,
                    if self.server_list_brackets {
                        ServerStatus::ON
                    } else {
                        ServerStatus::OFF
                    },
                ),
                (ServerStatus::SERVER_TYPE, self.server_list_type),
                (
                    ServerStatus::SERVER_LIST_STATUS,
                    if self.gm_only {
                        ServerStatus::STATUS_GM_ONLY
                    } else {
                        ServerStatus::STATUS_AUTO
                    },
                ),
                // NOTE(java-quirk): the Java game server numbers the age attribute 5, which the
                // login server knows as `TEST_SERVER` and ignores; it reads age as 6. So the
                // age never reaches the server list. Sent as Java sends it.
                (ServerStatus::TEST_SERVER, age),
            ],
        }
    }
}

/// A port from the config. Java would throw when it connects; here the attempts fail the same
/// way and are logged every [`RECONNECT_DELAY`].
fn port_of(key: &str, value: i32) -> u16 {
    u16::try_from(value).unwrap_or_else(|_| {
        warn!("{key} = {value} is not a TCP port.");
        0
    })
}

/// Java `ServerConfig.getServerTypeId` on the comma separated `ServerListType` value. Unknown
/// words count for nothing.
pub fn server_type_id(value: &str) -> i32 {
    value
        .split(',')
        .map(|word| match word.trim().to_lowercase().as_str() {
            "normal" => ServerStatus::SERVER_NORMAL,
            "relax" => ServerStatus::SERVER_RELAX,
            "test" => ServerStatus::SERVER_TEST,
            "nolabel" => ServerStatus::SERVER_NOLABEL,
            "restricted" => ServerStatus::SERVER_CREATION_RESTRICTED,
            "event" => ServerStatus::SERVER_EVENT,
            "free" => ServerStatus::SERVER_FREE,
            _ => 0,
        })
        .fold(0, |all, flag| all | flag)
}

/// `Java generateHexBytes`: random bytes, none of them zero.
// Java: HexUtil.generateHexBytes
fn random_nonzero_bytes(len: usize) -> Vec<u8> {
    let mut rng = rand::rng();
    let mut bytes = vec![0u8; len];
    rng.fill_bytes(&mut bytes);
    for byte in &mut bytes {
        let mut one = [0u8; 1];
        while *byte == 0 {
            rng.fill_bytes(&mut one);
            *byte = one[0];
        }
    }
    bytes
}

/// What the link looks like from outside.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LinkStatus {
    /// Connected and registered at the login server (the link carries game packets).
    pub registered: bool,
    /// The id the login server gave, once registered at least once.
    pub server_id: Option<u8>,
    /// The name the login server gave that id.
    pub server_name: String,
    /// The list status last set with [`LsLink::set_server_status`] (Java `_status`).
    pub server_status: i32,
    /// `MaximumOnlineUsers`, or what [`LsLink::set_max_player`] set.
    pub max_players: i32,
}

/// Not one of the `STATUS_*` values of `ServerStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("invalid server status: {0}")]
pub struct InvalidServerStatus(pub i32);

enum Command {
    AuthPlayer {
        account: String,
        key: SessionKey,
        reply: oneshot::Sender<bool>,
    },
    PlayerInGame {
        account: String,
    },
    PlayerLogout {
        account: String,
    },
    AccessLevel {
        account: String,
        level: i32,
    },
    ReplyCharacters {
        account: String,
        chars: u8,
        delete_times: Vec<i64>,
    },
    ServerStatus {
        id: i32,
        value: i32,
    },
    SetStatus(i32),
    SetMaxPlayer(i32),
    Stop {
        done: oneshot::Sender<()>,
    },
}

/// The handle other tasks use. Clone it freely; all clones talk to the same link task.
#[derive(Clone)]
pub struct LsLink {
    commands: mpsc::UnboundedSender<Command>,
    status: watch::Receiver<LinkStatus>,
    kicks: Arc<KickChannel>,
}

struct KickChannel {
    receiver: Mutex<Option<mpsc::UnboundedReceiver<KickRequest>>>,
    taken: AtomicBool,
}

impl LsLink {
    /// Creates the link and the future that runs it. Run the future as a task for as long as
    /// the server lives (`Server::spawn`), after which [`LsLink::shutdown`] is the graceful way
    /// to end it.
    pub fn new(
        params: LsLinkParams,
        characters: Arc<dyn CharacterCounts>,
        cancel: CancellationToken,
    ) -> (Self, impl Future<Output = ()> + Send + 'static) {
        let (commands_tx, commands_rx) = mpsc::unbounded_channel();
        let (kick_tx, kick_rx) = mpsc::unbounded_channel();
        let (status_tx, status_rx) = watch::channel(LinkStatus {
            max_players: params.max_players,
            ..LinkStatus::default()
        });
        let kicks = Arc::new(KickChannel {
            receiver: Mutex::new(Some(kick_rx)),
            taken: AtomicBool::new(false),
        });
        let actor = Actor {
            params: Arc::new(params),
            characters,
            cancel,
            commands: commands_rx,
            kick_sender: kick_tx,
            kicks_taken: Arc::clone(&kicks),
            status_tx,
            connection: None,
            registered: false,
            server_id: None,
            server_name: String::new(),
            status: ServerStatus::STATUS_AUTO,
            max_players: 0,
            accounts: BTreeSet::new(),
            in_game: BTreeSet::new(),
            waiting: HashMap::new(),
            lookups: JoinSet::new(),
        };
        let link = Self {
            commands: commands_tx,
            status: status_rx,
            kicks,
        };
        (link, actor.run())
    }

    /// Asks the login server whether the keys of this client are the ones it handed out.
    /// `true` means the account is now in the game for the login server (`PlayerInGame` was
    /// sent); the caller continues with the character screen.
    ///
    /// `false` means the client must be closed: the login server refused the keys (a
    /// `PlayerLogout` has been sent), the account already has a session on this game server
    /// (that session keeps the account), or the link is down.
    ///
    /// Whoever ends the session of an authenticated account must call
    /// [`player_logout`](Self::player_logout) afterwards.
    // Java: LoginServerThread.addGameServerLogin + addWaitingClientAndSendRequest, case 0x03
    //
    // NOTE(java-quirk): with the link down Java queues the client and never answers it, so it
    // hangs until it times out. Answering `false` at once is the same for the player (the
    // connection is closed) without leaving the client waiting.
    pub async fn auth_player(&self, account: &str, key: SessionKey) -> bool {
        let (reply, answer) = oneshot::channel();
        let command = Command::AuthPlayer {
            account: account.to_owned(),
            key,
            reply,
        };
        if self.commands.send(command).is_err() {
            return false;
        }
        answer.await.unwrap_or(false)
    }

    /// Tells the login server the account is in the game ([`auth_player`](Self::auth_player)
    /// already did that; this is for the world's `PlayerInGame` event). The account is also
    /// resent after a reconnect.
    // Java: new PlayerInGame(account)
    pub async fn player_in_game(&self, account: &str) {
        self.send_command(Command::PlayerInGame {
            account: account.to_owned(),
        });
    }

    /// The account left the game server (the client is gone and the character stored): the
    /// login server may let it log in again, here or on another game server. Safe to call for an
    /// account the link does not know.
    // Java: LoginServerThread.sendLogout
    pub async fn player_logout(&self, account: &str) {
        self.send_command(Command::PlayerLogout {
            account: account.to_owned(),
        });
    }

    /// Tells the login server to change the access level of an account: -1 bans it, 0 lifts the
    /// ban.
    // Java: LoginServerThread.sendAccessLevel
    pub async fn send_access_level(&self, account: &str, level: i32) {
        self.send_command(Command::AccessLevel {
            account: account.to_owned(),
            level,
        });
    }

    /// Answers a `RequestCharacters` by hand. The link answers those itself from the database;
    /// this is for callers that know better (and for tests).
    // Java: new ReplyCharacters(account, chars, charToDel)
    pub async fn reply_characters(&self, account: &str, chars: u8, delete_times: Vec<i64>) {
        self.send_command(Command::ReplyCharacters {
            account: account.to_owned(),
            chars,
            delete_times,
        });
    }

    /// Changes the status shown in the server list (one of `ServerStatus::STATUS_*`).
    // Java: LoginServerThread.setServerStatus
    pub async fn set_server_status(&self, status: i32) -> Result<(), InvalidServerStatus> {
        match status {
            ServerStatus::STATUS_AUTO
            | ServerStatus::STATUS_GOOD
            | ServerStatus::STATUS_NORMAL
            | ServerStatus::STATUS_FULL
            | ServerStatus::STATUS_DOWN
            | ServerStatus::STATUS_GM_ONLY => {
                self.send_command(Command::SetStatus(status));
                Ok(())
            }
            other => Err(InvalidServerStatus(other)),
        }
    }

    /// One attribute of `ServerStatus` (`ServerStatus::SERVER_TYPE`, ...).
    // Java: LoginServerThread.sendServerStatus
    pub async fn send_server_status(&self, id: i32, value: i32) {
        self.send_command(Command::ServerStatus { id, value });
    }

    /// Java `setMaxPlayer`: the limit shown in the server list.
    pub async fn set_max_player(&self, max_players: i32) {
        self.send_command(Command::SetMaxPlayer(max_players));
    }

    /// The current state of the link.
    pub fn status(&self) -> LinkStatus {
        self.status.borrow().clone()
    }

    /// A receiver that wakes when the state changes.
    pub fn subscribe(&self) -> watch::Receiver<LinkStatus> {
        self.status.clone()
    }

    /// The kick requests of the login server. Whoever owns the client sessions takes this once;
    /// until then (and when it is dropped) the link answers a `KickPlayer` with `PlayerLogout`
    /// itself, as there is nobody to kick.
    pub fn take_kicks(&self) -> Option<mpsc::UnboundedReceiver<KickRequest>> {
        let receiver = self
            .kicks
            .receiver
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if receiver.is_some() {
            self.kicks.taken.store(true, Ordering::Release);
        }
        receiver
    }

    /// Reports the server as down, closes the link and ends the link task.
    // Java: Shutdown sets STATUS_DOWN, then `LoginServerThread.interrupt()` closes the socket.
    pub async fn shutdown(&self) {
        let (done, finished) = oneshot::channel();
        if self.commands.send(Command::Stop { done }).is_err() {
            return;
        }
        if tokio::time::timeout(SHUTDOWN_WAIT, finished).await.is_err() {
            warn!("The login server link did not stop in time.");
        }
    }

    /// Queues a command. The link task being gone (shutdown) is not an error for the callers.
    fn send_command(&self, command: Command) {
        if self.commands.send(command).is_err() {
            debug!("The login server link is stopped; command dropped.");
        }
    }
}

// ------------------------------------------------------------------------------------------
// The connection: framing cipher and the frame handler
// ------------------------------------------------------------------------------------------

/// [`LinkCipher`] as the framing layer's cipher. The decrypted body keeps its padding and the
/// checksum slot, as Java hands the whole buffer to the packet readers.
struct LinkFrameCipher(LinkCipher);

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

/// What the connection task tells the link task.
enum LinkEvent {
    Packet(LsPacket),
    Closed,
}

struct LinkHandler {
    params: Arc<LsLinkParams>,
    /// Java `_maxPlayer` when the connection was made.
    max_players: i32,
    events: mpsc::UnboundedSender<LinkEvent>,
}

impl LinkHandler {
    /// The key exchange: the answer to `InitLS`.
    // Java: LoginServerThread case 0x00
    fn handshake(&self, init: &InitLS, ctl: &mut Control<'_>) {
        if init.protocol_revision != PROTOCOL_REV {
            warn!("/!\\ Revision mismatch between LS and GS /!\\");
            return;
        }
        let key = random_nonzero_bytes(BLOWFISH_KEY_SIZE);
        let Ok(encrypted_key) = encrypt_raw(&init.public_key, &RSA_PUBLIC_EXPONENT, &key) else {
            warn!("Trouble initializing RSA public key from login server.");
            return;
        };
        let Ok(session_cipher) = LinkCipher::new(&key) else {
            warn!("Cannot use the Blowfish key for the login server link.");
            return;
        };
        // The key travels under the default cipher; everything after it uses the new one.
        ctl.send(BlowFishKey { encrypted_key }.to_bytes());
        ctl.set_cipher(Box::new(LinkFrameCipher(session_cipher)));
        ctl.send(self.params.auth_request(self.max_players).to_bytes());
    }
}

impl FrameHandler for LinkHandler {
    async fn on_frame(&mut self, body: Vec<u8>, ctl: &mut Control<'_>) {
        let Some((&opcode, rest)) = body.split_first() else {
            return;
        };
        let packet = match decode_from_ls(opcode, &mut PacketReader::new(rest)) {
            Ok(packet) => packet,
            // Java's switch ignores opcodes it does not know.
            Err(e) => {
                debug!("Ignoring a packet from the login server: {e}");
                return;
            }
        };
        match packet {
            LsPacket::InitLS(init) => self.handshake(&init, ctl),
            other => {
                let _ = self.events.send(LinkEvent::Packet(other));
            }
        }
    }

    async fn on_disconnect(&mut self) {
        let _ = self.events.send(LinkEvent::Closed);
    }
}

// ------------------------------------------------------------------------------------------
// The link task
// ------------------------------------------------------------------------------------------

/// What to do after a command.
enum Flow {
    Continue,
    /// Close the link and end the task; answer this when done.
    Stop(oneshot::Sender<()>),
}

struct Actor {
    params: Arc<LsLinkParams>,
    characters: Arc<dyn CharacterCounts>,
    cancel: CancellationToken,
    commands: mpsc::UnboundedReceiver<Command>,
    kick_sender: mpsc::UnboundedSender<KickRequest>,
    kicks_taken: Arc<KickChannel>,
    status_tx: watch::Sender<LinkStatus>,

    /// The current connection, once the socket is open.
    connection: Option<ConnectionHandle>,
    /// `AuthResponse` received on this connection: game packets may be sent.
    registered: bool,
    server_id: Option<u8>,
    server_name: String,
    /// Java `_status`.
    status: i32,
    /// Java `_maxPlayer`, 0 until `set_max_player` or the first registration.
    max_players: i32,
    /// Java `_accountsInGameServer`: accounts with a session here, waiting for the keys to be
    /// checked or already in the game.
    accounts: BTreeSet<String>,
    /// The accounts the login server was told are in the game; resent after a reconnect.
    in_game: BTreeSet<String>,
    /// Java `_waitingClients`.
    waiting: HashMap<String, oneshot::Sender<bool>>,
    /// Database lookups for `RequestCharacters` that are running.
    lookups: JoinSet<(String, Result<Vec<u64>, String>)>,
}

impl Actor {
    async fn run(mut self) {
        self.max_players = self.params.max_players;
        loop {
            info!("Connecting to login on {}:{}", self.params.host, self.params.port);
            let target = (self.params.host.clone(), self.params.port);
            let Some(connected) = self.with_commands(TcpStream::connect(target)).await else {
                return;
            };
            match connected {
                Ok(stream) => {
                    if !self.session(stream).await {
                        return;
                    }
                }
                Err(e) => warn!("LoginServer not available, trying to reconnect... ({e})"),
            }
            let delay = self.params.reconnect_delay;
            if self.with_commands(tokio::time::sleep(delay)).await.is_none() {
                return;
            }
        }
    }

    /// Waits for `future` while the commands of other tasks are handled (there is no link, so
    /// most of them do nothing). `None` when the link task must end.
    async fn with_commands<F: Future>(&mut self, future: F) -> Option<F::Output> {
        tokio::pin!(future);
        loop {
            tokio::select! {
                output = &mut future => return Some(output),
                command = self.commands.recv() => match command {
                    Some(command) => {
                        if let Flow::Stop(done) = self.handle_command(command) {
                            let _ = done.send(());
                            return None;
                        }
                    }
                    None => return None,
                },
                () = self.cancel.cancelled() => return None,
            }
        }
    }

    /// One connection, from the open socket until it ends. `false` when the link task must end.
    async fn session(&mut self, stream: TcpStream) -> bool {
        let (events_tx, mut events) = mpsc::unbounded_channel();
        let handler = LinkHandler {
            params: Arc::clone(&self.params),
            max_players: self.max_players,
            events: events_tx,
        };
        let cipher = match LinkCipher::default_key() {
            Ok(cipher) => cipher,
            Err(e) => {
                // The key is a constant, so this cannot happen.
                warn!("Cannot set up the login server link cipher: {e}");
                return true;
            }
        };
        let connection = match spawn_connection(
            stream,
            Arc::new(ConnectionConfig::default()),
            Box::new(LinkFrameCipher(cipher)),
            handler,
            self.cancel.child_token(),
        ) {
            Ok(connection) => connection,
            Err(e) => {
                warn!("LoginServer not available, trying to reconnect... ({e})");
                return true;
            }
        };
        self.connection = Some(connection);

        let mut stop = None;
        loop {
            tokio::select! {
                event = events.recv() => match event {
                    Some(LinkEvent::Packet(packet)) => self.on_packet(packet),
                    Some(LinkEvent::Closed) | None => break,
                },
                command = self.commands.recv() => match command {
                    Some(command) => {
                        if let Flow::Stop(done) = self.handle_command(command) {
                            stop = Some(done);
                            break;
                        }
                    }
                    None => {
                        stop = Some(oneshot::channel().0);
                        break;
                    }
                },
                Some(finished) = self.lookups.join_next(), if !self.lookups.is_empty() => {
                    self.finish_lookup(finished);
                }
                () = self.cancel.cancelled() => {
                    stop = Some(oneshot::channel().0);
                    break;
                }
            }
        }

        if let Some(connection) = &self.connection {
            // Queued behind the packets sent above, so the status reaches the login server.
            connection.close();
        }
        if stop.is_some() {
            // Wait until the writer is done, so the last packets are on the wire.
            let _ = tokio::time::timeout(SHUTDOWN_WAIT, async {
                while let Some(event) = events.recv().await {
                    if matches!(event, LinkEvent::Closed) {
                        break;
                    }
                }
            })
            .await;
        }
        self.connection_lost();
        match stop {
            Some(done) => {
                let _ = done.send(());
                false
            }
            None => true,
        }
    }

    /// Java: the `finally` of `run` and the end of the read loop. Clients that were waiting for
    /// the answer get `false`: it can no longer come.
    fn connection_lost(&mut self) {
        info!("Disconnected from the login server.");
        self.connection = None;
        self.registered = false;
        for (account, reply) in self.waiting.drain() {
            self.accounts.remove(&account);
            let _ = reply.send(false);
        }
        self.lookups.abort_all();
        self.publish();
    }

    fn publish(&self) {
        self.status_tx.send_replace(LinkStatus {
            registered: self.registered,
            server_id: self.server_id,
            server_name: self.server_name.clone(),
            server_status: self.status,
            max_players: self.max_players,
        });
    }

    /// Queues a packet if the link carries game packets. Java `sendPacket` is silent when there
    /// is no socket.
    fn send<P: Encode>(&self, packet: &P) -> bool {
        match &self.connection {
            Some(connection) if self.registered => connection.send(packet.to_bytes()),
            _ => false,
        }
    }

    fn handle_command(&mut self, command: Command) -> Flow {
        match command {
            Command::AuthPlayer { account, key, reply } => self.auth_player(account, key, reply),
            Command::PlayerInGame { account } => {
                self.send(&PlayerInGame {
                    accounts: vec![account.clone()],
                });
                self.accounts.insert(account.clone());
                self.in_game.insert(account);
            }
            Command::PlayerLogout { account } => self.logout(&account),
            Command::AccessLevel { account, level } => {
                self.send(&ChangeAccessLevel { account, level });
            }
            Command::ReplyCharacters {
                account,
                chars,
                delete_times,
            } => {
                self.send(&ReplyCharacters {
                    account,
                    chars,
                    delete_times,
                });
            }
            Command::ServerStatus { id, value } => {
                self.send(&ServerStatus {
                    attributes: vec![(id, value)],
                });
            }
            Command::SetStatus(status) => {
                self.send(&ServerStatus {
                    attributes: vec![(ServerStatus::SERVER_LIST_STATUS, status)],
                });
                self.status = status;
                self.publish();
            }
            Command::SetMaxPlayer(max_players) => {
                self.send(&ServerStatus {
                    attributes: vec![(ServerStatus::MAX_PLAYERS, max_players)],
                });
                self.max_players = max_players;
                self.publish();
            }
            Command::Stop { done } => {
                if self.registered {
                    self.send(&ServerStatus {
                        attributes: vec![(ServerStatus::SERVER_LIST_STATUS, ServerStatus::STATUS_DOWN)],
                    });
                }
                return Flow::Stop(done);
            }
        }
        Flow::Continue
    }

    // Java: addGameServerLogin, addWaitingClientAndSendRequest
    fn auth_player(&mut self, account: String, key: SessionKey, reply: oneshot::Sender<bool>) {
        if !self.registered {
            warn!("Cannot check the session of an account: not registered at the login server.");
            let _ = reply.send(false);
            return;
        }
        // Preventing a duplicate login on this game server.
        if !self.accounts.insert(account.clone()) {
            let _ = reply.send(false);
            return;
        }
        let request = PlayerAuthRequest {
            account: account.clone(),
            play_ok1: key.play_ok1,
            play_ok2: key.play_ok2,
            login_ok1: key.login_ok1,
            login_ok2: key.login_ok2,
        };
        if !self.send(&request) {
            self.accounts.remove(&account);
            let _ = reply.send(false);
            return;
        }
        self.waiting.insert(account, reply);
    }

    /// Java `sendLogout`.
    fn logout(&mut self, account: &str) {
        self.accounts.remove(account);
        self.in_game.remove(account);
        if let Some(reply) = self.waiting.remove(account) {
            let _ = reply.send(false);
        }
        self.send(&PlayerLogout {
            account: account.to_owned(),
        });
    }

    fn on_packet(&mut self, packet: LsPacket) {
        match packet {
            LsPacket::InitLS(_) => {}
            // Java logs the reason and carries on; the login server closes the link.
            LsPacket::LoginServerFail(fail) => info!("Registration failed: {}", fail_reason(fail.reason)),
            LsPacket::AuthResponse(response) => self.on_registered(response),
            LsPacket::PlayerAuthResponse(response) => self.on_player_auth(response),
            LsPacket::KickPlayer(kick) => self.on_kick(kick.account),
            LsPacket::RequestCharacters(request) => self.request_characters(request.account),
            // Java parses it and does nothing with it.
            LsPacket::ChangePasswordResponse(_) => debug!("ChangePasswordResponse ignored."),
        }
    }

    // Java: LoginServerThread case 0x02
    fn on_registered(&mut self, response: AuthResponse) {
        let server_id = response.server_id;
        match hex_to_string(&self.params.hex_id) {
            Ok(text) => hexid::save(&self.params.config_dir, i32::from(server_id), &text),
            Err(e) => warn!("Cannot save the hex id: {e}"),
        }
        info!(
            "Registered on login as Server {server_id}: {}",
            response.server_name
        );
        self.server_id = Some(server_id);
        self.server_name = response.server_name;
        self.registered = true;
        self.send(&self.params.registration_status());
        // The accounts that were in the game before the link broke.
        if !self.in_game.is_empty() {
            self.send(&PlayerInGame {
                accounts: self.in_game.iter().cloned().collect(),
            });
        }
        self.publish();
    }

    // Java: LoginServerThread case 0x03
    fn on_player_auth(&mut self, response: PlayerAuthResponse) {
        let PlayerAuthResponse { account, authed } = response;
        // Java ignores an answer nobody waits for.
        let Some(reply) = self.waiting.remove(&account) else {
            return;
        };
        if authed {
            self.send(&PlayerInGame {
                accounts: vec![account.clone()],
            });
            self.in_game.insert(account);
        } else {
            warn!("Session key incorrect. Closing connection for account {account}");
            self.logout(&account);
        }
        // The caller may be gone (its client disconnected while it waited); it then calls
        // `player_logout` like any ended session.
        let _ = reply.send(authed);
    }

    // Java: LoginServerThread.doKickPlayer
    fn on_kick(&mut self, account: String) {
        let can_kick = self.accounts.contains(&account) && self.kicks_taken.taken.load(Ordering::Acquire);
        if can_kick
            && self
                .kick_sender
                .send(KickRequest {
                    account: account.clone(),
                })
                .is_ok()
        {
            info!("Kicked by login: {account}");
            return;
        }
        // Nobody to kick: the account is free as far as this game server is concerned.
        self.logout(&account);
    }

    // Java: LoginServerThread.getCharsOnServer
    fn request_characters(&mut self, account: String) {
        let characters = Arc::clone(&self.characters);
        self.lookups.spawn(async move {
            let result = characters.delete_times(&account).await;
            (account, result)
        });
    }

    fn finish_lookup(
        &mut self,
        finished: Result<(String, Result<Vec<u64>, String>), tokio::task::JoinError>,
    ) {
        let (account, result) = match finished {
            Ok(done) => done,
            Err(e) => {
                warn!("Exception in getCharsOnServer: {e}");
                return;
            }
        };
        let times = result.unwrap_or_else(|e| {
            warn!("Exception in getCharsOnServer: {e}");
            Vec::new()
        });
        let chars = u8::try_from(times.len()).unwrap_or(u8::MAX);
        let delete_times = times
            .into_iter()
            .filter(|&time| time != 0)
            .filter_map(|time| i64::try_from(time).ok())
            .collect();
        self.send(&ReplyCharacters {
            account,
            chars,
            delete_times,
        });
    }
}

/// Java `LoginServerFail.REASONS`.
fn fail_reason(reason: u8) -> String {
    match reason {
        0 => "None".to_owned(),
        1 => "Reason: ip banned".to_owned(),
        2 => "Reason: ip reserved".to_owned(),
        3 => "Reason: wrong hexid".to_owned(),
        4 => "Reason: id reserved".to_owned(),
        5 => "Reason: no free ID".to_owned(),
        6 => "Not authed".to_owned(),
        7 => "Reason: already logged in".to_owned(),
        other => format!("Reason: {other}"),
    }
}

/// Starts the link in `server`: reads the settings (`Server.ini`, `ipconfig.xml`, `hexid.txt`),
/// spawns the link task and keeps the handle in the server.
// Java: GameServer constructor, `LoginServerThread.getInstance().start()`
pub fn start(server: &mut Server) -> LsLink {
    let state = Arc::clone(server.state());
    let config_dir = state.paths.config_dir.clone();
    // `ServerListType` is one of the settings the config crate leaves to hand-written code.
    let server_list_type = IniFile::load(config_dir.join(SERVER_CONFIG_FILE))
        .map(|ini| server_type_id(&ini.get_string("ServerListType", "Free")))
        .unwrap_or_else(|_| server_type_id("Free"));
    let params = LsLinkParams::from_config(
        &state.config,
        server_list_type,
        IpConfig::load(&config_dir),
        state.hexid.as_ref(),
        config_dir,
    );
    let characters = Arc::new(DbCharacterCounts::new(state.pool.clone()));
    let (link, task) = LsLink::new(params, characters, server.cancel_token());
    server.spawn(task);
    server.ls_link = Some(link.clone());
    link
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(hexid: Option<&HexId>) -> LsLinkParams {
        let config = GameConfig::default();
        LsLinkParams::from_config(
            &config,
            server_type_id("Free"),
            IpConfig::parse(r#"<gameserver address="1.2.3.4"/>"#),
            hexid,
            PathBuf::from("config"),
        )
    }

    #[test]
    fn server_types_are_flags_like_java() {
        assert_eq!(server_type_id("Free"), 0x40);
        assert_eq!(server_type_id("Normal,Relax"), 0x03);
        assert_eq!(
            server_type_id(" test , NOLABEL ,restricted,event"),
            0x04 | 0x08 | 0x10 | 0x20
        );
        assert_eq!(server_type_id("bogus"), 0);
        assert_eq!(server_type_id(""), 0);
    }

    #[test]
    fn a_hexid_file_decides_the_identity() {
        let hexid = HexId {
            server_id: 7,
            hex_id: vec![0x2a, 0xd6],
        };
        let p = params(Some(&hexid));
        assert_eq!((p.request_id, p.hex_id.as_slice()), (7, [0x2a, 0xd6].as_slice()));
    }

    #[test]
    fn without_a_hexid_file_the_secret_is_new_and_the_id_is_requested() {
        let config = GameConfig::default();
        let a = params(None);
        let b = params(None);
        assert_eq!(a.request_id, config.server.request_id);
        assert_eq!(a.hex_id.len(), HEX_ID_SIZE);
        assert!(a.hex_id.iter().all(|&byte| byte != 0));
        assert_ne!(a.hex_id, b.hex_id);
    }

    #[test]
    fn the_auth_request_carries_the_configuration() {
        let hexid = HexId {
            server_id: 7,
            hex_id: vec![9, 8, 7],
        };
        let p = params(Some(&hexid));
        let request = p.auth_request(p.max_players);
        assert_eq!(request.desired_id, 7);
        assert_eq!(request.hex_id, [9, 8, 7]);
        assert_eq!(request.subnets, ["0.0.0.0/0"]);
        assert_eq!(request.hosts, ["1.2.3.4"]);
        assert!(!request.reserve_host);
        assert_eq!(
            request.accept_alternate,
            GameConfig::default().server.accept_alternate_id
        );
    }

    #[test]
    fn the_registration_status_is_what_java_sends() {
        let mut p = params(None);
        p.server_list_brackets = true;
        p.gm_only = true;
        p.server_list_age = 18;
        p.server_list_type = 0x41;
        assert_eq!(
            p.registration_status().attributes,
            vec![(0x03, 1), (0x02, 0x41), (0x01, 5), (0x05, 0x12)]
        );
        p.server_list_brackets = false;
        p.gm_only = false;
        p.server_list_age = 15;
        assert_eq!(
            p.registration_status().attributes,
            vec![(0x03, 0), (0x02, 0x41), (0x01, 0), (0x05, 0x0F)]
        );
        p.server_list_age = 7;
        assert_eq!(p.registration_status().attributes[3], (0x05, 0));
    }

    #[test]
    fn the_params_debug_output_hides_the_secret() {
        let p = params(Some(&HexId {
            server_id: 1,
            hex_id: vec![0xde, 0xad, 0xbe, 0xef],
        }));
        let shown = format!("{p:?}");
        assert!(
            !shown.contains("222") && !shown.to_lowercase().contains("dead"),
            "{shown}"
        );
    }

    #[test]
    fn failure_reasons_match_java() {
        assert_eq!(fail_reason(3), "Reason: wrong hexid");
        assert_eq!(fail_reason(7), "Reason: already logged in");
        assert_eq!(fail_reason(99), "Reason: 99");
    }
}
