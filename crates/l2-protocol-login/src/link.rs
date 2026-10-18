//! Packets between the login server and the game servers (port 9014).
//!
//! The LS side reads what the GS writes and the other way round. Each struct is one packet with
//! `OPCODE`, `impl Encode` (the writer, Java `write()`) and `impl Decode` (the reader, Java
//! `readImpl`). Both halves exist so the Rust LS, the fake GS in tests and the Rust GS in M2 can
//! all use the same structs. The `u16` length header and the link crypto (Blowfish, XOR checksum)
//! are not in this module; see `l2-crypto::link`.
//!
//! The opcode is the first byte of each payload. Java checks the opcode against the connection
//! state before it reads anything, so [`decode_from_gs`] does the same.

use std::fmt;

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

/// Protocol revision the LS sends in `InitLS` (Java `LoginServer.PROTOCOL_REV`).
pub const PROTOCOL_REV: i32 = 0x0106;
/// Longest hex id the LS accepts in `GameServerAuth` (Java `MAX_HEX_ID_BYTES`).
pub const MAX_HEX_ID_BYTES: i32 = 64;
/// Most subnet/host pairs the LS accepts in `GameServerAuth` (Java `MAX_HOST_PAIRS`).
pub const MAX_HOST_PAIRS: i32 = 32;

/// Where a GS link is in the handshake. Java: `GameServerPacketHandler.GameServerState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LinkState {
    /// Connected, the static Blowfish key is in use. Waiting for `BlowFishKey`.
    Connected,
    /// The GS Blowfish key is agreed. Waiting for `GameServerAuth`.
    BfConnected,
    /// Registered. The GS sends its game packets.
    Authed,
}

/// An error decoding a GS-to-LS packet.
#[derive(Debug, PartialEq, Eq)]
pub enum LinkError {
    /// The opcode is not allowed in `state`. Java `logInvalidOpcode` closes the link with
    /// `forceClose(LoginServerFail::NOT_AUTHED)`, so the caller must do the same.
    UnknownOpcode { opcode: u8, state: LinkState },
    /// The body is malformed: truncated, or a limit from Java was broken.
    Packet(PacketError),
}

impl fmt::Display for LinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownOpcode { opcode, state } => {
                write!(f, "unknown link opcode {opcode:#04x} in state {state:?}")
            }
            Self::Packet(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for LinkError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::UnknownOpcode { .. } => None,
            Self::Packet(err) => Some(err),
        }
    }
}

impl From<PacketError> for LinkError {
    fn from(err: PacketError) -> Self {
        Self::Packet(err)
    }
}

/// The error for a read that needs `needed` more bytes than the packet has.
fn truncated(r: &PacketReader, needed: usize) -> PacketError {
    PacketError::Truncated {
        offset: r.position(),
        needed,
        remaining: r.remaining(),
    }
}

/// Java `readBytes(readInt())`: an `i32` length, then that many bytes. Java rejects a negative
/// length, and one longer than the packet, so both are errors here.
fn len_prefixed_bytes(r: &mut PacketReader, field: &'static str) -> Result<Vec<u8>, PacketError> {
    let len = r.i32()?;
    let len =
        usize::try_from(len).map_err(|_| PacketError::invalid(field, format!("negative length {len}")))?;
    Ok(r.bytes(len)?.to_vec())
}

// ---------------------------------------------------------------------------------------------
// LS -> GS (the login server writes, the game server reads)
// ---------------------------------------------------------------------------------------------

// Java: loginserver/network/gameserverpackets/send/InitLS.java (GS reader: gameserver/.../receive/InitLS.java)
/// First packet on a link: the RSA public key the GS encrypts its Blowfish key with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitLS {
    pub protocol_revision: i32,
    /// RSA modulus as Java `BigInteger.toByteArray()`, so it may start with a `0x00` sign byte.
    pub public_key: Vec<u8>,
}

impl InitLS {
    pub const OPCODE: u8 = 0x00;

    /// An `InitLS` with the protocol revision Java sends.
    pub fn new(public_key: Vec<u8>) -> Self {
        Self {
            protocol_revision: PROTOCOL_REV,
            public_key,
        }
    }
}

impl Encode for InitLS {
    fn encode(&self, w: &mut PacketWriter) {
        w.u8(Self::OPCODE)
            .i32(self.protocol_revision)
            .i32(self.public_key.len() as i32)
            .bytes(&self.public_key);
    }
}

impl Decode for InitLS {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let protocol_revision = r.i32()?;
        let public_key = len_prefixed_bytes(r, "InitLS.public_key")?;
        Ok(Self {
            protocol_revision,
            public_key,
        })
    }
}

// Java: loginserver/network/gameserverpackets/send/LoginServerFail.java
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginServerFail {
    /// One byte on the wire. Java writes `writeByte(reason)`, so values above 0xFF lose their high bits.
    pub reason: u8,
}

impl LoginServerFail {
    pub const OPCODE: u8 = 0x01;
    pub const REASON_IP_BANNED: u8 = 1;
    pub const REASON_IP_RESERVED: u8 = 2;
    pub const REASON_WRONG_HEXID: u8 = 3;
    pub const REASON_ID_RESERVED: u8 = 4;
    pub const REASON_NO_FREE_ID: u8 = 5;
    /// What the LS sends when it closes a link for an unexpected packet.
    pub const NOT_AUTHED: u8 = 6;
    /// Java `REASON_ALREADY_LOGGED8IN` (the `8` is a typo in Java, not renamed here).
    pub const REASON_ALREADY_LOGGED_IN: u8 = 7;
}

