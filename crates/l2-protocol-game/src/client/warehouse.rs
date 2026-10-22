//! Client packets of the warehouses and the freight: deposit, withdraw, and sending a package to
//! another character of the account (M6-WH-01).
//!
//! The three list packets keep Java's "bad packet" rule: when the count is out of range, the
//! byte length does not match the count, or one entry has a nonsense value, Java reads no items
//! and the handler returns without a word. That is `items: None` here. Java also rejects a count
//! above `PlayerConfig.MAX_ITEM_IN_PACKET`; that limit comes from the configuration, so the world
//! compares `items.len()` with it and treats a longer list as `None`.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_opcode;
use crate::generated::opcodes::ClientOpcode;

/// Bytes per item in the three list packets: an int object id and a long count.
pub const BATCH_LENGTH: usize = 12;

/// One entry of a deposit, withdraw or package list (Java `ItemHolder(objectId, count)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WarehouseListEntry {
    /// The object id of the item.
    pub object_id: i32,
    pub count: i64,
}

/// Java's shared reading of the three list packets: `count >= 1` and `count * 12 == remaining()`,
/// then each entry; `None` when Java would drop the list. `valid` is the per-entry rule (the
/// deposit wants `count >= 1`, the other two accept 0).
fn read_list(
    r: &mut PacketReader,
    valid: impl Fn(&WarehouseListEntry) -> bool,
) -> Result<Option<Vec<WarehouseListEntry>>, PacketError> {
    let count = r.i32()?;
    // Java multiplies in `int`; a count that wraps would still be far above MAX_ITEM_IN_PACKET,
    // which the world rejects, so the product is computed without wrapping here.
    let expected = i64::from(count) * BATCH_LENGTH as i64;
    if count < 1 || expected != r.remaining() as i64 {
        return Ok(None);
    }
    let mut items = Vec::new();
    for _ in 0..count {
        let entry = WarehouseListEntry {
            object_id: r.i32()?,
            count: r.i64()?,
        };
        if !valid(&entry) {
            return Ok(None);
        }
        items.push(entry);
    }
    Ok(Some(items))
}

/// Writes a list: `None` (a list Java drops) is the count `0`, which is dropped the same way.
fn write_list(w: &mut PacketWriter, items: &Option<Vec<WarehouseListEntry>>) {
    w.i32(items.as_ref().map_or(0, |v| v.len() as i32));
    for item in items.iter().flatten() {
        w.i32(item.object_id).i64(item.count);
    }
}

// Java: gameserver/network/clientpackets/SendWareHouseDepositList.java
/// The player confirms what it puts into the warehouse (`SEND_WARE_HOUSE_DEPOSIT_LIST`). Java
/// drops the list when an entry has an object id below 1 or a count below 1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendWareHouseDepositList {
    /// `None`: Java's bad list (see the module notes).
    pub items: Option<Vec<WarehouseListEntry>>,
}

impl SendWareHouseDepositList {
    pub const OPCODE: ClientOpcode = ClientOpcode::SendWareHouseDepositList;
}

impl Encode for SendWareHouseDepositList {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        write_list(w, &self.items);
    }
}

impl Decode for SendWareHouseDepositList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            items: read_list(r, |e| e.object_id >= 1 && e.count >= 1)?,
        })
    }
}

// Java: gameserver/network/clientpackets/SendWareHouseWithDrawList.java
/// The player confirms what it takes out of the warehouse (`SEND_WARE_HOUSE_WITH_DRAW_LIST`).
/// Java drops the list when an entry has an object id below 1 or a count below 0 (a count of 0 is
/// accepted).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendWareHouseWithDrawList {
    /// `None`: Java's bad list (see the module notes).
    pub items: Option<Vec<WarehouseListEntry>>,
}

impl SendWareHouseWithDrawList {
    pub const OPCODE: ClientOpcode = ClientOpcode::SendWareHouseWithDrawList;
}

impl Encode for SendWareHouseWithDrawList {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        write_list(w, &self.items);
    }
}

impl Decode for SendWareHouseWithDrawList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            items: read_list(r, |e| e.object_id >= 1 && e.count >= 0)?,
        })
    }
}

// Java: gameserver/network/clientpackets/RequestPackageSend.java
/// The player sends items to the freight of another character of its account
/// (`REQUEST_PACKAGE_SEND`). Same entry rule as the withdraw list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestPackageSend {
    /// The object id of the receiving character.
    pub object_id: i32,
    /// `None`: Java's bad list (see the module notes).
    pub items: Option<Vec<WarehouseListEntry>>,
}

impl RequestPackageSend {
    pub const OPCODE: ClientOpcode = ClientOpcode::RequestPackageSend;
}

impl Encode for RequestPackageSend {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.object_id);
        write_list(w, &self.items);
    }
}

impl Decode for RequestPackageSend {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let object_id = r.i32()?;
        Ok(Self {
            object_id,
            items: read_list(r, |e| e.object_id >= 1 && e.count >= 0)?,
        })
    }
}

// Java: gameserver/network/clientpackets/RequestPackageSendableItemList.java
/// The player asks which items it can send to the character `object_id`
/// (`REQUEST_PACKAGE_SENDABLE_ITEM_LIST`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestPackageSendableItemList {
    pub object_id: i32,
}

impl RequestPackageSendableItemList {
    pub const OPCODE: ClientOpcode = ClientOpcode::RequestPackageSendableItemList;
}

impl Encode for RequestPackageSendableItemList {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.object_id);
    }
}

impl Decode for RequestPackageSendableItemList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { object_id: r.i32()? })
    }
}
