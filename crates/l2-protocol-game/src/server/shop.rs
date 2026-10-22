//! NPC shop server packets: the merchant's buy list and the sell/refund lists of the player.
//!
//! Java sends both with the same id (`EX_BUY_SELL_LIST`, `0xFE 0x00B7`); the first int tells them
//! apart: `0` is the merchant's list ([`BuyList`]), `1` is the player's side ([`ExBuySellList`]).
//! Same rules as [`super::skills`]: values only. The prices are what the client shows, the
//! caller (`l2_world::shop`) applies the tax and the siege guard rate.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::items::{ItemInfo, read_item, write_item};
use super::write_opcode;
use crate::generated::opcodes::ServerOpcode;

/// The two values of the first int of `EX_BUY_SELL_LIST`.
const KIND_BUY: i32 = 0;
const KIND_SELL: i32 = 1;

/// Bytes of one [`BuyListProduct`] on the wire.
const PRODUCT_LEN: usize = 76;

// Java: gameserver/network/serverpackets/BuyList.java (the fields of one product)
/// One product of a buy list as `BuyList.writeImpl` writes it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BuyListProduct {
    /// `product.getItemId()`; Java writes the template id here, not the display id.
    pub item_id: i32,
    /// `product.getCount()` (0 for an unlimited product; never negative).
    pub count: i64,
    /// `getItem().getType2()`.
    pub type2: i16,
    /// `getItem().getType1()`.
    pub type1: i16,
    /// `getItem().getBodyPart().getMask()`.
    pub body_part: i32,
    /// `getItem().getDefaultEnchantLevel()` (low 16 bits).
    pub enchant: i16,
    /// The price the client pays, with the tax applied.
    pub price: i64,
}

// Java: gameserver/network/serverpackets/BuyList.java
/// The merchant's buy window (`EX_BUY_SELL_LIST` with `0` in front).
///
/// NOTE(java-quirk): `size` is `list.getProducts().size()`, which also counts the products Java
/// leaves out because their stock is used up. The client reads `size` entries and gets fewer;
/// [`BuyList::new`] copies Java's number, [`Decode`] reads until the packet ends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuyList {
    /// The player's adena.
    pub money: i64,
    pub list_id: i32,
    /// The short Java writes in front of the entries.
    pub size: u16,
    pub products: Vec<BuyListProduct>,
}

impl BuyList {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExBuySellList;

    /// A list whose `size` is the number of entries.
    pub fn new(money: i64, list_id: i32, products: Vec<BuyListProduct>) -> Self {
        Self {
            money,
            list_id,
            size: u16::try_from(products.len()).unwrap_or(u16::MAX),
            products,
        }
    }
}

impl Encode for BuyList {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(KIND_BUY).i64(self.money).i32(self.list_id).u16(self.size);
        for p in &self.products {
            w.i32(p.item_id).i32(p.item_id).i32(0);
            w.i64(p.count);
            w.i16(p.type2).i16(p.type1).i16(0); // isEquipped
            w.i32(p.body_part);
            w.i16(p.enchant).i16(0); // custom type
            w.i32(0).i32(-1).i32(-9999); // augment, mana, time
            w.i16(0).i16(0); // element type, element power
            for _ in 0..6 {
                w.i16(0);
            }
            // Enchant effects.
            w.i16(0).i16(0).i16(0);
            w.i64(p.price);
        }
    }
}

impl Decode for BuyList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let kind = r.i32()?;
        if kind != KIND_BUY {
            return Err(PacketError::invalid(
                "buy list",
                format!("kind {kind}, expected 0"),
            ));
        }
        let money = r.i64()?;
        let list_id = r.i32()?;
        let size = r.u16()?;
        let mut products = Vec::new();
        while r.remaining() >= PRODUCT_LEN {
            let item_id = r.i32()?;
            r.skip(8)?; // the item id again, 0
            let count = r.i64()?;
            let type2 = r.i16()?;
            let type1 = r.i16()?;
            r.skip(2)?;
            let body_part = r.i32()?;
            let enchant = r.i16()?;
            r.skip(2 + 12 + 4 + 12 + 6)?;
            let price = r.i64()?;
            products.push(BuyListProduct {
                item_id,
                count,
                type2,
                type1,
                body_part,
                enchant,
                price,
            });
        }
        Ok(Self {
            money,
            list_id,
            size,
            products,
        })
    }
}

/// An item the player can sell, with the price the merchant pays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SellEntry {
    pub item: ItemInfo,
    /// `getReferencePrice() / 2`, or 0 with `MerchantZeroSellPrice`.
    pub price: i64,
}

/// An item in the refund list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefundEntry {
    pub item: ItemInfo,
    /// The index the client sends back in `RequestRefundItem`.
    pub index: i32,
    /// `(getReferencePrice() / 2) * count`, or 0 with `MerchantZeroSellPrice`.
    pub price: i64,
}

// Java: gameserver/network/serverpackets/ExBuySellList.java
/// The sell and refund tabs of a shop window (`EX_BUY_SELL_LIST` with `1` in front).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExBuySellList {
    pub sell_list: Vec<SellEntry>,
    pub refund_list: Vec<RefundEntry>,
    /// `true`: a transaction just finished.
    pub done: bool,
}

impl ExBuySellList {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExBuySellList;
}