impl Encode for LoginServerFail {
    fn encode(&self, w: &mut PacketWriter) {
        w.u8(Self::OPCODE).u8(self.reason);
    }
}

impl Decode for LoginServerFail {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { reason: r.u8()? })
    }
}

// Java: loginserver/network/gameserverpackets/send/AuthResponse.java
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthResponse {
    pub server_id: u8,
    /// Looked up from the server id by the LS. Java writes an empty string when the id is unknown.
    pub server_name: String,
}

impl AuthResponse {
    pub const OPCODE: u8 = 0x02;
}

impl Encode for AuthResponse {
    fn encode(&self, w: &mut PacketWriter) {
        w.u8(Self::OPCODE).u8(self.server_id).string(&self.server_name);
    }
}

impl Decode for AuthResponse {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            server_id: r.u8()?,
            server_name: r.string(),
        })
    }
}

// Java: loginserver/network/gameserverpackets/send/PlayerAuthResponse.java
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerAuthResponse {
    pub account: String,
    pub authed: bool,
}

impl PlayerAuthResponse {
    pub const OPCODE: u8 = 0x03;
}

impl Encode for PlayerAuthResponse {
    fn encode(&self, w: &mut PacketWriter) {
        w.u8(Self::OPCODE).string(&self.account).bool(self.authed);
    }
}

impl Decode for PlayerAuthResponse {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            account: r.string(),
            authed: r.bool()?,
        })
    }
}

// Java: loginserver/network/gameserverpackets/send/KickPlayer.java
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KickPlayer {
    pub account: String,
}

impl KickPlayer {
    pub const OPCODE: u8 = 0x04;
}

impl Encode for KickPlayer {
    fn encode(&self, w: &mut PacketWriter) {
        w.u8(Self::OPCODE).string(&self.account);
    }
}

impl Decode for KickPlayer {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { account: r.string() })
    }
}

// Java: loginserver/network/gameserverpackets/send/RequestCharacters.java
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestCharacters {
    pub account: String,
}

impl RequestCharacters {
    pub const OPCODE: u8 = 0x05;
}

impl Encode for RequestCharacters {
    fn encode(&self, w: &mut PacketWriter) {
        w.u8(Self::OPCODE).string(&self.account);
    }
}

impl Decode for RequestCharacters {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { account: r.string() })
    }
}

// Java: loginserver/network/gameserverpackets/send/ChangePasswordResponse.java
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangePasswordResponse {
    pub character_name: String,
    pub message: String,
}

impl ChangePasswordResponse {
    pub const OPCODE: u8 = 0x06;
}

impl Encode for ChangePasswordResponse {
    fn encode(&self, w: &mut PacketWriter) {
        w.u8(Self::OPCODE)
            .string(&self.character_name)
            .string(&self.message);
    }
}

impl Decode for ChangePasswordResponse {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            character_name: r.string(),
            message: r.string(),
        })
    }
}

// ---------------------------------------------------------------------------------------------
// GS -> LS (the game server writes, the login server reads)
// ---------------------------------------------------------------------------------------------

// Java: gameserver/network/loginserverpackets/send/BlowFishKey.java (LS reader: loginserver/.../receive/BlowFishKey.java)
/// The GS's Blowfish key, RSA-NoPadding encrypted with the key from `InitLS`.
///
/// This struct carries the encrypted block. The RSA step is not done here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlowFishKey {
    pub encrypted_key: Vec<u8>,
}

impl BlowFishKey {
    pub const OPCODE: u8 = 0x00;
}

impl Encode for BlowFishKey {
    fn encode(&self, w: &mut PacketWriter) {
        w.u8(Self::OPCODE)
            .i32(self.encrypted_key.len() as i32)
            .bytes(&self.encrypted_key);
    }
}

impl Decode for BlowFishKey {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let size = r.i32()?;
        // NOTE(java-quirk): Java logs a warning for a size <= 0 and returns without reading
        // anything. The connection stays CONNECTED, so no error here, just an empty block.
        if size <= 0 {
            return Ok(Self {
                encrypted_key: Vec::new(),
            });
        }
        Ok(Self {
            encrypted_key: r.bytes(size as usize)?.to_vec(),
        })
    }
}

// Java: gameserver/network/loginserverpackets/send/AuthRequest.java (LS reader: loginserver/.../receive/GameServerAuth.java)
/// Registration request. Java sends the subnet and host lists as two lists, and the LS reads them
/// back as pairs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameServerAuth {
    pub desired_id: u8,
    pub accept_alternate: bool,
    /// Written by the GS. The LS reads it and does not use it ("reserved host flag").
    pub reserve_host: bool,
    pub port: u16,
    pub max_players: i32,
    /// At most [`MAX_HEX_ID_BYTES`] bytes when decoded.
    pub hex_id: Vec<u8>,
    /// Subnet and host lists. Encoding writes the first `min(subnets, hosts)` pairs, and decoding
    /// gives two lists of equal length.
    pub subnets: Vec<String>,
    pub hosts: Vec<String>,
}

impl GameServerAuth {
    pub const OPCODE: u8 = 0x01;
}

impl Encode for GameServerAuth {
    fn encode(&self, w: &mut PacketWriter) {
        w.u8(Self::OPCODE)
            .u8(self.desired_id)
            .bool(self.accept_alternate)
            .bool(self.reserve_host)
            .u16(self.port)
            .i32(self.max_players)
            .i32(self.hex_id.len() as i32)
            .bytes(&self.hex_id);
        // NOTE(java-quirk): Java writes `min(subnets.size(), hosts.size())` pairs, so extra
        // entries on the longer list are dropped without an error. `zip` does the same.
        let pairs = self.subnets.len().min(self.hosts.len());
        w.i32(pairs as i32);
        for (subnet, host) in self.subnets.iter().zip(&self.hosts) {
            w.string(subnet).string(host);
        }
    }
}

