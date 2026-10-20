//! Client packets of the private stores (sell, buy and package sale): opening and closing the
//! manage window, the store message, the item lists the owner sets and the trades of customers.
//!
//! The list packets keep Java's "bad packet" rule: when the count is out of range, the byte
//! length does not match the count, or one entry has a nonsense value, Java reads no items and
//! the handler then answers `INCORRECT_ITEM_COUNT`. That is `items: None` here. Java also
//! rejects a count above `PlayerConfig.MAX_ITEM_IN_PACKET`; that limit comes from the
//! configuration, so the world compares `items.len()` with it and treats a larger list as `None`.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::{write_ex_opcode, write_opcode};
use crate::generated::opcodes::{ClientOpcode, ExClientOpcode};

/// Java's shared check of the item list packets: `count >= 1` and `count * BATCH_LENGTH ==
/// remaining()`. `None` means Java would drop the list (the handler then reports the error).
///
/// Java multiplies in `int`, so a count of 2^30 or more could wrap around and match; but Java
/// rejects every count above `MAX_ITEM_IN_PACKET` (a few hundred) before that matters, so the
/// product is computed without wrapping here.
fn read_batch<T>(
    r: &mut PacketReader,
    batch_length: usize,
    mut one: impl FnMut(&mut PacketReader) -> Result<Option<T>, PacketError>,
) -> Result<Option<Vec<T>>, PacketError> {
    let count = r.i32()?;
    // The batch lengths are small constants, so the cast cannot truncate.
    let expected = i64::from(count) * batch_length as i64;
    if count < 1 || expected != r.remaining() as i64 {
        return Ok(None);
    }
    let mut items = Vec::new();
    for _ in 0..count {
        match one(r)? {
            Some(item) => items.push(item),
            None => return Ok(None),
        }
    }
    Ok(Some(items))
}

/// Writes the count of an item list: Java's clients never send a list that [`read_batch`] would
/// drop, so `None` is written as the count `0`, which is dropped the same way.
fn write_count<T>(w: &mut PacketWriter, items: &Option<Vec<T>>) {
    w.i32(items.as_ref().map_or(0, |v| v.len() as i32));
}

macro_rules! empty_client_packet {
    ($(#[$meta:meta])* $name:ident, $opcode:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct $name;

        impl $name {
            pub const OPCODE: ClientOpcode = ClientOpcode::$opcode;
        }

        impl Encode for $name {
            fn encode(&self, w: &mut PacketWriter) {
                write_opcode(w, Self::OPCODE);
            }
        }

        impl Decode for $name {
            fn decode(_r: &mut PacketReader) -> Result<Self, PacketError> {
                Ok(Self)
            }
        }
    };
}

// Java: gameserver/network/clientpackets/RequestPrivateStoreManageSell.java
empty_client_packet!(
    /// The player opens the manage window of a sell store (`REQUEST_PRIVATE_STORE_MANAGE_SELL`).
    /// No fields: Java's `readImpl` has a TODO and reads nothing.
    RequestPrivateStoreManageSell,
    RequestPrivateStoreManageSell
);
// Java: gameserver/network/clientpackets/RequestPrivateStoreManageBuy.java
empty_client_packet!(
    /// The player opens the manage window of a buy store (`REQUEST_PRIVATE_STORE_MANAGE_BUY`).
    RequestPrivateStoreManageBuy,
    RequestPrivateStoreManageBuy
);
// Java: gameserver/network/clientpackets/RequestPrivateStoreQuitSell.java
empty_client_packet!(
    /// The player closes its sell store (`REQUEST_PRIVATE_STORE_QUIT_SELL`).
    RequestPrivateStoreQuitSell,
    RequestPrivateStoreQuitSell
);
// Java: gameserver/network/clientpackets/RequestPrivateStoreQuitBuy.java
empty_client_packet!(
    /// The player closes its buy store (`REQUEST_PRIVATE_STORE_QUIT_BUY`).
    RequestPrivateStoreQuitBuy,
    RequestPrivateStoreQuitBuy
);

// Java: gameserver/network/clientpackets/SetPrivateStoreMsgSell.java
/// The title of the sell store (`SET_PRIVATE_STORE_MSG_SELL`). Java reads it as is; the handler
/// punishes a title longer than 29 characters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetPrivateStoreMsgSell {
    pub store_msg: String,
}

impl SetPrivateStoreMsgSell {
    pub const OPCODE: ClientOpcode = ClientOpcode::SetPrivateStoreMsgSell;
}

impl Encode for SetPrivateStoreMsgSell {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.string(&self.store_msg);
    }
}

