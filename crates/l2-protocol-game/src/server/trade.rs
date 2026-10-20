//! Server packets of the player to player trade window.
//!
//! Same rules as [`super::skills`]: values only. The trade list, the partner and the items come
//! from the world; Java's `Player`/`TradeList` reads are plain fields here.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::items::{ItemInfo, read_item, write_item};
use super::write_opcode;
use crate::generated::opcodes::ServerOpcode;

// Java: gameserver/network/serverpackets/AbstractItemPacket.java (`writeTradeItem`) and
// gameserver/entity/item/holders/TradeItem.java
/// One item in a trade window as `writeTradeItem` sends it. The trade layout leaves out the
/// location and the equipped flag of [`ItemInfo`] and starts with the item's `type1`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TradeItemInfo {
    /// `getItem().getType1()`.
    pub type1: i16,
    pub object_id: i32,
    /// `getItem().getDisplayId()`.
    pub item_id: i32,
    /// The count in the trade (not the count in the inventory).
    pub count: i64,
    /// `getItem().getType2()`.
    pub type2: i16,
    /// `getCustomType1()`.
    pub custom_type1: i16,
    /// `getItem().getBodyPart().getMask()`.
    pub body_part: i32,
    pub enchant: i16,
    /// `getCustomType2()`.
    pub custom_type2: i16,
    /// `getAttackElementType()`: -2 when there is none.
    pub attack_element_type: i16,
    pub attack_element_power: i16,
    /// Fire, water, wind, earth, holy, dark.
    pub element_defense: [i16; 6],
    pub enchant_options: [i16; 3],
}

impl TradeItemInfo {
    /// Java `writeTradeItem`.
    pub fn write(&self, w: &mut PacketWriter) {
        w.i16(self.type1)
            .i32(self.object_id)
            .i32(self.item_id)
            .i64(self.count)
            .i16(self.type2)
            .i16(self.custom_type1)
            .i32(self.body_part)
            .i16(self.enchant)
            .i16(0) // Java: "Equipped : 00-No, 01-yes", always 0 here
            .i16(self.custom_type2)
            .i16(self.attack_element_type)
            .i16(self.attack_element_power);
        for def in self.element_defense {
            w.i16(def);
        }
        for option in self.enchant_options {
            w.i16(option);
        }
    }

    /// Reads what [`Self::write`] wrote.
    pub fn read(r: &mut PacketReader) -> Result<Self, PacketError> {
        let type1 = r.i16()?;
        let object_id = r.i32()?;
        let item_id = r.i32()?;
        let count = r.i64()?;
        let type2 = r.i16()?;
        let custom_type1 = r.i16()?;
        let body_part = r.i32()?;
        let enchant = r.i16()?;
        r.skip(2)?;
        let custom_type2 = r.i16()?;
        let attack_element_type = r.i16()?;
        let attack_element_power = r.i16()?;
        let mut element_defense = [0; 6];
        for def in &mut element_defense {
            *def = r.i16()?;
        }
        let mut enchant_options = [0; 3];
        for option in &mut enchant_options {
            *option = r.i16()?;
        }
        Ok(Self {
            type1,
            object_id,
            item_id,
            count,
            type2,
            custom_type1,
            body_part,
            enchant,
            custom_type2,
            attack_element_type,
            attack_element_power,
            element_defense,
            enchant_options,
        })
    }
}

// Java: gameserver/network/serverpackets/SendTradeRequest.java
/// Another player wants to trade with you (`TRADE_REQUEST`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SendTradeRequest {
    pub sender_id: i32,
}

impl SendTradeRequest {
    pub const OPCODE: ServerOpcode = ServerOpcode::TradeRequest;
}

impl Encode for SendTradeRequest {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.sender_id);
    }
}

impl Decode for SendTradeRequest {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { sender_id: r.i32()? })
    }
}

