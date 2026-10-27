//! `RequestPreviewItem`: the wear shop. The player pays `WearPrice` per item, sees the items on
//! its character (`ShopPreviewInfo`) and after `WearDelay` seconds hears the try-on is over.
// Java: network/clientpackets/RequestPreviewItem.java
// Java: network/serverpackets/ShopPreviewInfo.java

use l2_data::item::types::{ArmorType, WeaponType};
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::client::RequestPreviewItem;
use l2_protocol_game::server::{ActionFailed, ShopPreviewInfo, SystemMessage};
use l2_util::ObjectId;
use tracing::warn;

use crate::dialog::INTERACTION_DISTANCE;
use crate::flood::FloodKind;
use crate::inventory::paperdoll_slot_of;
use crate::leftover::LeftoverTimer;
use crate::target::distance_2d;
use crate::timer::{TimerKind, TimerWheel};
use crate::world::World;

/// Java `_listId >= 4000000`: the lists of the custom shops.
const MAX_LIST_ID: i32 = 4_000_000;
/// `player.getRace().ordinal() == 5`.
const RACE_KAMAEL: i32 = 5;

impl World {
    /// Java `RequestPreviewItem.runImpl`.
    pub(crate) fn on_request_preview_item(&mut self, id: ObjectId, request: RequestPreviewItem) {
        // `_items == null`: the list was longer than 100.
        let Some(items) = request.items else {
            return;
        };
        if !self.players.contains_key(&id) {
            return;
        }
        if !self.flood_allows(id, FloodKind::Transaction) {
            self.send_message(id, "You are previewing items too fast.");
            self.send(id, &ActionFailed);
            return;
        }
        if !self.allows_transaction(id) {
            self.send_message(id, "Transactions are disabled for your access level.");
            self.send(id, &ActionFailed);
            return;
        }
        // If Alternate rule Karma punishment is set to true, forbid Wear to player with Karma.
        if !self.settings.talk.karma_player_can_shop
            && self.players.get(&id).is_some_and(|p| p.data.karma > 0)
        {
            return;
        }

        // Check current target of the player and the INTERACTION_DISTANCE.
        let target = self.target_of(id);
        if !self.is_gm(id) {
            // No target (i.e. GM Shop), no merchant, or too far.
            let near_merchant = target.is_some_and(|t| {
                self.npc_is_merchant(t)
                    && matches!(
                        (self.position_of(id), self.position_of(t)),
                        (Some(a), Some(b)) if distance_2d(a, b) < f64::from(INTERACTION_DISTANCE)
                    )
            });
            if !near_merchant {
                return;
            }
        }

        let list_id = request.list_id;
        if items.is_empty() || list_id >= MAX_LIST_ID {
            self.send(id, &ActionFailed);
            return;
        }

        // Get the current merchant targeted by the player.
        if !target.is_some_and(|t| self.npc_is_merchant(t)) {
            warn!("RequestPreviewItem: Null merchant!");
            return;
        }

        let Some(list) = self.shop.lists.as_ref().and_then(|l| l.get(list_id)).cloned() else {
            let message = format!(
                "Warning!! Character {} of account {} sent a false BuyList list_id {list_id}",
                self.players.get(&id).map_or("?", |p| p.data.name.as_str()),
                self.account_of(id),
            );
            self.leftover_illegal_action(id, &message);
            return;
        };

        let templates = self.item_templates();
        let templates = templates.as_deref();
        let race = self.players.get(&id).map_or(0, |p| p.data.race);
        let max_adena = self.settings.ground.max_adena;
        let wear_price = i64::from(self.settings.leftover.wear_price);
        let mut total_price: i64 = 0;
        // Java `Map<Integer, Integer>`: slot to item id.
        let mut item_list: Vec<(usize, i32)> = Vec::new();
        for item_id in items {
            let Some(product) = list.product(item_id) else {
                let message = format!(
                    "Warning!! Character {} of account {} sent a false BuyList list_id {list_id} and item_id {item_id}",
                    self.players.get(&id).map_or("?", |p| p.data.name.as_str()),
                    self.account_of(id),
                );
                self.leftover_illegal_action(id, &message);
                return;
            };
            let Some(template) = templates.and_then(|t| t.get(product.item_id)) else {
                continue;
            };
            // `Inventory.getPaperdollIndex(template.getBodyPart())`: -1 for the parts that have no
            // slot of their own.
            let Some(slot) = paperdoll_slot_of(template.body_part) else {
                continue;
            };

            if template.is_weapon() {
                if race == RACE_KAMAEL {
                    let kind = template.weapon().map(|w| w.weapon_type);
                    if kind == Some(WeaponType::None) {
                        continue;
                    }
                    if matches!(
                        kind,
                        Some(WeaponType::Rapier | WeaponType::CrossBow | WeaponType::AncientSword)
                    ) {
                        continue;
                    }
                }
            } else if template.is_armor() {
                let kind = template.armor().map(|a| a.armor_type);
                if race == RACE_KAMAEL && matches!(kind, Some(ArmorType::Heavy | ArmorType::Magic)) {
                    continue;
                }
            }

            if item_list.iter().any(|(s, _)| *s == slot) {
                self.send(
                    id,
                    &SystemMessage::new(SystemMessageId::YouCanNotTryThoseItemsOnAtTheSameTime),
                );
                return;
            }

            item_list.push((slot, product.item_id));
            total_price += wear_price;
            if total_price > max_adena {
                let message = format!(
                    "Warning!! Character {} of account {} tried to purchase over {max_adena} adena worth of goods.",
                    self.players.get(&id).map_or("?", |p| p.data.name.as_str()),
                    self.account_of(id),
                );
                self.leftover_illegal_action(id, &message);
                return;
            }
        }

        // Charge buyer and add tax to castle treasury if not owned by npc clan because a Try On is
        // not Free.
        if total_price < 0 || !self.reduce_adena(id, total_price, true) {
            self.send(id, &SystemMessage::new(SystemMessageId::YouDoNotHaveEnoughAdena));
            return;
        }

        if !item_list.is_empty() {
            self.send(id, &ShopPreviewInfo::from_slots(&item_list));
            // Schedule task.
            let delay_ms = u64::try_from(self.settings.leftover.wear_delay)
                .unwrap_or(0)
                .saturating_mul(1000);
            let due = self.tick + TimerWheel::ms_to_ticks(delay_ms);
            self.timers
                .schedule_at(due, TimerKind::Leftover(LeftoverTimer::WearEnd(id)));
        }
    }

    /// Java `RemoveWearItemsTask.run`: the try-on is over, the client shows the real equipment.
    pub(crate) fn on_wear_end(&mut self, id: ObjectId) {
        if !self.players.contains_key(&id) {
            return;
        }
        self.send(
            id,
            &SystemMessage::new(SystemMessageId::YouAreNoLongerTryingOnEquipment2),
        );
        self.send_user_info(id);
    }
}
