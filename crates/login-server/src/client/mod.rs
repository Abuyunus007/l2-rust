//! Client connections on `LoginserverPort`: Init, GameGuard, login, server list.
//!
//! Java: `loginserver/network/LoginClient.java`, `LoginPacketHandler.java` and the client
//! packets in `network/clientpackets/`.

pub mod auth;
pub mod registry;
pub mod server_list;
pub mod session_key;

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use l2_crypto::LoginCipher;
use l2_net::{CipherError, ConnectionConfig, Control, Encode, FrameCipher, FrameHandler, PacketReader};
use l2_protocol_login::client::{RequestAuthLogin, RequestServerLogin, credentials_from_decrypted};
use l2_protocol_login::dispatch::{ClientPacket, ConnectionState, decode_client};
use l2_protocol_login::enums::{AccountKickedReason, LoginFailReason, PlayFailReason};
use l2_protocol_login::server::{AccountKicked, GGAuth, Init, LoginFail, LoginOk, PlayFail, PlayOk};
use l2_util::WorldRng;
use l2_util::time::now_millis;
use tokio::net::TcpListener;
use tracing::{debug, info};

use self::auth::{AccountInfo, LoginResult, can_check_in, retrieve_account_info};
use self::registry::AuthedClient;
use self::server_list::{
    CHARACTER_WAIT, CharacterReplies, ClientView, GsView, is_login_possible, make_server_list,
};
use self::session_key::SessionKey;
use crate::state::{LoginState, ServerStatus};
use crate::{Server, StartupError};

/// Binds `LoginserverHostname:LoginserverPort` and accepts clients until shutdown.
// Java: LoginServer.startLoginClientListener
pub async fn start_listener(server: &mut Server) -> Result<(), StartupError> {
    let state = Arc::clone(server.state());
    let login = &state.config.login;
    let host = if login.login_bind_address == "*" {
        "0.0.0.0"
    } else {
        login.login_bind_address.as_str()
    };
    let address = format!("{host}:{}", login.port_login);
    let listener = TcpListener::bind(&address)
        .await
        .map_err(|e| StartupError::Bind(address.clone(), e))?;
    info!("Login client listener started on {address}.");
    server.client_addr = listener.local_addr().ok();

    let config = Arc::new(ConnectionConfig {
        drop_packets: state.config.network.drop_packets,
        drop_packet_threshold: usize::try_from(state.config.network.drop_packet_threshold).unwrap_or(250),
        tcp_nodelay: true,
    });
    let next_id = AtomicU64::new(1);
    let factory = move |peer: SocketAddr| {
        let session = ClientSession::new(Arc::clone(&state), peer, next_id.fetch_add(1, Ordering::Relaxed))?;
        let cipher = LoginFrameCipher {
            inner: LoginCipher::new(&session.blowfish_key),
            rng: WorldRng::from_entropy(),
        };
        Some((session, Box::new(cipher) as Box<dyn FrameCipher>))
    };
    server.spawn(l2_net::listen(listener, config, server.cancel_token(), factory));
    Ok(())
}

/// `LoginCipher` as an `l2-net` frame cipher. The XOR key of the first packet is random, as in
/// Java (`Rnd.nextInt()` in `LoginEncryption.encrypt`).
struct LoginFrameCipher {
    inner: LoginCipher,
    rng: WorldRng,
}

impl FrameCipher for LoginFrameCipher {
    fn encrypt(&mut self, payload: Vec<u8>) -> Vec<u8> {
        let xor_key = self.rng.next_i32();
        self.inner.encrypt(&payload, xor_key)
    }

    fn decrypt(&mut self, body: &mut Vec<u8>) -> Result<(), CipherError> {
        self.inner.decrypt(body).map_err(|e| CipherError(e.to_string()))
    }
}

