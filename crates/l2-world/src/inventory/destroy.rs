//! `RequestDestroyItem`, `RequestDropItem` (the inventory side) and `RequestItemList`, with the
//! container operations they use: `destroyItem`, `removeItem`, `dropItem`.
// Java: network/clientpackets/{RequestDestroyItem,RequestDropItem,RequestItemList}.java
// Java: entity/itemcontainer/{ItemContainer,Inventory,PlayerInventory}.java (destroyItem,
//       removeItem, dropItem)
// Java: managers/ItemManager.java (destroyItem)
// Java: entity/actor/Player.java (dropItem, removeItemFromShortcut)
//
// # Database
// A destroyed item is deleted at once (`ItemManager.destroyItem` empties it, `updateDatabase()`
// removes the row). A count that only goes down is *not* written by Java until the next store
// (`changeCount` only marks the item dirty), and neither is it here: the full store compares the
// item list. A dropped part of a stack is written at once (`dropItem` calls `updateDatabase()`),
// a whole item that is dropped is deleted at once.
//
// # Dropping (M4-DROP-01)
// `World::drop_from_inventory` is the inventory half of `Player.dropItem`; the handler of
// `RequestDropItem` checks everything Java checks, takes a worn item off and puts the item on the
// ground (`ground_item.rs`).

use l2_data::item::TYPE2_QUEST;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::visibility::status;
use l2_protocol_game::server::{
    InventoryUpdate, ItemUpdate, StatusUpdate, SystemMessage, SystemMessageParam,
};
use l2_util::ObjectId;
use tracing::warn;

use super::{LOC_PAPERDOLL, item_name_param};
use crate::enter::current_load;
use crate::player::ItemData;
use crate::stats::skills::java_hash_map_order;
use crate::world::World;
use crate::zones::ZoneId;

/// `ItemLocation.VOID`.
const LOC_VOID: &str = "VOID";
/// `Math.abs(_z - player.getZ()) > 50` and `isInsideRadius2D(_x, _y, 0, 150)` of `RequestDropItem`.
const DROP_RANGE: f64 = 150.0;
const DROP_Z_RANGE: i32 = 50;

/// What `InventoryUpdate` does with its entries: a `HashMap` keyed by object id, so a later entry
/// for the same item replaces the earlier one and the packet lists them in the map's order.
pub(crate) fn order_like_hash_map(entries: Vec<ItemUpdate>) -> Vec<ItemUpdate> {
    let mut unique: Vec<ItemUpdate> = Vec::new();
    for entry in entries {
        match unique
            .iter_mut()
            .find(|e| e.item.object_id == entry.item.object_id)
        {
            Some(existing) => *existing = entry,
            None => unique.push(entry),
        }
    }
    let keys: Vec<i32> = unique.iter().map(|e| e.item.object_id).collect();
    java_hash_map_order(&keys)
        .into_iter()
        .filter_map(|key| unique.iter().find(|e| e.item.object_id == key).cloned())
        .collect()
}

impl World {
    /// `player.sendPacket(iu)`: an `InventoryUpdate` that goes out now.
    pub(crate) fn send_inventory_update_now(&self, id: ObjectId, entries: Vec<ItemUpdate>) {
        self.send(
            id,
            &InventoryUpdate {
                updates: order_like_hash_map(entries),
            },
        );
    }

    /// Java `Player.removeItemFromShortcut(objectId)`: the first shortcut that points to the item
    /// goes, and the client gets the whole shortcut list again (`shortcuts.rs`).
    fn remove_item_from_shortcut(&mut self, id: ObjectId, object_id: ObjectId) {
        self.delete_shortcut_by_object_id(id, object_id);
    }

    /// Java `PlayerInventory.removeItem(item)`: the shortcut goes, a worn item comes off, the item
    /// leaves the list. The item is returned as it was when it left.
    pub(crate) fn remove_inventory_item(&mut self, id: ObjectId, object_id: ObjectId) -> Option<ItemData> {
        self.inventory_item(id, object_id)?;
        // Removes any reference to the item from Shortcut bar.
        self.remove_item_from_shortcut(id, object_id);
        // Unequip item if equipped.
        let mut scratch = Vec::new();
        for slot in 0..l2_protocol_game::server::charscreen::paperdoll::TOTAL_SLOTS {
            if self.paperdoll_object(id, slot) == Some(object_id) {
                self.set_paperdoll_item(id, slot, None, &mut scratch);
            }
        }
        let has_skills = {
            let templates = self.item_templates();
            let item_id = self.inventory_item(id, object_id)?.item_id;
            templates
                .as_deref()
                .and_then(|t| t.get(item_id))
                .is_some_and(|t| t.has_skills())
        };
        if has_skills {
            // Refresh client skill window.
            self.send_skill_list(id);
        }
        let player = self.players.get_mut(&id)?;
        let index = player.data.items.iter().position(|i| i.object_id == object_id)?;
        Some(player.data.items.remove(index))
    }

