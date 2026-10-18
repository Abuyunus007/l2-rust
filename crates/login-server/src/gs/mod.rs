//! The game server side of the login server: the listener on `LoginPort` and one connection
//! task per game server.
// Java: org/l2jmobius/loginserver/GameServerListener.java
// Java: org/l2jmobius/loginserver/util/FloodProtectorListener.java
// Java: org/l2jmobius/loginserver/GameServerThread.java
// Java: org/l2jmobius/loginserver/network/GameServerPacketHandler.java
// Java: org/l2jmobius/loginserver/network/gameserverpackets/receive/BlowFishKey.java
// Java: org/l2jmobius/loginserver/network/gameserverpackets/receive/GameServerAuth.java
//
// Handshake, as the Java game server (`LoginServerThread`) drives it:
//
//   LS -> InitLS(protocol 0x0106, RSA-512 modulus)            default Blowfish key
//   GS -> BlowFishKey(RSA no-padding block with a 40-byte key) default key; both switch keys now
//   GS -> GameServerAuth(...)                                  new key
//   LS -> AuthResponse(id, name)  or  LoginServerFail(reason) + close
//
// The key switch is `Control::set_cipher` right after the BlowFishKey frame: l2-net handles
// frames strictly in order, so the game server's next frame is read with the new key.
//
// Layout: `link` = cipher + the `GsLink` handle, `table` = operations on the registered
// servers, `register` = the `GameServerAuth` decision and saving new servers, `subnet` = the
// address list.

pub mod link;
pub mod register;
pub mod subnet;
pub mod table;

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use l2_config::LoginConfig;
use l2_crypto::LinkCipher;
use l2_crypto::rsa::strip_leading_zeros;
use l2_net::{
    ConnectionConfig, Control, Encode, FrameCipher, FrameHandler, PacketError, PacketReader, listen,
};
use l2_protocol_login::link::{
    AuthResponse, GameServerAuth, GsPacket, InitLS, LinkError, LinkState, LoginServerFail, decode_from_gs,
};
use l2_util::time::now_millis;
use tokio::net::{TcpListener, lookup_host};
use tracing::{debug, error, info, warn};

use crate::gs::link::{GsLink, LinkFrameCipher};
use crate::gs::register::{GameServerStore, save_new_server};
use crate::state::LoginState;
use crate::{Server, StartupError};

static NEXT_LINK_ID: AtomicU64 = AtomicU64::new(1);

/// Opens the game server listener on `LoginHostname:LoginPort` and runs it until shutdown.
/// Java exits with a fatal error when the port cannot be bound; the error here does the same.
// Java: LoginServer.startGameServerListener
pub(crate) async fn start(server: &mut Server) -> Result<(), StartupError> {
    let store = Arc::new(server.state().repository.clone());
    start_with_store(server, store).await
}

/// [`start`] with another place to save new game servers (tests).
pub(crate) async fn start_with_store<S: GameServerStore>(
    server: &mut Server,
    store: Arc<S>,
) -> Result<(), StartupError> {
    let state = Arc::clone(server.state());
    let listener = bind(&state.config.login).await?;
    let address = listener
        .local_addr()
        .map_err(|source| listen_error(&state, source))?;
    state.game_servers.set_listen_addr(address);
    info!(
        "Game server listener is listening on {}:{}.",
        state.config.login.game_server_login_host,
        address.port()
    );
    let cancel = server.cancel_token();
    server.spawn(serve(listener, state, store, cancel));
    Ok(())
}

fn listen_error(state: &LoginState, source: std::io::Error) -> StartupError {
    let login = &state.config.login;
    StartupError::Listen {
        what: "Game server",
        address: format!(
            "{}:{}",
            login.game_server_login_host, login.game_server_login_port
        ),
        source,
    }
}

/// Binds `LoginHostname` (`*` = all interfaces) and `LoginPort`.
// Java: FloodProtectorListener constructor
async fn bind(login: &LoginConfig) -> Result<TcpListener, StartupError> {
    let describe = |source| StartupError::Listen {
        what: "Game server",
        address: format!(
            "{}:{}",
            login.game_server_login_host, login.game_server_login_port
        ),
        source,
    };
    let port = u16::try_from(login.game_server_login_port).map_err(|_| {
        describe(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "LoginPort is not a TCP port",
        ))
    })?;
    let address = if login.game_server_login_host == "*" {
        SocketAddr::from(([0, 0, 0, 0], port))
    } else {
        lookup_host((login.game_server_login_host.as_str(), port))
            .await
            .map_err(describe)?
            .next()
            .ok_or_else(|| {
                describe(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "LoginHostname did not resolve",
                ))
            })?
    };
    TcpListener::bind(address).await.map_err(describe)
}

