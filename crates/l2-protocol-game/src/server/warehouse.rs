//! Server packets of the warehouses and the freight (M6-WH-01).
//!
//! Values only, like the other item packets: the world builds the [`ItemInfo`]s (Java reads them
//! from the `Item`s of the container) and decides what goes in which list.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::items::{ItemInfo, read_item, write_item};
use super::write_opcode;
use crate::generated::opcodes::ServerOpcode;

/// `WareHouseDepositList.PRIVATE` / `WareHouseWithdrawalList.PRIVATE` / `FREIGHT`: the type the
/// private warehouse and the freight window are shown with.
pub const WH_TYPE_PRIVATE: i16 = 1;
/// `WareHouseWithdrawalList.CLAN`.
pub const WH_TYPE_CLAN: i16 = 4;
/// `WareHouseWithdrawalList.CASTLE` ("not sure" in Java).
pub const WH_TYPE_CASTLE: i16 = 3;
/// `WareHouseWithdrawalList.FREIGHT` (the same value as the private warehouse).
pub const WH_TYPE_FREIGHT: i16 = 1;

/// `SortedWareHouseWithdrawalList.PRIVATE`.
pub const SORTED_WH_TYPE_PRIVATE: i16 = 1;
/// `SortedWareHouseWithdrawalList.CLAN`. NOTE(java-quirk): not the 4 of the unsorted list.
pub const SORTED_WH_TYPE_CLAN: i16 = 2;
/// `SortedWareHouseWithdrawalList.CASTLE` ("not sure" in Java).
pub const SORTED_WH_TYPE_CASTLE: i16 = 3;
/// `SortedWareHouseWithdrawalList.FREIGHT` ("not sure" in Java).
pub const SORTED_WH_TYPE_FREIGHT: i16 = 4;

fn write_item_list(w: &mut PacketWriter, wh_type: i16, player_adena: i64, items: &[ItemInfo]) {
    w.i16(wh_type).i64(player_adena);
    // Java: `writeShort(size)`, the low 16 bits of the count.
    w.u16(items.len() as u16);
    for item in items {
        write_item(w, item);
        // Java repeats the item's object id after the item block.
        w.i32(item.object_id);
    }
}

fn read_item_list(r: &mut PacketReader) -> Result<(i16, i64, Vec<ItemInfo>), PacketError> {
    let wh_type = r.i16()?;
    let player_adena = r.i64()?;
    let count = r.u16()?;
    let mut items = Vec::new();
    for _ in 0..count {
        let item = read_item(r)?;
        r.skip(4)?;
        items.push(item);
    }
    Ok((wh_type, player_adena, items))
}

// Java: gameserver/network/serverpackets/WareHouseDepositList.java
/// The deposit window: what the player can put into the warehouse (`WAREHOUSE_DEPOSIT_LIST`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WareHouseDepositList {
    /// [`WH_TYPE_PRIVATE`] or [`WH_TYPE_CLAN`].
    pub wh_type: i16,
    pub player_adena: i64,
    pub items: Vec<ItemInfo>,
}

impl WareHouseDepositList {
    pub const OPCODE: ServerOpcode = ServerOpcode::WarehouseDepositList;
}

impl Encode for WareHouseDepositList {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        write_item_list(w, self.wh_type, self.player_adena, &self.items);
    }
}

impl Decode for WareHouseDepositList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let (wh_type, player_adena, items) = read_item_list(r)?;
        Ok(Self {
            wh_type,
            player_adena,
            items,
        })
    }
}

// Java: gameserver/network/serverpackets/WareHouseWithdrawalList.java
/// The withdraw window: what the warehouse or the freight holds (`WAREHOUSE_WITHDRAW_LIST`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WareHouseWithdrawalList {
    /// [`WH_TYPE_PRIVATE`], [`WH_TYPE_CLAN`] or [`WH_TYPE_FREIGHT`].
    pub wh_type: i16,
    pub player_adena: i64,
    pub items: Vec<ItemInfo>,
}

impl WareHouseWithdrawalList {
    pub const OPCODE: ServerOpcode = ServerOpcode::WarehouseWithdrawList;
}

impl Encode for WareHouseWithdrawalList {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        write_item_list(w, self.wh_type, self.player_adena, &self.items);
    }
}

impl Decode for WareHouseWithdrawalList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let (wh_type, player_adena, items) = read_item_list(r)?;
        Ok(Self {
            wh_type,
            player_adena,
            items,
        })
    }
}