impl Encode for ExBuySellList {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(KIND_SELL).u16(self.sell_list.len() as u16);
        for entry in &self.sell_list {
            write_item(w, &entry.item);
            w.i64(entry.price);
        }
        // Java writes the refund count only for a list that is not empty, else `0`.
        w.u16(self.refund_list.len() as u16);
        for entry in &self.refund_list {
            write_item(w, &entry.item);
            w.i32(entry.index).i64(entry.price);
        }
        w.u8(u8::from(self.done));
    }
}

impl Decode for ExBuySellList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let kind = r.i32()?;
        if kind != KIND_SELL {
            return Err(PacketError::invalid(
                "sell list",
                format!("kind {kind}, expected 1"),
            ));
        }
        let sell_count = usize::from(r.u16()?);
        let mut sell_list = Vec::new();
        for _ in 0..sell_count {
            sell_list.push(SellEntry {
                item: read_item(r)?,
                price: r.i64()?,
            });
        }
        let refund_count = usize::from(r.u16()?);
        let mut refund_list = Vec::new();
        for _ in 0..refund_count {
            refund_list.push(RefundEntry {
                item: read_item(r)?,
                index: r.i32()?,
                price: r.i64()?,
            });
        }
        let done = r.u8()? != 0;
        Ok(Self {
            sell_list,
            refund_list,
            done,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buy_list_layout_and_round_trip() {
        let list = BuyList {
            money: 1_000_000,
            list_id: 382,
            // Java counts the sold-out products in the size.
            size: 3,
            products: vec![BuyListProduct {
                item_id: 1835,
                count: 0,
                type2: 5,
                type1: 4,
                body_part: 0x0080,
                enchant: 2,
                price: 25,
            }],
        };
        let bytes = list.to_bytes();
        // 0xFE 0x00B7, kind 0, money, list id, size, then one 76 byte product.
        assert_eq!(&bytes[..3], [0xFE, 0xB7, 0x00]);
        assert_eq!(bytes.len(), 3 + 4 + 8 + 4 + 2 + PRODUCT_LEN);
        assert_eq!(&bytes[3..7], 0_i32.to_le_bytes());
        assert_eq!(&bytes[7..15], 1_000_000_i64.to_le_bytes());
        assert_eq!(&bytes[15..19], 382_i32.to_le_bytes());
        assert_eq!(&bytes[19..21], 3_u16.to_le_bytes());
        let product = &bytes[21..];
        // The item id twice, then an int 0, then the count.
        assert_eq!(&product[0..4], 1835_i32.to_le_bytes());
        assert_eq!(&product[4..8], 1835_i32.to_le_bytes());
        assert_eq!(&product[8..12], [0; 4]);
        assert_eq!(&product[12..20], [0; 8]);
        // type2, type1, equipped.
        assert_eq!(&product[20..26], [5, 0, 4, 0, 0, 0]);
        // body part, enchant, custom type.
        assert_eq!(&product[26..30], 0x80_i32.to_le_bytes());
        assert_eq!(&product[30..34], [2, 0, 0, 0]);
        // augment 0, mana -1, time -9999.
        assert_eq!(&product[34..38], [0; 4]);
        assert_eq!(&product[38..42], (-1_i32).to_le_bytes());
        assert_eq!(&product[42..46], (-9999_i32).to_le_bytes());
        // 2 element shorts, 6 defense shorts, 3 enchant effect shorts: all zero.
        assert_eq!(&product[46..68], [0; 22]);
        assert_eq!(&product[68..76], 25_i64.to_le_bytes());

        let decoded = BuyList::decode(&mut PacketReader::new(&bytes[3..])).unwrap();
        assert_eq!(decoded, list);
    }

    #[test]
    fn sell_list_layout_and_round_trip() {
        let item = ItemInfo::plain(0x1000_0001, 57, 500);
        let list = ExBuySellList {
            sell_list: vec![SellEntry {
                item: item.clone(),
                price: 7,
            }],
            refund_list: vec![RefundEntry {
                item: item.clone(),
                index: 0,
                price: 14,
            }],
            done: true,
        };
        let bytes = list.to_bytes();
        assert_eq!(&bytes[..3], [0xFE, 0xB7, 0x00]);
        assert_eq!(&bytes[3..7], 1_i32.to_le_bytes());
        assert_eq!(&bytes[7..9], 1_u16.to_le_bytes());
        // Last byte: the done flag.
        assert_eq!(bytes.last(), Some(&1));
        let decoded = ExBuySellList::decode(&mut PacketReader::new(&bytes[3..])).unwrap();
        assert_eq!(decoded, list);

        // No refund items: a short 0 and nothing else.
        let empty = ExBuySellList {
            sell_list: Vec::new(),
            refund_list: Vec::new(),
            done: false,
        };
        let bytes = empty.to_bytes();
        assert_eq!(bytes, [0xFE, 0xB7, 0x00, 1, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(
            ExBuySellList::decode(&mut PacketReader::new(&bytes[3..])).unwrap(),
            empty
        );
    }

    #[test]
    fn the_wrong_kind_is_rejected() {
        let sell = ExBuySellList {
            sell_list: Vec::new(),
            refund_list: Vec::new(),
            done: false,
        };
        let bytes = sell.to_bytes();
        assert!(BuyList::decode(&mut PacketReader::new(&bytes[3..])).is_err());
        let buy = BuyList::new(0, 1, Vec::new());
        let bytes = buy.to_bytes();
        assert!(ExBuySellList::decode(&mut PacketReader::new(&bytes[3..])).is_err());
    }
}
