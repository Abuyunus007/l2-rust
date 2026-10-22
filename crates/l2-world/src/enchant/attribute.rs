//! Element attributes: using a stone, crystal or jewel on an item, taking an element off at the
//! NPC, and the window of the NPC.
// Java: dist/game/data/scripts/handlers/items/EnchantAttribute.java
// Java: network/clientpackets/{RequestExEnchantItemAttribute,RequestExRemoveItemAttribute}.java
// Java: dist/game/data/scripts/handlers/bypass/npc/ReleaseAttribute.java
// Java: network/serverpackets/{ExChooseInventoryAttributeItem,ExShowBaseAttributeCancelWindow}.java
//
// The element rows are `item_elementals`; `World::set_item_elemental` and `clear_item_elemental`
// (`effects/item_extras.rs`) write them with the item, as Java's `setElementAttr` and
// `clearElementAttr` do.

use l2_data::enchant::elementals::{
    self, ARMOR_BONUS, ARMOR_VALUES, FIRST_WEAPON_BONUS, NEXT_WEAPON_BONUS, WEAPON_VALUES, opposite_element,
};
use l2_data::item::types::CrystalType;
use l2_data::{ElementalItemType, ItemTemplate};
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    ExAttributeEnchantResult, ExBaseAttributeCancelResult, ExChooseInventoryAttributeItem,
    ExShowBaseAttributeCancelWindow, SystemMessage, SystemMessageParam,
};
use l2_util::ObjectId;
use tracing::warn;

use crate::inventory::{LOC_INVENTORY, LOC_PAPERDOLL, item_name_param};
use crate::player::ItemData;
use crate::world::World;

/// Java `Item.isElementable()`: in the inventory or worn, and the template allows elements.
fn item_is_elementable(item: &ItemData, template: &ItemTemplate) -> bool {
    (item.loc == LOC_INVENTORY || item.loc == LOC_PAPERDOLL) && template.elementable
}

/// `RequestExRemoveItemAttribute.getPrice` and `ExShowBaseAttributeCancelWindow.getPrice` for one
/// item, given the price field the packet instance still holds.
// NOTE(java-quirk): `_price` is a field of the packet and a grade other than S, S80 and S84
// leaves it as it was, so the window shows the price of the previous item for such an item (0
// for the first), and a removal of one costs nothing.
fn cancel_price(template: &ItemTemplate, previous: i64) -> i64 {
    let weapon = template.is_weapon();
    match template.crystal_type {
        CrystalType::S => {
            if weapon {
                50_000
            } else {
                40_000
            }
        }
        CrystalType::S80 => {
            if weapon {
                100_000
            } else {
                80_000
            }
        }
        CrystalType::S84 => {
            if weapon {
                200_000
            } else {
                160_000
            }
        }
        _ => previous,
    }
}

/// Java `ReleaseAttribute.onCommand`: the NPC link shows the cancel window.
pub fn release_attribute_handler(
    world: &mut World,
    player: ObjectId,
    _command: &str,
    target: Option<ObjectId>,
) -> bool {
    if !target.is_some_and(|t| world.npcs.contains_key(&t)) {
        return false;
    }
    world.send_base_attribute_cancel_window(player);
    true
}

impl World {
    /// Java `new ExShowBaseAttributeCancelWindow(player)`: every item with an element and what it
    /// costs to take it off.
    pub fn send_base_attribute_cancel_window(&self, id: ObjectId) {
        let Some(player) = self.players.get(&id) else {
            return;
        };
        let templates = self.item_templates();
        let mut price = 0;
        let mut items = Vec::new();
        for item in player
            .data
            .items
            .iter()
            .filter(|i| (i.loc == LOC_INVENTORY || i.loc == LOC_PAPERDOLL) && !i.elementals.is_empty())
        {
            if let Some(template) = templates.as_ref().and_then(|t| t.get(item.item_id)) {
                price = cancel_price(template, price);
            }
            items.push((item.object_id.0, price));
        }
        self.send(id, &ExShowBaseAttributeCancelWindow { items });
    }

