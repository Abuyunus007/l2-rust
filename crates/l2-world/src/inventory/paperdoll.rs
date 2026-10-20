//! The paperdoll: `Inventory.setPaperdollItem`, `equipItem`, the unequip variants and the
//! listeners that run on every change.
// Java: entity/itemcontainer/Inventory.java (setPaperdollItem, equipItem, unEquipItemInSlot,
//       unEquipItemInBodySlot, equipTalisman, the PaperdollListener classes)
// Java: entity/item/enums/BodyPart.java
// Java: entity/actor/Player.java (checkItemRestriction)
// Java: entity/item/ItemTemplate.java (checkCondition)
//
// Java runs these listeners, in this order, on every change of a slot: `ArmorSetListener`,
// `BowCrossRodListener`, `ItemSkillsListener`, `BraceletListener`, `StatsListener`, and the
// `ChangeRecorder` of the caller last. The stat engine does the armor set, item skill and stat
// parts in one call (`World::on_equip` / `on_unequip`); here the bow/bracelet parts run before
// it. The only visible difference is the order of calculator entries of different items.
//
// `changed` is the `ChangeRecorder`: the objects whose slot changed, in the order the recorder
// saw them. A nested change (the arrow a bow takes, the talismans a bracelet takes) is recorded
// before the item that caused it, because the recorder is the last listener.

use l2_data::item::ItemTemplate;
use l2_data::item::types::{BodyPart, EtcItemType, WeaponType};
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::charscreen::paperdoll as pd;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;

use super::{LOC_INVENTORY, LOC_PAPERDOLL, etc_type, item_name_param, weapon_type};
use crate::player::ItemData;
use crate::stats::conditions::{self, CondEnv};
use crate::world::World;

/// Java `BodyPart.getPaperdollSlot()`; `None` where Java has -1 (the two-slot parts, the
/// two-handed, full armor and hair parts, the pet armor) or a slot past the paperdoll.
pub(crate) fn paperdoll_slot_of(part: BodyPart) -> Option<usize> {
    Some(match part {
        BodyPart::Underwear => pd::UNDER,
        BodyPart::RightEar => pd::REAR,
        BodyPart::LeftEar => pd::LEAR,
        BodyPart::Neck => pd::NECK,
        BodyPart::RightFinger => pd::RFINGER,
        BodyPart::LeftFinger => pd::LFINGER,
        BodyPart::Head => pd::HEAD,
        BodyPart::RightHand => pd::RHAND,
        BodyPart::LeftHand => pd::LHAND,
        BodyPart::Gloves => pd::GLOVES,
        BodyPart::Chest => pd::CHEST,
        BodyPart::Legs => pd::LEGS,
        BodyPart::Feet => pd::FEET,
        BodyPart::Back => pd::CLOAK,
        BodyPart::Hair => pd::HAIR,
        BodyPart::Hair2 => pd::HAIR2,
        BodyPart::RightBracelet => pd::RBRACELET,
        BodyPart::LeftBracelet => pd::LBRACELET,
        BodyPart::Deco => pd::DECO1,
        BodyPart::Belt => pd::BELT,
        _ => return None,
    })
}

/// Java `BodyPart.getPaperdollIndex(bodyPart)`: the slot an item of this part goes to or comes
/// off, with the special cases for two-handed weapons, full armor, all-in-one dresses and hair.
pub fn paperdoll_index_of(part: BodyPart) -> Option<usize> {
    match part {
        BodyPart::BothHands => Some(pd::RHAND),
        BodyPart::FullArmor | BodyPart::AllDress => Some(pd::CHEST),
        BodyPart::HairAll => Some(pd::HAIR),
        other => paperdoll_slot_of(other),
    }
}

