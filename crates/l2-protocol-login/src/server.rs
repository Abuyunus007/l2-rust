//! Packets the login server sends to the game client.
//!
//! Every packet has the same shape (see the pattern in `l2_net::packet`): one struct, `OPCODE`,
//! `impl Encode` (opcode + fields, as Java `writeImpl`) and `impl Decode` (fields after the
//! opcode). `Decode` exists too so the proxy and test clients can read what the server sends.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use crate::enums::{AccountKickedReason, LoginFailReason, PlayFailReason};

/// Length of the scrambled RSA modulus in `Init`.
pub const SCRAMBLED_MODULUS_LEN: usize = 128;
/// Length of the per-client Blowfish key in `Init`.
pub const BLOWFISH_KEY_LEN: usize = 16;

// Java: loginserver/network/serverpackets/Init.java
/// First packet of every connection, sent encrypted with the static Blowfish key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Init {
    pub session_id: i32,
    pub protocol: u32,
    pub scrambled_modulus: [u8; SCRAMBLED_MODULUS_LEN],
    /// Four "GG related" ints the client ignores. Java always sends [`Init::GG`].
    pub gg: [u32; 4],
    pub blowfish_key: [u8; BLOWFISH_KEY_LEN],
}

impl Init {
    pub const OPCODE: u8 = 0x00;
    /// Protocol revision Java writes (`0x0000c621`).
    pub const PROTOCOL_REVISION: u32 = 0x0000_C621;
    /// The GG values Java writes (`0x29DD954E, 0x77C39CFC, 0x97ADB620, 0x07BDE0F7`).
    pub const GG: [u32; 4] = [0x29DD_954E, 0x77C3_9CFC, 0x97AD_B620, 0x07BD_E0F7];

    /// An `Init` exactly as Java builds it: fixed protocol revision and GG values.
    pub fn new(
        session_id: i32,
        scrambled_modulus: [u8; SCRAMBLED_MODULUS_LEN],
        blowfish_key: [u8; BLOWFISH_KEY_LEN],
    ) -> Self {
        Self {
            session_id,
            protocol: Self::PROTOCOL_REVISION,
            scrambled_modulus,
            gg: Self::GG,
            blowfish_key,
        }
    }
}

impl Encode for Init {
    fn encode(&self, w: &mut PacketWriter) {
        w.u8(Self::OPCODE).i32(self.session_id).u32(self.protocol);
        w.bytes(&self.scrambled_modulus);
        for value in self.gg {
            w.u32(value);
        }
        // Java ends the packet with a 0 byte ("null termination").
        w.bytes(&self.blowfish_key).u8(0);
    }
}

impl Decode for Init {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let session_id = r.i32()?;
        let protocol = r.u32()?;
        let scrambled_modulus = r.byte_array()?;
        let gg = [r.u32()?, r.u32()?, r.u32()?, r.u32()?];
        let blowfish_key = r.byte_array()?;
        r.skip(1)?; // the terminating 0
        Ok(Self {
            session_id,
            protocol,
            scrambled_modulus,
            gg,
            blowfish_key,
        })
    }
}

// Java: loginserver/network/serverpackets/LoginFail.java
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginFail {
    pub reason: LoginFailReason,
}

impl LoginFail {
    pub const OPCODE: u8 = 0x01;
}

impl Encode for LoginFail {
    fn encode(&self, w: &mut PacketWriter) {
        w.u8(Self::OPCODE).u8(self.reason.code());
    }
}

impl Decode for LoginFail {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            reason: LoginFailReason::try_from(r.u8()?)?,
        })
    }
}

// Java: loginserver/network/serverpackets/AccountKicked.java
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountKicked {
    pub reason: AccountKickedReason,
}

impl AccountKicked {
    pub const OPCODE: u8 = 0x02;
}

impl Encode for AccountKicked {
    fn encode(&self, w: &mut PacketWriter) {
        // Unlike LoginFail/PlayFail, the reason is a 32-bit int.
        w.u8(Self::OPCODE).u32(self.reason.code());
    }
}

impl Decode for AccountKicked {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            reason: AccountKickedReason::try_from(r.u32()?)?,
        })
    }
}

