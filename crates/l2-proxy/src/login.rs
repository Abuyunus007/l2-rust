//! The login MITM: one client connection paired with one upstream connection to the real
//! login server.
//!
//! - Toward the **client** the proxy is a login server. It has its own RSA-1024 pair and its own
//!   Blowfish key, and sends its own `Init` that carries the *real* session id (so the
//!   `AuthGameGuard` the client answers with is accepted by the real server unchanged).
//! - Toward the **real login server** the proxy is a client. It decrypts the real `Init` with
//!   the static key, learns the real Blowfish key and RSA modulus, and re-encrypts the
//!   client's `RequestAuthLogin` credentials with that modulus.
//! - Everything else is forwarded as plaintext payloads, re-encrypted per connection, and
//!   recorded. A `ServerList` gets its addresses rewritten (see `rewrite`).
//!
//! Java: loginserver/network/LoginEncryption.java (both ciphers), serverpackets/Init.java,
//! clientpackets/RequestAuthLogin.java. Not in Java: the Java server has no proxy.
//!
//! Both connections are `l2-net` connection tasks with their own cipher, so a packet moves
//! from one task to the other through `ConnectionHandle::send` (plaintext in, encrypted by the
//! receiving connection with its own key).

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use l2_crypto::login::decrypt_init;
use l2_crypto::rsa::encrypt_raw;
use l2_crypto::scramble::unscramble_modulus;
use l2_crypto::{LoginCipher, LoginClientCipher, RsaKeyPair};
use l2_net::{
    CancellationToken, CipherError, Control, Decode, Encode, FrameCipher, FrameHandler, PacketError,
    PacketReader, spawn_connection,
};
use l2_protocol_login::dispatch::{REQUEST_PI_AGREEMENT, REQUEST_PI_AGREEMENT_CHECK};
use l2_protocol_login::server::{
    AccountKicked, GGAuth, Init, LoginFail, LoginOk, PlayFail, PlayOk, ServerList,
};
use l2_protocol_login::{
    AccountKickedReason, AuthGameGuard, LoginFailReason, PlayFailReason, RequestAuthLogin, RequestServerList,
    RequestServerLogin, credentials_from_decrypted,
};
use rand::{Rng, RngExt};
use serde_json::{Value, json};
use tokio::net::TcpStream;
use tokio::sync::oneshot;

use crate::capture::{Channel, Dir, REDACTED, Record};
use crate::{ProxyError, Shared};

/// The public exponent of every login RSA key (Java `RSAKeyGenParameterSpec.F4`).
const RSA_EXPONENT: [u8; 3] = [0x01, 0x00, 0x01];
/// How long connecting to the real login server may take.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Length of the `Init` payload: opcode, session id, protocol, modulus, 4 GG ints, key, 0 byte.
const INIT_LEN: usize = 1 + 4 + 4 + 128 + 16 + 16 + 1;
/// Bytes of padding and checksum a decrypted session packet ends with, at least.
const CHECKSUM_SLOT: usize = 4;

/// An RSA pair the proxy presents to clients, with its scrambled modulus precomputed.
pub(crate) struct ProxyKeys {
    pub(crate) pair: RsaKeyPair,
    pub(crate) scrambled_modulus: [u8; 128],
}

// ---------------------------------------------------------------------------------------------
// Recording
// ---------------------------------------------------------------------------------------------

/// Sends records of one connection to the capture channel.
#[derive(Clone)]
pub(crate) struct Recorder {
    shared: Arc<Shared>,
    conn: u64,
}

impl Recorder {
    fn emit(&self, dir: Dir, opcode: u8, name: &str, fields: Option<Value>, raw: &[u8]) {
        let record = Record::new(self.conn, dir, Channel::Login, opcode, name, fields, raw);
        // The receiver may be gone (GUI closed the inspector); the proxy keeps working.
        let _ = self.shared.records.send(record);
    }
}

