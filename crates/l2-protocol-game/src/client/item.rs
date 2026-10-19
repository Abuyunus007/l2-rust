//! Client packets of the inventory: use, unequip, drop and destroy an item, and the request for
//! the item list.
//!
//! These only read the packet. Moving items and checking them is the world's job (the handlers of
//! M4-INV-01 and M4-DROP-01).

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_opcode;
use crate::generated::opcodes::ClientOpcode;

// Java: gameserver/network/clientpackets/UseItem.java
/// Use an item from the inventory (`USE_ITEM`). `ctrl_pressed` is the ctrl key while the item is
/// used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UseItem {
    pub object_id: i32,
    pub ctrl_pressed: bool,
}

impl UseItem {
    pub const OPCODE: ClientOpcode = ClientOpcode::UseItem;
}

impl Encode for UseItem {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.object_id).i32(i32::from(self.ctrl_pressed));
    }
}

impl Decode for UseItem {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let object_id = r.i32()?;
        // Java `!= 0` here, unlike `== 1` in `RequestActionUse`.
        let ctrl_pressed = r.i32()? != 0;
        Ok(Self {
            object_id,
            ctrl_pressed,
        })
    }
}

// Java: gameserver/network/clientpackets/RequestUnEquipItem.java
/// Take off the item in a paperdoll slot (`REQUEST_UN_EQUIP_ITEM`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestUnEquipItem {
    /// The raw paperdoll slot number. Java maps it to a body part with
    /// `BodyPart.fromPaperdollSlot`; the world does that mapping.
    pub paperdoll_slot: i32,
}

impl RequestUnEquipItem {
    pub const OPCODE: ClientOpcode = ClientOpcode::RequestUnEquipItem;
}

impl Encode for RequestUnEquipItem {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.paperdoll_slot);
    }
}

impl Decode for RequestUnEquipItem {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            paperdoll_slot: r.i32()?,
        })
    }
}

// Java: gameserver/network/clientpackets/RequestDropItem.java
/// Drop `count` of an inventory item at a position on the ground (`REQUEST_DROP_ITEM`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestDropItem {
    pub object_id: i32,
    pub count: i64,
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl RequestDropItem {
    pub const OPCODE: ClientOpcode = ClientOpcode::RequestDropItem;
}

impl Encode for RequestDropItem {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.object_id)
            .i64(self.count)
            .i32(self.x)
            .i32(self.y)
            .i32(self.z);
    }
}

impl Decode for RequestDropItem {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            object_id: r.i32()?,
            count: r.i64()?,
            x: r.i32()?,
            y: r.i32()?,
            z: r.i32()?,
        })
    }
}

// Java: gameserver/network/clientpackets/RequestDestroyItem.java
/// Destroy `count` of an inventory item (`REQUEST_DESTROY_ITEM`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestDestroyItem {
    pub object_id: i32,
    pub count: i64,
}

impl RequestDestroyItem {
    pub const OPCODE: ClientOpcode = ClientOpcode::RequestDestroyItem;
}

impl Encode for RequestDestroyItem {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.object_id).i64(self.count);
    }
}

impl Decode for RequestDestroyItem {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            object_id: r.i32()?,
            count: r.i64()?,
        })
    }
}

// Java: gameserver/network/clientpackets/RequestItemList.java
/// Asks for the whole inventory (`REQUEST_ITEM_LIST`). The packet has no fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestItemList;

impl RequestItemList {
    pub const OPCODE: ClientOpcode = ClientOpcode::RequestItemList;
}

impl Encode for RequestItemList {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
    }
}

impl Decode for RequestItemList {
    fn decode(_r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self)
    }
}