/// Java `BodyPart.fromPaperdollSlot(slot)`: every talisman slot maps to `DECO`.
pub fn body_part_of_paperdoll_slot(slot: i32) -> Option<BodyPart> {
    Some(match usize::try_from(slot).ok()? {
        pd::UNDER => BodyPart::Underwear,
        pd::HEAD => BodyPart::Head,
        pd::HAIR => BodyPart::Hair,
        pd::HAIR2 => BodyPart::Hair2,
        pd::NECK => BodyPart::Neck,
        pd::RHAND => BodyPart::RightHand,
        pd::CHEST => BodyPart::Chest,
        pd::LHAND => BodyPart::LeftHand,
        pd::REAR => BodyPart::RightEar,
        pd::LEAR => BodyPart::LeftEar,
        pd::GLOVES => BodyPart::Gloves,
        pd::LEGS => BodyPart::Legs,
        pd::FEET => BodyPart::Feet,
        pd::RFINGER => BodyPart::RightFinger,
        pd::LFINGER => BodyPart::LeftFinger,
        pd::LBRACELET => BodyPart::LeftBracelet,
        pd::RBRACELET => BodyPart::RightBracelet,
        pd::DECO1..=pd::DECO6 => BodyPart::Deco,
        pd::CLOAK => BodyPart::Back,
        pd::BELT => BodyPart::Belt,
        _ => return None,
    })
}

/// Java `BodyPart.fromItem(item)` for a worn item: the template's part for the chest slot (a full
/// armor or a dress), else the part that belongs to the slot.
pub(crate) fn body_part_of_worn_item(item: &ItemData, template: &ItemTemplate) -> Option<BodyPart> {
    if usize::try_from(item.loc_data) == Ok(pd::CHEST) {
        Some(template.body_part)
    } else {
        body_part_of_paperdoll_slot(item.loc_data)
    }
}

/// Java `ArmorSet.containItem(slot, itemId)`.
fn set_contains_item(set: &l2_data::ArmorSet, slot: usize, item_id: i32) -> bool {
    match slot {
        pd::CHEST => set.chest_id == item_id,
        pd::LEGS => set.legs.contains(&item_id),
        pd::HEAD => set.head.contains(&item_id),
        pd::GLOVES => set.gloves.contains(&item_id),
        pd::FEET => set.feet.contains(&item_id),
        _ => false,
    }
}

impl World {
    /// Java `Item.setItemLocation(loc, locData)`: the item as it is afterwards.
    fn set_item_location(
        &mut self,
        player: ObjectId,
        object_id: ObjectId,
        loc: &str,
        loc_data: i32,
    ) -> Option<ItemData> {
        let item = self
            .players
            .get_mut(&player)?
            .data
            .items
            .iter_mut()
            .find(|i| i.object_id == object_id)?;
        item.loc = loc.to_string();
        item.loc_data = loc_data;
        Some(item.clone())
    }

    /// Java `Inventory.setPaperdollItem(slot, item)`: puts `item` (already in the inventory) into
    /// the slot, or empties it. The old item goes back to `INVENTORY`. Returns the old item.
    pub(crate) fn set_paperdoll_item(
        &mut self,
        id: ObjectId,
        slot: usize,
        item: Option<ObjectId>,
        changed: &mut Vec<ObjectId>,
    ) -> Option<ObjectId> {
        let old = self.paperdoll_object(id, slot);
        if old != item {
            if let Some(old_id) = old {
                // Put old item from paperdoll slot to base location.
                if let Some(old_item) = self.set_item_location(id, old_id, LOC_INVENTORY, 0) {
                    // Notify all paperdoll listener in order to unequip old item in slot.
                    self.notify_unequipped(id, slot, &old_item, changed);
                    self.update_item_in_db(id, old_id);
                }
            }
            // Add new item in slot of paperdoll.
            if let Some(new_id) = item
                && let Some(new_item) =
                    self.set_item_location(id, new_id, LOC_PAPERDOLL, i32::try_from(slot).unwrap_or(i32::MAX))
            {
                self.notify_equipped(id, slot, &new_item, changed);
                self.update_item_in_db(id, new_id);
            }
        }
        // Notify to scripts (`ON_PLAYER_ITEM_UNEQUIP`): the script API is M7.
        old
    }