// ---------------------------------------------------------------------------------------------
// Packet lengths and descriptions
// ---------------------------------------------------------------------------------------------

/// The true payload length of a decrypted client packet. A decrypted body also holds zero
/// padding and the checksum slot, which must not be forwarded or recorded as payload.
pub(crate) fn c2s_len(body: &[u8]) -> usize {
    let len = match body.first() {
        Some(&AuthGameGuard::OPCODE) => 1 + 4 + 16,
        Some(&RequestServerList::OPCODE) => 1 + 8,
        Some(&RequestServerLogin::OPCODE) => 1 + 9,
        Some(&RequestAuthLogin::OPCODE) => {
            // Same rule as `RequestAuthLogin::decode`: 256 bytes after the opcode is the new method.
            if body.len() > 256 { 1 + 256 } else { 1 + 128 }
        }
        // Unknown packet: everything except the checksum slot.
        _ => body.len().saturating_sub(CHECKSUM_SLOT),
    };
    len.min(body.len())
}

/// The true payload length of a decrypted server packet (see [`c2s_len`]).
pub(crate) fn s2c_len(body: &[u8]) -> usize {
    let len = match body.first() {
        Some(&LoginFail::OPCODE | &PlayFail::OPCODE) => 2,
        Some(&AccountKicked::OPCODE) => 1 + 4,
        Some(&LoginOk::OPCODE) => 1 + 8 + 40,
        Some(&PlayOk::OPCODE) => 1 + 8,
        Some(&GGAuth::OPCODE) => 1 + 20,
        Some(&ServerList::OPCODE) => {
            let mut reader = PacketReader::new(&body[1..]);
            match ServerList::decode(&mut reader) {
                Ok(_) => 1 + reader.position(),
                Err(_) => body.len().saturating_sub(CHECKSUM_SLOT),
            }
        }
        _ => body.len().saturating_sub(CHECKSUM_SLOT),
    };
    len.min(body.len())
}

fn parse<P: Decode>(payload: &[u8]) -> Option<P> {
    P::decode(&mut PacketReader::new(payload.get(1..)?)).ok()
}

fn hex_u32(value: u32) -> String {
    format!("{value:08x}")
}

/// Name and decoded fields of a client packet. Never includes credentials.
pub(crate) fn describe_c2s(payload: &[u8]) -> (&'static str, Option<Value>) {
    match payload.first().copied() {
        Some(AuthGameGuard::OPCODE) => (
            "AuthGameGuard",
            parse::<AuthGameGuard>(payload).map(|p| json!({ "session_id": p.session_id })),
        ),
        Some(RequestAuthLogin::OPCODE) => ("RequestAuthLogin", Some(json!({ "credentials": REDACTED }))),
        Some(RequestServerList::OPCODE) => (
            "RequestServerList",
            parse::<RequestServerList>(payload)
                .map(|p| json!({ "login_ok1": p.login_ok1, "login_ok2": p.login_ok2 })),
        ),
        Some(RequestServerLogin::OPCODE) => (
            "RequestServerLogin",
            parse::<RequestServerLogin>(payload).map(
                |p| json!({ "login_ok1": p.login_ok1, "login_ok2": p.login_ok2, "server_id": p.server_id }),
            ),
        ),
        Some(REQUEST_PI_AGREEMENT_CHECK) => ("RequestPIAgreementCheck", None),
        Some(REQUEST_PI_AGREEMENT) => ("RequestPIAgreement", None),
        _ => ("unknown", None),
    }
}

fn reason_fields(code: u32, name: Option<&'static str>) -> Value {
    json!({ "reason": code, "reason_name": name })
}