// Java: loginserver/network/serverpackets/LoginOk.java
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginOk {
    pub login_ok1: i32,
    pub login_ok2: i32,
}

impl LoginOk {
    pub const OPCODE: u8 = 0x03;
    /// The one non-zero reserved int (`RESERVED_VALUE_3EA`), fifth int after the opcode.
    pub const RESERVED_3EA: i32 = 0x3EA;
    /// Reserved bytes after the two keys: 2 zero ints, `RESERVED_3EA`, 3 zero ints, 16 zero bytes.
    const RESERVED_LEN: usize = 4 * 6 + 16;
}

impl Encode for LoginOk {
    fn encode(&self, w: &mut PacketWriter) {
        w.u8(Self::OPCODE).i32(self.login_ok1).i32(self.login_ok2);
        w.i32(0).i32(0).i32(Self::RESERVED_3EA).i32(0).i32(0).i32(0);
        w.bytes(&[0; 16]);
    }
}

impl Decode for LoginOk {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let packet = Self {
            login_ok1: r.i32()?,
            login_ok2: r.i32()?,
        };
        r.skip(Self::RESERVED_LEN)?; // reserved block, ignored
        Ok(packet)
    }
}

/// One game server in a `ServerList`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerEntry {
    pub server_id: u8,
    pub ip: [u8; 4],
    pub port: i32,
    pub age_limit: u8,
    pub pvp: bool,
    pub current_players: u16,
    pub max_players: u16,
    /// Java writes `status == STATUS_DOWN ? 0 : 1`; the other status values are not sent.
    pub up: bool,
    /// Bit flags: 1 normal, 2 relax, 4 public test, 8 no label, 16 character creation
    /// restricted, 32 event, 64 free.
    pub server_type: i32,
    pub brackets: bool,
}

impl ServerEntry {
    // Not a packet, so no opcode and no `Encode`: sub-structures use plain `write` / `read`.
    fn write(&self, w: &mut PacketWriter) {
        w.u8(self.server_id).bytes(&self.ip).i32(self.port);
        w.u8(self.age_limit).bool(self.pvp);
        w.u16(self.current_players).u16(self.max_players);
        w.bool(self.up).i32(self.server_type).bool(self.brackets);
    }

    fn read(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            server_id: r.u8()?,
            ip: r.byte_array()?,
            port: r.i32()?,
            age_limit: r.u8()?,
            pvp: r.bool()?,
            current_players: r.u16()?,
            max_players: r.u16()?,
            up: r.bool()?,
            server_type: r.i32()?,
            brackets: r.bool()?,
        })
    }
}

/// Characters an account has on one game server, from the `ReplyCharacters` link packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerCharInfo {
    pub server_id: u8,
    pub chars: u8,
    /// Seconds until each character marked for deletion is deleted. Java computes
    /// `(deleteTime - now) / 1000` while writing, so callers pass the already-computed values.
    pub seconds_until_delete: Vec<i32>,
}

impl ServerCharInfo {
    fn write(&self, w: &mut PacketWriter) {
        w.u8(self.server_id).u8(self.chars);
        // NOTE(java-quirk): Java writes the count with `writeByte`, so it wraps at 256.
        w.u8(self.seconds_until_delete.len() as u8);
        for &seconds in &self.seconds_until_delete {
            w.i32(seconds);
        }
    }

    fn read(r: &mut PacketReader) -> Result<Self, PacketError> {
        let server_id = r.u8()?;
        let chars = r.u8()?;
        let count = r.u8()?;
        let mut seconds_until_delete = Vec::with_capacity(usize::from(count));
        for _ in 0..count {
            seconds_until_delete.push(r.i32()?);
        }
        Ok(Self {
            server_id,
            chars,
            seconds_until_delete,
        })
    }
}

// Java: loginserver/network/serverpackets/ServerList.java
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerList {
    pub last_server: u8,
    pub servers: Vec<ServerEntry>,
    /// `None` is Java's `_charsOnServers == null` (no `ReplyCharacters` arrived in time).
    /// Both `None` and `Some(vec![])` write a single `0` byte, so decoding gives `None`.
    pub char_info: Option<Vec<ServerCharInfo>>,
}

impl ServerList {
    pub const OPCODE: u8 = 0x04;
}