impl Decode for GameServerAuth {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let desired_id = r.u8()?;
        let accept_alternate = r.bool()?;
        let reserve_host = r.bool()?;
        let port = r.u16()?;
        let max_players = r.i32()?;

        // Java sends `forceClose(REASON_WRONG_HEXID)` for these limits. The caller does the same
        // when it gets this error.
        let hex_size = r.i32()?;
        if !(0..=MAX_HEX_ID_BYTES).contains(&hex_size) {
            return Err(PacketError::invalid(
                "GameServerAuth.hex_id",
                format!("length {hex_size} is outside 0..={MAX_HEX_ID_BYTES}"),
            ));
        }
        let hex_id = r.bytes(hex_size as usize)?.to_vec();

        let pairs = r.i32()?;
        if !(0..=MAX_HOST_PAIRS).contains(&pairs) {
            return Err(PacketError::invalid(
                "GameServerAuth.hosts",
                format!("{pairs} host pairs is outside 0..={MAX_HOST_PAIRS}"),
            ));
        }
        let mut subnets = Vec::with_capacity(pairs as usize);
        let mut hosts = Vec::with_capacity(pairs as usize);
        for _ in 0..pairs {
            subnets.push(r.string());
            hosts.push(r.string());
        }

        Ok(Self {
            desired_id,
            accept_alternate,
            reserve_host,
            port,
            max_players,
            hex_id,
            subnets,
            hosts,
        })
    }
}

// Java: gameserver/network/loginserverpackets/send/PlayerInGame.java (LS reader: loginserver/.../receive/PlayerInGame.java)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerInGame {
    pub accounts: Vec<String>,
}

impl PlayerInGame {
    pub const OPCODE: u8 = 0x02;
}

impl Encode for PlayerInGame {
    fn encode(&self, w: &mut PacketWriter) {
        // NOTE(java-quirk): Java `writeShort(size)`, so more than 65535 accounts wrap.
        w.u8(Self::OPCODE).u16(self.accounts.len() as u16);
        for account in &self.accounts {
            w.string(account);
        }
    }
}

impl Decode for PlayerInGame {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let count = r.u16()?;
        let mut accounts = Vec::with_capacity(usize::from(count));
        for _ in 0..count {
            // Java throws when fewer than 2 bytes are left for the next name. `string()` would
            // return "" at the end of the buffer, so the check is needed here.
            if r.remaining() < 2 {
                return Err(truncated(r, 2));
            }
            accounts.push(r.string());
        }
        Ok(Self { accounts })
    }
}

// Java: gameserver/network/loginserverpackets/send/PlayerLogout.java (LS reader: loginserver/.../receive/PlayerLogout.java)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerLogout {
    pub account: String,
}

impl PlayerLogout {
    pub const OPCODE: u8 = 0x03;
}

impl Encode for PlayerLogout {
    fn encode(&self, w: &mut PacketWriter) {
        w.u8(Self::OPCODE).string(&self.account);
    }
}

impl Decode for PlayerLogout {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        // Java: `remaining() < 3` with the opcode counted, so the body needs at least 2 bytes.
        if r.remaining() < 2 {
            return Err(truncated(r, 2));
        }
        Ok(Self { account: r.string() })
    }
}

// Java: gameserver/network/loginserverpackets/send/ChangeAccessLevel.java (LS reader: loginserver/.../receive/ChangeAccessLevel.java)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeAccessLevel {
    pub account: String,
    pub level: i32,
}

impl ChangeAccessLevel {
    pub const OPCODE: u8 = 0x04;
}

impl Encode for ChangeAccessLevel {
    fn encode(&self, w: &mut PacketWriter) {
        // The level comes before the account name on the wire.
        w.u8(Self::OPCODE).i32(self.level).string(&self.account);
    }
}

impl Decode for ChangeAccessLevel {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let level = r.i32()?;
        Ok(Self {
            account: r.string(),
            level,
        })
    }
}

// Java: gameserver/network/loginserverpackets/send/PlayerAuthRequest.java (LS reader: loginserver/.../receive/PlayerAuthRequest.java)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerAuthRequest {
    pub account: String,
    pub play_ok1: i32,
    pub play_ok2: i32,
    pub login_ok1: i32,
    pub login_ok2: i32,
}

impl PlayerAuthRequest {
    pub const OPCODE: u8 = 0x05;
}

impl Encode for PlayerAuthRequest {
    fn encode(&self, w: &mut PacketWriter) {
        w.u8(Self::OPCODE)
            .string(&self.account)
            .i32(self.play_ok1)
            .i32(self.play_ok2)
            .i32(self.login_ok1)
            .i32(self.login_ok2);
    }
}

impl Decode for PlayerAuthRequest {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            account: r.string(),
            play_ok1: r.i32()?,
            play_ok2: r.i32()?,
            login_ok1: r.i32()?,
            login_ok2: r.i32()?,
        })
    }
}

// Java: gameserver/network/loginserverpackets/send/ServerStatus.java (LS reader: loginserver/.../receive/ServerStatus.java)
/// A list of `(attribute id, value)` pairs. Use the consts below for both.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerStatus {
    pub attributes: Vec<(i32, i32)>,
}

