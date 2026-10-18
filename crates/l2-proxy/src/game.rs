//! The game relay: one local listener per game server id from the `ServerList`, and a
//! pass-through for each client connection that splits the traffic into frames, decrypts a
//! copy and records every packet with its opcode name.
//!
//! Unlike the login MITM, the proxy does **not** terminate the game connection. The game
//! cipher is a plain XOR stream whose key the real server picks and announces in the first
//! packet, so the proxy forwards the bytes exactly as they arrive (it never re-encrypts) and
//! only decodes a copy for the capture. The real client and the real server therefore stay in
//! sync on their own.
//!
//! Java: gameserver/network/GamePacketHandler.java (opcode dispatch: `0xD0` + `u16` ex id),
//! serverpackets/KeyPacket.java (the key, the encryption flag), GameClient.java
//! (`enableCrypt`, `setConnectionState`), Encryption.java (via `l2_crypto::GameCipher`).
//! Not in Java: the Java server has no proxy.
//!
//! How the key is found: the first server frame is always the `KeyPacket` (`VERSION_CHECK`,
//! 0x2E) and always plaintext, because Java's first `encrypt` call only switches the cipher on.
//! If its encryption flag is non-zero, every later frame in both directions is encrypted with
//! the 8 random key bytes from that packet. The proxy runs two [`GameCipher`] instances, one
//! per direction, and only ever calls `decrypt` on them. `GameCipher::decrypt` does nothing
//! until the cipher is enabled, and the only way to enable it is an `encrypt` call, so each
//! instance gets one zero-length `encrypt` first (see [`decrypt_cipher`]).
//!
//! The key is found in the server-to-client task and needed by the client-to-server task, so
//! [`Session`] carries it between them. The server-to-client task stores it **before** it
//! forwards the `KeyPacket`, so no encrypted client packet can arrive before the key is known.

use std::collections::HashMap;
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::Duration;

use l2_crypto::{GameCipher, game_key_from_random};
use l2_net::CancellationToken;
use l2_protocol_game::{ClientOpcode, ConnectionState, ExClientOpcode, ServerOpcode};
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::UnboundedSender;

use crate::capture::{Channel, Dir, REDACTED, Record};

/// How long connecting to the real game server may take.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Size of one read from a socket.
const READ_CHUNK: usize = 16 * 1024;
/// Client opcode that introduces an ex packet (`ClientOpcode::ExPacket`).
const EX_CLIENT: u8 = 0xD0;
/// Server opcode byte of every ex packet.
const EX_SERVER: u8 = 0xFE;
/// `KeyPacket` payload: opcode, result, 8 key bytes, encryption flag (`i32`). The rest (server
/// id, a 1 byte, an obfuscation key) is not needed.
const KEY_PACKET_MIN_LEN: usize = 1 + 1 + 8 + 4;

// ---------------------------------------------------------------------------------------------
// Frame decoding (pure, no sockets)
// ---------------------------------------------------------------------------------------------

/// What the two directions of one game connection share.
#[derive(Default)]
struct Session {
    /// The 16-byte game key, set once when a `KeyPacket` announces encryption.
    key: OnceLock<[u8; 16]>,
    /// Index into [`ConnectionState::ALL`].
    state: AtomicU8,
}

impl Session {
    fn state(&self) -> ConnectionState {
        ConnectionState::ALL
            .get(usize::from(self.state.load(Ordering::Relaxed)))
            .copied()
            .unwrap_or(ConnectionState::Connected)
    }

    fn set_state(&self, state: ConnectionState) {
        let index = ConnectionState::ALL.iter().position(|s| *s == state).unwrap_or(0);
        // The table has six entries, so the index always fits.
        self.state
            .store(u8::try_from(index).unwrap_or(0), Ordering::Relaxed);
    }
}

