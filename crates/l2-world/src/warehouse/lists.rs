//! The item lists the warehouse windows show: what can be deposited, what a container holds, and
//! what can be sent as a package.
// Java: network/serverpackets/{WareHouseDepositList,WareHouseWithdrawalList,PackageSendableList}.java
// Java: entity/itemcontainer/PlayerInventory.java (getAvailableItems)
// Java: entity/item/instance/Item.java (isDepositable, isShadowItem)

use l2_data::item::ItemTemplate;
use l2_protocol_game::server::{
    ItemInfo, PackageSendableList, WareHouseDepositList, WareHouseWithdrawalList,
};
use l2_util::ObjectId;

use crate::data::ItemStatic;
use crate::enter::item_info;
use crate::inventory::{LOC_INVENTORY, LOC_PAPERDOLL};
use crate::player::ItemData;
use crate::world::{World, unix_ms_now};

impl World {
    /// `new ItemInfo(item)` for an item of a container (or of the inventory).
    pub(crate) fn warehouse_item_info(&self, item: &ItemData, now_ms: i64) -> ItemInfo {
        let stat = self
            .data
            .item(item.item_id)
            .unwrap_or_else(|| ItemStatic::unknown(item.item_id));
        item_info(
            item,
            &stat,
            self.data.stat_data().and_then(|d| d.items.get(item.item_id)),
            now_ms,
        )
    }

    /// Java `PlayerInventory.getAvailableItems(allowAdena, allowNonTradeable, freightable)`: the
    /// items of the inventory a transaction may use. `canManipulateWithItemId` (the inventory
    /// block) is not modelled.
    pub(crate) fn available_items(
        &self,
        player: ObjectId,
        allow_adena: bool,
        allow_non_tradeable: bool,
        freightable: bool,
    ) -> Vec<ItemData> {
        let Some(p) = self.players.get(&player) else {
            return Vec::new();
        };
        let Some(templates) = self.item_templates() else {
            return Vec::new();
        };
        p.data
            .items
            .iter()
            .filter(|i| i.loc == LOC_INVENTORY || i.loc == LOC_PAPERDOLL)
            .filter(|i| {
                let Some(template) = templates.get(i.item_id) else {
                    return false;
                };
                if !self.is_item_available(player, i, template, allow_adena, allow_non_tradeable) {
                    return false;
                }
                // `else if (feightable)`: only what sits in the inventory and is freightable.
                !freightable || (i.loc == LOC_INVENTORY && template.freightable)
            })
            .cloned()
            .collect()
    }

    /// Java `Item.isDepositable(isPrivateWareHouse)`: the template allows it and the item is not
    /// worn; for the clan warehouse and the freight it must also be tradeable and no shadow item.
    pub(crate) fn item_is_depositable(
        &self,
        item: &ItemData,
        template: &ItemTemplate,
        is_private: bool,
    ) -> bool {
        if !template.depositable || item.loc == LOC_PAPERDOLL {
            return false;
        }
        // `isShadowItem()` is `_mana >= 0`.
        if !is_private && (!self.item_is_tradeable(item, template) || item.mana_left >= 0) {
            return false;
        }
        true
    }

    /// Java `new WareHouseDepositList(player, type)`: the items the player may put in.
    pub(crate) fn warehouse_deposit_list(
        &self,
        player: ObjectId,
        wh_type: i16,
    ) -> Option<WareHouseDepositList> {
        let is_private = wh_type == l2_protocol_game::server::warehouse::WH_TYPE_PRIVATE;
        let templates = self.item_templates()?;
        let now_ms = unix_ms_now();
        let items = self
            .available_items(player, true, is_private, false)
            .iter()
            .filter(|i| {
                templates
                    .get(i.item_id)
                    .is_some_and(|t| self.item_is_depositable(i, t, is_private))
            })
            .map(|i| self.warehouse_item_info(i, now_ms))
            .collect();
        Some(WareHouseDepositList {
            wh_type,
            player_adena: self.adena_of(player),
            items,
        })
    }

    /// Java `new WareHouseWithdrawalList(player, type)`: what the active warehouse holds. `None`
    /// without an active warehouse (Java logs "Error while sending withdraw request").
    pub(crate) fn warehouse_withdrawal_list(
        &self,
        player: ObjectId,
        wh_type: i16,
    ) -> Option<WareHouseWithdrawalList> {
        let active = self.players.get(&player)?.warehouse.active?;
        let now_ms = unix_ms_now();
        let items = self
            .container_items(active)?
            .iter()
            .map(|i| self.warehouse_item_info(i, now_ms))
            .collect();
        Some(WareHouseWithdrawalList {
            wh_type,
            player_adena: self.adena_of(player),
            items,
        })
    }

    /// Java `new PackageSendableList(items, objectId, adena)` for `RequestPackageSendableItemList`:
    /// the freightable inventory items.
    pub(crate) fn package_sendable_list(&self, player: ObjectId, receiver: i32) -> PackageSendableList {
        let now_ms = unix_ms_now();
        PackageSendableList {
            player_object_id: receiver,
            adena: self.adena_of(player),
            items: self
                .available_items(player, true, true, true)
                .iter()
                .map(|i| self.warehouse_item_info(i, now_ms))
                .collect(),
        }
    }
}
