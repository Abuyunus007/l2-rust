//! The lists of the private stores: what the owner sees while it sets a store up
//! (`PrivateStoreManageList*`), what a customer sees (`PrivateStoreList*`), and the
//! `TradeList.adjustAvailableItem` / `updateItems` they use. Also `Player.doInteract` and the
//! store title a viewer gets with `sendInfo`.
// Java: network/serverpackets/{PrivateStoreManageListSell,PrivateStoreManageListBuy,
//       PrivateStoreListSell,PrivateStoreListBuy,AbstractItemPacket,PrivateStoreMsgSell,
//       PrivateStoreMsgBuy,ExPrivateStoreSetWholeMsg}.java
// Java: network/holders/{TradeList,TradeItem}.java (getAvailableItems, adjustAvailableItem,
//       updateItems)
// Java: entity/itemcontainer/PlayerInventory.java (getUniqueItems, getAvailableItems(TradeList),
//       adjustAvailableItem)
// Java: entity/actor/Player.java (doInteract, sendInfo)
// Java: entity/item/holders/ItemInfo.java (the `TradeItem` constructor)

use l2_data::item::{ADENA_ID, ItemTemplate};
use l2_protocol_game::server::{
    ActionFailed, BuyStoreEntry, ExPrivateStoreSetWholeMsg, ItemInfo, PrivateStoreListBuy,
    PrivateStoreListSell, PrivateStoreManageListBuy, PrivateStoreManageListSell, PrivateStoreMsgBuy,
    PrivateStoreMsgSell, SellStoreEntry, StoreCandidate,
};
use l2_util::ObjectId;

use super::store::PrivateStoreType;
use super::{TradeItem, TradeList};
use crate::data::ItemStatic;
use crate::enter::item_info;
use crate::inventory::{LOC_INVENTORY, LOC_PAPERDOLL, is_equipable};
use crate::player::ItemData;
use crate::world::{World, unix_ms_now};

/// `getReferencePrice() * 2` (an `int` product, written as a long).
fn double_reference_price(template: &ItemTemplate) -> i64 {
    i64::from(template.price.wrapping_mul(2))
}

impl TradeList {
    /// Java `TradeList.adjustAvailableItem(item)`: the entry that offers `item` for this list: a
    /// stackable item that is already on the list only offers what is left over, and nothing
    /// (`None`) when all of it is listed.
    pub(crate) fn adjust_available_item(
        &self,
        item: &ItemData,
        template: &ItemTemplate,
    ) -> Option<TradeItem> {
        if template.stackable {
            for listed in &self.items {
                if listed.item.item_id == item.item_id {
                    return if item.count <= listed.count {
                        None
                    } else {
                        Some(TradeItem {
                            object_id: item.object_id,
                            item: item.clone(),
                            count: item.count - listed.count,
                            price: i64::from(template.price),
                        })
                    };
                }
            }
        }
        Some(TradeItem {
            object_id: item.object_id,
            item: item.clone(),
            count: item.count,
            price: i64::from(template.price),
        })
    }
}

impl World {
    /// `new ItemInfo(tradeItem)` as `AbstractItemPacket.writeItem` writes it: the item as it was
    /// when it was listed, with the count that is offered. `from_item` is false for the entries of
    /// a buy list, which Java builds from templates (no elements: `Elementals.NONE`).
    pub(crate) fn store_item_info(&self, titem: &TradeItem, from_item: bool) -> ItemInfo {
        let item_id = titem.item.item_id;
        let stat = self
            .data
            .item(item_id)
            .unwrap_or_else(|| ItemStatic::unknown(item_id));
        let template = self.data.stat_data().and_then(|d| d.items.get(item_id));
        let mut snapshot = titem.item.clone();
        snapshot.object_id = titem.object_id;
        snapshot.count = titem.count;
        // `_equipped = 0`, `_augmentation = 0`, `_mana = -1`, `_time = -9999`.
        snapshot.loc = LOC_INVENTORY.to_string();
        snapshot.augmentation = None;
        snapshot.mana_left = -1;
        snapshot.time = -1;
        let mut info = item_info(&snapshot, &stat, template, unix_ms_now());
        if !from_item {
            info.attack_element_type = -2;
            info.attack_element_power = 0;
            info.element_defense = [0; 6];
        }
        info
    }

