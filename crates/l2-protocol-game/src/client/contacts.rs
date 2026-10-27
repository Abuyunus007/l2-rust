//! Client packets of the mail contact list and the friend list of the mail window (L-04).
//!
//! These only read the packet. The lists are the world's job (`l2_world::contacts`).

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_ex_opcode;
use crate::generated::opcodes::ExClientOpcode;

// Java: gameserver/network/clientpackets/RequestExShowContactList.java
/// The player opens the contact list (`0xD0` 0x0086, `REQUEST_EX_SHOW_POST_FRIEND_FOR_POST_BOX`).
/// No fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestExShowContactList;

impl RequestExShowContactList {
    pub const EX_OPCODE: ExClientOpcode = ExClientOpcode::RequestExShowPostFriendForPostBox;
}

impl Encode for RequestExShowContactList {
    fn encode(&self, w: &mut PacketWriter) {
        write_ex_opcode(w, Self::EX_OPCODE);
    }
}

impl Decode for RequestExShowContactList {
    fn decode(_r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self)
    }
}

// Java: gameserver/network/clientpackets/RequestExAddContactToContactList.java
/// Add a character to the contact list (`0xD0` 0x0084, `REQUEST_EX_ADD_POST_FRIEND_FOR_POST_BOX`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestExAddContactToContactList {
    pub name: String,
}

impl RequestExAddContactToContactList {
    pub const EX_OPCODE: ExClientOpcode = ExClientOpcode::RequestExAddPostFriendForPostBox;
}

impl Encode for RequestExAddContactToContactList {
    fn encode(&self, w: &mut PacketWriter) {
        write_ex_opcode(w, Self::EX_OPCODE);
        w.string(&self.name);
    }
}

impl Decode for RequestExAddContactToContactList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { name: r.string() })
    }
}

// Java: gameserver/network/clientpackets/RequestExDeleteContactFromContactList.java
/// Delete a character from the contact list (`0xD0` 0x0085,
/// `REQUEST_EX_DELETE_POST_FRIEND_FOR_POST_BOX`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestExDeleteContactFromContactList {
    pub name: String,
}

impl RequestExDeleteContactFromContactList {
    pub const EX_OPCODE: ExClientOpcode = ExClientOpcode::RequestExDeletePostFriendForPostBox;
}

impl Encode for RequestExDeleteContactFromContactList {
    fn encode(&self, w: &mut PacketWriter) {
        write_ex_opcode(w, Self::EX_OPCODE);
        w.string(&self.name);
    }
}

impl Decode for RequestExDeleteContactFromContactList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { name: r.string() })
    }
}

// Java: gameserver/network/clientpackets/RequestExFriendListExtended.java
/// The mail window asks for the friend list with class and level (`0xD0` 0x0087,
/// `REQUEST_EX_FRIEND_LIST_FOR_POST_BOX`). No fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestExFriendListExtended;

impl RequestExFriendListExtended {
    pub const EX_OPCODE: ExClientOpcode = ExClientOpcode::RequestExFriendListForPostBox;
}

impl Encode for RequestExFriendListExtended {
    fn encode(&self, w: &mut PacketWriter) {
        write_ex_opcode(w, Self::EX_OPCODE);
    }
}

impl Decode for RequestExFriendListExtended {
    fn decode(_r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ex_ids_are_javas() {
        assert_eq!(
            RequestExAddContactToContactList { name: String::new() }.to_bytes()[..3],
            [0xD0, 0x84, 0]
        );
        assert_eq!(
            RequestExDeleteContactFromContactList { name: String::new() }.to_bytes()[..3],
            [0xD0, 0x85, 0]
        );
        assert_eq!(RequestExShowContactList.to_bytes(), [0xD0, 0x86, 0]);
        assert_eq!(RequestExFriendListExtended.to_bytes(), [0xD0, 0x87, 0]);
    }

    #[test]
    fn the_name_round_trips() {
        let packet = RequestExAddContactToContactList { name: "Buddy".into() };
        let bytes = packet.to_bytes();
        let decoded = RequestExAddContactToContactList::decode(&mut PacketReader::new(&bytes[3..])).unwrap();
        assert_eq!(decoded, packet);
        let packet = RequestExDeleteContactFromContactList { name: "Buddy".into() };
        let bytes = packet.to_bytes();
        let decoded =
            RequestExDeleteContactFromContactList::decode(&mut PacketReader::new(&bytes[3..])).unwrap();
        assert_eq!(decoded, packet);
    }
}
