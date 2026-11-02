//! Transmog (`Transmog.ini`): an equipped item shows the look of another item the player owned.
//! The appearance collection of a player, the transmog id of an item and what the packets show.
// Java: dist/game/data/scripts/custom/Transmog/Transmog.java (the collection)
// Java: entity/item/instance/Item.java (`getTransmogId`, `setTransmogId`, `removeTransmog`)
// Java: entity/itemcontainer/Inventory.java (`getPaperdollItemDisplayId`)
//
// # How it fits in
// - The collection (`PLAYER_TRANSMOGS`) and the transmog ids of the items (`item_variables`,
//   `transmogId`) live here, by object id. They are read when the player enters the world: the
//   world asks the persistence task (`PersistCommand::TransmogLoadRequest`, D-15) and gets
//   `WorldCommand::TransmogLoaded` back, then shows the look to everybody.
// - The writes are per-player rows (D-39): the collection is stored when the player leaves
//   (`character_transmogs`, owner = account name with `TransmogShareAccount`, else the object id),
//   a transmog id as soon as it changes (`item_variables`).
// - The script (`l2-scripts/src/custom/transmog.rs`) is the NPC and the menu; the look in
//   `UserInfo` / `CharInfo` is `World::transmog_display_id`.

use std::collections::BTreeMap;

use l2_data::item::types::BodyPart;
use l2_util::ObjectId;
use tracing::warn;

use crate::outbox::PersistCommand;
use crate::world::World;

/// The state of the transmog system.
#[derive(Debug, Default)]
pub struct TransmogWorld {
    /// Java `PLAYER_TRANSMOGS`: player -> body part mask -> the item ids of the collection, in
    /// the order they were added (a Java `HashSet` iterates by hash, see [`java_set_order`]).
    pub(crate) collections: BTreeMap<ObjectId, BTreeMap<i32, Vec<i32>>>,
    /// The `transmogId` of the items that have one: item object id -> id of the item copied.
    pub(crate) item_transmog: BTreeMap<ObjectId, i32>,
}

/// What the persistence task read for a player entering the world: the collection (item ids)
/// and the `transmogId` of each item of the inventory that has one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TransmogLoad {
    /// The item ids of `character_transmogs`.
    pub collection: Vec<i32>,
    /// `(item object id, transmog id)`.
    pub items: Vec<(i32, i32)>,
}

/// The order a Java `HashSet<Integer>` iterates values that were added in this order: by bucket
/// (`hash ^ (hash >>> 16)` modulo the table size), in the order added inside a bucket. The table
/// starts at 16 and doubles when more than three quarters of it are used. A value that repeats
/// is kept once.
// NOTE: the table never shrinks in Java; the size here comes from the present number of values.
pub fn java_set_order(values: &[i32]) -> Vec<i32> {
    let mut unique: Vec<i32> = Vec::with_capacity(values.len());
    for value in values {
        if !unique.contains(value) {
            unique.push(*value);
        }
    }
    let mut table: usize = 16;
    while unique.len() > table / 4 * 3 {
        table *= 2;
    }
    let mut buckets: BTreeMap<usize, Vec<i32>> = BTreeMap::new();
    for value in &unique {
        let hash = *value as u32;
        let spread = ((hash ^ (hash >> 16)) & 0x7fff_ffff) as usize;
        buckets.entry(spread & (table - 1)).or_default().push(*value);
    }
    buckets.into_values().flatten().collect()
}

/// Java `Transmog.getBodypart(slot)`: the body part of a menu slot name (`NONE` for another).
pub fn transmog_body_part(slot: &str) -> BodyPart {
    match slot {
        "R_HAND" => BodyPart::RightHand,
        "L_HAND" => BodyPart::LeftHand,
        "GLOVES" => BodyPart::Gloves,
        "CHEST" => BodyPart::Chest,
        "LEGS" => BodyPart::Legs,
        "FEET" => BodyPart::Feet,
        "BACK" => BodyPart::Back,
        "LR_HAND" => BodyPart::BothHands,
        "FULL_ARMOR" => BodyPart::FullArmor,
        "HAIRALL" => BodyPart::HairAll,
        "HAIR2" => BodyPart::Hair2,
        "HAIR" => BodyPart::Hair,
        _ => BodyPart::None,
    }
}

impl World {
    /// `TransmogConfig.ENABLE_TRANSMOG`.
    pub fn transmog_enabled(&self) -> bool {
        self.custom_scripts.settings.transmog.enabled
    }

