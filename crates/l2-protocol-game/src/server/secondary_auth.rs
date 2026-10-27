//! Server packets of the secondary password (L-04): the window to show, the verdict of a typed
//! password and the answer to creating or changing it.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_opcode;
use crate::generated::opcodes::ServerOpcode;

// Java: gameserver/network/serverpackets/Ex2ndPasswordCheck.java
/// Which password window the client shows (`EX_2ND_PASSWORD_CHECK`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ex2ndPasswordCheck {
    /// One of the `PASSWORD_*` values.
    pub window_type: i32,
}

impl Ex2ndPasswordCheck {
    pub const OPCODE: ServerOpcode = ServerOpcode::Ex2ndPasswordCheck;
    /// The account has no password yet: the "create password" window.
    pub const PASSWORD_NEW: i32 = 0;
    /// The account has a password: the "enter password" window.
    pub const PASSWORD_PROMPT: i32 = 1;
    /// No password is needed.
    pub const PASSWORD_OK: i32 = 2;
}

impl Encode for Ex2ndPasswordCheck {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.window_type).i32(0);
    }
}

impl Decode for Ex2ndPasswordCheck {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let window_type = r.i32()?;
        r.skip(4)?;
        Ok(Self { window_type })
    }
}

// Java: gameserver/network/serverpackets/Ex2ndPasswordVerify.java
/// The verdict on a typed password (`EX_2ND_PASSWORD_VERIFY`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ex2ndPasswordVerify {
    /// One of the `PASSWORD_*` values.
    pub mode: i32,
    /// The wrong attempts so far (the limit for `PASSWORD_BAN`).
    pub wrong_attempts: i32,
}

impl Ex2ndPasswordVerify {
    pub const OPCODE: ServerOpcode = ServerOpcode::Ex2ndPasswordVerify;
    pub const PASSWORD_OK: i32 = 0;
    pub const PASSWORD_WRONG: i32 = 1;
    pub const PASSWORD_BAN: i32 = 2;
}

impl Encode for Ex2ndPasswordVerify {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.mode).i32(self.wrong_attempts);
    }
}

impl Decode for Ex2ndPasswordVerify {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            mode: r.i32()?,
            wrong_attempts: r.i32()?,
        })
    }
}

// Java: gameserver/network/serverpackets/Ex2ndPasswordAck.java
/// The answer to creating or changing the password (`EX_2ND_PASSWORD_ACK`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ex2ndPasswordAck {
    /// `SUCCESS` or `WRONG_PATTERN`.
    pub response: i32,
}

impl Ex2ndPasswordAck {
    pub const OPCODE: ServerOpcode = ServerOpcode::Ex2ndPasswordAck;
    pub const SUCCESS: i32 = 0;
    pub const WRONG_PATTERN: i32 = 1;
}

impl Encode for Ex2ndPasswordAck {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        // Java: a byte 0, then `writeInt(_response == WRONG_PATTERN)`, then an int 0.
        w.u8(0)
            .i32(i32::from(self.response == Self::WRONG_PATTERN))
            .i32(0);
    }
}

impl Decode for Ex2ndPasswordAck {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        r.skip(1)?;
        let wrong = r.i32()? != 0;
        r.skip(4)?;
        Ok(Self {
            response: if wrong { Self::WRONG_PATTERN } else { Self::SUCCESS },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_is_the_window_and_a_zero() {
        let packet = Ex2ndPasswordCheck {
            window_type: Ex2ndPasswordCheck::PASSWORD_PROMPT,
        };
        let bytes = packet.to_bytes();
        assert_eq!(bytes[0], 0xFE);
        assert_eq!(bytes[3..], [1, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(
            Ex2ndPasswordCheck::decode(&mut PacketReader::new(&bytes[3..])).unwrap(),
            packet
        );
    }

    #[test]
    fn verify_is_the_mode_and_the_attempts() {
        let packet = Ex2ndPasswordVerify {
            mode: Ex2ndPasswordVerify::PASSWORD_WRONG,
            wrong_attempts: 3,
        };
        let bytes = packet.to_bytes();
        assert_eq!(bytes[3..], [1, 0, 0, 0, 3, 0, 0, 0]);
        assert_eq!(
            Ex2ndPasswordVerify::decode(&mut PacketReader::new(&bytes[3..])).unwrap(),
            packet
        );
    }

    #[test]
    fn ack_is_a_byte_a_flag_and_a_zero() {
        for (response, flag) in [
            (Ex2ndPasswordAck::SUCCESS, 0),
            (Ex2ndPasswordAck::WRONG_PATTERN, 1),
        ] {
            let packet = Ex2ndPasswordAck { response };
            let bytes = packet.to_bytes();
            assert_eq!(bytes[3..], [0, flag, 0, 0, 0, 0, 0, 0, 0]);
            assert_eq!(
                Ex2ndPasswordAck::decode(&mut PacketReader::new(&bytes[3..])).unwrap(),
                packet
            );
        }
    }
}
