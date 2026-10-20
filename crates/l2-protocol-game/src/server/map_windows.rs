//! The two packets that open a client window from an item: the map and the calculator.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_opcode;
use crate::generated::opcodes::ServerOpcode;

// Java: gameserver/network/serverpackets/ShowMiniMap.java
/// Opens the mini map (`SHOW_MINIMAP`, 0xA3) for a map item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShowMiniMap {
    /// Java `_mapId`: the id of the map item.
    pub map_id: i32,
    /// Java `SevenSigns.getInstance().getCurrentPeriod()`: the client colours the seals by it.
    pub seven_signs_period: u8,
}

impl ShowMiniMap {
    pub const OPCODE: ServerOpcode = ServerOpcode::ShowMinimap;
}

impl Encode for ShowMiniMap {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.map_id).u8(self.seven_signs_period);
    }
}

impl Decode for ShowMiniMap {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            map_id: r.i32()?,
            seven_signs_period: r.u8()?,
        })
    }
}

// Java: gameserver/network/serverpackets/ShowCalculator.java
/// Opens the calculator (`SHOW_CALC`, 0xE2) for a calculator item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShowCalculator {
    /// Java `_calculatorId`: the id of the calculator item.
    pub calculator_id: i32,
}

impl ShowCalculator {
    pub const OPCODE: ServerOpcode = ServerOpcode::ShowCalc;
}

impl Encode for ShowCalculator {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.calculator_id);
    }
}

impl Decode for ShowCalculator {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            calculator_id: r.i32()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes_of(packet: &impl Encode) -> Vec<u8> {
        let mut w = PacketWriter::new();
        packet.encode(&mut w);
        w.into_bytes()
    }

    #[test]
    fn show_mini_map_layout_and_round_trip() {
        let packet = ShowMiniMap {
            map_id: 1665,
            seven_signs_period: 3,
        };
        let bytes = bytes_of(&packet);
        // 0xA3, the map id (little endian), the period byte.
        assert_eq!(bytes, [0xA3, 0x81, 0x06, 0x00, 0x00, 0x03]);
        assert_eq!(
            ShowMiniMap::decode(&mut PacketReader::new(&bytes[1..])).unwrap(),
            packet
        );
    }

    #[test]
    fn show_calculator_layout_and_round_trip() {
        let packet = ShowCalculator { calculator_id: 4393 };
        let bytes = bytes_of(&packet);
        // 0xE2, the item id (little endian).
        assert_eq!(bytes, [0xE2, 0x29, 0x11, 0x00, 0x00]);
        assert_eq!(
            ShowCalculator::decode(&mut PacketReader::new(&bytes[1..])).unwrap(),
            packet
        );
    }
}
