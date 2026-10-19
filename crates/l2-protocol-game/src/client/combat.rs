//! Client packets of fighting and dying: `AttackRequest`, `RequestActionUse` and
//! `RequestRestartPoint`.
//!
//! These only read the packet. What an attack, an action or a restart point does is the world's
//! job (the handlers of M4-COMBAT-01 and M4-DEATH-01).

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_opcode;
use crate::generated::opcodes::ClientOpcode;

// Java: gameserver/network/clientpackets/AttackRequest.java
/// A click to attack an object (a target, a monster). Java reads the same fields as `Action`.
///
/// Java registers this class for two ids: `ATTACK` (0x01) and `ATTACK_REQUEST` (0x32). The
/// dispatcher maps both to this struct, [`Self::OPCODE`] is the one written by `encode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttackRequest {
    pub object_id: i32,
    /// The client's position at the click. Java reads them and never uses them.
    pub origin_x: i32,
    pub origin_y: i32,
    pub origin_z: i32,
    /// [`Self::SIMPLE_CLICK`] or [`Self::SHIFT_CLICK`]. Java reads a signed byte.
    pub attack_id: i8,
}

impl AttackRequest {
    /// `ATTACK` (0x01), the id `encode` writes.
    pub const OPCODE: ClientOpcode = ClientOpcode::Attack;
    /// `ATTACK_REQUEST` (0x32), the second id of the same Java class.
    pub const OPCODE_REQUEST: ClientOpcode = ClientOpcode::AttackRequest;
    pub const SIMPLE_CLICK: i8 = 0;
    pub const SHIFT_CLICK: i8 = 1;
}

impl Encode for AttackRequest {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.object_id)
            .i32(self.origin_x)
            .i32(self.origin_y)
            .i32(self.origin_z)
            .i8(self.attack_id);
    }
}

impl Decode for AttackRequest {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let object_id = r.i32()?;
        let origin_x = r.i32()?;
        let origin_y = r.i32()?;
        let origin_z = r.i32()?;
        let attack_id = r.i8()?;
        Ok(Self {
            object_id,
            origin_x,
            origin_y,
            origin_z,
            attack_id,
        })
    }
}

// Java: gameserver/network/clientpackets/RequestActionUse.java
/// A click on an action button: the action id is the one the client sends, Java's `runImpl`
/// decides what it does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestActionUse {
    pub action_id: i32,
    pub ctrl_pressed: bool,
    pub shift_pressed: bool,
}

impl RequestActionUse {
    pub const OPCODE: ClientOpcode = ClientOpcode::RequestActionUse;
}

impl Encode for RequestActionUse {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.action_id)
            .i32(i32::from(self.ctrl_pressed))
            .i8(i8::from(self.shift_pressed));
    }
}

impl Decode for RequestActionUse {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let action_id = r.i32()?;
        // NOTE(java-quirk): Java compares with `== 1` for both flags (`UseItem` uses `!= 0`).
        // Any other value reads as "not pressed".
        let ctrl_pressed = r.i32()? == 1;
        let shift_pressed = r.i8()? == 1;
        Ok(Self {
            action_id,
            ctrl_pressed,
            shift_pressed,
        })
    }
}

// Java: gameserver/network/clientpackets/RequestRestartPoint.java
/// The player picks a restart point after death (village, clan hall, castle, fortress...).
///
/// The value is the raw Java `_requestedPointType`. Its meaning is decided by the `switch` in
/// Java's `runImpl`, which the death handler (M4-DEATH-01) ports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestRestartPoint {
    pub requested_point_type: i32,
}

impl RequestRestartPoint {
    pub const OPCODE: ClientOpcode = ClientOpcode::RequestRestartPoint;
}

impl Encode for RequestRestartPoint {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.requested_point_type);
    }
}

impl Decode for RequestRestartPoint {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            requested_point_type: r.i32()?,
        })
    }
}
