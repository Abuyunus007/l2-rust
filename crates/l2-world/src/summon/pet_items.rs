//! The pet's inventory and what the player does with it: the five pet packets, the pickup of the
//! pet, the pet food, and what the pet wears.
// Java: entity/itemcontainer/{PetInventory,Inventory,ItemContainer}.java
// Java: entity/actor/instance/Pet.java (transferItem, doPickupItem, destroyItem, dropAllItems,
//       setName)
// Java: entity/actor/Player.java (transferItem, validateItemManipulation)
// Java: network/clientpackets/{RequestPetGetItem,RequestGiveItemToPet,RequestGetItemFromPet,
//       RequestPetUseItem,RequestChangePetName}.java
// Java: dist/game/data/scripts/handlers/items/PetFood.java
// Java: data/sql/PetNameTable.java
//
// # Rows
// The pet's items are rows of the owner's `items` (`PetInventory.getOwnerId()` is the owner), so
// moving an item between the owner and the pet is the same `UPDATE items SET loc = ..` Java does
// (`ItemChanges` of the one owner, written at once like `Item.updateDatabase()`).
//
// # Left out
// - The flood protectors (`canPerformTransaction`, `canUseItem`), the enchant cancel of the two
//   item moves (enchanting is not ported), `MercTicketManager` and `FortSiegeManager.isCombat`.
// - `PunishmentManager.handleIllegalPlayerAction` for a wrong amount: logged, nothing else.
// - The pet using a non-food item through its own cast (`ItemSkillsTemplate` with a pet): the
//   shared handler is called with the pet, see `inventory/handlers/item_skills.rs`.

use l2_data::item::BodyPart;
use l2_data::item::ItemTemplate;
use l2_data::item::types::EtcItemType;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    ActionFailed, GetItem, InventoryUpdate, MagicSkillUse, PetInventoryUpdate, SkillPosition, StatusUpdate,
    StopMove, SystemMessage, SystemMessageParam,
};
use l2_util::ObjectId;
use tracing::warn;

use crate::inventory::handlers::ItemHandlerKind;
use crate::inventory::item_name_param;
use crate::player::ItemData;
use crate::stats::calculators::FuncOwner;
use crate::world::World;

/// `ItemLocation.PET`.
pub const LOC_PET: &str = "PET";
/// `ItemLocation.PET_EQUIP`.
pub const LOC_PET_EQUIP: &str = "PET_EQUIP";
/// `ItemLocation.INVENTORY` / `PAPERDOLL`.
const LOC_INVENTORY: &str = "INVENTORY";
const LOC_PAPERDOLL: &str = "PAPERDOLL";
/// `PetNameTable`/`RequestChangePetName`: "Pet name limit 1-8 characters."
const MAX_PET_NAME: usize = 8;

/// The packets about the pet (`RequestPetGetItem`, ...), as the session hands them over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PetInput {
    /// Java `RequestPetGetItem`: the pet picks up the ground item.
    GetItem { object_id: i32 },
    /// Java `RequestGiveItemToPet`.
    GiveItemToPet { object_id: i32, amount: i64 },
    /// Java `RequestGetItemFromPet` (the unknown int is not read by Java).
    GetItemFromPet { object_id: i32, amount: i64 },
    /// Java `RequestPetUseItem`.
    UseItem { object_id: i32 },
    /// Java `RequestChangePetName`.
    ChangeName { name: String },
}

fn in_pet(item: &ItemData) -> bool {
    item.loc == LOC_PET || item.loc == LOC_PET_EQUIP
}

fn in_inventory(item: &ItemData) -> bool {
    item.loc == LOC_INVENTORY || item.loc == LOC_PAPERDOLL
}

/// An item that was added to the pet: its state now and the change flag the client is told.
#[derive(Debug, Clone, PartialEq)]
struct AddedPetItem {
    item: ItemData,
    change: i16,
}

/// What `transfer` did: the item the source had (with its count after), and the item in the target.
#[derive(Debug, Clone, PartialEq)]
struct Moved {
    old: ItemData,
    new: ItemData,
    /// `oldItem == newItem`: the item object itself changed container.
    moved_whole: bool,
}

impl World {
    // ------------------------------------------------------------------ the container

    /// The items of the owner's pet inventory (`PetInventory.getItems()`).
    pub(crate) fn pet_items(&self, owner: ObjectId) -> impl Iterator<Item = &ItemData> {
        self.players
            .get(&owner)
            .into_iter()
            .flat_map(|p| p.data.items.iter())
            .filter(|i| in_pet(i))
    }

    /// Java `PetInventory.getItemByObjectId(id)`.
    pub(crate) fn pet_item(&self, owner: ObjectId, object_id: ObjectId) -> Option<&ItemData> {
        self.pet_items(owner).find(|i| i.object_id == object_id)
    }

    /// Java `PetInventory.getItemByItemId(id)`: the first row.
    pub(crate) fn pet_item_by_item_id(&self, owner: ObjectId, item_id: i32) -> Option<&ItemData> {
        self.pet_items(owner).find(|i| i.item_id == item_id)
    }

    /// Java `PetInventory.getSize()`.
    pub(crate) fn pet_item_count(&self, owner: ObjectId) -> usize {
        self.pet_items(owner).count()
    }

    /// Java `PetInventory.getTotalWeight()`.
    pub fn pet_total_weight(&self, owner: ObjectId) -> i32 {
        let total: i64 = self
            .pet_items(owner)
            .map(|i| {
                self.data
                    .item(i.item_id)
                    .map_or(0, |s| i64::from(s.weight).saturating_mul(i.count))
            })
            .sum();
        // `_totalWeight = (int) Math.min(weight, Integer.MAX_VALUE)`.
        i32::try_from(total).unwrap_or(i32::MAX)
    }

    /// Java `PetInventory.validateCapacity(item)`: one slot, none for a stack the pet has or a herb.
    fn pet_validate_capacity(&self, owner: ObjectId, template: &ItemTemplate) -> bool {
        let slot = (!template.stackable || self.pet_item_by_item_id(owner, template.id).is_none())
            && !template.ex_immediate_effect;
        let slots = usize::from(slot);
        let limit = usize::try_from(self.summons.pet_world.settings.inventory_maximum_pet).unwrap_or(0);
        self.pet_item_count(owner) + slots <= limit
    }