    /// The listeners' `notifyEquiped`. `item` is already in `slot`.
    fn notify_equipped(&mut self, id: ObjectId, slot: usize, item: &ItemData, changed: &mut Vec<ObjectId>) {
        // BowCrossRodListener
        self.bow_cross_rod_equipped(id, slot, item, changed);
        // ArmorSetListener, ItemSkillsListener, StatsListener
        // `Options.apply` sends the skill list for every option of an augmented item.
        let skills_listed =
            self.skill_list_changes_on_equip(id, slot, item) || self.augmentation_sends_skill_list(item);
        let stats = self.on_equip(id, item);
        self.broadcast_modified_stats(id, &stats);
        self.refresh_max_vitals(id, false);
        if skills_listed {
            self.send_skill_list(id);
        }
        // ChangeRecorder
        changed.push(item.object_id);
    }

    /// The listeners' `notifyUnequiped`. `item` has already left `slot`.
    fn notify_unequipped(&mut self, id: ObjectId, slot: usize, item: &ItemData, changed: &mut Vec<ObjectId>) {
        // ArmorSetListener decides from the chest as it is now.
        let set_removal = self.armor_set_removal(id, slot, item);
        let skills_before = self.players.get(&id).map(|p| p.core.skills.clone());
        let templates = self.item_templates();
        let template = templates.as_deref().and_then(|t| t.get(item.item_id));
        let mut skills_listed = set_removal
            || self.augmentation_sends_skill_list(item)
            || template.is_some_and(|t| {
                (item.enchant_level >= 4 && t.skills.enchant4_skill.is_some())
                    || t.skills.item_skills.iter().flatten().next().is_some()
            });
        // BowCrossRodListener
        self.bow_cross_rod_unequipped(id, slot, template, changed);
        // BraceletListener
        if template.is_some_and(|t| t.body_part == BodyPart::RightBracelet) {
            for deco in pd::DECO1..=pd::DECO6 {
                self.set_paperdoll_item(id, deco, None, changed);
            }
        }
        // ArmorSetListener, ItemSkillsListener, StatsListener
        let stats = self.on_unequip(id, item, slot);
        // `ArmorSetListener`: "player.checkItemRestriction(); player.sendSkillList();"
        if set_removal {
            self.check_item_restriction(id, changed);
        }
        // An armor taken off can give another item's skills back (`readd` in the stat engine).
        if !skills_listed
            && let (Some(before), Some(after)) =
                (skills_before, self.players.get(&id).map(|p| &p.core.skills))
        {
            skills_listed = before != *after;
        }
        self.broadcast_modified_stats(id, &stats);
        self.refresh_max_vitals(id, false);
        if skills_listed {
            self.send_skill_list(id);
        }
        // ChangeRecorder
        changed.push(item.object_id);
    }

    /// `BowCrossRodListener.notifyEquiped`: a bow takes arrows, a crossbow bolts, of its grade.
    fn bow_cross_rod_equipped(
        &mut self,
        id: ObjectId,
        slot: usize,
        item: &ItemData,
        changed: &mut Vec<ObjectId>,
    ) {
        if slot != pd::RHAND {
            return;
        }
        let templates = self.item_templates();
        let Some(template) = templates.as_deref().and_then(|t| t.get(item.item_id)) else {
            return;
        };
        let ammo = match weapon_type(template) {
            Some(WeaponType::Bow) => EtcItemType::Arrow,
            Some(WeaponType::CrossBow) => EtcItemType::Bolt,
            _ => return,
        };
        // `findArrowForBow` / `findBoltForCrossBow`: the first etc item of the same grade.
        // NOTE(java-quirk): Java walks a hash set, so which stack of the right grade wins is
        // arbitrary there; the first one in the inventory list wins here.
        let found = self.players.get(&id).and_then(|p| {
            p.data
                .items
                .iter()
                .filter(|i| i.loc == LOC_INVENTORY || i.loc == LOC_PAPERDOLL)
                .find(|i| {
                    templates
                        .as_deref()
                        .and_then(|t| t.get(i.item_id))
                        .is_some_and(|t| {
                            t.is_etc_item()
                                && t.crystal_type_plus() == template.crystal_type_plus()
                                && etc_type(t) == Some(ammo)
                        })
                })
                .map(|i| i.object_id)
        });
        if let Some(ammo) = found {
            self.set_paperdoll_item(id, pd::LHAND, Some(ammo), changed);
        }
    }