impl Decode for SetPrivateStoreMsgSell {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            store_msg: r.string(),
        })
    }
}

// Java: gameserver/network/clientpackets/SetPrivateStoreMsgBuy.java
/// The title of the buy store (`SET_PRIVATE_STORE_MSG_BUY`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetPrivateStoreMsgBuy {
    pub store_msg: String,
}

impl SetPrivateStoreMsgBuy {
    pub const OPCODE: ClientOpcode = ClientOpcode::SetPrivateStoreMsgBuy;
}

impl Encode for SetPrivateStoreMsgBuy {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.string(&self.store_msg);
    }
}

impl Decode for SetPrivateStoreMsgBuy {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            store_msg: r.string(),
        })
    }
}

// Java: gameserver/network/clientpackets/SetPrivateStoreWholeMsg.java
/// The title of a package sale (`SET_PRIVATE_STORE_WHOLE_MSG`). Ex packet `0xD0 0x0049`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetPrivateStoreWholeMsg {
    pub msg: String,
}

impl SetPrivateStoreWholeMsg {
    pub const EX_OPCODE: ExClientOpcode = ExClientOpcode::SetPrivateStoreWholeMsg;
}

impl Encode for SetPrivateStoreWholeMsg {
    fn encode(&self, w: &mut PacketWriter) {
        write_ex_opcode(w, Self::EX_OPCODE);
        w.string(&self.msg);
    }
}

impl Decode for SetPrivateStoreWholeMsg {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { msg: r.string() })
    }
}

/// One item the owner puts up for sale (Java's private `Item` of `SetPrivateStoreListSell`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SellListEntry {
    /// Java names it `itemId`, but the handler passes it to `TradeList.addItem(objectId, ...)`:
    /// it is the inventory object id.
    pub item_id: i32,
    pub count: i64,
    /// The price of one unit.
    pub price: i64,
}

// Java: gameserver/network/clientpackets/SetPrivateStoreListSell.java
/// The owner confirms the items of its sell store (`SET_PRIVATE_STORE_LIST_SELL`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetPrivateStoreListSell {
    pub package_sale: bool,
    /// `None`: Java's bad list (see the module notes).
    pub items: Option<Vec<SellListEntry>>,
}

impl SetPrivateStoreListSell {
    pub const OPCODE: ClientOpcode = ClientOpcode::SetPrivateStoreListSell;
    /// Bytes per item: an int and two longs.
    pub const BATCH_LENGTH: usize = 20;
}

impl Encode for SetPrivateStoreListSell {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(i32::from(self.package_sale));
        write_count(w, &self.items);
        for item in self.items.iter().flatten() {
            w.i32(item.item_id).i64(item.count).i64(item.price);
        }
    }
}

impl Decode for SetPrivateStoreListSell {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let package_sale = r.i32()? == 1;
        let items = read_batch(r, Self::BATCH_LENGTH, |r| {
            let item_id = r.i32()?;
            let count = r.i64()?;
            let price = r.i64()?;
            Ok(
                (item_id >= 1 && count >= 1 && price >= 0).then_some(SellListEntry {
                    item_id,
                    count,
                    price,
                }),
            )
        })?;
        Ok(Self { package_sale, items })
    }
}

/// One item the owner wants to buy (Java's private `Item` of `SetPrivateStoreListBuy`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuyListEntry {
    /// The item template id.
    pub item_id: i32,
    /// Java: `readInt(); // TODO analyze this`.
    pub unknown1: i32,
    pub count: i64,
    /// The price of one unit.
    pub price: i64,
    /// Java: four `readInt(); // Unk`.
    pub unknown2: [i32; 4],
}

// Java: gameserver/network/clientpackets/SetPrivateStoreListBuy.java
/// The owner confirms the items of its buy store (`SET_PRIVATE_STORE_LIST_BUY`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetPrivateStoreListBuy {
    /// `None`: Java's bad list (see the module notes).
    pub items: Option<Vec<BuyListEntry>>,
}

impl SetPrivateStoreListBuy {
    pub const OPCODE: ClientOpcode = ClientOpcode::SetPrivateStoreListBuy;
    /// Bytes per item: two ints, two longs and four ints.
    pub const BATCH_LENGTH: usize = 40;
}

impl Encode for SetPrivateStoreListBuy {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        write_count(w, &self.items);
        for item in self.items.iter().flatten() {
            w.i32(item.item_id)
                .i32(item.unknown1)
                .i64(item.count)
                .i64(item.price);
            for unknown in item.unknown2 {
                w.i32(unknown);
            }
        }
    }
}

