//! Picking items up: the click on an item (`ItemClick`), the `PICK_UP` intention of a player and
//! `Player.doPickupItem`.
// Java: dist/game/data/scripts/handlers/actions/click/ItemClick.java
// Java: ai/CreatureAI.java (setIntentionPickUp, notifyActionForgetObject)
// Java: ai/PlayerAI.java (thinkPickUp)
// Java: entity/actor/Player.java (doPickupItem)
// Java: entity/item/instance/Item.java (pickupMe, resetOwnerTimer)
//
// # The flow
// A click on an item calls `setIntentionPickUp`, which walks to the item (`moveToPawn`, offset 20).
// When the player has arrived (the movement tick raises `ARRIVED`), `thinkPickUp` checks the
// distance again (36 plus nothing, items have no collision radius) and then calls `doPickupItem`,
// which does the checks of the pickup and moves the item into the inventory.
//
// # Left out
// - Merc tickets (`MercTicketManager`): the castle lord rule of `ItemClick`.
// - Fake death, combat flags, cursed weapons.
// - The auto equip of bolts for a crossbow: `World::check_and_equip_arrows` is the shared (and
//   arrow-only) helper of the attack code.

use l2_data::item::ADENA_ID;
use l2_data::item::types::{EtcItemType, WeaponType};
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ActionFailed, GetItem, StopMove, SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use tracing::warn;

use crate::inventory::handlers::ItemHandlerKind;
use crate::inventory::item_name_param;
use crate::movement::Intention;
use crate::objects::ObjectKind;
use crate::world::World;

/// `moveToPawn(object, 20)` of `setIntentionPickUp`.
const PICK_UP_MOVE_OFFSET: i32 = 20;
/// `maybeMoveToPawn(target, 36)` of `thinkPickUp`.
const PICK_UP_THINK_OFFSET: i32 = 36;

impl World {
    /// `ItemClick.onAction`: the player walks to the item and picks it up.
    pub(crate) fn item_click(&mut self, player: ObjectId, item: ObjectId) {
        // Merc tickets can only be picked up by the castle lord (`MercTicketManager`): sieges are
        // M5. `!player.isFlying()`: nobody flies.
        // This causes the validate position handler to do the pickup if the location is reached.
        self.set_intention_pick_up(player, item);
    }

    /// Java `CreatureAI.setIntentionPickUp(object)` for a player.
    pub(crate) fn set_intention_pick_up(&mut self, id: ObjectId, object: ObjectId) {
        self.stop_follow(id);
        let Some(player) = self.players.get(&id) else {
            return;
        };
        if player.movement.intention == Intention::Rest
            || self.is_all_skills_disabled(id)
            || self.is_casting_now_for_effects(id)
        {
            // Cancel action client side by sending Server->Client packet ActionFailed.
            self.send(id, &ActionFailed);
            return;
        }
        // Stop the actor auto-attack client side.
        self.client_stop_auto_attack(id);

        // `object.isItem() && getItemLocation() != VOID`: an item on the ground is always `VOID`.

        // Set the Intention of this AbstractAI to PICK_UP, and the AI pick up target.
        if let Some(player) = self.players.get_mut(&id) {
            player.movement.intention = Intention::PickUp;
            player.movement.pawn.ai_target = Some(object);
        }

        // NOTE(java-quirk): an object at 0,0 is moved next to the actor ("using a temporary fix").
        if let (Some(me), Some((0, 0, _))) = (self.position_of(id), self.position_of(object)) {
            if let Some(item) = self.ground_item_mut(object) {
                (item.x, item.y, item.z) = (me.0, me.1, me.2 + 5);
            }
        }

        // Move the actor to Pawn server side AND client side.
        self.move_to_pawn(id, object, PICK_UP_MOVE_OFFSET);
    }

    /// Java `PlayerAI.thinkPickUp`.
    pub(crate) fn think_pick_up(&mut self, id: ObjectId) {
        if self.is_all_skills_disabled(id) || self.is_casting_now_for_effects(id) {
            return;
        }
        let target = self.players.get(&id).and_then(|p| p.movement.pawn.ai_target);
        // `checkTargetLost`: the target is gone.
        let Some(target) = target.filter(|t| self.objects.contains_key(t)) else {
            self.set_intention_idle(id);
            return;
        };
        if self.maybe_move_to_pawn(id, target, PICK_UP_THINK_OFFSET) {
            return;
        }
        self.set_intention_idle(id);
        self.do_pickup_item(id, target);
    }

