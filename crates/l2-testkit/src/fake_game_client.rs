//! A minimal H5 game client: raw frames, the real game cipher, the packets of the character
//! screen and of the first minutes in the world.
//!
//! It speaks the wire protocol the way the game client does (`ProtocolVersion`, the clear
//! `KeyPacket`, then the rolling XOR cipher in both directions, `AuthLogin`, ...) but knows
//! nothing about the game server's code. Methods that expect a certain packet panic with a clear
//! message when something else arrives, because they only run inside tests.
//!
//! In the world it can walk (`MoveToLocation`, `ValidatePosition`), click (`Action`), attack
//! (`AttackRequest`), follow a link of an html page (`RequestBypassToServer`), pick a button of the
//! death window (`RequestRestartPoint`) and answer a teleport (`Appearing`); the
//! `skip_until`, `expect_later` and `read_quiet` helpers collect what the world sends on its own.
//!
//! Every packet that is sent or received is also written to a log of `(direction, packet name)`
//! ([`FakeGameClient::log`]), so a test can compare the whole conversation with a capture of the
//! Java game server by name and order (see `crates/game-server/tests/e2e.rs`).
//!
//! ```ignore
//! let mut client = FakeGameClient::connect(game_server.client_addr().unwrap()).await;
//! client.handshake(273).await;
//! client.auth("hero", &login_ok, &play_ok).await;
//! let list = client.char_selection_info().await;
//! ```

use std::net::SocketAddr;
use std::time::Duration;

use l2_crypto::{GameCipher, game_key_from_random};
use l2_net::{Decode, Encode, PacketReader};
use l2_protocol_game::client::{
    Action, Appearing, AttackRequest, AuthLogin, CharacterCreate, CharacterSelect, EnterWorld, Logout,
    MoveToLocation, NewCharacter, ProtocolVersion, RequestBypassToServer, RequestGotoLobby, RequestRestart,
    RequestRestartPoint, Say2, ValidatePosition,
};
use l2_protocol_game::server::{
    CharCreateFail, CharCreateOk, CharSelected, CharSelectionInfo, KeyPacket, NewCharacterSuccess,
    RestartResponse,
};
use l2_protocol_game::{ClientOpcode, ExClientOpcode, ServerOpcode};
use l2_protocol_login::server::{LoginOk, PlayOk};
use l2_proxy::Dir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// How long [`FakeGameClient::recv`] waits for a packet.
pub const RECV_TIMEOUT: Duration = Duration::from_secs(10);

/// The server packet that ends the `EnterWorld` answer of the Rust game server (and that the
/// client can wait for: Java sends more after it, the Rust server stops here for now).
pub const ENTER_WORLD_LAST_PACKET: &str = "SKILL_LIST";

/// What [`FakeGameClient::recv_within`] saw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recv {
    /// One decrypted packet (opcode byte first).
    Packet(Vec<u8>),
    /// The server closed the connection (or it broke).
    Closed,
    /// Nothing arrived in time.
    Silent,
}

/// One entry of [`FakeGameClient::log`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Logged {
    /// `C2s` for what the client sent, `S2c` for what it received.
    pub dir: Dir,
    /// The opcode name as in the generated tables (`USER_INFO`, `ENTER_WORLD`, ...).
    pub name: String,
}

/// A connected game client. The cipher is switched on after `KeyPacket` when the server asks
/// for it.
pub struct FakeGameClient {
    stream: TcpStream,
    cipher: Option<GameCipher>,
    log: Vec<Logged>,
}

impl FakeGameClient {
    /// Connects. Nothing is sent yet; call [`handshake`](Self::handshake) next.
    pub async fn connect(addr: SocketAddr) -> Self {
        Self {
            stream: TcpStream::connect(addr).await.unwrap(),
            cipher: None,
            log: Vec::new(),
        }
    }

    /// Every packet sent and received so far, in order.
    pub fn log(&self) -> &[Logged] {
        &self.log
    }

    /// The names of the packets received so far, in order.
    pub fn received_names(&self) -> Vec<String> {
        self.names(Dir::S2c)
    }

    /// The names of the packets sent so far, in order.
    pub fn sent_names(&self) -> Vec<String> {
        self.names(Dir::C2s)
    }

    fn names(&self, dir: Dir) -> Vec<String> {
        self.log
            .iter()
            .filter(|entry| entry.dir == dir)
            .map(|entry| entry.name.clone())
            .collect()
    }

    // ---- raw frames ---------------------------------------------------------------------------