/// Name and decoded fields of a server packet. `proxied` holds the replacement address per
/// `ServerList` entry when the list was rewritten.
pub(crate) fn describe_s2c(
    payload: &[u8],
    proxied: Option<&[Option<(Ipv4Addr, u16)>]>,
) -> (&'static str, Option<Value>) {
    match payload.first().copied() {
        Some(Init::OPCODE) => ("Init", parse::<Init>(payload).map(|p| init_fields(&p))),
        Some(LoginFail::OPCODE) => {
            let code = payload.get(1).copied();
            let fields = code
                .map(|c| reason_fields(u32::from(c), LoginFailReason::try_from(c).ok().map(|r| r.name())));
            ("LoginFail", fields)
        }
        Some(AccountKicked::OPCODE) => {
            let fields = parse::<AccountKicked>(payload)
                .map(|p| reason_fields(p.reason.code(), Some(p.reason.name())));
            let fields = fields.or_else(|| {
                let code = u32::from_le_bytes(payload.get(1..5)?.try_into().ok()?);
                Some(reason_fields(
                    code,
                    AccountKickedReason::try_from(code).ok().map(|r| r.name()),
                ))
            });
            ("AccountKicked", fields)
        }
        Some(LoginOk::OPCODE) => (
            "LoginOk",
            parse::<LoginOk>(payload).map(|p| json!({ "login_ok1": p.login_ok1, "login_ok2": p.login_ok2 })),
        ),
        Some(ServerList::OPCODE) => (
            "ServerList",
            parse::<ServerList>(payload).map(|list| server_list_fields(&list, proxied)),
        ),
        Some(PlayFail::OPCODE) => {
            let code = payload.get(1).copied();
            let fields =
                code.map(|c| reason_fields(u32::from(c), PlayFailReason::try_from(c).ok().map(|r| r.name())));
            ("PlayFail", fields)
        }
        Some(PlayOk::OPCODE) => (
            "PlayOk",
            parse::<PlayOk>(payload).map(|p| json!({ "play_ok1": p.play_ok1, "play_ok2": p.play_ok2 })),
        ),
        Some(GGAuth::OPCODE) => (
            "GGAuth",
            parse::<GGAuth>(payload).map(|p| json!({ "response": p.response })),
        ),
        _ => ("unknown", None),
    }
}

fn init_fields(init: &Init) -> Value {
    json!({
        "session_id": init.session_id,
        "protocol": init.protocol,
        "gg": init.gg.iter().map(|v| hex_u32(*v)).collect::<Vec<_>>(),
        "scrambled_modulus": hex::encode(init.scrambled_modulus),
        "blowfish_key": hex::encode(init.blowfish_key),
    })
}

fn server_list_fields(list: &ServerList, proxied: Option<&[Option<(Ipv4Addr, u16)>]>) -> Value {
    let servers: Vec<Value> = list
        .servers
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let mut entry = json!({
                "id": s.server_id,
                "ip": Ipv4Addr::from(s.ip).to_string(),
                "port": s.port,
                "age_limit": s.age_limit,
                "pvp": s.pvp,
                "players": s.current_players,
                "max_players": s.max_players,
                "up": s.up,
                "type": s.server_type,
                "brackets": s.brackets,
            });
            if let Some((ip, port)) = proxied.and_then(|p| p.get(i)).copied().flatten() {
                entry["proxy_ip"] = json!(ip.to_string());
                entry["proxy_port"] = json!(port);
            }
            entry
        })
        .collect();
    let characters = list.char_info.as_ref().map(|infos| {
        infos
            .iter()
            .map(|c| {
                json!({
                    "server_id": c.server_id,
                    "chars": c.chars,
                    "delete_in_secs": c.seconds_until_delete,
                })
            })
            .collect::<Vec<_>>()
    });
    json!({ "last_server": list.last_server, "servers": servers, "characters": characters })
}

// ---------------------------------------------------------------------------------------------
// Ciphers
// ---------------------------------------------------------------------------------------------

/// Toward the client: the proxy is the server (`LoginCipher`); the first packet out is `Init`.
struct ClientCipher(LoginCipher);