    /// The owner column of `character_transmogs`: the account name with `TransmogShareAccount`,
    /// else the object id.
    fn transmog_owner(&self, player: ObjectId) -> Option<String> {
        let account = &self.players.get(&player)?.data.account;
        Some(if self.custom_scripts.settings.transmog.share_account {
            account.clone()
        } else {
            player.0.to_string()
        })
    }

    /// Java `onPlayerLogin`: asks for the collection and the transmog ids of the items.
    pub fn transmog_request_load(&mut self, player: ObjectId) {
        let Some(owner) = self.transmog_owner(player) else {
            return;
        };
        let item_object_ids = self
            .players
            .get(&player)
            .map(|p| p.data.items.iter().map(|i| i.object_id.0).collect())
            .unwrap_or_default();
        self.outbox.persist(PersistCommand::TransmogLoadRequest {
            player,
            owner,
            item_object_ids,
        });
    }

    /// The answer of the read: the collection by body part and the items' transmog ids. A failed
    /// read leaves the player without a collection (Java logs "Problem with Transmog").
    pub(crate) fn on_transmog_loaded(&mut self, player: ObjectId, result: Result<TransmogLoad, ()>) {
        if !self.players.contains_key(&player) {
            return;
        }
        let load = match result {
            Ok(load) => load,
            Err(()) => {
                warn!("Problem with Transmog: the rows of {} could not be read.", player.0);
                return;
            }
        };
        let mut collection: BTreeMap<i32, Vec<i32>> = BTreeMap::new();
        for item_id in load.collection {
            // `ItemData.getTemplate(itemId) != null`.
            let Some(mask) = self.item_templates().and_then(|t| t.get(item_id).map(|t| t.body_part.mask())) else {
                continue;
            };
            let ids = collection.entry(mask).or_default();
            if !ids.contains(&item_id) {
                ids.push(item_id);
            }
        }
        if !collection.is_empty() {
            self.custom_scripts.transmog.collections.insert(player, collection);
        }
        let mut changed = false;
        for (object_id, transmog_id) in load.items {
            if transmog_id > 0 {
                self.custom_scripts.transmog.item_transmog.insert(ObjectId(object_id), transmog_id);
                changed = true;
            }
        }
        if changed {
            self.broadcast_user_info(player);
        }
    }

    /// Java `onPlayerLogout`: the collection is stored (`REPLACE`, one row per item) and the
    /// player's entries are forgotten.
    pub fn transmog_store_and_forget(&mut self, player: ObjectId) {
        let Some(owner) = self.transmog_owner(player) else {
            return;
        };
        let item_ids: Vec<i32> = self
            .custom_scripts
            .transmog
            .collections
            .get(&player)
            .map(|by_part| by_part.values().flat_map(|ids| java_set_order(ids)).collect())
            .unwrap_or_default();
        self.outbox
            .persist(PersistCommand::TransmogSaveCollection { owner, item_ids });
        self.custom_scripts.transmog.collections.remove(&player);
        let own: Vec<ObjectId> = self
            .players
            .get(&player)
            .map(|p| p.data.items.iter().map(|i| i.object_id).collect())
            .unwrap_or_default();
        for object_id in own {
            self.custom_scripts.transmog.item_transmog.remove(&object_id);
        }
    }

    /// Java `onPlayerItemAdd`: a new item the player owns joins the collection of its body part
    /// (by its display id), with the whisper of the collection. Not an etc item, not an NPC
    /// item, only the body parts from the right hand to the whole hair, not a banned id.
    pub fn transmog_on_item_added(&mut self, player: ObjectId, item_id: i32) {
        use l2_data::item::ItemKind;
        let Some(template) = self.item_templates().and_then(|t| t.get(item_id).cloned()) else {
            return;
        };
        if matches!(template.kind, ItemKind::Etc(_)) || template.for_npc {
            return;
        }
        let mask = template.body_part.mask();
        if mask < BodyPart::RightHand.mask() || mask > BodyPart::HairAll.mask() {
            return;
        }
        if self.custom_scripts.settings.transmog.banned_item_ids.contains(&template.id) {
            return;
        }
        let ids = self
            .custom_scripts
            .transmog
            .collections
            .entry(player)
            .or_default()
            .entry(mask)
            .or_default();
        if ids.contains(&template.display_id) {
            return;
        }
        ids.push(template.display_id);
        let text = format!("{} has been added to your appearance collection.", template.name);
        self.send_creature_say_text(
            player,
            None,
            l2_protocol_game::server::ChatType::Whisper,
            "[Transmog]",
            &text,
        );
    }