/// Accepts game server connections until the token is cancelled.
// Java: FloodProtectorListener.run, GameServerListener.addClient
async fn serve<S: GameServerStore>(
    listener: TcpListener,
    state: Arc<LoginState>,
    store: Arc<S>,
    cancel: l2_net::CancellationToken,
) {
    let flood = Arc::new(FloodProtector::new(&state.config.login));
    listen(
        listener,
        Arc::new(ConnectionConfig::default()),
        cancel,
        move |peer: SocketAddr| {
            if !flood.admit(peer.ip(), now_millis()) {
                return None;
            }
            let cipher = match LinkCipher::default_key() {
                Ok(cipher) => cipher,
                Err(error) => {
                    // The key is a constant, so this cannot happen.
                    error!("cannot set up the game server link cipher: {error}");
                    flood.release(peer.ip());
                    return None;
                }
            };
            let handler = GsConnection::new(Arc::clone(&state), Arc::clone(&store), Arc::clone(&flood));
            Some((
                handler,
                Box::new(LinkFrameCipher::new(cipher)) as Box<dyn FrameCipher>,
            ))
        },
    )
    .await;
}

/// Java `isBannedGameserverIP`.
// NOTE(java-quirk): this is a TODO stub in Java that always answers false, so no game server
// address is ever refused.
pub fn is_banned_game_server_ip(_address: IpAddr) -> bool {
    false
}

#[derive(Debug)]
struct Visitor {
    connections: i32,
    last_connection: i64,
    flooding: bool,
}

/// Refuses addresses that connect too often or with too many connections at once.
// Java: FloodProtectorListener (`run`, `removeFloodProtection`)
pub struct FloodProtector {
    enabled: bool,
    fast_connection_limit: i32,
    normal_connection_time: i64,
    fast_connection_time: i64,
    max_connection_per_ip: i32,
    visitors: Mutex<HashMap<IpAddr, Visitor>>,
}

impl FloodProtector {
    pub fn new(login: &LoginConfig) -> Self {
        Self {
            enabled: login.flood_protection,
            fast_connection_limit: login.fast_connection_limit,
            normal_connection_time: i64::from(login.normal_connection_time),
            fast_connection_time: i64::from(login.fast_connection_time),
            max_connection_per_ip: login.max_connection_per_ip,
            visitors: Mutex::new(HashMap::new()),
        }
    }

    fn visitors(&self) -> std::sync::MutexGuard<'_, HashMap<IpAddr, Visitor>> {
        self.visitors
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Called for each accepted connection. False means close it right away.
    pub fn admit(&self, address: IpAddr, now_ms: i64) -> bool {
        if !self.enabled {
            return true;
        }
        let mut visitors = self.visitors();
        let Some(visitor) = visitors.get_mut(&address) else {
            visitors.insert(
                address,
                Visitor {
                    connections: 1,
                    last_connection: now_ms,
                    flooding: false,
                },
            );
            return true;
        };
        visitor.connections += 1;
        let since_last = now_ms - visitor.last_connection;
        if (visitor.connections > self.fast_connection_limit && since_last < self.normal_connection_time)
            || since_last < self.fast_connection_time
            || visitor.connections > self.max_connection_per_ip
        {
            visitor.last_connection = now_ms;
            visitor.connections -= 1;
            if !visitor.flooding {
                warn!("Potential flood from address {address}.");
            }
            visitor.flooding = true;
            return false;
        }
        if visitor.flooding {
            visitor.flooding = false;
            info!("Address {address} is no longer considered as flooding.");
        }
        visitor.last_connection = now_ms;
        true
    }

    /// Called when an admitted connection ended.
    pub fn release(&self, address: IpAddr) {
        if !self.enabled {
            return;
        }
        let mut visitors = self.visitors();
        match visitors.get_mut(&address) {
            Some(visitor) => {
                visitor.connections -= 1;
                if visitor.connections == 0 {
                    visitors.remove(&address);
                }
            }
            None => warn!("Attempted to remove flood protection for unknown IP {address}."),
        }
    }
}

/// The login server's side of one game server connection.
// Java: GameServerThread + GameServerPacketHandler
struct GsConnection<S: GameServerStore> {
    state: Arc<LoginState>,
    store: Arc<S>,
    flood: Arc<FloodProtector>,
    link_state: LinkState,
    link: Option<Arc<GsLink>>,
    /// Which of the RSA-512 pairs this link uses (Java `_privateKey` / `_publicKey`).
    key_index: Option<usize>,
    /// Set once the server is registered; the id of its row in the table.
    server_id: Option<i32>,
    peer_ip: Option<IpAddr>,
}