impl FrameCipher for ClientCipher {
    fn encrypt(&mut self, payload: Vec<u8>) -> Vec<u8> {
        // Java draws `Rnd.nextInt()` for the XOR pass of the first packet.
        self.0.encrypt(&payload, rand::rng().random())
    }

    fn decrypt(&mut self, body: &mut Vec<u8>) -> Result<(), CipherError> {
        self.0.decrypt(body).map_err(|e| CipherError(e.to_string()))
    }
}

/// Toward the real server: the proxy is the client. Until the first frame (`Init`) arrives it
/// does not know the Blowfish key, so it decrypts that frame itself and then switches.
struct UpstreamCipher {
    session: Option<LoginClientCipher>,
}

impl FrameCipher for UpstreamCipher {
    fn encrypt(&mut self, payload: Vec<u8>) -> Vec<u8> {
        match &self.session {
            Some(cipher) => cipher.encrypt(&payload),
            None => {
                // Nothing is sent upstream before Init is known; see `ClientSide::on_connect`.
                tracing::warn!("dropping a packet for the login server: its Init has not arrived yet");
                Vec::new()
            }
        }
    }

    fn decrypt(&mut self, body: &mut Vec<u8>) -> Result<(), CipherError> {
        if let Some(cipher) = &mut self.session {
            return cipher.decrypt(body).map_err(|e| CipherError(e.to_string()));
        }
        decrypt_init(body).map_err(|e| CipherError(e.to_string()))?;
        let init = parse_init(body).map_err(|e| CipherError(e.to_string()))?;
        self.session = Some(LoginClientCipher::after_init(&init.blowfish_key));
        Ok(())
    }
}

/// Parses a decrypted `Init` body (opcode first, padding after the payload is ignored).
fn parse_init(body: &[u8]) -> Result<Init, PacketError> {
    match body.first() {
        Some(&Init::OPCODE) => Init::decode(&mut PacketReader::new(&body[1..])),
        _ => Err(PacketError::invalid("Init", "first packet is not an Init")),
    }
}

// ---------------------------------------------------------------------------------------------
// The two sides of a session
// ---------------------------------------------------------------------------------------------

/// Handles frames from the real client (the proxy acts as login server).
struct ClientSide {
    rec: Recorder,
    upstream: l2_net::ConnectionHandle,
    keys: Arc<ProxyKeys>,
    real_modulus: [u8; 128],
    /// The proxy's own `Init`, sent as soon as the client is connected.
    init_payload: Vec<u8>,
}

impl ClientSide {
    /// Decrypts the client's RSA blocks with the proxy key and encrypts them again with the real
    /// server's modulus. Returns the rebuilt packet and, if requested, the account name.
    ///
    /// The decrypted blocks (account and password) only live in this function and are zeroed
    /// before it returns.
    fn reencrypt_credentials(&self, payload: &[u8]) -> Result<(Vec<u8>, Option<String>), ProxyError> {
        let packet = RequestAuthLogin::decode(&mut PacketReader::new(&payload[1..]))?;
        let blocks: Vec<[u8; 128]> = match packet {
            RequestAuthLogin::Rsa128 { block } => vec![block],
            RequestAuthLogin::Rsa256 { block1, block2 } => vec![block1, block2],
        };
        let mut plain = Vec::with_capacity(256);
        let mut forwarded = Vec::with_capacity(blocks.len());
        let mut failure = None;
        for block in &blocks {
            let result = self.keys.pair.decrypt_raw(block).and_then(|decrypted| {
                let encrypted = encrypt_raw(&self.real_modulus, &RSA_EXPONENT, &decrypted);
                plain.extend_from_slice(&decrypted);
                encrypted
            });
            match result {
                Ok(encrypted) => forwarded.push(encrypted),
                Err(e) => {
                    failure = Some(e);
                    break;
                }
            }
        }
        let account = (failure.is_none()).then(|| credentials_from_decrypted(&plain).0);
        plain.fill(0);
        if let Some(e) = failure {
            return Err(e.into());
        }

        let mut arrays = Vec::with_capacity(forwarded.len());
        for block in forwarded {
            let array: [u8; 128] = block
                .try_into()
                .map_err(|_| ProxyError::Protocol("re-encrypted RSA block is not 128 bytes".into()))?;
            arrays.push(array);
        }
        let rebuilt = match arrays.as_slice() {
            [block] => RequestAuthLogin::Rsa128 { block: *block },
            [block1, block2] => RequestAuthLogin::Rsa256 {
                block1: *block1,
                block2: *block2,
            },
            _ => return Err(ProxyError::Protocol("unexpected number of RSA blocks".into())),
        };
        Ok((rebuilt.to_bytes(), account))
    }
}

