//! Inside the trade window: `AddTradeItem`, `TradeDone` and the lock/confirm logic of
//! `TradeList`.
// Java: network/clientpackets/{AddTradeItem,TradeDone}.java
// Java: network/holders/TradeList.java (addItem, confirm, validate, invalidateConfirmation)

use l2_data::item::ItemTemplate;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, TradeOtherAdd, TradeOwnAdd, TradeUpdate};
use l2_util::ObjectId;
use tracing::warn;

use super::{TRADE_RANGE, TradeItem, TradeList};
use crate::player::ItemData;
use crate::world::World;

/// What `TradeList.addItem` reads from the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AddItemRules {
    /// `item.isTradeable() || (owner.isGM() && GMTradeRestrictedItems)`.
    pub tradeable: bool,
    /// `MAX_ADENA` (`Inventory.MAX_ADENA`).
    pub max_adena: i64,
}

impl TradeList {
    /// Java `TradeList.addItem(objectId, count, price)` for an item the owner has: the entry is
    /// added (and the owner's confirmation withdrawn), or `None` when Java refuses it.
    pub fn add_item(
        &mut self,
        item: &ItemData,
        template: &ItemTemplate,
        count: i64,
        price: i64,
        rules: AddItemRules,
    ) -> Option<TradeItem> {
        if self.locked {
            return None;
        }
        if !rules.tradeable || template.quest_item {
            return None;
        }
        // `canManipulateWithItemId` is true: no inventory block is modelled.
        if count <= 0 || count > item.count {
            return None;
        }
        if !template.stackable && count > 1 {
            return None;
        }
        if (rules.max_adena / count) < price {
            return None;
        }
        if self.items.iter().any(|i| i.object_id == item.object_id) {
            return None;
        }
        let entry = TradeItem {
            object_id: item.object_id,
            item: item.clone(),
            count,
            price,
        };
        self.items.push(entry.clone());
        // If Player has already confirmed this trade, invalidate the confirmation.
        self.invalidate_confirmation();
        Some(entry)
    }
}

impl World {
    /// `World.getPlayer(partner.getObjectId()) == null`.
    fn is_online(&self, id: ObjectId) -> bool {
        self.players.contains_key(&id)
    }

    /// The trade list of a player and whether it is confirmed.
    fn trade_confirmed(&self, id: ObjectId) -> Option<bool> {
        self.players
            .get(&id)
            .and_then(|p| p.trade.active.as_ref())
            .map(|l| l.confirmed)
    }

    // ------------------------------------------------------------------ AddTradeItem

    /// Java `AddTradeItem.runImpl`.
    pub(crate) fn on_add_trade_item(&mut self, id: ObjectId, trade_id: i32, object_id: i32, count: i64) {
        if !self.is_online(id) || count < 1 {
            return;
        }
        let Some(partner) = self
            .players
            .get(&id)
            .and_then(|p| p.trade.active.as_ref())
            .map(|l| l.partner)
        else {
            self.warn_no_trade(
                id,
                &format!("requested item:{object_id} add without active tradelist:{trade_id}"),
            );
            return;
        };

        let partner_ok = partner
            .filter(|p| self.is_online(*p))
            .filter(|p| self.players.get(p).is_some_and(|q| q.trade.active.is_some()));
        let Some(partner) = partner_ok else {
            // Trade partner not found, cancel trade.
            if partner.is_some() {
                self.warn_no_trade(id, &format!("requested invalid trade object: {object_id}"));
            }
            self.send(id, &SystemMessage::new(SystemMessageId::ThatPlayerIsNotOnline));
            self.cancel_active_trade(id);
            return;
        };

        if self.trade_confirmed(id) == Some(true) || self.trade_confirmed(partner) == Some(true) {
            self.send(
                id,
                &SystemMessage::new(
                    SystemMessageId::YouMayNoLongerAdjustItemsInTheTradeBecauseTheTradeHasBeenConfirmed,
                ),
            );
            return;
        }

        if !self.allows_transaction(id) {
            self.send_message(id, "Transactions are disabled for your Access Level.");
            self.cancel_active_trade(id);
            return;
        }

        let object_id = ObjectId(object_id);
        if !self.validate_item_manipulation(id, object_id) {
            self.send(id, &SystemMessage::new(SystemMessageId::NothingHappened));
            return;
        }

        let Some(item) = self.trade_list_add_item(id, object_id, count, 0) else {
            return;
        };
        // `new TradeUpdate(player, item)`: what is left in the inventory decides the change.
        let left = self
            .inventory_item(id, object_id)
            .map_or(0, |i| i.count)
            .wrapping_sub(item.count);
        let stackable = self.data.item_stackable(item.item.item_id);
        let info = self.trade_item_info(&item);
        self.send(id, &TradeOwnAdd { item: info.clone() });
        self.send(
            id,
            &TradeUpdate {
                change: TradeUpdate::change_for(left, stackable),
                item: info.clone(),
            },
        );
        self.send(partner, &TradeOtherAdd { item: info });
    }

