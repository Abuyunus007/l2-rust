//! Server packets of fences and door effects: `ExColosseumFenceInfo` and `OnEventTrigger`.
//!
//! Values only, like [`super::interaction`]: the world passes in what Java reads from the fence
//! and door objects.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_opcode;
use crate::generated::opcodes::ServerOpcode;

// Java: gameserver/network/serverpackets/ExColosseumFenceInfo.java
/// Shows one fence piece (`EX_COLOSSEUM_FENCE_INFO`). A fence with `height > 1` sends one packet
/// for the fence object and one per extra height id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExColosseumFenceInfo {
    pub object_id: i32,
    /// `FenceState.getClientId()`: 0 hidden, 1 opened, 2 closed.
    pub state: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub width: i32,
    pub length: i32,
}

impl ExColosseumFenceInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExColosseumFenceInfo;
}

impl Encode for ExColosseumFenceInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.object_id)
            .i32(self.state)
            .i32(self.x)
            .i32(self.y)
            .i32(self.z)
            .i32(self.width)
            .i32(self.length);
    }
}

impl Decode for ExColosseumFenceInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            object_id: r.i32()?,
            state: r.i32()?,
            x: r.i32()?,
            y: r.i32()?,
            z: r.i32()?,
            width: r.i32()?,
            length: r.i32()?,
        })
    }
}

// Java: gameserver/network/serverpackets/OnEventTrigger.java
/// Switches a client-side effect emitter on or off (`EVENT_TRIGGER`); doors with an `emitter_id`
/// send it with `enabled = !open`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OnEventTrigger {
    pub emitter_id: i32,
    pub enabled: bool,
}

impl OnEventTrigger {
    pub const OPCODE: ServerOpcode = ServerOpcode::EventTrigger;
}

impl Encode for OnEventTrigger {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.emitter_id).u8(u8::from(self.enabled));
    }
}

impl Decode for OnEventTrigger {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            emitter_id: r.i32()?,
            enabled: r.u8()? != 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fence_info_layout_and_round_trip() {
        let packet = ExColosseumFenceInfo {
            object_id: 0x0102_0304,
            state: 2,
            x: -114_552,
            y: -251_256,
            z: -2_992,
            width: 100,
            length: 200,
        };
        let bytes = packet.to_bytes();
        // 0xFE, ex id 0x0003, then seven little-endian ints in Java's writeImpl order.
        let mut expected = vec![0xFE, 0x03, 0x00];
        for v in [0x0102_0304_i32, 2, -114_552, -251_256, -2_992, 100, 200] {
            expected.extend(v.to_le_bytes());
        }
        assert_eq!(bytes, expected);
        let back = ExColosseumFenceInfo::decode(&mut PacketReader::new(&bytes[3..])).unwrap();
        assert_eq!(back, packet);
    }

    #[test]
    fn event_trigger_layout_and_round_trip() {
        let packet = OnEventTrigger {
            emitter_id: 21_100_001,
            enabled: true,
        };
        let bytes = packet.to_bytes();
        let mut expected = vec![0xCF];
        expected.extend(21_100_001_i32.to_le_bytes());
        expected.push(1);
        assert_eq!(bytes, expected);
        let back = OnEventTrigger::decode(&mut PacketReader::new(&bytes[1..])).unwrap();
        assert_eq!(back, packet);
        assert_eq!(
            *OnEventTrigger {
                emitter_id: 1,
                enabled: false
            }
            .to_bytes()
            .last()
            .unwrap(),
            0
        );
    }
}