impl<S: GameServerStore> GsConnection<S> {
    fn new(state: Arc<LoginState>, store: Arc<S>, flood: Arc<FloodProtector>) -> Self {
        Self {
            state,
            store,
            flood,
            link_state: LinkState::Connected,
            link: None,
            key_index: None,
            server_id: None,
            peer_ip: None,
        }
    }

    /// Sends the reason, then closes the link.
    // Java: GameServerThread.forceClose
    fn force_close(ctl: &mut Control<'_>, reason: u8) {
        ctl.send_and_close(LoginServerFail { reason }.to_bytes());
    }

    // Java: BlowFishKey (constructor)
    fn handle_blowfish_key(&mut self, encrypted_key: &[u8], ctl: &mut Control<'_>) {
        // NOTE(java-quirk): all four problems below only log and leave the link in the CONNECTED
        // state. The game server is not told, and it can send another BlowFishKey.
        if encrypted_key.is_empty() {
            warn!("Invalid RSA block size for Blowfish key: 0");
            return;
        }
        let Some(pair) = self
            .key_index
            .and_then(|index| self.state.game_servers.key_pairs().get(index))
        else {
            return;
        };
        let decrypted = match pair.decrypt_raw(encrypted_key) {
            Ok(block) => block,
            Err(error) => {
                error!("Error while decrypting Blowfish key (RSA). {error}");
                return;
            }
        };
        // NOTE(java-quirk): leading zero bytes are dropped from the decrypted block. A random
        // key that starts with 0x00 therefore ends up shorter (and wrong) on this side.
        let key = strip_leading_zeros(&decrypted);
        if key.is_empty() {
            warn!("Decrypted Blowfish key is empty.");
            return;
        }
        match LinkCipher::new(key) {
            Ok(cipher) => {
                ctl.set_cipher(Box::new(LinkFrameCipher::new(cipher)));
                self.link_state = LinkState::BfConnected;
            }
            Err(error) => error!("Error while decrypting Blowfish key (RSA). {error}"),
        }
    }

    // Java: GameServerAuth (constructor, then `handleRegProcess`)
    async fn handle_auth(&mut self, request: &GameServerAuth, ctl: &mut Control<'_>) {
        let Some(link) = self.link.clone() else {
            return;
        };
        let accept_new = self.state.config.login.accept_new_gameserver;
        match self
            .state
            .game_servers
            .register_game_server(accept_new, request, &link)
        {
            Err(reason) => Self::force_close(ctl, reason),
            Ok(registration) => {
                self.server_id = Some(registration.server_id);
                if registration.is_new {
                    save_new_server(&*self.store, &registration).await;
                }
                let server_id = registration.server_id;
                ctl.send(
                    AuthResponse {
                        // Java `writeByte(serverId)`.
                        server_id: server_id as u8,
                        server_name: self
                            .state
                            .game_servers
                            .server_name(server_id)
                            .unwrap_or_default()
                            .to_owned(),
                    }
                    .to_bytes(),
                );
                self.link_state = LinkState::Authed;
            }
        }
    }

    /// Packets of a registered game server.
    // TODO(M1-LS-05): PlayerInGame, PlayerLogout, ChangeAccessLevel, PlayerAuthRequest,
    // ServerStatus, PlayerTracert, ReplyCharacters, RequestTempBan and ChangePassword.
    fn handle_authed(&mut self, packet: GsPacket) {
        debug!("game server packet not handled yet: {packet:?}");
    }
}

impl<S: GameServerStore> FrameHandler for GsConnection<S> {
    async fn on_connect(&mut self, ctl: &mut Control<'_>) {
        let peer_ip = ctl.peer().ip();
        self.peer_ip = Some(peer_ip);
        let link = GsLink::new(NEXT_LINK_ID.fetch_add(1, Ordering::Relaxed), ctl.handle().clone());
        self.link = Some(Arc::new(link));

        if is_banned_game_server_ip(peer_ip) {
            info!(
                "GameServerRegistration: IP Address {peer_ip} is on the banned IP list. Connection rejected."
            );
            Self::force_close(ctl, LoginServerFail::REASON_IP_BANNED);
            return;
        }

        let Some((index, pair)) = self.state.random_game_server_key() else {
            error!("No RSA keys for game server links; closing the connection.");
            ctl.close();
            return;
        };
        self.key_index = Some(index);
        ctl.send(InitLS::new(pair.java_to_byte_array_modulus()).to_bytes());
    }