// Java: gameserver/network/serverpackets/TradeStart.java
/// The trade window opens (`TRADE_START`): the partner and your tradable items.
///
/// Java sends nothing when the player has no active trade list or the list has no partner; that
/// check is the caller's.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TradeStart {
    pub partner_object_id: i32,
    /// `getInventory().getAvailableItems(true, ...)`, in the order Java lists them.
    pub items: Vec<ItemInfo>,
}

impl TradeStart {
    pub const OPCODE: ServerOpcode = ServerOpcode::TradeStart;
}

impl Encode for TradeStart {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.partner_object_id).u16(self.items.len() as u16);
        for item in &self.items {
            write_item(w, item);
        }
    }
}

impl Decode for TradeStart {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let partner_object_id = r.i32()?;
        let count = r.u16()?;
        let mut items = Vec::new();
        for _ in 0..count {
            items.push(read_item(r)?);
        }
        Ok(Self {
            partner_object_id,
            items,
        })
    }
}

// Java: gameserver/network/serverpackets/TradeOwnAdd.java
/// You added an item to the trade (`TRADE_OWN_ADD`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TradeOwnAdd {
    pub item: TradeItemInfo,
}

impl TradeOwnAdd {
    pub const OPCODE: ServerOpcode = ServerOpcode::TradeOwnAdd;
}

impl Encode for TradeOwnAdd {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i16(1); // items added count
        self.item.write(w);
    }
}

impl Decode for TradeOwnAdd {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        r.skip(2)?;
        Ok(Self {
            item: TradeItemInfo::read(r)?,
        })
    }
}

// Java: gameserver/network/serverpackets/TradeOtherAdd.java
/// The partner added an item to the trade (`TRADE_OTHER_ADD`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TradeOtherAdd {
    pub item: TradeItemInfo,
}

impl TradeOtherAdd {
    pub const OPCODE: ServerOpcode = ServerOpcode::TradeOtherAdd;
}

impl Encode for TradeOtherAdd {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i16(1); // item count
        self.item.write(w);
    }
}

impl Decode for TradeOtherAdd {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        r.skip(2)?;
        Ok(Self {
            item: TradeItemInfo::read(r)?,
        })
    }
}

// Java: gameserver/network/serverpackets/TradeUpdate.java
/// The count of an item in your inventory window changed because of the trade (`TRADE_UPDATE`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TradeUpdate {
    /// Java writes `3` (modify) when part of a stackable item stays in the inventory and `2`
    /// (remove) otherwise; see [`Self::change_for`].
    pub change: i16,
    pub item: TradeItemInfo,
}

impl TradeUpdate {
    pub const OPCODE: ServerOpcode = ServerOpcode::TradeUpdate;

    /// Java `(_newCount > 0) && _item.getItem().isStackable() ? 3 : 2`, where `_newCount` is the
    /// inventory count minus the traded count.
    pub fn change_for(new_count: i64, stackable: bool) -> i16 {
        if new_count > 0 && stackable { 3 } else { 2 }
    }
}

impl Encode for TradeUpdate {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i16(1).i16(self.change);
        self.item.write(w);
    }
}

impl Decode for TradeUpdate {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        r.skip(2)?;
        let change = r.i16()?;
        Ok(Self {
            change,
            item: TradeItemInfo::read(r)?,
        })
    }
}

// Java: gameserver/network/serverpackets/TradeOtherDone.java
/// The partner pressed OK (`TRADE_PRESS_OTHER_OK`). No fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TradeOtherDone;

impl TradeOtherDone {
    pub const OPCODE: ServerOpcode = ServerOpcode::TradePressOtherOk;
}

impl Encode for TradeOtherDone {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
    }
}

impl Decode for TradeOtherDone {
    fn decode(_r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self)
    }
}

// Java: gameserver/network/serverpackets/TradeDone.java
/// The trade ended (`TRADE_DONE`): `1` completed, `0` canceled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TradeDone {
    pub num: i32,
}

impl TradeDone {
    pub const OPCODE: ServerOpcode = ServerOpcode::TradeDone;
}

impl Encode for TradeDone {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.num);
    }
}

impl Decode for TradeDone {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { num: r.i32()? })
    }
}
