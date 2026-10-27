//! Client packets of the teleport bookmarks ("My Teleports", L-04).
//!
//! All six are the ex packet `0xD0` 0x0051 (`EX_BOOKMARK_PACKET`) followed by an `i32` sub id that
//! Java's `GamePacketHandler` reads to pick the packet. [`crate::dispatch`] reads that sub id; the
//! structs here read what comes after it. The rules are the world's job (`l2_world::bookmarks`).

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_ex_opcode;
use crate::generated::opcodes::ExClientOpcode;

/// Sub id of [`RequestBookMarkSlotInfo`].
pub const SUB_SLOT_INFO: i32 = 0;
/// Sub id of [`RequestSaveBookMarkSlot`].
pub const SUB_SAVE: i32 = 1;
/// Sub id of [`RequestModifyBookMarkSlot`].
pub const SUB_MODIFY: i32 = 2;
/// Sub id of [`RequestDeleteBookMarkSlot`].
pub const SUB_DELETE: i32 = 3;
/// Sub id of [`RequestTeleportBookMark`].
pub const SUB_TELEPORT: i32 = 4;
/// Sub id of [`RequestChangeBookMarkSlot`].
pub const SUB_CHANGE_SLOT: i32 = 5;

/// The ex id and the sub id.
fn write_head(w: &mut PacketWriter, sub: i32) {
    write_ex_opcode(w, ExClientOpcode::ExBookmarkPacket);
    w.i32(sub);
}

// Java: gameserver/network/clientpackets/RequestBookMarkSlotInfo.java
/// The player opens "My Teleports": the server sends the list. No fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestBookMarkSlotInfo;

impl Encode for RequestBookMarkSlotInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_head(w, SUB_SLOT_INFO);
    }
}

impl Decode for RequestBookMarkSlotInfo {
    fn decode(_r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self)
    }
}

// Java: gameserver/network/clientpackets/RequestSaveBookMarkSlot.java
/// Save the current location as a bookmark.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestSaveBookMarkSlot {
    pub name: String,
    pub icon: i32,
    pub tag: String,
}

impl Encode for RequestSaveBookMarkSlot {
    fn encode(&self, w: &mut PacketWriter) {
        write_head(w, SUB_SAVE);
        w.string(&self.name).i32(self.icon).string(&self.tag);
    }
}

impl Decode for RequestSaveBookMarkSlot {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let name = r.string();
        let icon = r.i32()?;
        let tag = r.string();
        Ok(Self { name, icon, tag })
    }
}

// Java: gameserver/network/clientpackets/RequestModifyBookMarkSlot.java
/// Change the name, icon and tag of a bookmark.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestModifyBookMarkSlot {
    pub id: i32,
    pub name: String,
    pub icon: i32,
    pub tag: String,
}

impl Encode for RequestModifyBookMarkSlot {
    fn encode(&self, w: &mut PacketWriter) {
        write_head(w, SUB_MODIFY);
        w.i32(self.id).string(&self.name).i32(self.icon).string(&self.tag);
    }
}

impl Decode for RequestModifyBookMarkSlot {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let id = r.i32()?;
        let name = r.string();
        let icon = r.i32()?;
        let tag = r.string();
        Ok(Self { id, name, icon, tag })
    }
}

// Java: gameserver/network/clientpackets/RequestDeleteBookMarkSlot.java
/// Delete a bookmark.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestDeleteBookMarkSlot {
    pub id: i32,
}

impl Encode for RequestDeleteBookMarkSlot {
    fn encode(&self, w: &mut PacketWriter) {
        write_head(w, SUB_DELETE);
        w.i32(self.id);
    }
}

impl Decode for RequestDeleteBookMarkSlot {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { id: r.i32()? })
    }
}

// Java: gameserver/network/clientpackets/RequestTeleportBookMark.java
/// Teleport to a bookmark.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestTeleportBookMark {
    pub id: i32,
}

impl Encode for RequestTeleportBookMark {
    fn encode(&self, w: &mut PacketWriter) {
        write_head(w, SUB_TELEPORT);
        w.i32(self.id);
    }
}

impl Decode for RequestTeleportBookMark {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { id: r.i32()? })
    }
}

// Java: gameserver/network/clientpackets/RequestChangeBookMarkSlot.java
/// Java reads nothing and does nothing ("change slot" is not implemented).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestChangeBookMarkSlot;

impl Encode for RequestChangeBookMarkSlot {
    fn encode(&self, w: &mut PacketWriter) {
        write_head(w, SUB_CHANGE_SLOT);
    }
}

impl Decode for RequestChangeBookMarkSlot {
    fn decode(_r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bytes after `0xD0`, the ex id and the sub id.
    fn body<T: Encode>(packet: &T, sub: i32) -> Vec<u8> {
        let bytes = packet.to_bytes();
        assert_eq!(bytes[..3], [0xD0, 0x51, 0x00]);
        assert_eq!(bytes[3..7], sub.to_le_bytes());
        bytes[7..].to_vec()
    }

    #[test]
    fn save_round_trips() {
        let packet = RequestSaveBookMarkSlot {
            name: "Home".into(),
            icon: 3,
            tag: "fav".into(),
        };
        let body = body(&packet, SUB_SAVE);
        // name, then the icon int, then the tag.
        assert_eq!(&body[..10], [b'H', 0, b'o', 0, b'm', 0, b'e', 0, 0, 0]);
        assert_eq!(
            RequestSaveBookMarkSlot::decode(&mut PacketReader::new(&body)).unwrap(),
            packet
        );
    }

    #[test]
    fn modify_round_trips() {
        let packet = RequestModifyBookMarkSlot {
            id: 2,
            name: "Gludio".into(),
            icon: 7,
            tag: String::new(),
        };
        let body = body(&packet, SUB_MODIFY);
        assert_eq!(body[..4], 2i32.to_le_bytes());
        assert_eq!(
            RequestModifyBookMarkSlot::decode(&mut PacketReader::new(&body)).unwrap(),
            packet
        );
    }

    #[test]
    fn id_packets_round_trip() {
        let delete = RequestDeleteBookMarkSlot { id: 9 };
        let bytes = body(&delete, SUB_DELETE);
        assert_eq!(bytes, 9i32.to_le_bytes());
        assert_eq!(
            RequestDeleteBookMarkSlot::decode(&mut PacketReader::new(&bytes)).unwrap(),
            delete
        );
        let go = RequestTeleportBookMark { id: -1 };
        let bytes = body(&go, SUB_TELEPORT);
        assert_eq!(
            RequestTeleportBookMark::decode(&mut PacketReader::new(&bytes)).unwrap(),
            go
        );
        assert!(RequestTeleportBookMark::decode(&mut PacketReader::new(&bytes[..3])).is_err());
    }

    #[test]
    fn packets_without_fields_are_only_the_head() {
        assert!(body(&RequestBookMarkSlotInfo, SUB_SLOT_INFO).is_empty());
        assert!(body(&RequestChangeBookMarkSlot, SUB_CHANGE_SLOT).is_empty());
    }
}