    async fn on_frame(&mut self, body: Vec<u8>, ctl: &mut Control<'_>) {
        let Some((&opcode, rest)) = body.split_first() else {
            warn!("Received empty packet from game server {:?}", self.peer_ip);
            Self::force_close(ctl, LoginServerFail::NOT_AUTHED);
            return;
        };
        let mut reader = PacketReader::new(rest);
        let packet = match decode_from_gs(self.link_state, opcode, &mut reader) {
            Ok(packet) => packet,
            Err(LinkError::UnknownOpcode { opcode, state }) => {
                warn!(
                    "Unknown opcode ({opcode:02X}) in state {state:?} from {:?}, closing connection.",
                    self.peer_ip
                );
                Self::force_close(ctl, LoginServerFail::NOT_AUTHED);
                return;
            }
            Err(LinkError::Packet(PacketError::Invalid { field, reason }))
                if opcode == GameServerAuth::OPCODE =>
            {
                // Java checks the hex id size and the host pair count first and answers both
                // with WRONG_HEXID.
                warn!("Rejected GS auth: invalid {field}: {reason}.");
                Self::force_close(ctl, LoginServerFail::REASON_WRONG_HEXID);
                return;
            }
            Err(LinkError::Packet(error)) => {
                // Java throws out of its read loop on a short packet, which ends the connection.
                warn!("Bad packet from game server {:?}: {error}", self.peer_ip);
                ctl.close();
                return;
            }
        };
        match packet {
            None => {}
            Some(GsPacket::BlowFishKey(key)) => self.handle_blowfish_key(&key.encrypted_key, ctl),
            Some(GsPacket::GameServerAuth(request)) => self.handle_auth(&request, ctl).await,
            Some(other) => self.handle_authed(other),
        }
    }