// Java: gameserver/network/serverpackets/SortedWareHouseWithdrawalList.java
// Java: gameserver/entity/item/WarehouseItem.java
/// One item of the sorted withdraw list: the fields of Java's `WarehouseItem`, in the order
/// `SortedWareHouseWithdrawalList.writeImpl` writes them. The bytes are those of [`ItemInfo`] (the
/// "equipped" short is a constant 0), but the values come from a `WarehouseItem`, which differs
/// in one place: a time limited item without a lifetime shows -1 here, -9999 in an `ItemInfo`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SortedWarehouseItem {
    pub object_id: i32,
    /// `getItem().getDisplayId()`.
    pub display_id: i32,
    /// `getLocationSlot()`.
    pub location_slot: i32,
    pub count: i64,
    /// `getItem().getType2()`.
    pub type2: i16,
    pub custom_type1: i16,
    /// `getItem().getBodyPart().getMask()`.
    pub body_part: i32,
    pub enchant: i16,
    pub custom_type2: i16,
    /// The augmentation id, 0 when the item is not augmented.
    pub augmentation: i32,
    /// `getMana()`.
    pub mana: i32,
    /// `isTimeLimitedItem() ? remaining seconds : -1`. NOTE(java-quirk): -1, where `ItemInfo`
    /// shows -9999 for an item without a lifetime.
    pub time: i32,
    pub attack_element_type: i16,
    pub attack_element_power: i16,
    pub element_defense: [i16; 6],
    pub enchant_options: [i16; 3],
}

/// The sorted withdraw list (`WAREHOUSE_WITHDRAW_LIST` again, with another item layout).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SortedWareHouseWithdrawalList {
    /// One of the `SORTED_WH_TYPE_*` values.
    pub wh_type: i16,
    pub player_adena: i64,
    pub items: Vec<SortedWarehouseItem>,
}

impl SortedWareHouseWithdrawalList {
    pub const OPCODE: ServerOpcode = ServerOpcode::WarehouseWithdrawList;
}

impl Encode for SortedWareHouseWithdrawalList {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i16(self.wh_type).i64(self.player_adena);
        w.u16(self.items.len() as u16);
        for item in &self.items {
            w.i32(item.object_id).i32(item.display_id).i32(item.location_slot);
            w.i64(item.count);
            w.i16(item.type2).i16(item.custom_type1);
            // "Can't be equipped in WH".
            w.i16(0);
            w.i32(item.body_part);
            w.i16(item.enchant).i16(item.custom_type2);
            w.i32(item.augmentation).i32(item.mana).i32(item.time);
            w.i16(item.attack_element_type).i16(item.attack_element_power);
            for def in item.element_defense {
                w.i16(def);
            }
            for option in item.enchant_options {
                w.i16(option);
            }
            w.i32(item.object_id);
        }
    }
}

impl Decode for SortedWareHouseWithdrawalList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let wh_type = r.i16()?;
        let player_adena = r.i64()?;
        let count = r.u16()?;
        let mut items = Vec::new();
        for _ in 0..count {
            let object_id = r.i32()?;
            let display_id = r.i32()?;
            let location_slot = r.i32()?;
            let item_count = r.i64()?;
            let type2 = r.i16()?;
            let custom_type1 = r.i16()?;
            r.skip(2)?;
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
            r.skip(4)?;
            items.push(SortedWarehouseItem {
                object_id,
                display_id,
                location_slot,
                count: item_count,
                type2,
                custom_type1,
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
            });
        }
        Ok(Self {
            wh_type,
            player_adena,
            items,
        })
    }
}

// Java: gameserver/network/serverpackets/PackageToList.java
/// The characters of the account a package can go to (`PACKAGE_TO_LIST`): `(object id, name)` in
/// the order of Java's `LinkedHashMap`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PackageToList {
    pub players: Vec<(i32, String)>,
}

impl PackageToList {
    pub const OPCODE: ServerOpcode = ServerOpcode::PackageToList;
}

impl Encode for PackageToList {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.players.len() as i32);
        for (object_id, name) in &self.players {
            w.i32(*object_id).string(name);
        }
    }
}

impl Decode for PackageToList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let count = r.i32()?;
        let mut players = Vec::new();
        for _ in 0..count.max(0) {
            let object_id = r.i32()?;
            players.push((object_id, r.string()));
        }
        Ok(Self { players })
    }
}

// Java: gameserver/network/serverpackets/PackageSendableList.java
/// The items the player can send to the character `player_object_id` (`PACKAGE_SENDABLE_LIST`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageSendableList {
    /// The receiving character's object id (the one the client asked about).
    pub player_object_id: i32,
    pub adena: i64,
    pub items: Vec<ItemInfo>,
}

impl PackageSendableList {
    pub const OPCODE: ServerOpcode = ServerOpcode::PackageSendableList;
}

impl Encode for PackageSendableList {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.player_object_id).i64(self.adena);
        w.i32(self.items.len() as i32);
        for item in &self.items {
            write_item(w, item);
            w.i32(item.object_id);
        }
    }
}

impl Decode for PackageSendableList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let player_object_id = r.i32()?;
        let adena = r.i64()?;
        let count = r.i32()?;
        let mut items = Vec::new();
        for _ in 0..count.max(0) {
            let item = read_item(r)?;
            r.skip(4)?;
            items.push(item);
        }
        Ok(Self {
            player_object_id,
            adena,
            items,
        })
    }
}
