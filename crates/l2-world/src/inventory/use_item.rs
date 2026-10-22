//! `UseItem` (the equippable branch and the handler branch), `Player.useEquippableItem` and
//! `RequestUnEquipItem`.
// Java: network/clientpackets/UseItem.java
// Java: network/clientpackets/RequestUnEquipItem.java
// Java: entity/actor/Player.java (useEquippableItem)

use l2_data::item::TYPE2_QUEST;
use l2_data::item::types::{ArmorType, BodyPart, WeaponType};
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ActionFailed, SystemMessage, SystemMessageParam};
use l2_util::ObjectId;

use super::paperdoll::{body_part_of_paperdoll_slot, body_part_of_worn_item};
use super::{LOC_PAPERDOLL, is_equipable, item_name_param};
use crate::effects::EffectType;
use crate::enter::storage_max_count;
use crate::flood::FloodKind;
use crate::zones::ZoneId;

/// Java `Race.KAMAEL.ordinal()`; `Race.DWARF` is `crate::data::RACE_DWARF`.
const RACE_KAMAEL: i32 = 5;
/// The Combat Flag of the territory and fortress sieges.
const COMBAT_FLAG_ID: i32 = 9819;
/// The Formal Wear (`UseItem` refuses it while a cursed weapon is equipped).
const FORMAL_WEAR_ID: i32 = 6408;

