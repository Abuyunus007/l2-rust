//! A player's inventory: equipping, unequipping, destroying and dropping items, the paperdoll
//! rules and the packets and database rows that follow them (M4-INV-01).
// Java: network/clientpackets/{UseItem,RequestUnEquipItem,RequestDestroyItem,RequestItemList,
//       RequestDropItem}.java
// Java: entity/itemcontainer/{Inventory,PlayerInventory,ItemContainer}.java
// Java: entity/item/instance/Item.java (setItemLocation, updateDatabase)
// Java: entity/actor/Player.java (useEquippableItem, refreshOverloaded, checkItemRestriction)
//
// # How it fits together
// The items of a player are `PlayerData::items` (one row per item, `loc`/`loc_data` as in the
// `items` table), so the paperdoll is not a second structure: slot `n` is the item with
// `loc == "PAPERDOLL"` and `loc_data == n`. Moving an item changes those two fields, exactly the
// columns Java's `Item.setItemLocation` changes.
//
// - `paperdoll.rs`: `Inventory.setPaperdollItem` and `equipItem` with Java's listener order
//   (armor set, bow/crossbow/rod, item skills, bracelet, stats, recorder), the body part tables.
// - `use_item.rs`: `UseItem`, `Player.useEquippableItem`, `RequestUnEquipItem`.
// - `handlers/`: the item handlers (potions, scrolls, herbs, boxes) and the item reuse (M5-ITEM-01).
// - `destroy.rs`: `RequestDestroyItem`, `RequestDropItem` (the inventory side), `RequestItemList`.
// - `notify.rs`: `broadcastUserInfo`, `broadcastModifiedStats`, the delayed `sendSkillList` /
//   `sendItemList`, `refreshWeight`.
// - `handlers/`: item handlers; `shots.rs` has the soulshots and spiritshots (M5-SHOT-01).
// - `db.rs`: `Item.updateDatabase()` as `PersistCommand::ItemChanges`.
// - `capacity.rs`: `validateCapacity` / `validateWeight` (checked when items are added).
//
// The stat side of an equip (calculators, item and armor set skills) is the stat engine's
// (`World::on_equip` / `on_unequip`, M4-STAT-01); this module moves the items and sends what
// Java sends around it.
//
// # Left out on purpose (each has a marker where it belongs)
// - Casting: `isCastingNow` and the cast-finished equip of `UseItem` (M5). The attack parts
//   (`isAttackingNow`, `abortAttack`, the equip at the attack's end) are in (M4-COMBAT-01).
// - Trade, private stores, mounts, summons, fishing, cursed weapons, transformations, the
//   olympiad, augmentations and element attributes: not modelled yet, treated as "not active".
// - The over-enchant protection of `UseItem` (needs `EnchantItemGroupsData`).
// - `ItemProcessType` logging (`LogItems`) and the GM audit.

mod add;
mod capacity;
mod db;
mod destroy;
pub mod handlers;
mod notify;
mod paperdoll;
mod use_item;

pub(crate) use paperdoll::body_part_of_worn_item;

use std::sync::Arc;

use l2_data::ItemTemplates;
use l2_data::item::ItemTemplate;
use l2_data::item::types::{EtcItemType, WeaponType};
use l2_protocol_game::server::ItemUpdate;
use l2_util::ObjectId;

use crate::player::ItemData;
use crate::timer::TimerId;
use crate::world::World;

pub use paperdoll::{body_part_of_paperdoll_slot, paperdoll_index_of};

/// `ItemLocation.INVENTORY` / `PAPERDOLL` as the `items.loc` column spells them.
pub const LOC_INVENTORY: &str = "INVENTORY";
pub const LOC_PAPERDOLL: &str = "PAPERDOLL";

/// The values of `General.ini` / `Player.ini` this module reads. `Default` is the Java default
/// of each key.
#[derive(Debug, Clone, PartialEq)]
pub struct InventorySettings {
    /// `GeneralConfig.ALLOW_DISCARDITEM` (`AllowDiscardItem`).
    pub allow_discard_item: bool,
    /// `GeneralConfig.DESTROY_ALL_ITEMS` (`DestroyAllItems`).
    pub destroy_all_items: bool,
    /// `GeneralConfig.GM_TRADE_RESTRICTED_ITEMS` (`GMTradeRestrictedItems`).
    pub gm_trade_restricted_items: bool,
    /// `GeneralConfig.GM_ITEM_RESTRICTION` (`GMItemRestriction`): GMs obey item conditions.
    pub gm_item_restriction: bool,
    /// `PlayerConfig.AUTO_LOOT_SLOT_LIMIT` (`AutoLootSlotLimit`).
    pub auto_loot_slot_limit: bool,
    /// `RatesConfig.RATE_EXTRACTABLE` (`RateExtractable`): the amounts a box gives.
    pub rate_extractable: f32,
}