impl FrameHandler for ClientSide {
    async fn on_connect(&mut self, ctl: &mut Control<'_>) {
        if self.upstream.is_closed() {
            ctl.close();
            return;
        }
        ctl.send(self.init_payload.clone());
    }

    async fn on_frame(&mut self, body: Vec<u8>, ctl: &mut Control<'_>) {
        let Some(&opcode) = body.first() else { return };
        let payload = &body[..c2s_len(&body)];

        if opcode == RequestAuthLogin::OPCODE {
            match self.reencrypt_credentials(payload) {
                Ok((forward, account)) => {
                    let mut fields = json!({ "credentials": REDACTED });
                    if let (Some(account), true) = (account, self.rec.shared.config.log_accounts) {
                        fields["account"] = json!(account);
                    }
                    // Only the opcode: the RSA blocks are not recorded.
                    self.rec
                        .emit(Dir::C2s, opcode, "RequestAuthLogin", Some(fields), &[opcode]);
                    self.upstream.send(forward);
                }
                Err(e) => {
                    tracing::warn!("closing a client: could not re-encrypt its login packet: {e}");
                    ctl.close();
                }
            }
            return;
        }

        let (name, fields) = describe_c2s(payload);
        self.rec.emit(Dir::C2s, opcode, name, fields, payload);
        self.upstream.send(payload.to_vec());
    }

    async fn on_disconnect(&mut self) {
        self.upstream.close();
    }
}

/// Handles frames from the real login server (the proxy acts as client).
struct UpstreamSide {
    shared: Arc<Shared>,
    rec: Recorder,
    /// Taken by the first frame, which is the real `Init`.
    init_tx: Option<oneshot::Sender<Init>>,
    client: Arc<OnceLock<l2_net::ConnectionHandle>>,
}

impl UpstreamSide {
    /// Replacement addresses for every entry of a `ServerList`: the proxy's game relay for that
    /// server id. `None` leaves an entry alone (no usable address, or the relay failed to start).
    async fn proxy_addresses(&self, list: &ServerList) -> Vec<Option<(Ipv4Addr, u16)>> {
        let mut out = Vec::with_capacity(list.servers.len());
        for server in &list.servers {
            let Ok(port) = u16::try_from(server.port) else {
                out.push(None);
                continue;
            };
            let real = SocketAddr::new(IpAddr::V4(Ipv4Addr::from(server.ip)), port);
            match self.shared.relays.ensure(server.server_id, real).await {
                Ok(local_port) => out.push(Some((self.shared.advertise_ip, local_port))),
                Err(e) => {
                    tracing::warn!(
                        "no game relay for server {}: {e}; its address stays unchanged",
                        server.server_id
                    );
                    out.push(None);
                }
            }
        }
        out
    }
}

