//! Friend server packets: the friend request, the friend added or removed, the online status of
//! a friend and the private message. The friend list itself is `enterworld::FriendList`.
//!
//! Same rules as [`super::party`]: values only. The world decides who is online and what the
//! names are.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_opcode;
use crate::generated::opcodes::ServerOpcode;

// Java: gameserver/network/serverpackets/FriendAddRequest.java
/// The invited character gets the request (`FRIEND_ADD_REQUEST`). The client shows the dialog
/// with the requestor's name; the trailing int is always 0 in Java.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FriendAddRequest {
    /// The name of the character who asks.
    pub requestor_name: String,
}

impl FriendAddRequest {
    pub const OPCODE: ServerOpcode = ServerOpcode::FriendAddRequest;
}

impl Encode for FriendAddRequest {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.string(&self.requestor_name).i32(0);
    }
}

impl Decode for FriendAddRequest {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let requestor_name = r.string();
        r.skip(4)?; // always 0 in Java
        Ok(Self { requestor_name })
    }
}

// Java: gameserver/network/serverpackets/FriendPacket.java
/// A friend was added or removed (`L2_FRIEND`, `0x76`): the chat-with-friends window updates one
/// entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FriendPacket {
    /// `true` for an added friend (Java writes 1), `false` for a removed one (Java writes 3).
    pub added: bool,
    pub object_id: i32,
    pub name: String,
    pub online: bool,
}

impl FriendPacket {
    pub const OPCODE: ServerOpcode = ServerOpcode::L2Friend;
}

impl Encode for FriendPacket {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        let online_object_id = if self.online { self.object_id } else { 0 };
        w.i32(if self.added { 1 } else { 3 })
            .i32(self.object_id)
            .string(&self.name)
            .i32(i32::from(self.online))
            .i32(online_object_id);
    }
}

impl Decode for FriendPacket {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let added = r.i32()? == 1;
        let object_id = r.i32()?;
        let name = r.string();
        let online = r.i32()? != 0;
        r.skip(4)?; // the object id when online, derived again on encode
        Ok(Self {
            added,
            object_id,
            name,
            online,
        })
    }
}

// Java: gameserver/network/serverpackets/FriendStatusPacket.java
/// A friend logged in or out (`L2_FRIEND_STATUS`, `0x77`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FriendStatusPacket {
    pub online: bool,
    pub name: String,
    pub object_id: i32,
}

impl FriendStatusPacket {
    pub const OPCODE: ServerOpcode = ServerOpcode::L2FriendStatus;
}

impl Encode for FriendStatusPacket {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(i32::from(self.online))
            .string(&self.name)
            .i32(self.object_id);
    }
}

impl Decode for FriendStatusPacket {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let online = r.i32()? != 0;
        let name = r.string();
        let object_id = r.i32()?;
        Ok(Self {
            online,
            name,
            object_id,
        })
    }
}

// Java: gameserver/network/serverpackets/L2FriendSay.java
/// A private message from a friend (`L2_FRIEND_SAY`, `0x78`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct L2FriendSay {
    /// The name of the sender (the player who wrote the message).
    pub sender: String,
    /// The receiver's name as the sender typed it (Java writes `_receiver` from the request).
    pub receiver: String,
    pub message: String,
}

impl L2FriendSay {
    pub const OPCODE: ServerOpcode = ServerOpcode::L2FriendSay;
}

impl Encode for L2FriendSay {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        // Java: `writeInt(0); // ??`
        w.i32(0)
            .string(&self.receiver)
            .string(&self.sender)
            .string(&self.message);
    }
}

impl Decode for L2FriendSay {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        r.skip(4)?; // the `??` int, always 0
        let receiver = r.string();
        let sender = r.string();
        let message = r.string();
        Ok(Self {
            sender,
            receiver,
            message,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip<P: Encode + Decode + PartialEq + std::fmt::Debug>(packet: &P) {
        let bytes = packet.to_bytes();
        let decoded = P::decode(&mut PacketReader::new(&bytes[1..])).unwrap();
        assert_eq!(&decoded, packet);
    }

    #[test]
    fn add_request_is_the_name_and_a_zero() {
        let packet = FriendAddRequest {
            requestor_name: "Alpha".into(),
        };
        round_trip(&packet);
        let bytes = packet.to_bytes();
        assert_eq!(bytes[0], 0x83);
        // "Alpha" (5 units + terminator = 12 bytes), then the int 0.
        assert_eq!(bytes.len(), 1 + 12 + 4);
        assert_eq!(&bytes[bytes.len() - 4..], &[0, 0, 0, 0]);
    }

    #[test]
    fn friend_packet_writes_javas_layout() {
        // Java: 1 for an added friend, 3 for a removed one; the object id when online, else 0.
        let online = FriendPacket {
            added: true,
            object_id: 0x6000_0001,
            name: "Bob".into(),
            online: true,
        };
        round_trip(&online);
        let bytes = online.to_bytes();
        assert_eq!(bytes[0], 0x76);
        assert_eq!(&bytes[1..5], &1_i32.to_le_bytes());
        assert_eq!(&bytes[5..9], &0x6000_0001_i32.to_le_bytes());
        assert_eq!(&bytes[bytes.len() - 4..], &0x6000_0001_i32.to_le_bytes());

        let removed = FriendPacket {
            added: false,
            object_id: 42,
            name: "Bob".into(),
            online: false,
        };
        round_trip(&removed);
        let bytes = removed.to_bytes();
        assert_eq!(&bytes[1..5], &3_i32.to_le_bytes());
        assert_eq!(&bytes[bytes.len() - 4..], &[0, 0, 0, 0]);
    }

    #[test]
    fn status_packet_round_trips() {
        round_trip(&FriendStatusPacket {
            online: true,
            name: "Bob".into(),
            object_id: 7,
        });
        round_trip(&FriendStatusPacket {
            online: false,
            name: String::new(),
            object_id: 7,
        });
    }

    #[test]
    fn friend_say_starts_with_an_int_then_receiver_sender_message() {
        let packet = L2FriendSay {
            sender: "Alpha".into(),
            receiver: "Bob".into(),
            message: "hi".into(),
        };
        round_trip(&packet);
        let bytes = packet.to_bytes();
        assert_eq!(bytes[0], 0x78);
        assert_eq!(&bytes[1..5], &[0, 0, 0, 0]);
        // The receiver comes first in the packet.
        assert_eq!(&bytes[5..11], &[b'B', 0, b'o', 0, b'b', 0]);
    }
}