impl Default for InventorySettings {
    fn default() -> Self {
        Self {
            allow_discard_item: true,
            destroy_all_items: false,
            gm_trade_restricted_items: false,
            gm_item_restriction: true,
            auto_loot_slot_limit: false,
            rate_extractable: 1.0,
        }
    }
}

/// The delayed sends of a player (Java keeps one scheduled task per kind and ignores or replaces
/// a second request). Network bookkeeping on the world thread, not game state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InventoryTasks {
    /// Java `_broadcastCharInfoTask`: set while a `CharInfo` broadcast is waiting.
    pub(crate) char_info: Option<TimerId>,
    /// Java `_skillListTask`.
    pub(crate) skill_list: Option<TimerId>,
    /// Java `_itemListTask` and the `open` flag of the last request.
    pub(crate) item_list: Option<(TimerId, bool)>,
}

/// Java `Item.isEquipable()`: has a body part and is not an arrow, bolt or lure.
pub(crate) fn is_equipable(template: &ItemTemplate) -> bool {
    template.body_part != l2_data::item::types::BodyPart::None
        && !matches!(
            etc_type(template),
            Some(EtcItemType::Arrow | EtcItemType::Bolt | EtcItemType::Lure)
        )
}

/// Java `getItemType()` of a weapon.
pub(crate) fn weapon_type(template: &ItemTemplate) -> Option<WeaponType> {
    template.weapon().map(|w| w.weapon_type)
}

/// Java `getItemType()` of an etc item.
pub(crate) fn etc_type(template: &ItemTemplate) -> Option<EtcItemType> {
    template.etc().map(|e| e.etc_type)
}

/// Java `addItemName(id)`: the item's name as text when the client is shown another id.
pub(crate) fn item_name_param(
    templates: Option<&ItemTemplates>,
    item_id: i32,
) -> l2_protocol_game::server::SystemMessageParam {
    use l2_protocol_game::server::SystemMessageParam;
    match templates.and_then(|t| t.get(item_id)) {
        Some(t) if t.display_id != item_id => SystemMessageParam::Text(t.name.clone()),
        _ => SystemMessageParam::ItemName(item_id),
    }
}

impl World {
    /// The item templates, shared (`Arc`) so a function can read them while it changes the
    /// world. `None` when no datapack is installed.
    pub(crate) fn item_templates(&self) -> Option<Arc<ItemTemplates>> {
        self.data.stat_data().map(|d| Arc::clone(&d.items))
    }

    /// Java `getInventory().getItemByObjectId(id)`: an item in the inventory or worn.
    pub(crate) fn inventory_item(&self, player: ObjectId, object_id: ObjectId) -> Option<&ItemData> {
        self.players
            .get(&player)?
            .data
            .items
            .iter()
            .find(|i| i.object_id == object_id && (i.loc == LOC_INVENTORY || i.loc == LOC_PAPERDOLL))
    }

    /// Java `getInventory().getPaperdollItem(slot)` as an object id.
    pub fn paperdoll_object(&self, player: ObjectId, slot: usize) -> Option<ObjectId> {
        let slot = i32::try_from(slot).ok()?;
        self.players
            .get(&player)?
            .data
            .items
            .iter()
            .find(|i| i.loc == LOC_PAPERDOLL && i.loc_data == slot)
            .map(|i| i.object_id)
    }

    /// `new ItemInfo(item)` for an item as it is now, with Java's change flag for equipping
    /// (`Item.getLastChange()` is `MODIFIED` after every `setPaperdollItem`).
    pub(crate) fn modified_entry(&self, player: ObjectId, object_id: ObjectId) -> Option<ItemUpdate> {
        let item = self.inventory_item(player, object_id)?.clone();
        Some(self.update_entry(&item, l2_protocol_game::server::InventoryUpdate::MODIFIED))
    }
}