    /// `BowCrossRodListener.notifyUnequiped`: whatever is in the left hand (arrows, bolts, a lure)
    /// goes with the bow, crossbow or fishing rod.
    fn bow_cross_rod_unequipped(
        &mut self,
        id: ObjectId,
        slot: usize,
        template: Option<&ItemTemplate>,
        changed: &mut Vec<ObjectId>,
    ) {
        if slot != pd::RHAND {
            return;
        }
        if matches!(
            template.and_then(weapon_type),
            Some(WeaponType::Bow | WeaponType::CrossBow | WeaponType::FishingRod)
        ) && self.paperdoll_object(id, pd::LHAND).is_some()
        {
            self.set_paperdoll_item(id, pd::LHAND, None, changed);
        }
    }

    /// Whether `ArmorSetListener.notifyUnequiped` removes skills for this item (it then also runs
    /// `checkItemRestriction` and sends the skill list).
    fn armor_set_removal(&self, id: ObjectId, slot: usize, item: &ItemData) -> bool {
        let Some(data) = self.data.stat_data() else {
            return false;
        };
        if slot == pd::CHEST {
            return data.armor_sets.is_armor_set(item.item_id);
        }
        let Some(chest) = self
            .paperdoll_object(id, pd::CHEST)
            .and_then(|c| self.inventory_item(id, c))
        else {
            return false;
        };
        let Some(set) = data.armor_sets.get(chest.item_id) else {
            return false;
        };
        set_contains_item(set, slot, item.item_id) || set.shield.contains(&item.item_id)
    }

    /// Whether the skill listeners would tell the client about new skills: `update` of
    /// `ArmorSetListener.notifyEquiped` and `ItemSkillsListener.notifyEquiped`.
    fn skill_list_changes_on_equip(&self, id: ObjectId, slot: usize, item: &ItemData) -> bool {
        let Some(data) = self.data.stat_data() else {
            return false;
        };
        if let Some(t) = data.items.get(item.item_id)
            && ((item.enchant_level >= 4 && t.skills.enchant4_skill.is_some())
                || t.skills.item_skills.iter().flatten().next().is_some())
        {
            return true;
        }
        let Some(view) = self.stats(id) else {
            return false;
        };
        let Some(chest) = view.paperdoll(pd::CHEST) else {
            return false;
        };
        let Some(set) = data.armor_sets.get(chest.item_id) else {
            return false;
        };
        if set_contains_item(set, slot, item.item_id) {
            if !view.armor_set_complete(set) {
                return false;
            }
            let shield_worn = view
                .paperdoll(pd::LHAND)
                .is_some_and(|s| set.shield.contains(&s.item_id));
            !set.skills.is_empty()
                || (shield_worn && !set.shield_skills.is_empty())
                || (view.armor_set_enchanted6(set) && !set.enchant6_skills.is_empty())
        } else {
            set.shield.contains(&item.item_id) && !set.shield_skills.is_empty()
        }
    }

