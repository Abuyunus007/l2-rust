//! `RequestCrystallizeItem`: an item is destroyed for crystals.
// Java: network/clientpackets/RequestCrystallizeItem.java
//
// # Not Java's
// `Player.isInCrystallize()` / `setInCrystallize` have no state here: Java sets the flag so that a
// second packet on another thread is refused while the first one runs, and the world handles one
// packet at a time.

use l2_data::item::types::CrystalType;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::client::RequestCrystallizeItem;
use l2_protocol_game::server::{
    ActionFailed, InventoryUpdate, ItemUpdate, SystemMessage, SystemMessageParam,
};
use l2_util::ObjectId;
use tracing::{info, warn};

use crate::flood::FloodKind;
use crate::inventory::item_name_param;
use crate::world::World;

/// Java `CommonSkill.CRYSTALLIZE` (id 248, level 1).
pub const CRYSTALLIZE_SKILL_ID: i32 = 248;
/// `Race.DWARF.ordinal()`.
const RACE_DWARF: i32 = 4;
/// The class ids (Fortune Seeker, Warsmith) Java leaves out of its "unexpected class" log.
const CLASS_FORTUNE_SEEKER: i32 = 117;
const CLASS_WARSMITH: i32 = 55;

impl World {
    /// Java `RequestCrystallizeItem.runImpl`.
    pub(crate) fn on_request_crystallize_item(&mut self, id: ObjectId, request: RequestCrystallizeItem) {
        let object_id = ObjectId(request.object_id);
        let mut count = request.count;
        if !self.players.contains_key(&id) {
            return;
        }
        if !self.flood_allows(id, FloodKind::Transaction) {
            self.send_message(id, "You are crystallizing too fast.");
            return;
        }
        if count < 1 {
            let name = self.players.get(&id).map_or("?", |p| p.data.name.as_str());
            let message = format!(
                "[RequestCrystallizeItem] count <= 0! ban! oid: {} owner: {name}",
                request.object_id
            );
            self.leftover_illegal_action(id, &message);
            return;
        }
        if self.is_in_store_mode(id) {
            self.send(
                id,
                &SystemMessage::new(
                    SystemMessageId::WhileOperatingAPrivateStoreOrWorkshopYouCannotDiscardDestroyOrTradeAnItem,
                ),
            );
            return;
        }

        let skill_level = self.known_skill(id, CRYSTALLIZE_SKILL_ID).map_or(0, |k| k.level);
        if skill_level <= 0 {
            self.send(
                id,
                &SystemMessage::new(
                    SystemMessageId::YouMayNotCrystallizeThisItemYourCrystallizationSkillLevelIsTooLow,
                ),
            );
            self.send(id, &ActionFailed);
            if let Some(p) = self.players.get(&id)
                && p.data.race != RACE_DWARF
                && p.data.class_id != CLASS_FORTUNE_SEEKER
                && p.data.class_id != CLASS_WARSMITH
            {
                info!(
                    "Player {} used crystalize with classid: {}",
                    p.data.name, p.data.class_id
                );
            }
            return;
        }

        let templates = self.item_templates();
        let templates = templates.as_deref();
        let Some(item) = self.inventory_item(id, object_id).cloned() else {
            self.send(id, &ActionFailed);
            return;
        };
        let template = templates.and_then(|t| t.get(item.item_id));
        let hero = template.is_some_and(|t| t.is_hero_item());
        if hero || (!self.settings.inventory.alt_allow_augment_destroy && item.is_augmented()) {
            self.send(id, &ActionFailed);
            return;
        }
        if count > item.count {
            count = item.count;
        }

        // `itemToRemove.isShadowItem() || itemToRemove.isTimeLimitedItem()`
        if item.mana_left >= 0 || item.time > 0 {
            return;
        }
        let Some(template) = template else {
            return;
        };
        if !template.is_crystallizable() {
            warn!(
                "{} ({}) tried to crystallize {}",
                self.players.get(&id).map_or("?", |p| p.data.name.as_str()),
                id.0,
                template.id
            );
            return;
        }
        if !self.can_manipulate_with_item_id(id, item.item_id) {
            self.send_message(id, "You cannot use this item.");
            return;
        }

        // Check if the char can crystallize items and return if false.
        let can_crystallize = match template.crystal_type_plus() {
            CrystalType::C => skill_level > 1,
            CrystalType::B => skill_level > 2,
            CrystalType::A => skill_level > 3,
            CrystalType::S => skill_level > 4,
            _ => true,
        };
        if !can_crystallize {
            self.send(
                id,
                &SystemMessage::new(
                    SystemMessageId::YouMayNotCrystallizeThisItemYourCrystallizationSkillLevelIsTooLow,
                ),
            );
            self.send(id, &ActionFailed);
            return;
        }

        let crystal_id = template.crystal_item_id();
        // `itemToRemove.getCrystalCount()`: the template's count at the item's enchant level.
        let crystal_amount = i64::from(template.crystal_count_at(item.enchant_level));

        // Unequip if needed.
        if item.loc == "PAPERDOLL" {
            let unequipped =
                self.unequip_item_in_slot_and_record(id, usize::try_from(item.loc_data).unwrap_or(0));
            let entries: Vec<ItemUpdate> = unequipped
                .iter()
                .filter_map(|o| self.modified_entry(id, *o))
                .collect();
            // Sent inventory update for unequip instantly.
            self.send_inventory_update_now(id, entries);
            let message = self.disarmed_message(item.enchant_level, item.item_id, templates);
            self.send(id, &message);
        }

        // Remove from inventory.
        let Some(removed) = self.destroy_inventory_item(id, object_id, count) else {
            return;
        };
        // NOTE(java-quirk): the update always says "removed", even when a part of a stack is left.
        let entry = self.update_entry(&removed, InventoryUpdate::REMOVED);
        // Sent inventory update for destruction instantly.
        self.send_inventory_update_now(id, vec![entry]);

        // Add crystals.
        let created = self.inventory_add_item_by_id(id, crystal_id, crystal_amount);
        let templates = self.item_templates();
        let templates = templates.as_deref();
        self.send(
            id,
            &SystemMessage::new(SystemMessageId::S1HasBeenCrystallized)
                .with(item_name_param(templates, removed.item_id)),
        );
        if let Some(created) = created {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouHaveEarnedS2S1S)
                    .with(item_name_param(templates, created.item.item_id))
                    .with(SystemMessageParam::LongNumber(crystal_amount)),
            );
        }

        self.broadcast_user_info(id);
    }
}
