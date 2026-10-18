//! Packets that move objects around on the client: `MoveToLocation`, `StopMove`,
//! `MoveToPawn` and `TeleportToLocation`. (`ValidateLocation` lives in [`super::enterworld`].)

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_opcode;
use crate::generated::opcodes::ServerOpcode;

// Java: gameserver/network/serverpackets/MoveToLocation.java
/// An object starts walking to a point (`MOVE_TO_LOCATION`).
///
/// Java's `canBeDropped` is true: under load the network layer may drop it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MoveToLocation {
    /// `getObjectId()`.
    pub object_id: i32,
    /// `getXdestination()`, `getYdestination()`, `getZdestination()`.
    pub dest_x: i32,
    pub dest_y: i32,
    pub dest_z: i32,
    /// `getX()`, `getY()`, `getZ()`: where the object is now.
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl MoveToLocation {
    pub const OPCODE: ServerOpcode = ServerOpcode::MoveToLocation;
}

impl Encode for MoveToLocation {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        // The destination comes first, then the current position.
        w.i32(self.object_id)
            .i32(self.dest_x)
            .i32(self.dest_y)
            .i32(self.dest_z)
            .i32(self.x)
            .i32(self.y)
            .i32(self.z);
    }
}

impl Decode for MoveToLocation {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            object_id: r.i32()?,
            dest_x: r.i32()?,
            dest_y: r.i32()?,
            dest_z: r.i32()?,
            x: r.i32()?,
            y: r.i32()?,
            z: r.i32()?,
        })
    }
}

// Java: gameserver/network/serverpackets/StopMove.java
/// An object stops where it is (`STOP_MOVE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StopMove {
    /// `getObjectId()`.
    pub object_id: i32,
    /// `getX()`, `getY()`, `getZ()`.
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// `getHeading()`.
    pub heading: i32,
}

impl StopMove {
    pub const OPCODE: ServerOpcode = ServerOpcode::StopMove;
}

impl Encode for StopMove {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.object_id)
            .i32(self.x)
            .i32(self.y)
            .i32(self.z)
            .i32(self.heading);
    }
}

impl Decode for StopMove {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            object_id: r.i32()?,
            x: r.i32()?,
            y: r.i32()?,
            z: r.i32()?,
            heading: r.i32()?,
        })
    }
}

// Java: gameserver/network/serverpackets/MoveToPawn.java
/// An object starts following another one (`MOVE_TO_PAWN`).
///
/// Java's `canBeDropped` is true, like `MoveToLocation`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MoveToPawn {
    /// The mover's `getObjectId()`.
    pub object_id: i32,
    /// The target's `getObjectId()`.
    pub target_id: i32,
    /// The distance at which the mover stops.
    pub distance: i32,
    /// The mover's `getX()`, `getY()`, `getZ()`.
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// The target's `getX()`, `getY()`, `getZ()`.
    pub target_x: i32,
    pub target_y: i32,
    pub target_z: i32,
}

impl MoveToPawn {
    pub const OPCODE: ServerOpcode = ServerOpcode::MoveToPawn;
}

impl Encode for MoveToPawn {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.object_id)
            .i32(self.target_id)
            .i32(self.distance)
            .i32(self.x)
            .i32(self.y)
            .i32(self.z)
            .i32(self.target_x)
            .i32(self.target_y)
            .i32(self.target_z);
    }
}

impl Decode for MoveToPawn {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            object_id: r.i32()?,
            target_id: r.i32()?,
            distance: r.i32()?,
            x: r.i32()?,
            y: r.i32()?,
            z: r.i32()?,
            target_x: r.i32()?,
            target_y: r.i32()?,
            target_z: r.i32()?,
        })
    }
}

// Java: gameserver/network/serverpackets/TeleportToLocation.java
/// An object is teleported (`TELEPORT_TO_LOCATION`). The receiving player answers with
/// `Appearing` when its client has loaded the target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TeleportToLocation {
    /// The teleported object's `getObjectId()`.
    pub object_id: i32,
    /// The target position (a packet argument, not the object's current one).
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// The target heading (a packet argument).
    pub heading: i32,
}

impl TeleportToLocation {
    pub const OPCODE: ServerOpcode = ServerOpcode::TeleportToLocation;
}

impl Encode for TeleportToLocation {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.object_id).i32(self.x).i32(self.y).i32(self.z);
        w.i32(0); // fade 0, instant 1: Java always writes 0
        w.i32(self.heading);
    }
}

impl Decode for TeleportToLocation {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let object_id = r.i32()?;
        let x = r.i32()?;
        let y = r.i32()?;
        let z = r.i32()?;
        r.skip(4)?; // fade/instant flag, always 0
        let heading = r.i32()?;
        Ok(Self {
            object_id,
            x,
            y,
            z,
            heading,
        })
    }
}