    /// Java `Inventory.equipItem(item)`: the switch over the body part with its side effects.
    pub(crate) fn equip_item(&mut self, id: ObjectId, object_id: ObjectId, changed: &mut Vec<ObjectId>) {
        let Some(item) = self.inventory_item(id, object_id).cloned() else {
            return;
        };
        let templates = self.item_templates();
        let templates = templates.as_deref();
        let Some(template) = templates.and_then(|t| t.get(item.item_id)) else {
            return;
        };
        // Store mode is not modelled. `isHero()` is not modelled either: nobody is a hero.
        if !self.is_gm(id) && template.is_hero_item() {
            return;
        }
        let part = template.body_part;

        // Check if player is using Formal Wear and item isn't Wedding Bouquet.
        let formal_is_all_dress = self
            .paperdoll_object(id, pd::CHEST)
            .and_then(|c| self.inventory_item(id, c))
            .and_then(|c| templates.and_then(|t| t.get(c.item_id)))
            .is_some_and(|t| t.body_part == BodyPart::AllDress);
        if item.item_id != 21163 && formal_is_all_dress {
            // Only chest target can pass this.
            if matches!(
                part,
                BodyPart::BothHands
                    | BodyPart::LeftHand
                    | BodyPart::RightHand
                    | BodyPart::Legs
                    | BodyPart::Feet
                    | BodyPart::Gloves
                    | BodyPart::Head
            ) {
                return;
            }
        }

        let slot_empty = |world: &World, slot: usize| world.paperdoll_object(id, slot).is_none();
        let set = |world: &mut World, slot: usize, what: Option<ObjectId>, changed: &mut Vec<ObjectId>| {
            world.set_paperdoll_item(id, slot, what, changed);
        };
        let this = Some(object_id);

        // Handle special cases.
        match part {
            BodyPart::BothHands => {
                set(self, pd::LHAND, None, changed);
                set(self, pd::RHAND, this, changed);
            }
            BodyPart::LeftHand => {
                let right = self
                    .paperdoll_object(id, pd::RHAND)
                    .and_then(|r| self.inventory_item(id, r))
                    .and_then(|r| templates.and_then(|t| t.get(r.item_id)));
                let right_two_handed = right.is_some_and(|rh| rh.body_part == BodyPart::BothHands);
                let ammo_for_it = right.is_some_and(|rh| {
                    let item_type = etc_type(template);
                    (weapon_type(rh) == Some(WeaponType::Bow) && item_type == Some(EtcItemType::Arrow))
                        || (weapon_type(rh) == Some(WeaponType::CrossBow)
                            && item_type == Some(EtcItemType::Bolt))
                        || (weapon_type(rh) == Some(WeaponType::FishingRod)
                            && item_type == Some(EtcItemType::Lure))
                });
                if right_two_handed && !ammo_for_it {
                    set(self, pd::RHAND, None, changed);
                }
                set(self, pd::LHAND, this, changed);
            }
            // Don't care about arrows, listener will unequip them (hopefully).
            BodyPart::RightHand => set(self, pd::RHAND, this, changed),
            BodyPart::LeftEar | BodyPart::RightEar | BodyPart::BothEars => {
                if slot_empty(self, pd::LEAR) {
                    set(self, pd::LEAR, this, changed);
                } else if slot_empty(self, pd::REAR) {
                    set(self, pd::REAR, this, changed);
                } else {
                    set(self, pd::LEAR, this, changed);
                }
            }
            BodyPart::LeftFinger | BodyPart::RightFinger | BodyPart::BothFingers => {
                if slot_empty(self, pd::LFINGER) {
                    set(self, pd::LFINGER, this, changed);
                } else if slot_empty(self, pd::RFINGER) {
                    set(self, pd::RFINGER, this, changed);
                } else {
                    set(self, pd::LFINGER, this, changed);
                }
            }
            BodyPart::Neck => set(self, pd::NECK, this, changed),
            BodyPart::FullArmor => {
                set(self, pd::LEGS, None, changed);
                set(self, pd::CHEST, this, changed);
            }
            BodyPart::Chest => set(self, pd::CHEST, this, changed),
            BodyPart::Legs => {
                let chest_is_full_armor = self
                    .paperdoll_object(id, pd::CHEST)
                    .and_then(|c| self.inventory_item(id, c))
                    .and_then(|c| templates.and_then(|t| t.get(c.item_id)))
                    .is_some_and(|t| t.body_part == BodyPart::FullArmor);
                if chest_is_full_armor {
                    set(self, pd::CHEST, None, changed);
                }
                set(self, pd::LEGS, this, changed);
            }
            BodyPart::Feet => set(self, pd::FEET, this, changed),
            BodyPart::Gloves => set(self, pd::GLOVES, this, changed),
            BodyPart::Head => set(self, pd::HEAD, this, changed),
            BodyPart::Hair => {
                let hair_all = self
                    .paperdoll_object(id, pd::HAIR)
                    .and_then(|h| self.inventory_item(id, h))
                    .and_then(|h| templates.and_then(|t| t.get(h.item_id)))
                    .is_some_and(|t| t.body_part == BodyPart::HairAll);
                if hair_all {
                    set(self, pd::HAIR2, None, changed);
                } else {
                    set(self, pd::HAIR, None, changed);
                }
                set(self, pd::HAIR, this, changed);
            }
            BodyPart::Hair2 => {
                let hair_all = self
                    .paperdoll_object(id, pd::HAIR)
                    .and_then(|h| self.inventory_item(id, h))
                    .and_then(|h| templates.and_then(|t| t.get(h.item_id)))
                    .is_some_and(|t| t.body_part == BodyPart::HairAll);
                if hair_all {
                    set(self, pd::HAIR, None, changed);
                } else {
                    set(self, pd::HAIR2, None, changed);
                }
                set(self, pd::HAIR2, this, changed);
            }
            BodyPart::HairAll => {
                set(self, pd::HAIR2, None, changed);
                set(self, pd::HAIR, this, changed);
            }
            BodyPart::Underwear => set(self, pd::UNDER, this, changed),
            BodyPart::Back => set(self, pd::CLOAK, this, changed),
            BodyPart::LeftBracelet => set(self, pd::LBRACELET, this, changed),
            BodyPart::RightBracelet => set(self, pd::RBRACELET, this, changed),
            BodyPart::Deco => self.equip_talisman(id, &item, changed),
            BodyPart::Belt => set(self, pd::BELT, this, changed),
            BodyPart::AllDress => {
                set(self, pd::LEGS, None, changed);
                set(self, pd::LHAND, None, changed);
                set(self, pd::RHAND, None, changed);
                set(self, pd::HEAD, None, changed);
                set(self, pd::FEET, None, changed);
                set(self, pd::GLOVES, None, changed);
                set(self, pd::CHEST, this, changed);
            }
            other => {
                tracing::warn!("Unknown body slot {other:?} for Item ID: {}", item.item_id);
            }
        }
    }

