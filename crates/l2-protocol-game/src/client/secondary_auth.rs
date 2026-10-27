//! Client packets of the secondary password (L-04). They are accepted while the client is at the
//! character screen (`AUTHENTICATED`).
//!
//! These only read the packet. The password is the session's job (`game-server` `secondary.rs`).

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_ex_opcode;
use crate::generated::opcodes::ExClientOpcode;

// Java: gameserver/network/clientpackets/RequestEx2ndPasswordCheck.java
/// The client asks whether it must show the password window (`0xD0` 0x0093). No fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestEx2ndPasswordCheck;

impl RequestEx2ndPasswordCheck {
    pub const EX_OPCODE: ExClientOpcode = ExClientOpcode::RequestEx2ndPasswordCheck;
}

impl Encode for RequestEx2ndPasswordCheck {
    fn encode(&self, w: &mut PacketWriter) {
        write_ex_opcode(w, Self::EX_OPCODE);
    }
}

impl Decode for RequestEx2ndPasswordCheck {
    fn decode(_r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self)
    }
}

// Java: gameserver/network/clientpackets/RequestEx2ndPasswordVerify.java
/// The password typed to unlock the character screen (`0xD0` 0x0094).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestEx2ndPasswordVerify {
    pub password: String,
}

impl RequestEx2ndPasswordVerify {
    pub const EX_OPCODE: ExClientOpcode = ExClientOpcode::RequestEx2ndPasswordVerify;
}

impl Encode for RequestEx2ndPasswordVerify {
    fn encode(&self, w: &mut PacketWriter) {
        write_ex_opcode(w, Self::EX_OPCODE);
        w.string(&self.password);
    }
}

impl Decode for RequestEx2ndPasswordVerify {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { password: r.string() })
    }
}

// Java: gameserver/network/clientpackets/RequestEx2ndPasswordReq.java
/// Create (`change_pass` 0) or change (2) the password (`0xD0` 0x0095).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestEx2ndPasswordReq {
    /// Java `readByte()`: 0 creates the password, 2 changes it, anything else does nothing.
    pub change_pass: u8,
    /// The new password for 0, the old one for 2.
    pub password: String,
    /// Only on the wire when `change_pass` is 2.
    pub new_password: Option<String>,
}

impl RequestEx2ndPasswordReq {
    pub const EX_OPCODE: ExClientOpcode = ExClientOpcode::RequestEx2ndPasswordReq;
}

impl Encode for RequestEx2ndPasswordReq {
    fn encode(&self, w: &mut PacketWriter) {
        write_ex_opcode(w, Self::EX_OPCODE);
        w.u8(self.change_pass).string(&self.password);
        if self.change_pass == 2 {
            w.string(self.new_password.as_deref().unwrap_or(""));
        }
    }
}

impl Decode for RequestEx2ndPasswordReq {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let change_pass = r.u8()?;
        let password = r.string();
        let new_password = if change_pass == 2 { Some(r.string()) } else { None };
        Ok(Self {
            change_pass,
            password,
            new_password,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ex_ids_are_javas() {
        assert_eq!(RequestEx2ndPasswordCheck.to_bytes(), [0xD0, 0x93, 0]);
        assert_eq!(
            RequestEx2ndPasswordVerify {
                password: String::new()
            }
            .to_bytes()[..3],
            [0xD0, 0x94, 0]
        );
    }

    #[test]
    fn verify_round_trips() {
        let packet = RequestEx2ndPasswordVerify {
            password: "123457".into(),
        };
        let bytes = packet.to_bytes();
        let decoded = RequestEx2ndPasswordVerify::decode(&mut PacketReader::new(&bytes[3..])).unwrap();
        assert_eq!(decoded, packet);
    }

    #[test]
    fn req_reads_the_new_password_only_for_a_change() {
        let create = RequestEx2ndPasswordReq {
            change_pass: 0,
            password: "123457".into(),
            new_password: None,
        };
        let bytes = create.to_bytes();
        assert_eq!(bytes[..4], [0xD0, 0x95, 0, 0]);
        assert_eq!(
            RequestEx2ndPasswordReq::decode(&mut PacketReader::new(&bytes[3..])).unwrap(),
            create
        );
        let change = RequestEx2ndPasswordReq {
            change_pass: 2,
            password: "123457".into(),
            new_password: Some("654321".into()),
        };
        let bytes = change.to_bytes();
        assert_eq!(
            RequestEx2ndPasswordReq::decode(&mut PacketReader::new(&bytes[3..])).unwrap(),
            change
        );
    }
}
