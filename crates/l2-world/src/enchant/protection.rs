//! `OverEnchantProtection`: items enchanted past what `EnchantItemGroupsData` has a chance for.
// Java: network/clientpackets/UseItem.java (the "Over-enchant protection." block)
// Java: network/clientpackets/EnterWorld.java (the same block, for every item at login)
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminCreateItem.java (getMaxEnchant)

use l2_data::item::{ItemTemplate, TYPE2_ACCESSORY};
use l2_protocol_game::server::ExShowScreenMessage;
use l2_util::ObjectId;
use tracing::info;

use crate::inventory::is_equipable;
use crate::player::ItemData;
use crate::world::World;

impl World {
    /// Java `AdminCreateItem.getMaxEnchant(item)`, and the limits of `UseItem` and `EnterWorld`:
    /// the highest enchant level of the item's kind. A world without `EnchantItemGroupsData`
    /// (nothing installed) has no limit.
    pub fn max_enchant_of(&self, template: &ItemTemplate) -> i32 {
        let Some(groups) = &self.enchant.groups else {
            return i32::MAX;
        };
        if template.is_weapon() {
            groups.max_weapon_enchant()
        } else if template.type2 == TYPE2_ACCESSORY {
            groups.max_accessory_enchant()
        } else {
            groups.max_armor_enchant()
        }
    }

    /// The condition shared by `UseItem` and `EnterWorld`: the item is enchanted past the maximum of
    /// its kind (`isWeapon`, accessory, then armor that is no accessory).
    fn is_over_enchanted(&self, item: &ItemData, template: &ItemTemplate) -> bool {
        let Some(groups) = &self.enchant.groups else {
            return false;
        };
        let level = item.enchant_level;
        (template.is_weapon() && level > groups.max_weapon_enchant())
            || (template.type2 == TYPE2_ACCESSORY && level > groups.max_accessory_enchant())
            || (template.is_armor()
                && template.type2 != TYPE2_ACCESSORY
                && level > groups.max_armor_enchant())
    }

    /// `UseItem`'s over-enchant block for `item` that is about to be equipped or taken off: when it
    /// holds, the item is destroyed and `true` is returned (the caller stops).
    // NOTE(java-quirk): Java destroys it with `Inventory.destroyItem`, which sends the client
    // nothing; the item stays in the client's list until the next `ItemList`.
    pub(crate) fn over_enchant_protection_on_use(
        &mut self,
        id: ObjectId,
        item: &ItemData,
        template: &ItemTemplate,
    ) -> bool {
        if !self.settings.enchant.over_enchant_protection
            || self.is_gm(id)
            || !self.is_over_enchanted(item, template)
        {
            return false;
        }
        info!(
            "Over-enchanted (+{}) {} has been removed from {}",
            item.enchant_level,
            template.label(),
            self.player_text(id)
        );
        self.destroy_inventory_item(id, item.object_id, item.count);
        self.punish_over_enchant(id, true);
        true
    }

    /// `EnterWorld`'s over-enchant block: every equipable item past the maximum is destroyed, and
    /// the player is punished once.
    pub(crate) fn over_enchant_protection_on_enter(&mut self, id: ObjectId) {
        if !self.settings.enchant.over_enchant_protection || self.is_gm(id) {
            return;
        }
        let Some(templates) = self.item_templates() else {
            return;
        };
        let Some(player) = self.players.get(&id) else {
            return;
        };
        let over: Vec<ItemData> = player
            .data
            .items
            .iter()
            .filter(|item| item.loc == "INVENTORY" || item.loc == "PAPERDOLL")
            .filter(|item| {
                templates
                    .get(item.item_id)
                    .is_some_and(|t| is_equipable(t) && self.is_over_enchanted(item, t))
            })
            .cloned()
            .collect();
        let mut punish = false;
        for item in over {
            info!(
                "Over-enchanted (+{}) {} has been removed from {}",
                item.enchant_level,
                templates
                    .get(item.item_id)
                    .map_or_else(String::new, ItemTemplate::label),
                self.player_text(id)
            );
            self.destroy_inventory_item(id, item.object_id, item.count);
            punish = true;
        }
        self.punish_over_enchant(id, punish);
    }

    /// The end of both blocks: the messages and `OVER_ENCHANT_PUNISHMENT` (unless it is `NONE`).
    fn punish_over_enchant(&mut self, id: ObjectId, punish: bool) {
        let punishment = self.settings.enchant.over_enchant_punishment;
        if !punish || punishment == crate::punishments::IllegalActionPunishment::None {
            return;
        }
        self.send_message(id, "[Server]: You have over-enchanted items!");
        self.send_message(id, "[Server]: Respect our server rules.");
        self.send(
            id,
            &ExShowScreenMessage::text("You have over-enchanted items!", 6000),
        );
        let name = self
            .players
            .get(&id)
            .map(|p| p.data.name.clone())
            .unwrap_or_default();
        self.handle_illegal_player_action(id, &format!("{name} has over-enchanted items."), punishment);
    }
}