    /// Java `Inventory.equipTalisman(item)`.
    fn equip_talisman(&mut self, id: ObjectId, item: &ItemData, changed: &mut Vec<ObjectId>) {
        let slots = usize::try_from(self.stats(id).map_or(0, |v| v.talisman_slots())).unwrap_or(0);
        if slots == 0 {
            return;
        }
        let last = (pd::DECO1 + slots).min(pd::DECO6 + 1);
        // Find same (or incompatible) talisman type.
        for slot in pd::DECO1..last {
            let same = self
                .paperdoll_object(id, slot)
                .and_then(|o| self.inventory_item(id, o))
                .is_some_and(|worn| worn.item_id == item.item_id);
            if same {
                // overwrite
                self.set_paperdoll_item(id, slot, Some(item.object_id), changed);
                return;
            }
        }
        // no free slot found - put on first free
        for slot in pd::DECO1..last {
            if self.paperdoll_object(id, slot).is_none() {
                self.set_paperdoll_item(id, slot, Some(item.object_id), changed);
                return;
            }
        }
        // no free slots - put on first
        self.set_paperdoll_item(id, pd::DECO1, Some(item.object_id), changed);
    }

    /// Java `Inventory.equipItemAndRecord(item)`.
    pub(crate) fn equip_item_and_record(&mut self, id: ObjectId, object_id: ObjectId) -> Vec<ObjectId> {
        let mut changed = Vec::new();
        self.equip_item(id, object_id, &mut changed);
        changed
    }

    /// Java `Inventory.unEquipItemInSlotAndRecord(slot)`: always followed by
    /// `refreshExpertisePenalty()`.
    pub(crate) fn unequip_item_in_slot_and_record(&mut self, id: ObjectId, slot: usize) -> Vec<ObjectId> {
        let mut changed = Vec::new();
        self.set_paperdoll_item(id, slot, None, &mut changed);
        self.refresh_expertise(id);
        changed
    }

    /// Java `Inventory.unEquipItemInBodySlot(bodyPart)`: `refreshExpertisePenalty()` only when
    /// something came off.
    pub(crate) fn unequip_item_in_body_slot(
        &mut self,
        id: ObjectId,
        part: BodyPart,
        changed: &mut Vec<ObjectId>,
    ) -> Option<ObjectId> {
        // Special handling for HAIRALL.
        if part == BodyPart::HairAll {
            self.set_paperdoll_item(id, pd::HAIR, None, changed);
        }
        let slot = paperdoll_index_of(part)?;
        let old = self.set_paperdoll_item(id, slot, None, changed);
        if old.is_some() {
            self.refresh_expertise(id);
        }
        old
    }