    /// Java `TradeList.addItem(objectId, count, price)` on the active list of `owner`.
    pub(crate) fn trade_list_add_item(
        &mut self,
        owner: ObjectId,
        object_id: ObjectId,
        count: i64,
        price: i64,
    ) -> Option<TradeItem> {
        let item = self.inventory_item(owner, object_id)?.clone();
        let templates = self.item_templates()?;
        let template = templates.get(item.item_id)?;
        let tradeable = self.item_is_tradeable(&item, template)
            || (self.is_gm(owner) && self.settings.inventory.gm_trade_restricted_items);
        let rules = AddItemRules {
            tradeable,
            max_adena: self.settings.ground.max_adena,
        };
        self.players
            .get_mut(&owner)?
            .trade
            .active
            .as_mut()?
            .add_item(&item, template, count, price, rules)
    }

    // ------------------------------------------------------------------ TradeDone

    /// Java `TradeDone.runImpl` (client packet).
    pub(crate) fn on_trade_done(&mut self, id: ObjectId, response: i32) {
        if !self.is_online(id) {
            return;
        }
        // The flood protector `canPerformTransaction` ("You are trading too fast.") is not ported.
        let Some(trade) = self.players.get(&id).and_then(|p| p.trade.active.as_ref()) else {
            return;
        };
        if trade.locked {
            return;
        }
        let partner = trade.partner;

        if response != 1 {
            self.cancel_active_trade(id);
            return;
        }

        let Some(partner) = partner.filter(|p| self.is_online(*p)) else {
            // Trade partner not found, cancel trade.
            self.cancel_active_trade(id);
            self.send(id, &SystemMessage::new(SystemMessageId::ThatPlayerIsNotOnline));
            return;
        };

        // Either player is using an enchant scroll (`getActiveEnchantItemId() != ID_NONE`).
        if self.active_enchant_item(id).is_some() || self.active_enchant_item(partner).is_some() {
            return;
        }

        if !self.allows_transaction(id) {
            self.cancel_active_trade(id);
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouAreNotAuthorizedToDoThat),
            );
            return;
        }

        let instance = self.objects.get(&id).map_or(0, |e| e.instance_id);
        let partner_instance = self.objects.get(&partner).map_or(0, |e| e.instance_id);
        if instance != partner_instance && instance != -1 {
            self.cancel_active_trade(id);
            return;
        }

        if self.distance_3d(id, partner).is_none_or(|d| d > TRADE_RANGE) {
            self.cancel_active_trade(id);
            return;
        }

        self.trade_confirm(id);
    }

    // ------------------------------------------------------------------ confirm and validate

    /// Java `TradeList.confirm()` on the list of `owner`. The second confirmation runs the
    /// exchange.
    pub(crate) fn trade_confirm(&mut self, owner: ObjectId) -> bool {
        let Some(list) = self.players.get(&owner).and_then(|p| p.trade.active.as_ref()) else {
            return false;
        };
        if list.confirmed {
            return true; // Already confirmed
        }
        let Some(partner) = list.partner else {
            if let Some(l) = self.players.get_mut(&owner).and_then(|p| p.trade.active.as_mut()) {
                l.confirmed = true;
            }
            return true;
        };

        // If Partner has already confirmed this trade, proceed exchange.
        let Some(partner_confirmed) = self.trade_confirmed(partner) else {
            warn!(
                "{}: Trading partner ({}) is invalid in this trade!",
                self.player_name_of(partner),
                self.player_name_of(partner)
            );
            return false;
        };
        if let Some(l) = self.players.get_mut(&owner).and_then(|p| p.trade.active.as_mut()) {
            l.confirmed = true;
        }
        if partner_confirmed {
            if let Some(l) = self
                .players
                .get_mut(&partner)
                .and_then(|p| p.trade.active.as_mut())
            {
                l.lock();
            }
            if let Some(l) = self.players.get_mut(&owner).and_then(|p| p.trade.active.as_mut()) {
                l.lock();
            }
            // NOTE(java-quirk): when the validation fails both lists stay confirmed and locked and
            // nobody is told; the players have to close the window.
            let partner_list = self.players.get(&partner).and_then(|p| p.trade.active.clone());
            let own_list = self.players.get(&owner).and_then(|p| p.trade.active.clone());
            let valid = match (&partner_list, &own_list) {
                (Some(p), Some(o)) => self.validate_trade_list(p) && self.validate_trade_list(o),
                _ => false,
            };
            if !valid {
                return false;
            }
            self.do_exchange(owner, partner);
        } else {
            self.on_trade_confirm(partner, owner);
        }
        true
    }

    fn player_name_of(&self, id: ObjectId) -> String {
        self.players
            .get(&id)
            .map(|p| p.data.name.clone())
            .unwrap_or_default()
    }

    /// Java `TradeList.validate()`: the owner is in the world and still has every item in the
    /// count that is offered.
    pub(crate) fn validate_trade_list(&self, list: &TradeList) -> bool {
        // Check for Owner validity.
        if !self.is_online(list.owner) {
            warn!("Invalid owner of TradeList");
            return false;
        }
        // Check for Item validity.
        for titem in &list.items {
            let item = self.check_item_manipulation(list.owner, titem.object_id, titem.count);
            if item.is_none_or(|i| i.count < 1) {
                warn!("{}: Invalid Item in TradeList", self.player_name_of(list.owner));
                return false;
            }
        }
        true
    }
}
