//! Server packets of augmentation: the two windows and what they answer.
//!
//! Values only, like the other server packets: the world works out the numbers.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_opcode;
use crate::generated::opcodes::ServerOpcode;

// Java: gameserver/network/serverpackets/ExShowVariationMakeWindow.java
server_unit_packet!(
    /// Opens the augmentation window (`EX_SHOW_VARIATION_MAKE_WINDOW`).
    ExShowVariationMakeWindow,
    ExShowVariationMakeWindow
);

// Java: gameserver/network/serverpackets/ExShowVariationCancelWindow.java
server_unit_packet!(
    /// Opens the augmentation removal window (`EX_SHOW_VARIATION_CANCEL_WINDOW`).
    ExShowVariationCancelWindow,
    ExShowVariationCancelWindow
);

// Java: gameserver/network/serverpackets/ExVariationResult.java
server_packet!(
    /// The result of the augmentation (`EX_VARIATION_RESULT`): the two option ids and 1 for
    /// success, all 0 for a failure. Java names the constructor arguments `unk1`, `unk2`, `unk3`
    /// and the fields `stat12`, `stat34`, `unk3`.
    ExVariationResult,
    ExVariationResult {
        stat12: i32,
        stat34: i32,
        result: i32,
    }
);

// Java: gameserver/network/serverpackets/ExVariationCancelResult.java
server_packet!(
    /// The result of the removal (`EX_VARIATION_CANCEL_RESULT`): 1 removed, 0 not.
    ExVariationCancelResult,
    ExVariationCancelResult {
        result: i32,
    }
);

// Java: gameserver/network/serverpackets/ExPutItemResultForVariationMake.java
/// The item in the augmentation window is acceptable (`EX_PUT_ITEM_RESULT_FOR_VARIATION_MAKE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExPutItemResultForVariationMake {
    pub item_object_id: i32,
    /// The template id (`item.getId()`).
    pub item_id: i32,
}

impl ExPutItemResultForVariationMake {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExPutItemResultForVariationMake;
}

impl Encode for ExPutItemResultForVariationMake {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        // The last int is the constant 1.
        w.i32(self.item_object_id).i32(self.item_id).i32(1);
    }
}

impl Decode for ExPutItemResultForVariationMake {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let item_object_id = r.i32()?;
        let item_id = r.i32()?;
        let _one = r.i32()?;
        Ok(Self {
            item_object_id,
            item_id,
        })
    }
}

// Java: gameserver/network/serverpackets/ExPutIntensiveResultForVariationMake.java
/// The life stone in the window is acceptable, and the gemstones it needs
/// (`EX_PUT_INTENSIVE_RESULT_FOR_VARIATION_MAKE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExPutIntensiveResultForVariationMake {
    pub refiner_item_object_id: i32,
    pub life_stone_id: i32,
    pub gemstone_item_id: i32,
    /// Java declares the field `int` and writes it with `writeLong`.
    pub gemstone_count: i64,
}

impl ExPutIntensiveResultForVariationMake {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExPutIntensiveResultForVariationMake;
}

impl Encode for ExPutIntensiveResultForVariationMake {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.refiner_item_object_id)
            .i32(self.life_stone_id)
            .i32(self.gemstone_item_id)
            .i64(self.gemstone_count)
            // `_unk2 = 1`.
            .i32(1);
    }
}

impl Decode for ExPutIntensiveResultForVariationMake {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let refiner_item_object_id = r.i32()?;
        let life_stone_id = r.i32()?;
        let gemstone_item_id = r.i32()?;
        let gemstone_count = r.i64()?;
        let _one = r.i32()?;
        Ok(Self {
            refiner_item_object_id,
            life_stone_id,
            gemstone_item_id,
            gemstone_count,
        })
    }
}

// Java: gameserver/network/serverpackets/ExPutCommissionResultForVariationMake.java
/// The gemstones in the window are right (`EX_PUT_COMMISSION_RESULT_FOR_VARIATION_MAKE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExPutCommissionResultForVariationMake {
    pub gemstone_object_id: i32,
    /// The template id of the gemstone.
    pub item_id: i32,
    pub gemstone_count: i64,
}

impl ExPutCommissionResultForVariationMake {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExPutCommissionResultForVariationMake;
}

impl Encode for ExPutCommissionResultForVariationMake {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.gemstone_object_id)
            .i32(self.item_id)
            .i64(self.gemstone_count)
            // `_unk1 = 0`, `_unk2 = 0`, `_unk3 = 1`.
            .i32(0)
            .i32(0)
            .i32(1);
    }
}

impl Decode for ExPutCommissionResultForVariationMake {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let gemstone_object_id = r.i32()?;
        let item_id = r.i32()?;
        let gemstone_count = r.i64()?;
        let (_unk1, _unk2, _unk3) = (r.i32()?, r.i32()?, r.i32()?);
        Ok(Self {
            gemstone_object_id,
            item_id,
            gemstone_count,
        })
    }
}

// Java: gameserver/network/serverpackets/ExPutItemResultForVariationCancel.java
/// The augmented item in the removal window and the price
/// (`EX_PUT_ITEM_RESULT_FOR_VARIATION_CANCEL`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExPutItemResultForVariationCancel {
    pub item_object_id: i32,
    /// The display id (`item.getDisplayId()`).
    pub item_id: i32,
    /// The first option id: `(short) augmentationId`, sign extended.
    pub aug1: i32,
    /// The second option id: `augmentationId >> 16`.
    pub aug2: i32,
    /// Java declares the field `int` and writes it with `writeLong`.
    pub price: i64,
}

impl ExPutItemResultForVariationCancel {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExPutItemResultForVariationCancel;
}

impl Encode for ExPutItemResultForVariationCancel {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.item_object_id)
            .i32(self.item_id)
            .i32(self.aug1)
            .i32(self.aug2)
            .i64(self.price)
            // The last int is the constant 1.
            .i32(1);
    }
}

impl Decode for ExPutItemResultForVariationCancel {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let item_object_id = r.i32()?;
        let item_id = r.i32()?;
        let aug1 = r.i32()?;
        let aug2 = r.i32()?;
        let price = r.i64()?;
        let _one = r.i32()?;
        Ok(Self {
            item_object_id,
            item_id,
            aug1,
            aug2,
            price,
        })
    }
}