    /// The item ids of the collection of a body part, in the order a Java `HashSet` lists them.
    pub fn transmog_collection(&self, player: ObjectId, mask: i32) -> Vec<i32> {
        self.custom_scripts
            .transmog
            .collections
            .get(&player)
            .and_then(|by_part| by_part.get(&mask))
            .map(|ids| java_set_order(ids))
            .unwrap_or_default()
    }

    /// Java `Item.getTransmogId()`: 0 while the system is off.
    pub fn item_transmog_id(&self, item: ObjectId) -> i32 {
        if !self.transmog_enabled() {
            return 0;
        }
        self.custom_scripts.transmog.item_transmog.get(&item).copied().unwrap_or(0)
    }

    /// Java `isEquipable() && getTransmogId() > 0`, what keeps a transmogrified item from being
    /// dropped, traded or sold.
    pub(crate) fn item_is_transmogrified(&self, item: ObjectId, template: &l2_data::item::ItemTemplate) -> bool {
        crate::inventory::is_equipable(template) && self.item_transmog_id(item) > 0
    }

    /// Java `Inventory.getPaperdollItemDisplayId(slot)` with the transmog on: the id the packets
    /// show instead of the display id of the item.
    pub fn transmog_display_id(&self, item: ObjectId) -> Option<i32> {
        let id = self.item_transmog_id(item);
        (id > 0).then_some(id)
    }

    /// Java `item.setTransmogId(id)` and `storeMe()`.
    pub fn transmog_set_item(&mut self, item: ObjectId, transmog_id: i32) {
        self.custom_scripts.transmog.item_transmog.insert(item, transmog_id);
        self.outbox.persist(PersistCommand::ItemTransmog {
            object_id: item.0,
            transmog_id: Some(transmog_id),
        });
    }

    /// Java `item.removeTransmog()` and `storeMe()`.
    pub fn transmog_remove_item(&mut self, item: ObjectId) {
        self.custom_scripts.transmog.item_transmog.remove(&item);
        self.outbox.persist(PersistCommand::ItemTransmog {
            object_id: item.0,
            transmog_id: None,
        });
    }

    /// Java `player.getInventory().unEquipItemInBodySlot(part)`: the item that came off.
    pub fn transmog_unequip_body_slot(&mut self, player: ObjectId, slot: &str) -> Option<ObjectId> {
        let part = transmog_body_part(slot);
        let mut changed = Vec::new();
        let old = self.unequip_item_in_body_slot(player, part, &mut changed);
        self.send_item_list(player, false);
        old
    }

    /// The template facts the menu shows: name, icon and display id.
    pub fn transmog_item_info(&self, item_id: i32) -> Option<(String, Option<String>, i32)> {
        let templates = self.item_templates()?;
        let t = templates.get(item_id)?;
        Some((t.name.clone(), t.icon.clone(), t.display_id))
    }

    /// The body part mask of a menu slot (0 for a name that is none).
    pub fn transmog_slot_mask(slot: &str) -> i32 {
        transmog_body_part(slot).mask()
    }

    /// Java `HtmlUtil.sendCBHtml(player, html)`: a page on the community board window.
    pub fn transmog_send_page(&mut self, player: ObjectId, html: &str) {
        self.send_cb_html(player, html);
    }

    /// Java `PlayerInventory.equipItem(item)` of the item that was taken off for the menu.
    pub fn transmog_equip(&mut self, player: ObjectId, item: ObjectId) {
        self.event_equip_object(player, item);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_java_hash_set_lists_small_ints_by_bucket() {
        // 16 buckets: 17 -> 1, 33 -> 1, 2 -> 2, 15 -> 15; the two in bucket 1 keep their order.
        assert_eq!(java_set_order(&[15, 17, 2, 33]), [17, 33, 2, 15]);
        // A repeated value is kept once.
        assert_eq!(java_set_order(&[3, 3, 1]), [1, 3]);
    }

    #[test]
    fn the_table_doubles_above_three_quarters() {
        // 12 values fit the table of 16; the 13th makes it 32: 20 moves from bucket 4 to 20.
        let mut values: Vec<i32> = (100..112).collect();
        values.push(0);
        values.insert(0, 20);
        // 13 values: table 32. 20 -> 20, 100.. -> 4 (100), 5 ..., 0 -> 0.
        let order = java_set_order(&values);
        assert_eq!(order[0], 0);
        assert!(order.iter().position(|v| *v == 100) < order.iter().position(|v| *v == 20));
    }

    #[test]
    fn menu_slots_map_to_body_parts() {
        assert_eq!(transmog_body_part("CHEST").mask(), 0x400);
        assert_eq!(transmog_body_part("HAIRALL").mask(), 0x80000);
        assert_eq!(transmog_body_part("HEAD").mask(), 0);
    }
}