/// One decoded game packet, ready to become a [`Record`].
#[derive(Debug, Clone, PartialEq, Eq)]
struct Packet {
    dir: Dir,
    opcode: u8,
    /// The `u16` after `0xD0` (client) or `0xFE` (server).
    ex_opcode: Option<u16>,
    /// The Java constant name from the generated tables, or `"unknown"`.
    name: &'static str,
    /// Our guess of the connection state when the packet was seen (before the packet's own
    /// effect on it). The capture format has no key for it yet.
    state: ConnectionState,
    /// The plaintext payload, opcode included. For a redacted packet only the opcode byte.
    raw: Vec<u8>,
    /// The packet carries credentials (`AuthLogin`: account name and session keys), so `raw` was
    /// cut down to the opcode and the capture says `"<redacted>"` instead of fields.
    redacted: bool,
}

/// A cipher that can decrypt from the first byte on. `GameCipher` only switches on at its first
/// `encrypt`, which does not touch the buffer or move the key, so a zero-length call is enough.
fn decrypt_cipher(key: [u8; 16]) -> GameCipher {
    let mut cipher = GameCipher::new(key);
    cipher.encrypt(&mut []);
    cipher
}

/// Splits one direction of a game connection into frames (`u16 LE` total length including the
/// 2 header bytes) and names every packet. It only reads a copy of the stream.
struct Decoder {
    dir: Dir,
    pending: Vec<u8>,
    cipher: Option<GameCipher>,
    frames_seen: u64,
}

impl Decoder {
    fn new(dir: Dir) -> Self {
        Self {
            dir,
            pending: Vec::new(),
            cipher: None,
            frames_seen: 0,
        }
    }

    /// Adds received bytes and returns every packet that is complete now.
    fn feed(&mut self, bytes: &[u8], session: &Session) -> Vec<Packet> {
        self.pending.extend_from_slice(bytes);
        let mut packets = Vec::new();
        let mut pos = 0;
        while let Some(header) = self.pending.get(pos..pos + 2) {
            let total = usize::from(u16::from_le_bytes([header[0], header[1]]));
            if total <= 2 {
                // NOTE(java-quirk): Java's ReadHandler treats a length of 2 or less as "no body"
                // and reads the next header, so empty frames vanish. Same here.
                pos += 2;
                continue;
            }
            let Some(frame) = self.pending.get(pos..pos + total) else {
                break;
            };
            let body = frame[2..].to_vec();
            pos += total;
            packets.push(self.packet(body, session));
        }
        self.pending.drain(..pos);
        packets
    }

    fn packet(&mut self, mut body: Vec<u8>, session: &Session) -> Packet {
        let first = self.frames_seen == 0;
        self.frames_seen = self.frames_seen.saturating_add(1);
        let is_key_packet = first && self.dir == Dir::S2c;

        if !is_key_packet {
            if self.cipher.is_none() && self.dir == Dir::C2s {
                // The client starts encrypting after it has seen the KeyPacket.
                self.cipher = session.key.get().map(|key| decrypt_cipher(*key));
            }
            if let Some(cipher) = self.cipher.as_mut() {
                cipher.decrypt(&mut body);
            }
        }

        let state = session.state();
        let (opcode, ex_opcode, name) = match self.dir {
            Dir::C2s => name_client(&body),
            Dir::S2c => name_server(&body),
        };

        if is_key_packet {
            self.read_key_packet(&body, session);
        }
        match (self.dir, name) {
            (Dir::S2c, "CHARACTER_SELECTION_INFO") => session.set_state(ConnectionState::Authenticated),
            (Dir::S2c, "CHARACTER_SELECTED") => session.set_state(ConnectionState::Entering),
            (Dir::C2s, "ENTER_WORLD") => session.set_state(ConnectionState::InGame),
            _ => {}
        }

        // Captures are public fixtures: the game `AuthLogin` holds the account name and the
        // login/play session keys, which never go into a capture (same rule as the login side).
        let redacted = self.dir == Dir::C2s && ex_opcode.is_none() && opcode == ClientOpcode::AuthLogin.id();
        if redacted {
            body.truncate(1);
        }

        Packet {
            dir: self.dir,
            opcode,
            ex_opcode,
            name,
            state,
            raw: body,
            redacted,
        }
    }