impl Decode for SetPrivateStoreListBuy {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let items = read_batch(r, Self::BATCH_LENGTH, |r| {
            let item_id = r.i32()?;
            let unknown1 = r.i32()?;
            let count = r.i64()?;
            let price = r.i64()?;
            // Java checks the values before it reads the four unknown ints, which only matters
            // for a bad entry: the whole list is dropped then anyway.
            let valid = item_id >= 1 && count >= 1 && price >= 0;
            let unknown2 = [r.i32()?, r.i32()?, r.i32()?, r.i32()?];
            Ok(valid.then_some(BuyListEntry {
                item_id,
                unknown1,
                count,
                price,
                unknown2,
            }))
        })?;
        Ok(Self { items })
    }
}

/// One item a customer buys from a sell store (Java `RequestTrade`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoreBuyRequestEntry {
    pub object_id: i32,
    pub count: i64,
    /// The price per unit the customer saw; the handler compares it with the store's price.
    pub price: i64,
}

// Java: gameserver/network/clientpackets/RequestPrivateStoreBuy.java
/// A customer buys items from a sell store or package sale (`REQUEST_PRIVATE_STORE_BUY`).
///
/// NOTE(java-quirk): Java collects the entries in a `HashSet<RequestTrade>` whose `equals` is
/// inverted (`objectId != other.objectId`), so entries with the same object id are all kept and
/// the iteration order is the hash order. The entries are kept in packet order here and none is
/// dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestPrivateStoreBuy {
    pub store_player_id: i32,
    /// `None`: Java's bad list (see the module notes).
    pub items: Option<Vec<StoreBuyRequestEntry>>,
}

impl RequestPrivateStoreBuy {
    pub const OPCODE: ClientOpcode = ClientOpcode::RequestPrivateStoreBuy;
    /// Bytes per item: an int and two longs.
    pub const BATCH_LENGTH: usize = 20;
}

impl Encode for RequestPrivateStoreBuy {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.store_player_id);
        write_count(w, &self.items);
        for item in self.items.iter().flatten() {
            w.i32(item.object_id).i64(item.count).i64(item.price);
        }
    }
}

impl Decode for RequestPrivateStoreBuy {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let store_player_id = r.i32()?;
        let items = read_batch(r, Self::BATCH_LENGTH, |r| {
            let object_id = r.i32()?;
            let count = r.i64()?;
            let price = r.i64()?;
            Ok(
                (object_id >= 1 && count >= 1 && price >= 0).then_some(StoreBuyRequestEntry {
                    object_id,
                    count,
                    price,
                }),
            )
        })?;
        Ok(Self {
            store_player_id,
            items,
        })
    }
}

/// One item a customer sells to a buy store (Java `RequestTrade` with an item id).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoreSellRequestEntry {
    pub object_id: i32,
    pub item_id: i32,
    /// Java: two `readShort(); // TODO: analyze this`.
    pub unknown: [i16; 2],
    pub count: i64,
    pub price: i64,
}

// Java: gameserver/network/clientpackets/RequestPrivateStoreSell.java
/// A customer sells items to a buy store (`REQUEST_PRIVATE_STORE_SELL`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestPrivateStoreSell {
    pub store_player_id: i32,
    /// `None`: Java's bad list (see the module notes).
    pub items: Option<Vec<StoreSellRequestEntry>>,
}

impl RequestPrivateStoreSell {
    pub const OPCODE: ClientOpcode = ClientOpcode::RequestPrivateStoreSell;
    /// Bytes per item: two ints, two shorts and two longs.
    pub const BATCH_LENGTH: usize = 28;
}

impl Encode for RequestPrivateStoreSell {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.store_player_id);
        write_count(w, &self.items);
        for item in self.items.iter().flatten() {
            w.i32(item.object_id)
                .i32(item.item_id)
                .i16(item.unknown[0])
                .i16(item.unknown[1])
                .i64(item.count)
                .i64(item.price);
        }
    }
}

impl Decode for RequestPrivateStoreSell {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let store_player_id = r.i32()?;
        let items = read_batch(r, Self::BATCH_LENGTH, |r| {
            let object_id = r.i32()?;
            let item_id = r.i32()?;
            let unknown = [r.i16()?, r.i16()?];
            let count = r.i64()?;
            let price = r.i64()?;
            Ok(
                (object_id >= 1 && item_id >= 1 && count >= 1 && price >= 0).then_some(
                    StoreSellRequestEntry {
                        object_id,
                        item_id,
                        unknown,
                        count,
                        price,
                    },
                ),
            )
        })?;
        Ok(Self {
            store_player_id,
            items,
        })
    }
}
