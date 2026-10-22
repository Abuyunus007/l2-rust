//! The manufacture shop: setting it up, closing it, looking at it and ordering from it.
// Java: network/clientpackets/{RequestRecipeShopMessageSet,RequestRecipeShopListSet,
//       RequestRecipeShopManageQuit,RequestRecipeShopMakeInfo,RequestRecipeShopMakeItem,
//       RequestRecipeShopManagePrev,RequestActionUse (actions 37 and 51)}.java
// Java: network/serverpackets/{RecipeShopManageList,RecipeShopSellList,RecipeShopMsg,
//       RecipeShopItemInfo}.java
// Java: entity/actor/Player.java (getManufactureItems, hasManufactureShop, doInteract, sendInfo)

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::client::RecipeShopListEntry;
use l2_protocol_game::server::{
    ActionFailed, ManufactureEntry, RecipeShopItemInfo, RecipeShopManageList, RecipeShopMsg,
    RecipeShopSellList, SystemMessage,
};
use l2_util::ObjectId;

use super::ManufactureItem;
use crate::trade::PrivateStoreType;
use crate::world::World;
use crate::zones::ZoneId;

/// `RequestRecipeShopMessageSet.MAX_MSG_LENGTH`.
const MAX_MSG_LENGTH: usize = 29;

/// `LocationUtil.checkIfInRange(150, player, manufacturer, true)` of `RequestRecipeShopMakeItem`.
const MAKE_ITEM_RANGE: i32 = 150;

impl World {
    // ------------------------------------------------------------------ the shop state

    /// Java `Player.hasManufactureShop()`.
    pub fn has_manufacture_shop(&self, id: ObjectId) -> bool {
        self.players
            .get(&id)
            .is_some_and(|p| !p.craft.manufacture.is_empty())
    }

