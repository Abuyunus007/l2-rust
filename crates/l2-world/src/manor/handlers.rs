//! The manor's client packets: the manor list, the lord's seed and crop settings, the seed
//! purchase and the crop sale.
// Java: network/clientpackets/{RequestManorList,RequestSetSeed,RequestSetCrop,RequestBuySeed,
//       RequestProcureCropList}.java

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::client::{RequestBuySeed, RequestProcureCropList, RequestSetCrop, RequestSetSeed};
use l2_protocol_game::server::{ActionFailed, SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use tracing::debug;

use super::{CropProcure, SeedProduction};
use crate::clan::model::ClanAccess;
use crate::flood::FloodKind;
use crate::inventory::item_name_param;
use crate::residence::{CastleId, Writes};
use crate::world::World;

/// The manor packets as the world gets them (`ClientInput::Manor`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManorInput {
    /// Java `RequestManorList`.
    ManorList,
    /// Java `RequestSetSeed`.
    SetSeed(RequestSetSeed),
    /// Java `RequestSetCrop`.
    SetCrop(RequestSetCrop),
    /// Java `RequestBuySeed`.
    BuySeed(RequestBuySeed),
    /// Java `RequestProcureCropList`.
    ProcureCropList(RequestProcureCropList),
}

impl World {
    /// Dispatches the manor packets of `ClientInput::Manor`.
    pub(crate) fn on_manor_input(&mut self, id: ObjectId, input: ManorInput) {
        if !self.players.contains_key(&id) {
            return;
        }
        match input {
            ManorInput::ManorList => self.on_request_manor_list(id),
            ManorInput::SetSeed(request) => self.on_request_set_seed(id, request),
            ManorInput::SetCrop(request) => self.on_request_set_crop(id, request),
            ManorInput::BuySeed(request) => self.on_request_buy_seed(id, request),
            ManorInput::ProcureCropList(request) => self.on_request_procure_crop_list(id, request),
        }
    }

    /// Java `RequestManorList.runImpl`: `ExSendManorList`.
    fn on_request_manor_list(&mut self, id: ObjectId) {
        let packet = self.manor_list_packet();
        self.send(id, &packet);
    }

    /// Java `NpcTemplate.getParameters().getInt("manor_id", -1)` of an NPC.
    pub(crate) fn npc_manor_id(&self, npc: ObjectId) -> i32 {
        self.npcs
            .get(&npc)
            .and_then(|n| self.npc_world.template(n.npc_id))
            .and_then(|t| t.param_int("manor_id"))
            .unwrap_or(-1)
    }

    /// The privilege check the two setting packets share: a clan with this castle, the
    /// `CASTLE_MANOR` right, and the NPC the player talked to within reach. `ActionFailed` went
    /// out for `Err(false)`; `Err(true)` is where Java throws (no NPC talked to): nothing is sent.
    fn manor_lord_check(&mut self, id: ObjectId, manor_id: i32) -> Result<(), ()> {
        let castle_of_clan = self.clan_of(id).map(|c| c.castle_id);
        let privileged =
            castle_of_clan == Some(manor_id) && self.has_clan_access(id, ClanAccess::CastleManor);
        if !privileged {
            self.send(id, &ActionFailed);
            return Err(());
        }
        // `!player.getLastFolkNPC().canInteract(player)`: a null NPC throws.
        let Some(npc) = self.players.get(&id).and_then(|p| p.talk.last_folk_npc) else {
            debug!("{id}: manor settings without an NPC talked to");
            return Err(());
        };
        if !self.can_interact(npc, id) {
            self.send(id, &ActionFailed);
            return Err(());
        }
        Ok(())
    }

    /// Java `RequestSetSeed.runImpl`.
    fn on_request_set_seed(&mut self, id: ObjectId, request: RequestSetSeed) {
        // A list over `MAX_ITEM_IN_PACKET` left `_items` null in `readImpl` (Java throws).
        if request.items.is_empty()
            || request.items.len() > self.max_item_in_packet()
            || request
                .items
                .iter()
                .any(|s| s.item_id < 1 || s.sales < 0 || s.price < 0)
        {
            return;
        }
        if !self.manor_is_modifiable() {
            self.send(id, &ActionFailed);
            return;
        }
        // Check player privileges.
        if self.manor_lord_check(id, request.manor_id).is_err() {
            return;
        }
        let rate = self.manor.settings.rate_drop_manor;
        // Filter seeds with start amount lower than 0 and incorrect price.
        let list: Vec<SeedProduction> = request
            .items
            .iter()
            .filter(|sale| {
                self.manor.seeds.get(sale.item_id).is_some_and(|s| {
                    sale.sales <= i64::from(s.seed_limit(rate))
                        && sale.price >= i64::from(s.seed_min_price())
                        && sale.price <= i64::from(s.seed_max_price())
                })
            })
            .map(|sale| SeedProduction {
                id: sale.item_id,
                amount: sale.sales,
                price: sale.price,
                start_amount: sale.sales,
            })
            .collect();
        // Save new list.
        self.manor_set_next_seed_production(list, request.manor_id);
    }