impl ServerStatus {
    pub const OPCODE: u8 = 0x06;

    // Attribute ids. The LS reads these (Java `loginserver` ServerStatus).
    pub const SERVER_LIST_STATUS: i32 = 0x01;
    pub const SERVER_TYPE: i32 = 0x02;
    pub const SERVER_LIST_SQUARE_BRACKET: i32 = 0x03;
    pub const MAX_PLAYERS: i32 = 0x04;
    pub const TEST_SERVER: i32 = 0x05;
    /// NOTE(java-quirk): the GS writer (`gameserver` ServerStatus) sends age as 5, which is
    /// `TEST_SERVER` here. The LS reads 6 as age, so a GS age attribute is ignored by the LS.
    pub const SERVER_AGE: i32 = 0x06;

    // Values for `SERVER_LIST_STATUS`.
    pub const STATUS_AUTO: i32 = 0x00;
    pub const STATUS_GOOD: i32 = 0x01;
    pub const STATUS_NORMAL: i32 = 0x02;
    pub const STATUS_FULL: i32 = 0x03;
    pub const STATUS_DOWN: i32 = 0x04;
    pub const STATUS_GM_ONLY: i32 = 0x05;

    // Bit flags for `SERVER_TYPE`.
    pub const SERVER_NORMAL: i32 = 0x01;
    pub const SERVER_RELAX: i32 = 0x02;
    pub const SERVER_TEST: i32 = 0x04;
    pub const SERVER_NOLABEL: i32 = 0x08;
    pub const SERVER_CREATION_RESTRICTED: i32 = 0x10;
    pub const SERVER_EVENT: i32 = 0x20;
    pub const SERVER_FREE: i32 = 0x40;

    // Values for `SERVER_AGE`.
    pub const SERVER_AGE_ALL: i32 = 0x00;
    pub const SERVER_AGE_15: i32 = 0x0F;
    pub const SERVER_AGE_18: i32 = 0x12;

    // Values for `SERVER_LIST_SQUARE_BRACKET`.
    pub const OFF: i32 = 0x00;
    pub const ON: i32 = 0x01;
}

impl Encode for ServerStatus {
    fn encode(&self, w: &mut PacketWriter) {
        w.u8(Self::OPCODE).i32(self.attributes.len() as i32);
        for &(id, value) in &self.attributes {
            w.i32(id).i32(value);
        }
    }
}

impl Decode for ServerStatus {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let count = r.i32()?;
        // A negative count gives no pairs in Java (the loop never runs). No pre-allocation: a
        // huge count fails on the first missing pair.
        let mut attributes = Vec::new();
        for _ in 0..count.max(0) {
            let id = r.i32()?;
            let value = r.i32()?;
            attributes.push((id, value));
        }
        Ok(Self { attributes })
    }
}

// Java: gameserver/network/loginserverpackets/send/PlayerTracert.java (LS reader: loginserver/.../receive/PlayerTracert.java)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerTracert {
    pub account: String,
    pub pc_ip: String,
    pub hop1: String,
    pub hop2: String,
    pub hop3: String,
    pub hop4: String,
}

impl PlayerTracert {
    pub const OPCODE: u8 = 0x07;
}

impl Encode for PlayerTracert {
    fn encode(&self, w: &mut PacketWriter) {
        w.u8(Self::OPCODE)
            .string(&self.account)
            .string(&self.pc_ip)
            .string(&self.hop1)
            .string(&self.hop2)
            .string(&self.hop3)
            .string(&self.hop4);
    }
}

impl Decode for PlayerTracert {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            account: r.string(),
            pc_ip: r.string(),
            hop1: r.string(),
            hop2: r.string(),
            hop3: r.string(),
            hop4: r.string(),
        })
    }
}

// Java: gameserver/network/loginserverpackets/send/ReplyCharacters.java (LS reader: loginserver/.../receive/ReplyCharacters.java)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplyCharacters {
    pub account: String,
    /// One byte on the wire.
    pub chars: u8,
    /// Delete times in ms, one per character marked for deletion.
    pub delete_times: Vec<i64>,
}

impl ReplyCharacters {
    pub const OPCODE: u8 = 0x08;
}

impl Encode for ReplyCharacters {
    fn encode(&self, w: &mut PacketWriter) {
        // NOTE(java-quirk): Java `writeByte(size)`, so 256 or more delete times wrap.
        w.u8(Self::OPCODE)
            .string(&self.account)
            .u8(self.chars)
            .u8(self.delete_times.len() as u8);
        for &time in &self.delete_times {
            w.i64(time);
        }
    }
}

impl Decode for ReplyCharacters {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let account = r.string();
        let chars = r.u8()?;
        let count = r.u8()?;
        let mut delete_times = Vec::with_capacity(usize::from(count));
        for _ in 0..count {
            delete_times.push(r.i64()?);
        }
        Ok(Self {
            account,
            chars,
            delete_times,
        })
    }
}

// Java: gameserver/network/loginserverpackets/send/SendMail.java
/// Not handled by the LS: `GameServerPacketHandler` returns `null` for opcode 0x09 ("not
/// implemented"), so [`decode_from_gs`] returns `Ok(None)`. The struct exists for the GS writer
/// and for tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendMail {
    pub account: String,
    pub mail_id: String,
    pub args: Vec<String>,
}

impl SendMail {
    pub const OPCODE: u8 = 0x09;
}

impl Encode for SendMail {
    fn encode(&self, w: &mut PacketWriter) {
        // NOTE(java-quirk): Java `writeByte(args.length)`, so 256 or more args wrap.
        w.u8(Self::OPCODE)
            .string(&self.account)
            .string(&self.mail_id)
            .u8(self.args.len() as u8);
        for arg in &self.args {
            w.string(arg);
        }
    }
}

