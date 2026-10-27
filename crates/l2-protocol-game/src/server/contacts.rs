//! Server packets of the mail contact list and the friend list of the mail window (L-04). The
//! contact list itself is `entry::ExShowContactList`.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::{read_count, write_opcode};
use crate::generated::opcodes::ServerOpcode;

// Java: gameserver/network/serverpackets/ExConfirmAddingContact.java
/// The answer to adding a contact (`EX_CONFIRM_ADDING_POST_FRIEND`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExConfirmAddingContact {
    pub name: String,
    pub added: bool,
}

impl ExConfirmAddingContact {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExConfirmAddingPostFriend;
}

impl Encode for ExConfirmAddingContact {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.string(&self.name).i32(i32::from(self.added));
    }
}

impl Decode for ExConfirmAddingContact {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let name = r.string();
        let added = r.i32()? != 0;
        Ok(Self { name, added })
    }
}

/// One friend of [`FriendListExtended`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FriendListExtendedEntry {
    pub object_id: i32,
    pub name: String,
    pub online: bool,
    pub class_id: i32,
    pub level: i32,
}

// Java: gameserver/network/serverpackets/FriendListExtended.java
/// The friend list with class and level (`FRIEND_LIST`, not the `L2_FRIEND_LIST` of the normal
/// friend list).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FriendListExtended {
    pub friends: Vec<FriendListExtendedEntry>,
}

impl FriendListExtended {
    pub const OPCODE: ServerOpcode = ServerOpcode::FriendList;
}

impl Encode for FriendListExtended {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.friends.len() as i32);
        for friend in &self.friends {
            let online_object_id = if friend.online { friend.object_id } else { 0 };
            w.i32(friend.object_id)
                .string(&friend.name)
                .i32(i32::from(friend.online))
                .i32(online_object_id)
                .i32(friend.class_id)
                .i32(friend.level);
        }
    }
}

impl Decode for FriendListExtended {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let count = read_count(r, "friend count")?;
        let mut friends = Vec::new();
        for _ in 0..count {
            let object_id = r.i32()?;
            let name = r.string();
            let online = r.i32()? != 0;
            r.skip(4)?; // the object id when online, derived again on encode
            friends.push(FriendListExtendedEntry {
                object_id,
                name,
                online,
                class_id: r.i32()?,
                level: r.i32()?,
            });
        }
        Ok(Self { friends })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirm_round_trips() {
        let packet = ExConfirmAddingContact {
            name: "Buddy".into(),
            added: true,
        };
        let bytes = packet.to_bytes();
        assert_eq!(bytes[..3], [0xFE, 0xD2, 0x00]);
        assert_eq!(
            ExConfirmAddingContact::decode(&mut PacketReader::new(&bytes[3..])).unwrap(),
            packet
        );
        assert_eq!(bytes[bytes.len() - 4..], [1, 0, 0, 0]);
    }

    #[test]
    fn the_friend_list_has_class_and_level_and_the_object_id_only_online() {
        let packet = FriendListExtended {
            friends: vec![
                FriendListExtendedEntry {
                    object_id: 10,
                    name: "A".into(),
                    online: true,
                    class_id: 88,
                    level: 80,
                },
                FriendListExtendedEntry {
                    object_id: 11,
                    name: "B".into(),
                    online: false,
                    class_id: 1,
                    level: 5,
                },
            ],
        };
        let bytes = packet.to_bytes();
        assert_eq!(bytes[0], 0x58);
        // count, then the first friend: id, "A" (UTF-16 + terminator), online, object id
        assert_eq!(bytes[1..5], [2, 0, 0, 0]);
        assert_eq!(bytes[5..9], [10, 0, 0, 0]);
        assert_eq!(bytes[9..13], [b'A', 0, 0, 0]);
        assert_eq!(bytes[13..17], [1, 0, 0, 0]);
        assert_eq!(bytes[17..21], [10, 0, 0, 0]);
        assert_eq!(
            FriendListExtended::decode(&mut PacketReader::new(&bytes[1..])).unwrap(),
            packet
        );
        // The second one is offline: its online flag and object id field are 0. The first friend
        // is six ints (the name is one) = 24 bytes after the count.
        let second = &bytes[5 + 24..];
        assert_eq!(second[..4], [11, 0, 0, 0]);
        assert_eq!(second[8..12], [0, 0, 0, 0]);
        assert_eq!(second[12..16], [0, 0, 0, 0]);
    }
}