    /// Reads the key and the encryption flag from the first server packet.
    fn read_key_packet(&mut self, payload: &[u8], session: &Session) {
        if payload.first() != Some(&ServerOpcode::VersionCheck.id()) || payload.len() < KEY_PACKET_MIN_LEN {
            tracing::debug!("the first server packet is not a KeyPacket; assuming no encryption");
            return;
        }
        let flag = i32::from_le_bytes([payload[10], payload[11], payload[12], payload[13]]);
        if flag == 0 {
            return;
        }
        let mut random = [0u8; 8];
        random.copy_from_slice(&payload[2..10]);
        let key = game_key_from_random(random);
        // A second KeyPacket on one connection keeps the first key: Java only sends one.
        let _ = session.key.set(key);
        self.cipher = Some(decrypt_cipher(key));
    }
}

/// `(opcode, ex opcode, name)` of a decrypted client payload (`GamePacketHandler`).
fn name_client(payload: &[u8]) -> (u8, Option<u16>, &'static str) {
    let Some(&opcode) = payload.first() else {
        return (0, None, "unknown");
    };
    if opcode == EX_CLIENT && payload.len() >= 3 {
        // Ex packets: a `u16` sub-opcode. Ex 0x51 (bookmarks) has a further `i32` sub id; it stays
        // in `raw` until the field decoders of M2 name it.
        let ex = u16::from_le_bytes([payload[1], payload[2]]);
        let name = ExClientOpcode::from_id(ex).map_or("unknown", ExClientOpcode::name);
        return (opcode, Some(ex), name);
    }
    let name = ClientOpcode::from_id(opcode).map_or("unknown", ClientOpcode::name);
    (opcode, None, name)
}

/// `(opcode, ex opcode, name)` of a decrypted server payload.
fn name_server(payload: &[u8]) -> (u8, Option<u16>, &'static str) {
    let Some(&opcode) = payload.first() else {
        return (0, None, "unknown");
    };
    if opcode == EX_SERVER {
        return match payload.len() {
            // NOTE(java-quirk): Java writes the ex id only when it is above 0, so EX_DUMMY
            // (0xFE, 0x0000) is a single 0xFE byte on the wire.
            1 => (
                opcode,
                None,
                ServerOpcode::from_id(opcode, Some(0)).map_or("unknown", ServerOpcode::name),
            ),
            0..=2 => (opcode, None, "unknown"),
            _ => {
                let ex = u16::from_le_bytes([payload[1], payload[2]]);
                let name = ServerOpcode::from_id(opcode, Some(ex)).map_or("unknown", ServerOpcode::name);
                (opcode, Some(ex), name)
            }
        };
    }
    let name = ServerOpcode::from_id(opcode, None).map_or("unknown", ServerOpcode::name);
    (opcode, None, name)
}

// ---------------------------------------------------------------------------------------------
// Sockets
// ---------------------------------------------------------------------------------------------

/// What every game connection needs from the proxy.
#[derive(Clone)]
struct GameContext {
    records: UnboundedSender<Record>,
    next_conn: Arc<AtomicU64>,
    cancel: CancellationToken,
}

struct RelayEntry {
    port: u16,
    /// The real game server address. Updated when a later `ServerList` reports another one.
    target: Arc<Mutex<SocketAddr>>,
}

/// All game listeners of one proxy run.
pub(crate) struct Relays {
    bind_ip: IpAddr,
    /// `Some(base)`: server id `n` listens on `base + n`. `None`: an ephemeral port.
    port_base: Option<u16>,
    context: GameContext,
    entries: Mutex<HashMap<u8, RelayEntry>>,
}

impl Relays {
    /// `records` and `next_conn` are the proxy's capture channel and connection counter, so game
    /// connections are numbered and recorded like login connections.
    pub(crate) fn new(
        bind_ip: IpAddr,
        port_base: Option<u16>,
        cancel: CancellationToken,
        records: UnboundedSender<Record>,
        next_conn: Arc<AtomicU64>,
    ) -> Self {
        Self {
            bind_ip,
            port_base,
            context: GameContext {
                records,
                next_conn,
                cancel,
            },
            entries: Mutex::new(HashMap::new()),
        }
    }

