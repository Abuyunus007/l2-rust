//! Client packets of the manor: the manor list, the crop sales to a manor, the seed and crop
//! settings of the lord and the seed purchase.
//!
//! Like the warehouse lists ([`super::warehouse`]), the list packets keep Java's "bad packet" rule:
//! when the count is out of range, the byte length does not match the count, or one entry has a
//! nonsense value, Java reads no list. That is `items: None` (or an empty list for the two setting
//! packets, where Java's `runImpl` returns on an empty list too). Java also rejects a count above
//! `PlayerConfig.MAX_ITEM_IN_PACKET`; that limit comes from the configuration, so the world
//! compares `items.len()` with it and treats a longer list as bad.
//!
//! Who may do what (manor period, castle owner, `ClanAccess.CASTLE_MANOR`, the folk NPC, prices,
//! adena) is the world's job (M7 manor cards).

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::{write_ex_opcode, write_opcode};
use crate::generated::opcodes::{ClientOpcode, ExClientOpcode};

/// Reads the `count` of a list packet, Java's first check: `None` when Java would read no list, a
/// count below 1 or a byte length that is not exactly `count * batch`. The
/// `MAX_ITEM_IN_PACKET` check is the world's (see the module notes).
fn read_list_count(r: &mut PacketReader, batch: usize) -> Result<Option<usize>, PacketError> {
    let count = r.i32()?;
    // Java multiplies in `int`; a count that wraps would never match the length, so the product
    // is computed without wrapping here.
    let expected = i64::from(count) * batch as i64;
    if count < 1 || expected != r.remaining() as i64 {
        return Ok(None);
    }
    Ok(usize::try_from(count).ok())
}

// Java: gameserver/network/clientpackets/RequestManorList.java
client_unit_packet!(
    /// Asks for the manor list, which the server answers with the default crop table
    /// (`0xD0` 0x0001, `REQUEST_MANOR_LIST`).
    RequestManorList,
    ex RequestManorList
);

/// One crop a player sells to a manor (Java `CropHolder`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CropSaleItem {
    /// The object id of the crop item in the player's inventory.
    pub object_id: i32,
    /// The crop item id.
    pub item_id: i32,
    /// The manor the crop goes to (the castle id, not the crop).
    pub manor_id: i32,
    /// How many crops.
    pub count: i64,
}

// Java: gameserver/network/clientpackets/RequestProcureCropList.java
/// The player sells crops to the manor (`0xD0` 0x0002, `REQUEST_PROCURE_CROP_LIST`, Java name
/// `RequestProcureCropList`). Each entry is 20 bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestProcureCropList {
    /// `None`: Java's bad list (see the module notes).
    pub items: Option<Vec<CropSaleItem>>,
}

impl RequestProcureCropList {
    pub const EX_OPCODE: ExClientOpcode = ExClientOpcode::RequestProcedureCropList;
}

impl Encode for RequestProcureCropList {
    fn encode(&self, w: &mut PacketWriter) {
        write_ex_opcode(w, Self::EX_OPCODE);
        w.i32(self.items.as_ref().map_or(0, |v| v.len() as i32));
        for item in self.items.iter().flatten() {
            w.i32(item.object_id)
                .i32(item.item_id)
                .i32(item.manor_id)
                .i64(item.count);
        }
    }
}

impl Decode for RequestProcureCropList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let Some(count) = read_list_count(r, 20)? else {
            return Ok(Self { items: None });
        };
        let mut items = Vec::with_capacity(count);
        for _ in 0..count {
            let item = CropSaleItem {
                object_id: r.i32()?,
                item_id: r.i32()?,
                manor_id: r.i32()?,
                count: r.i64()?,
            };
            if item.object_id < 1 || item.item_id < 1 || item.manor_id < 0 || item.count < 0 {
                return Ok(Self { items: None });
            }
            items.push(item);
        }
        Ok(Self { items: Some(items) })
    }
}

/// One seed the lord puts up for sale in the next period (Java `SeedProduction` as read by
/// `RequestSetSeed`). Only entries with `sales > 0` are kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeedSale {
    pub item_id: i32,
    /// The amount of seeds for sale (the start amount, which is also the limit).
    pub sales: i64,
    /// The price of one seed.
    pub price: i64,
}

// Java: gameserver/network/clientpackets/RequestSetSeed.java
/// The lord sets the seed production of the next period (`0xD0` 0x0003, `REQUEST_SET_SEED`).
/// Each entry is 20 bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestSetSeed {
    /// The castle id.
    pub manor_id: i32,
    /// The kept entries. Empty for a bad list (Java returns on an empty list too).
    pub items: Vec<SeedSale>,
}

impl RequestSetSeed {
    pub const EX_OPCODE: ExClientOpcode = ExClientOpcode::RequestSetSeed;
}

