//! The regeneration bar packet of heal-over-time skills.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_opcode;
use crate::generated::opcodes::ServerOpcode;

// Java: gameserver/network/serverpackets/ExRegenMax.java
/// The client's HP regeneration bar of a heal-over-time skill (`EX_REGEN_MAX`, ex id `0x01`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExRegenMax {
    /// Seconds the effect lasts (Java `_time`, the skill's abnormal time).
    pub time: i32,
    /// Java `_tickInterval`: the effect's `ticks`.
    pub tick_interval: i32,
    /// Java `_amountPerTick`: the effect's `power`.
    pub amount_per_tick: f64,
}

impl ExRegenMax {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExRegenMax;
}

impl Encode for ExRegenMax {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        // The constant `1` Java writes first (the client's "count").
        w.i32(1)
            .i32(self.time)
            .i32(self.tick_interval)
            .f64(self.amount_per_tick);
    }
}

impl Decode for ExRegenMax {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        r.skip(4)?;
        Ok(Self {
            time: r.i32()?,
            tick_interval: r.i32()?,
            amount_per_tick: r.f64()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ex_regen_max_layout_and_round_trip() {
        let packet = ExRegenMax {
            time: 15,
            tick_interval: 3,
            amount_per_tick: 2.5,
        };
        let mut w = PacketWriter::new();
        packet.encode(&mut w);
        let bytes = w.into_bytes();
        let mut expected = vec![0xFE, 0x01, 0x00];
        expected.extend_from_slice(&1_i32.to_le_bytes());
        expected.extend_from_slice(&15_i32.to_le_bytes());
        expected.extend_from_slice(&3_i32.to_le_bytes());
        expected.extend_from_slice(&2.5_f64.to_le_bytes());
        assert_eq!(bytes, expected);
        // After the opcode (3 bytes) the fields decode back.
        let mut r = PacketReader::new(&bytes[3..]);
        assert_eq!(ExRegenMax::decode(&mut r).unwrap(), packet);
    }
}
