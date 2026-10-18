//! Client packets about walking and position: `MoveToLocation`, `ValidatePosition`,
//! `CannotMoveAnymore` and `Appearing`.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_opcode;
use crate::generated::opcodes::ClientOpcode;

// Java: gameserver/network/clientpackets/MoveToLocation.java
/// The player clicked (or used the cursor keys) to walk somewhere. This is Java's
/// `MOVE_BACKWARD_TO_LOCATION` packet (opcode `0x0F`; the generated table calls it
/// `MoveToLocation`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MoveToLocation {
    /// `_targetX`, `_targetY`, `_targetZ`: where the player wants to go.
    pub target_x: i32,
    pub target_y: i32,
    pub target_z: i32,
    /// `_originX`, `_originY`, `_originZ`: where the client thinks the player is.
    pub origin_x: i32,
    pub origin_y: i32,
    pub origin_z: i32,
    /// `_movementMode`: 0 when the cursor keys are used, 1 when the mouse is used.
    pub movement_mode: i32,
}

impl MoveToLocation {
    pub const OPCODE: ClientOpcode = ClientOpcode::MoveToLocation;
}

impl Encode for MoveToLocation {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.target_x)
            .i32(self.target_y)
            .i32(self.target_z)
            .i32(self.origin_x)
            .i32(self.origin_y)
            .i32(self.origin_z)
            .i32(self.movement_mode);
    }
}

impl Decode for MoveToLocation {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            target_x: r.i32()?,
            target_y: r.i32()?,
            target_z: r.i32()?,
            origin_x: r.i32()?,
            origin_y: r.i32()?,
            origin_z: r.i32()?,
            movement_mode: r.i32()?,
        })
    }
}

// Java: gameserver/network/clientpackets/ValidatePosition.java
/// The client's periodic report of where it thinks the player is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ValidatePosition {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
    /// Java reads it and throws it away (`readInt(); // vehicle id`); kept so the packet
    /// re-encodes to the same bytes.
    pub vehicle_id: i32,
}

impl ValidatePosition {
    pub const OPCODE: ClientOpcode = ClientOpcode::ValidatePosition;
}

impl Encode for ValidatePosition {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.x)
            .i32(self.y)
            .i32(self.z)
            .i32(self.heading)
            .i32(self.vehicle_id);
    }
}

impl Decode for ValidatePosition {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            x: r.i32()?,
            y: r.i32()?,
            z: r.i32()?,
            heading: r.i32()?,
            vehicle_id: r.i32()?,
        })
    }
}

// Java: gameserver/network/clientpackets/CannotMoveAnymore.java
/// The client could not walk any further (it ran into something).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CannotMoveAnymore {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
}

impl CannotMoveAnymore {
    pub const OPCODE: ClientOpcode = ClientOpcode::CannotMoveAnymore;
}

impl Encode for CannotMoveAnymore {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.x).i32(self.y).i32(self.z).i32(self.heading);
    }
}

impl Decode for CannotMoveAnymore {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            x: r.i32()?,
            y: r.i32()?,
            z: r.i32()?,
            heading: r.i32()?,
        })
    }
}

// Java: gameserver/network/clientpackets/Appearing.java
/// The client finished loading after a teleport. No fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Appearing;

impl Appearing {
    pub const OPCODE: ClientOpcode = ClientOpcode::Appearing;
}

impl Encode for Appearing {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
    }
}

impl Decode for Appearing {
    fn decode(_r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self)
    }
}