    /// Encrypts (when the cipher is on) and sends one packet body (opcode first).
    pub async fn send(&mut self, mut body: Vec<u8>) {
        self.log.push(Logged {
            dir: Dir::C2s,
            name: client_packet_name(&body),
        });
        if let Some(cipher) = &mut self.cipher {
            cipher.encrypt(&mut body);
        }
        let mut frame = u16::try_from(body.len() + 2).unwrap().to_le_bytes().to_vec();
        frame.extend(body);
        self.stream.write_all(&frame).await.unwrap();
    }

    pub async fn send_packet<P: Encode>(&mut self, packet: &P) {
        self.send(packet.to_bytes()).await;
    }

    /// The next packet, waiting at most `wait`.
    pub async fn recv_within(&mut self, wait: Duration) -> Recv {
        let mut header = [0u8; 2];
        match tokio::time::timeout(wait, self.stream.read_exact(&mut header)).await {
            Err(_) => return Recv::Silent,
            Ok(Err(_)) => return Recv::Closed,
            Ok(Ok(_)) => {}
        }
        let Some(len) = usize::from(u16::from_le_bytes(header)).checked_sub(2) else {
            return Recv::Closed;
        };
        let mut body = vec![0u8; len];
        match tokio::time::timeout(wait, self.stream.read_exact(&mut body)).await {
            Ok(Ok(_)) => {}
            Ok(Err(_)) | Err(_) => return Recv::Closed,
        }
        if let Some(cipher) = &mut self.cipher {
            cipher.decrypt(&mut body);
        }
        if !body.is_empty() {
            self.log.push(Logged {
                dir: Dir::S2c,
                name: packet_name(&body),
            });
        }
        Recv::Packet(body)
    }

    /// The next packet within [`RECV_TIMEOUT`], `None` when the server closed the connection.
    /// Panics when nothing arrives.
    pub async fn recv(&mut self) -> Option<Vec<u8>> {
        match self.recv_within(RECV_TIMEOUT).await {
            Recv::Packet(body) => Some(body),
            Recv::Closed => None,
            Recv::Silent => panic!("timed out waiting for a packet from the game server"),
        }
    }

    /// Like [`recv`](Self::recv), but a closed connection fails the test too.
    pub async fn expect(&mut self) -> Vec<u8> {
        self.recv()
            .await
            .expect("a packet from the game server, but it closed the connection")
    }

    /// The next packet, which must be called `name`.
    pub async fn expect_named(&mut self, name: &str) -> Vec<u8> {
        let body = self.expect().await;
        assert_eq!(packet_name(&body), name, "the next packet from the game server");
        body
    }

    /// Reads until the server closes the connection and returns what arrived before that.
    pub async fn expect_closed(&mut self) -> Vec<Vec<u8>> {
        let mut before = Vec::new();
        loop {
            match self.recv_within(RECV_TIMEOUT).await {
                Recv::Packet(body) => before.push(body),
                Recv::Closed => return before,
                Recv::Silent => panic!("the game server should have closed the connection"),
            }
        }
    }

    /// Nothing at all arrives for `wait` (and the connection stays open).
    pub async fn expect_silence(&mut self, wait: Duration) {
        match self.recv_within(wait).await {
            Recv::Silent => {}
            Recv::Packet(body) => panic!("unexpected packet {}", packet_name(&body)),
            Recv::Closed => panic!("the game server closed the connection"),
        }
    }

    /// Reads packets until one is called `name` and returns that one; the packets before it are
    /// dropped (they stay in [`log`](Self::log)). For tests that only care about one answer among
    /// the traffic the world sends on its own.
    pub async fn skip_until(&mut self, name: &str) -> Vec<u8> {
        self.read_until(name)
            .await
            .pop()
            .expect("read_until returns the match")
    }

    /// Like [`skip_until`](Self::skip_until), decoded as `P` (a single-byte opcode packet).
    pub async fn expect_later<P: Decode>(&mut self, name: &str) -> P {
        decode(&self.skip_until(name).await)
    }

    /// Everything that arrives until nothing has arrived for `quiet` (the connection must stay
    /// open). The way to collect what a world event made the server send.
    pub async fn read_quiet(&mut self, quiet: Duration) -> Vec<Vec<u8>> {
        let mut packets = Vec::new();
        loop {
            match self.recv_within(quiet).await {
                Recv::Packet(body) => packets.push(body),
                Recv::Silent => return packets,
                Recv::Closed => panic!("the game server closed the connection"),
            }
        }
    }

