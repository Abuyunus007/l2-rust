//! Client packets of the shortcut bar: put something on it and take it off.
//!
//! These only read the packet. The page check, the inventory check and the rows are the world's
//! job (`l2_world::shortcuts`).

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_opcode;
use crate::generated::opcodes::ClientOpcode;

// Java: gameserver/network/clientpackets/RequestShortcutReg.java
/// Put a skill, item, action, macro or recipe on the shortcut bar (`REQUEST_SHORT_CUT_REG`).
///
/// The fields are the raw wire values: Java turns `type_id` into a `ShortcutType` (anything outside
/// 1..=6 becomes `NONE`) and splits `slot` into `slot % 12` and page `slot / 12`; the world does
/// that.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestShortCutReg {
    pub type_id: i32,
    /// The wire slot: `slot + page * 12`.
    pub slot: i32,
    /// The skill id, the item's object id, the action id, the macro id or the recipe id.
    pub id: i32,
    pub level: i32,
    /// 1 player, 2 pet.
    pub character_type: i32,
}

impl RequestShortCutReg {
    pub const OPCODE: ClientOpcode = ClientOpcode::RequestShortCutReg;
}

impl Encode for RequestShortCutReg {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.type_id)
            .i32(self.slot)
            .i32(self.id)
            .i32(self.level)
            .i32(self.character_type);
    }
}

impl Decode for RequestShortCutReg {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            type_id: r.i32()?,
            slot: r.i32()?,
            id: r.i32()?,
            level: r.i32()?,
            character_type: r.i32()?,
        })
    }
}

// Java: gameserver/network/clientpackets/RequestShortcutDel.java
/// Take a shortcut off the bar (`REQUEST_SHORT_CUT_DEL`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestShortCutDel {
    /// The wire slot: `slot + page * 12`.
    pub slot: i32,
}

impl RequestShortCutDel {
    pub const OPCODE: ClientOpcode = ClientOpcode::RequestShortCutDel;
}

impl Encode for RequestShortCutDel {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.slot);
    }
}

impl Decode for RequestShortCutDel {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { slot: r.i32()? })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip<T: Encode + Decode + PartialEq + std::fmt::Debug>(packet: &T) {
        let bytes = packet.to_bytes();
        let decoded = T::decode(&mut PacketReader::new(&bytes[1..])).unwrap();
        assert_eq!(&decoded, packet);
    }

    #[test]
    fn reg_layout_and_round_trip() {
        let packet = RequestShortCutReg {
            type_id: 2,
            slot: 25,
            id: 1204,
            level: 7,
            character_type: 1,
        };
        assert_eq!(
            packet.to_bytes(),
            [
                0x3D, 2, 0, 0, 0, 25, 0, 0, 0, 0xB4, 0x04, 0, 0, 7, 0, 0, 0, 1, 0, 0, 0
            ]
        );
        round_trip(&packet);
        assert!(RequestShortCutReg::decode(&mut PacketReader::new(&[0; 19])).is_err());
    }

    #[test]
    fn del_layout_and_round_trip() {
        let packet = RequestShortCutDel { slot: 13 };
        assert_eq!(packet.to_bytes(), [0x3F, 13, 0, 0, 0]);
        round_trip(&packet);
        assert!(RequestShortCutDel::decode(&mut PacketReader::new(&[0; 3])).is_err());
    }
}