    /// Java `PetInventory.validateWeight(item, count)`.
    fn pet_validate_weight(
        &self,
        pet: ObjectId,
        owner: ObjectId,
        template: &ItemTemplate,
        count: i64,
    ) -> bool {
        let weight = (count * i64::from(template.weight)) as i32;
        let max_load = self.stats(pet).map_or(0, |s| s.max_load());
        i64::from(self.pet_total_weight(owner)) + i64::from(weight) <= i64::from(max_load)
    }

    /// `Pet.updateAndBroadcastStatus(1)` after the weight changed (`PetInventory.refreshWeight`):
    /// the overload skill, then the pet's info.
    fn pet_weight_changed(&mut self, pet: ObjectId) {
        self.pet_refresh_overloaded(pet);
        self.update_and_broadcast_summon_status(pet, 1);
    }

    fn pet_update_row(&mut self, owner: ObjectId, object_id: ObjectId) {
        self.update_item_in_db(owner, object_id);
    }

    fn pet_item_mut(&mut self, owner: ObjectId, object_id: ObjectId) -> Option<&mut ItemData> {
        self.players
            .get_mut(&owner)?
            .data
            .items
            .iter_mut()
            .find(|i| i.object_id == object_id)
    }

    /// `Item.changeCount(process, count, ...)` of an item of the owner: never below 0.
    fn pet_change_count(&mut self, owner: ObjectId, object_id: ObjectId, count: i64) -> Option<ItemData> {
        let item = self.pet_item_mut(owner, object_id)?;
        item.count = item.count.wrapping_add(count).max(0);
        Some(item.clone())
    }

    // ------------------------------------------------------------------ moving items

    /// Java `ItemContainer.transferItem(process, objectId, count, target, actor, reference)`
    /// between the owner's inventory and its pet's. `to_pet` picks the direction.
    fn pet_transfer(
        &mut self,
        owner: ObjectId,
        object_id: ObjectId,
        count: i64,
        to_pet: bool,
    ) -> Option<Moved> {
        let source = self
            .players
            .get(&owner)?
            .data
            .items
            .iter()
            .find(|i| i.object_id == object_id && if to_pet { in_inventory(i) } else { in_pet(i) })?
            .clone();
        let stackable = self.data.item_stackable(source.item_id);
        let target_item = if stackable {
            self.players
                .get(&owner)?
                .data
                .items
                .iter()
                .find(|i| i.item_id == source.item_id && if to_pet { in_pet(i) } else { in_inventory(i) })
                .cloned()
        } else {
            None
        };
        // Check if requested quantity is available.
        let count = count.min(source.count);

        let (old, new, moved_whole);
        // If possible, move entire item object.
        if source.count == count && target_item.is_none() && !stackable {
            if to_pet {
                // `removeItem` of the player's inventory: the shortcut goes, a worn item comes off.
                let removed = self.remove_inventory_item(owner, object_id)?;
                let mut moved = removed;
                moved.loc = LOC_PET.to_string();
                moved.loc_data = 0;
                self.players.get_mut(&owner)?.data.items.push(moved.clone());
                (old, new) = (moved.clone(), moved);
            } else {
                let moved = self.pet_unequip_to_inventory_row(owner, object_id)?;
                (old, new) = (moved.clone(), moved);
            }
            moved_whole = true;
        } else {
            moved_whole = false;
            if source.count > count {
                // If possible, only update counts.
                old = self.pet_change_count(owner, object_id, -count)?;
            } else {
                // Otherwise destroy old item.
                if to_pet {
                    self.remove_inventory_item(owner, object_id)?;
                } else {
                    let index = self
                        .players
                        .get(&owner)?
                        .data
                        .items
                        .iter()
                        .position(|i| i.object_id == object_id)?;
                    self.players.get_mut(&owner)?.data.items.remove(index);
                }
                self.remove_item_from_db(owner, object_id);
                self.release_item_id(object_id);
                let mut gone = source.clone();
                gone.count = 0;
                old = gone;
            }
            new = if let Some(existing) = target_item {
                // If possible, only update counts.
                self.pet_change_count(owner, existing.object_id, count)?
            } else {
                // Otherwise add new item.
                let templates = self.item_templates()?;
                let template = templates.get(source.item_id)?;
                let id = self.allocate_item_id()?;
                let mut data = self.new_item_data(template, id, if template.stackable { count } else { 1 });
                data.loc = if to_pet { LOC_PET } else { LOC_INVENTORY }.to_string();
                data.loc_data = 0;
                data.enchant_level = template.default_enchant_level;
                self.players.get_mut(&owner)?.data.items.push(data.clone());
                data
            };
        }

        // Updates database.
        if moved_whole {
            self.pet_update_row(owner, new.object_id);
        } else {
            if old.count > 0 {
                self.pet_update_row(owner, object_id);
            }
            self.pet_update_row(owner, new.object_id);
        }
        self.refresh_weight(owner);
        Some(Moved {
            old,
            new,
            moved_whole,
        })
    }

    /// A pet item that moves to the owner's inventory: a worn item comes off first
    /// (`PetInventory.removeItem` -> `unEquipItemInSlot`); the row is an inventory row now.
    fn pet_unequip_to_inventory_row(&mut self, owner: ObjectId, object_id: ObjectId) -> Option<ItemData> {
        if let Some(pet) = self.summon_of(owner).filter(|s| self.is_pet(*s)) {
            let slot = self
                .pet_item(owner, object_id)
                .filter(|i| i.loc == LOC_PET_EQUIP)
                .map(|i| i.loc_data);
            if let Some(slot) = slot.and_then(|s| usize::try_from(s).ok()) {
                self.pet_set_paperdoll(owner, pet, slot, None);
            }
        }
        let item = self.pet_item_mut(owner, object_id)?;
        item.loc = LOC_INVENTORY.to_string();
        item.loc_data = 0;
        Some(item.clone())
    }

    /// Java `PetInventory.transferItemsToOwner()`: every item goes back to the owner.
    pub(crate) fn pet_transfer_items_to_owner(&mut self, owner: ObjectId) {
        let ids: Vec<ObjectId> = self.pet_items(owner).map(|i| i.object_id).collect();
        for object_id in ids {
            let Some(count) = self.pet_item(owner, object_id).map(|i| i.count) else {
                continue;
            };
            // `Pet.transferItem(.., owner.getInventory(), owner, pet)`.
            self.pet_transfer_to_owner(owner, object_id, count);
        }
    }