/// One client connection. Java: `LoginClient`.
pub struct ClientSession {
    state: Arc<LoginState>,
    ip: String,
    ip_addr: IpAddr,
    connection_id: u64,
    connected_at_ms: i64,
    session_id: i32,
    /// Index into `state.client_keys.pairs`.
    key_index: usize,
    blowfish_key: [u8; 16],
    rng: WorldRng,
    connection_state: ConnectionState,
    account: Option<String>,
    session_key: Option<SessionKey>,
    access_level: i32,
    last_server: i32,
    /// Shared with the registry entry, which the purge reads.
    joined_gs: Arc<AtomicBool>,
    /// Filled by the game server links; shared with the registry entry.
    characters: Arc<CharacterReplies>,
}

impl ClientSession {
    /// `None` when the key pools are empty (cannot happen after a successful startup).
    fn new(state: Arc<LoginState>, peer: SocketAddr, connection_id: u64) -> Option<Self> {
        let pair = state.random_client_key_pair()?;
        let key_index = state
            .client_keys
            .pairs
            .iter()
            .position(|p| std::ptr::eq(p, pair))?;
        let blowfish_key = *state.random_blowfish_key()?;
        let mut rng = WorldRng::from_entropy();
        Some(Self {
            ip: peer.ip().to_string(),
            ip_addr: peer.ip(),
            connection_id,
            connected_at_ms: now_millis(),
            session_id: rng.next_i32(),
            key_index,
            blowfish_key,
            rng,
            connection_state: ConnectionState::Connected,
            account: None,
            session_key: None,
            access_level: 0,
            last_server: 0,
            joined_gs: Arc::default(),
            characters: Arc::default(),
            state,
        })
    }

    /// Java `close(LoginFailReason)`: sends `LoginFail` but does **not** close the connection.
    /// NOTE(java-quirk): the real client disconnects by itself; the purge catches the rest.
    fn login_fail(ctl: &mut Control<'_>, reason: LoginFailReason) {
        ctl.send(LoginFail { reason }.to_bytes());
    }

    // Java: AuthGameGuard.run
    fn on_auth_game_guard(&mut self, session_id: i32, ctl: &mut Control<'_>) {
        if session_id != self.session_id {
            debug!("AuthGameGuard: session mismatch from {}", self.ip);
            Self::login_fail(ctl, LoginFailReason::AccessFailed);
            return;
        }
        self.connection_state = ConnectionState::AuthedGg;
        ctl.send(
            GGAuth {
                response: self.session_id,
            }
            .to_bytes(),
        );
    }

    /// RSA-decrypts the credential block(s) off the async threads.
    async fn decrypt_credentials(&self, request: RequestAuthLogin) -> Option<Vec<u8>> {
        let state = Arc::clone(&self.state);
        let index = self.key_index;
        let decrypted = tokio::task::spawn_blocking(move || {
            let pair = &state.client_keys.pairs.get(index)?.key_pair;
            match request {
                RequestAuthLogin::Rsa128 { block } => pair.decrypt_raw(&block).ok(),
                RequestAuthLogin::Rsa256 { block1, block2 } => {
                    let mut out = pair.decrypt_raw(&block1).ok()?;
                    out.extend(pair.decrypt_raw(&block2).ok()?);
                    Some(out)
                }
            }
        })
        .await;
        decrypted.ok().flatten()
    }

