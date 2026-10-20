//! Private store server packets: the manage windows, the store lists a customer sees and the
//! store titles.
//!
//! Same rules as [`super::skills`]: values only. Java builds the lists from `TradeList`,
//! `Inventory` and the item templates; here the caller passes [`ItemInfo`]s and the numbers.
//! "Reference price" is what Java writes as `getReferencePrice() * 2`: the caller does the
//! doubling.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::items::{ItemInfo, read_item, write_item};
use super::{read_count, write_opcode};
use crate::generated::opcodes::ServerOpcode;

/// An inventory item that could be put into a store, with the reference price shown next to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreCandidate {
    pub item: ItemInfo,
    /// `getReferencePrice() * 2`.
    pub reference_price: i64,
}

/// An item already in a sell store: the owner's price and the reference price.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SellStoreEntry {
    pub item: ItemInfo,
    /// `TradeItem.getPrice()`: the price of one unit.
    pub price: i64,
    /// `getReferencePrice() * 2`.
    pub reference_price: i64,
}

/// An item in a buy store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuyStoreEntry {
    pub item: ItemInfo,
    pub price: i64,
    /// `getReferencePrice() * 2`.
    pub reference_price: i64,
    /// [`PrivateStoreManageListBuy`]: `TradeItem.getCount()`, the amount wanted.
    /// [`PrivateStoreListBuy`]: `TradeItem.getStoreCount()`, the amount still wanted.
    pub count: i64,
}

fn read_candidates(r: &mut PacketReader) -> Result<Vec<StoreCandidate>, PacketError> {
    let count = read_count(r, "store candidates")?;
    let mut items = Vec::new();
    for _ in 0..count {
        items.push(StoreCandidate {
            item: read_item(r)?,
            reference_price: r.i64()?,
        });
    }
    Ok(items)
}

fn write_candidates(w: &mut PacketWriter, items: &[StoreCandidate]) {
    w.i32(items.len() as i32);
    for entry in items {
        write_item(w, &entry.item);
        w.i64(entry.reference_price);
    }
}

fn read_sell_entries(r: &mut PacketReader) -> Result<Vec<SellStoreEntry>, PacketError> {
    let count = read_count(r, "sell store items")?;
    let mut items = Vec::new();
    for _ in 0..count {
        items.push(SellStoreEntry {
            item: read_item(r)?,
            price: r.i64()?,
            reference_price: r.i64()?,
        });
    }
    Ok(items)
}

fn write_sell_entries(w: &mut PacketWriter, items: &[SellStoreEntry]) {
    w.i32(items.len() as i32);
    for entry in items {
        write_item(w, &entry.item);
        w.i64(entry.price).i64(entry.reference_price);
    }
}

// Java: gameserver/network/serverpackets/PrivateStoreManageListSell.java
/// The manage window of a sell store (`PRIVATE_STORE_SELL_MANAGE_LIST`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivateStoreManageListSell {
    pub object_id: i32,
    pub package_sale: bool,
    pub player_adena: i64,
    /// The inventory items that can still be added.
    pub item_list: Vec<StoreCandidate>,
    /// The items already in the store.
    pub sell_list: Vec<SellStoreEntry>,
}

impl PrivateStoreManageListSell {
    pub const OPCODE: ServerOpcode = ServerOpcode::PrivateStoreSellManageList;
}

impl Encode for PrivateStoreManageListSell {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.object_id)
            .i32(i32::from(self.package_sale))
            .i64(self.player_adena);
        write_candidates(w, &self.item_list);
        write_sell_entries(w, &self.sell_list);
    }
}

impl Decode for PrivateStoreManageListSell {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            object_id: r.i32()?,
            package_sale: r.i32()? != 0,
            player_adena: r.i64()?,
            item_list: read_candidates(r)?,
            sell_list: read_sell_entries(r)?,
        })
    }
}

// Java: gameserver/network/serverpackets/PrivateStoreManageListBuy.java
/// The manage window of a buy store (`PRIVATE_STORE_BUY_MANAGE_LIST`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivateStoreManageListBuy {
    pub object_id: i32,
    pub player_adena: i64,
    /// The inventory items that could be bought (`getUniqueItems`).
    pub item_list: Vec<StoreCandidate>,
    /// The items already wanted.
    pub buy_list: Vec<BuyStoreEntry>,
}

impl PrivateStoreManageListBuy {
    pub const OPCODE: ServerOpcode = ServerOpcode::PrivateStoreBuyManageList;
}

impl Encode for PrivateStoreManageListBuy {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.object_id).i64(self.player_adena);
        write_candidates(w, &self.item_list);
        w.i32(self.buy_list.len() as i32);
        for entry in &self.buy_list {
            write_item(w, &entry.item);
            w.i64(entry.price).i64(entry.reference_price).i64(entry.count);
        }
    }
}