impl FrameHandler for UpstreamSide {
    async fn on_frame(&mut self, body: Vec<u8>, ctl: &mut Control<'_>) {
        if let Some(tx) = self.init_tx.take() {
            // The cipher already checked that this parses.
            match parse_init(&body) {
                Ok(init) => {
                    let payload = &body[..INIT_LEN.min(body.len())];
                    let (name, fields) = describe_s2c(payload, None);
                    self.rec.emit(Dir::S2c, Init::OPCODE, name, fields, payload);
                    let _ = tx.send(init);
                }
                Err(e) => {
                    tracing::warn!("the login server's first packet is not a valid Init: {e}");
                    ctl.close();
                }
            }
            return;
        }

        let Some(&opcode) = body.first() else { return };
        let Some(client) = self.client.get() else {
            tracing::warn!("login server sent {opcode:#04x} before the client side was ready; dropped");
            return;
        };
        let payload = &body[..s2c_len(&body)];

        if opcode == ServerList::OPCODE && self.shared.config.rewrite_server_list {
            let forward = match parse::<ServerList>(payload) {
                Some(list) => {
                    let replacements = self.proxy_addresses(&list).await;
                    let (name, fields) = describe_s2c(payload, Some(&replacements));
                    self.rec.emit(Dir::S2c, opcode, name, fields, payload);
                    match crate::rewrite::patch_server_list(payload, &replacements) {
                        Ok(patched) => patched,
                        Err(e) => {
                            tracing::warn!("could not rewrite the ServerList: {e}; forwarding it unchanged");
                            payload.to_vec()
                        }
                    }
                }
                None => {
                    let (name, fields) = describe_s2c(payload, None);
                    self.rec.emit(Dir::S2c, opcode, name, fields, payload);
                    payload.to_vec()
                }
            };
            client.send(forward);
            return;
        }

        let (name, fields) = describe_s2c(payload, None);
        self.rec.emit(Dir::S2c, opcode, name, fields, payload);
        client.send(payload.to_vec());
    }