    /// Java `TradeList.updateItems()` on the sell list: an item the owner no longer has leaves the
    /// list, a count above what the owner has is cut down.
    pub(crate) fn update_sell_list_items(&mut self, owner: ObjectId) {
        let Some(mut list) = self.players.get(&owner).and_then(|p| p.trade.sell_list.clone()) else {
            return;
        };
        let mut index = 0;
        while index < list.items.len() {
            let titem = list.items[index].clone();
            let held = self.inventory_item(owner, titem.object_id).map(|i| i.count);
            if held.is_none() || titem.count < 1 {
                // `removeItem(objectId, -1, -1)` (it does nothing on a locked list).
                let before = list.items.len();
                list.remove_item(titem.object_id.0, -1, -1);
                if list.items.len() < before {
                    continue;
                }
            } else if let Some(held) = held
                && held < titem.count
            {
                list.items[index].count = held;
            }
            index += 1;
        }
        if let Some(slot) = self.sell_list_mut(owner) {
            *slot = list;
        }
    }

    /// Java `PlayerInventory.adjustAvailableItem(item)`: `item` (a buy list entry) is matched
    /// against the inventory of `customer`: the object id and enchant level of the first item with
    /// that id, and the count cut to what the customer has. Nothing is offered (count 0) when every
    /// copy of an equipable item is worn.
    fn adjust_to_inventory(&self, customer: ObjectId, item: &mut TradeItem) {
        let Some(player) = self.players.get(&customer) else {
            item.count = 0;
            return;
        };
        let templates = self.item_templates();
        let in_inventory = |i: &&ItemData| i.loc == LOC_INVENTORY || i.loc == LOC_PAPERDOLL;
        let mut not_all_equipped = false;
        for adj in player
            .data
            .items
            .iter()
            .filter(in_inventory)
            .filter(|i| i.item_id == item.item.item_id)
        {
            let equipable = templates
                .as_deref()
                .and_then(|t| t.get(adj.item_id))
                .is_some_and(is_equipable);
            if equipable {
                if adj.loc != LOC_PAPERDOLL {
                    not_all_equipped = true;
                }
            } else {
                not_all_equipped = true;
                break;
            }
        }
        if not_all_equipped
            && let Some(first) = player
                .data
                .items
                .iter()
                .filter(in_inventory)
                .find(|i| i.item_id == item.item.item_id)
        {
            item.object_id = first.object_id;
            item.item.enchant_level = first.enchant_level;
            if first.count < item.count {
                item.count = first.count;
            }
            return;
        }
        item.count = 0;
    }

    // ------------------------------------------------------------------ the owner's windows

    /// Java `new PrivateStoreManageListSell(player, isPackageSale)`.
    pub(crate) fn private_store_manage_list_sell(
        &mut self,
        id: ObjectId,
        package_sale: bool,
    ) -> Option<PrivateStoreManageListSell> {
        self.update_sell_list_items(id);
        let templates = self.item_templates()?;
        let player = self.players.get(&id)?;
        let sell = self.sell_list_copy(id);

        // `player.getInventory().getAvailableItems(player.getSellList())`.
        let mut item_list = Vec::new();
        for item in &player.data.items {
            let Some(template) = templates.get(item.item_id) else {
                continue;
            };
            if !self.is_item_available(id, item, template, false, false) {
                continue;
            }
            if let Some(adjusted) = sell.adjust_available_item(item, template) {
                item_list.push(StoreCandidate {
                    item: self.store_item_info(&adjusted, true),
                    reference_price: double_reference_price(template),
                });
            }
        }

        let sell_list = sell
            .items
            .iter()
            .map(|t| SellStoreEntry {
                item: self.store_item_info(t, true),
                price: t.price,
                reference_price: templates.get(t.item.item_id).map_or(0, double_reference_price),
            })
            .collect();
        Some(PrivateStoreManageListSell {
            object_id: id.0,
            package_sale,
            player_adena: self.adena_of(id),
            item_list,
            sell_list,
        })
    }

    /// Java `new PrivateStoreManageListBuy(player)`.
    pub(crate) fn private_store_manage_list_buy(&self, id: ObjectId) -> Option<PrivateStoreManageListBuy> {
        let templates = self.item_templates()?;
        let player = self.players.get(&id)?;
        let now_ms = unix_ms_now();

        // `getUniqueItems(false, true, true, false)`: one item per item id, not adena, the ones
        // that may be sold or bought.
        let mut unique: Vec<&ItemData> = Vec::new();
        for item in &player.data.items {
            if item.item_id == ADENA_ID {
                continue;
            }
            let duplicate = unique.iter().any(|added| added.item_id == item.item_id);
            let available = templates
                .get(item.item_id)
                .is_some_and(|t| self.is_item_available(id, item, t, false, false));
            if !duplicate && available {
                unique.push(item);
            }
        }
        let item_list = unique
            .into_iter()
            .map(|item| {
                let stat = self
                    .data
                    .item(item.item_id)
                    .unwrap_or_else(|| ItemStatic::unknown(item.item_id));
                StoreCandidate {
                    item: item_info(
                        item,
                        &stat,
                        self.data.stat_data().and_then(|d| d.items.get(item.item_id)),
                        now_ms,
                    ),
                    reference_price: templates.get(item.item_id).map_or(0, double_reference_price),
                }
            })
            .collect();

        let buy_list = self
            .buy_list_copy(id)
            .items
            .iter()
            .map(|t| BuyStoreEntry {
                item: self.store_item_info(t, false),
                price: t.price,
                reference_price: templates.get(t.item.item_id).map_or(0, double_reference_price),
                count: t.count,
            })
            .collect();
        Some(PrivateStoreManageListBuy {
            object_id: id.0,
            player_adena: self.adena_of(id),
            item_list,
            buy_list,
        })
    }