    /// Java `Inventory.unEquipItemInBodySlotAndRecord(bodyPart)`.
    pub(crate) fn unequip_item_in_body_slot_and_record(
        &mut self,
        id: ObjectId,
        part: BodyPart,
    ) -> Vec<ObjectId> {
        let mut changed = Vec::new();
        self.unequip_item_in_body_slot(id, part, &mut changed);
        changed
    }

    /// Java `ItemTemplate.checkCondition(player, player, sendMessage)`: the item's pre-conditions
    /// hold for the player; otherwise the failing condition's message goes to the player.
    pub(crate) fn check_item_condition(
        &self,
        id: ObjectId,
        template: &ItemTemplate,
        send_message: bool,
    ) -> bool {
        if self.is_gm(id) && !self.settings.inventory.gm_item_restriction {
            return true;
        }
        // The olympiad restriction (`isOlyRestrictedItem() || _heroItem`) needs the olympiad.
        if !template.is_condition_attached() {
            return true;
        }
        let Some(view) = self.stats(id) else {
            return true;
        };
        let env = CondEnv {
            actor: &view,
            target: Some(id),
            skill: None,
        };
        for pre in &template.conditions {
            if conditions::test(&pre.condition, &env) {
                continue;
            }
            if send_message {
                if let Some(text) = &pre.message.message {
                    self.send_message(id, text);
                } else if pre.message.message_id != 0
                    && let Some(message_id) = SystemMessageId::from_id(pre.message.message_id)
                {
                    let mut sm = SystemMessage::new(message_id);
                    if pre.message.add_name {
                        sm = sm.with(SystemMessageParam::ItemName(template.id));
                    }
                    self.send(id, &sm);
                }
            }
            return false;
        }
        true
    }

    /// Java `Player.checkItemRestriction()`: worn items whose conditions no longer hold come off
    /// (a cloak that needs its armor set, ...).
    pub(crate) fn check_item_restriction(&mut self, id: ObjectId, changed: &mut Vec<ObjectId>) {
        let templates = self.item_templates();
        for slot in 0..pd::TOTAL_SLOTS {
            let Some(worn_id) = self.paperdoll_object(id, slot) else {
                continue;
            };
            let Some(worn) = self.inventory_item(id, worn_id).cloned() else {
                continue;
            };
            let Some(template) = templates.as_deref().and_then(|t| t.get(worn.item_id)) else {
                continue;
            };
            if self.check_item_condition(id, template, false) {
                continue;
            }
            self.set_paperdoll_item(id, slot, None, changed);
            if let Some(entry) = self.modified_entry(id, worn_id) {
                self.send_inventory_update(id, entry);
            }
            if template.body_part == BodyPart::Back {
                self.send(
                    id,
                    &SystemMessage::new(
                        SystemMessageId::YourCloakHasBeenUnequippedBecauseYourArmorSetIsNoLongerComplete,
                    ),
                );
                return;
            }
            self.send(
                id,
                &self.disarmed_message(worn.enchant_level, worn.item_id, templates.as_deref()),
            );
        }
    }

    /// The message of an item that came off: "The equipment +$s1 $s2 has been removed." for an
    /// enchanted one, else "$s1 has been disarmed."
    pub(crate) fn disarmed_message(
        &self,
        enchant_level: i32,
        item_id: i32,
        templates: Option<&l2_data::ItemTemplates>,
    ) -> SystemMessage {
        let name = item_name_param(templates, item_id);
        if enchant_level > 0 {
            SystemMessage::new(SystemMessageId::TheEquipmentS1S2HasBeenRemoved)
                .with(SystemMessageParam::IntNumber(enchant_level))
                .with(name)
        } else {
            SystemMessage::new(SystemMessageId::S1HasBeenDisarmed).with(name)
        }
    }
}