    /// Java `Pet.transferItem(process, objectId, count, ownerInventory, actor, reference)`: the
    /// move and what the owner hears about it.
    fn pet_transfer_to_owner(
        &mut self,
        owner: ObjectId,
        object_id: ObjectId,
        count: i64,
    ) -> Option<ItemData> {
        let old_item = self.pet_item(owner, object_id)?.clone();
        let player_had = self
            .players
            .get(&owner)?
            .data
            .items
            .iter()
            .any(|i| i.item_id == old_item.item_id && in_inventory(i));
        let moved = self.pet_transfer(owner, object_id, count, false)?;
        // Send inventory update packet.
        let pet_entry = if moved.old.count > 0 && !moved.moved_whole {
            self.update_entry(&moved.old, InventoryUpdate::MODIFIED)
        } else {
            self.update_entry(&moved.old, InventoryUpdate::REMOVED)
        };
        self.send(
            owner,
            &PetInventoryUpdate {
                updates: vec![pet_entry],
            },
        );
        // Send target update packet.
        let stackable = self.data.item_stackable(moved.new.item_id);
        if !stackable {
            let entry = self.update_entry(&moved.new, InventoryUpdate::ADDED);
            self.send_inventory_update(owner, entry);
        } else if player_had && stackable {
            let entry = self.update_entry(&moved.new, InventoryUpdate::MODIFIED);
            self.send_inventory_update(owner, entry);
        }
        if let Some(pet) = self.summon_of(owner).filter(|s| self.is_pet(*s)) {
            self.pet_weight_changed(pet);
        }
        Some(moved.new)
    }

    // ------------------------------------------------------------------ the packets

    /// Dispatches the pet packets of a player.
    pub(crate) fn on_pet_input(&mut self, player: ObjectId, input: PetInput) {
        if !self.players.contains_key(&player) {
            return;
        }
        match input {
            PetInput::GetItem { object_id } => self.on_request_pet_get_item(player, ObjectId(object_id)),
            PetInput::GiveItemToPet { object_id, amount } => {
                self.on_request_give_item_to_pet(player, ObjectId(object_id), amount)
            }
            PetInput::GetItemFromPet { object_id, amount } => {
                self.on_request_get_item_from_pet(player, ObjectId(object_id), amount)
            }
            PetInput::UseItem { object_id } => self.on_request_pet_use_item(player, ObjectId(object_id)),
            PetInput::ChangeName { name } => self.on_request_change_pet_name(player, &name),
        }
    }

