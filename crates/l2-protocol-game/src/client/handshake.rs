//! Client packets of the first connection steps: protocol check and game server login.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_opcode;
use crate::generated::opcodes::ClientOpcode;

// Java: gameserver/network/clientpackets/ProtocolVersion.java
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtocolVersion {
    /// `-2` is the "ping" of the new C2 client, which Java answers by disconnecting.
    pub version: i32,
}

impl ProtocolVersion {
    pub const OPCODE: ClientOpcode = ClientOpcode::ProtocolVersion;
}

impl Encode for ProtocolVersion {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.version);
    }
}

impl Decode for ProtocolVersion {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        // NOTE(java-quirk): Java wraps `readInt()` in a try/catch and uses version 0 when the
        // packet is too short, so this packet never fails to decode. Version 0 is then rejected
        // by the protocol list like any other wrong version.
        Ok(Self {
            version: r.i32().unwrap_or(0),
        })
    }
}

// Java: gameserver/network/clientpackets/AuthLogin.java
/// Session keys from the login server. The game server compares them with what the login server
/// reported for this account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthLogin {
    /// Already lower-cased, as Java stores it.
    pub login_name: String,
    // The keys are in wire order. Note that play key 2 comes before play key 1.
    pub play_key2: i32,
    pub play_key1: i32,
    pub login_key1: i32,
    pub login_key2: i32,
}

impl AuthLogin {
    pub const OPCODE: ClientOpcode = ClientOpcode::AuthLogin;
}

impl Encode for AuthLogin {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.string(&self.login_name)
            .i32(self.play_key2)
            .i32(self.play_key1)
            .i32(self.login_key1)
            .i32(self.login_key2);
    }
}

impl Decode for AuthLogin {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        // Java: `readString().toLowerCase()`.
        let login_name = r.string().to_lowercase();
        Ok(Self {
            login_name,
            play_key2: r.i32()?,
            play_key1: r.i32()?,
            login_key1: r.i32()?,
            login_key2: r.i32()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_version_layout() {
        // 0x0E + int 273 (0x0111)
        let bytes = ProtocolVersion { version: 273 }.to_bytes();
        assert_eq!(bytes, [0x0E, 0x11, 0x01, 0x00, 0x00]);
        let decoded = ProtocolVersion::decode(&mut PacketReader::new(&bytes[1..])).unwrap();
        assert_eq!(decoded.version, 273);
    }

    #[test]
    fn protocol_version_short_packet_is_version_zero() {
        for body in [&[][..], &[1, 2, 3][..]] {
            let decoded = ProtocolVersion::decode(&mut PacketReader::new(body)).unwrap();
            assert_eq!(decoded.version, 0);
        }
        let ping = ProtocolVersion::decode(&mut PacketReader::new(&[0xFE, 0xFF, 0xFF, 0xFF])).unwrap();
        assert_eq!(ping.version, -2);
    }

    #[test]
    fn auth_login_layout_and_lowercase() {
        // "AbC" as UTF-16LE + terminator, then four ints in wire order.
        let mut body = vec![0x41, 0, 0x62, 0, 0x43, 0, 0, 0];
        for key in [2_i32, 1, 3, 4] {
            body.extend(key.to_le_bytes());
        }
        let decoded = AuthLogin::decode(&mut PacketReader::new(&body)).unwrap();
        assert_eq!(
            decoded,
            AuthLogin {
                login_name: "abc".into(),
                play_key2: 2,
                play_key1: 1,
                login_key1: 3,
                login_key2: 4,
            }
        );
        let encoded = decoded.to_bytes();
        assert_eq!(encoded[0], 0x2B);
        assert_eq!(
            encoded[1..],
            [
                0x61, 0, 0x62, 0, 0x63, 0, 0, 0, 2, 0, 0, 0, 1, 0, 0, 0, 3, 0, 0, 0, 4, 0, 0, 0
            ]
        );
    }

    #[test]
    fn auth_login_truncated() {
        let bytes = AuthLogin {
            login_name: "x".into(),
            play_key2: 0,
            play_key1: 0,
            login_key1: 0,
            login_key2: 0,
        }
        .to_bytes();
        assert!(AuthLogin::decode(&mut PacketReader::new(&bytes[1..bytes.len() - 1])).is_err());
    }
}
