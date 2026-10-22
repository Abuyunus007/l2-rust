//! Client packets of the friend list and the block list (`Chat with Friends` and `/block`).
//!
//! These only read the packet. The checks (who is online, the block list, the olympiad rule ...)
//! and the lists themselves are the world's job (`l2-world/src/friends.rs`).

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_opcode;
use crate::generated::opcodes::ClientOpcode;

// Java: gameserver/network/clientpackets/RequestFriendInvite.java
/// The player asks another character to become a friend (`REQUEST_FRIEND_INVITE`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestFriendInvite {
    /// The name the player typed.
    pub name: String,
}

impl RequestFriendInvite {
    pub const OPCODE: ClientOpcode = ClientOpcode::RequestFriendInvite;
}

impl Encode for RequestFriendInvite {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.string(&self.name);
    }
}

impl Decode for RequestFriendInvite {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { name: r.string() })
    }
}

// Java: gameserver/network/clientpackets/RequestAnswerFriendInvite.java
/// The invited player answers the friend request (`REQUEST_ANSWER_FRIEND_INVITE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestAnswerFriendInvite {
    /// 1 accept, anything else refuses (Java checks for 1 only).
    pub response: i32,
}

impl RequestAnswerFriendInvite {
    pub const OPCODE: ClientOpcode = ClientOpcode::RequestAnswerFriendInvite;
}

impl Encode for RequestAnswerFriendInvite {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.response);
    }
}

impl Decode for RequestAnswerFriendInvite {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { response: r.i32()? })
    }
}

// Java: gameserver/network/clientpackets/RequestFriendList.java
/// The player opens the friend list window (`REQUEST_FRIEND_LIST`). No fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestFriendList;

impl RequestFriendList {
    pub const OPCODE: ClientOpcode = ClientOpcode::RequestFriendList;
}

impl Encode for RequestFriendList {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
    }
}

impl Decode for RequestFriendList {
    fn decode(_r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self)
    }
}

// Java: gameserver/network/clientpackets/RequestFriendDel.java
/// The player removes a friend (`REQUEST_FRIEND_DEL`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestFriendDel {
    /// The name the player typed.
    pub name: String,
}

impl RequestFriendDel {
    pub const OPCODE: ClientOpcode = ClientOpcode::RequestFriendDel;
}

impl Encode for RequestFriendDel {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.string(&self.name);
    }
}

impl Decode for RequestFriendDel {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { name: r.string() })
    }
}

// Java: gameserver/network/clientpackets/RequestSendFriendMsg.java
/// A private message to a friend (`REQUEST_SEND_FRIEND_MSG`): the text first, then the receiver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestSendFriendMsg {
    pub message: String,
    pub receiver: String,
}

impl RequestSendFriendMsg {
    pub const OPCODE: ClientOpcode = ClientOpcode::RequestSendFriendMsg;
}

impl Encode for RequestSendFriendMsg {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.string(&self.message).string(&self.receiver);
    }
}

impl Decode for RequestSendFriendMsg {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            message: r.string(),
            receiver: r.string(),
        })
    }
}

// Java: gameserver/network/clientpackets/RequestBlock.java
/// The block list window and the `/block`, `/unblock`, `/allblock`, `/allunblock` and
/// `/blocklist` commands (`REQUEST_BLOCK`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestBlock {
    /// 0 block, 1 unblock, 2 the list, 3 refuse all messages, 4 accept messages again. Other values
    /// are logged by Java and ignored.
    pub kind: i32,
    /// The name of the character, read for block and unblock only (`None` for the others).
    pub name: Option<String>,
}

impl RequestBlock {
    pub const OPCODE: ClientOpcode = ClientOpcode::RequestBlock;
    /// Java `RequestBlock.BLOCK`.
    pub const BLOCK: i32 = 0;
    /// Java `RequestBlock.UNBLOCK`.
    pub const UNBLOCK: i32 = 1;
    /// Java `RequestBlock.BLOCKLIST`.
    pub const BLOCKLIST: i32 = 2;
    /// Java `RequestBlock.ALLBLOCK`.
    pub const ALLBLOCK: i32 = 3;
    /// Java `RequestBlock.ALLUNBLOCK`.
    pub const ALLUNBLOCK: i32 = 4;
}

impl Encode for RequestBlock {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.kind);
        if let Some(name) = &self.name {
            w.string(name);
        }
    }
}

impl Decode for RequestBlock {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let kind = r.i32()?;
        let name = matches!(kind, Self::BLOCK | Self::UNBLOCK).then(|| r.string());
        Ok(Self { kind, name })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip<P: Encode + Decode + PartialEq + std::fmt::Debug>(packet: &P) {
        let bytes = packet.to_bytes();
        // Strip the opcode byte, which `decode` does not read.
        let decoded = P::decode(&mut PacketReader::new(&bytes[1..])).unwrap();
        assert_eq!(&decoded, packet);
    }

    #[test]
    fn friend_invite_and_del_carry_the_name() {
        round_trip(&RequestFriendInvite { name: "Alpha".into() });
        round_trip(&RequestFriendDel { name: "Bravo".into() });
        // UTF-16LE with a terminating zero, like every Java string.
        let bytes = RequestFriendInvite { name: "A".into() }.to_bytes();
        assert_eq!(bytes, vec![0x77, b'A', 0, 0, 0]);
    }

    #[test]
    fn answer_is_one_int() {
        round_trip(&RequestAnswerFriendInvite { response: 1 });
        assert_eq!(
            RequestAnswerFriendInvite { response: 1 }.to_bytes(),
            vec![0x78, 1, 0, 0, 0]
        );
    }

    #[test]
    fn friend_list_has_no_fields() {
        round_trip(&RequestFriendList);
        assert_eq!(RequestFriendList.to_bytes(), vec![0x79]);
    }

    #[test]
    fn send_friend_msg_is_the_text_then_the_receiver() {
        let packet = RequestSendFriendMsg {
            message: "hi".into(),
            receiver: "Bob".into(),
        };
        round_trip(&packet);
        let bytes = packet.to_bytes();
        // The first string is the message: "hi" in UTF-16LE and its terminating zero, then the
        // receiver "Bob" the same way.
        assert_eq!(&bytes[1..7], &[b'h', 0, b'i', 0, 0, 0]);
        assert_eq!(&bytes[7..], &[b'B', 0, b'o', 0, b'b', 0, 0, 0]);
    }

    #[test]
    fn block_reads_a_name_only_for_block_and_unblock() {
        round_trip(&RequestBlock {
            kind: RequestBlock::BLOCK,
            name: Some("Eve".into()),
        });
        round_trip(&RequestBlock {
            kind: RequestBlock::UNBLOCK,
            name: Some("Eve".into()),
        });
        for kind in [
            RequestBlock::BLOCKLIST,
            RequestBlock::ALLBLOCK,
            RequestBlock::ALLUNBLOCK,
        ] {
            round_trip(&RequestBlock { kind, name: None });
        }
        // A list request with trailing bytes: the name is not read, so the bytes are ignored.
        let packet = RequestBlock::decode(&mut PacketReader::new(&[2, 0, 0, 0, 9, 9])).unwrap();
        assert_eq!(packet, RequestBlock { kind: 2, name: None });
    }

    #[test]
    fn truncated_int_is_an_error() {
        assert!(RequestAnswerFriendInvite::decode(&mut PacketReader::new(&[1, 0])).is_err());
        assert!(RequestBlock::decode(&mut PacketReader::new(&[0, 0])).is_err());
    }
}