    /// Java `PlayerInventory.destroyItem(process, item, count, ...)`. Returns the item: with the
    /// count it has left, or with count 0 when it is gone. `None` when the player has less than
    /// `count` or the item is not there.
    pub(crate) fn destroy_inventory_item(
        &mut self,
        id: ObjectId,
        object_id: ObjectId,
        count: i64,
    ) -> Option<ItemData> {
        let item = self.inventory_item(id, object_id)?.clone();
        // Adjust item quantity.
        if item.count > count {
            let player = self.players.get_mut(&id)?;
            let target = player.data.items.iter_mut().find(|i| i.object_id == object_id)?;
            // `changeCount(-count)`: never below 0.
            target.count = (target.count - count).max(0);
            let updated = target.clone();
            self.refresh_weight(id);
            return Some(updated);
        }
        if item.count < count {
            return None;
        }
        let mut removed = self.remove_inventory_item(id, object_id)?;
        // `ItemManager.destroyItem`: nothing left, nobody's, nowhere.
        // NOTE: Java also deletes the `pets` row of a pet collar here; pets are not modelled.
        removed.count = 0;
        removed.loc = LOC_VOID.to_string();
        removed.loc_data = 0;
        self.remove_item_from_db(id, object_id);
        self.refresh_weight(id);
        Some(removed)
    }

    /// Java `RequestItemList.runImpl`.
    pub(crate) fn on_request_item_list(&mut self, id: ObjectId) {
        // `isInventoryDisabled()` is a trade/enchant flag nothing sets yet.
        self.send_item_list(id, true);
    }