    /// Java `Player.getManufactureItems()`: the recipes for sale with their prices, in the order
    /// they were put.
    pub fn manufacture_items(&self, id: ObjectId) -> Vec<(i32, i64)> {
        self.players
            .get(&id)
            .map(|p| {
                p.craft
                    .manufacture
                    .iter()
                    .map(|m| (m.recipe_id, m.cost))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Java `Player.getStoreName()`.
    pub fn recipe_store_name(&self, id: ObjectId) -> String {
        self.players
            .get(&id)
            .map(|p| p.craft.store_name.clone())
            .unwrap_or_default()
    }

    /// `getManufactureItems().put(recipeId, new ManufactureItem(recipeId, cost))` of a
    /// `LinkedHashMap`: a recipe that is there keeps its place and gets the new price.
    pub(crate) fn put_manufacture_item(&mut self, id: ObjectId, recipe_id: i32, cost: i64) {
        let Some(p) = self.players.get_mut(&id) else {
            return;
        };
        match p.craft.manufacture.iter_mut().find(|m| m.recipe_id == recipe_id) {
            Some(existing) => existing.cost = cost,
            None => p.craft.manufacture.push(ManufactureItem { recipe_id, cost }),
        }
    }

    fn manufacture_entries(&self, id: ObjectId) -> Vec<ManufactureEntry> {
        self.players
            .get(&id)
            .map(|p| {
                p.craft
                    .manufacture
                    .iter()
                    .map(|m| ManufactureEntry {
                        recipe_id: m.recipe_id,
                        cost: m.cost,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// `ManufactureItem.isDwarven()`: read from the recipe when the item was made.
    fn manufacture_item_is_dwarven(&self, recipe_id: i32) -> Option<bool> {
        self.recipe_data()?.get(recipe_id).map(|r| r.is_dwarven)
    }

    // ------------------------------------------------------------------ the packets

    /// Java `new RecipeShopManageList(seller, isDwarven)`. The constructor first drops the shop
    /// items that are of the other kind or that the seller does not know any more.
    pub(crate) fn recipe_shop_manage_list(
        &mut self,
        seller: ObjectId,
        is_dwarven: bool,
    ) -> Option<RecipeShopManageList> {
        let recipes = if is_dwarven && self.has_dwarven_craft(seller) {
            self.dwarven_recipe_book(seller)
        } else {
            self.common_recipe_book(seller)
        };
        if self.has_manufacture_shop(seller) {
            let keep: Vec<ManufactureItem> = self
                .players
                .get(&seller)?
                .craft
                .manufacture
                .iter()
                .copied()
                .filter(|m| {
                    self.manufacture_item_is_dwarven(m.recipe_id) == Some(is_dwarven)
                        && self.has_recipe_list(seller, m.recipe_id)
                })
                .collect();
            self.players.get_mut(&seller)?.craft.manufacture = keep;
        }
        Some(RecipeShopManageList {
            seller_id: seller.0,
            // `(int) _seller.getAdena()`
            adena: self.adena_of(seller) as i32,
            is_common: !is_dwarven,
            recipes,
            items: self.manufacture_entries(seller),
        })
    }

    /// Java `new RecipeShopSellList(buyer, manufacturer)`.
    pub(crate) fn recipe_shop_sell_list(
        &self,
        buyer: ObjectId,
        manufacturer: ObjectId,
    ) -> Option<RecipeShopSellList> {
        let m = self.players.get(&manufacturer)?;
        Some(RecipeShopSellList {
            manufacturer_id: manufacturer.0,
            cur_mp: m.core.cur_mp as i32,
            max_mp: m.core.max_mp as i32,
            buyer_adena: self.adena_of(buyer),
            items: self.manufacture_entries(manufacturer),
        })
    }

    /// Java `new RecipeShopItemInfo(player, recipeId)`.
    pub(crate) fn recipe_shop_item_info(&self, shop: ObjectId, recipe_id: i32) -> Option<RecipeShopItemInfo> {
        let p = self.players.get(&shop)?;
        Some(RecipeShopItemInfo {
            object_id: shop.0,
            recipe_id,
            cur_mp: p.core.cur_mp as i32,
            max_mp: p.core.max_mp as i32,
        })
    }

    /// Java `new RecipeShopMsg(player)`.
    pub(crate) fn recipe_shop_msg(&self, id: ObjectId) -> RecipeShopMsg {
        RecipeShopMsg {
            object_id: id.0,
            message: self.recipe_store_name(id),
        }
    }

    // ------------------------------------------------------------------ the actions

    /// `RequestActionUse` actions 37 (dwarven manufacture) and 51 (general manufacture): the
    /// manage window of the shop.
    // Action 37 also refuses a mounted player and one who sells buffs (`SellBuffsManager`); the
    // card for actions is `M6-ACTION-01`, mounts and buff shops do not exist yet.
    pub(crate) fn action_manufacture(&mut self, id: ObjectId, dwarven: bool) {
        // Player shouldn't be able to set stores if he/she is alike dead (dead or fake death).
        if self.is_alike_dead(id) {
            self.send(id, &ActionFailed);
            return;
        }
        if self.is_in_store_mode(id) {
            self.set_private_store_type(id, PrivateStoreType::None);
            self.broadcast_user_info(id);
        }
        if self.is_sitting(id) {
            self.stand_up(id);
        }
        if let Some(list) = self.recipe_shop_manage_list(id, dwarven) {
            self.send(id, &list);
        }
    }

    /// Java `RequestRecipeShopMessageSet.runImpl`.
    pub(crate) fn on_request_recipe_shop_message_set(&mut self, id: ObjectId, name: &str) {
        if name.encode_utf16().count() > MAX_MSG_LENGTH {
            let text = format!("{} tried to overflow recipe shop message", self.player_text(id));
            self.store_illegal_action(id, &text);
            return;
        }
        if let Some(p) = self.players.get_mut(&id) {
            p.craft.store_name = name.to_string();
        }
    }

    /// Java `RequestRecipeShopListSet.runImpl` (and the read that drops a bad list).
    pub(crate) fn on_request_recipe_shop_list_set(
        &mut self,
        id: ObjectId,
        items: Option<Vec<RecipeShopListEntry>>,
    ) {
        // `count > MAX_ITEM_IN_PACKET` makes the list bad.
        let max = usize::try_from(self.settings.store.max_item_in_packet).unwrap_or(0);
        let items = items.filter(|list| list.len() <= max);
        if let Some(list) = &items {
            // NOTE(java-quirk): `new ManufactureItem(id, cost)` reads the recipe, so a recipe id
            // that does not exist throws inside `readImpl` and the packet is dropped without a
            // word (the shop is not closed either).
            let data = self.recipe_data();
            if list
                .iter()
                .any(|e| data.as_ref().is_none_or(|d| d.get(e.recipe_id).is_none()))
            {
                return;
            }
        }
        let Some(items) = items else {
            self.set_private_store_type(id, PrivateStoreType::None);
            self.broadcast_user_info(id);
            return;
        };

        if self.has_attack_stance_task(id) || self.is_in_duel(id) {
            self.send(
                id,
                &SystemMessage::new(
                    SystemMessageId::WhileYouAreEngagedInCombatYouCannotOperateAPrivateStoreOrPrivateWorkshop,
                ),
            );
            self.send(id, &ActionFailed);
            return;
        }
        if self.is_inside_zone(id, ZoneId::NoStore) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouCannotOpenAPrivateWorkshopHere),
            );
            self.send(id, &ActionFailed);
            return;
        }
        if !self.can_open_private_store(id) {
            self.send(id, &ActionFailed);
            return;
        }

        if let Some(p) = self.players.get_mut(&id) {
            p.craft.manufacture.clear();
        }
        let max_adena = self.settings.ground.max_adena;
        for entry in &items {
            if !self.has_recipe_list(id, entry.recipe_id) {
                let text = format!(
                    "Warning!! {} of account {} tried to set recipe which he does not have.",
                    self.player_text(id),
                    self.account_of(id)
                );
                self.store_illegal_action(id, &text);
                return;
            }
            if entry.cost > max_adena {
                let text = format!(
                    "Warning!! {} of account {} tried to set price more than {max_adena} adena in Private Manufacture.",
                    self.player_text(id),
                    self.account_of(id)
                );
                self.store_illegal_action(id, &text);
                return;
            }
            self.put_manufacture_item(id, entry.recipe_id, entry.cost);
        }

        if !self.has_manufacture_shop(id)
            && let Some(p) = self.players.get_mut(&id)
        {
            p.craft.store_name = String::new();
        }
        self.set_private_store_type(id, PrivateStoreType::Manufacture);
        self.sit_down(id, true);
        self.broadcast_user_info(id);
        let msg = self.recipe_shop_msg(id);
        self.broadcast_to_self_and_visible(id, &msg);
    }

    /// Java `RequestRecipeShopManageQuit.runImpl`.
    pub(crate) fn on_request_recipe_shop_manage_quit(&mut self, id: ObjectId) {
        self.set_private_store_type(id, PrivateStoreType::None);
        self.broadcast_user_info(id);
        self.stand_up(id);
    }

    /// Java `RequestRecipeShopMakeInfo.runImpl`.
    pub(crate) fn on_request_recipe_shop_make_info(&mut self, id: ObjectId, shop_id: i32, recipe_id: i32) {
        let shop = ObjectId(shop_id);
        if self.private_store_type(shop) != PrivateStoreType::Manufacture {
            return;
        }
        if let Some(info) = self.recipe_shop_item_info(shop, recipe_id) {
            self.send(id, &info);
        }
    }

    /// Java `RequestRecipeShopManagePrev.runImpl`.
    pub(crate) fn on_request_recipe_shop_manage_prev(&mut self, id: ObjectId) {
        let target = self.target_of(id).filter(|t| self.is_player(*t));
        let Some(target) = target.filter(|_| !self.is_alike_dead(id)) else {
            self.send(id, &ActionFailed);
            return;
        };
        if let Some(list) = self.recipe_shop_sell_list(id, target) {
            self.send(id, &list);
        }
    }

    /// Java `RequestRecipeShopMakeItem.runImpl`.
    // The flood protector `canManufacture` is not modelled.
    pub(crate) fn on_request_recipe_shop_make_item(
        &mut self,
        id: ObjectId,
        manufacturer: i32,
        recipe_id: i32,
    ) {
        let manufacturer = ObjectId(manufacturer);
        if !self.players.contains_key(&manufacturer) {
            return;
        }
        let instance = |o: ObjectId| self.objects.get(&o).map_or(0, |e| e.instance_id);
        // `(manufacturer.getInstanceId() != player.getInstanceId()) && (player.getInstanceId() != -1)`
        if instance(manufacturer) != instance(id) {
            return;
        }
        if self.is_in_store_mode(id) {
            self.send_message(id, "You cannot create items while trading.");
            return;
        }
        if self.private_store_type(manufacturer) != PrivateStoreType::Manufacture {
            return;
        }
        if self.is_crafting(id) || self.is_crafting(manufacturer) {
            self.send_message(id, "You are currently in Craft Mode.");
            return;
        }
        if self.in_shop_range(id, manufacturer, MAKE_ITEM_RANGE) {
            self.request_manufacture_item(manufacturer, recipe_id, id);
        }
    }
}
