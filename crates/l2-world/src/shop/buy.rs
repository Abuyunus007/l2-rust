//! Opening a merchant's buy window (`Merchant.showBuyWindow`, the `Buy` bypass) and
//! `RequestBuyItem`.
// Java: entity/actor/instance/Merchant.java (showBuyWindow)
// Java: dist/game/data/scripts/handlers/bypass/npc/Buy.java
// Java: network/clientpackets/RequestBuyItem.java

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::client::{BuyItemEntry, RequestBuyItem};
use l2_protocol_game::server::{ActionFailed, SystemMessage};
use l2_util::ObjectId;
use tracing::warn;

use super::CUSTOM_CB_SELL_LIST;
use super::packets::is_siege_guard_item;
use crate::dialog::INTERACTION_DISTANCE;
use crate::target::distance_3d;
use crate::world::World;

/// The `Buy` bypass (Java `Buy.onCommand`): `Buy <list id>` on a merchant opens its buy window.
pub fn buy_handler(world: &mut World, player: ObjectId, command: &str, target: Option<ObjectId>) -> bool {
    let Some(npc) = target.filter(|t| world.npc_is_merchant(*t)) else {
        return false;
    };
    // `StringTokenizer(command, " ")`: the command, then the list id.
    let mut tokens = command.split(' ').filter(|t| !t.is_empty());
    tokens.next();
    let Some(value) = tokens.next() else {
        return false;
    };
    match value.parse::<i32>() {
        Ok(list_id) => {
            world.show_buy_window(npc, player, list_id);
            true
        }
        Err(_) => {
            warn!("Exception in Buy: bad list id {value}");
            false
        }
    }
}

impl World {
    /// `target instanceof Merchant`: the NPC's class is `Merchant` or one of its subclasses.
    pub(crate) fn npc_is_merchant(&self, npc: ObjectId) -> bool {
        self.npcs
            .get(&npc)
            .and_then(|n| self.npc_world.template(n.npc_id))
            .is_some_and(|t| crate::dialog::is_merchant(&t.npc_type))
    }

    /// Java `Merchant.showBuyWindow(player, value)` (with the tax).
    pub(crate) fn show_buy_window(&mut self, npc: ObjectId, player: ObjectId, list_id: i32) {
        let npc_id = self.npcs.get(&npc).map_or(0, |n| n.npc_id);
        let allowed = match self.shop.lists.as_ref().and_then(|l| l.get(list_id)) {
            None => {
                warn!("BuyList not found! BuyListId:{list_id}");
                self.send(player, &ActionFailed);
                return;
            }
            Some(list) => list.is_npc_allowed(npc_id),
        };
        if !allowed {
            warn!("Npc not allowed in BuyList! BuyListId:{list_id} NpcId:{npc_id}");
            self.send(player, &ActionFailed);
            return;
        }

        let tax_rate = self.merchant_total_tax_rate(npc);
        self.set_inventory_blocking_status(player);
        if let Some(packet) = self.buy_list_packet(player, list_id, tax_rate) {
            self.send(player, &packet);
        }
        self.send_ex_buy_sell_list(player, false);
        self.send(player, &ActionFailed);
    }

    /// The target check the three packets share: unless the player is a GM or uses the community
    /// board list, the target must be a merchant within `INTERACTION_DISTANCE` in the same
    /// instance. `Err` means `ActionFailed` went out; `Ok(None)` is a GM or the community board.
    pub(super) fn shop_merchant(&mut self, id: ObjectId, list_id: i32) -> Result<Option<ObjectId>, ()> {
        if self.is_gm(id) || list_id == CUSTOM_CB_SELL_LIST {
            return Ok(None);
        }
        let target = self.target_of(id).filter(|t| self.npc_is_merchant(*t));
        let Some(target) = target else {
            self.send(id, &ActionFailed);
            return Err(());
        };
        let (Some(me), Some(there)) = (self.position_of(id), self.position_of(target)) else {
            self.send(id, &ActionFailed);
            return Err(());
        };
        let instance_of = |world: &World, who: ObjectId| world.objects.get(&who).map_or(0, |e| e.instance_id);
        if distance_3d(me, there) >= f64::from(INTERACTION_DISTANCE)
            || instance_of(self, id) != instance_of(self, target)
        {
            self.send(id, &ActionFailed);
            return Err(());
        }
        Ok(Some(target))
    }

