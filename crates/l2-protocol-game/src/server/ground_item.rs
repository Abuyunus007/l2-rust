//! Items on the ground: an item appears (`SPAWN_ITEM`), a player drops one (`DROP_ITEM`) or picks
//! one up (`GET_ITEM`).
//!
//! Same rules as [`super::enterworld`]: these carry values only. The world reads the item and
//! passes its object id, display id, position, stackability and count in.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_opcode;
use crate::generated::opcodes::ServerOpcode;

// Java: gameserver/network/serverpackets/SpawnItem.java
/// An item on the ground, seen by the nearby players (`SPAWN_ITEM`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpawnItem {
    pub object_id: i32,
    /// The item id the client shows (Java `getDisplayId()`).
    pub display_id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// Java: "Only show item count if it is a stackable item". The count is always written; this
    /// flag only tells the client whether to show it.
    pub stackable: bool,
    pub count: i64,
}

impl SpawnItem {
    pub const OPCODE: ServerOpcode = ServerOpcode::SpawnItem;
}

impl Encode for SpawnItem {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.object_id)
            .i32(self.display_id)
            .i32(self.x)
            .i32(self.y)
            .i32(self.z)
            .i32(i32::from(self.stackable))
            .i64(self.count);
        // Java: two constant 0 ints (`c2` and an unknown one).
        w.i32(0).i32(0);
    }
}

impl Decode for SpawnItem {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let object_id = r.i32()?;
        let display_id = r.i32()?;
        let x = r.i32()?;
        let y = r.i32()?;
        let z = r.i32()?;
        let stackable = r.i32()? != 0;
        let count = r.i64()?;
        r.skip(8)?; // two constant 0 ints
        Ok(Self {
            object_id,
            display_id,
            x,
            y,
            z,
            stackable,
            count,
        })
    }
}

// Java: gameserver/network/serverpackets/DropItem.java
/// A player dropped an item on the ground (`DROP_ITEM`). Same item fields as [`SpawnItem`], plus
/// the id of the player who dropped it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DropItem {
    /// The player who dropped the item (Java `playerObjId`).
    pub player_id: i32,
    pub object_id: i32,
    pub display_id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// See [`SpawnItem::stackable`].
    pub stackable: bool,
    pub count: i64,
}

impl DropItem {
    pub const OPCODE: ServerOpcode = ServerOpcode::DropItem;
}

impl Encode for DropItem {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.player_id)
            .i32(self.object_id)
            .i32(self.display_id)
            .i32(self.x)
            .i32(self.y)
            .i32(self.z)
            .i32(i32::from(self.stackable))
            .i64(self.count);
        // Java writes a constant 1 at the end (`unknown`).
        w.i32(1);
    }
}

impl Decode for DropItem {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let player_id = r.i32()?;
        let object_id = r.i32()?;
        let display_id = r.i32()?;
        let x = r.i32()?;
        let y = r.i32()?;
        let z = r.i32()?;
        let stackable = r.i32()? != 0;
        let count = r.i64()?;
        r.skip(4)?; // constant 1
        Ok(Self {
            player_id,
            object_id,
            display_id,
            x,
            y,
            z,
            stackable,
            count,
        })
    }
}

// Java: gameserver/network/serverpackets/GetItem.java
/// A player picked an item up from the ground (`GET_ITEM`). Java sends the item's position with
/// it, the position the item had on the ground.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GetItem {
    /// The player who picked the item up (Java `playerId`).
    pub player_id: i32,
    pub object_id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl GetItem {
    pub const OPCODE: ServerOpcode = ServerOpcode::GetItem;
}

impl Encode for GetItem {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.player_id)
            .i32(self.object_id)
            .i32(self.x)
            .i32(self.y)
            .i32(self.z);
    }
}

impl Decode for GetItem {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            player_id: r.i32()?,
            object_id: r.i32()?,
            x: r.i32()?,
            y: r.i32()?,
            z: r.i32()?,
        })
    }
}
