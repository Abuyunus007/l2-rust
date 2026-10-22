//! The private warehouse, the clan warehouse and the freight: the NPC bypasses that open them,
//! the deposit and withdraw packets, the fees and the rows (M6-WH-01).
// Java: entity/itemcontainer/{ItemContainer,Warehouse,PlayerWarehouse,ClanWarehouse,PlayerFreight}.java
// Java: network/clientpackets/{SendWareHouseDepositList,SendWareHouseWithDrawList,RequestPackageSend,
//       RequestPackageSendableItemList}.java
// Java: network/serverpackets/{WareHouseDepositList,WareHouseWithdrawalList,
//       SortedWareHouseWithdrawalList,PackageToList,PackageSendableList}.java
// Java: dist/game/data/scripts/handlers/bypass/npc/{PrivateWarehouse,ClanWarehouse,Freight}.java
// Java: entity/actor/instance/Warehouse.java, entity/item/WarehouseItem.java
//
// # Where the items live
// A container is a `Vec<ItemData>` in the world, like the inventory is (D-10, D-11):
// - the private warehouse and the freight of a character are on its [`PlayerWarehouse`]
//   (`Player::warehouse`), read at the character select like the inventory (`WarehouseLoad`);
//   the freight of the *other* characters of the account is there too, because a package sent to
//   one of them has to join the stacks it already holds (Java restores `new PlayerFreight(id)`);
// - the clan warehouse is `Clan::warehouse`, read with the clan.
// - `Player::warehouse.active` is Java `_activeWarehouse`: which container the next deposit or
//   withdraw packet acts on, set by the bypasses ([`ContainerRef`]).
//
// # Rows (D-39)
// Java moves an item by changing `owner_id`, `loc` and `loc_data` of its row and calling
// `updateDatabase`. Every move here queues the same end state as `PersistCommand::ItemRows`
// (`ItemRowOp`): an `UPDATE` of the moved row (new owner and location), the `UPDATE` of a count,
// the `INSERT` of a new stack (a new object id of the Rust range) and the `DELETE` of a stack that
// went away whole. `items.loc` is `WAREHOUSE` for the private warehouse, `CLANWH` (owner = clan
// id) for the clan warehouse and `FREIGHT` (owner = the receiving character) for the freight.
//
// # Left out on purpose (each has a comment where it belongs)
// - The flood protector `canPerformTransaction`, `setInventoryBlockingStatus(true)` (nothing
//   reads the 1.5 s inventory lock yet), cursed weapons, the olympiad, a summoned pet's
//   control item.
// - The recipe level sort of the sorted list (`RecipeData` is the craft card's).
// - Clan halls and castles (`ClanHallManager`, `FortManager`, the vault): M7.
// - `ItemProcessType` logging (`LogItems`), the GM audit, the clan warehouse script events.
// - The duplicate stackable rows `ItemContainer.restore()` merges and deletes.

mod container;
mod handlers;
mod lists;
mod moves;
mod sorted;

pub use container::ContainerRef;
pub(crate) use moves::Place;
pub use sorted::WarehouseListType;

use std::collections::BTreeMap;

use l2_protocol_game::client::WarehouseListEntry;
use l2_util::ObjectId;

use crate::bypass::BypassHandlers;
use crate::player::ItemData;

/// The values of `Player.ini` / `Custom/WarehouseSorting.ini` the warehouses read. `Default` is
/// the Java default of each key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WarehouseSettings {
    /// `PlayerConfig.ALT_FREIGHT_SLOTS` (`MaximumFreightSlots`).
    pub freight_slots: i32,
    /// `PlayerConfig.ALT_FREIGHT_PRICE` (`FreightPrice`): adena per item sent.
    pub freight_price: i32,
    /// `PlayerConfig.ALT_MEMBERS_CAN_WITHDRAW_FROM_CLANWH` (`AltMembersCanWithdrawFromClanWH`).
    pub members_can_withdraw_from_clanwh: bool,
    /// `WarehouseSortingConfig.ENABLE_WAREHOUSESORTING_PRIVATE` (`EnableWarehouseSortingPrivate`).
    pub sorting_private: bool,
    /// `WarehouseSortingConfig.ENABLE_WAREHOUSESORTING_CLAN` (`EnableWarehouseSortingClan`).
    pub sorting_clan: bool,
}

impl Default for WarehouseSettings {
    fn default() -> Self {
        Self {
            freight_slots: 200,
            freight_price: 1000,
            members_can_withdraw_from_clanwh: false,
            sorting_private: false,
            sorting_clan: false,
        }
    }
}

/// What the session read for the warehouses before the character entered the world
/// (`PlayerProfile::warehouse`). The world takes it when the player is created
/// ([`Player::from_data`](crate::Player::from_data)), so snapshots never carry it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WarehouseLoad {
    /// `loc = 'WAREHOUSE'` rows of the character (Java `getWarehouse().restore()`).
    pub private: Vec<ItemData>,
    /// `loc = 'FREIGHT'` rows of the character itself and of each other character of the account
    /// (Java `getFreight().restore()`; the others are read for `RequestPackageSend`).
    pub freight: Vec<(ObjectId, Vec<ItemData>)>,
    /// Java `_chars`: `SELECT charId, char_name FROM characters WHERE account_name=? AND
    /// charId<>?`, in the database's order.
    pub account_chars: Vec<(ObjectId, String)>,
}

/// The warehouses of a player in the world (Java `_warehouse`, `_freight`, `_activeWarehouse`,
/// `_chars`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlayerWarehouse {
    /// Java `_warehouse`: the private warehouse.
    pub private: Vec<ItemData>,
    /// Java `_freight`, and the freight of the other characters of the account, by owner.
    pub freight: BTreeMap<ObjectId, Vec<ItemData>>,
    /// Java `_chars`: the other characters of the account, `(object id, name)` in Java's
    /// `LinkedHashMap` order.
    pub account_chars: Vec<(ObjectId, String)>,
    /// Java `_activeWarehouse`.
    pub active: Option<ContainerRef>,
}

impl PlayerWarehouse {
    /// The warehouses of a player that has just been read from the database.
    pub fn from_load(load: WarehouseLoad, player: ObjectId) -> Self {
        let mut freight: BTreeMap<ObjectId, Vec<ItemData>> = BTreeMap::new();
        for (owner, items) in load.freight {
            freight.entry(owner).or_default().extend(items);
        }
        // The own freight exists even when it is empty.
        freight.entry(player).or_default();
        Self {
            private: load.private,
            freight,
            account_chars: load.account_chars,
            active: None,
        }
    }
}

/// The warehouse packets as the world gets them (`ClientInput::Warehouse`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WarehouseInput {
    /// Java `SendWareHouseDepositList`; `None` is a list Java drops.
    Deposit { items: Option<Vec<WarehouseListEntry>> },
    /// Java `SendWareHouseWithDrawList`.
    Withdraw { items: Option<Vec<WarehouseListEntry>> },
    /// Java `RequestPackageSend`: the freight of the character `object_id`.
    PackageSend {
        object_id: i32,
        items: Option<Vec<WarehouseListEntry>>,
    },
    /// Java `RequestPackageSendableItemList`.
    PackageSendableList { object_id: i32 },
}

/// Registers the bypass commands of the warehouse NPCs (the `PrivateWarehouse`, `ClanWarehouse`
/// and `Freight` handlers; Java registers them under every word of `getCommandList()`).
pub fn register_bypass_handlers(handlers: &mut BypassHandlers) {
    handlers::register(handlers);
}