    /// Makes sure a listener exists for `server_id` that relays to `real`, and returns its local
    /// port. A second call for the same id keeps the port and only updates the target.
    pub(crate) async fn ensure(&self, server_id: u8, real: SocketAddr) -> io::Result<u16> {
        if let Some(port) = self.update_existing(server_id, real) {
            return Ok(port);
        }
        let port = match self.port_base {
            Some(base) => base.checked_add(u16::from(server_id)).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "game port base + server id overflows",
                )
            })?,
            None => 0,
        };
        let listener = TcpListener::bind(SocketAddr::new(self.bind_ip, port)).await?;
        let port = listener.local_addr()?.port();

        let target = Arc::new(Mutex::new(real));
        {
            let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some(existing) = entries.get(&server_id) {
                // Another connection created it while we were binding; use that one.
                *existing.target.lock().unwrap_or_else(PoisonError::into_inner) = real;
                return Ok(existing.port);
            }
            entries.insert(
                server_id,
                RelayEntry {
                    port,
                    target: Arc::clone(&target),
                },
            );
        }
        tracing::info!(
            "game relay for server {server_id}: {}:{port} -> {real}",
            self.bind_ip
        );
        tokio::spawn(accept_loop(listener, target, self.context.clone(), server_id));
        Ok(port)
    }

    fn update_existing(&self, server_id: u8, real: SocketAddr) -> Option<u16> {
        let entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        let entry = entries.get(&server_id)?;
        let mut target = entry.target.lock().unwrap_or_else(PoisonError::into_inner);
        if *target != real {
            tracing::info!("game server {server_id} moved: {} -> {real}", *target);
            *target = real;
        }
        Some(entry.port)
    }

    /// `(server id, local port, real address)` for every listener, sorted by id.
    pub(crate) fn snapshot(&self) -> Vec<(u8, u16, SocketAddr)> {
        let entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        let mut all: Vec<_> = entries
            .iter()
            .map(|(id, e)| {
                (
                    *id,
                    e.port,
                    *e.target.lock().unwrap_or_else(PoisonError::into_inner),
                )
            })
            .collect();
        all.sort_by_key(|(id, _, _)| *id);
        all
    }
}

async fn accept_loop(
    listener: TcpListener,
    target: Arc<Mutex<SocketAddr>>,
    context: GameContext,
    server_id: u8,
) {
    loop {
        let accepted = tokio::select! {
            accepted = listener.accept() => accepted,
            () = context.cancel.cancelled() => return,
        };
        let (client, peer) = match accepted {
            Ok(accepted) => accepted,
            Err(e) => {
                tracing::warn!("game relay {server_id}: accept failed: {e}");
                continue;
            }
        };
        let real = *target.lock().unwrap_or_else(PoisonError::into_inner);
        let context = context.clone();
        tokio::spawn(async move {
            let conn = context.next_conn.fetch_add(1, Ordering::Relaxed);
            tracing::info!("game relay {server_id}: connection {conn} from {peer} -> {real}");
            tokio::select! {
                result = serve_client(client, real, conn, &context) => {
                    if let Err(e) = result {
                        tracing::debug!("game connection {conn} ({peer}) ended: {e}");
                    }
                }
                () = context.cancel.cancelled() => {}
            }
        });
    }
}

/// Connects to the real game server and pumps both directions until both have ended.
async fn serve_client(
    client: TcpStream,
    real: SocketAddr,
    conn: u64,
    context: &GameContext,
) -> io::Result<()> {
    let server = tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(real))
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "connecting to the game server timed out"))??;
    client.set_nodelay(true)?;
    server.set_nodelay(true)?;

    let session = Arc::new(Session::default());
    let (client_read, client_write) = client.into_split();
    let (server_read, server_write) = server.into_split();
    let c2s = pump(
        client_read,
        server_write,
        Dir::C2s,
        conn,
        &session,
        &context.records,
    );
    let s2c = pump(
        server_read,
        client_write,
        Dir::S2c,
        conn,
        &session,
        &context.records,
    );
    let (c2s, s2c) = tokio::join!(c2s, s2c);
    c2s.and(s2c)
}

