//! Client packets of the symbol maker (hennas): the lists, the previews, draw and remove (L-01).
//!
//! These only read the packet. The rules are the world's job (`l2_world::henna`).

use l2_net::{Decode, Encode, PacketError, PacketReader};

use super::write_opcode;
use crate::generated::opcodes::ClientOpcode;

/// Declares a henna packet that carries one int and nothing else Java uses.
macro_rules! one_int_packet {
    ($(#[$meta:meta])* $name:ident, $field:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct $name {
            pub $field: i32,
        }

        impl $name {
            pub const OPCODE: ClientOpcode = ClientOpcode::$name;
        }

        impl Encode for $name {
            fn encode(&self, w: &mut l2_net::PacketWriter) {
                write_opcode(w, Self::OPCODE);
                w.i32(self.$field);
            }
        }

        impl Decode for $name {
            fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
                Ok(Self { $field: r.i32()? })
            }
        }
    };
}

one_int_packet!(
    // Java: gameserver/network/clientpackets/RequestHennaItemList.java
    /// Open the list of dyes that can be drawn (`REQUEST_HENNA_ITEM_LIST`). Java reads one int
    /// and ignores it ("Unknown").
    RequestHennaItemList,
    unknown
);
one_int_packet!(
    // Java: gameserver/network/clientpackets/RequestHennaItemInfo.java
    /// Preview of drawing a dye (`REQUEST_HENNA_ITEM_INFO`).
    RequestHennaItemInfo,
    symbol_id
);
one_int_packet!(
    // Java: gameserver/network/clientpackets/RequestHennaEquip.java
    /// Draw a dye (`REQUEST_HENNA_EQUIP`).
    RequestHennaEquip,
    symbol_id
);
one_int_packet!(
    // Java: gameserver/network/clientpackets/RequestHennaRemoveList.java
    /// Open the list of worn dyes (`REQUEST_HENNA_REMOVE_LIST`). Java reads one int and ignores
    /// it ("Unknown").
    RequestHennaRemoveList,
    unknown
);
one_int_packet!(
    // Java: gameserver/network/clientpackets/RequestHennaItemRemoveInfo.java
    /// Preview of removing a dye (`REQUEST_HENNA_ITEM_REMOVE_INFO`).
    RequestHennaItemRemoveInfo,
    symbol_id
);
one_int_packet!(
    // Java: gameserver/network/clientpackets/RequestHennaRemove.java
    /// Remove a dye (`REQUEST_HENNA_REMOVE`).
    RequestHennaRemove,
    symbol_id
);

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip<T: Encode + Decode + PartialEq + std::fmt::Debug>(packet: &T, opcode: u8) {
        let bytes = packet.to_bytes();
        assert_eq!(bytes[0], opcode);
        assert_eq!(bytes.len(), 5);
        let decoded = T::decode(&mut PacketReader::new(&bytes[1..])).unwrap();
        assert_eq!(&decoded, packet);
        assert!(T::decode(&mut PacketReader::new(&bytes[1..4])).is_err());
    }

    #[test]
    fn layouts_and_round_trips() {
        round_trip(&RequestHennaItemList { unknown: 0 }, 0xC3);
        round_trip(&RequestHennaItemInfo { symbol_id: 7 }, 0xC4);
        round_trip(&RequestHennaEquip { symbol_id: 300 }, 0x6F);
        round_trip(&RequestHennaRemoveList { unknown: 1 }, 0x70);
        round_trip(&RequestHennaItemRemoveInfo { symbol_id: -2 }, 0x71);
        round_trip(&RequestHennaRemove { symbol_id: 12 }, 0x72);
        assert_eq!(RequestHennaEquip { symbol_id: 300 }.to_bytes(), [0x6F, 0x2C, 1, 0, 0]);
    }
}
