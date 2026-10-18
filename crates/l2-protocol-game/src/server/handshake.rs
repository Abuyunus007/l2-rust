//! Server packets of the first connection steps.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_opcode;
use crate::generated::opcodes::ServerOpcode;

/// Length of the Blowfish key the server announces in [`KeyPacket`].
pub const KEY_LEN: usize = 8;

// Java: gameserver/network/serverpackets/KeyPacket.java
/// The answer to `ProtocolVersion` (opcode name `VERSION_CHECK`). It is the last packet that goes
/// out unencrypted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyPacket {
    /// [`Self::RESULT_OK`] or [`Self::RESULT_WRONG_PROTOCOL`].
    pub result: u8,
    /// First 8 bytes of the key for this connection (Java writes `_key[0..8]`).
    pub key: [u8; KEY_LEN],
    /// `ServerConfig.PACKET_ENCRYPTION`, written as an int.
    pub use_encryption: bool,
    /// `ServerConfig.SERVER_ID`.
    pub server_id: i32,
}

impl KeyPacket {
    pub const OPCODE: ServerOpcode = ServerOpcode::VersionCheck;
    pub const RESULT_WRONG_PROTOCOL: u8 = 0;
    pub const RESULT_OK: u8 = 1;
}

impl Encode for KeyPacket {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.u8(self.result).bytes(&self.key);
        w.i32(i32::from(self.use_encryption)).i32(self.server_id);
        w.u8(1); // Java: constant 1
        w.i32(0); // Java: "obfuscation key", always 0
    }
}

impl Decode for KeyPacket {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let result = r.u8()?;
        let key = r.byte_array()?;
        let use_encryption = r.i32()? != 0;
        let server_id = r.i32()?;
        r.skip(1 + 4)?; // the constant byte and the obfuscation key
        Ok(Self {
            result,
            key,
            use_encryption,
            server_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_packet_layout() {
        let packet = KeyPacket {
            result: KeyPacket::RESULT_OK,
            key: [1, 2, 3, 4, 5, 6, 7, 8],
            use_encryption: true,
            server_id: 2,
        };
        let bytes = packet.to_bytes();
        // 0x2E, result, 8 key bytes, int 1, int 2, byte 1, int 0.
        let expected = [
            0x2E, 1, 1, 2, 3, 4, 5, 6, 7, 8, 1, 0, 0, 0, 2, 0, 0, 0, 1, 0, 0, 0, 0,
        ];
        assert_eq!(bytes, expected);
        assert_eq!(
            KeyPacket::decode(&mut PacketReader::new(&bytes[1..])).unwrap(),
            packet
        );
    }

    #[test]
    fn wrong_protocol_answer() {
        let packet = KeyPacket {
            result: KeyPacket::RESULT_WRONG_PROTOCOL,
            key: [0; KEY_LEN],
            use_encryption: false,
            server_id: 1,
        };
        let bytes = packet.to_bytes();
        assert_eq!(bytes[1], 0);
        assert_eq!(bytes[10..14], [0, 0, 0, 0]);
        assert!(KeyPacket::decode(&mut PacketReader::new(&bytes[1..bytes.len() - 1])).is_err());
    }
}