    /// Reads packets until one is called `name` (that one is the last of the result).
    pub async fn read_until(&mut self, name: &str) -> Vec<Vec<u8>> {
        let mut packets = Vec::new();
        loop {
            let body = self.expect().await;
            let found = packet_name(&body) == name;
            packets.push(body);
            if found {
                return packets;
            }
        }
    }

    // ---- the conversation ---------------------------------------------------------------------

    /// `ProtocolVersion`, then the `KeyPacket` (in clear). Switches the cipher on when the server
    /// says so, like the real client. Fails the test when the protocol is refused.
    pub async fn handshake(&mut self, version: i32) -> KeyPacket {
        let key = self.handshake_any(version).await;
        assert_eq!(
            key.result,
            KeyPacket::RESULT_OK,
            "the server accepts protocol {version}"
        );
        key
    }

    /// Like [`handshake`](Self::handshake), but returns a refusal instead of failing.
    pub async fn handshake_any(&mut self, version: i32) -> KeyPacket {
        self.send_packet(&ProtocolVersion { version }).await;
        let body = self.expect_named("VERSION_CHECK").await;
        let key: KeyPacket = decode(&body);
        if key.use_encryption {
            let mut cipher = GameCipher::new(game_key_from_random(key.key));
            // The first `encrypt` only switches the client's copy of the cipher on.
            cipher.encrypt(&mut []);
            self.cipher = Some(cipher);
        }
        key
    }

    /// `AuthLogin` with the keys the login server gave out (`LoginOk` and `PlayOk`). Does not
    /// wait for the answer.
    pub async fn auth(&mut self, account: &str, login_ok: &LoginOk, play_ok: &PlayOk) {
        self.send_packet(&AuthLogin {
            login_name: account.to_owned(),
            play_key2: play_ok.play_ok2,
            play_key1: play_ok.play_ok1,
            login_key1: login_ok.login_ok1,
            login_key2: login_ok.login_ok2,
        })
        .await;
    }

    /// The next packet, which must be a `CHARACTER_SELECTION_INFO`.
    pub async fn char_selection_info(&mut self) -> CharSelectionInfo {
        decode(&self.expect_named("CHARACTER_SELECTION_INFO").await)
    }

    /// `NewCharacter` and its `NEW_CHARACTER_SUCCESS` (the creation screen's templates).
    pub async fn new_character(&mut self) -> NewCharacterSuccess {
        self.send_packet(&NewCharacter).await;
        decode(&self.expect_named("NEW_CHARACTER_SUCCESS").await)
    }

    /// `CharacterCreate`: `Ok` for `CHARACTER_CREATE_SUCCESS`, `Err(reason)` for a
    /// `CHARACTER_CREATE_FAIL`.
    pub async fn create_character(&mut self, packet: &CharacterCreate) -> Result<(), i32> {
        self.send_packet(packet).await;
        let body = self.expect().await;
        if body == CharCreateOk.to_bytes() {
            return Ok(());
        }
        assert_eq!(
            packet_name(&body),
            "CHARACTER_CREATE_FAIL",
            "the answer to CharacterCreate"
        );
        Err(decode::<CharCreateFail>(&body).reason)
    }

    /// `RequestGotoLobby` and the character list that answers it.
    pub async fn goto_lobby(&mut self) -> CharSelectionInfo {
        self.send_packet(&RequestGotoLobby).await;
        self.char_selection_info().await
    }

    /// `CharacterSelect` for `slot`: reads `SSQ_INFO`, `CHARACTER_SELECTED` and the `SKILL_LIST`
    /// that follows it, and returns the middle one.
    pub async fn select_character(&mut self, slot: i32) -> CharSelected {
        self.send_packet(&CharacterSelect {
            char_slot: slot,
            unk1: 0,
            unk2: 0,
            unk3: 0,
            unk4: 0,
        })
        .await;
        self.expect_named("SSQ_INFO").await;
        let selected = decode(&self.expect_named("CHARACTER_SELECTED").await);
        self.expect_named(ENTER_WORLD_LAST_PACKET).await;
        selected
    }

    /// `EnterWorld`, then every packet up to and including the closing `SKILL_LIST`; returns
    /// their names.
    pub async fn enter_world(&mut self) -> Vec<String> {
        self.send_packet(&enter_world_packet()).await;
        self.read_until(ENTER_WORLD_LAST_PACKET)
            .await
            .iter()
            .map(|body| packet_name(body))
            .collect()
    }