/// Copies `from` to `to` unchanged. Every chunk is decoded **before** it is forwarded, so the
/// session key is known before the client can answer the `KeyPacket`.
async fn pump(
    mut from: OwnedReadHalf,
    mut to: OwnedWriteHalf,
    dir: Dir,
    conn: u64,
    session: &Session,
    records: &UnboundedSender<Record>,
) -> io::Result<()> {
    let mut decoder = Decoder::new(dir);
    let mut buf = vec![0u8; READ_CHUNK];
    let result = loop {
        let n = match from.read(&mut buf).await {
            Ok(0) => break Ok(()),
            Ok(n) => n,
            Err(e) => break Err(e),
        };
        for packet in decoder.feed(&buf[..n], session) {
            tracing::trace!(
                "[conn {conn}] {:?} {} (state {})",
                packet.dir,
                packet.name,
                packet.state.name()
            );
            let fields = packet.redacted.then(|| json!({ "credentials": REDACTED }));
            let mut record = Record::new(
                conn,
                packet.dir,
                Channel::Game,
                packet.opcode,
                packet.name,
                fields,
                &packet.raw,
            );
            record.ex_opcode = packet.ex_opcode;
            // The receiver may be gone (GUI closed the inspector); the relay keeps working.
            let _ = records.send(record);
        }
        if let Err(e) = to.write_all(&buf[..n]).await {
            break Err(e);
        }
    };
    // Pass the end of this direction on, so the other side sees EOF like it would without us.
    let _ = to.shutdown().await;
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY_RANDOM: [u8; 8] = [1, 2, 3, 4, 5, 6, 7, 8];

    fn frame(payload: &[u8]) -> Vec<u8> {
        let total = u16::try_from(payload.len() + 2).unwrap();
        let mut out = total.to_le_bytes().to_vec();
        out.extend_from_slice(payload);
        out
    }

    fn key_packet(flag: i32) -> Vec<u8> {
        let mut p = vec![ServerOpcode::VersionCheck.id(), 1];
        p.extend_from_slice(&KEY_RANDOM);
        p.extend_from_slice(&flag.to_le_bytes());
        p.extend_from_slice(&1i32.to_le_bytes()); // server id
        p.push(1);
        p.extend_from_slice(&0i32.to_le_bytes()); // obfuscation key
        p
    }

    /// The sending side of the game cipher, as the real server and client use it.
    fn sender() -> GameCipher {
        decrypt_cipher(game_key_from_random(KEY_RANDOM))
    }

    fn encrypt_frame(cipher: &mut GameCipher, payload: &[u8]) -> Vec<u8> {
        let mut body = payload.to_vec();
        cipher.encrypt(&mut body);
        frame(&body)
    }

    #[test]
    fn plaintext_frames_get_names_and_ex_opcodes() {
        let session = Session::default();
        let mut c2s = Decoder::new(Dir::C2s);
        let mut bytes = frame(&[0x0E, 0x21, 0xC6, 0, 0]); // ProtocolVersion
        bytes.extend(frame(&[0xD0, 0x36, 0x00])); // ex: RequestGotoLobby
        bytes.extend(frame(&[0x02, 0x00])); // not in the table
        bytes.extend(frame(&[0xD0, 0xFF, 0xFF, 0xAA])); // unknown ex id
        let packets = c2s.feed(&bytes, &session);
        let got: Vec<_> = packets.iter().map(|p| (p.opcode, p.ex_opcode, p.name)).collect();
        assert_eq!(
            got,
            vec![
                (0x0E, None, "PROTOCOL_VERSION"),
                (0xD0, Some(0x36), "REQUEST_GOTO_LOBBY"),
                (0x02, None, "unknown"),
                (0xD0, Some(0xFFFF), "unknown"),
            ]
        );
        assert_eq!(packets[0].raw, vec![0x0E, 0x21, 0xC6, 0, 0]);
        assert_eq!(packets[0].dir, Dir::C2s);
    }

    #[test]
    fn server_names_including_ex_and_the_ex_dummy_quirk() {
        let session = Session::default();
        let mut s2c = Decoder::new(Dir::S2c);
        // The first server frame is the KeyPacket (encryption off here).
        let mut bytes = frame(&key_packet(0));
        bytes.extend(frame(&[0x09, 0, 0, 0])); // CharacterSelectionInfo
        bytes.extend(frame(&[0xFE, 0xE8, 0x00, 0x01])); // ExSay2Fail
        bytes.extend(frame(&[0xFE])); // single-byte EX_DUMMY
        bytes.extend(frame(&[0xFE, 0x12])); // truncated ex id
        bytes.extend(frame(&[0x07])); // not in the table
        let got: Vec<_> = s2c
            .feed(&bytes, &session)
            .iter()
            .map(|p| (p.opcode, p.ex_opcode, p.name))
            .collect();
        assert_eq!(
            got,
            vec![
                (0x2E, None, "VERSION_CHECK"),
                (0x09, None, "CHARACTER_SELECTION_INFO"),
                (0xFE, Some(0xE8), "EX_SAY2_FAIL"),
                (0xFE, None, "EX_DUMMY"),
                (0xFE, None, "unknown"),
                (0x07, None, "unknown"),
            ]
        );
        assert!(session.key.get().is_none(), "flag 0: no encryption");
    }

    #[test]
    fn frames_split_across_reads_and_joined_in_one_read() {
        let session = Session::default();
        let mut dec = Decoder::new(Dir::C2s);
        let one = frame(&[0x14]); // RequestItemList
        let two = frame(&[0x19, 1, 2, 3, 4]); // UseItem
        let mut all = one.clone();
        all.extend(&two);
        // One byte at a time.
        let mut names = Vec::new();
        for byte in &all {
            names.extend(dec.feed(&[*byte], &session).into_iter().map(|p| p.name));
        }
        assert_eq!(names, vec!["REQUEST_ITEM_LIST", "USE_ITEM"]);
        // Both in one read, plus the start of a third.
        let mut chunk = all.clone();
        chunk.push(5);
        assert_eq!(dec.feed(&chunk, &session).len(), 2);
        assert_eq!(
            dec.feed(&[0], &session).len(),
            0,
            "header of a 5-byte frame, body missing"
        );
        assert_eq!(dec.feed(&[0x14, 0, 0], &session).len(), 1);
    }

    #[test]
    fn empty_frames_are_skipped_like_java() {
        let session = Session::default();
        let mut dec = Decoder::new(Dir::C2s);
        let mut bytes = vec![0, 0, 2, 0]; // length 0 and length 2: no body
        bytes.extend(frame(&[0x14]));
        let packets = dec.feed(&bytes, &session);
        assert_eq!(packets.len(), 1);
        assert_eq!(packets[0].name, "REQUEST_ITEM_LIST");
    }

    #[test]
    fn encrypted_traffic_is_decrypted_in_both_directions() {
        let session = Session::default();
        let mut s2c = Decoder::new(Dir::S2c);
        let mut c2s = Decoder::new(Dir::C2s);
        let mut server = sender(); // the server's outbound cipher: the KeyPacket enables it
        let mut client = sender();

        // Client packet before the KeyPacket: plaintext.
        let hello = c2s.feed(&frame(&[0x0E, 1, 0, 0, 0]), &session);
        assert_eq!(hello[0].name, "PROTOCOL_VERSION");

        // KeyPacket: plaintext on the wire. Everything after it is encrypted.
        let key = key_packet(1);
        let first = s2c.feed(&frame(&key), &session);
        assert_eq!(first[0].name, "VERSION_CHECK");
        assert_eq!(first[0].raw, key);
        assert!(session.key.get().is_some());

        // Server frames, several, so the rolling offset matters.
        let payloads: [&[u8]; 3] = [&[0x09, 1, 2, 3], &[0x0B, 9, 9], &[0xFE, 0x8D, 0x00, 7, 7, 7]];
        for (i, payload) in payloads.iter().enumerate() {
            let wire = encrypt_frame(&mut server, payload);
            assert_ne!(&wire[2..], *payload, "frame {i} is encrypted on the wire");
            let got = s2c.feed(&wire, &session);
            assert_eq!(got.len(), 1);
            assert_eq!(got[0].raw, *payload);
        }
        assert_eq!(session.state(), ConnectionState::Entering);

        // Client frames after the KeyPacket are encrypted too, with their own key state.
        let wire = encrypt_frame(&mut client, &[0x12, 0, 0, 0, 0]);
        let got = c2s.feed(&wire, &session);
        assert_eq!(
            (got[0].name, got[0].raw.as_slice()),
            ("CHARACTER_SELECT", &[0x12, 0, 0, 0, 0][..])
        );
        let wire = encrypt_frame(&mut client, &[0x11]);
        let got = c2s.feed(&wire, &session);
        assert_eq!(got[0].name, "ENTER_WORLD");
        assert_eq!(session.state(), ConnectionState::InGame);
    }

    #[test]
    fn the_state_guess_follows_the_login_sequence() {
        let session = Session::default();
        let mut s2c = Decoder::new(Dir::S2c);
        let mut c2s = Decoder::new(Dir::C2s);
        let mut seen = Vec::new();
        let mut step = |to_server: bool, payload: Vec<u8>| {
            let dec = if to_server { &mut c2s } else { &mut s2c };
            let packet = dec.feed(&frame(&payload), &session).remove(0);
            seen.push((packet.name, packet.state));
        };
        step(true, vec![0x0E, 0, 0, 0, 0]);
        step(false, key_packet(0));
        step(true, vec![0x2B, 0]); // AuthLogin
        step(false, vec![0x09, 0]); // CharacterSelectionInfo
        step(true, vec![0x12, 0]); // CharacterSelect
        step(false, vec![0x0B, 0]); // CharacterSelected
        step(true, vec![0x11]); // EnterWorld
        step(false, vec![0x32, 0]); // UserInfo
        use ConnectionState::*;
        assert_eq!(
            seen,
            vec![
                ("PROTOCOL_VERSION", Connected),
                ("VERSION_CHECK", Connected),
                ("AUTH_LOGIN", Connected),
                ("CHARACTER_SELECTION_INFO", Connected),
                ("CHARACTER_SELECT", Authenticated),
                ("CHARACTER_SELECTED", Authenticated),
                ("ENTER_WORLD", Entering),
                ("USER_INFO", InGame),
            ]
        );
    }

    #[test]
    fn a_non_key_first_server_packet_means_no_encryption() {
        let session = Session::default();
        let mut s2c = Decoder::new(Dir::S2c);
        let got = s2c.feed(&frame(&[0x09, 1]), &session);
        assert_eq!(got[0].name, "CHARACTER_SELECTION_INFO");
        assert!(session.key.get().is_none());
        assert_eq!(
            s2c.feed(&frame(&[0x0B, 1]), &session)[0].name,
            "CHARACTER_SELECTED"
        );
    }

    #[test]
    fn a_short_key_packet_is_survived() {
        let session = Session::default();
        let mut s2c = Decoder::new(Dir::S2c);
        let got = s2c.feed(&frame(&[0x2E, 1, 2, 3]), &session);
        assert_eq!(got[0].name, "VERSION_CHECK");
        assert!(session.key.get().is_none());
    }

    #[test]
    fn auth_login_is_redacted_even_when_encrypted() {
        let session = Session::default();
        let mut s2c = Decoder::new(Dir::S2c);
        let mut c2s = Decoder::new(Dir::C2s);
        let mut client = sender();
        s2c.feed(&frame(&key_packet(1)), &session);

        let mut payload = vec![0x2B];
        payload.extend("someaccount".encode_utf16().flat_map(u16::to_le_bytes));
        payload.extend([0, 0]);
        payload.extend(0x1234_5678i32.to_le_bytes());
        let packets = c2s.feed(&encrypt_frame(&mut client, &payload), &session);
        assert_eq!(packets[0].name, "AUTH_LOGIN");
        assert!(packets[0].redacted);
        assert_eq!(packets[0].raw, vec![0x2B], "only the opcode is kept");

        // The stream stays in sync: the next packet is decoded fine.
        let packets = c2s.feed(&encrypt_frame(&mut client, &[0x14]), &session);
        assert_eq!(
            (packets[0].name, packets[0].redacted),
            ("REQUEST_ITEM_LIST", false)
        );
    }
}
