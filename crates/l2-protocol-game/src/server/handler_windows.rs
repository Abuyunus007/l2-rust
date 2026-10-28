//! Header-only packets the NPC bypass handlers send: `TutorialCloseHtml` and `ExShowQuestInfo`.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_opcode;
use crate::generated::opcodes::ServerOpcode;

// Java: gameserver/network/serverpackets/TutorialCloseHtml.java
/// Closes the tutorial window (`TUTORIAL_CLOSE_HTML`, 0xA9). No fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TutorialCloseHtml;

impl TutorialCloseHtml {
    pub const OPCODE: ServerOpcode = ServerOpcode::TutorialCloseHtml;
}

impl Encode for TutorialCloseHtml {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
    }
}

impl Decode for TutorialCloseHtml {
    fn decode(_r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self)
    }
}

// Java: gameserver/network/serverpackets/ExShowQuestInfo.java
/// Opens the quest list window (`EX_SHOW_QUEST_INFO`, 0xFE 0x0020). No fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExShowQuestInfo;

impl ExShowQuestInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExShowQuestInfo;
}

impl Encode for ExShowQuestInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
    }
}

impl Decode for ExShowQuestInfo {
    fn decode(_r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self)
    }
}

// Java: gameserver/network/serverpackets/ShowXMasSeal.java
/// Shows the Christmas seal window of an item (`SHOW_XMAS_SEAL`, 0xF8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShowXMasSeal {
    /// Java `_item`: the item template id.
    pub item_id: i32,
}

impl ShowXMasSeal {
    pub const OPCODE: ServerOpcode = ServerOpcode::ShowXmasSeal;
}

impl Encode for ShowXMasSeal {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.item_id);
    }
}

impl Decode for ShowXMasSeal {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { item_id: r.i32()? })
    }
}

// Java: gameserver/network/serverpackets/ShopPreviewList.java
/// One entry of a [`ShopPreviewList`] (the items the player may try on).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShopPreviewProduct {
    pub item_id: i32,
    /// `ItemTemplate.getType2()`.
    pub type2: i16,
    /// `getBodyPart().getMask()`, or 0 for an item of type1 4 (`TYPE1_ITEM_QUESTITEM_ADENA`).
    pub body_part: i16,
    /// `GeneralConfig.WEAR_PRICE`.
    pub price: i64,
}

/// The wear window of a shop (`SHOP_PREVIEW_LIST`, 0xF5). The caller has already left out the
/// items the player's grade does not allow (Java filters twice with the same test and writes the
/// count of the passing ones).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShopPreviewList {
    /// The player's adena.
    pub money: i64,
    pub list_id: i32,
    pub products: Vec<ShopPreviewProduct>,
}

impl ShopPreviewList {
    pub const OPCODE: ServerOpcode = ServerOpcode::ShopPreviewList;
}

impl Encode for ShopPreviewList {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        // Java: four bytes of unknown meaning (`0xC0, 0x13, 0, 0`).
        w.u8(0xC0).u8(0x13).u8(0).u8(0);
        w.i64(self.money).i32(self.list_id);
        w.u16(u16::try_from(self.products.len()).unwrap_or(u16::MAX));
        for p in &self.products {
            w.i32(p.item_id).i16(p.type2).i16(p.body_part).i64(p.price);
        }
    }
}

impl Decode for ShopPreviewList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        r.skip(4)?;
        let money = r.i64()?;
        let list_id = r.i32()?;
        let count = r.u16()?;
        let mut products = Vec::new();
        for _ in 0..count {
            products.push(ShopPreviewProduct {
                item_id: r.i32()?,
                type2: r.i16()?,
                body_part: r.i16()?,
                price: r.i64()?,
            });
        }
        Ok(Self {
            money,
            list_id,
            products,
        })
    }
}

// Java: gameserver/network/serverpackets/Dice.java
/// A thrown dice (`DICE`, 0xDA): the player, the dice item, the number and where it lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dice {
    /// Object id of the player.
    pub object_id: i32,
    /// Item id of the dice (4625 to 4628).
    pub item_id: i32,
    /// The number rolled.
    pub number: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl Dice {
    pub const OPCODE: ServerOpcode = ServerOpcode::Dice;
}

impl Encode for Dice {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.object_id);
        w.i32(self.item_id);
        w.i32(self.number);
        w.i32(self.x);
        w.i32(self.y);
        w.i32(self.z);
    }
}

impl Decode for Dice {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            object_id: r.i32()?,
            item_id: r.i32()?,
            number: r.i32()?,
            x: r.i32()?,
            y: r.i32()?,
            z: r.i32()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shop_preview_list_layout() {
        let packet = ShopPreviewList {
            money: 1000,
            list_id: 300_001,
            products: vec![ShopPreviewProduct {
                item_id: 57,
                type2: 1,
                body_part: 0x4000,
                price: 10,
            }],
        };
        let bytes = packet.to_bytes();
        assert_eq!(bytes[0], 0xF5);
        assert_eq!(&bytes[1..5], &[0xC0, 0x13, 0, 0]);
        assert_eq!(bytes.len(), 1 + 4 + 8 + 4 + 2 + (4 + 2 + 2 + 8));
        assert_eq!(
            ShopPreviewList::decode(&mut PacketReader::new(&bytes[1..])).unwrap(),
            packet
        );
    }

    #[test]
    fn dice_is_six_ints() {
        let packet = Dice {
            object_id: 0x1000_0001,
            item_id: 4625,
            number: 6,
            x: -71_338,
            y: 258_271,
            z: -3_104,
        };
        let bytes = packet.to_bytes();
        assert_eq!(bytes.len(), 1 + 6 * 4);
        assert_eq!(bytes[0], 0xDA);
        assert_eq!(&bytes[1..5], &0x1000_0001_i32.to_le_bytes());
        assert_eq!(&bytes[9..13], &6_i32.to_le_bytes());
        assert_eq!(Dice::decode(&mut PacketReader::new(&bytes[1..])).unwrap(), packet);
    }

    #[test]
    fn tutorial_close_is_one_byte() {
        assert_eq!(TutorialCloseHtml.to_bytes(), vec![0xA9]);
        assert_eq!(
            TutorialCloseHtml::decode(&mut PacketReader::new(&[])).unwrap(),
            TutorialCloseHtml
        );
    }

    #[test]
    fn xmas_seal_is_the_item_id() {
        let packet = ShowXMasSeal { item_id: 5555 };
        let bytes = packet.to_bytes();
        assert_eq!(bytes, vec![0xF8, 0xB3, 0x15, 0, 0]);
        assert_eq!(
            ShowXMasSeal::decode(&mut PacketReader::new(&bytes[1..])).unwrap(),
            packet
        );
    }

    #[test]
    fn show_quest_info_is_the_ex_header() {
        assert_eq!(ExShowQuestInfo.to_bytes(), vec![0xFE, 0x20, 0x00]);
        assert_eq!(
            ExShowQuestInfo::decode(&mut PacketReader::new(&[])).unwrap(),
            ExShowQuestInfo
        );
    }
}
