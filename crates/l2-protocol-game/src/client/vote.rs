//! The recommendation request (`RequestVoteNew`).
//!
//! This only reads the packet. What a recommendation does is the world's job
//! (`rewards/recommendations.rs`).

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_ex_opcode;
use crate::generated::opcodes::ExClientOpcode;

// Java: gameserver/network/clientpackets/RequestVoteNew.java
/// The player recommends the player it has selected. Ex packet `0xD0 0x007E`
/// (`REQUEST_NEW_VOTE_SYSTEM`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestVoteNew {
    /// The object id the client thinks it has selected. Java compares it with the real target
    /// and ignores the request when they differ.
    pub target_id: i32,
}

impl RequestVoteNew {
    pub const EX_OPCODE: ExClientOpcode = ExClientOpcode::RequestNewVoteSystem;
}

impl Encode for RequestVoteNew {
    fn encode(&self, w: &mut PacketWriter) {
        write_ex_opcode(w, Self::EX_OPCODE);
        w.i32(self.target_id);
    }
}

impl Decode for RequestVoteNew {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { target_id: r.i32()? })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_0xd0_the_ex_id_and_the_target() {
        let packet = RequestVoteNew {
            target_id: 0x1234_5678,
        };
        assert_eq!(packet.to_bytes(), [0xD0, 0x7E, 0x00, 0x78, 0x56, 0x34, 0x12]);
    }

    #[test]
    fn round_trips() {
        let packet = RequestVoteNew { target_id: -5 };
        let bytes = packet.to_bytes();
        let decoded = RequestVoteNew::decode(&mut PacketReader::new(&bytes[3..])).unwrap();
        assert_eq!(decoded, packet);
    }

    #[test]
    fn a_short_body_is_an_error() {
        assert!(RequestVoteNew::decode(&mut PacketReader::new(&[1, 2, 3])).is_err());
    }
}