    /// Java `EnchantAttribute.onItemUse`: opens the window to choose the item.
    pub(crate) fn enchant_attribute_on_use(&mut self, id: ObjectId, item: &ItemData) -> bool {
        if self.is_casting_now(id) {
            return false;
        }
        if self.is_enchanting(id) {
            self.send(
                id,
                &SystemMessage::new(
                    SystemMessageId::AnotherEnchantmentIsInProgressPleaseCompleteThePreviousTaskThenTryAgain,
                ),
            );
            return false;
        }
        if let Some(player) = self.players.get_mut(&id) {
            player.enchant.active_attr = Some(item.object_id);
        }
        // `new ExChooseInventoryAttributeItem(item)` reads the stone by its display id.
        let templates = self.item_templates();
        let display_id = templates
            .as_ref()
            .and_then(|t| t.get(item.item_id))
            .map_or(item.item_id, |t| t.display_id);
        let elementals = self.enchant.elementals.clone();
        let element = elementals
            .as_ref()
            .map_or(elementals::NONE, |e| e.item_element(display_id));
        if element == elementals::NONE {
            // NOTE(java-quirk): the constructor throws `IllegalArgumentException` after the stone
            // was stored as the active one, so the player cannot log out until the window is
            // cancelled or the stone leaves the inventory.
            warn!("EnchantAttribute: Undefined Atribute item: {}", item.item_id);
            return false;
        }
        let max_level = elementals
            .as_ref()
            .map_or(-1, |e| e.max_element_level(display_id));
        self.send(
            id,
            &ExChooseInventoryAttributeItem::new(display_id, element, max_level),
        );
        true
    }

    /// `setActiveEnchantAttrItemId(ID_NONE)`.
    fn clear_active_enchant_attr(&mut self, id: ObjectId) {
        if let Some(player) = self.players.get_mut(&id) {
            player.enchant.active_attr = None;
        }
    }