    /// `Say2`. Does not wait for the echo.
    pub async fn say(&mut self, chat_type: i32, text: &str, target: Option<&str>) {
        self.send_packet(&Say2 {
            text: text.to_owned(),
            chat_type,
            target: target.map(str::to_owned),
        })
        .await;
    }

    /// `MoveToLocation` with the mouse (`movement_mode` 1): walk to `target`, with the position
    /// the client believes it has as `origin`. Does not wait for the answer.
    pub async fn move_to(&mut self, target: (i32, i32, i32), origin: (i32, i32, i32)) {
        self.send_packet(&MoveToLocation {
            target_x: target.0,
            target_y: target.1,
            target_z: target.2,
            origin_x: origin.0,
            origin_y: origin.1,
            origin_z: origin.2,
            movement_mode: 1,
        })
        .await;
    }

    /// `ValidatePosition`: where the client thinks the player is.
    pub async fn validate_position(&mut self, x: i32, y: i32, z: i32, heading: i32) {
        self.send_packet(&ValidatePosition {
            x,
            y,
            z,
            heading,
            vehicle_id: 0,
        })
        .await;
    }

    /// `Action`: a simple click on an object (target it; a second click talks to an NPC).
    /// `origin` is where the client stands; Java reads it and ignores it.
    pub async fn click(&mut self, object_id: i32, origin: (i32, i32, i32)) {
        self.send_packet(&Action {
            object_id,
            origin_x: origin.0,
            origin_y: origin.1,
            origin_z: origin.2,
            action_id: Action::SIMPLE_CLICK,
        })
        .await;
    }

    /// `AttackRequest`: a simple click that attacks (the client sends it instead of `Action` for
    /// something it knows is attackable and already targeted). Does not wait for the answer.
    pub async fn attack(&mut self, object_id: i32, origin: (i32, i32, i32)) {
        self.send_packet(&AttackRequest {
            object_id,
            origin_x: origin.0,
            origin_y: origin.1,
            origin_z: origin.2,
            attack_id: AttackRequest::SIMPLE_CLICK,
        })
        .await;
    }

    /// `RequestRestartPoint`: the button of the death window (0 = "to the village"). Does not
    /// wait for the answer.
    pub async fn restart_point(&mut self, requested_point_type: i32) {
        self.send_packet(&RequestRestartPoint { requested_point_type })
            .await;
    }

    /// `RequestBypassToServer`: the command of a link in an html page.
    pub async fn bypass(&mut self, command: &str) {
        self.send_packet(&RequestBypassToServer {
            command: command.to_owned(),
        })
        .await;
    }

    /// `Appearing`: the client has loaded the place of a teleport.
    pub async fn appearing(&mut self) {
        self.send_packet(&Appearing).await;
    }

    /// `Logout` in the world: returns the packets that arrive before the server closes the
    /// connection.
    pub async fn logout(&mut self) -> Vec<Vec<u8>> {
        self.send_packet(&Logout).await;
        self.expect_closed().await
    }

    /// `RequestRestart` in the world: expects `RESTART_RESPONSE` (true) and then the character
    /// list of the character screen.
    pub async fn restart(&mut self) -> CharSelectionInfo {
        self.send_packet(&RequestRestart).await;
        let body = self.expect_named("RESTART_RESPONSE").await;
        assert_eq!(
            body,
            RestartResponse { result: true }.to_bytes(),
            "the restart is allowed"
        );
        self.char_selection_info().await
    }
}

/// The names of packet bodies, in order.
pub fn names_of(bodies: &[Vec<u8>]) -> Vec<String> {
    bodies.iter().map(|body| packet_name(body)).collect()
}

/// An `EnterWorld` like the client's: the unknown blocks are zero and the five trace route
/// hops are empty.
pub fn enter_world_packet() -> EnterWorld {
    EnterWorld {
        unknown1: [0; 32],
        unknown2: [0; 4],
        unknown3: [0; 32],
        unknown4: 0,
        tracert: [[0; 4]; 5],
    }
}

/// A `CharacterCreate` for a male human of `class_id` with face, hair style and colour 1 (all
/// three are valid for every class). The six base stats are zero: the server ignores them.
pub fn character_create(name: &str, class_id: i32) -> CharacterCreate {
    CharacterCreate {
        name: name.to_owned(),
        race: 0,
        is_female: false,
        class_id,
        int: 0,
        str: 0,
        con: 0,
        men: 0,
        dex: 0,
        wit: 0,
        hair_style: 1,
        hair_color: 1,
        face: 1,
    }
}

