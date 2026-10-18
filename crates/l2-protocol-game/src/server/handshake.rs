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

// Java: gameserver/network/serverpackets/LoginFail.java
/// The login server refused the session key of `AuthLogin` (sent right before the connection is
/// closed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoginFail {
    /// One of the `REASON_*` / Java constants; a plain int on the wire.
    pub reason: i32,
}

impl LoginFail {
    pub const OPCODE: ServerOpcode = ServerOpcode::LoginFail;
    pub const NO_TEXT: i32 = 0;
    /// "System error. Please log in again later."
    pub const SYSTEM_ERROR_LOGIN_LATER: i32 = 1;
    pub const PASSWORD_DOES_NOT_MATCH_THIS_ACCOUNT: i32 = 2;
    pub const PASSWORD_DOES_NOT_MATCH_THIS_ACCOUNT2: i32 = 3;
    pub const ACCESS_FAILED_TRY_LATER: i32 = 4;
    pub const INCORRECT_ACCOUNT_INFO_CONTACT_CUSTOMER_SUPPORT: i32 = 5;
    pub const ACCESS_FAILED_TRY_LATER2: i32 = 6;
    pub const ACOUNT_ALREADY_IN_USE: i32 = 7;
    pub const ACCESS_FAILED_TRY_LATER3: i32 = 8;
    pub const ACCESS_FAILED_TRY_LATER4: i32 = 9;
    pub const ACCESS_FAILED_TRY_LATER5: i32 = 10;
}

impl Encode for LoginFail {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.reason);
    }
}

impl Decode for LoginFail {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { reason: r.i32()? })
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
    fn login_fail_layout() {
        let packet = LoginFail {
            reason: LoginFail::SYSTEM_ERROR_LOGIN_LATER,
        };
        let bytes = packet.to_bytes();
        assert_eq!(bytes, [0x0A, 1, 0, 0, 0]);
        assert_eq!(
            LoginFail::decode(&mut PacketReader::new(&bytes[1..])).unwrap(),
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