    /// Java `RequestExEnchantItemAttribute.runImpl`.
    pub(crate) fn on_request_enchant_item_attribute(&mut self, id: ObjectId, object_id: i32) {
        if object_id == -1 {
            // Player canceled enchant.
            self.clear_active_enchant_attr(id);
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::AttributeItemUsageHasBeenCancelled),
            );
            return;
        }
        let object_id = ObjectId(object_id);

        if self.is_in_store_mode(id) {
            self.send(
                id,
                &SystemMessage::new(
                    SystemMessageId::YouCannotAddElementalPowerWhileOperatingAPrivateStoreOrPrivateWorkshop,
                ),
            );
            self.clear_active_enchant_attr(id);
            return;
        }

        // Restrict enchant during a trade (bug if enchant fails).
        if self.active_requester(id).is_some() {
            // Cancel trade
            self.cancel_active_trade(id);
            self.clear_active_enchant_attr(id);
            self.send_message(id, "You cannot add elemental power while trading.");
            return;
        }

        let item = self.inventory_item(id, object_id).cloned();
        let stone = self
            .players
            .get(&id)
            .and_then(|p| p.enchant.active_attr)
            .and_then(|o| self.inventory_item(id, o))
            .cloned();
        let (Some(item), Some(stone)) = (item, stone) else {
            self.clear_active_enchant_attr(id);
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::AttributeItemUsageHasBeenCancelled),
            );
            return;
        };

        let templates = self.item_templates();
        let Some(item_template) = templates.as_ref().and_then(|t| t.get(item.item_id)) else {
            self.clear_active_enchant_attr(id);
            return;
        };
        let elemental_data = self.enchant.elementals.clone();

        if !item_is_elementable(&item, item_template) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::ElementalPowerEnhancerUsageRequirementIsNotSufficient),
            );
            self.clear_active_enchant_attr(id);
            return;
        }
        // `item.getItemLocation()` is the inventory or the paperdoll here: `inventory_item` finds
        // nothing else, so the exploit branch of Java's `switch` cannot happen.

        let stone_id = stone.item_id;
        let mut element_to_add = elemental_data
            .as_ref()
            .map_or(elementals::NONE, |e| e.item_element(stone_id));
        let weapon = item_template.is_weapon();
        let armor = item_template.is_armor();

        // Armors have the opposite element.
        if armor {
            element_to_add = opposite_element(element_to_add);
        }

        let old_element = item.elemental(element_to_add);
        // NOTE(java-quirk): `getElemental(elementToAdd)` only finds the same element, so for a
        // weapon `oldElement.getElement() != elementToAdd` is never true and a weapon takes a
        // second element.
        if (weapon && old_element.is_some_and(|old| old.element != element_to_add && old.element != -2))
            || (armor && old_element.is_none() && !item.elementals.is_empty() && item.elementals.len() >= 3)
        {
            self.send(
                id,
                &SystemMessage::new(
                    SystemMessageId::AnotherElementalPowerHasAlreadyBeenAddedThisElementalPowerCannotBeAdded,
                ),
            );
            self.clear_active_enchant_attr(id);
            return;
        }

        let element_value = old_element.map_or(0, |old| old.value);
        let opposite = opposite_element(element_to_add);
        let limit = Self::attribute_limit(
            weapon,
            elemental_data
                .as_ref()
                .and_then(|e| e.get(stone_id))
                .map(|h| h.kind),
        );
        let mut power_to_add = Self::attribute_power_to_add(
            elemental_data
                .as_ref()
                .map_or(elementals::NONE, |e| e.item_element(stone_id)),
            element_value,
            weapon,
            armor,
        );

        if armor && !item.elementals.is_empty() {
            // cannot add opposite element
            for elemental in &item.elementals {
                if elemental.element == opposite {
                    self.clear_active_enchant_attr(id);
                    let message = format!("{} tried to add oposite attribute to item!", self.player_text(id));
                    self.enchant_illegal_action(id, &message);
                    return;
                }
            }
        }

        let mut new_power = element_value.wrapping_add(power_to_add);
        if new_power > limit {
            new_power = limit;
            power_to_add = limit.wrapping_sub(element_value);
        }

        if power_to_add <= 0 {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::AttributeItemUsageHasBeenCancelled),
            );
            self.clear_active_enchant_attr(id);
            return;
        }

        if !self.destroy_item_by_object_id(id, stone.object_id, 1, true) {
            self.send(id, &SystemMessage::new(SystemMessageId::IncorrectItemCount2));
            let message = format!(
                "{} tried to attribute enchant with a stone he doesn't have",
                self.player_text(id)
            );
            self.enchant_illegal_action(id, &message);
            self.clear_active_enchant_attr(id);
            return;
        }

        let settings = &self.enchant_chances();
        let kind = elemental_data
            .as_ref()
            .and_then(|e| e.get(stone_id))
            .map(|h| h.kind);
        let chance = match kind {
            Some(ElementalItemType::Stone | ElementalItemType::RoughOre) => Some(settings.0),
            Some(ElementalItemType::Crystal) => Some(settings.1),
            Some(ElementalItemType::Jewel) => Some(settings.2),
            Some(ElementalItemType::Energy) => Some(settings.3),
            None => None,
        };
        // `Rnd.get(100) < chance`; a stone Java has no entry for would throw a
        // `NullPointerException` here, after it was used up.
        let success = chance.is_some_and(|chance| f64::from(self.rng().below(100)) < chance);

        if success {
            let real_element = if armor { opposite } else { element_to_add };
            let name = item_name_param(templates.as_deref(), item.item_id);
            let mut sm;
            if item.enchant_level == 0 {
                sm = SystemMessage::new(if armor {
                    SystemMessageId::TheS2SAttributeWasSuccessfullyBestowedOnS1AndResistanceToS3WasIncreased
                } else {
                    SystemMessageId::S2ElementalPowerHasBeenAddedSuccessfullyToS1
                })
                .with(name)
                .with(SystemMessageParam::ElementName(real_element));
            } else {
                sm = SystemMessage::new(if armor {
                    SystemMessageId::TheS3SAttributeWasSuccessfullyBestowedOnS1S2AndResistanceToS4WasIncreased
                } else {
                    SystemMessageId::S3ElementalPowerHasBeenAddedSuccessfullyToS1S2
                })
                .with(SystemMessageParam::IntNumber(item.enchant_level))
                .with(name)
                .with(SystemMessageParam::ElementName(real_element));
            }
            if armor {
                sm = sm.with(SystemMessageParam::ElementName(opposite_element(real_element)));
            }
            self.send(id, &sm);

            self.set_item_elemental(item.object_id, element_to_add, new_power);
            if item.loc == LOC_PAPERDOLL {
                self.update_item_element_bonus(id, item.object_id);
            }

            // send packets
            if let Some(entry) = self.modified_entry(id, item.object_id) {
                self.send_inventory_update(id, entry);
            }
        } else {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouHaveFailedToAddElementalPower),
            );
        }

        self.send(id, &ExAttributeEnchantResult { result: power_to_add });
        self.send_user_info(id);
        self.clear_active_enchant_attr(id);
    }

    /// `PlayerConfig.ENCHANT_CHANCE_ELEMENT_STONE`, `_CRYSTAL`, `_JEWEL`, `_ENERGY`.
    fn enchant_chances(&self) -> (f64, f64, f64, f64) {
        let s = &self.settings.enchant;
        (
            s.enchant_chance_element_stone,
            s.enchant_chance_element_crystal,
            s.enchant_chance_element_jewel,
            s.enchant_chance_element_energy,
        )
    }

    /// Java `RequestExEnchantItemAttribute.getLimit(item, stoneId)`: the most an item can get from
    /// that kind of stone; 0 for an item that is no stone.
    fn attribute_limit(weapon: bool, kind: Option<ElementalItemType>) -> i32 {
        let Some(kind) = kind else {
            return 0;
        };
        let index = usize::try_from(kind.max_level()).unwrap_or(0);
        let table = if weapon { &WEAPON_VALUES } else { &ARMOR_VALUES };
        table.get(index).copied().unwrap_or(0)
    }

    /// Java `RequestExEnchantItemAttribute.getPowerToAdd(stoneId, oldValue, item)`.
    fn attribute_power_to_add(stone_element: i32, old_value: i32, weapon: bool, armor: bool) -> i32 {
        if stone_element != elementals::NONE {
            if weapon {
                if old_value == 0 {
                    return FIRST_WEAPON_BONUS;
                }
                return NEXT_WEAPON_BONUS;
            } else if armor {
                return ARMOR_BONUS;
            }
        }
        0
    }

    /// Java `RequestExRemoveItemAttribute.runImpl`.
    pub(crate) fn on_request_remove_item_attribute(&mut self, id: ObjectId, object_id: i32, element: i32) {
        // `(byte) readInt()`.
        let element = i32::from(element as i8);
        let Some(item) = self.inventory_item(id, ObjectId(object_id)).cloned() else {
            return;
        };
        if item.elementals.is_empty() || item.elemental(element).is_none() {
            return;
        }
        let templates = self.item_templates();
        let Some(template) = templates.as_ref().and_then(|t| t.get(item.item_id)) else {
            return;
        };

        let price = cancel_price(template, 0);
        if self.reduce_adena(id, price, true) {
            let equipped = item.loc == LOC_PAPERDOLL;
            self.clear_item_elemental(item.object_id, element);
            if equipped {
                // `getElemental(element).removeBonus(player)`: the other elements stay on.
                self.update_item_element_bonus(id, item.object_id);
            }
            self.send_user_info(id);

            if let Some(entry) = self.modified_entry(id, item.object_id) {
                self.send_inventory_update(id, entry);
            }
            let armor = template.is_armor();
            let real_element = if armor { opposite_element(element) } else { element };
            let name = item_name_param(templates.as_deref(), item.item_id);
            let mut sm;
            if item.enchant_level > 0 {
                sm = SystemMessage::new(if armor {
                    SystemMessageId::S1S2SS3AttributeWasRemovedSoResistanceToS4WasDecreased
                } else {
                    SystemMessageId::S1S2SS3AttributeHasBeenRemoved
                })
                .with(SystemMessageParam::IntNumber(item.enchant_level))
                .with(name)
                .with(SystemMessageParam::ElementName(real_element));
            } else {
                sm = SystemMessage::new(if armor {
                    SystemMessageId::S1SS2AttributeWasRemovedAndResistanceToS3WasDecreased
                } else {
                    SystemMessageId::S1SS2AttributeHasBeenRemoved
                })
                .with(name)
                .with(SystemMessageParam::ElementName(real_element));
            }
            if armor {
                sm = sm.with(SystemMessageParam::ElementName(opposite_element(real_element)));
            }
            self.send(id, &sm);
            self.send(
                id,
                &ExBaseAttributeCancelResult {
                    object_id: item.object_id.0,
                    attribute: element,
                },
            );
        } else {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouDoNotHaveEnoughFundsToCancelThisAttribute),
            );
        }
    }
}