/// The opcode name of a server packet body (`?XX` for an opcode the tables do not know).
pub fn packet_name(body: &[u8]) -> String {
    let Some(&opcode) = body.first() else {
        return "<empty>".to_owned();
    };
    // 0xFE packets carry a second id; a body too short for it is named by the plain opcode.
    let ex = (opcode == 0xFE && body.len() >= 3).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(opcode, ex).map_or_else(|| format!("?{opcode:02X}"), |o| o.name().to_owned())
}

/// The opcode name of a client packet body.
fn client_packet_name(body: &[u8]) -> String {
    let Some(&opcode) = body.first() else {
        return "<empty>".to_owned();
    };
    if opcode == ClientOpcode::ExPacket.id() && body.len() >= 3 {
        // 0xD0 is followed by the u16 ex id (the lobby request is one of these).
        let ex = u16::from_le_bytes([body[1], body[2]]);
        return ExClientOpcode::from_id(ex).map_or_else(|| format!("?D0:{ex:04X}"), |o| o.name().to_owned());
    }
    ClientOpcode::from_id(opcode).map_or_else(|| format!("?{opcode:02X}"), |o| o.name().to_owned())
}

/// Decodes the fields after the opcode of a single-byte-opcode server packet.
pub fn decode<P: Decode>(body: &[u8]) -> P {
    P::decode(&mut PacketReader::new(&body[1..])).unwrap_or_else(|e| {
        panic!(
            "cannot decode {} as {}: {e:?}",
            packet_name(body),
            std::any::type_name::<P>()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    /// Reads one frame the way a server does.
    async fn read_frame(stream: &mut TcpStream) -> Vec<u8> {
        let mut header = [0u8; 2];
        stream.read_exact(&mut header).await.unwrap();
        let mut body = vec![0u8; usize::from(u16::from_le_bytes(header)) - 2];
        stream.read_exact(&mut body).await.unwrap();
        body
    }

    async fn write_frame(stream: &mut TcpStream, body: &[u8]) {
        let mut frame = u16::try_from(body.len() + 2).unwrap().to_le_bytes().to_vec();
        frame.extend_from_slice(body);
        stream.write_all(&frame).await.unwrap();
    }

    /// A scripted game server: key exchange, one encrypted request, one encrypted answer, close.
    #[tokio::test]
    async fn the_handshake_and_the_cipher_work_in_both_directions() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            // ProtocolVersion arrives in clear.
            let body = read_frame(&mut stream).await;
            assert_eq!(body[0], ProtocolVersion::OPCODE.id());
            let random = [1, 2, 3, 4, 5, 6, 7, 8];
            let key = KeyPacket {
                result: KeyPacket::RESULT_OK,
                key: random,
                use_encryption: true,
                server_id: 5,
            };
            // KeyPacket goes out in clear, everything after it is encrypted.
            write_frame(&mut stream, &key.to_bytes()).await;
            let mut cipher = GameCipher::new(game_key_from_random(random));
            cipher.encrypt(&mut []);

            let mut request = read_frame(&mut stream).await;
            cipher.decrypt(&mut request);
            assert_eq!(request, RequestGotoLobby.to_bytes());

            let mut answer = RequestRestart.to_bytes();
            cipher.encrypt(&mut answer);
            write_frame(&mut stream, &answer).await;
            // Closing ends `expect_closed`.
        });

        let mut client = FakeGameClient::connect(addr).await;
        let key = client.handshake(273).await;
        assert_eq!(key.server_id, 5);
        assert!(key.use_encryption);
        client.send_packet(&RequestGotoLobby).await;
        let answer = client.expect().await;
        assert_eq!(answer, RequestRestart.to_bytes());
        server.await.unwrap();
        assert!(client.expect_closed().await.is_empty());

        // The log names both directions in order.
        assert_eq!(client.sent_names(), ["PROTOCOL_VERSION", "REQUEST_GOTO_LOBBY"]);
        assert_eq!(client.received_names().len(), 2);
        assert_eq!(client.received_names()[0], "VERSION_CHECK");
    }

    #[tokio::test]
    async fn without_encryption_the_traffic_stays_in_clear() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            read_frame(&mut stream).await;
            let key = KeyPacket {
                result: KeyPacket::RESULT_OK,
                key: [0; 8],
                use_encryption: false,
                server_id: 1,
            };
            write_frame(&mut stream, &key.to_bytes()).await;
            let request = read_frame(&mut stream).await;
            assert_eq!(request, Logout.to_bytes());
            write_frame(&mut stream, &l2_protocol_game::server::LeaveWorld.to_bytes()).await;
        });
        let mut client = FakeGameClient::connect(addr).await;
        assert!(!client.handshake(273).await.use_encryption);
        let before_close = client.logout().await;
        assert_eq!(before_close, [l2_protocol_game::server::LeaveWorld.to_bytes()]);
        server.await.unwrap();
    }

    /// The in-world senders write the packets Java reads, and the log names them.
    #[tokio::test]
    async fn the_world_packets_are_sent_and_named() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            read_frame(&mut stream).await;
            let key = KeyPacket {
                result: KeyPacket::RESULT_OK,
                key: [0; 8],
                use_encryption: false,
                server_id: 1,
            };
            write_frame(&mut stream, &key.to_bytes()).await;
            let mut received = Vec::new();
            for _ in 0..5 {
                received.push(read_frame(&mut stream).await);
            }
            received
        });
        let mut client = FakeGameClient::connect(addr).await;
        client.handshake(273).await;
        client.move_to((1, 2, 3), (4, 5, 6)).await;
        client.validate_position(7, 8, 9, 10).await;
        client.click(11, (0, 0, 0)).await;
        client.bypass("npc_5_showTeleports").await;
        client.appearing().await;
        let received = server.await.unwrap();
        assert_eq!(
            client.sent_names(),
            [
                "PROTOCOL_VERSION",
                "MOVE_TO_LOCATION",
                "VALIDATE_POSITION",
                "ACTION",
                "REQUEST_BYPASS_TO_SERVER",
                "APPEARING"
            ]
        );
        assert_eq!(
            received[0],
            MoveToLocation {
                target_x: 1,
                target_y: 2,
                target_z: 3,
                origin_x: 4,
                origin_y: 5,
                origin_z: 6,
                movement_mode: 1,
            }
            .to_bytes()
        );
        assert_eq!(received[4], Appearing.to_bytes());
    }

    /// The combat senders write the packets Java reads, and the log names them.
    #[tokio::test]
    async fn the_combat_packets_are_sent_and_named() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            read_frame(&mut stream).await;
            let key = KeyPacket {
                result: KeyPacket::RESULT_OK,
                key: [0; 8],
                use_encryption: false,
                server_id: 1,
            };
            write_frame(&mut stream, &key.to_bytes()).await;
            let mut received = Vec::new();
            for _ in 0..2 {
                received.push(read_frame(&mut stream).await);
            }
            received
        });
        let mut client = FakeGameClient::connect(addr).await;
        client.handshake(273).await;
        client.attack(77, (1, 2, 3)).await;
        client.restart_point(0).await;
        let received = server.await.unwrap();
        assert_eq!(
            client.sent_names(),
            ["PROTOCOL_VERSION", "ATTACK", "REQUEST_RESTART_POINT"]
        );
        assert_eq!(
            received[0],
            AttackRequest {
                object_id: 77,
                origin_x: 1,
                origin_y: 2,
                origin_z: 3,
                attack_id: AttackRequest::SIMPLE_CLICK,
            }
            .to_bytes()
        );
        assert_eq!(
            received[1],
            RequestRestartPoint {
                requested_point_type: 0
            }
            .to_bytes()
        );
    }

    #[tokio::test]
    async fn a_refused_protocol_is_reported_by_handshake_any() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            read_frame(&mut stream).await;
            let key = KeyPacket {
                result: KeyPacket::RESULT_WRONG_PROTOCOL,
                key: [0; 8],
                use_encryption: false,
                server_id: 1,
            };
            write_frame(&mut stream, &key.to_bytes()).await;
        });
        let mut client = FakeGameClient::connect(addr).await;
        let key = client.handshake_any(999).await;
        assert_eq!(key.result, KeyPacket::RESULT_WRONG_PROTOCOL);
    }

    #[test]
    fn packet_names_come_from_the_opcode_tables() {
        assert_eq!(packet_name(&[0x2E, 0, 0]), "VERSION_CHECK");
        assert_eq!(packet_name(&CharCreateOk.to_bytes()), "CHARACTER_CREATE_SUCCESS");
        assert_eq!(packet_name(&[0xFF]), "?FF");
        assert_eq!(packet_name(&[]), "<empty>");
        assert_eq!(client_packet_name(&Logout.to_bytes()), "LOGOUT");
        assert_eq!(
            client_packet_name(&enter_world_packet().to_bytes()),
            "ENTER_WORLD"
        );
    }
}