    async fn on_disconnect(&mut self) {
        // Java: the `finally` block of `GameServerThread.run`.
        if let (Some(server_id), Some(link)) = (self.server_id, &self.link) {
            self.state
                .game_servers
                .link_closed(server_id, link.id(), self.state.status());
        }
        if let Some(address) = self.peer_ip {
            self.flood.release(address);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use l2_crypto::rsa::encrypt_raw;
    use l2_net::Encode;
    use l2_protocol_login::link::{
        BlowFishKey, KickPlayer, LsPacket, PROTOCOL_REV, PlayerInGame, RequestCharacters, decode_from_ls,
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;
    use tokio::task::JoinSet;
    use tokio::time::timeout;

    use super::register::tests::{RecordingStore, auth};
    use super::*;
    use crate::state::test_support::{default_config, state_with};
    use crate::state::{ExitReason, ServerStatus};

    /// A game server speaking the real link protocol, the way `LoginServerThread` does.
    struct FakeGs {
        stream: TcpStream,
        cipher: LinkCipher,
    }

    const WAIT: Duration = Duration::from_secs(10);

    impl FakeGs {
        async fn connect(address: SocketAddr) -> Self {
            Self {
                stream: TcpStream::connect(address).await.unwrap(),
                cipher: LinkCipher::default_key().unwrap(),
            }
        }

        /// The next packet's plaintext (padding and checksum slot included), or `None` when the
        /// login server closed the connection.
        async fn recv(&mut self) -> Option<Vec<u8>> {
            let mut header = [0u8; 2];
            timeout(WAIT, self.stream.read_exact(&mut header))
                .await
                .unwrap()
                .ok()?;
            let length = usize::from(u16::from_le_bytes(header));
            let mut body = vec![0u8; length - 2];
            timeout(WAIT, self.stream.read_exact(&mut body))
                .await
                .unwrap()
                .ok()?;
            Some(self.cipher.open(&mut body).unwrap().to_vec())
        }

        async fn recv_ls(&mut self) -> Option<LsPacket> {
            let body = self.recv().await?;
            Some(decode_from_ls(body[0], &mut PacketReader::new(&body[1..])).unwrap())
        }

        async fn send_raw(&mut self, payload: &[u8]) {
            let body = self.cipher.seal(payload);
            let mut frame = u16::try_from(body.len() + 2).unwrap().to_le_bytes().to_vec();
            frame.extend_from_slice(&body);
            self.stream.write_all(&frame).await.unwrap();
        }

        async fn send<P: Encode>(&mut self, packet: &P) {
            self.send_raw(&packet.to_bytes()).await;
        }

        async fn read_init(&mut self) -> InitLS {
            match self.recv_ls().await {
                Some(LsPacket::InitLS(init)) => init,
                other => panic!("expected InitLS, got {other:?}"),
            }
        }

        /// Everything up to and including the key switch, like `LoginServerThread` case 0x00.
        async fn handshake(&mut self, session_key: &[u8]) -> InitLS {
            let init = self.read_init().await;
            let encrypted = encrypt_raw(&init.public_key, &[1, 0, 1], session_key).unwrap();
            self.send(&BlowFishKey {
                encrypted_key: encrypted,
            })
            .await;
            self.cipher = LinkCipher::new(session_key).unwrap();
            init
        }

        async fn is_closed(&mut self) -> bool {
            self.recv().await.is_none()
        }
    }

    /// A 40-byte session key whose first byte is not zero (the LS strips leading zeros).
    fn session_key() -> Vec<u8> {
        (0..40u8).map(|i| i.wrapping_mul(7).wrapping_add(0x11)).collect()
    }

    struct Harness {
        server: Server,
        address: SocketAddr,
        store: Arc<RecordingStore>,
    }

    impl Harness {
        async fn new() -> Self {
            Self::with_config(|_| {}).await
        }

        /// A server on a free loopback port. Flood protection is off unless `tweak` turns it on.
        async fn with_config(tweak: impl FnOnce(&mut l2_config::LoginConfigSet)) -> Self {
            let mut config = default_config();
            config.login.game_server_login_host = "127.0.0.1".to_owned();
            config.login.game_server_login_port = 0;
            config.login.flood_protection = false;
            tweak(&mut config);
            let (state, shutdown_requests) = state_with(config);
            let mut server = Server {
                state: Arc::new(state),
                shutdown_requests,
                tasks: JoinSet::new(),
                cancel: l2_net::CancellationToken::new(),
            };
            let store = Arc::new(RecordingStore::default());
            start_with_store(&mut server, Arc::clone(&store)).await.unwrap();
            let address = server.state().game_servers.listen_addr().unwrap();
            Self {
                server,
                address,
                store,
            }
        }

        fn state(&self) -> &Arc<LoginState> {
            self.server.state()
        }

        async fn gs(&self) -> FakeGs {
            FakeGs::connect(self.address).await
        }

        /// A registered game server.
        async fn authed_gs(&self, request: &GameServerAuth) -> FakeGs {
            let mut gs = self.gs().await;
            gs.handshake(&session_key()).await;
            gs.send(request).await;
            assert!(matches!(gs.recv_ls().await, Some(LsPacket::AuthResponse(_))));
            gs
        }

        async fn finish(self) {
            assert_eq!(self.server.shutdown(false).await, ExitReason::Shutdown);
        }
    }

    async fn until(mut condition: impl FnMut() -> bool) {
        for _ in 0..200 {
            if condition() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        panic!("condition not met in time");
    }

    #[tokio::test]
    async fn the_listener_binds_and_sends_init_ls() {
        let harness = Harness::new().await;
        let mut gs = harness.gs().await;
        let init = gs.read_init().await;
        assert_eq!(init.protocol_revision, PROTOCOL_REV);
        assert_eq!(init.protocol_revision, 0x0106);
        let state = harness.state();
        assert!(
            state
                .game_servers
                .key_pairs()
                .iter()
                .any(|pair| pair.java_to_byte_array_modulus() == init.public_key),
            "the modulus is one of the pool's RSA-512 keys"
        );
        harness.finish().await;
    }

    #[tokio::test]
    async fn a_known_server_registers_and_gets_its_name() {
        let harness = Harness::new().await;
        let mut gs = harness.gs().await;
        gs.handshake(&session_key()).await;
        gs.send(&auth(1, &[0x12, 0x34], true)).await;
        assert_eq!(
            gs.recv_ls().await,
            Some(LsPacket::AuthResponse(AuthResponse {
                server_id: 1,
                server_name: "Bartz".to_owned(),
            }))
        );
        let row = harness.state().game_servers.registered(1).unwrap();
        assert!(row.authed);
        assert_eq!(row.port, 7777);
        assert_eq!(row.max_players, 100);
        assert!(row.link.is_some());
        assert!(
            harness.store.saved.lock().unwrap().is_empty(),
            "a known server is not saved again"
        );
        harness.finish().await;
    }

    #[tokio::test]
    async fn a_new_server_is_registered_and_saved() {
        let harness = Harness::new().await;
        let mut gs = harness.gs().await;
        gs.handshake(&session_key()).await;
        gs.send(&auth(2, &[0x2A, 0xD6, 0x6B], false)).await;
        assert_eq!(
            gs.recv_ls().await,
            Some(LsPacket::AuthResponse(AuthResponse {
                server_id: 2,
                server_name: "Sieghardt".to_owned(),
            }))
        );
        assert!(harness.state().game_servers.registered(2).unwrap().authed);
        assert_eq!(
            *harness.store.saved.lock().unwrap(),
            [(2, "2ad66b".to_owned(), "play.example.org".to_owned())]
        );
        harness.finish().await;
    }

    #[tokio::test]
    async fn an_unnamed_id_is_answered_with_an_empty_name() {
        let harness = Harness::new().await;
        let mut gs = harness.gs().await;
        gs.handshake(&session_key()).await;
        gs.send(&auth(9, &[1], false)).await;
        assert_eq!(
            gs.recv_ls().await,
            Some(LsPacket::AuthResponse(AuthResponse {
                server_id: 9,
                server_name: String::new(),
            }))
        );
        harness.finish().await;
    }

    async fn expect_fail(gs: &mut FakeGs, reason: u8) {
        assert_eq!(
            gs.recv_ls().await,
            Some(LsPacket::LoginServerFail(LoginServerFail { reason }))
        );
        assert!(gs.is_closed().await, "the link is closed after a refusal");
    }

    #[tokio::test]
    async fn wrong_hex_id_is_refused_with_reason_3() {
        let harness = Harness::new().await;
        let mut gs = harness.gs().await;
        gs.handshake(&session_key()).await;
        gs.send(&auth(1, &[9, 9], false)).await;
        expect_fail(&mut gs, LoginServerFail::REASON_WRONG_HEXID).await;
        harness.finish().await;
    }

    #[tokio::test]
    async fn new_servers_can_be_switched_off() {
        let harness = Harness::with_config(|config| config.login.accept_new_gameserver = false).await;
        let mut gs = harness.gs().await;
        gs.handshake(&session_key()).await;
        gs.send(&auth(5, &[9, 9], true)).await;
        expect_fail(&mut gs, LoginServerFail::REASON_WRONG_HEXID).await;
        assert!(!harness.state().game_servers.has_registered(5));
        harness.finish().await;
    }

    #[tokio::test]
    async fn a_second_session_gets_reason_7_and_the_first_stays() {
        let harness = Harness::new().await;
        let mut first = harness.authed_gs(&auth(1, &[0x12, 0x34], false)).await;
        let mut second = harness.gs().await;
        second.handshake(&session_key()).await;
        second.send(&auth(1, &[0x12, 0x34], false)).await;
        expect_fail(&mut second, LoginServerFail::REASON_ALREADY_LOGGED_IN).await;
        assert!(harness.state().game_servers.registered(1).unwrap().authed);
        // The first link still works: the table can reach it.
        harness.state().game_servers.request_characters("somebody");
        assert_eq!(
            first.recv_ls().await,
            Some(LsPacket::RequestCharacters(RequestCharacters {
                account: "somebody".to_owned()
            }))
        );
        harness.finish().await;
    }

    #[tokio::test]
    async fn an_alternative_id_is_assigned_when_allowed() {
        let harness = Harness::new().await;
        let mut gs = harness.gs().await;
        gs.handshake(&session_key()).await;
        gs.send(&auth(1, &[9, 9], true)).await;
        assert_eq!(
            gs.recv_ls().await,
            Some(LsPacket::AuthResponse(AuthResponse {
                server_id: 2,
                server_name: "Sieghardt".to_owned(),
            }))
        );
        assert_eq!(harness.store.saved.lock().unwrap().len(), 1);
        assert_eq!(harness.store.saved.lock().unwrap()[0].0, 2);
        harness.finish().await;
    }

    #[tokio::test]
    async fn no_free_id_is_reason_5() {
        let harness = Harness::new().await;
        let _first = harness.authed_gs(&auth(1, &[9, 9], true)).await; // takes id 2
        let mut gs = harness.gs().await;
        gs.handshake(&session_key()).await;
        gs.send(&auth(1, &[7, 7], true)).await;
        expect_fail(&mut gs, LoginServerFail::REASON_NO_FREE_ID).await;
        harness.finish().await;
    }

    #[tokio::test]
    async fn oversize_hex_id_or_host_list_is_refused_with_reason_3() {
        let harness = Harness::new().await;
        let mut gs = harness.gs().await;
        gs.handshake(&session_key()).await;
        gs.send(&auth(1, &[1; 65], false)).await;
        expect_fail(&mut gs, LoginServerFail::REASON_WRONG_HEXID).await;

        let mut gs = harness.gs().await;
        gs.handshake(&session_key()).await;
        let mut many = auth(1, &[0x12, 0x34], false);
        many.subnets = vec!["0.0.0.0/0".to_owned(); 33];
        many.hosts = vec!["h".to_owned(); 33];
        gs.send(&many).await;
        expect_fail(&mut gs, LoginServerFail::REASON_WRONG_HEXID).await;
        harness.finish().await;
    }

    #[tokio::test]
    async fn a_packet_in_the_wrong_state_closes_the_link_with_reason_6() {
        let harness = Harness::new().await;
        // GameServerAuth before the BlowFishKey.
        let mut gs = harness.gs().await;
        gs.read_init().await;
        gs.send(&auth(1, &[0x12, 0x34], false)).await;
        expect_fail(&mut gs, LoginServerFail::NOT_AUTHED).await;

        // A game packet before registering.
        let mut gs = harness.gs().await;
        gs.handshake(&session_key()).await;
        gs.send(&PlayerInGame {
            accounts: vec!["a".to_owned()],
        })
        .await;
        expect_fail(&mut gs, LoginServerFail::NOT_AUTHED).await;
        harness.finish().await;
    }

    #[tokio::test]
    async fn a_bad_blowfish_key_block_leaves_the_link_connected() {
        let harness = Harness::new().await;
        let mut gs = harness.gs().await;
        let init = gs.read_init().await;
        // Not smaller than the modulus: the RSA step fails. Java only logs this.
        gs.send(&BlowFishKey {
            encrypted_key: vec![0xFF; 64],
        })
        .await;
        // A size of zero is ignored as well.
        gs.send(&BlowFishKey {
            encrypted_key: vec![],
        })
        .await;
        // An all-zero block decrypts to nothing.
        gs.send(&BlowFishKey {
            encrypted_key: vec![0],
        })
        .await;
        // Still in the CONNECTED state with the default key, so a correct exchange works.
        let key = session_key();
        let encrypted = encrypt_raw(&init.public_key, &[1, 0, 1], &key).unwrap();
        gs.send(&BlowFishKey {
            encrypted_key: encrypted,
        })
        .await;
        gs.cipher = LinkCipher::new(&key).unwrap();
        gs.send(&auth(1, &[0x12, 0x34], false)).await;
        assert!(matches!(gs.recv_ls().await, Some(LsPacket::AuthResponse(_))));
        harness.finish().await;
    }

    #[tokio::test]
    async fn a_packet_with_a_broken_checksum_closes_the_link() {
        let harness = Harness::new().await;
        let mut gs = harness.gs().await;
        gs.read_init().await;
        // 8 bytes of garbage do not decrypt to a valid checksum.
        gs.stream
            .write_all(&[10, 0, 1, 2, 3, 4, 5, 6, 7, 8])
            .await
            .unwrap();
        assert!(gs.is_closed().await);
        harness.finish().await;
    }

    #[tokio::test]
    async fn going_away_marks_the_server_down_and_keeps_it_in_the_table() {
        let harness = Harness::new().await;
        let gs = harness.authed_gs(&auth(1, &[0x12, 0x34], false)).await;
        let table = &harness.state().game_servers;
        assert!(table.registered(1).unwrap().authed);
        drop(gs);
        until(|| !table.registered(1).unwrap().authed).await;
        let row = table.registered(1).unwrap();
        assert_eq!(row.status, ServerStatus::Down);
        assert_eq!(row.port, 0);
        assert!(row.link.is_none());
        assert!(table.has_registered(1));
        // It can come back.
        let _again = harness.authed_gs(&auth(1, &[0x12, 0x34], false)).await;
        assert!(table.registered(1).unwrap().authed);
        harness.finish().await;
    }

    #[tokio::test]
    async fn a_refused_second_session_leaving_does_not_take_the_first_down() {
        let harness = Harness::new().await;
        let _first = harness.authed_gs(&auth(1, &[0x12, 0x34], false)).await;
        let mut second = harness.gs().await;
        second.handshake(&session_key()).await;
        second.send(&auth(1, &[0x12, 0x34], false)).await;
        expect_fail(&mut second, LoginServerFail::REASON_ALREADY_LOGGED_IN).await;
        drop(second);
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(harness.state().game_servers.registered(1).unwrap().authed);
        harness.finish().await;
    }

    #[tokio::test]
    async fn the_table_reaches_the_links_of_connected_servers() {
        let harness = Harness::new().await;
        let mut gs = harness.authed_gs(&auth(1, &[0x12, 0x34], false)).await;
        let table = &harness.state().game_servers;
        let link = table.link(1).unwrap();

        assert!(!table.is_account_on_any_gs("player"));
        assert!(!table.kick_player("player"));
        link.add_account("player");
        assert!(table.is_account_on_any_gs("player"));
        assert_eq!(table.account_on_game_server("player"), Some(1));
        assert_eq!(table.registered(1).unwrap().current_player_count(), 1);

        assert!(table.kick_player("player"));
        assert_eq!(
            gs.recv_ls().await,
            Some(LsPacket::KickPlayer(KickPlayer {
                account: "player".to_owned()
            }))
        );
        table.request_characters("player");
        assert_eq!(
            gs.recv_ls().await,
            Some(LsPacket::RequestCharacters(RequestCharacters {
                account: "player".to_owned()
            }))
        );
        link.change_password_response("Char", "done");
        assert!(matches!(
            gs.recv_ls().await,
            Some(LsPacket::ChangePasswordResponse(_))
        ));

        link.remove_account("player");
        assert!(!table.is_account_on_any_gs("player"));
        harness.finish().await;
    }

    #[tokio::test]
    async fn flood_protection_refuses_a_second_fast_connection() {
        let harness = Harness::with_config(|config| config.login.flood_protection = true).await;
        let mut first = harness.gs().await;
        first.read_init().await;
        // Within FastConnectionTime (350 ms) of the first one: closed without an InitLS.
        let mut second = harness.gs().await;
        assert!(second.is_closed().await);
        harness.finish().await;
    }

    #[test]
    fn flood_protector_follows_the_java_rules() {
        let mut config = default_config();
        config.login.flood_protection = true;
        let flood = FloodProtector::new(&config.login);
        let ip: IpAddr = "10.1.1.1".parse().unwrap();

        assert!(flood.admit(ip, 1_000), "first connection");
        assert!(!flood.admit(ip, 1_100), "within FastConnectionTime (350 ms)");
        assert!(flood.admit(ip, 1_500), "well after the last one");
        // The refused attempt did not count: two connections are open.
        flood.release(ip);
        flood.release(ip);
        assert!(
            flood.visitors().is_empty(),
            "the entry goes with the last connection"
        );

        // FastConnectionLimit (15): more than 15 open connections, each less than
        // NormalConnectionTime (700 ms) after the one before.
        let mut now = 10_000;
        for _ in 0..15 {
            assert!(flood.admit(ip, now));
            now += 400;
        }
        assert!(!flood.admit(ip, now), "the 16th, 400 ms after the 15th");
        assert!(flood.admit(ip, now + 800), "slower connections are fine");
        for _ in 0..16 {
            flood.release(ip);
        }
        assert!(flood.visitors().is_empty());

        // MaxConnectionPerIP (50): spaced out enough, but too many at once.
        let mut now = 100_000;
        for _ in 0..50 {
            assert!(flood.admit(ip, now));
            now += 800;
        }
        assert!(!flood.admit(ip, now), "the 51st");
    }

    #[test]
    fn flood_protector_can_be_off() {
        let mut config = default_config();
        config.login.flood_protection = false;
        let flood = FloodProtector::new(&config.login);
        let ip: IpAddr = "10.1.1.1".parse().unwrap();
        assert!(flood.admit(ip, 1));
        assert!(flood.admit(ip, 2));
        flood.release(ip);
    }

    #[test]
    fn the_banned_address_check_is_the_java_stub() {
        assert!(!is_banned_game_server_ip("127.0.0.1".parse().unwrap()));
        assert!(!is_banned_game_server_ip("203.0.113.9".parse().unwrap()));
    }

    #[tokio::test]
    async fn a_port_in_use_is_a_startup_error() {
        let first = Harness::new().await;
        let port = i32::from(first.address.port());
        let (state, shutdown_requests) = state_with({
            let mut config = default_config();
            config.login.game_server_login_host = "127.0.0.1".to_owned();
            config.login.game_server_login_port = port;
            config
        });
        let mut second = Server {
            state: Arc::new(state),
            shutdown_requests,
            tasks: JoinSet::new(),
            cancel: l2_net::CancellationToken::new(),
        };
        let error = start_with_store(&mut second, Arc::new(RecordingStore::default()))
            .await
            .unwrap_err();
        assert!(matches!(error, StartupError::Listen { .. }), "{error}");
        first.finish().await;
    }
}