    /// `PunishmentManager.handleIllegalPlayerAction(player, "Warning!! Character <name> of account
    /// <account> <what>", DEFAULT_PUNISH)`.
    pub(super) fn shop_illegal_action(&mut self, id: ObjectId, what: &str) {
        let name = self.players.get(&id).map_or("?", |p| p.data.name.as_str());
        let message = format!(
            "Warning!! Character {name} of account {} {what}",
            self.account_of(id)
        );
        let punishment = self.default_punish();
        self.handle_illegal_player_action(id, &message, punishment);
    }

    /// `PlayerConfig.MAX_ITEM_IN_PACKET`: the largest of the three inventory limits.
    pub(crate) fn max_item_in_packet(&self) -> usize {
        let s = &self.settings;
        let max = s
            .inventory_maximum_no_dwarf
            .max(s.inventory_maximum_dwarf.max(s.inventory_maximum_gm));
        usize::try_from(max).unwrap_or(0)
    }

    /// Java `RequestBuyItem.runImpl`.
    pub(crate) fn on_request_buy_item(&mut self, id: ObjectId, request: RequestBuyItem) {
        if !self.players.contains_key(&id) {
            return;
        }
        // FOLLOW-UP: the flood protector `canPerformTransaction` ("You are buying too fast.").

        let list_id = request.list_id;
        // `readImpl` left the list null: a bad count, a bad length or a bad entry.
        // (The decoder already drops an entry below 1; the world input can come from elsewhere.)
        let items: Option<Vec<BuyItemEntry>> = request.items.filter(|v| {
            v.len() <= self.max_item_in_packet() && v.iter().all(|e| e.item_id >= 1 && e.count >= 1)
        });
        let Some(items) = items else {
            self.send(id, &ActionFailed);
            return;
        };

        // Alt game - Karma punishment.
        if !self.settings.talk.karma_player_can_shop
            && self.players.get(&id).is_some_and(|p| p.data.karma > 0)
        {
            self.send(id, &ActionFailed);
            return;
        }

        let Ok(merchant) = self.shop_merchant(id, list_id) else {
            return;
        };

        let Some(list) = self.shop.lists.as_ref().and_then(|l| l.get(list_id)).cloned() else {
            self.shop_illegal_action(id, &format!("sent a false BuyList list_id {list_id}"));
            return;
        };

        // The tax comes from the merchant the player is talking to; a GM without one pays none.
        let mut base_tax_rate = 0.0;
        let mut castle_tax_rate = 0.0_f64;
        if let Some(npc) = merchant {
            let npc_id = self.npcs.get(&npc).map_or(0, |n| n.npc_id);
            if !list.is_npc_allowed(npc_id) {
                self.send(id, &ActionFailed);
                return;
            }
            castle_tax_rate = self.merchant_castle_tax_rate(npc);
            base_tax_rate = self.merchant_base_tax_rate(npc);
        }

        let Some(templates) = self.item_templates() else {
            self.send(id, &ActionFailed);
            return;
        };
        let max_adena = self.settings.ground.max_adena;
        let is_gm = self.is_gm(id);

        let mut sub_total: i64 = 0;
        // Check for buylist validity and calculates summary values.
        let mut slots: i64 = 0;
        let mut weight: i64 = 0;
        for entry in &items {
            let Some(product) = list.product(entry.item_id) else {
                self.shop_illegal_action(
                    id,
                    &format!(
                        "sent a false BuyList list_id {list_id} and item_id {}",
                        entry.item_id
                    ),
                );
                return;
            };
            let Some(template) = templates.get(product.item_id) else {
                self.send(id, &ActionFailed);
                return;
            };

            if !template.stackable && entry.count > 1 {
                self.shop_illegal_action(
                    id,
                    "tried to purchase invalid quantity of items at the same time.",
                );
                self.send(
                    id,
                    &SystemMessage::new(SystemMessageId::YouHaveExceededTheQuantityThatCanBeInputted),
                );
                return;
            }

            let mut price = product.price_or(template.price);
            if is_siege_guard_item(product.item_id) {
                // `price *= RATE_SIEGE_GUARDS_PRICE`: a float product, cut back to a long.
                price = ((price as f32) * self.settings.shop.rate_siege_guards_price) as i64;
            }

            if price < 0 {
                tracing::warn!("ERROR, no price found .. wrong buylist ??");
                self.send(id, &ActionFailed);
                return;
            }

            if price == 0 && !is_gm && self.settings.shop.only_gm_items_free {
                self.send_message(id, "Ohh Cheat does not work? You have a problem now!");
                self.shop_illegal_action(id, "tried buy item for 0 adena.");
                return;
            }

            // Trying to buy more then available.
            if product.has_limited_stock() && entry.count > self.product_count(list_id, product.item_id) {
                self.send(id, &ActionFailed);
                return;
            }

            if (max_adena / entry.count) < price {
                self.shop_illegal_action(
                    id,
                    &format!("tried to purchase over {max_adena} adena worth of goods."),
                );
                return;
            }

            // First calculate price per item with tax, then multiply by count.
            price = (price as f64 * (1.0 + castle_tax_rate + base_tax_rate)) as i64;
            sub_total = sub_total.wrapping_add(entry.count.wrapping_mul(price));
            if sub_total > max_adena {
                self.shop_illegal_action(
                    id,
                    &format!("tried to purchase over {max_adena} adena worth of goods."),
                );
                return;
            }

            weight = weight.wrapping_add(entry.count.wrapping_mul(i64::from(template.weight)));
            if self.inventory_item_by_item_id(id, product.item_id).is_none() {
                slots += 1;
            }
        }

        if !is_gm && (weight > i64::from(i32::MAX) || weight < 0 || !self.validate_weight(id, weight)) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouHaveExceededTheWeightLimit),
            );
            self.send(id, &ActionFailed);
            return;
        }

        if !is_gm && (slots > i64::from(i32::MAX) || slots < 0 || !self.validate_capacity(id, slots, false)) {
            self.send(id, &SystemMessage::new(SystemMessageId::YourInventoryIsFull));
            self.send(id, &ActionFailed);
            return;
        }

        // Charge buyer and add tax to castle treasury if not owned by npc clan.
        if sub_total < 0 || !self.reduce_adena(id, sub_total, false) {
            self.send(id, &SystemMessage::new(SystemMessageId::YouDoNotHaveEnoughAdena));
            self.send(id, &ActionFailed);
            return;
        }

        // Proceed the purchase
        for entry in &items {
            let Some(product) = list.product(entry.item_id) else {
                self.shop_illegal_action(
                    id,
                    &format!(
                        "sent a false BuyList list_id {list_id} and item_id {}",
                        entry.item_id
                    ),
                );
                continue;
            };
            if product.has_limited_stock() {
                // NOTE(java-quirk): when the stock ran out between the check and here (the same
                // item twice in one request) the adena is gone and the item is not given.
                if self.decrease_product_count(list_id, product.item_id, entry.count) {
                    self.inventory_add_item_by_id(id, entry.item_id, entry.count);
                }
            } else {
                self.inventory_add_item_by_id(id, entry.item_id, entry.count);
            }
        }

        // Add to castle treasury: `((Merchant) merchant).getCastle()` (the castle of the merchant's
        // town, M7-RES-01), which may be another one than the price config's.
        if let Some(castle) = merchant.and_then(|npc| self.npc_castle(npc)) {
            self.castle_add_to_treasury(castle, (sub_total as f64 * castle_tax_rate) as i64);
        }

        self.send_cur_load(id);
        self.send_ex_buy_sell_list(id, true);
        self.send(id, &SystemMessage::new(SystemMessageId::TheTransactionIsComplete));
    }
}