    /// Java `RequestDestroyItem.runImpl`.
    pub(crate) fn on_request_destroy_item(&mut self, id: ObjectId, object_id: i32, count: i64) {
        if count < 1 {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouCannotDestroyItBecauseTheNumberIsIncorrect),
            );
            if count < 0 {
                // `PunishmentManager.handleIllegalPlayerAction`.
                self.illegal_action(
                    id,
                    &format!("tried to destroy item with oid {object_id} but has count < 0!"),
                );
            }
            return;
        }
        // Flood protector `canPerformTransaction`: not modelled. Store mode: M6-STORE-01.
        if self.is_processing_transaction(id) {
            self.send(
                id,
                &SystemMessage::new(
                    SystemMessageId::WhileOperatingAPrivateStoreOrWorkshopYouCannotDiscardDestroyOrTradeAnItem,
                ),
            );
            return;
        }

        let object_id = ObjectId(object_id);
        let discarded = SystemMessage::new(SystemMessageId::ThisItemCannotBeDiscarded);
        let Some(item) = self.inventory_item(id, object_id).cloned() else {
            // If we can't find the requested item, it is actually a cheat.
            self.send(id, &discarded);
            return;
        };
        let templates = self.item_templates();
        let templates = templates.as_deref();
        let Some(template) = templates.and_then(|t| t.get(item.item_id)) else {
            self.send(id, &discarded);
            return;
        };

        // Cannot discard item that the skill is consuming: needs the cast state (M5).

        // Cursed weapons are not modelled.
        let gm = self.is_gm(id);
        // `Item.isDestroyable()`: the template, and for an augmented item `AltAllowAugmentDestroy`.
        let destroyable = template.destroyable
            && (!item.is_augmented() || self.settings.inventory.alt_allow_augment_destroy);
        if !self.settings.inventory.destroy_all_items && (!gm && !destroyable) {
            if template.is_hero_item() {
                self.send(
                    id,
                    &SystemMessage::new(SystemMessageId::HeroWeaponsCannotBeDestroyed),
                );
            } else {
                self.send(id, &discarded);
            }
            return;
        }

        if !template.stackable && count > 1 {
            self.illegal_action(
                id,
                &format!(
                    "tried to destroy a non-stackable item with oid {} but has count > 1!",
                    object_id.0
                ),
            );
            return;
        }

        // `canManipulateWithItemId` is always true here.

        let mut count = count;
        if count > item.count {
            count = item.count;
        }

        // A pet collar of an active pet or mount is refused here; pets are not modelled.

        if item.loc == LOC_PAPERDOLL {
            self.send(
                id,
                &self.disarmed_message(item.enchant_level, item.item_id, templates),
            );
            let unequipped =
                self.unequip_item_in_slot_and_record(id, usize::try_from(item.loc_data).unwrap_or(0));
            let entries: Vec<ItemUpdate> = unequipped
                .iter()
                .filter_map(|o| self.modified_entry(id, *o))
                .collect();
            // Sent inventory update for unequip instantly.
            self.send_inventory_update_now(id, entries);
        }

        let Some(removed) = self.destroy_inventory_item(id, object_id, count) else {
            return;
        };

        let change = if removed.count == 0 {
            InventoryUpdate::REMOVED
        } else {
            InventoryUpdate::MODIFIED
        };
        let entry = self.update_entry(&removed, change);
        // Sent inventory update for destruction instantly.
        self.send_inventory_update_now(id, vec![entry]);

        self.send_cur_load(id);

        let name = item_name_param(templates, removed.item_id);
        let message = if count > 1 {
            SystemMessage::new(SystemMessageId::S2S1HasDisappeared)
                .with(name)
                .with(SystemMessageParam::LongNumber(count))
        } else {
            SystemMessage::new(SystemMessageId::S1HasDisappeared).with(name)
        };
        self.send(id, &message);
    }

    /// `new StatusUpdate(player)` with `CUR_LOAD`.
    pub(crate) fn send_cur_load(&self, id: ObjectId) {
        let Some(player) = self.players.get(&id) else {
            return;
        };
        let mut update = StatusUpdate::new(id.0);
        update.add_attribute(status::CUR_LOAD, current_load(&player.data, &*self.data));
        self.send(id, &update);
    }

    /// Java `AdminData.broadcastMessageToGMs(message)`: `sendMessage` to every GM online.
    fn broadcast_message_to_gms(&self, text: &str) {
        let mut gms: Vec<ObjectId> = self.players.keys().copied().filter(|p| self.is_gm(*p)).collect();
        gms.sort_unstable_by_key(|p| p.0);
        for gm in gms {
            self.send_message(gm, text);
        }
    }

    /// `PunishmentManager.handleIllegalPlayerAction` with `DefaultPunish`: the punishment system
    /// does not exist, the attempt is logged.
    fn illegal_action(&self, id: ObjectId, what: &str) {
        let name = self.players.get(&id).map_or("?", |p| p.data.name.as_str());
        warn!("Illegal action by character {name} ({id}): {what}");
    }

    /// Java `RequestDropItem.runImpl`, up to the point where the item would leave the inventory.
    pub(crate) fn on_request_drop_item(
        &mut self,
        id: ObjectId,
        object_id: i32,
        count: i64,
        x: i32,
        y: i32,
        z: i32,
    ) {
        if self.is_alike_dead(id) {
            return;
        }
        // Flood protect drop to avoid packet lag: the flood protectors are the session's.

        let object_id = ObjectId(object_id);
        let gm = self.is_gm(id);
        let settings = self.settings.inventory.clone();
        let item = self.inventory_item(id, object_id).cloned();
        let templates = self.item_templates();
        let templates = templates.as_deref();
        let template = item
            .as_ref()
            .and_then(|i| templates.and_then(|t| t.get(i.item_id)));

        let discard_refused = SystemMessage::new(SystemMessageId::ThisItemCannotBeDiscarded);
        // `validateItemManipulation` (owner, summon, mount, active enchant scroll, cursed weapon)
        // is true for any item the inventory has: none of those exist yet.
        let refused = template.is_none()
            || count == 0
            || (!settings.allow_discard_item && !gm)
            // `Item.isDropable()`: the template, and for an augmented item `AltAllowAugmentTrade`.
            || (!(template.is_some_and(|t| t.dropable)
                && item
                    .as_ref()
                    .is_none_or(|i| !i.is_augmented() || settings.alt_allow_augment_trade))
                && !(gm && settings.gm_trade_restricted_items))
            || self.is_inside_zone(id, ZoneId::NoItemDrop);
        let (Some(item), Some(template)) = (item, template) else {
            self.send(id, &discard_refused);
            return;
        };
        if refused {
            // An augmented item has its own message.
            if item.is_augmented() {
                self.send(
                    id,
                    &SystemMessage::new(SystemMessageId::TheAugmentedItemCannotBeDiscarded),
                );
            } else {
                self.send(id, &discard_refused);
            }
            return;
        }

        if template.quest_item && !(gm && settings.gm_trade_restricted_items) {
            return;
        }

        if count > item.count {
            self.send(id, &discard_refused);
            return;
        }

        // `PLAYER_SPAWN_PROTECTION > 0 && isInvul()`: no invulnerability state yet.

        if count < 0 {
            self.illegal_action(
                id,
                &format!("tried to drop item with oid {} but has count < 0!", object_id.0),
            );
            return;
        }

        if !template.stackable && count > 1 {
            self.illegal_action(
                id,
                &format!(
                    "tried to drop non-stackable item with oid {} but has count > 1!",
                    object_id.0
                ),
            );
            return;
        }

        if self.settings.zones.jail_disable_transaction && self.is_inside_zone(id, ZoneId::Jail) {
            self.send_message(id, "You cannot drop items in Jail.");
            return;
        }

        if !self.allows_transaction(id) {
            self.send_message(id, "Transactions are disabled for your Access Level.");
            self.send(id, &SystemMessage::new(SystemMessageId::NothingHappened));
            return;
        }

        // Transactions (a trade or an open request). Private stores (M6-STORE-01), fishing, flying
        // and a skill that consumes the item or is being cast (`isCastingNow`): none of them exist
        // yet.
        if self.is_processing_transaction(id) {
            self.send(
                id,
                &SystemMessage::new(
                    SystemMessageId::WhileOperatingAPrivateStoreOrWorkshopYouCannotDiscardDestroyOrTradeAnItem,
                ),
            );
            return;
        }

        if template.type2 == TYPE2_QUEST && !gm {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::ThatItemCannotBeDiscardedOrExchanged),
            );
            return;
        }

        let Some((px, py, pz)) = self.position_of(id) else {
            return;
        };
        let (dx, dy) = (f64::from(x.wrapping_sub(px)), f64::from(y.wrapping_sub(py)));
        if (dx * dx + dy * dy).sqrt() >= DROP_RANGE || (z.wrapping_sub(pz)).abs() > DROP_Z_RANGE {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouCannotDiscardSomethingThatFarAwayFromYou),
            );
            return;
        }

        // `canManipulateWithItemId` is always true here.

        if item.loc == LOC_PAPERDOLL {
            let unequipped =
                self.unequip_item_in_slot_and_record(id, usize::try_from(item.loc_data).unwrap_or(0));
            for o in &unequipped {
                self.un_charge_all_shots(*o);
            }
            let entries: Vec<ItemUpdate> = unequipped
                .iter()
                .filter_map(|o| self.modified_entry(id, *o))
                .collect();
            for entry in entries {
                self.send_inventory_update(id, entry);
            }
            self.broadcast_user_info(id);
            self.send_item_list(id, true);
        }

        let dropped = self.player_drop_item(id, object_id, count, x, y, z);

        // Java: `GMAudit.logAction(name [objectId], "Drop", target, "(id: ... x: ... z: ...)")` for a
        // GM (the file target `log/GMAudit/<name>.txt` comes with the logging card, D-25). Java
        // reads the dropped item before it checks it for null.
        if gm && self.settings.gm_audit {
            let name = self.players.get(&id).map_or("?", |p| p.data.name.as_str());
            let target = self
                .target_of(id)
                .and_then(|t| self.object_name(t))
                .unwrap_or_else(|| "no-target".to_owned());
            if let (Some(dropped), Some((px, py, pz))) = (dropped.as_ref(), self.position_of(id)) {
                let item_name = self.data.item_name(dropped.item_id).unwrap_or_default();
                tracing::info!(
                    target: "gmaudit",
                    "{name} [{id}]>Drop>{target}>(id: {} name: {item_name} objId: {} x: {px} y: {py} z: {pz})",
                    dropped.item_id,
                    dropped.object_id.0
                );
            }
        }

        if let Some(dropped) = dropped
            && dropped.item_id == l2_data::item::ADENA_ID
            && dropped.count >= 1_000_000
        {
            let name = self.players.get(&id).map_or("?", |p| p.data.name.as_str());
            let msg = format!(
                "Character ({name}) has dropped ({})adena at ({x},{y},{z})",
                dropped.count
            );
            warn!("{msg}");
            // `AdminData.broadcastMessageToGMs(msg)`.
            self.broadcast_message_to_gms(&msg);
        }
    }

    /// Java `Player.dropItem(process, objectId, count, x, y, z, reference, sendMessage,
    /// protectItem)` with `sendMessage` and `protectItem` false, as `RequestDropItem` calls it.
    /// Returns the item that lies on the ground now (the split-off part of a stack).
    pub(crate) fn player_drop_item(
        &mut self,
        id: ObjectId,
        object_id: ObjectId,
        count: i64,
        x: i32,
        y: i32,
        z: i32,
    ) -> Option<ItemData> {
        // A split stack needs a new object id.
        let new_object_id = match self.inventory_item(id, object_id) {
            Some(item) if item.count > count => self.allocate_item_id()?,
            _ => ObjectId(0),
        };
        let Some((dropped, shown)) = self.drop_from_inventory_quiet(id, object_id, count, new_object_id)
        else {
            if new_object_id.0 != 0 {
                self.release_item_id(new_object_id);
            }
            return None;
        };
        let placed = self.player_drop_to_ground(id, dropped.clone(), x, y, z, false);
        placed?;
        // Send inventory update packet; update current load as well.
        self.send_inventory_update(id, shown);
        self.send_cur_load(id);
        Some(dropped)
    }

    /// Java `Inventory.dropItem(process, objectId, count, ...)` and the inventory half of
    /// `Player.dropItem`: takes `count` of the item out of the inventory, tells the client
    /// (`InventoryUpdate`, `CUR_LOAD`) and returns the item to put on the ground.
    ///
    /// A worn item must be taken off first (the handler does that). When `count` is less than the
    /// stack, the returned item is a new one with `new_object_id`; otherwise it is the item
    /// itself, now owned by nobody (`loc` `VOID`). `None` when the item is not in the inventory.
    pub fn drop_from_inventory(
        &mut self,
        id: ObjectId,
        object_id: ObjectId,
        count: i64,
        new_object_id: ObjectId,
    ) -> Option<ItemData> {
        let (dropped, shown) = self.drop_from_inventory_quiet(id, object_id, count, new_object_id)?;
        // Send inventory update packet.
        self.send_inventory_update(id, shown);
        // Update current load as well.
        self.send_cur_load(id);
        Some(dropped)
    }

    /// [`Self::drop_from_inventory`] without the two packets: `Player.dropItem` sends them after
    /// `dropMe`, so the ground item is shown first. Returns the item to put on the ground and
    /// the entry of the `InventoryUpdate`.
    pub(crate) fn drop_from_inventory_quiet(
        &mut self,
        id: ObjectId,
        object_id: ObjectId,
        count: i64,
        new_object_id: ObjectId,
    ) -> Option<(ItemData, ItemUpdate)> {
        let item = self.inventory_item(id, object_id)?.clone();
        let (dropped, shown) = if item.count > count {
            // Adjust item quantity and create new instance to drop.
            let player = self.players.get_mut(&id)?;
            let target = player.data.items.iter_mut().find(|i| i.object_id == object_id)?;
            target.count = (target.count - count).max(0);
            let kept = target.clone();
            // `item.updateDatabase()`: the smaller stack is written at once.
            self.update_item_in_db(id, object_id);
            // `ItemManager.createItem`: owner 0, so no row until the ground item says so.
            let mut split = item.clone();
            split.object_id = new_object_id;
            split.count = count;
            split.loc = LOC_VOID.to_string();
            split.loc_data = 0;
            (split, self.update_entry(&kept, InventoryUpdate::MODIFIED))
        } else {
            // Directly drop entire item.
            let mut removed = self.remove_inventory_item(id, object_id)?;
            removed.loc = LOC_VOID.to_string();
            removed.loc_data = 0;
            self.remove_item_from_db(id, object_id);
            let shown = self.update_entry(&removed, InventoryUpdate::REMOVED);
            (removed, shown)
        };
        self.refresh_weight(id);
        Some((dropped, shown))
    }
}