impl Decode for SendMail {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let account = r.string();
        let mail_id = r.string();
        let count = r.u8()?;
        let mut args = Vec::with_capacity(usize::from(count));
        for _ in 0..count {
            args.push(r.string());
        }
        Ok(Self {
            account,
            mail_id,
            args,
        })
    }
}

// Java: gameserver/network/loginserverpackets/send/TempBan.java (LS reader: loginserver/.../receive/RequestTempBan.java)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestTempBan {
    pub account: String,
    pub ip: String,
    /// Absolute time the ban ends, in ms since the epoch. Java's GS writer computes it as
    /// `now + minutes * 60000` (clamped to 0 minutes), so it is not a duration.
    pub ban_time: i64,
    /// Optional reason. The LS reads it and does not store it.
    pub reason: Option<String>,
}

impl RequestTempBan {
    pub const OPCODE: u8 = 0x0A;
}

impl Encode for RequestTempBan {
    fn encode(&self, w: &mut PacketWriter) {
        w.u8(Self::OPCODE)
            .string(&self.account)
            .string(&self.ip)
            .i64(self.ban_time)
            .bool(self.reason.is_some());
        if let Some(reason) = &self.reason {
            w.string(reason);
        }
    }
}

impl Decode for RequestTempBan {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let account = r.string();
        let ip = r.string();
        let ban_time = r.i64()?;
        let reason = if r.bool()? { Some(r.string()) } else { None };
        Ok(Self {
            account,
            ip,
            ban_time,
            reason,
        })
    }
}

// Java: gameserver/network/loginserverpackets/send/ChangePassword.java (LS reader: loginserver/.../receive/ChangePassword.java)
/// The only GS-to-LS packet whose Java reader keeps the opcode in a local (`packetId`, for the
/// log line). The layout is the same as the others: opcode, then four strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangePassword {
    pub account: String,
    pub character_name: String,
    pub old_pass: String,
    pub new_pass: String,
}

impl ChangePassword {
    pub const OPCODE: u8 = 0x0B;
}

impl Encode for ChangePassword {
    fn encode(&self, w: &mut PacketWriter) {
        w.u8(Self::OPCODE)
            .string(&self.account)
            .string(&self.character_name)
            .string(&self.old_pass)
            .string(&self.new_pass);
    }
}

impl Decode for ChangePassword {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            account: r.string(),
            character_name: r.string(),
            old_pass: r.string(),
            new_pass: r.string(),
        })
    }
}

// ---------------------------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------------------------

/// A decoded GS-to-LS packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GsPacket {
    BlowFishKey(BlowFishKey),
    GameServerAuth(GameServerAuth),
    PlayerInGame(PlayerInGame),
    PlayerLogout(PlayerLogout),
    ChangeAccessLevel(ChangeAccessLevel),
    PlayerAuthRequest(PlayerAuthRequest),
    ServerStatus(ServerStatus),
    PlayerTracert(PlayerTracert),
    ReplyCharacters(ReplyCharacters),
    RequestTempBan(RequestTempBan),
    ChangePassword(ChangePassword),
}

/// Decodes a GS-to-LS packet whose opcode was already read from `reader`.
///
/// Returns `Ok(None)` for `SendMail` in `Authed`: Java knows the opcode and does nothing, and it
/// does not close the link. Returns [`LinkError::UnknownOpcode`] for any other opcode in `state`,
/// where the caller must close the link with `NOT_AUTHED`.
pub fn decode_from_gs(
    state: LinkState,
    opcode: u8,
    reader: &mut PacketReader,
) -> Result<Option<GsPacket>, LinkError> {
    use LinkState::{Authed, BfConnected, Connected};
    let packet = match (state, opcode) {
        (Connected, BlowFishKey::OPCODE) => GsPacket::BlowFishKey(BlowFishKey::decode(reader)?),
        (BfConnected, GameServerAuth::OPCODE) => GsPacket::GameServerAuth(GameServerAuth::decode(reader)?),
        (Authed, PlayerInGame::OPCODE) => GsPacket::PlayerInGame(PlayerInGame::decode(reader)?),
        (Authed, PlayerLogout::OPCODE) => GsPacket::PlayerLogout(PlayerLogout::decode(reader)?),
        (Authed, ChangeAccessLevel::OPCODE) => {
            GsPacket::ChangeAccessLevel(ChangeAccessLevel::decode(reader)?)
        }
        (Authed, PlayerAuthRequest::OPCODE) => {
            GsPacket::PlayerAuthRequest(PlayerAuthRequest::decode(reader)?)
        }
        (Authed, ServerStatus::OPCODE) => GsPacket::ServerStatus(ServerStatus::decode(reader)?),
        (Authed, PlayerTracert::OPCODE) => GsPacket::PlayerTracert(PlayerTracert::decode(reader)?),
        (Authed, ReplyCharacters::OPCODE) => GsPacket::ReplyCharacters(ReplyCharacters::decode(reader)?),
        // Java: `case 0x09: return null; // RequestSendMail (not implemented)`.
        (Authed, SendMail::OPCODE) => return Ok(None),
        (Authed, RequestTempBan::OPCODE) => GsPacket::RequestTempBan(RequestTempBan::decode(reader)?),
        (Authed, ChangePassword::OPCODE) => GsPacket::ChangePassword(ChangePassword::decode(reader)?),
        _ => return Err(LinkError::UnknownOpcode { opcode, state }),
    };
    Ok(Some(packet))
}

