//! The containers: which `Vec<ItemData>` a warehouse is, its owner column, its location name and
//! its capacity.
// Java: entity/itemcontainer/{ItemContainer,PlayerWarehouse,ClanWarehouse,PlayerFreight}.java
// Java: entity/actor/Player.java (getWareHouseLimit)

use l2_data::Stat;
use l2_util::ObjectId;

use crate::data::RACE_DWARF;
use crate::player::ItemData;
use crate::world::World;

/// `ItemLocation.WAREHOUSE` as the `items.loc` column spells it.
pub const LOC_WAREHOUSE: &str = "WAREHOUSE";
/// `ItemLocation.CLANWH`.
pub const LOC_CLANWH: &str = "CLANWH";
/// `ItemLocation.FREIGHT`.
pub const LOC_FREIGHT: &str = "FREIGHT";

/// Which warehouse a player works with (Java `_activeWarehouse`, and the containers it points to).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ContainerRef {
    /// `PlayerWarehouse` of this player.
    Private(ObjectId),
    /// `ClanWarehouse` of this clan.
    Clan(i32),
    /// `PlayerFreight`: the freight of the character `owner`, held in the world by the player
    /// `holder` (the same player, or the one who sends a package to `owner`).
    Freight { holder: ObjectId, owner: ObjectId },
}

impl ContainerRef {
    /// Java `instanceof PlayerWarehouse`.
    pub fn is_private(self) -> bool {
        matches!(self, Self::Private(_))
    }

    /// Java `instanceof ClanWarehouse`.
    pub fn is_clan(self) -> bool {
        matches!(self, Self::Clan(_))
    }

    /// Java `getOwnerId()`: the `owner_id` column of the rows.
    pub fn owner_id(self) -> i32 {
        match self {
            Self::Private(player) => player.0,
            Self::Clan(clan) => clan,
            Self::Freight { owner, .. } => owner.0,
        }
    }

    /// Java `getBaseLocation()` as the `items.loc` column spells it.
    pub fn base_location(self) -> &'static str {
        match self {
            Self::Private(_) => LOC_WAREHOUSE,
            Self::Clan(_) => LOC_CLANWH,
            Self::Freight { .. } => LOC_FREIGHT,
        }
    }
}

impl World {
    /// The items of a container (Java `getItems()`), `None` when the player or clan is gone.
    /// A freight nobody put anything in is an empty list.
    pub(crate) fn container_items(&self, container: ContainerRef) -> Option<&[ItemData]> {
        match container {
            ContainerRef::Private(player) => Some(&self.players.get(&player)?.warehouse.private),
            ContainerRef::Clan(clan) => Some(&self.clan(clan)?.warehouse),
            ContainerRef::Freight { holder, owner } => {
                let freight = &self.players.get(&holder)?.warehouse.freight;
                Some(freight.get(&owner).map_or(&[][..], Vec::as_slice))
            }
        }
    }

    pub(crate) fn container_items_mut(&mut self, container: ContainerRef) -> Option<&mut Vec<ItemData>> {
        match container {
            ContainerRef::Private(player) => Some(&mut self.players.get_mut(&player)?.warehouse.private),
            ContainerRef::Clan(clan) => Some(&mut self.clan_mut(clan)?.warehouse),
            ContainerRef::Freight { holder, owner } => Some(
                self.players
                    .get_mut(&holder)?
                    .warehouse
                    .freight
                    .entry(owner)
                    .or_default(),
            ),
        }
    }

    /// Java `getSize()`.
    pub(crate) fn container_size(&self, container: ContainerRef) -> usize {
        self.container_items(container).map_or(0, <[ItemData]>::len)
    }

    /// Java `getItemByObjectId(objectId)`.
    pub(crate) fn container_item(&self, container: ContainerRef, object_id: ObjectId) -> Option<&ItemData> {
        self.container_items(container)?
            .iter()
            .find(|i| i.object_id == object_id)
    }

    /// Java `getItemByItemId(itemId)`: the first item with the template id.
    pub(crate) fn container_item_by_item_id(
        &self,
        container: ContainerRef,
        item_id: i32,
    ) -> Option<&ItemData> {
        self.container_items(container)?
            .iter()
            .find(|i| i.item_id == item_id)
    }

    /// Java `Player.getWareHouseLimit()`: the slots of the race plus the `WH_LIM` stat.
    pub(crate) fn warehouse_limit(&self, player: ObjectId) -> i64 {
        let Some(p) = self.players.get(&player) else {
            return 0;
        };
        let base = if p.data.race == RACE_DWARF {
            self.settings.warehouse_slots_dwarf
        } else {
            self.settings.warehouse_slots_no_dwarf
        };
        let bonus = self.stats(player).map_or(0.0, |view| view.calc(Stat::WhLim, 0.0));
        i64::from(base) + bonus as i64
    }

    /// Java `validateCapacity(slots)` of each container: `size + slots` within the limit.
    pub(crate) fn container_validate_capacity(&self, container: ContainerRef, slots: i64) -> bool {
        let size = i64::try_from(self.container_size(container)).unwrap_or(i64::MAX);
        let limit = match container {
            ContainerRef::Private(player) => self.warehouse_limit(player),
            ContainerRef::Clan(_) => i64::from(self.settings.warehouse_slots_clan),
            // `_owner == null ? ALT_FREIGHT_SLOTS : ALT_FREIGHT_SLOTS + FREIGHT_LIM`: only the
            // freight built for the player itself has an owner. A package for another character
            // is a bare `new PlayerFreight(objectId)`.
            ContainerRef::Freight { holder, owner } => {
                let base = i64::from(self.settings.warehouse.freight_slots);
                if holder == owner {
                    let bonus = self
                        .stats(holder)
                        .map_or(0.0, |view| view.calc(Stat::FreightLim, 0.0));
                    base + bonus as i64
                } else {
                    base
                }
            }
        };
        size.saturating_add(slots) <= limit
    }

    /// Keeps the items whose template the datapack knows (Java `Item.restoreFromDb` returns
    /// `null` for the others, so they never join a container; their rows stay in the table).
    pub(crate) fn drop_unknown_items(&self, items: &mut Vec<ItemData>) {
        if let Some(templates) = self.item_templates() {
            items.retain(|i| templates.get(i.item_id).is_some());
        }
    }

    /// The warehouses of a player that entered: unknown templates leave the lists
    /// ([`drop_unknown_items`](Self::drop_unknown_items)).
    pub(crate) fn clean_loaded_warehouses(&mut self, player: ObjectId) {
        let Some(mut taken) = self.players.get_mut(&player).map(|p| {
            (
                std::mem::take(&mut p.warehouse.private),
                std::mem::take(&mut p.warehouse.freight),
            )
        }) else {
            return;
        };
        self.drop_unknown_items(&mut taken.0);
        for items in taken.1.values_mut() {
            self.drop_unknown_items(items);
        }
        if let Some(p) = self.players.get_mut(&player) {
            p.warehouse.private = taken.0;
            p.warehouse.freight = taken.1;
        }
    }
}