impl Decode for PrivateStoreManageListBuy {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let object_id = r.i32()?;
        let player_adena = r.i64()?;
        let item_list = read_candidates(r)?;
        let count = read_count(r, "buy store items")?;
        let mut buy_list = Vec::new();
        for _ in 0..count {
            buy_list.push(BuyStoreEntry {
                item: read_item(r)?,
                price: r.i64()?,
                reference_price: r.i64()?,
                count: r.i64()?,
            });
        }
        Ok(Self {
            object_id,
            player_adena,
            item_list,
            buy_list,
        })
    }
}

// Java: gameserver/network/serverpackets/PrivateStoreListSell.java
/// What a customer sees when it opens a sell store (`PRIVATE_STORE_SELL_LIST`).
///
/// Java instead sends the buff menu when the seller is a buff seller (`isSellingBuffs`); that
/// choice is the caller's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivateStoreListSell {
    pub seller_object_id: i32,
    pub package_sale: bool,
    /// The customer's adena.
    pub player_adena: i64,
    pub items: Vec<SellStoreEntry>,
}

impl PrivateStoreListSell {
    pub const OPCODE: ServerOpcode = ServerOpcode::PrivateStoreSellList;
}

impl Encode for PrivateStoreListSell {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.seller_object_id)
            .i32(i32::from(self.package_sale))
            .i64(self.player_adena);
        write_sell_entries(w, &self.items);
    }
}

impl Decode for PrivateStoreListSell {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            seller_object_id: r.i32()?,
            package_sale: r.i32()? != 0,
            player_adena: r.i64()?,
            items: read_sell_entries(r)?,
        })
    }
}

// Java: gameserver/network/serverpackets/PrivateStoreListBuy.java
/// What a customer sees when it opens a buy store (`PRIVATE_STORE_BUY_LIST`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivateStoreListBuy {
    pub store_object_id: i32,
    /// The customer's adena.
    pub player_adena: i64,
    pub items: Vec<BuyStoreEntry>,
}

impl PrivateStoreListBuy {
    pub const OPCODE: ServerOpcode = ServerOpcode::PrivateStoreBuyList;
}

impl Encode for PrivateStoreListBuy {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.store_object_id)
            .i64(self.player_adena)
            .i32(self.items.len() as i32);
        for entry in &self.items {
            write_item(w, &entry.item);
            // Java repeats the item's object id after the item block.
            w.i32(entry.item.object_id)
                .i64(entry.price)
                .i64(entry.reference_price)
                .i64(entry.count);
        }
    }
}

impl Decode for PrivateStoreListBuy {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let store_object_id = r.i32()?;
        let player_adena = r.i64()?;
        let count = read_count(r, "buy store items")?;
        let mut items = Vec::new();
        for _ in 0..count {
            let item = read_item(r)?;
            r.skip(4)?;
            items.push(BuyStoreEntry {
                item,
                price: r.i64()?,
                reference_price: r.i64()?,
                count: r.i64()?,
            });
        }
        Ok(Self {
            store_object_id,
            player_adena,
            items,
        })
    }
}

macro_rules! store_message_packet {
    ($(#[$meta:meta])* $name:ident, $opcode:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $name {
            pub object_id: i32,
            /// Java `String _message`; a missing title is written as the empty string.
            pub message: String,
        }

        impl $name {
            pub const OPCODE: ServerOpcode = ServerOpcode::$opcode;
        }

        impl Encode for $name {
            fn encode(&self, w: &mut PacketWriter) {
                write_opcode(w, Self::OPCODE);
                w.i32(self.object_id).string(&self.message);
            }
        }

        impl Decode for $name {
            fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
                Ok(Self {
                    object_id: r.i32()?,
                    message: r.string(),
                })
            }
        }
    };
}

// Java: gameserver/network/serverpackets/PrivateStoreMsgSell.java
store_message_packet!(
    /// The title over a sell store (`PRIVATE_STORE_SELL_MSG`).
    PrivateStoreMsgSell,
    PrivateStoreSellMsg
);
// Java: gameserver/network/serverpackets/PrivateStoreMsgBuy.java
store_message_packet!(
    /// The title over a buy store (`PRIVATE_STORE_BUY_MSG`).
    PrivateStoreMsgBuy,
    PrivateStoreBuyMsg
);
// Java: gameserver/network/serverpackets/ExPrivateStoreSetWholeMsg.java
store_message_packet!(
    /// The title over a package sale (`EX_PRIVATE_STORE_PACKAGE_MSG`).
    ExPrivateStoreSetWholeMsg,
    ExPrivateStorePackageMsg
);