/// A decoded LS-to-GS packet (read by the game server).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LsPacket {
    InitLS(InitLS),
    LoginServerFail(LoginServerFail),
    AuthResponse(AuthResponse),
    PlayerAuthResponse(PlayerAuthResponse),
    KickPlayer(KickPlayer),
    RequestCharacters(RequestCharacters),
    ChangePasswordResponse(ChangePasswordResponse),
}

/// Decodes an LS-to-GS packet whose opcode was already read. The game server has no state for
/// these, so there is no state argument (Java `LoginServerThread` switches on the opcode only).
pub fn decode_from_ls(opcode: u8, reader: &mut PacketReader) -> Result<LsPacket, PacketError> {
    Ok(match opcode {
        InitLS::OPCODE => LsPacket::InitLS(InitLS::decode(reader)?),
        LoginServerFail::OPCODE => LsPacket::LoginServerFail(LoginServerFail::decode(reader)?),
        AuthResponse::OPCODE => LsPacket::AuthResponse(AuthResponse::decode(reader)?),
        PlayerAuthResponse::OPCODE => LsPacket::PlayerAuthResponse(PlayerAuthResponse::decode(reader)?),
        KickPlayer::OPCODE => LsPacket::KickPlayer(KickPlayer::decode(reader)?),
        RequestCharacters::OPCODE => LsPacket::RequestCharacters(RequestCharacters::decode(reader)?),
        ChangePasswordResponse::OPCODE => {
            LsPacket::ChangePasswordResponse(ChangePasswordResponse::decode(reader)?)
        }
        _ => {
            return Err(PacketError::invalid(
                "opcode",
                format!("unknown LS-to-GS link opcode {opcode:#04x}"),
            ));
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use LinkState::{Authed, BfConnected, Connected};

    /// decode(encode(x)) == x, and the opcode is the first byte.
    fn round_trip<P: Encode + Decode + PartialEq + fmt::Debug>(packet: P, opcode: u8) {
        let bytes = packet.to_bytes();
        assert_eq!(bytes[0], opcode);
        let decoded = P::decode(&mut PacketReader::new(&bytes[1..])).unwrap();
        assert_eq!(decoded, packet);
    }

    fn text(s: &str) -> String {
        s.to_owned()
    }

    #[test]
    fn ls_to_gs_round_trip() {
        round_trip(InitLS::new(vec![0, 1, 2, 3]), InitLS::OPCODE);
        round_trip(
            LoginServerFail {
                reason: LoginServerFail::NOT_AUTHED,
            },
            LoginServerFail::OPCODE,
        );
        round_trip(
            AuthResponse {
                server_id: 2,
                server_name: text("Bartz"),
            },
            AuthResponse::OPCODE,
        );
        round_trip(
            PlayerAuthResponse {
                account: text("player1"),
                authed: true,
            },
            PlayerAuthResponse::OPCODE,
        );
        round_trip(KickPlayer { account: text("a") }, KickPlayer::OPCODE);
        round_trip(
            RequestCharacters {
                account: text("Привет"),
            },
            RequestCharacters::OPCODE,
        );
        round_trip(
            ChangePasswordResponse {
                character_name: text("Hero"),
                message: text("Password changed."),
            },
            ChangePasswordResponse::OPCODE,
        );
    }

    #[test]
    fn gs_to_ls_round_trip() {
        round_trip(
            BlowFishKey {
                encrypted_key: vec![0xAB; 64],
            },
            BlowFishKey::OPCODE,
        );
        round_trip(
            GameServerAuth {
                desired_id: 255,
                accept_alternate: true,
                reserve_host: false,
                port: u16::MAX,
                max_players: -1,
                hex_id: vec![1, 2, 3],
                subnets: vec![text("192.168.0.0/16"), text("10.0.0.0/8")],
                hosts: vec![text("192.168.1.2"), text("game.example")],
            },
            GameServerAuth::OPCODE,
        );
        round_trip(
            PlayerInGame {
                accounts: vec![text("a"), text("Привет")],
            },
            PlayerInGame::OPCODE,
        );
        round_trip(
            PlayerLogout {
                account: text("player1"),
            },
            PlayerLogout::OPCODE,
        );
        round_trip(
            ChangeAccessLevel {
                account: text("gm1"),
                level: -100,
            },
            ChangeAccessLevel::OPCODE,
        );
        round_trip(
            PlayerAuthRequest {
                account: text("player1"),
                play_ok1: 1,
                play_ok2: 2,
                login_ok1: 3,
                login_ok2: i32::MIN,
            },
            PlayerAuthRequest::OPCODE,
        );
        round_trip(
            ServerStatus {
                attributes: vec![
                    (ServerStatus::SERVER_LIST_STATUS, ServerStatus::STATUS_GOOD),
                    (ServerStatus::MAX_PLAYERS, 3000),
                ],
            },
            ServerStatus::OPCODE,
        );
        round_trip(
            PlayerTracert {
                account: text("a"),
                pc_ip: text("192.168.0.5"),
                hop1: text("10.0.0.1"),
                hop2: text("10.0.1.1"),
                hop3: String::new(),
                hop4: text("x"),
            },
            PlayerTracert::OPCODE,
        );
        round_trip(
            ReplyCharacters {
                account: text("player1"),
                chars: 7,
                delete_times: vec![1, i64::MAX, -1],
            },
            ReplyCharacters::OPCODE,
        );
        round_trip(
            SendMail {
                account: text("player1"),
                mail_id: text("welcome"),
                args: vec![text("a"), text("b")],
            },
            SendMail::OPCODE,
        );
        round_trip(
            RequestTempBan {
                account: text("player1"),
                ip: text("10.1.2.3"),
                ban_time: 1_700_000_300_000,
                reason: None,
            },
            RequestTempBan::OPCODE,
        );
        round_trip(
            RequestTempBan {
                account: text("player1"),
                ip: text("10.1.2.3"),
                ban_time: 1,
                reason: Some(text("spam")),
            },
            RequestTempBan::OPCODE,
        );
        round_trip(
            ChangePassword {
                account: text("acc"),
                character_name: text("Hero"),
                old_pass: text("old"),
                new_pass: text("new"),
            },
            ChangePassword::OPCODE,
        );
    }

    #[test]
    fn opcodes_match_java() {
        // Java `OPCODE` constants, LS side and GS side.
        let ls = [
            InitLS::OPCODE,
            LoginServerFail::OPCODE,
            AuthResponse::OPCODE,
            PlayerAuthResponse::OPCODE,
            KickPlayer::OPCODE,
            RequestCharacters::OPCODE,
            ChangePasswordResponse::OPCODE,
        ];
        assert_eq!(ls, [0, 1, 2, 3, 4, 5, 6]);
        let gs = [
            BlowFishKey::OPCODE,
            GameServerAuth::OPCODE,
            PlayerInGame::OPCODE,
            PlayerLogout::OPCODE,
            ChangeAccessLevel::OPCODE,
            PlayerAuthRequest::OPCODE,
            ServerStatus::OPCODE,
            PlayerTracert::OPCODE,
            ReplyCharacters::OPCODE,
            SendMail::OPCODE,
            RequestTempBan::OPCODE,
            ChangePassword::OPCODE,
        ];
        assert_eq!(gs, [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]);
    }

    #[test]
    fn change_access_level_writes_level_before_account() {
        let bytes = ChangeAccessLevel {
            account: text("gm1"),
            level: 100,
        }
        .to_bytes();
        assert_eq!(bytes, [0x04, 0x64, 0, 0, 0, b'g', 0, b'm', 0, b'1', 0, 0, 0]);
    }

    #[test]
    fn game_server_auth_writes_min_of_both_lists() {
        let auth = GameServerAuth {
            desired_id: 4,
            accept_alternate: false,
            reserve_host: true,
            port: 7777,
            max_players: 100,
            hex_id: vec![0xFB],
            subnets: vec![text("a"), text("b"), text("c")],
            hosts: vec![text("x")],
        };
        let decoded = GameServerAuth::decode(&mut PacketReader::new(&auth.to_bytes()[1..])).unwrap();
        assert_eq!(decoded.subnets, vec![text("a")]);
        assert_eq!(decoded.hosts, vec![text("x")]);
    }

    #[test]
    fn game_server_auth_limits_are_errors() {
        // hex id length 65 (over 64).
        let mut w = PacketWriter::new();
        w.u8(1).u8(1).u8(0).u16(7777).i32(5000).i32(65);
        w.bytes(&[0; 65]).i32(0);
        assert!(GameServerAuth::decode(&mut PacketReader::new(w.as_bytes())).is_err());

        // negative hex id length.
        let mut w = PacketWriter::new();
        w.u8(1).u8(1).u8(0).u16(7777).i32(5000).i32(-1);
        assert!(GameServerAuth::decode(&mut PacketReader::new(w.as_bytes())).is_err());

        // 33 host pairs (over 32).
        let mut w = PacketWriter::new();
        w.u8(1).u8(1).u8(0).u16(7777).i32(5000).i32(0).i32(33);
        assert!(GameServerAuth::decode(&mut PacketReader::new(w.as_bytes())).is_err());

        // The limits themselves are allowed.
        let max = GameServerAuth {
            desired_id: 1,
            accept_alternate: false,
            reserve_host: false,
            port: 7777,
            max_players: 5000,
            hex_id: vec![7; 64],
            subnets: vec![text("s"); 32],
            hosts: vec![text("h"); 32],
        };
        round_trip(max, GameServerAuth::OPCODE);
    }

    #[test]
    fn negative_lengths_are_errors() {
        // InitLS with a negative key length.
        let bytes = [0, 0x06, 0x01, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF];
        assert!(InitLS::decode(&mut PacketReader::new(&bytes[1..])).is_err());
    }

    #[test]
    fn blow_fish_key_empty_block_is_not_an_error() {
        // Java: size <= 0 logs and returns, the connection stays CONNECTED.
        for size in [0i32, -3] {
            let mut w = PacketWriter::new();
            w.u8(0).i32(size);
            let decoded =
                decode_from_gs(Connected, 0x00, &mut PacketReader::new(&w.as_bytes()[1..])).unwrap();
            assert_eq!(
                decoded,
                Some(GsPacket::BlowFishKey(BlowFishKey {
                    encrypted_key: vec![]
                }))
            );
        }
    }

    #[test]
    fn player_in_game_short_list_is_an_error() {
        // Count 2, but only one name follows.
        let mut w = PacketWriter::new();
        w.u16(2).string("a");
        assert!(PlayerInGame::decode(&mut PacketReader::new(w.as_bytes())).is_err());
    }

    #[test]
    fn player_logout_needs_two_bytes() {
        assert!(PlayerLogout::decode(&mut PacketReader::new(&[])).is_err());
        assert!(PlayerLogout::decode(&mut PacketReader::new(&[0])).is_err());
        let ok = PlayerLogout::decode(&mut PacketReader::new(&[0, 0])).unwrap();
        assert_eq!(ok.account, "");
    }

    #[test]
    fn server_status_negative_count_is_empty() {
        let bytes = [0xFF, 0xFF, 0xFF, 0xFF];
        let decoded = ServerStatus::decode(&mut PacketReader::new(&bytes)).unwrap();
        assert!(decoded.attributes.is_empty());
    }

    #[test]
    fn reply_characters_truncated_timers_are_an_error() {
        let mut w = PacketWriter::new();
        w.string("a").u8(1).u8(2).i64(5);
        assert!(ReplyCharacters::decode(&mut PacketReader::new(w.as_bytes())).is_err());
    }

    #[test]
    fn decode_from_gs_allowed_opcodes_per_state() {
        // (state, opcode) pairs Java accepts, with a body that decodes for each.
        let cases: [(LinkState, u8, Vec<u8>); 3] = [
            (
                Connected,
                0x00,
                BlowFishKey {
                    encrypted_key: vec![1; 8],
                }
                .to_bytes(),
            ),
            (
                BfConnected,
                0x01,
                GameServerAuth {
                    desired_id: 1,
                    accept_alternate: false,
                    reserve_host: false,
                    port: 1,
                    max_players: 1,
                    hex_id: vec![],
                    subnets: vec![],
                    hosts: vec![],
                }
                .to_bytes(),
            ),
            (Authed, 0x03, PlayerLogout { account: text("a") }.to_bytes()),
        ];
        for (state, opcode, bytes) in cases {
            let decoded = decode_from_gs(state, opcode, &mut PacketReader::new(&bytes[1..])).unwrap();
            assert!(decoded.is_some(), "opcode {opcode:#04x} in {state:?}");
        }
    }

    /// Every opcode 0..=255 in every state. Only the Java pairs decode; SendMail is `Ok(None)`;
    /// everything else is an `UnknownOpcode` error, and the body is not read.
    #[test]
    fn decode_from_gs_every_opcode_in_every_state() {
        let allowed = |state: LinkState, opcode: u8| match state {
            Connected => opcode == 0x00,
            BfConnected => opcode == 0x01,
            Authed => (0x02..=0x0B).contains(&opcode),
        };
        // A body that is valid for any opcode: empty strings and zero counts.
        let body = [0u8; 64];
        for state in [Connected, BfConnected, Authed] {
            for opcode in 0..=u8::MAX {
                let mut reader = PacketReader::new(&body);
                let result = decode_from_gs(state, opcode, &mut reader);
                if allowed(state, opcode) {
                    if opcode == SendMail::OPCODE {
                        assert_eq!(result, Ok(None), "opcode {opcode:#04x} in {state:?}");
                    } else {
                        assert!(
                            matches!(result, Ok(Some(_)) | Err(LinkError::Packet(_))),
                            "opcode {opcode:#04x} in {state:?}: {result:?}"
                        );
                    }
                } else {
                    assert_eq!(
                        result,
                        Err(LinkError::UnknownOpcode { opcode, state }),
                        "opcode {opcode:#04x} in {state:?}"
                    );
                    assert_eq!(reader.remaining(), body.len(), "body must not be read");
                }
            }
        }
    }

    #[test]
    fn decode_from_gs_unknown_opcode_asks_for_not_authed_close() {
        let err = decode_from_gs(Connected, 0x01, &mut PacketReader::new(&[])).unwrap_err();
        assert_eq!(
            err,
            LinkError::UnknownOpcode {
                opcode: 0x01,
                state: Connected
            }
        );
        assert!(err.to_string().contains("0x01"));
    }

    #[test]
    fn decode_from_gs_pairs_each_packet_with_its_opcode() {
        let auth = GameServerAuth {
            desired_id: 2,
            accept_alternate: true,
            reserve_host: false,
            port: 9,
            max_players: 10,
            hex_id: vec![5],
            subnets: vec![text("s")],
            hosts: vec![text("h")],
        };
        let decoded =
            decode_from_gs(BfConnected, 0x01, &mut PacketReader::new(&auth.to_bytes()[1..])).unwrap();
        assert_eq!(decoded, Some(GsPacket::GameServerAuth(auth)));

        let body = ChangePassword {
            account: text("acc"),
            character_name: text("Hero"),
            old_pass: text("old"),
            new_pass: text("new"),
        }
        .to_bytes();
        let decoded = decode_from_gs(Authed, 0x0B, &mut PacketReader::new(&body[1..])).unwrap();
        assert!(matches!(decoded, Some(GsPacket::ChangePassword(_))));
    }

    #[test]
    fn decode_from_gs_send_mail_is_ignored_without_reading() {
        let mut reader = PacketReader::new(&[1, 2, 3]);
        assert_eq!(decode_from_gs(Authed, 0x09, &mut reader), Ok(None));
        assert_eq!(reader.remaining(), 3);
    }

    #[test]
    fn decode_from_ls_reads_each_opcode_and_rejects_unknown() {
        let bytes = AuthResponse {
            server_id: 1,
            server_name: text("Bartz"),
        }
        .to_bytes();
        assert_eq!(
            decode_from_ls(0x02, &mut PacketReader::new(&bytes[1..])).unwrap(),
            LsPacket::AuthResponse(AuthResponse {
                server_id: 1,
                server_name: text("Bartz"),
            })
        );
        assert!(decode_from_ls(0x07, &mut PacketReader::new(&[])).is_err());
        assert!(decode_from_ls(0x0B, &mut PacketReader::new(&[])).is_err());
    }
}