    /// Java `RequestPetGetItem.runImpl`.
    fn on_request_pet_get_item(&mut self, player: ObjectId, object_id: ObjectId) {
        let Some(pet) = self.summon_of(player).filter(|s| self.is_pet(*s)) else {
            self.send(player, &ActionFailed);
            return;
        };
        if !self.ground.items.contains_key(&object_id) {
            self.send(player, &ActionFailed);
            return;
        }
        // `MercTicketManager.getTicketCastleId` and `FortSiegeManager.isCombat`: not ported.
        if self.is_dead(pet) || self.is_out_of_control(pet) {
            self.send(player, &ActionFailed);
            return;
        }
        if self.pet_is_uncontrollable(pet) {
            // NOTE(java-quirk): Java sends YOU_ARE_NOT_ALLOWED_TO_ENTER_THE_PARTY_ROOM here.
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouAreNotAllowedToEnterThePartyRoom),
            );
            return;
        }
        self.pet_set_intention_pick_up(pet, object_id);
    }

    /// Java `CreatureAI.setIntentionPickUp(object)` of the pet's `SummonAI`.
    fn pet_set_intention_pick_up(&mut self, pet: ObjectId, object: ObjectId) {
        self.stop_follow(pet);
        if self.intention_of(pet) == crate::movement::Intention::Rest
            || self.is_all_skills_disabled(pet)
            || self.is_casting_now(pet)
        {
            self.client_action_failed(pet);
            return;
        }
        // Stop the actor auto-attack client side by sending Server->Client packet AutoAttackStop.
        self.client_stop_auto_attack(pet);
        // `object.isItem() && getItemLocation() != VOID`: an item on the ground is always `VOID`.
        self.set_intention_value(pet, crate::movement::Intention::PickUp);
        self.set_ai_target(pet, Some(object));
        // NOTE(java-quirk): an object at 0,0 is moved next to the actor ("using a temporary fix").
        if let (Some(me), Some((0, 0, _))) = (self.position_of(pet), self.position_of(object))
            && let Some(item) = self.ground_item_mut(object)
        {
            (item.x, item.y, item.z) = (me.0, me.1, me.2 + 5);
        }
        // Move the actor to Pawn server side AND client side.
        self.move_to_pawn(pet, object, 20);
    }

    /// Java `Pet.doPickupItem(object)`.
    pub(crate) fn pet_do_pickup_item(&mut self, pet: ObjectId, object: ObjectId) {
        if self.is_dead(pet) {
            return;
        }
        let Some(owner) = self.summon_owner(pet) else {
            return;
        };
        self.summon_set_intention_idle(pet);
        if let Some((x, y, z)) = self.position_of(pet) {
            let heading = self.heading_of(pet);
            self.broadcast_packet(
                pet,
                &StopMove {
                    object_id: pet.0,
                    x,
                    y,
                    z,
                    heading,
                },
            );
        }
        if self.objects.get(&object).map(|e| e.kind) != Some(crate::objects::ObjectKind::Item) {
            // Do not try to pickup anything that is not an item :)
            warn!("{pet} trying to pickup wrong target.{object}");
            self.send(owner, &ActionFailed);
            return;
        }
        // Cursed weapons and combat flags: not modelled.
        let follow = self.follow_status(pet);
        let templates = self.item_templates();
        let templates = templates.as_deref();
        // Check if the target to pick up is visible.
        let Some(target) = self
            .ground
            .items
            .get(&object)
            .filter(|_| self.objects.get(&object).is_some_and(|e| e.spawned))
            .cloned()
        else {
            self.send(owner, &ActionFailed);
            return;
        };
        let item_id = target.data.item_id;
        let template = templates.and_then(|t| t.get(item_id));
        if !self.try_pick_up(&target, owner) {
            self.send(owner, &ActionFailed);
            self.send(
                owner,
                &SystemMessage::new(SystemMessageId::YouHaveFailedToPickUpS1)
                    .with(item_name_param(templates, item_id)),
            );
            return;
        }
        let party_keepers = self.party_of(owner).is_none_or(|party| {
            self.party_distribution_type(party)
                == Some(l2_protocol_game::server::PartyDistributionType::FindersKeepers)
        });
        if party_keepers
            && let Some(template) = template
            && !self.pet_validate_capacity(owner, template)
        {
            self.send(owner, &ActionFailed);
            self.send(
                owner,
                &SystemMessage::new(SystemMessageId::YourPetCannotCarryAnyMoreItems),
            );
            return;
        }
        if target
            .owner
            .is_some_and(|o| o != owner && !self.is_in_looter_party(owner, o))
        {
            let message = if item_id == l2_data::item::ADENA_ID {
                SystemMessage::new(SystemMessageId::YouHaveFailedToPickUpS1Adena)
                    .with(SystemMessageParam::LongNumber(target.data.count))
            } else if target.data.count > 1 {
                SystemMessage::new(SystemMessageId::YouHaveFailedToPickUpS2S1S)
                    .with(item_name_param(templates, item_id))
                    .with(SystemMessageParam::LongNumber(target.data.count))
            } else {
                SystemMessage::new(SystemMessageId::YouHaveFailedToPickUpS1)
                    .with(item_name_param(templates, item_id))
            };
            self.send(owner, &ActionFailed);
            self.send(owner, &message);
            return;
        }
        // Remove the Item from the ground: `pickupMe(this)` and the saved row.
        let (x, y, z) = (target.x, target.y, target.z);
        self.broadcast_packet(
            pet,
            &GetItem {
                player_id: pet.0,
                object_id: object.0,
                x,
                y,
                z,
            },
        );
        let Some(taken) = self.take_ground_item(object) else {
            return;
        };

        let herb = template.is_some_and(|t| t.ex_immediate_effect);
        if herb {
            // Herbs.
            match template.and_then(ItemHandlerKind::of_template) {
                Some(_) => {
                    self.pet_item_handler_use(pet, &taken.data);
                }
                None => warn!("No item handler registered for item ID: {item_id}."),
            }
            // `ItemManager.destroyItem(NONE, target, getOwner(), null)`.
            self.release_item_id(taken.data.object_id);
            self.broadcast_status_update(pet);
        } else {
            let message = if item_id == l2_data::item::ADENA_ID {
                SystemMessage::new(SystemMessageId::YourPetPickedUpS1Adena)
                    .with(SystemMessageParam::LongNumber(taken.data.count))
            } else if taken.data.enchant_level > 0 {
                SystemMessage::new(SystemMessageId::YourPetPickedUpS1S2)
                    .with(SystemMessageParam::IntNumber(taken.data.enchant_level))
                    .with(item_name_param(templates, item_id))
            } else if taken.data.count > 1 {
                SystemMessage::new(SystemMessageId::YourPetPickedUpS2S1S)
                    .with(SystemMessageParam::LongNumber(taken.data.count))
                    .with(item_name_param(templates, item_id))
            } else {
                SystemMessage::new(SystemMessageId::YourPetPickedUpS1)
                    .with(item_name_param(templates, item_id))
            };
            self.send(owner, &message);

            // If owner is in party and it is not finders keepers, distribute the item instead of
            // stealing it -.-
            if self.is_in_party(owner) && !party_keepers {
                self.party_distribute_ground_item(owner, taken.data);
            } else if let Some(item) = self.pet_add_item_object(owner, taken.data) {
                let entry = self.update_entry(&item.item, item.change);
                self.send(owner, &PetInventoryUpdate { updates: vec![entry] });
                self.pet_weight_changed(pet);
            }
        }

        self.summon_set_intention_idle(pet);
        if follow {
            self.set_follow_status(pet, true);
        }
    }

    /// Java `Inventory.addItem(process, item, actor, reference)` of the pet's inventory for an
    /// item that exists already: a stack joins the one the pet has, else the item becomes a row.
    fn pet_add_item_object(&mut self, owner: ObjectId, mut item: ItemData) -> Option<AddedPetItem> {
        let old = self.pet_item_by_item_id(owner, item.item_id).cloned();
        match old {
            // If stackable item is found in inventory just add to current quantity.
            Some(old) if self.data.item_stackable(old.item_id) => {
                let merged = self.pet_change_count(owner, old.object_id, item.count)?;
                // And destroys the item.
                self.release_item_id(item.object_id);
                self.pet_update_row(owner, merged.object_id);
                Some(AddedPetItem {
                    item: merged,
                    change: InventoryUpdate::MODIFIED,
                })
            }
            // If item hasn't be found in inventory, create new one.
            _ => {
                item.loc = LOC_PET.to_string();
                item.loc_data = 0;
                self.players.get_mut(&owner)?.data.items.push(item.clone());
                self.pet_update_row(owner, item.object_id);
                Some(AddedPetItem {
                    item,
                    change: InventoryUpdate::ADDED,
                })
            }
        }
    }

    /// Java `RequestGiveItemToPet.runImpl`.
    fn on_request_give_item_to_pet(&mut self, player: ObjectId, object_id: ObjectId, amount: i64) {
        let Some(pet) = self.summon_of(player).filter(|s| self.is_pet(*s)) else {
            return;
        };
        if amount <= 0 {
            return;
        }
        // The flood protector and the enchant cancel: not ported.
        // Alt game - Karma punishment.
        if !self.settings.inventory.alt_game_karma_player_can_trade
            && self.players.get(&player).is_some_and(|p| p.data.karma > 0)
        {
            return;
        }
        if self.is_in_store_mode(player) {
            self.send_message(player, "You cannot give items while trading.");
            return;
        }
        let Some(item) = self.inventory_item(player, object_id).cloned() else {
            return;
        };
        if amount > item.count {
            warn!(
                "RequestGiveItemToPet: {player} tried to give item with oid {object_id} to pet but has invalid count {amount} item count: {}",
                item.count
            );
            return;
        }
        if item.is_augmented() {
            return;
        }
        let templates = self.item_templates();
        let Some(template) = templates.as_deref().and_then(|t| t.get(item.item_id)) else {
            return;
        };
        // `isHeroItem()` is not a template field here (heroes are M8).
        if !template.dropable || !template.destroyable || !self.item_is_tradeable(&item, template) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YourPetCannotCarryThisItem),
            );
            return;
        }
        if self.is_dead(pet) {
            self.send(
                player,
                &SystemMessage::new(
                    SystemMessageId::YourPetIsDeadAndAnyAttemptYouMakeToGiveItSomethingGoesUnrecognized,
                ),
            );
            return;
        }
        if !self.pet_validate_capacity(player, template) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YourPetCannotCarryAnyMoreItems),
            );
            return;
        }
        if !self.pet_validate_weight(pet, player, template, amount) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YourPetCannotCarryAnyMoreItems),
            );
            return;
        }
        // `player.transferItem(TRANSFER, objectId, amount, pet.getInventory(), pet)`.
        if self
            .player_transfer_to_pet(player, pet, object_id, amount)
            .is_none()
        {
            warn!("Invalid item transfer request: {pet}(pet) --> {player}");
        }
    }

    /// Java `Player.transferItem(.., pet.getInventory(), pet)`.
    fn player_transfer_to_pet(
        &mut self,
        player: ObjectId,
        pet: ObjectId,
        object_id: ObjectId,
        count: i64,
    ) -> Option<ItemData> {
        let old_item = self.check_item_manipulation(player, object_id, count)?;
        let moved = self.pet_transfer(player, object_id, count, true)?;
        // Send inventory update packet.
        let entry = if old_item.count > 0 && !moved.moved_whole && moved.old.count > 0 {
            self.update_entry(&moved.old, InventoryUpdate::MODIFIED)
        } else {
            self.update_entry(&moved.old, InventoryUpdate::REMOVED)
        };
        self.send_inventory_update(player, entry);
        // Update current load as well.
        let mut su = StatusUpdate::new(player.0);
        su.add_attribute(
            l2_protocol_game::server::visibility::status::CUR_LOAD,
            self.stats(player).map_or(0, |v| v.current_load()),
        );
        self.send(player, &su);
        // Send target update packet.
        let change = if moved.new.count > count {
            InventoryUpdate::MODIFIED
        } else {
            InventoryUpdate::ADDED
        };
        let entry = self.update_entry(&moved.new, change);
        self.send(pet, &PetInventoryUpdate { updates: vec![entry] });
        self.pet_weight_changed(pet);
        Some(moved.new)
    }

    /// Java `RequestGetItemFromPet.runImpl`.
    fn on_request_get_item_from_pet(&mut self, player: ObjectId, object_id: ObjectId, amount: i64) {
        let Some(_pet) = self.summon_of(player).filter(|s| self.is_pet(*s)) else {
            return;
        };
        if amount <= 0 {
            return;
        }
        let Some(item) = self.pet_item(player, object_id).cloned() else {
            return;
        };
        if amount > item.count {
            warn!(
                "RequestGetItemFromPet: {player} tried to get item with oid {object_id} from pet but has invalid count {amount} item count: {}",
                item.count
            );
            return;
        }
        if self.pet_transfer_to_owner(player, object_id, amount).is_none() {
            warn!("Invalid item transfer request: pet --> {player}");
        }
    }

    /// Java `RequestPetUseItem.runImpl`.
    fn on_request_pet_use_item(&mut self, player: ObjectId, object_id: ObjectId) {
        let Some(pet) = self.summon_of(player).filter(|s| self.is_pet(*s)) else {
            return;
        };
        let Some(item) = self.pet_item(player, object_id).cloned() else {
            return;
        };
        let templates = self.item_templates();
        let Some(template) = templates.as_deref().and_then(|t| t.get(item.item_id)) else {
            return;
        };
        if !template.for_npc {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::ThisPetCannotUseThisItem),
            );
            return;
        }
        if self.is_alike_dead(player) || self.is_dead(pet) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::S1CannotBeUsedDueToUnsuitableTerms)
                    .with(item_name_param(templates.as_deref(), item.item_id)),
            );
            return;
        }
        // If the item has reuse time and it has not passed.
        if template.reuse_delay > 0 && self.item_remaining_reuse_time(pet, item.object_id) > 0 {
            return;
        }
        if item.loc != LOC_PET_EQUIP && !self.check_item_condition(pet, template, true) {
            return;
        }
        self.pet_use_item(player, pet, &item, template);
    }

    /// `RequestPetUseItem.useItem(pet, item, player)`.
    fn pet_use_item(&mut self, player: ObjectId, pet: ObjectId, item: &ItemData, template: &ItemTemplate) {
        let templates = self.item_templates();
        if crate::inventory::is_equipable(template) {
            if !template.is_condition_attached() {
                self.send(
                    player,
                    &SystemMessage::new(SystemMessageId::ThisPetCannotUseThisItem),
                );
                return;
            }
            if item.loc == LOC_PET_EQUIP {
                if let Ok(slot) = usize::try_from(item.loc_data) {
                    self.pet_set_paperdoll(player, pet, slot, None);
                }
                self.send(
                    player,
                    &SystemMessage::new(SystemMessageId::YourPetTookOffS1)
                        .with(item_name_param(templates.as_deref(), item.item_id)),
                );
            } else {
                self.pet_equip_item(player, pet, item, template);
                self.send(
                    player,
                    &SystemMessage::new(SystemMessageId::YourPetPutOnS1)
                        .with(item_name_param(templates.as_deref(), item.item_id)),
                );
            }
            let list = self.pet_item_list(player);
            self.send(player, &list);
            self.update_and_broadcast_summon_status(pet, 1);
        } else if ItemHandlerKind::of_template(template).is_some() {
            if self.pet_item_handler_use(pet, item) {
                if template.reuse_delay > 0 {
                    // `player.addTimeStampItem(item, reuseDelay)`.
                    self.add_time_stamp_item(player, item, template, i64::from(template.reuse_delay));
                }
                self.update_and_broadcast_summon_status(pet, 1);
            }
        } else {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::ThisPetCannotUseThisItem),
            );
            warn!("No item handler registered for itemId: {}", item.item_id);
        }
    }

    /// `ItemHandler.getHandler(item.getEtcItem())` exists for the item.
    pub(crate) fn item_has_handler(&self, item_id: i32) -> bool {
        self.item_templates()
            .and_then(|t| t.get(item_id).and_then(ItemHandlerKind::of_template))
            .is_some()
    }

    /// `handler.onItemUse(pet, item, false)`: `true` when the item was used.
    pub(crate) fn pet_item_handler_use(&mut self, pet: ObjectId, item: &ItemData) -> bool {
        let Some(templates) = self.item_templates() else {
            return false;
        };
        let Some(template) = templates.get(item.item_id) else {
            return false;
        };
        match ItemHandlerKind::of_template(template) {
            Some(kind) => self.run_item_handler(kind, pet, item, template, false),
            None => false,
        }
    }

    /// Java `ItemContainer.addItem(process, itemId, count, actor, reference)` of the pet's
    /// inventory: a stack the pet has grows, else new items are made. The last item.
    fn pet_add_item_by_id(&mut self, owner: ObjectId, item_id: i32, count: i64) -> Option<ItemData> {
        let templates = self.item_templates()?;
        if let Some(existing) = self.pet_item_by_item_id(owner, item_id).cloned()
            && self.data.item_stackable(item_id)
        {
            let merged = self.pet_change_count(owner, existing.object_id, count)?;
            self.pet_update_row(owner, merged.object_id);
            return Some(merged);
        }
        let Some(template) = templates.get(item_id) else {
            warn!("Invalid ItemId ({item_id}) requested by {owner}");
            return None;
        };
        let mut last = None;
        for _ in 0..count.max(0) {
            let id = self.allocate_item_id()?;
            let mut data = self.new_item_data(template, id, if template.stackable { count } else { 1 });
            data.loc = LOC_PET.to_string();
            data.loc_data = 0;
            data.enchant_level = template.default_enchant_level;
            self.players.get_mut(&owner)?.data.items.push(data.clone());
            self.pet_update_row(owner, id);
            last = Some(data);
            // If stackable, end loop as entire count is included in 1 instance of item.
            if template.stackable || !self.settings.ground.multiple_item_drop {
                break;
            }
        }
        last
    }

    /// The pet part of the `Restoration` effect: the item goes into the pet's inventory, with the
    /// enchant level, and the owner gets the list.
    pub(crate) fn pet_restoration_item(
        &mut self,
        pet: ObjectId,
        item_id: i32,
        count: i64,
        enchant_level: i32,
    ) {
        let Some(owner) = self.summon_owner(pet) else {
            return;
        };
        let Some(item) = self.pet_add_item_by_id(owner, item_id, count) else {
            return;
        };
        if enchant_level > 0 {
            if let Some(row) = self.pet_item_mut(owner, item.object_id) {
                row.enchant_level = enchant_level;
            }
        }
        self.pet_weight_changed(pet);
        let list = self.pet_item_list(owner);
        self.send(owner, &list);
    }

    // ------------------------------------------------------------------ pet food

    /// Java `PetFood.onItemUse(playable, item, forceUse)`: a pet eats, a player (no mounts here)
    /// is told the item cannot be used.
    pub(crate) fn pet_food_on_use(&mut self, id: ObjectId, item: &ItemData, template: &ItemTemplate) -> bool {
        let is_pet = self.is_pet(id);
        if is_pet && !self.pet_data_of(id).is_some_and(|d| d.can_eat(item.item_id)) {
            self.send_to_summon_owner(id, &SystemMessage::new(SystemMessageId::ThisPetCannotUseThisItem));
            return false;
        }
        let skills = template.skills.item_skills.clone().unwrap_or_default();
        for skill_ref in skills {
            self.pet_use_food(id, skill_ref.id, skill_ref.level, item);
        }
        true
    }

    /// `PetFood.useFood(activeChar, skillId, skillLevel, item)`.
    fn pet_use_food(&mut self, id: ObjectId, skill_id: i32, skill_level: i32, item: &ItemData) -> bool {
        let Some(table) = self.skill_table() else {
            return false;
        };
        let Some(skill) = table.get(skill_id, skill_level) else {
            return false;
        };
        let feed = skill.feed;
        if self.is_pet(id) {
            let Some(owner) = self.summon_owner(id) else {
                return false;
            };
            if self.pet_destroy_item(owner, item.object_id, 1, false) {
                if let Some((x, y, z)) = self.position_of(id) {
                    let here = SkillPosition { x, y, z };
                    self.broadcast_packet(
                        id,
                        &MagicSkillUse {
                            creature_id: id.0,
                            target_id: id.0,
                            skill_id,
                            skill_level,
                            hit_time: 0,
                            reuse_delay: 0,
                            creature: here,
                            ground_location: None,
                            target: here,
                        },
                    );
                }
                let rate = self.summons.pet_world.settings.pet_food_rate;
                let fed = self.pet_current_fed(id).wrapping_add(feed.wrapping_mul(rate));
                self.pet_set_current_fed(id, fed);
                self.broadcast_status_update(id);
                if self.pet_is_hungry(id) {
                    self.send_to_summon_owner(
                        id,
                        &SystemMessage::new(SystemMessageId::YourPetAteALittleButIsStillHungry),
                    );
                }
                return true;
            }
        } else if self.is_player(id) {
            // The mount branch (`isMounted()`) needs mounts.
            let templates = self.item_templates();
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::S1CannotBeUsedDueToUnsuitableTerms)
                    .with(item_name_param(templates.as_deref(), item.item_id)),
            );
        }
        false
    }

    /// Java `Pet.destroyItem(process, objectId, count, reference, sendMessage)`: an item of the
    /// pet is used up, the owner sees the change.
    pub(crate) fn pet_destroy_item(
        &mut self,
        owner: ObjectId,
        object_id: ObjectId,
        count: i64,
        send_message: bool,
    ) -> bool {
        let Some(pet) = self.summon_of(owner).filter(|s| self.is_pet(*s)) else {
            return false;
        };
        let Some(item) = self.pet_item(owner, object_id).cloned() else {
            if send_message {
                self.send(owner, &SystemMessage::new(SystemMessageId::IncorrectItemCount2));
            }
            return false;
        };
        if item.count < count {
            if send_message {
                self.send(owner, &SystemMessage::new(SystemMessageId::IncorrectItemCount2));
            }
            return false;
        }
        // `PetInventory.destroyItem`: adjust the quantity, or remove the item.
        let after = if item.count > count {
            self.pet_change_count(owner, object_id, -count)
        } else {
            let index = self
                .players
                .get(&owner)
                .and_then(|p| p.data.items.iter().position(|i| i.object_id == object_id));
            if let Some(player) = self.players.get_mut(&owner)
                && let Some(index) = index
            {
                player.data.items.remove(index);
            }
            self.pet_remove_item_funcs(pet, &item);
            self.remove_item_from_db(owner, object_id);
            self.release_item_id(object_id);
            let mut gone = item.clone();
            gone.count = 0;
            Some(gone)
        };
        let Some(after) = after else {
            return false;
        };
        if item.count > count {
            self.pet_update_row(owner, object_id);
        }
        // Send Pet inventory update packet.
        let change = if after.count == 0 {
            InventoryUpdate::REMOVED
        } else {
            InventoryUpdate::MODIFIED
        };
        let entry = self.update_entry(&after, change);
        self.send(owner, &PetInventoryUpdate { updates: vec![entry] });
        self.pet_weight_changed(pet);
        if send_message {
            let templates = self.item_templates();
            let name = item_name_param(templates.as_deref(), item.item_id);
            let message = if count > 1 {
                SystemMessage::new(SystemMessageId::S2S1HasDisappeared)
                    .with(name)
                    .with(SystemMessageParam::LongNumber(count))
            } else {
                SystemMessage::new(SystemMessageId::S1HasDisappeared).with(name)
            };
            self.send(owner, &message);
        }
        true
    }

    // ------------------------------------------------------------------ equipment

    /// The stat functions of a worn pet item go (`StatsListener.notifyUnequiped`).
    fn pet_remove_item_funcs(&mut self, pet: ObjectId, item: &ItemData) {
        if item.loc != LOC_PET_EQUIP {
            return;
        }
        if let Some(npc) = self.npcs.get_mut(&pet) {
            npc.core.calculators.remove_owner(FuncOwner::Item(item.object_id));
        }
    }

    /// Java `Inventory.setPaperdollItem(slot, item)` of the pet: the item in the slot goes back to
    /// the pet's bag, the new one is worn (its functions join the pet's calculators). Returns the
    /// item that was there.
    pub(crate) fn pet_set_paperdoll(
        &mut self,
        owner: ObjectId,
        pet: ObjectId,
        slot: usize,
        item: Option<ObjectId>,
    ) -> Option<ObjectId> {
        let slot_value = i32::try_from(slot).ok()?;
        let old = self
            .pet_items(owner)
            .find(|i| i.loc == LOC_PET_EQUIP && i.loc_data == slot_value)
            .cloned();
        if old.as_ref().map(|o| o.object_id) == item {
            return item;
        }
        if let Some(old) = &old {
            // Put old item from paperdoll slot to base location.
            self.pet_remove_item_funcs(pet, old);
            if let Some(row) = self.pet_item_mut(owner, old.object_id) {
                row.loc = LOC_PET.to_string();
                row.loc_data = 0;
            }
            self.pet_update_row(owner, old.object_id);
        }
        if let Some(object_id) = item {
            if let Some(row) = self.pet_item_mut(owner, object_id) {
                row.loc = LOC_PET_EQUIP.to_string();
                row.loc_data = slot_value;
            }
            self.pet_add_item_funcs(pet, owner, object_id);
            self.pet_update_row(owner, object_id);
        }
        old.map(|o| o.object_id)
    }

    /// `StatsListener.notifyEquiped`: the functions of the item join the pet's.
    fn pet_add_item_funcs(&mut self, pet: ObjectId, owner: ObjectId, object_id: ObjectId) {
        let Some(item) = self.pet_item(owner, object_id).cloned() else {
            return;
        };
        let funcs = match self.stats(pet) {
            Some(view) => crate::stats::player_stat::item_funcs(&view, &item),
            None => return,
        };
        if let Some(npc) = self.npcs.get_mut(&pet) {
            npc.core.calculators.add_all(funcs);
        }
    }

    /// Java `Inventory.equipItem(item)` for the slots a pet uses.
    fn pet_equip_item(&mut self, owner: ObjectId, pet: ObjectId, item: &ItemData, template: &ItemTemplate) {
        use l2_protocol_game::server::charscreen::paperdoll as pd;
        let part = template.body_part;
        let worn = |world: &World, slot: usize| -> Option<ItemData> {
            let slot = i32::try_from(slot).ok()?;
            world
                .pet_items(owner)
                .find(|i| i.loc == LOC_PET_EQUIP && i.loc_data == slot)
                .cloned()
        };
        let object = item.object_id;
        // Check if the pet is using Formal Wear and the item isn't Wedding Bouquet.
        if item.item_id != 21163
            && let Some(formal) = worn(self, pd::CHEST)
            && self
                .item_templates()
                .and_then(|t| t.get(formal.item_id).map(|f| f.body_part))
                == Some(BodyPart::AllDress)
            && matches!(
                part,
                BodyPart::BothHands
                    | BodyPart::LeftHand
                    | BodyPart::RightHand
                    | BodyPart::Legs
                    | BodyPart::Feet
                    | BodyPart::Gloves
                    | BodyPart::Head
            )
        {
            return;
        }
        match part {
            BodyPart::BothHands => {
                self.pet_set_paperdoll(owner, pet, pd::LHAND, None);
                self.pet_set_paperdoll(owner, pet, pd::RHAND, Some(object));
            }
            BodyPart::LeftHand => {
                if let Some(rh) = worn(self, pd::RHAND) {
                    let two_handed = self
                        .item_templates()
                        .and_then(|t| t.get(rh.item_id).map(|r| r.body_part))
                        == Some(BodyPart::BothHands);
                    // The ammunition exceptions (bow/arrow, crossbow/bolt, rod/lure) are for
                    // players; a pet's left hand is never ammunition.
                    if two_handed && !self.pet_left_hand_is_ammo(&rh, template) {
                        self.pet_set_paperdoll(owner, pet, pd::RHAND, None);
                    }
                }
                self.pet_set_paperdoll(owner, pet, pd::LHAND, Some(object));
            }
            BodyPart::RightHand => {
                self.pet_set_paperdoll(owner, pet, pd::RHAND, Some(object));
            }
            BodyPart::FullArmor => {
                self.pet_set_paperdoll(owner, pet, pd::LEGS, None);
                self.pet_set_paperdoll(owner, pet, pd::CHEST, Some(object));
            }
            BodyPart::Legs => {
                if let Some(chest) = worn(self, pd::CHEST)
                    && self
                        .item_templates()
                        .and_then(|t| t.get(chest.item_id).map(|c| c.body_part))
                        == Some(BodyPart::FullArmor)
                {
                    self.pet_set_paperdoll(owner, pet, pd::CHEST, None);
                }
                self.pet_set_paperdoll(owner, pet, pd::LEGS, Some(object));
            }
            BodyPart::AllDress => {
                for slot in [pd::LEGS, pd::LHAND, pd::RHAND, pd::HEAD, pd::FEET, pd::GLOVES] {
                    self.pet_set_paperdoll(owner, pet, slot, None);
                }
                self.pet_set_paperdoll(owner, pet, pd::CHEST, Some(object));
            }
            other => match crate::inventory::paperdoll_index_of(other) {
                Some(slot) => {
                    self.pet_set_paperdoll(owner, pet, slot, Some(object));
                }
                None => warn!("Unknown body slot {other:?} for Item ID: {}", item.item_id),
            },
        }
    }

    /// `(rh.getItemType() == BOW && item.getItemType() == ARROW) || ...` of `equipItem`.
    fn pet_left_hand_is_ammo(&self, right_hand: &ItemData, left: &ItemTemplate) -> bool {
        use l2_data::item::types::WeaponType;
        let Some(templates) = self.item_templates() else {
            return false;
        };
        let weapon = templates
            .get(right_hand.item_id)
            .and_then(|t| t.weapon())
            .map(|w| w.weapon_type);
        let etc = left.etc().map(|e| e.etc_type);
        matches!(
            (weapon, etc),
            (Some(WeaponType::Bow), Some(EtcItemType::Arrow))
                | (Some(WeaponType::CrossBow), Some(EtcItemType::Bolt))
                | (Some(WeaponType::FishingRod), Some(EtcItemType::Lure))
        )
    }

    /// `PetInventory.restore()`: the worn items of the pet put their functions on, and the ones
    /// whose conditions no longer hold come off.
    pub(crate) fn pet_restore_equipment(&mut self, owner: ObjectId, pet: ObjectId) {
        let worn: Vec<(ObjectId, i32)> = self
            .pet_items(owner)
            .filter(|i| i.loc == LOC_PET_EQUIP)
            .map(|i| (i.object_id, i.loc_data))
            .collect();
        for (object_id, _) in &worn {
            self.pet_add_item_funcs(pet, owner, *object_id);
        }
        let templates = self.item_templates();
        // "check for equipped items from other pets"
        for (object_id, slot) in worn {
            let Some(item) = self.pet_item(owner, object_id).cloned() else {
                continue;
            };
            let Some(template) = templates.as_deref().and_then(|t| t.get(item.item_id)) else {
                continue;
            };
            if !self.check_item_condition(pet, template, false)
                && let Ok(slot) = usize::try_from(slot)
            {
                self.pet_set_paperdoll(owner, pet, slot, None);
            }
        }
    }

    /// Java `Pet.getWeapon()`: the id of the item in the right hand.
    pub(crate) fn pet_weapon_id(&self, pet: ObjectId) -> i32 {
        self.pet_paperdoll_item_id(pet, l2_protocol_game::server::charscreen::paperdoll::RHAND)
    }

    /// Java `Pet.getArmor()`: the id of the item on the chest.
    pub(crate) fn pet_armor_id(&self, pet: ObjectId) -> i32 {
        self.pet_paperdoll_item_id(pet, l2_protocol_game::server::charscreen::paperdoll::CHEST)
    }

    /// Java `Pet.getJewel()`: the id of the item on the neck.
    pub(crate) fn pet_jewel_id(&self, pet: ObjectId) -> i32 {
        self.pet_paperdoll_item_id(pet, l2_protocol_game::server::charscreen::paperdoll::NECK)
    }

    fn pet_paperdoll_item_id(&self, pet: ObjectId, slot: usize) -> i32 {
        let Some(owner) = self.summon_owner(pet) else {
            return 0;
        };
        let Ok(slot) = i32::try_from(slot) else {
            return 0;
        };
        self.pet_items(owner)
            .find(|i| i.loc == LOC_PET_EQUIP && i.loc_data == slot)
            .map_or(0, |i| i.item_id)
    }

    // ------------------------------------------------------------------ the name

    /// Java `RequestChangePetName.runImpl`.
    fn on_request_change_pet_name(&mut self, player: ObjectId, name: &str) {
        let Some(pet) = self.summon_of(player) else {
            return;
        };
        if !self.is_pet(pet) {
            self.send(player, &SystemMessage::new(SystemMessageId::YouDoNotHaveAPet));
            return;
        }
        if self.summon(pet).is_some_and(|s| !s.name.is_empty()) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouCannotSetTheNameOfThePet),
            );
            return;
        }
        let collar = self
            .summon_npc_id(pet)
            .and_then(|npc| self.pet_table()?.pet_item_by_npc(npc))
            .unwrap_or(0);
        if self
            .summons
            .pet_world
            .names
            .contains(&(name.to_lowercase(), collar))
        {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::ThisIsAlreadyInUseByAnotherPet),
            );
            return;
        }
        // Pet name limit 1-8 characters.
        if name.is_empty() || name.encode_utf16().count() > MAX_PET_NAME {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YourPetSNameCanBeUpTo8CharactersInLength),
            );
            return;
        }
        // No spaces.
        if name.contains(' ') {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::ThereIsASpaceInTheName),
            );
            return;
        }
        // `PetNameTable.isValidPetName`: only alphanumeric names that match `PetNameTemplate`.
        // NOTE: Java's `Character.isLetterOrDigit` is close to `char::is_alphanumeric`.
        if !name.chars().all(char::is_alphanumeric)
            || !(self.summons.pet_world.settings.pet_name_template)(name)
        {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::AnInvalidCharacterIsIncludedInThePetSName),
            );
            return;
        }
        // `pet.setName(_name)`.
        if let Some(state) = self.summon_mut(pet) {
            state.name = name.to_owned();
        }
        self.summons.pet_world.names.insert((name.to_lowercase(), collar));
        if let Some(control) = self.pet_control_object_id(pet) {
            self.pet_sync_control_name(player, control);
        }
        // Save to DB immediately.
        self.pet_store_me(pet);
        self.update_and_broadcast_summon_status(pet, 1);
    }

    /// `Pet.setName(name)` for a name that was set: the collar's second custom type becomes 1.
    fn pet_sync_control_name(&mut self, owner: ObjectId, control: ObjectId) {
        if self
            .inventory_item(owner, control)
            .is_some_and(|i| i.custom_type2 == 0)
        {
            if let Some(item) = self.pet_item_mut(owner, control) {
                item.custom_type2 = 1;
            }
            self.update_item_in_db(owner, control);
            if let Some(entry) = self.modified_entry(owner, control) {
                self.send_inventory_update(owner, entry);
            }
        }
    }

    /// Debug aid for tests: the pet's inventory as `(object id, item id, count, loc)`.
    pub fn pet_inventory_summary(&self, owner: ObjectId) -> Vec<(ObjectId, i32, i64, String)> {
        self.pet_items(owner)
            .map(|i| (i.object_id, i.item_id, i.count, i.loc.clone()))
            .collect()
    }
}