    // Java: RequestAuthLogin.run
    async fn on_request_auth_login(&mut self, request: RequestAuthLogin, ctl: &mut Control<'_>) {
        let Some(decrypted) = self.decrypt_credentials(request).await else {
            // Java logs the GeneralSecurityException and drops the packet.
            info!("RequestAuthLogin: RSA decryption failed for {}", self.ip);
            return;
        };
        let (user, password) = credentials_from_decrypted(&decrypted);

        let Some(info) = retrieve_account_info(&self.state, &self.ip, &user, &password).await else {
            Self::login_fail(ctl, LoginFailReason::UserOrPassWrong);
            return;
        };

        match self.try_check_in(&info, ctl).await {
            LoginResult::AuthSuccess => {
                let key = SessionKey::random(&mut self.rng);
                self.account = Some(info.login.clone());
                self.session_key = Some(key);
                self.connection_state = ConnectionState::AuthedLogin;
                // Java assignSessionKeyToClient: put (replaces the entry tryCheckin added).
                self.state
                    .clients
                    .insert(&info.login, self.registry_entry(key, ctl));
                // Java getCharactersOnAccount: ask every authed GS how many characters the
                // account has; the server list waits for the replies.
                let asked = self.state.game_servers.request_characters(&info.login);
                self.characters.expect(asked);
                if self.state.config.login.show_licence {
                    ctl.send(
                        LoginOk {
                            login_ok1: key.login_ok1,
                            login_ok2: key.login_ok2,
                        }
                        .to_bytes(),
                    );
                } else {
                    self.send_server_list(ctl).await;
                }
                info!("Account '{}' logged in from {}.", info.login, self.ip);
            }
            LoginResult::InvalidPassword => Self::login_fail(ctl, LoginFailReason::UserOrPassWrong),
            LoginResult::AccountBanned => {
                ctl.send_and_close(
                    AccountKicked {
                        reason: AccountKickedReason::PermanentlyBanned,
                    }
                    .to_bytes(),
                );
            }
            LoginResult::AlreadyOnLs => {
                if let Some(old) = self.state.clients.remove(&info.login) {
                    // Kick the other client (LoginFail without close, like Java).
                    old.handle.send(
                        LoginFail {
                            reason: LoginFailReason::AccountInUse,
                        }
                        .to_bytes(),
                    );
                }
                Self::login_fail(ctl, LoginFailReason::AccountInUse);
            }
            LoginResult::AlreadyOnGs => {
                Self::login_fail(ctl, LoginFailReason::AccountInUse);
                // TODO(M1-LS-04): kick the account on the game server that has it.
            }
        }
    }

    // Java: LoginController.tryCheckinAccount
    async fn try_check_in(&mut self, info: &AccountInfo, ctl: &mut Control<'_>) -> LoginResult {
        if info.access_level < 0 {
            return LoginResult::AccountBanned;
        }
        if !can_check_in(&self.state, &self.ip, info).await {
            return LoginResult::InvalidPassword;
        }
        // canCheckin sets these on the client.
        self.access_level = info.access_level;
        self.last_server = info.last_server;
        // TODO(M1-LS-04): `isAccountInAnyGameServer` → LoginResult::AlreadyOnGs.
        let placeholder = SessionKey {
            login_ok1: 0,
            login_ok2: 0,
            play_ok1: 0,
            play_ok2: 0,
        };
        if self
            .state
            .clients
            .try_insert(&info.login, self.registry_entry(placeholder, ctl))
        {
            LoginResult::AuthSuccess
        } else {
            LoginResult::AlreadyOnLs
        }
    }

    fn registry_entry(&self, session_key: SessionKey, ctl: &Control<'_>) -> AuthedClient {
        AuthedClient {
            session_key,
            handle: ctl.handle().clone(),
            connection_id: self.connection_id,
            connected_at_ms: self.connected_at_ms,
            access_level: self.access_level,
            last_server: self.last_server,
            joined_gs: Arc::clone(&self.joined_gs),
            characters: Arc::clone(&self.characters),
        }
    }

    /// Builds the `ServerList` (waiting up to [`CHARACTER_WAIT`] for the character counts) and
    /// sends it.
    // Java: new ServerList(client)
    async fn send_server_list(&self, ctl: &mut Control<'_>) {
        let client = ClientView {
            ip: self.ip_addr,
            access_level: self.access_level,
            last_server: self.last_server,
        };
        let list = make_server_list(
            &self.state.game_servers,
            &self.characters,
            &client,
            CHARACTER_WAIT,
        )
        .await;
        ctl.send(list.to_bytes());
    }

    // Java: RequestServerList.run
    async fn on_request_server_list(&mut self, login_ok1: i32, login_ok2: i32, ctl: &mut Control<'_>) {
        let valid = self
            .session_key
            .is_some_and(|k| k.check_login_pair(login_ok1, login_ok2));
        if valid {
            self.send_server_list(ctl).await;
        } else {
            Self::login_fail(ctl, LoginFailReason::AccessFailed);
        }
    }

