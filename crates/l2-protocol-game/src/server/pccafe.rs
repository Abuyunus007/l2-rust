//! The PC cafe packet: `ExPCCafePointInfo`.
//!
//! Java: `gameserver/network/serverpackets/ExPCCafePointInfo.java`.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_opcode;
use crate::generated::opcodes::ServerOpcode;

// Java: gameserver/network/serverpackets/ExPCCafePointInfo.java
/// The PC cafe points of the player and the change that just happened (`EX_PC_CAFE_POINT_INFO`).
/// The fields are what goes on the wire; [`ExPcCafePointInfo::new`] fills them like Java's
/// `ExPCCafePointInfo(points, pointsToAdd, time)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ExPcCafePointInfo {
    /// `_points`: the points the player has now.
    pub points: i32,
    /// `_mAddPoint`: the points shown as gained (positive) or spent (negative).
    pub add_point: i32,
    /// `_mPeriodType`: 0 don't show the window, 1 acquisition, 2 use points.
    pub period_type: u8,
    /// `_remainTime`: "period hours left".
    pub remain_time: i32,
    /// `_pointType`: the colour of the shown change (0 yellow, 1 cyan-blue, 2 red, else black).
    pub point_type: u8,
    /// `_time * 3`: "value is in seconds * 3".
    pub time: i32,
}

impl ExPcCafePointInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExPcCafePointInfo;

    /// Java `new ExPCCafePointInfo(points, pointsToAdd, time)`.
    pub fn new(points: i32, points_to_add: i32, time: i32) -> Self {
        Self {
            points,
            add_point: points_to_add,
            period_type: 1,
            // "No idea why but retail sends 42.."
            remain_time: 42,
            // "When using points is 3".
            point_type: if points_to_add < 0 { 3 } else { 0 },
            time: time.wrapping_mul(3),
        }
    }
}

impl Encode for ExPcCafePointInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.points);
        w.i32(self.add_point);
        w.u8(self.period_type);
        w.i32(self.remain_time);
        w.u8(self.point_type);
        w.i32(self.time);
    }
}

impl Decode for ExPcCafePointInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            points: r.i32()?,
            add_point: r.i32()?,
            period_type: r.u8()?,
            remain_time: r.i32()?,
            point_type: r.u8()?,
            time: r.i32()?,
        })
    }
}