impl Encode for ServerList {
    fn encode(&self, w: &mut PacketWriter) {
        // NOTE(java-quirk): all counts are `writeByte(size)` in Java, so they wrap at 256.
        w.u8(Self::OPCODE)
            .u8(self.servers.len() as u8)
            .u8(self.last_server);
        for server in &self.servers {
            server.write(w);
        }
        w.u16(0); // unknown in Java, always 0
        match &self.char_info {
            Some(infos) => {
                w.u8(infos.len() as u8);
                for info in infos {
                    info.write(w);
                }
            }
            None => {
                w.u8(0);
            }
        }
    }
}

impl Decode for ServerList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let server_count = r.u8()?;
        let last_server = r.u8()?;
        let mut servers = Vec::with_capacity(usize::from(server_count));
        for _ in 0..server_count {
            servers.push(ServerEntry::read(r)?);
        }
        r.skip(2)?; // the unknown u16
        let info_count = r.u8()?;
        let char_info = if info_count == 0 {
            None
        } else {
            let mut infos = Vec::with_capacity(usize::from(info_count));
            for _ in 0..info_count {
                infos.push(ServerCharInfo::read(r)?);
            }
            Some(infos)
        };
        Ok(Self {
            last_server,
            servers,
            char_info,
        })
    }
}

// Java: loginserver/network/serverpackets/PlayFail.java
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayFail {
    pub reason: PlayFailReason,
}

impl PlayFail {
    pub const OPCODE: u8 = 0x06;
}

impl Encode for PlayFail {
    fn encode(&self, w: &mut PacketWriter) {
        w.u8(Self::OPCODE).u8(self.reason.code());
    }
}

impl Decode for PlayFail {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            reason: PlayFailReason::try_from(r.u8()?)?,
        })
    }
}

// Java: loginserver/network/serverpackets/PlayOk.java
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayOk {
    pub play_ok1: i32,
    pub play_ok2: i32,
}

impl PlayOk {
    pub const OPCODE: u8 = 0x07;
}

impl Encode for PlayOk {
    fn encode(&self, w: &mut PacketWriter) {
        w.u8(Self::OPCODE).i32(self.play_ok1).i32(self.play_ok2);
    }
}

impl Decode for PlayOk {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            play_ok1: r.i32()?,
            play_ok2: r.i32()?,
        })
    }
}

// Java: loginserver/network/serverpackets/GGAuth.java
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GGAuth {
    pub response: i32,
}

impl GGAuth {
    pub const OPCODE: u8 = 0x0B;
}

impl Encode for GGAuth {
    fn encode(&self, w: &mut PacketWriter) {
        w.u8(Self::OPCODE).i32(self.response);
        w.i32(0).i32(0).i32(0).i32(0); // four reserved ints
    }
}

impl Decode for GGAuth {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let response = r.i32()?;
        r.skip(16)?; // four reserved ints
        Ok(Self { response })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// decode(encode(x)) == x, and the opcode is the first byte.
    fn round_trip<P: Encode + Decode + PartialEq + std::fmt::Debug>(packet: P, opcode: u8) {
        let bytes = packet.to_bytes();
        assert_eq!(bytes[0], opcode);
        let decoded = P::decode(&mut PacketReader::new(&bytes[1..])).unwrap();
        assert_eq!(decoded, packet);
    }

    fn entry(server_id: u8) -> ServerEntry {
        ServerEntry {
            server_id,
            ip: [10, 0, 0, server_id],
            port: 7777,
            age_limit: 15,
            pvp: true,
            current_players: 65_535,
            max_players: 3000,
            up: true,
            server_type: 0x41,
            brackets: true,
        }
    }

    #[test]
    fn init_round_trip_and_constants() {
        let init = Init::new(-5, [7; 128], [9; 16]);
        assert_eq!(init.protocol, 50_721);
        round_trip(init.clone(), Init::OPCODE);
        let bytes = init.to_bytes();
        assert_eq!(bytes.len(), 1 + 4 + 4 + 128 + 16 + 16 + 1);
        assert_eq!(bytes[9 + 128..9 + 128 + 4], 0x29DD_954E_u32.to_le_bytes());
    }