    // ------------------------------------------------------------------ the customer's windows

    /// Java `new PrivateStoreListSell(player, seller)`.
    // `seller.isSellingBuffs()` sends the buff menu instead (`SellBuffsManager`): not ported.
    pub(crate) fn private_store_list_sell(
        &self,
        customer: ObjectId,
        seller: ObjectId,
    ) -> Option<PrivateStoreListSell> {
        let templates = self.item_templates()?;
        let sell = self.sell_list_copy(seller);
        let items = sell
            .items
            .iter()
            .map(|t| SellStoreEntry {
                item: self.store_item_info(t, true),
                price: t.price,
                reference_price: templates.get(t.item.item_id).map_or(0, double_reference_price),
            })
            .collect();
        Some(PrivateStoreListSell {
            seller_object_id: seller.0,
            package_sale: sell.packaged,
            player_adena: self.adena_of(customer),
            items,
        })
    }

    /// Java `new PrivateStoreListBuy(player, storePlayer)`.
    pub(crate) fn private_store_list_buy(
        &mut self,
        customer: ObjectId,
        store: ObjectId,
    ) -> Option<PrivateStoreListBuy> {
        // NOTE(java-quirk): the constructor refreshes the store's *sell* list ("Update SellList for
        // case inventory content has changed"), not the buy list it shows.
        self.update_sell_list_items(store);
        let templates = self.item_templates()?;
        let buy = self.buy_list_copy(store);
        let items = buy
            .items
            .iter()
            .map(|listed| {
                // `getAvailableItems(inventory)`: a copy of each entry, matched to the customer.
                let mut copy = listed.clone();
                let store_count = copy.count;
                self.adjust_to_inventory(customer, &mut copy);
                BuyStoreEntry {
                    item: self.store_item_info(&copy, false),
                    price: copy.price,
                    reference_price: templates.get(copy.item.item_id).map_or(0, double_reference_price),
                    count: store_count,
                }
            })
            .collect();
        Some(PrivateStoreListBuy {
            store_object_id: store.0,
            player_adena: self.adena_of(customer),
            items,
        })
    }

    /// Java `Player.doInteract(target)` for a player: `ActionFailed`, then the store list of the
    /// target.
    pub(crate) fn do_interact_player(&mut self, id: ObjectId, target: ObjectId) {
        self.send(id, &ActionFailed);
        match self.private_store_type(target) {
            PrivateStoreType::Sell | PrivateStoreType::PackageSell => {
                if let Some(packet) = self.private_store_list_sell(id, target) {
                    self.send(id, &packet);
                }
            }
            PrivateStoreType::Buy => {
                if let Some(packet) = self.private_store_list_buy(id, target) {
                    self.send(id, &packet);
                }
            }
            PrivateStoreType::Manufacture => {
                if let Some(packet) = self.recipe_shop_sell_list(id, target) {
                    self.send(id, &packet);
                }
            }
            _ => {}
        }
    }

    /// The store title of the end of Java `Player.sendInfo(viewer)`: what a player who comes into
    /// view of `viewer` has written over its store.
    pub(crate) fn send_store_message(&self, subject: ObjectId, viewer: ObjectId) {
        match self.private_store_type(subject) {
            PrivateStoreType::Sell => {
                self.send(
                    viewer,
                    &PrivateStoreMsgSell {
                        object_id: subject.0,
                        message: self.sell_list_copy(subject).title,
                    },
                );
            }
            PrivateStoreType::PackageSell => {
                self.send(
                    viewer,
                    &ExPrivateStoreSetWholeMsg {
                        object_id: subject.0,
                        message: self.sell_list_copy(subject).title,
                    },
                );
            }
            PrivateStoreType::Buy => {
                self.send(
                    viewer,
                    &PrivateStoreMsgBuy {
                        object_id: subject.0,
                        message: self.buy_list_copy(subject).title,
                    },
                );
            }
            PrivateStoreType::Manufacture => {
                self.send(viewer, &self.recipe_shop_msg(subject));
            }
            _ => {}
        }
    }
}
