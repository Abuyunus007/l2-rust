//! Item packets: the shared item-info writer and `ItemList`.
//!
//! Java: `AbstractItemPacket` (the writer helpers), `AbstractInventoryUpdate` (the entry list)
//! and `ItemList`. Later inventory packets (`InventoryUpdate`, `PetItemList`, warehouse, ...)
//! reuse [`write_item`] / [`read_item`], [`write_item_updates`] and [`write_inventory_block`].
//!
//! Everything Java derives from an `Item` / `ItemTemplate` / `Inventory` is a plain field here:
//! the caller builds an [`ItemInfo`] (Java `ItemInfo`) from its own item data. Filtering (quest
//! items stay out of `ItemList`) and ordering are the caller's job as well.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_opcode;
use crate::generated::opcodes::ServerOpcode;

// Java: gameserver/entity/item/holders/ItemInfo.java (the fields `writeItem` reads)
/// One item as the client sees it. All shorts are the low 16 bits of Java ints.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ItemInfo {
    /// `getObjectId()`.
    pub object_id: i32,
    /// `getItem().getDisplayId()` (not the template id: Java shows the display id).
    pub item_id: i32,
    /// `getLocation()`: the item's `locData` (inventory slot or paperdoll slot).
    pub location: i32,
    /// `getCount()`.
    pub count: i64,
    /// `getItem().getType2()`: 0 weapon, 1 shield/armor, 2 ring/earring/necklace, 3 quest item,
    /// 4 adena, 5 item.
    pub type2: i16,
    /// `getCustomType1()` (lottery / race tickets).
    pub custom_type1: i16,
    /// `getEquipped()`: 0 or 1.
    pub equipped: i16,
    /// `getItem().getBodyPart().getMask()`.
    pub body_part: i32,
    /// `getEnchant()` (pet level for pet control items).
    pub enchant: i16,
    /// `getCustomType2()` (pet name exists, for pet control items).
    pub custom_type2: i16,
    /// `getAugmentationBonus()`: the augmentation id, 0 if none.
    pub augmentation: i32,
    /// `getMana()`: shadow item mana, -1 for normal items.
    pub mana: i32,
    /// `getTime()`: seconds left of a time limited item, -9999 for normal items.
    pub time: i32,
    /// `getAttackElementType()`: -2 when there is none (also for everything that is no weapon).
    pub attack_element_type: i16,
    /// `getAttackElementPower()`.
    pub attack_element_power: i16,
    /// `getElementDefAttr(0..6)`: fire, water, wind, earth, holy, dark.
    pub element_defense: [i16; 6],
    /// `getEnchantOptions()`: three option ids, `[0, 0, 0]` by default.
    pub enchant_options: [i16; 3],
}

impl ItemInfo {
    /// What `ItemInfo` holds for an item without elements, augmentation, shadow mana or
    /// lifetime: mana -1, time -9999, attack element -2 (the values Java's `Item` reports).
    pub fn plain(object_id: i32, item_id: i32, count: i64) -> Self {
        Self {
            object_id,
            item_id,
            count,
            mana: -1,
            time: -9999,
            attack_element_type: -2,
            ..Self::default()
        }
    }
}

/// Java `AbstractItemPacket.writeItem(ItemInfo, buffer)`.
pub fn write_item(w: &mut PacketWriter, item: &ItemInfo) {
    w.i32(item.object_id).i32(item.item_id).i32(item.location);
    w.i64(item.count);
    w.i16(item.type2).i16(item.custom_type1).i16(item.equipped);
    w.i32(item.body_part);
    w.i16(item.enchant).i16(item.custom_type2);
    w.i32(item.augmentation).i32(item.mana).i32(item.time);
    write_item_elemental_and_enchant(w, item);
}

/// Java `AbstractItemPacket.writeItemElementalAndEnchant`.
pub fn write_item_elemental_and_enchant(w: &mut PacketWriter, item: &ItemInfo) {
    w.i16(item.attack_element_type).i16(item.attack_element_power);
    for def in item.element_defense {
        w.i16(def);
    }
    for option in item.enchant_options {
        w.i16(option);
    }
}