    // Java: RequestServerLogin.run
    async fn on_request_server_login(&mut self, request: RequestServerLogin, ctl: &mut Control<'_>) {
        let Some(key) = self.session_key else {
            return;
        };
        // If the licence was not shown, the client never got the pair, so it cannot be checked.
        if self.state.config.login.show_licence && !key.check_login_pair(request.login_ok1, request.login_ok2)
        {
            Self::login_fail(ctl, LoginFailReason::AccessFailed);
            return;
        }
        let status = self.state.status();
        if status == ServerStatus::Down || (status == ServerStatus::GmOnly && self.access_level < 1) {
            Self::login_fail(ctl, LoginFailReason::AccessFailed);
            return;
        }
        // NOTE(java-quirk): Java reads the id with the signed `readByte()`, so ids above 127 are
        // negative and match no game server.
        let server_id = i32::from(request.server_id as i8);
        let possible = is_login_possible(
            &self.state,
            &self.state.game_servers,
            self.account.as_deref().unwrap_or_default(),
            self.access_level,
            self.last_server,
            server_id,
        )
        .await;
        if possible {
            self.joined_gs.store(true, Ordering::Relaxed);
            ctl.send(
                PlayOk {
                    play_ok1: key.play_ok1,
                    play_ok2: key.play_ok2,
                }
                .to_bytes(),
            );
        } else {
            // Like `close(LoginFailReason)`, Java's `close(PlayFailReason)` leaves the
            // connection open.
            ctl.send(
                PlayFail {
                    reason: PlayFailReason::ServerOverloaded,
                }
                .to_bytes(),
            );
        }
    }
}

impl FrameHandler for ClientSession {
    // Java: LoginClient constructor (ban check) + onConnected (Init)
    async fn on_connect(&mut self, ctl: &mut Control<'_>) {
        if self.state.bans.is_banned(&self.ip) {
            // Deliberate deviation: Java only sends LoginFail(NOT_AUTHED) and keeps the
            // connection (then even sends Init). A banned address gets nothing more here.
            info!("Refused banned address {}.", self.ip);
            ctl.send_and_close(
                LoginFail {
                    reason: LoginFailReason::NotAuthed,
                }
                .to_bytes(),
            );
            return;
        }
        let Some(pair) = self.state.client_keys.pairs.get(self.key_index) else {
            ctl.close();
            return;
        };
        let init = Init::new(self.session_id, pair.scrambled_modulus, self.blowfish_key);
        ctl.send(init.to_bytes());
    }

    // Java: LoginPacketHandler.handle + LoginClientPacket.read/run
    async fn on_frame(&mut self, body: Vec<u8>, ctl: &mut Control<'_>) {
        let mut reader = PacketReader::new(&body);
        let Ok(opcode) = reader.u8() else {
            Self::login_fail(ctl, LoginFailReason::AccessFailed);
            return;
        };
        let packet = match decode_client(self.connection_state, opcode, &mut reader) {
            Ok(Some(packet)) => packet,
            Ok(None) => return, // unknown opcode or wrong state: ignored, like Java
            Err(e) => {
                // Java: read() fails → the packet is not run.
                debug!("Bad login packet 0x{opcode:02X} from {}: {e}", self.ip);
                return;
            }
        };
        match packet {
            ClientPacket::AuthGameGuard(p) => self.on_auth_game_guard(p.session_id, ctl),
            ClientPacket::RequestAuthLogin(p) => self.on_request_auth_login(*p, ctl).await,
            ClientPacket::RequestServerList(p) => {
                self.on_request_server_list(p.login_ok1, p.login_ok2, ctl).await;
            }
            ClientPacket::RequestServerLogin(p) => self.on_request_server_login(p, ctl).await,
        }
    }

    // Java: LoginClient.onDisconnection
    async fn on_disconnect(&mut self) {
        if !self.joined_gs.load(Ordering::Relaxed)
            && let Some(account) = &self.account
        {
            // Java removes by account name; we only remove our own entry, so a newer login of
            // the same account isn't dropped. (Java also sleeps 1 s here for no visible reason.)
            self.state
                .clients
                .remove_if_connection(account, self.connection_id);
        }
    }
}