    /// Java `RequestSetCrop.runImpl`.
    fn on_request_set_crop(&mut self, id: ObjectId, request: RequestSetCrop) {
        if request.items.is_empty()
            || request.items.len() > self.max_item_in_packet()
            || request
                .items
                .iter()
                .any(|s| s.item_id < 1 || s.sales < 0 || s.price < 0)
        {
            return;
        }
        if !self.manor_is_modifiable() {
            self.send(id, &ActionFailed);
            return;
        }
        // Check player privileges.
        if self.manor_lord_check(id, request.manor_id).is_err() {
            return;
        }
        let rate = self.manor.settings.rate_drop_manor;
        // Filter crops with start amount lower than 0 and incorrect price.
        let list: Vec<CropProcure> = request
            .items
            .iter()
            .filter(|sale| {
                self.manor
                    .seeds
                    .by_crop_in_castle(sale.item_id, request.manor_id)
                    .is_some_and(|s| {
                        sale.sales <= i64::from(s.crop_limit(rate))
                            && sale.price >= i64::from(s.crop_min_price())
                            && sale.price <= i64::from(s.crop_max_price())
                    })
            })
            .map(|sale| CropProcure {
                id: sale.item_id,
                amount: sale.sales,
                reward: i32::from(sale.reward_type),
                start_amount: sale.sales,
                price: sale.price,
            })
            .collect();
        // Save crop list.
        self.manor_set_next_crop_procure(list, request.manor_id);
    }

    /// `PunishmentManager.handleIllegalPlayerAction(player, "Warning!! Character <name> of account
    /// <account> <what>", DEFAULT_PUNISH)`.
    fn manor_illegal_action(&mut self, id: ObjectId, what: &str) {
        let name = self.players.get(&id).map_or("?", |p| p.data.name.as_str());
        let message = format!(
            "Warning!! Character {name} of account {} {what}",
            self.account_of(id)
        );
        let punishment = self.default_punish();
        self.handle_illegal_player_action(id, &message, punishment);
    }