    /// Java `Player.doPickupItem(object)`.
    pub fn do_pickup_item(&mut self, id: ObjectId, object: ObjectId) {
        if self.is_alike_dead(id) {
            return;
        }
        if self.players.get(&id).is_some_and(|p| p.trade.active.is_some()) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouCannotPickUpOrUseItemsWhileTrading),
            );
            self.send(id, &ActionFailed);
            return;
        }

        // Set the AI Intention to IDLE.
        self.set_intention_idle(id);

        // Check if the WorldObject to pick up is a Item.
        if self.objects.get(&object).map(|e| e.kind) != Some(ObjectKind::Item) {
            // Do not try to pickup anything that is not an item :)
            warn!("{id} trying to pickup wrong target {object}.");
            return;
        }

        let Some(me) = self
            .players
            .get(&id)
            .map(|p| (p.core.x, p.core.y, p.core.z, p.core.heading))
        else {
            return;
        };
        self.send(
            id,
            &StopMove {
                object_id: id.0,
                x: me.0,
                y: me.1,
                z: me.2,
                heading: me.3,
            },
        );

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
            // Send a Server->Client packet ActionFailed to this Player.
            self.send(id, &ActionFailed);
            return;
        };
        let item_id = target.data.item_id;
        let template = templates.and_then(|t| t.get(item_id));

        if !self.try_pick_up(&target, id) {
            self.send(id, &ActionFailed);
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouHaveFailedToPickUpS1)
                    .with(item_name_param(templates, item_id)),
            );
            return;
        }

        // Only a player without a party, or with finders keepers, needs room for the item itself:
        // the other loot rules look for a member who has it.
        let needs_room = self.party_of(id).is_none_or(|party| {
            self.party_distribution_type(party)
                == Some(l2_protocol_game::server::PartyDistributionType::FindersKeepers)
        });
        if needs_room
            && let Some(template) = template
            && !self.validate_capacity_for_item(id, template)
        {
            self.send(id, &ActionFailed);
            self.send(id, &SystemMessage::new(SystemMessageId::YourInventoryIsFull));
            return;
        }

        if self.objects.get(&id).is_some_and(|e| e.invisible) && !self.is_gm(id) {
            return;
        }

        if target
            .owner
            .is_some_and(|owner| owner != id && !self.is_in_looter_party(id, owner))
        {
            let message = if item_id == ADENA_ID {
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
            self.send(id, &ActionFailed);
            self.send(id, &message);
            return;
        }

        // You can pickup only 1 combat flag (`FortSiegeManager`): sieges are M5.

        // `if (target.getItemLootShedule() != null && owner == this) target.resetOwnerTimer()`
        // is moot: the timer goes with the item (`take_ground_item`).

        // Remove the Item from the world and send server->client GetItem packets.
        let (x, y, z) = (target.x, target.y, target.z);
        self.broadcast_to_self_and_visible(
            id,
            &GetItem {
                player_id: id.0,
                object_id: object.0,
                x,
                y,
                z,
            },
        );
        let Some(taken) = self.take_ground_item(object) else {
            return;
        };

        let Some(template) = template else {
            // Unknown item: it cannot be told what it is, so it just joins the inventory.
            self.add_item_object(id, taken.data, true);
            return;
        };

        // Auto use herbs - pick up.
        if template.ex_immediate_effect {
            match ItemHandlerKind::of_template(template) {
                // The herb is used from the ground: it never enters the inventory.
                Some(kind) => {
                    self.run_item_handler(kind, id, &taken.data, template, false);
                }
                None => warn!("No item handler registered for item ID: {item_id}."),
            }
            // `ItemManager.destroyItem(NONE, target, this, null)`.
            self.release_item_id(taken.data.object_id);
            return;
        }

        // If item is instance of ArmorType or WeaponType broadcast an "Attention" system message.
        if template.is_weapon() || template.is_armor() {
            let name = self
                .players
                .get(&id)
                .map(|p| p.data.name.clone())
                .unwrap_or_default();
            let message = if taken.data.enchant_level > 0 {
                SystemMessage::new(SystemMessageId::AttentionC1HasPickedUpS2S3)
                    .with(SystemMessageParam::PlayerName(name))
                    .with(SystemMessageParam::IntNumber(taken.data.enchant_level))
                    .with(SystemMessageParam::ItemName(item_id))
            } else {
                SystemMessage::new(SystemMessageId::AttentionC1HasPickedUpS2)
                    .with(SystemMessageParam::PlayerName(name))
                    .with(SystemMessageParam::ItemName(item_id))
            };
            self.broadcast_to_self_and_visible(id, &message);
        }

        // Check if a Party is in progress.
        if self.is_in_party(id) {
            self.party_distribute_ground_item(id, taken.data);
        } else if item_id == ADENA_ID && self.adena_of(id) > 0 {
            self.add_adena(id, taken.data.count, true);
            // `ItemManager.destroyItem(PICKUP, target, this, null)`.
            self.release_item_id(taken.data.object_id);
        } else {
            self.add_item_object(id, taken.data, true);

            // Auto-Equip arrows if player has a bow and player picks up arrows.
            let weapon = self
                .paperdoll_object(id, l2_protocol_game::server::charscreen::paperdoll::RHAND)
                .and_then(|o| self.inventory_item(id, o))
                .map(|w| w.item_id)
                .and_then(|w| templates.and_then(|t| t.get(w)));
            if let Some(weapon) = weapon
                && template.etc().is_some_and(|e| e.etc_type == EtcItemType::Arrow)
                && weapon.weapon().is_some_and(|w| w.weapon_type == WeaponType::Bow)
            {
                self.check_and_equip_arrows(id, weapon.crystal_type_plus());
            }
        }
    }
}
