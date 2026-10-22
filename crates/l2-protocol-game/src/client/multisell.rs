//! Client packet of the multisell window (the NPC exchange lists).
//!
//! Only reads the packet. The list check, the ingredient checks and the item moves are the
//! world's job (`l2_world::multisell`).

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_opcode;
use crate::generated::opcodes::ClientOpcode;

// Java: gameserver/network/clientpackets/MultiSellChoose.java
/// The player picks an entry of the open multisell list (`MULTI_SELL_CHOOSE`).
///
/// Java reads the fields below `amount` and ignores them (`_unk1` to `_unk11`; the last eight
/// shorts are "elemental attributes" the client echoes). They are kept so the packet round-trips.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MultiSellChoose {
    /// The id of the list the client has open.
    pub list_id: i32,
    /// The entry id of the prepared list (`template id * 100000` + the enchant level when the list
    /// keeps enchantments).
    pub entry_id: i32,
    /// How many times to run the entry.
    pub amount: i64,
    /// Java `_unk1` (a short).
    pub unk1: i16,
    /// Java `_unk2`.
    pub unk2: i32,
    /// Java `_unk3`.
    pub unk3: i32,
    /// Java `_unk4` to `_unk11` (shorts).
    pub elemental: [i16; 8],
}

impl MultiSellChoose {
    pub const OPCODE: ClientOpcode = ClientOpcode::MultiSellChoose;

    /// A packet with the unknown fields at 0, which is what tests and tools usually want.
    pub fn new(list_id: i32, entry_id: i32, amount: i64) -> Self {
        Self {
            list_id,
            entry_id,
            amount,
            unk1: 0,
            unk2: 0,
            unk3: 0,
            elemental: [0; 8],
        }
    }
}

impl Encode for MultiSellChoose {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.list_id)
            .i32(self.entry_id)
            .i64(self.amount)
            .i16(self.unk1)
            .i32(self.unk2)
            .i32(self.unk3);
        for value in self.elemental {
            w.i16(value);
        }
    }
}

impl Decode for MultiSellChoose {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let list_id = r.i32()?;
        let entry_id = r.i32()?;
        let amount = r.i64()?;
        let unk1 = r.i16()?;
        let unk2 = r.i32()?;
        let unk3 = r.i32()?;
        let mut elemental = [0; 8];
        for value in &mut elemental {
            *value = r.i16()?;
        }
        Ok(Self {
            list_id,
            entry_id,
            amount,
            unk1,
            unk2,
            unk3,
            elemental,
        })
    }
}