impl crate::world::World {
    /// Java `UseItem.runImpl`.
    pub(crate) fn on_use_item(&mut self, id: ObjectId, object_id: i32, ctrl_pressed: bool) {
        // Flood protect UseItem.
        if !self.flood_allows(id, FloodKind::UseItem) {
            return;
        }

        // Jail restriction.
        if self.is_inside_zone(id, ZoneId::Jail) {
            self.send_message(id, "You cannot use items while jailed.");
            return;
        }
        let object_id = ObjectId(object_id);
        let Some(item) = self.inventory_item(id, object_id).cloned() else {
            return;
        };
        let templates = self.item_templates();
        let templates = templates.as_deref();
        let Some(template) = templates.and_then(|t| t.get(item.item_id)) else {
            return;
        };

        // Pet Collar exclusion. This is handled in SummonItems handler.
        let is_pet_collar = super::etc_type(template) == Some(l2_data::item::types::EtcItemType::PetCollar);
        if self.players.get(&id).is_some_and(|p| p.trade.active.is_some()) && !is_pet_collar {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouCannotPickUpOrUseItemsWhileTrading),
            );
            return;
        }
        if self.is_in_store_mode(id) && !is_pet_collar {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouMayNotUseItemsInAPrivateStoreOrPrivateWorkShop),
            );
            self.send(id, &ActionFailed);
            return;
        }

        if template.type2 == TYPE2_QUEST {
            self.send(id, &SystemMessage::new(SystemMessageId::YouCannotUseQuestItems));
            return;
        }

        // No UseItem is allowed while the player is in special conditions.
        if self.is_stunned(id)
            || self.is_paralyzed(id)
            || self.is_sleeping(id)
            || self.is_control_blocked(id)
            || self.is_alike_dead(id)
        {
            return;
        }

        // Char cannot use item when dead (handled above) or when the inventory block forbids it.
        if !self.can_manipulate_with_item_id(id, item.item_id) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::S1CannotBeUsedDueToUnsuitableTerms)
                    .with(item_name_param(templates, item.item_id)),
            );
            return;
        }

        let equipped = item.loc == LOC_PAPERDOLL;
        if !equipped && !self.check_item_condition(id, template, true) {
            return;
        }

        // Fishing (`isFishing()`): not modelled.

        // The karma teleport rule: `AltKarmaPlayerCanTeleport` against item skills with a
        // teleport effect.
        let karma = self.players.get(&id).map_or(0, |p| p.data.karma);
        if !self.settings.cast.alt_game_karma_player_can_teleport
            && karma > 0
            && let Some(skills) = &template.skills.item_skills
            && let Some(table) = self.skill_table()
        {
            for holder in skills {
                if let Some(skill) = table.get(holder.id, holder.level)
                    && self.skill_has_effect_type(skill, &[EffectType::Teleport])
                {
                    return;
                }
            }
        }

        // If the item has reuse time and it has not passed. Message from reuse delay must come
        // from item.
        if self.item_reuse_blocks_use(id, &item, template) {
            return;
        }

        // `player.onActionRequest()`: only the pet restore is modelled (`summon/pet.rs`).
        self.on_action_request(id);

        if is_equipable(template) {
            // Check if player is casting or using a skill: casting is M5 (`isCastingNow()` and
            // `isCastingSimultaneouslyNow()` are false until then), which sends
            // YOU_CANNOT_CHANGE_WEAPONS_DURING_AN_ATTACK + ActionFailed here.

            // Don't allow to put formal wear while a cursed weapon is equipped: no cursed weapons.
            let _ = FORMAL_WEAR_ID;
            // Equip or unEquip. `FortSiegeManager.isCombat(itemId)` and `isCombatFlagEquipped`
            // belong to the sieges: nobody carries a flag.

            let cannot_equip =
                SystemMessage::new(SystemMessageId::YouDoNotMeetTheRequiredConditionToEquipThatItem);
            match template.body_part {
                BodyPart::BothHands | BodyPart::LeftHand | BodyPart::RightHand => {
                    // Prevent players to equip weapon while wearing combat flag.
                    let worn_weapon = self
                        .paperdoll_object(id, l2_protocol_game::server::charscreen::paperdoll::RHAND)
                        .and_then(|w| self.inventory_item(id, w))
                        .map(|w| w.item_id);
                    if worn_weapon == Some(COMBAT_FLAG_ID) {
                        self.send(id, &cannot_equip);
                        return;
                    }
                    if self.is_mounted(id) {
                        self.send(id, &cannot_equip);
                        return;
                    }
                    if self.is_disarmed(id) {
                        self.send(id, &cannot_equip);
                        return;
                    }

                    // Don't allow other Race to Wear Kamael exclusive Weapons.
                    if !equipped && template.is_weapon() && !self.is_gm(id) {
                        let race = self.players.get(&id).map_or(0, |p| p.data.race);
                        let weapon = super::weapon_type(template);
                        let refused = if race == RACE_KAMAEL {
                            weapon == Some(WeaponType::None)
                        } else {
                            matches!(
                                weapon,
                                Some(WeaponType::Rapier | WeaponType::CrossBow | WeaponType::AncientSword)
                            )
                        };
                        if refused {
                            self.send(id, &cannot_equip);
                            return;
                        }
                    }
                }
                BodyPart::Chest
                | BodyPart::Back
                | BodyPart::Gloves
                | BodyPart::Feet
                | BodyPart::Head
                | BodyPart::FullArmor
                | BodyPart::Legs => {
                    let race = self.players.get(&id).map_or(0, |p| p.data.race);
                    let armor = template.armor().map(|a| a.armor_type);
                    if race == RACE_KAMAEL && matches!(armor, Some(ArmorType::Heavy | ArmorType::Magic)) {
                        self.send(id, &cannot_equip);
                        return;
                    }
                }
                BodyPart::Deco => {
                    let talisman_slots = self.stats(id).map_or(0, |v| v.talisman_slots());
                    if !equipped && talisman_slots == 0 {
                        self.send(id, &cannot_equip);
                        return;
                    }
                }
                _ => {}
            }

            // Over-enchant protection.
            if self.over_enchant_protection_on_use(id, &item, template) {
                return;
            }

            // M5: while casting, the equip waits for the cast to finish (`NextAction` on
            // FINISH_CASTING).
            // Equip or unEquip: during an attack animation, when it ends (without aborting it);
            // otherwise at once, aborting the attack.
            let now = self.world_ms();
            let attack_end = self.combat_state(id).map_or(0, |c| c.attack_end_ms);
            if attack_end > now {
                let delay = u64::try_from(attack_end - now).unwrap_or(0);
                let due = self.tick + crate::timer::TimerWheel::ms_to_ticks(delay);
                self.timers.schedule_at(
                    due,
                    crate::timer::TimerKind::EquipAfterAttack {
                        player: id,
                        item: object_id,
                    },
                );
            } else {
                self.use_equippable_item(id, object_id, true);
            }
        } else {
            self.use_item_with_handler(id, &item, template, ctrl_pressed);
        }
    }

    /// Java `Player.useEquippableItem(item, abortAttack)`: take the item off if it is worn, else
    /// put it on, and tell the client.
    pub(crate) fn use_equippable_item(&mut self, id: ObjectId, object_id: ObjectId, abort_attack: bool) {
        // Check if the item is owned by this player and in the inventory or worn.
        let Some(item) = self.inventory_item(id, object_id).cloned() else {
            return;
        };
        let templates = self.item_templates();
        let templates = templates.as_deref();
        let Some(template) = templates.and_then(|t| t.get(item.item_id)) else {
            return;
        };
        // Equip or unEquip.
        let is_equipped = item.loc == LOC_PAPERDOLL;
        let old_inventory_limit = self.stats(id).map_or(0, |v| v.inventory_limit());
        let changed;
        if is_equipped {
            let part = body_part_of_worn_item(&item, template);
            self.send(
                id,
                &self.disarmed_message(item.enchant_level, item.item_id, templates),
            );
            // We can't unequip talisman by body slot.
            changed = if part == Some(BodyPart::Deco) {
                self.unequip_item_in_slot_and_record(id, usize::try_from(item.loc_data).unwrap_or(usize::MAX))
            } else {
                match part {
                    Some(part) => self.unequip_item_in_body_slot_and_record(id, part),
                    None => Vec::new(),
                }
            };
        } else {
            changed = self.equip_item_and_record(id, object_id);
            let now = self.inventory_item(id, object_id).cloned();
            if now.as_ref().is_some_and(|i| i.loc == LOC_PAPERDOLL) {
                let name = item_name_param(templates, item.item_id);
                let message = if item.enchant_level > 0 {
                    SystemMessage::new(SystemMessageId::EquippedS1S2)
                        .with(SystemMessageParam::IntNumber(item.enchant_level))
                        .with(name)
                } else {
                    SystemMessage::new(SystemMessageId::YouHaveEquippedYourS1).with(name)
                };
                self.send(id, &message);
                // M5: `item.decreaseMana(false)` (shadow items); the `ON_PLAYER_ITEM_EQUIP` script
                // event.
                if matches!(template.body_part, BodyPart::RightHand | BodyPart::BothHands) {
                    self.recharge_shots(id, true, true);
                }
            } else {
                self.send(
                    id,
                    &SystemMessage::new(SystemMessageId::YouDoNotMeetTheRequiredConditionToEquipThatItem),
                );
            }
        }

        self.refresh_expertise(id);
        self.broadcast_user_info(id);

        // `iu.addItems(items)`: one entry per object, as the items are now.
        self.send_changed_items(id, &changed);

        if abort_attack {
            self.abort_attack(id);
        }

        let limit = self.stats(id).map_or(0, |v| v.inventory_limit());
        if limit != old_inventory_limit {
            self.send_storage_max_count(id);
        }
    }

    /// `InventoryUpdate.addItems(changed)` + `Player.sendInventoryUpdate`: the items whose slot
    /// changed, each once, with the change flag `MODIFIED` that `setPaperdollItem` leaves.
    pub(crate) fn send_changed_items(&mut self, id: ObjectId, changed: &[ObjectId]) {
        let mut seen: Vec<ObjectId> = Vec::new();
        for object_id in changed {
            if seen.contains(object_id) {
                continue;
            }
            seen.push(*object_id);
            if let Some(entry) = self.modified_entry(id, *object_id) {
                self.send_inventory_update(id, entry);
            }
        }
    }

    /// `new ExStorageMaxCount(player)`: the limits, with the inventory limit from the stat engine
    /// (`INV_LIM` included).
    pub(crate) fn send_storage_max_count(&self, id: ObjectId) {
        let Some(player) = self.players.get(&id) else {
            return;
        };
        let mut packet = storage_max_count(&player.data, self.is_gm(id), &self.settings);
        if let Some(view) = self.stats(id) {
            packet.inventory = view.inventory_limit();
        }
        self.send(id, &packet);
    }

    /// Java `RequestUnEquipItem.runImpl`.
    pub(crate) fn on_request_unequip_item(&mut self, id: ObjectId, paperdoll_slot: i32) {
        let part = body_part_of_paperdoll_slot(paperdoll_slot);
        // `getPaperdollItemByBodyPart(slot)` goes by `BodyPart.getPaperdollSlot()`.
        // NOTE(java-quirk): every talisman slot maps to `DECO`, whose slot is the first one, so a
        // request for the third talisman takes the first one off.
        let Some(worn_id) = part
            .and_then(super::paperdoll::paperdoll_slot_of)
            .and_then(|slot| self.paperdoll_object(id, slot))
        else {
            // Wear-items are not to be unequipped.
            return;
        };
        let Some(item) = self.inventory_item(id, worn_id).cloned() else {
            return;
        };
        let templates = self.item_templates();
        let templates = templates.as_deref();
        let Some(template) = templates.and_then(|t| t.get(item.item_id)) else {
            return;
        };

        // Prevent player from unequipping items in special conditions.
        if self.is_stunned(id) || self.is_sleeping(id) || self.is_paralyzed(id) || self.is_alike_dead(id) {
            return;
        }
        // The English system message say weapon, but it's applied to any equipped item.
        // M5: `isCastingNow() || isCastingSimultaneouslyNow()` join the check with the casts.
        if self.is_attacking_now(id) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouCannotChangeWeaponsDuringAnAttack),
            );
            return;
        }

        // Arrows and bolts.
        if part == Some(BodyPart::LeftHand) && template.is_etc_item() {
            return;
        }
        // Prevent of unequipping a cursed weapon: `_slot == LR_HAND` can never be the part of a
        // paperdoll slot, so Java never reaches that check either.

        // Prevent player from unequipping items in special conditions (stun, sleep, paralysis).
        if self.is_alike_dead(id) {
            return;
        }

        if !self.can_manipulate_with_item_id(id, item.item_id) {
            self.send(id, &SystemMessage::new(SystemMessageId::ThatItemCannotBeTakenOff));
            return;
        }

        if template.weapon().is_some_and(|w| w.force_equip) && !self.is_gm(id) {
            self.send(id, &SystemMessage::new(SystemMessageId::ThatItemCannotBeTakenOff));
            return;
        }

        let Some(part) = part else {
            return;
        };
        let unequipped = self.unequip_item_in_body_slot_and_record(id, part);
        self.broadcast_user_info(id);

        // This can be 0 if the user pressed the right mouse button twice very fast.
        if let Some(first) = unequipped.first().copied() {
            // NOTE(java-quirk): the message names the first item the recorder saw. Taking a bow
            // off records its arrows first, so the message is about the arrows.
            if let Some(first_item) = self.inventory_item(id, first).cloned() {
                let message = self.disarmed_message(first_item.enchant_level, first_item.item_id, templates);
                self.send(id, &message);
            }
            self.send_changed_items(id, &unequipped);
        }
    }
}