impl Encode for RequestSetSeed {
    fn encode(&self, w: &mut PacketWriter) {
        write_ex_opcode(w, Self::EX_OPCODE);
        w.i32(self.manor_id).i32(self.items.len() as i32);
        for item in &self.items {
            w.i32(item.item_id).i64(item.sales).i64(item.price);
        }
    }
}

impl Decode for RequestSetSeed {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let manor_id = r.i32()?;
        let Some(count) = read_list_count(r, 20)? else {
            return Ok(Self {
                manor_id,
                items: Vec::new(),
            });
        };
        let mut items = Vec::with_capacity(count);
        for _ in 0..count {
            let item_id = r.i32()?;
            let sales = r.i64()?;
            let price = r.i64()?;
            if item_id < 1 || sales < 0 || price < 0 {
                return Ok(Self {
                    manor_id,
                    items: Vec::new(),
                });
            }
            // NOTE(java-quirk): Java drops the zero-sales rows here, so a list of only those is
            // empty. Encoding a value with such a row does not decode to the same value.
            if sales > 0 {
                items.push(SeedSale {
                    item_id,
                    sales,
                    price,
                });
            }
        }
        Ok(Self { manor_id, items })
    }
}

/// One crop the lord puts up for procurement in the next period (Java `CropProcure` as read by
/// `RequestSetCrop`). Only entries with `sales > 0` are kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CropSale {
    /// The crop item id.
    pub item_id: i32,
    /// The amount the manor wants (the start amount, which is also the limit).
    pub sales: i64,
    /// The price the manor pays for one crop.
    pub price: i64,
    /// The reward type of the seed (Java `CropProcure.getReward()`). Signed, because Java reads it
    /// with `readByte()`.
    pub reward_type: i8,
}

// Java: gameserver/network/clientpackets/RequestSetCrop.java
/// The lord sets the crop procurement of the next period (`0xD0` 0x0004, `REQUEST_SET_CROP`).
/// Each entry is 21 bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestSetCrop {
    /// The castle id.
    pub manor_id: i32,
    /// The kept entries. Empty for a bad list (Java returns on an empty list too).
    pub items: Vec<CropSale>,
}

impl RequestSetCrop {
    pub const EX_OPCODE: ExClientOpcode = ExClientOpcode::RequestSetCrop;
}

impl Encode for RequestSetCrop {
    fn encode(&self, w: &mut PacketWriter) {
        write_ex_opcode(w, Self::EX_OPCODE);
        w.i32(self.manor_id).i32(self.items.len() as i32);
        for item in &self.items {
            w.i32(item.item_id)
                .i64(item.sales)
                .i64(item.price)
                .i8(item.reward_type);
        }
    }
}

impl Decode for RequestSetCrop {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let manor_id = r.i32()?;
        let Some(count) = read_list_count(r, 21)? else {
            return Ok(Self {
                manor_id,
                items: Vec::new(),
            });
        };
        let mut items = Vec::with_capacity(count);
        for _ in 0..count {
            let item_id = r.i32()?;
            let sales = r.i64()?;
            let price = r.i64()?;
            let reward_type = r.i8()?;
            if item_id < 1 || sales < 0 || price < 0 {
                return Ok(Self {
                    manor_id,
                    items: Vec::new(),
                });
            }
            // NOTE(java-quirk): zero-sales rows are dropped, as in RequestSetSeed.
            if sales > 0 {
                items.push(CropSale {
                    item_id,
                    sales,
                    price,
                    reward_type,
                });
            }
        }
        Ok(Self { manor_id, items })
    }
}

/// One seed the player buys from the manor (Java `ItemHolder(itemId, count)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeedPurchase {
    pub item_id: i32,
    pub count: i64,
}

// Java: gameserver/network/clientpackets/RequestBuySeed.java
/// The player buys seeds from the manor (`REQUEST_BUY_SEED`). Each entry is 12 bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestBuySeed {
    /// The castle id.
    pub manor_id: i32,
    /// `None`: Java's bad list (see the module notes). The world answers `ActionFailed` for it.
    pub items: Option<Vec<SeedPurchase>>,
}

impl RequestBuySeed {
    pub const OPCODE: ClientOpcode = ClientOpcode::RequestBuySeed;
}

impl Encode for RequestBuySeed {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.manor_id);
        w.i32(self.items.as_ref().map_or(0, |v| v.len() as i32));
        for item in self.items.iter().flatten() {
            w.i32(item.item_id).i64(item.count);
        }
    }
}

impl Decode for RequestBuySeed {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let manor_id = r.i32()?;
        let Some(count) = read_list_count(r, 12)? else {
            return Ok(Self {
                manor_id,
                items: None,
            });
        };
        let mut items = Vec::with_capacity(count);
        for _ in 0..count {
            let item = SeedPurchase {
                item_id: r.i32()?,
                count: r.i64()?,
            };
            if item.count < 1 || item.item_id < 1 {
                return Ok(Self {
                    manor_id,
                    items: None,
                });
            }
            items.push(item);
        }
        Ok(Self {
            manor_id,
            items: Some(items),
        })
    }
}