    #[test]
    fn init_decode_needs_the_whole_packet() {
        let bytes = Init::new(1, [0; 128], [0; 16]).to_bytes();
        for cut in [1, 2, 10, 17, 18] {
            let short = &bytes[1..bytes.len() - cut];
            assert!(Init::decode(&mut PacketReader::new(short)).is_err(), "cut {cut}");
        }
    }

    #[test]
    fn reason_packets_round_trip() {
        for &reason in LoginFailReason::ALL {
            round_trip(LoginFail { reason }, LoginFail::OPCODE);
        }
        for &reason in PlayFailReason::ALL {
            round_trip(PlayFail { reason }, PlayFail::OPCODE);
        }
        for &reason in AccountKickedReason::ALL {
            round_trip(AccountKicked { reason }, AccountKicked::OPCODE);
        }
    }

    #[test]
    fn unknown_reason_is_an_error() {
        assert!(LoginFail::decode(&mut PacketReader::new(&[0x03])).is_err());
        assert!(PlayFail::decode(&mut PacketReader::new(&[0x06])).is_err());
        assert!(AccountKicked::decode(&mut PacketReader::new(&[0x02, 0, 0, 0])).is_err());
    }

    #[test]
    fn fixed_packets_round_trip() {
        round_trip(
            LoginOk {
                login_ok1: -1,
                login_ok2: i32::MAX,
            },
            LoginOk::OPCODE,
        );
        round_trip(
            PlayOk {
                play_ok1: i32::MIN,
                play_ok2: 3,
            },
            PlayOk::OPCODE,
        );
        round_trip(GGAuth { response: -7 }, GGAuth::OPCODE);
    }

    #[test]
    fn login_ok_layout() {
        let bytes = LoginOk {
            login_ok1: 1,
            login_ok2: 2,
        }
        .to_bytes();
        let mut expected = vec![0x03, 1, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        expected.extend([0xEA, 0x03, 0, 0]); // 0x3EA
        expected.extend([0; 12 + 16]);
        assert_eq!(bytes, expected);
    }

    #[test]
    fn server_list_round_trip() {
        let list = ServerList {
            last_server: 2,
            servers: vec![
                entry(1),
                ServerEntry {
                    up: false,
                    ..entry(2)
                },
            ],
            char_info: Some(vec![
                ServerCharInfo {
                    server_id: 2,
                    chars: 1,
                    seconds_until_delete: vec![],
                },
                ServerCharInfo {
                    server_id: 1,
                    chars: 3,
                    seconds_until_delete: vec![3600, -5],
                },
            ]),
        };
        round_trip(list, ServerList::OPCODE);
    }

    #[test]
    fn server_list_delete_timers_layout() {
        let list = ServerList {
            last_server: 1,
            servers: vec![],
            char_info: Some(vec![ServerCharInfo {
                server_id: 1,
                chars: 2,
                seconds_until_delete: vec![0x0102, -1],
            }]),
        };
        let expected = [
            0x04, 0, 1, // opcode, server count, last server
            0, 0, // unknown u16
            1, // one char entry
            1, 2, 2, // server id, chars, timer count
            0x02, 0x01, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF,
        ];
        assert_eq!(list.to_bytes(), expected);
    }

    #[test]
    fn server_list_without_char_info_decodes_as_none() {
        let none = ServerList {
            last_server: 0,
            servers: vec![],
            char_info: None,
        };
        let empty = ServerList {
            char_info: Some(vec![]),
            ..none.clone()
        };
        assert_eq!(none.to_bytes(), empty.to_bytes());
        round_trip(none, ServerList::OPCODE);
        // Java cannot tell them apart on the wire either.
        let decoded = ServerList::decode(&mut PacketReader::new(&empty.to_bytes()[1..])).unwrap();
        assert_eq!(decoded.char_info, None);
    }

    #[test]
    fn server_list_truncated_is_an_error() {
        let list = ServerList {
            last_server: 1,
            servers: vec![entry(1)],
            char_info: None,
        };
        let bytes = list.to_bytes();
        for len in 1..bytes.len() {
            assert!(
                ServerList::decode(&mut PacketReader::new(&bytes[1..len])).is_err(),
                "len {len}"
            );
        }
    }
}