    async fn on_disconnect(&mut self) {
        if let Some(client) = self.client.get() {
            client.close();
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Session setup
// ---------------------------------------------------------------------------------------------

fn random_blowfish_key() -> [u8; 16] {
    let mut key = [0u8; 16];
    rand::rng().fill_bytes(&mut key);
    key
}

fn pick_keys(shared: &Shared) -> Arc<ProxyKeys> {
    let index = rand::rng().random_range(0..shared.keys.len());
    Arc::clone(&shared.keys[index])
}

/// Serves one accepted client: connects to the real login server, waits for its `Init`, then
/// starts both connection tasks. Returns when the setup is done; the connections run on their
/// own tasks. On any failure the client is simply dropped (closed).
pub(crate) async fn serve_client(shared: Arc<Shared>, stream: TcpStream, peer: SocketAddr, conn: u64) {
    let rec = Recorder {
        shared: Arc::clone(&shared),
        conn,
    };
    let cancel: CancellationToken = shared.cancel.child_token();
    tracing::info!("connection {conn}: client {peer} connected");

    let upstream_stream =
        match tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(shared.config.upstream)).await {
            Ok(Ok(stream)) => stream,
            Ok(Err(e)) => {
                tracing::warn!(
                    "connection {conn}: cannot reach the login server {}: {e}",
                    shared.config.upstream
                );
                return;
            }
            Err(_) => {
                tracing::warn!(
                    "connection {conn}: connecting to the login server {} timed out",
                    shared.config.upstream
                );
                return;
            }
        };

    let (init_tx, init_rx) = oneshot::channel();
    let client_slot = Arc::new(OnceLock::new());
    let upstream_handler = UpstreamSide {
        shared: Arc::clone(&shared),
        rec: rec.clone(),
        init_tx: Some(init_tx),
        client: Arc::clone(&client_slot),
    };
    let upstream = match spawn_connection(
        upstream_stream,
        Arc::clone(&shared.conn_config),
        Box::new(UpstreamCipher { session: None }),
        upstream_handler,
        cancel.clone(),
    ) {
        Ok(handle) => handle,
        Err(e) => {
            tracing::warn!("connection {conn}: cannot start the upstream connection: {e}");
            return;
        }
    };

    let real_init = match tokio::time::timeout(shared.config.init_timeout, init_rx).await {
        Ok(Ok(init)) => init,
        _ => {
            tracing::warn!("connection {conn}: the login server did not send a valid Init");
            upstream.close();
            return;
        }
    };

    // The proxy's own Init: real session id, protocol and GG values; own RSA modulus and BF key.
    let keys = pick_keys(&shared);
    let blowfish_key = random_blowfish_key();
    let proxy_init = Init {
        session_id: real_init.session_id,
        protocol: real_init.protocol,
        scrambled_modulus: keys.scrambled_modulus,
        gg: real_init.gg,
        blowfish_key,
    };
    let handler = ClientSide {
        rec,
        upstream: upstream.clone(),
        keys,
        real_modulus: unscramble_modulus(&real_init.scrambled_modulus),
        init_payload: proxy_init.to_bytes(),
    };
    match spawn_connection(
        stream,
        Arc::clone(&shared.conn_config),
        Box::new(ClientCipher(LoginCipher::new(&blowfish_key))),
        handler,
        cancel,
    ) {
        Ok(handle) => {
            let _ = client_slot.set(handle);
        }
        Err(e) => {
            tracing::warn!("connection {conn}: cannot start the client connection: {e}");
            upstream.close();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use l2_protocol_login::server::{ServerCharInfo, ServerEntry};

    fn server_list() -> ServerList {
        ServerList {
            last_server: 1,
            servers: vec![ServerEntry {
                server_id: 1,
                ip: [192, 168, 1, 5],
                port: 7777,
                age_limit: 0,
                pvp: false,
                current_players: 5,
                max_players: 100,
                up: true,
                server_type: 1,
                brackets: false,
            }],
            char_info: Some(vec![ServerCharInfo {
                server_id: 1,
                chars: 2,
                seconds_until_delete: vec![30],
            }]),
        }
    }

    /// A decrypted body as the cipher hands it over: payload, zero padding, a checksum slot.
    fn as_body(mut payload: Vec<u8>) -> Vec<u8> {
        payload.resize((payload.len() + 4).next_multiple_of(8), 0);
        payload
    }

    #[test]
    fn c2s_lengths_strip_padding_and_checksum() {
        let cases: Vec<Vec<u8>> = vec![
            AuthGameGuard { session_id: 5 }.to_bytes(),
            RequestServerList {
                login_ok1: 1,
                login_ok2: 2,
            }
            .to_bytes(),
            RequestServerLogin {
                login_ok1: 1,
                login_ok2: 2,
                server_id: 3,
            }
            .to_bytes(),
            RequestAuthLogin::Rsa128 { block: [1; 128] }.to_bytes(),
            RequestAuthLogin::Rsa256 {
                block1: [1; 128],
                block2: [2; 128],
            }
            .to_bytes(),
        ];
        for payload in cases {
            let body = as_body(payload.clone());
            assert_eq!(&body[..c2s_len(&body)], payload.as_slice());
        }
        // Unknown packets lose only the checksum slot.
        assert_eq!(c2s_len(&[0x77, 1, 2, 3, 4, 5, 6, 7]), 4);
        assert_eq!(c2s_len(&[]), 0);
    }

    #[test]
    fn s2c_lengths_strip_padding_and_checksum() {
        let cases: Vec<Vec<u8>> = vec![
            LoginFail {
                reason: LoginFailReason::UserOrPassWrong,
            }
            .to_bytes(),
            AccountKicked {
                reason: AccountKickedReason::PermanentlyBanned,
            }
            .to_bytes(),
            LoginOk {
                login_ok1: 1,
                login_ok2: 2,
            }
            .to_bytes(),
            PlayFail {
                reason: PlayFailReason::ServerOverloaded,
            }
            .to_bytes(),
            PlayOk {
                play_ok1: 1,
                play_ok2: 2,
            }
            .to_bytes(),
            GGAuth { response: 9 }.to_bytes(),
            server_list().to_bytes(),
        ];
        for payload in cases {
            let body = as_body(payload.clone());
            assert_eq!(
                &body[..s2c_len(&body)],
                payload.as_slice(),
                "opcode {:#04x}",
                payload[0]
            );
        }
    }

    #[test]
    fn auth_login_description_never_has_credentials() {
        let payload = RequestAuthLogin::Rsa256 {
            block1: [0xab; 128],
            block2: [0xcd; 128],
        }
        .to_bytes();
        let (name, fields) = describe_c2s(&payload);
        assert_eq!(name, "RequestAuthLogin");
        assert_eq!(fields, Some(json!({ "credentials": "<redacted>" })));
    }

    #[test]
    fn c2s_descriptions() {
        let (name, fields) = describe_c2s(&AuthGameGuard { session_id: -7 }.to_bytes());
        assert_eq!(
            (name, fields),
            ("AuthGameGuard", Some(json!({ "session_id": -7 })))
        );
        let (name, fields) = describe_c2s(
            &RequestServerLogin {
                login_ok1: 1,
                login_ok2: 2,
                server_id: 9,
            }
            .to_bytes(),
        );
        assert_eq!(name, "RequestServerLogin");
        assert_eq!(fields.unwrap()["server_id"], 9);
        assert_eq!(describe_c2s(&[0x0e, 0, 0]).0, "RequestPIAgreementCheck");
        assert_eq!(describe_c2s(&[0x55]), ("unknown", None));
        assert_eq!(describe_c2s(&[]), ("unknown", None));
    }

    #[test]
    fn s2c_descriptions() {
        let (name, fields) = describe_s2c(
            &LoginFail {
                reason: LoginFailReason::AccountInUse,
            }
            .to_bytes(),
            None,
        );
        assert_eq!(name, "LoginFail");
        assert_eq!(fields.unwrap()["reason_name"], "REASON_ACCOUNT_IN_USE");
        // An unknown reason code still names the packet and keeps the code.
        let (name, fields) = describe_s2c(&[0x01, 0x77], None);
        assert_eq!(name, "LoginFail");
        assert_eq!(fields.unwrap(), json!({ "reason": 0x77, "reason_name": null }));
        let (_, fields) = describe_s2c(
            &PlayOk {
                play_ok1: 3,
                play_ok2: 4,
            }
            .to_bytes(),
            None,
        );
        assert_eq!(fields.unwrap(), json!({ "play_ok1": 3, "play_ok2": 4 }));
        assert_eq!(describe_s2c(&[0x55], None), ("unknown", None));
    }

    #[test]
    fn server_list_description_shows_original_and_proxy_addresses() {
        let payload = server_list().to_bytes();
        let replacements = [Some((Ipv4Addr::LOCALHOST, 40001))];
        let (name, fields) = describe_s2c(&payload, Some(&replacements));
        assert_eq!(name, "ServerList");
        let fields = fields.unwrap();
        let server = &fields["servers"][0];
        assert_eq!(server["ip"], "192.168.1.5");
        assert_eq!(server["port"], 7777);
        assert_eq!(server["proxy_ip"], "127.0.0.1");
        assert_eq!(server["proxy_port"], 40001);
        assert_eq!(fields["characters"][0]["chars"], 2);

        let (_, plain) = describe_s2c(&payload, None);
        assert!(plain.unwrap()["servers"][0].get("proxy_ip").is_none());
    }

    #[test]
    fn init_description() {
        let init = Init::new(0x1234, [7; 128], [9; 16]);
        let (name, fields) = describe_s2c(&init.to_bytes(), None);
        assert_eq!(name, "Init");
        let fields = fields.unwrap();
        assert_eq!(fields["session_id"], 0x1234);
        assert_eq!(fields["protocol"], 0xc621);
        assert_eq!(fields["gg"][0], "29dd954e");
    }
}