/// Reads what [`write_item`] wrote.
pub fn read_item(r: &mut PacketReader) -> Result<ItemInfo, PacketError> {
    let object_id = r.i32()?;
    let item_id = r.i32()?;
    let location = r.i32()?;
    let count = r.i64()?;
    let type2 = r.i16()?;
    let custom_type1 = r.i16()?;
    let equipped = r.i16()?;
    let body_part = r.i32()?;
    let enchant = r.i16()?;
    let custom_type2 = r.i16()?;
    let augmentation = r.i32()?;
    let mana = r.i32()?;
    let time = r.i32()?;
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
    Ok(ItemInfo {
        object_id,
        item_id,
        location,
        count,
        type2,
        custom_type1,
        equipped,
        body_part,
        enchant,
        custom_type2,
        augmentation,
        mana,
        time,
        attack_element_type,
        attack_element_power,
        element_defense,
        enchant_options,
    })
}

// Java: gameserver/network/serverpackets/AbstractInventoryUpdate.java (`writeItems`)
/// One entry of an inventory update: what happened to the item, then the item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemUpdate {
    /// Java `ItemInfo.getChange()`: 1 add, 2 modify, 3 remove (0 when no change was recorded).
    pub change: i16,
    pub item: ItemInfo,
}

/// Java `AbstractInventoryUpdate.writeItems`: a `u16` count, then change + item per entry.
///
/// NOTE(java-quirk): Java keeps the entries in a `HashMap<objectId, ItemInfo>`, so an item
/// appears once and the order is the map's iteration order. The caller passes the entries in the
/// order it wants and must not repeat an object id.
pub fn write_item_updates(w: &mut PacketWriter, updates: &[ItemUpdate]) {
    // Java: `writeShort(size)`, the low 16 bits of the count.
    w.u16(updates.len() as u16);
    for update in updates {
        w.i16(update.change);
        write_item(w, &update.item);
    }
}

/// Reads what [`write_item_updates`] wrote.
pub fn read_item_updates(r: &mut PacketReader) -> Result<Vec<ItemUpdate>, PacketError> {
    let count = r.u16()?;
    let mut updates = Vec::new();
    for _ in 0..count {
        let change = r.i16()?;
        let item = read_item(r)?;
        updates.push(ItemUpdate { change, item });
    }
    Ok(updates)
}

// Java: AbstractItemPacket.writeInventoryBlock (the PlayerInventory block fields)
/// Items the client may not use (`PlayerInventory.getBlockItems()` and `getBlockMode()`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryBlock {
    /// `getBlockMode()`: 0 blocks only the listed items, 1 allows only the listed items.
    pub mode: u8,
    /// `getBlockItems()`: template ids.
    pub items: Vec<i32>,
}

/// Java `AbstractItemPacket.writeInventoryBlock`: `None` (no block) is just a `0` short.
pub fn write_inventory_block(w: &mut PacketWriter, block: Option<&InventoryBlock>) {
    match block {
        Some(block) => {
            w.u16(block.items.len() as u16);
            w.u8(block.mode);
            for &item in &block.items {
                w.i32(item);
            }
        }
        None => {
            w.u16(0);
        }
    }
}

/// Reads what [`write_inventory_block`] wrote.
pub fn read_inventory_block(r: &mut PacketReader) -> Result<Option<InventoryBlock>, PacketError> {
    let count = r.u16()?;
    if count == 0 {
        return Ok(None);
    }
    let mode = r.u8()?;
    let mut items = Vec::new();
    for _ in 0..count {
        items.push(r.i32()?);
    }
    Ok(Some(InventoryBlock { mode, items }))
}

// Java: gameserver/network/serverpackets/ItemList.java
/// The inventory window contents, without quest items (`ExQuestItemList` carries those; Java
/// sends it from `ItemList.runImpl`, which is the caller's job).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ItemList {
    /// Open the inventory window on the client (Java writes the boolean as a short).
    pub show_window: bool,
    pub items: Vec<ItemInfo>,
    pub block: Option<InventoryBlock>,
}

impl ItemList {
    pub const OPCODE: ServerOpcode = ServerOpcode::ItemList;
}

impl Encode for ItemList {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.u16(u16::from(self.show_window));
        w.u16(self.items.len() as u16);
        for item in &self.items {
            write_item(w, item);
        }
        write_inventory_block(w, self.block.as_ref());
    }
}

impl Decode for ItemList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let show_window = r.u16()? != 0;
        let count = r.u16()?;
        let mut items = Vec::new();
        for _ in 0..count {
            items.push(read_item(r)?);
        }
        let block = read_inventory_block(r)?;
        Ok(Self {
            show_window,
            items,
            block,
        })
    }
}