    /// Java `RequestBuySeed.runImpl`.
    fn on_request_buy_seed(&mut self, id: ObjectId, request: RequestBuySeed) {
        if !self.flood_allows(id, FloodKind::Transaction) {
            self.send_message(id, "You are buying seeds too fast!");
            return;
        }
        // `readImpl` left the list null: a bad count, a bad length or a bad entry.
        let items = request.items.filter(|items| {
            !items.is_empty()
                && items.len() <= self.max_item_in_packet()
                && items.iter().all(|i| i.count >= 1 && i.item_id >= 1)
        });
        let Some(items) = items else {
            self.send(id, &ActionFailed);
            return;
        };
        if self.manor_is_under_maintenance() {
            self.send(id, &ActionFailed);
            return;
        }
        let manor_id = request.manor_id;
        if self.castle(CastleId(manor_id)).is_none() {
            self.send(id, &ActionFailed);
            return;
        }
        // `!(manager instanceof Merchant) || !manager.canInteract(player) || manor_id != _manorId`
        let manager = self.players.get(&id).and_then(|p| p.talk.last_folk_npc);
        if !manager.is_some_and(|m| {
            self.npc_is_merchant(m) && self.can_interact(m, id) && self.npc_manor_id(m) == manor_id
        }) {
            self.send(id, &ActionFailed);
            return;
        }
        let Some(templates) = self.item_templates() else {
            return;
        };
        // Java throws for the production of a castle the lists do not hold.
        if self.manor_seed_production(manor_id, false).is_none() {
            debug!("{id}: RequestBuySeed for manor {manor_id} without lists");
            return;
        }
        let max_adena = self.settings.ground.max_adena;

        let mut total_price: i64 = 0;
        let mut slots: i32 = 0;
        let mut total_weight: i32 = 0;
        for item in &items {
            let found = self.manor_seed_product(manor_id, item.item_id, false);
            let Some(sp) = found
                .filter(|sp| sp.price > 0 && sp.amount >= item.count && (max_adena / item.count) >= sp.price)
            else {
                self.send(id, &ActionFailed);
                return;
            };
            let price = sp.price;

            // Calculate price
            total_price = total_price.wrapping_add(price.wrapping_mul(item.count));
            if total_price > max_adena {
                self.manor_illegal_action(
                    id,
                    &format!("tried to purchase over {max_adena} adena worth of goods."),
                );
                self.send(id, &ActionFailed);
                return;
            }

            // Calculate weight
            let Some(template) = templates.get(item.item_id) else {
                // Java throws for an item without a template.
                debug!("{id}: RequestBuySeed for the unknown item {}", item.item_id);
                return;
            };
            // NOTE(java-quirk): `int += long` narrows the sum.
            total_weight = (i64::from(total_weight)
                .wrapping_add(item.count.wrapping_mul(i64::from(template.weight))))
                as i32;

            // Calculate slots
            if !template.stackable {
                slots = (i64::from(slots).wrapping_add(item.count)) as i32;
            } else if self.inventory_item_by_item_id(id, item.item_id).is_none() {
                slots = slots.wrapping_add(1);
            }
        }

        if !self.validate_weight(id, i64::from(total_weight)) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouHaveExceededTheWeightLimit),
            );
            return;
        } else if !self.validate_capacity(id, i64::from(slots), false) {
            self.send(id, &SystemMessage::new(SystemMessageId::YourInventoryIsFull));
            return;
        } else if total_price < 0 || self.adena_of(id) < total_price {
            self.send(id, &SystemMessage::new(SystemMessageId::YouDoNotHaveEnoughAdena));
            return;
        }

        // Proceed the purchase
        for item in &items {
            let Some(price) = self
                .manor_seed_product(manor_id, item.item_id, false)
                .map(|sp| sp.price.wrapping_mul(item.count))
            else {
                continue;
            };
            // Take Adena and decrease seed amount.
            // NOTE(java-quirk): the amount is decreased before the adena is taken; when the adena
            // is short the seeds stay gone.
            let decreased = self
                .manor_seed_product_mut(manor_id, item.item_id, false)
                .is_some_and(|sp| sp.decrease_amount(item.count));
            if !decreased || !self.reduce_adena(id, price, false) {
                // Failed buy, reduce total price.
                total_price -= price;
                continue;
            }
            // Add item to player's inventory.
            self.add_item(id, item.item_id, item.count, true);
        }

        // Adding to treasury for Manor Castle.
        if total_price > 0 {
            // The treasury is the castle's own row: it follows the tax income (D-51).
            self.castle_add_to_treasury_no_tax(CastleId(manor_id), total_price, Writes::Job);
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::S1AdenaDisappeared)
                    .with(SystemMessageParam::LongNumber(total_price)),
            );
            if self.manor.settings.save_all_actions {
                // `productInfo.values()`: one entry per seed id.
                let mut ids: Vec<i32> = items.iter().map(|i| i.item_id).collect();
                ids.sort_unstable();
                ids.dedup();
                let amounts: Vec<(i32, i64)> = ids
                    .into_iter()
                    .filter_map(|seed| Some((seed, self.manor_seed_product(manor_id, seed, false)?.amount)))
                    .collect();
                self.manor_update_current_production(manor_id, amounts);
            }
        }
    }

    /// Java `RequestProcureCropList.runImpl`.
    fn on_request_procure_crop_list(&mut self, id: ObjectId, request: RequestProcureCropList) {
        let items = request.items.filter(|items| {
            !items.is_empty()
                && items.len() <= self.max_item_in_packet()
                && items
                    .iter()
                    .all(|i| i.object_id >= 1 && i.item_id >= 1 && i.manor_id >= 0 && i.count >= 0)
        });
        let Some(items) = items else {
            return;
        };
        if self.manor_is_under_maintenance() {
            self.send(id, &ActionFailed);
            return;
        }
        let manager = self.players.get(&id).and_then(|p| p.talk.last_folk_npc);
        let Some(manager) = manager.filter(|m| self.npc_is_merchant(*m) && self.can_interact(*m, id)) else {
            self.send(id, &ActionFailed);
            return;
        };
        // `manager.getCastle().getResidenceId()`
        let Some(castle_id) = self.npc_castle(manager).map(|c| c.0) else {
            debug!("{id}: RequestProcureCropList at an NPC without a castle");
            return;
        };
        if self.npc_manor_id(manager) != castle_id {
            self.send(id, &ActionFailed);
            return;
        }
        let Some(templates) = self.item_templates() else {
            return;
        };

        let mut slots: i32 = 0;
        let mut weight: i32 = 0;
        for crop in &items {
            let item = self.inventory_item(id, ObjectId(crop.object_id)).cloned();
            let Some(item) = item.filter(|i| i.count >= crop.count && i.item_id == crop.item_id) else {
                self.send(id, &ActionFailed);
                return;
            };
            let _ = item;
            let procure = self.manor_crop_procure_of(crop.manor_id, crop.item_id, false);
            let Some(procure) = procure.filter(|cp| cp.amount >= crop.count) else {
                self.send(id, &ActionFailed);
                return;
            };
            let reward = procure.reward;
            // `getSeedByCrop(_cp.getId()).getReward(_cp.getReward())`
            let Some(reward_id) = self.manor.seeds.by_crop(crop.item_id).map(|s| s.reward(reward)) else {
                debug!("{id}: a crop without a seed: {}", crop.item_id);
                return;
            };
            let Some(template) = templates.get(reward_id) else {
                debug!("{id}: a reward without a template: {reward_id}");
                return;
            };
            // NOTE(java-quirk): `int += long` narrows the sum.
            weight =
                (i64::from(weight).wrapping_add(crop.count.wrapping_mul(i64::from(template.weight)))) as i32;
            if !template.stackable {
                slots = (i64::from(slots).wrapping_add(crop.count)) as i32;
            } else if self.inventory_item_by_item_id(id, reward_id).is_none() {
                slots = slots.wrapping_add(1);
            }
        }

        if !self.validate_weight(id, i64::from(weight)) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouHaveExceededTheWeightLimit),
            );
            return;
        } else if !self.validate_capacity(id, i64::from(slots), false) {
            self.send(id, &SystemMessage::new(SystemMessageId::YourInventoryIsFull));
            return;
        }

        // Used when Config.ALT_MANOR_SAVE_ALL_ACTIONS == true.
        let mut update_list: Vec<(i32, i32)> = Vec::new();

        // Proceed the purchase
        for crop in &items {
            // `i.getRewardId()` and `i.getPrice()` read the procure the validation found.
            let Some(procure) = self.manor_crop_procure_of(crop.manor_id, crop.item_id, false) else {
                continue;
            };
            let (reward, unit_price) = (procure.reward, procure.price);
            let Some(reward_id) = self.manor.seeds.by_crop(crop.item_id).map(|s| s.reward(reward)) else {
                continue;
            };
            let Some(reward_template) = templates.get(reward_id) else {
                continue;
            };
            let reward_price = i64::from(reward_template.price);
            if reward_price == 0 {
                continue;
            }
            let price = crop.count.wrapping_mul(unit_price);
            let reward_item_count = price / reward_price;
            if reward_item_count < 1 {
                let name = item_name_param(Some(&templates), crop.item_id);
                self.send(
                    id,
                    &SystemMessage::new(SystemMessageId::FailedInTradingS2OfS1Crops)
                        .with(name)
                        .with(SystemMessageParam::LongNumber(crop.count)),
                );
                continue;
            }

            // Fee for selling to other manors.
            let fee: i64 = if castle_id == crop.manor_id {
                0
            } else {
                (price as f64 * 0.05) as i64
            };
            if fee != 0 && self.adena_of(id) < fee {
                let name = item_name_param(Some(&templates), crop.item_id);
                self.send(
                    id,
                    &SystemMessage::new(SystemMessageId::FailedInTradingS2OfS1Crops)
                        .with(name)
                        .with(SystemMessageParam::LongNumber(crop.count)),
                );
                self.send(id, &SystemMessage::new(SystemMessageId::YouDoNotHaveEnoughAdena));
                continue;
            }

            // NOTE(java-quirk): the crop's amount is taken first; when the fee or the item cannot
            // be taken the amount stays gone.
            let decreased = self
                .manor_crop_procure_mut(crop.manor_id, crop.item_id, false)
                .is_some_and(|cp| cp.decrease_amount(crop.count));
            if !decreased
                || (fee > 0 && !self.reduce_adena(id, fee, true))
                || !self.destroy_item_by_object_id(id, ObjectId(crop.object_id), crop.count, true)
            {
                continue;
            }
            self.add_item(id, reward_id, reward_item_count, true);
            if self.manor.settings.save_all_actions {
                update_list.push((crop.manor_id, crop.item_id));
            }
        }

        if self.manor.settings.save_all_actions {
            // `updateCurrentProcure(castleId, updateList)`: the rows are those of the manager's
            // castle, with the amounts of the crops sold (also another castle's).
            let amounts: Vec<(i32, i64)> = update_list
                .into_iter()
                .filter_map(|(manor, crop)| {
                    Some((crop, self.manor_crop_procure_of(manor, crop, false)?.amount))
                })
                .collect();
            self.manor_update_current_procure(castle_id, amounts);
        }
    }
}
