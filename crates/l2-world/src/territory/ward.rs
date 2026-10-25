//! The territory wards: the places of a territory's wards, the ward NPC (who may hit it, the
//! damage it takes, its death), the ward item and its carrier, the transport to an outpost and
//! the residential skills of the wards a territory owns.
// Java: managers/TerritoryWarManager.java (addTerritoryWard, getTerritoryWard, dropCombatFlag,
//       Territory.getFreeWardSpawnPlace, Territory.removeWard, updateTerritoryData)
// Java: mechanics/siege/TerritoryWard.java (spawnBack, spawnMe, unSpawnMe, activate, dropIt)
// Java: entity/actor/instance/TerritoryWard.java (isAutoAttackable, reduceCurrentHp,
//       reduceCurrentHpByDOT, doDie, onAction)
// Java: mechanics/siege/Castle.java (giveResidentialSkills, removeResidentialSkills: the ward part)
// Java: entity/actor/Player.java (addItem: the ward branch)
// Java: dist/game/data/scripts/quests/TerritoryWarScripts/TerritoryWarSuperClass.java (onSkillSee)

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ActionFailed, InventoryUpdate, SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use tracing::warn;

use super::{OUTPOST_NPC_ID, TERRITORY_ID_BASE, TerritoryWrite, WARD_ITEM_BASE, WARD_NPC_BASE};
use crate::inventory::item_name_param;
use crate::residence::{CastleId, residential_skills_of};
use crate::world::World;

/// `PlayerInventory` location names.
const LOC_PAPERDOLL: &str = "PAPERDOLL";
/// Skill 845 (Outpost Demolition) and 847 (Ward Transport) of `TerritoryWarSuperClass.onSkillSee`.
const OUTPOST_DEMOLITION: i32 = 845;
const WARD_TRANSPORT: i32 = 847;

/// A ward place whose NPC Java decayed (`decayMe()`): the place is taken, the NPC is not in the
/// world. No NPC ever has object id 0.
const DECAYED: ObjectId = ObjectId(0);

impl World {
    // ------------------------------------------------------------------ lookups

    /// Java `getTerritoryWard(territoryId)` (and `getTerritoryWardForOwner`): the index into
    /// `_territoryWards`.
    pub(crate) fn territory_ward_index(&self, territory_id: i32) -> Option<usize> {
        self.territory
            .wards
            .iter()
            .position(|w| w.territory_id == territory_id)
    }

    /// Java `getTerritoryWard(player)`: the ward the player carries.
    pub(crate) fn territory_ward_of_player(&self, player: ObjectId) -> Option<usize> {
        self.territory.wards.iter().position(|w| w.player == Some(player))
    }

    /// Whether the player carries a ward (Java's `_combatFlagEquippedId` of a ward carrier).
    pub fn is_ward_carrier(&self, player: ObjectId) -> bool {
        self.territory_ward_of_player(player).is_some()
    }

    /// The ward NPC's name without " Ward" (`getName().replace(" Ward", "")`), by territory.
    fn ward_short_name(&self, territory_id: i32) -> String {
        self.npc_world
            .template(WARD_NPC_BASE + territory_id)
            .map(|t| t.name.replace(" Ward", ""))
            .unwrap_or_default()
    }

    // ------------------------------------------------------------------ the places

    /// Java `Territory.getFreeWardSpawnPlace()`.
    fn territory_free_ward_place(&self, castle_id: i32) -> Option<usize> {
        let territory = self.territory.territories.get(&castle_id)?;
        if let Some(index) = territory.ward_places.iter().position(|p| p.npc.is_none()) {
            return Some(index);
        }
        warn!(
            "Territory: no free Ward spawn found for territory: {}",
            territory.territory_id
        );
        for (i, place) in territory.ward_places.iter().enumerate() {
            match place.npc {
                Some(npc) => warn!(
                    "Territory: territory ward spawn place {i} has npc name: {}",
                    self.creature_name(npc)
                ),
                None => warn!("Territory: territory ward spawn place {i} is empty!"),
            }
        }
        None
    }

    /// `wardSpawn.setNPC(wardSpawn.getNpc().getSpawn().doSpawn(false))`: the ward of the place
    /// stands there again (a new NPC; the old one is deleted).
    pub(crate) fn territory_respawn_ward_place(&mut self, castle_id: i32, index: usize) -> Option<ObjectId> {
        let place = self
            .territory
            .territories
            .get(&castle_id)?
            .ward_places
            .get(index)?
            .clone();
        if let Some(old) = place.npc {
            self.delete_npc(old);
        }
        let npc = self.spawn_npc_at(
            WARD_NPC_BASE + place.npc_id,
            place.x,
            place.y,
            place.z,
            place.heading,
        );
        if let Some(slot) = self
            .territory
            .territories
            .get_mut(&castle_id)
            .and_then(|t| t.ward_places.get_mut(index))
        {
            slot.npc = npc;
        }
        npc
    }

    /// Java `Territory.removeWard(wardId)`.
    fn territory_remove_ward(&mut self, castle_id: i32, ward_id: i32) {
        let Some(index) = self
            .territory
            .territories
            .get(&castle_id)
            .and_then(|t| t.ward_places.iter().position(|p| p.npc_id == ward_id))
        else {
            let territory_id = castle_id + TERRITORY_ID_BASE;
            warn!("Territory: Can't delete wardId: {ward_id} for territory: {territory_id}");
            return;
        };
        let npc = self
            .territory
            .territories
            .get(&castle_id)
            .and_then(|t| t.ward_places.get(index))
            .and_then(|p| p.npc);
        if let Some(npc) = npc {
            self.delete_npc(npc);
        }
        if let Some(place) = self
            .territory
            .territories
            .get_mut(&castle_id)
            .and_then(|t| t.ward_places.get_mut(index))
        {
            place.npc = None;
            place.npc_id = 0;
        }
    }

    /// Java `updateTerritoryData(ter)`: the owned ward ids of the territory, `"81;85;"`.
    fn territory_update_data(&mut self, castle_id: i32) {
        let Some(territory) = self.territory.territories.get(&castle_id) else {
            return;
        };
        let ward_ids: String = territory
            .owned_ward_ids()
            .iter()
            .map(|id| format!("{id};"))
            .collect();
        let territory_id = territory.territory_id;
        self.territory_write(TerritoryWrite::OwnedWardIds {
            territory_id,
            ward_ids,
        });
    }

    /// Java `addTerritoryWard(territoryId, newOwnerId, oldOwnerId, broadcastMessage)`: the ward
    /// goes to a free place of the new owner's territory, the new owner's members get the ward
    /// skills, the old owner loses the ward, its skills and the rows follow. Returns the new NPC.
    pub(crate) fn territory_add_ward(
        &mut self,
        territory_id: i32,
        new_owner: i32,
        old_owner: i32,
        broadcast: bool,
    ) -> Option<ObjectId> {
        if !self.territory.territories.contains_key(&new_owner) {
            warn!("TerritoryWarManager: Missing territory for new Ward owner: {new_owner};{territory_id}");
            return None;
        }
        let mut ret = None;
        if let Some(index) = self.territory_free_ward_place(new_owner) {
            let in_world =
                self.territory.in_progress || self.territory.settings.spawn_wards_when_tw_is_not_in_progress;
            let npc = if in_world {
                let place = self
                    .territory
                    .territories
                    .get(&new_owner)
                    .and_then(|t| t.ward_places.get(index))
                    .cloned();
                place.and_then(|p| self.spawn_npc_at(WARD_NPC_BASE + territory_id, p.x, p.y, p.z, p.heading))
            } else {
                // `ret.decayMe()`: the place holds an NPC that is not in the world.
                Some(DECAYED)
            };
            if let Some(place) = self
                .territory
                .territories
                .get_mut(&new_owner)
                .and_then(|t| t.ward_places.get_mut(index))
            {
                place.npc_id = territory_id;
                place.npc = npc;
            }
            ret = npc.filter(|n| *n != DECAYED);
            // The new owner's members get the skills of every ward the territory owns, once it
            // owns its own ward.
            let owner = self
                .territory
                .territories
                .get(&new_owner)
                .and_then(|t| t.owner_clan);
            let owned = self
                .territory
                .territories
                .get(&new_owner)
                .map(|t| t.owned_ward_ids())
                .unwrap_or_default();
            if let Some(owner) = owner
                && owned.contains(&(new_owner + TERRITORY_ID_BASE))
            {
                for ward_id in owned {
                    for member in self.clan_online_members(owner, 0) {
                        // M8: `!member.isInOlympiadMode()`.
                        self.territory_add_ward_skills(member, ward_id);
                    }
                }
            }
        }
        if self.territory.territories.contains_key(&old_owner) {
            self.territory_remove_ward(old_owner, territory_id);
            self.territory_update_data(old_owner);
            self.territory_update_data(new_owner);
            if broadcast {
                let new_owner_name = self
                    .territory
                    .territories
                    .get(&new_owner)
                    .and_then(|t| t.owner_clan)
                    .and_then(|c| self.clan(c))
                    .map(|c| c.name.clone());
                match new_owner_name {
                    Some(name) => {
                        let sm = SystemMessage::new(
                            SystemMessageId::ClanS1HasSucceededInCapturingS2STerritoryWard,
                        )
                        .with(SystemMessageParam::Text(name))
                        .with(SystemMessageParam::CastleName(territory_id));
                        self.tw_announce_to_participants(&sm, 135_000, 13_500);
                    }
                    // `terNew.getOwnerClan().getName()` throws without an owner.
                    None => {
                        warn!("TerritoryWarManager: NullPointerException: no owner of territory {new_owner}")
                    }
                }
            }
            let old_clan = self
                .territory
                .territories
                .get(&old_owner)
                .and_then(|t| t.owner_clan);
            if let Some(old_clan) = old_clan {
                for member in self.clan_online_members(old_clan, 0) {
                    self.territory_remove_ward_skills(member, territory_id);
                }
                let owned = self
                    .territory
                    .territories
                    .get(&old_owner)
                    .map(|t| t.owned_ward_ids())
                    .unwrap_or_default();
                if !owned.is_empty() && !owned.contains(&(old_owner + TERRITORY_ID_BASE)) {
                    for ward_id in owned {
                        for member in self.clan_online_members(old_clan, 0) {
                            self.territory_remove_ward_skills(member, ward_id);
                        }
                    }
                }
            }
        }
        ret
    }

    /// The wards a territory owns as the database has them now (side by side): the places follow
    /// and the owner's online members get their ward skills again. Outside the war only.
    pub(crate) fn territory_apply_owned_wards(&mut self, castle_id: i32, wards: &[i32]) {
        let Some(current) = self
            .territory
            .territories
            .get(&castle_id)
            .map(|t| t.owned_ward_ids())
        else {
            return;
        };
        if current == wards {
            return;
        }
        let owner = self
            .territory
            .territories
            .get(&castle_id)
            .and_then(|t| t.owner_clan);
        let members = owner.map(|c| self.clan_online_members(c, 0)).unwrap_or_default();
        for member in &members {
            self.territory_remove_castle_ward_skills(castle_id, *member);
        }
        let spawn_wards = self.territory.settings.spawn_wards_when_tw_is_not_in_progress;
        let places = self
            .territory
            .territories
            .get(&castle_id)
            .map_or(0, |t| t.ward_places.len());
        for index in 0..places {
            let npc = self
                .territory
                .territories
                .get(&castle_id)
                .and_then(|t| t.ward_places.get(index))
                .and_then(|p| p.npc);
            if let Some(npc) = npc {
                self.delete_npc(npc);
            }
            if let Some(place) = self
                .territory
                .territories
                .get_mut(&castle_id)
                .and_then(|t| t.ward_places.get_mut(index))
            {
                place.npc = None;
                place.npc_id = 0;
            }
        }
        for (index, ward) in wards.iter().enumerate().take(places) {
            if let Some(place) = self
                .territory
                .territories
                .get_mut(&castle_id)
                .and_then(|t| t.ward_places.get_mut(index))
            {
                place.npc_id = *ward;
                place.npc = Some(DECAYED);
            }
            if spawn_wards {
                self.territory_respawn_ward_place(castle_id, index);
            }
        }
        for member in &members {
            self.territory_give_castle_ward_skills(castle_id, *member);
            self.send_skill_list(*member);
        }
    }

    // ------------------------------------------------------------------ the ward skills

    /// The residential skills of one ward (`getAvailableResidentialSkills(wardId)`), not stored.
    fn territory_add_ward_skills(&mut self, player: ObjectId, ward_id: i32) -> bool {
        let trees = std::sync::Arc::clone(&self.rewards.skill_trees);
        let mut added = false;
        for (skill_id, level) in residential_skills_of(&trees, ward_id) {
            added |= self.add_skill(player, skill_id, level);
        }
        added
    }

    fn territory_remove_ward_skills(&mut self, player: ObjectId, ward_id: i32) {
        let trees = std::sync::Arc::clone(&self.rewards.skill_trees);
        for (skill_id, _) in residential_skills_of(&trees, ward_id) {
            self.remove_skill(player, skill_id, true);
        }
    }

    /// The ward part of `Castle.giveResidentialSkills(player)`: the skills of every ward the
    /// territory owns, once it owns its own. Returns whether the player got any.
    pub(crate) fn territory_give_castle_ward_skills(&mut self, castle_id: i32, player: ObjectId) -> bool {
        let Some(owned) = self
            .territory
            .territories
            .get(&castle_id)
            .map(|t| t.owned_ward_ids())
        else {
            return false;
        };
        if !owned.contains(&(castle_id + TERRITORY_ID_BASE)) {
            return false;
        }
        let mut added = false;
        for ward_id in owned {
            added |= self.territory_add_ward_skills(player, ward_id);
        }
        added
    }

    /// The ward part of `Castle.removeResidentialSkills(player)`: the skills of every ward the
    /// territory owns.
    pub(crate) fn territory_remove_castle_ward_skills(&mut self, castle_id: i32, player: ObjectId) {
        let Some(owned) = self
            .territory
            .territories
            .get(&castle_id)
            .map(|t| t.owned_ward_ids())
        else {
            return;
        };
        for ward_id in owned {
            self.territory_remove_ward_skills(player, ward_id);
        }
    }

    // ------------------------------------------------------------------ the ward mechanic

    /// Java `TerritoryWard.spawnBack()`: the ward stands where it was picked up.
    fn territory_ward_spawn_back(&mut self, index: usize) {
        if self
            .territory
            .wards
            .get(index)
            .is_some_and(|w| w.player.is_some())
        {
            self.territory_ward_drop_it(index);
        }
        let Some(ward) = self.territory.wards.get(index).cloned() else {
            return;
        };
        // NOTE(java-quirk): `_oldLocation` is null for a ward nobody picked up; Java's spawn then
        // throws inside `spawnNPC` (caught and logged) and the ward has no NPC.
        let npc = ward
            .old_location
            .and_then(|(x, y, z, h)| self.spawn_npc_at(WARD_NPC_BASE + ward.territory_id, x, y, z, h));
        if let Some(w) = self.territory.wards.get_mut(index) {
            w.npc = npc;
        }
    }

    /// Java `TerritoryWard.spawnMe()`: the ward stands where its carrier was last.
    pub(crate) fn territory_ward_spawn_me(&mut self, index: usize) {
        if self
            .territory
            .wards
            .get(index)
            .is_some_and(|w| w.player.is_some())
        {
            self.territory_ward_drop_it(index);
        }
        let Some(ward) = self.territory.wards.get(index).cloned() else {
            return;
        };
        let (x, y, z, h) = ward.location;
        let npc = self.spawn_npc_at(WARD_NPC_BASE + ward.territory_id, x, y, z, h);
        if let Some(w) = self.territory.wards.get_mut(index) {
            w.npc = npc;
        }
    }

    /// Java `TerritoryWard.unSpawnMe()`: the carrier drops it, the NPC goes.
    pub(crate) fn territory_ward_unspawn_me(&mut self, index: usize) {
        if self
            .territory
            .wards
            .get(index)
            .is_some_and(|w| w.player.is_some())
        {
            self.territory_ward_drop_it(index);
        }
        if let Some(npc) = self.territory.wards.get(index).and_then(|w| w.npc) {
            self.delete_npc(npc);
        }
    }

    /// Java `TerritoryWard.activate(player, item)`: the player wears the ward item, or (mounted,
    /// not registered) loses it and the ward stands where he is. `false` when he does not wear it.
    fn territory_ward_activate(&mut self, index: usize, player: ObjectId, item: ObjectId) -> bool {
        if self.is_mounted(player) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouDoNotMeetTheRequiredConditionToEquipThatItem),
            );
            self.territory_destroy_ward_item(player, item);
            self.territory_ward_spawn_me(index);
            return false;
        } else if self.registered_territory_id(player) == 0 {
            self.send_message(player, "Non participants can't pickup Territory Wards!");
            self.territory_destroy_ward_item(player, item);
            self.territory_ward_spawn_me(index);
            return false;
        }
        // Player holding it data.
        let old_location = self
            .territory
            .wards
            .get(index)
            .and_then(|w| w.npc)
            .and_then(|npc| {
                let (x, y, z) = self.position_of(npc)?;
                Some((x, y, z, self.heading_of(npc)))
            });
        let territory_id = match self.territory.wards.get_mut(index) {
            Some(w) => {
                w.player = Some(player);
                // NOTE(java-quirk): Java reads the coordinates of the ward NPC, which is decayed
                // by now but keeps them; a ward without an NPC throws here.
                if old_location.is_some() {
                    w.old_location = old_location;
                }
                w.npc = None;
                w.item = Some(item);
                w.territory_id
            }
            None => return false,
        };
        // Equip with the weapon.
        self.equip_item_and_record(player, item);
        let item_id = self
            .inventory_item(player, item)
            .map_or(territory_id + WARD_ITEM_BASE, |i| i.item_id);
        let templates = self.item_templates();
        self.send(
            player,
            &SystemMessage::new(SystemMessageId::YouHaveEquippedYourS1)
                .with(item_name_param(templates.as_deref(), item_id)),
        );
        // Refresh inventory.
        if let Some(entry) = self.modified_entry(player, item) {
            self.send_inventory_update(player, entry);
        }
        // Refresh player stats.
        self.broadcast_user_info(player);
        self.send(
            player,
            &SystemMessage::new(SystemMessageId::YouVeAcquiredTheWardMoveQuicklyToYourForcesOutpost),
        );
        self.territory_give_point(player, territory_id, 5);
        true
    }

    /// Java `TerritoryWard.dropIt()`: the worn item comes off and is destroyed; the ward will
    /// stand where the carrier is.
    fn territory_ward_drop_it(&mut self, index: usize) {
        let Some(ward) = self.territory.wards.get_mut(index) else {
            return;
        };
        let (Some(player), item) = (ward.player.take(), ward.item.take()) else {
            return;
        };
        if let Some(item) = item {
            self.territory_destroy_ward_item(player, item);
        }
        self.broadcast_user_info(player);
        let location = self
            .position_of(player)
            .map(|(x, y, z)| (x, y, z, self.heading_of(player)));
        if let (Some(location), Some(w)) = (location, self.territory.wards.get_mut(index)) {
            w.location = location;
        }
    }

    /// `unEquipItemInBodySlot(bodyPart)` and `destroyItem(DESTROY, item, null, true)`.
    fn territory_destroy_ward_item(&mut self, player: ObjectId, item: ObjectId) {
        let Some(worn) = self.inventory_item(player, item).cloned() else {
            return;
        };
        if worn.loc == LOC_PAPERDOLL {
            self.unequip_item_in_slot_and_record(player, usize::try_from(worn.loc_data).unwrap_or(0));
        }
        let Some(removed) = self.destroy_inventory_item(player, item, worn.count) else {
            return;
        };
        let entry = self.update_entry(&removed, InventoryUpdate::REMOVED);
        self.send_inventory_update_now(player, vec![entry]);
        self.send_cur_load(player);
        let templates = self.item_templates();
        self.send(
            player,
            &SystemMessage::new(SystemMessageId::S1HasDisappeared)
                .with(item_name_param(templates.as_deref(), removed.item_id)),
        );
    }

    /// The ward branch of `Player.addItem`: a ward item that came into the inventory is worn.
    pub(crate) fn territory_ward_item_added(&mut self, player: ObjectId, item: ObjectId, item_id: i32) {
        if let Some(index) = self.territory_ward_index(item_id - WARD_ITEM_BASE) {
            self.territory_ward_activate(index, player, item);
        }
    }

    /// Java `dropCombatFlag(player, isKilled, isSpawnBack)`: the carrier loses the ward; during
    /// the war it stands where he died, where it was picked up, or back on its place.
    pub fn territory_drop_combat_flag(&mut self, player: ObjectId, is_killed: bool, is_spawn_back: bool) {
        let indices: Vec<usize> = self
            .territory
            .wards
            .iter()
            .enumerate()
            .filter(|(_, w)| w.player == Some(player))
            .map(|(i, _)| i)
            .collect();
        for index in indices {
            self.territory_ward_drop_it(index);
            if self.territory.in_progress {
                if is_killed {
                    self.territory_ward_spawn_me(index);
                } else if is_spawn_back {
                    self.territory_ward_spawn_back(index);
                } else {
                    self.territory_ward_return_to_place(index);
                }
            }
            if is_killed {
                let territory_id = self.territory.wards.get(index).map_or(0, |w| w.territory_id);
                let sm = SystemMessage::new(SystemMessageId::TheCharacterThatAcquiredS1SWardHasBeenKilled)
                    .with(SystemMessageParam::Text(self.ward_short_name(territory_id)));
                self.tw_announce_to_participants(&sm, 0, 0);
            }
        }
    }

    /// The `else` of `dropCombatFlag` and the own-territory branch of the ward transport: the ward
    /// stands on its place of the territory that holds it again.
    fn territory_ward_return_to_place(&mut self, index: usize) {
        let Some(ward) = self.territory.wards.get(index).cloned() else {
            return;
        };
        let place = self
            .territory
            .territories
            .get(&ward.owner_castle_id)
            .and_then(|t| t.ward_places.iter().position(|p| p.npc_id == ward.territory_id));
        if let Some(place) = place {
            let npc = self.territory_respawn_ward_place(ward.owner_castle_id, place);
            self.territory_ward_unspawn_me(index);
            if let Some(w) = self.territory.wards.get_mut(index) {
                w.npc = npc;
            }
        }
    }

    // ------------------------------------------------------------------ the ward NPC

    fn is_ward_npc(&self, npc: ObjectId) -> bool {
        self.npc_template(npc).is_some_and(|t| t.is_type("TerritoryWard"))
    }

    /// The castle the ward NPC stands at, with its siege zone active.
    fn ward_active_castle(&self, npc: ObjectId) -> Option<CastleId> {
        self.npc_castle(npc).filter(|c| {
            self.castle(*c)
                .and_then(|c| c.siege_zone)
                .is_some_and(|z| self.is_siege_zone_active(z))
        })
    }

    /// Java `TerritoryWard.isAutoAttackable(attacker)`; `None` for an NPC that is no ward.
    pub(crate) fn territory_ward_auto_attackable(&self, npc: ObjectId, attacker: ObjectId) -> Option<bool> {
        if !self.is_ward_npc(npc) {
            return None;
        }
        if self.is_invul(npc) {
            return Some(false);
        }
        let Some(castle) = self.ward_active_castle(npc) else {
            return Some(false);
        };
        let Some(player) = self.acting_player(attacker) else {
            return Some(false);
        };
        if self.siege_side_of(player) == 0 {
            return Some(false);
        }
        Some(!self.is_ally_field(player, castle.0))
    }

    /// Java `TerritoryWard.reduceCurrentHp` for a hit without a skill: `true` when the ward takes
    /// no damage.
    pub(crate) fn territory_ward_refuses_damage(&self, npc: ObjectId, attacker: Option<ObjectId>) -> bool {
        if !self.is_ward_npc(npc) {
            return false;
        }
        if !self.territory.in_progress {
            return true;
        }
        let Some(player) = attacker.and_then(|a| self.acting_player(a)) else {
            return true;
        };
        if self.is_combat_flag_equipped(player) || self.siege_side_of(player) == 0 {
            return true;
        }
        let Some(castle) = self.npc_castle(npc) else {
            return true;
        };
        self.is_ally_field(player, castle.0)
    }

    /// Java `TerritoryWard.reduceCurrentHp(.., skill)` with a skill, and `reduceCurrentHpByDOT`:
    /// a ward takes no skill damage at all.
    pub(crate) fn territory_ward_refuses_skill_damage(&self, npc: ObjectId) -> bool {
        self.is_ward_npc(npc)
    }

    /// Java `TerritoryWard.onAction(player, interact)` (and `onForcedAttack`): `true` when the NPC
    /// is a ward and the click is handled.
    pub(crate) fn territory_ward_click(&mut self, player: ObjectId, npc: ObjectId, interact: bool) -> bool {
        if !self.is_ward_npc(npc) {
            return false;
        }
        if self.target_of(player) != Some(npc) {
            // Set the target of the Player player.
            self.set_target(player, Some(npc));
        } else if interact {
            let dz = match (self.position_of(player), self.position_of(npc)) {
                (Some(p), Some(n)) => (p.2 - n.2).abs(),
                _ => i32::MAX,
            };
            if self.territory_ward_auto_attackable(npc, player) == Some(true) && dz < 100 {
                self.set_intention_attack(player, npc);
            } else {
                self.send(player, &ActionFailed);
            }
        }
        true
    }

    /// Java `TerritoryWard.doDie(killer)` after `Attackable.doDie`: during the war a player on a
    /// side without a flag takes the ward item; anybody else puts the ward where it was.
    pub(crate) fn territory_ward_do_die(&mut self, npc: ObjectId, killer: Option<ObjectId>) {
        if !self.is_ward_npc(npc) {
            return;
        }
        if self.npc_castle(npc).is_none() || !self.territory.in_progress {
            return;
        }
        let npc_id = self.npcs.get(&npc).map_or(0, |n| n.npc_id);
        let territory_id = npc_id - WARD_NPC_BASE;
        let ward = self.territory_ward_index(territory_id);
        match killer.filter(|k| self.is_player(*k)) {
            Some(player) => {
                if self.siege_side_of(player) > 0 && !self.is_combat_flag_equipped(player) {
                    // `addItem(PICKUP, getId() - 23012, 1, null, false)`: the ward item.
                    self.add_item(player, territory_id + WARD_ITEM_BASE, 1, false);
                } else {
                    match ward {
                        Some(index) => self.territory_ward_spawn_me(index),
                        None => warn!("TerritoryWard: NullPointerException: no ward {territory_id}"),
                    }
                }
                let name = self
                    .players
                    .get(&player)
                    .map(|p| p.data.name.clone())
                    .unwrap_or_default();
                let sm =
                    SystemMessage::new(SystemMessageId::TheS1WardHasBeenDestroyedC2NowHasTheTerritoryWard)
                        .with(SystemMessageParam::Text(
                            self.creature_name(npc).replace(" Ward", ""),
                        ))
                        .with(SystemMessageParam::PlayerName(name));
                self.tw_announce_to_participants(&sm, 0, 0);
            }
            None => match ward {
                Some(index) => self.territory_ward_spawn_me(index),
                None => warn!("TerritoryWard: NullPointerException: no ward {territory_id}"),
            },
        }
        // `decayMe()`.
        self.delete_npc(npc);
    }

    // ------------------------------------------------------------------ the outpost skills

    /// `TerritoryWarSuperClass.onSkillSee` of the outpost (36590): Outpost Demolition (845) by
    /// the owner clan takes the outpost down, Ward Transport (847) brings the carried ward home
    /// or into the caster's territory.
    pub fn territory_on_skill_see(
        &mut self,
        npc: ObjectId,
        caster: ObjectId,
        skill_id: i32,
        targets: &[ObjectId],
    ) {
        if self.npcs.get(&npc).is_none_or(|n| n.npc_id != OUTPOST_NPC_ID) || !targets.contains(&npc) {
            return;
        }
        match skill_id {
            OUTPOST_DEMOLITION => {
                // `getHQForClan(caster.getClan())` throws for a caster without a clan.
                let Some(clan) = self.clan_of(caster).map(|c| c.id) else {
                    return;
                };
                if self.territory_hq_for_clan(clan) != Some(npc) {
                    return;
                }
                self.delete_npc(npc);
                self.territory_set_hq_for_clan(clan, None);
            }
            WARD_TRANSPORT => {
                let side = self.siege_side_of(caster);
                if self.territory_hq(side) != Some(npc) {
                    return;
                }
                let Some(index) = self.territory_ward_of_player(caster) else {
                    return;
                };
                let Some(ward) = self.territory.wards.get(index).cloned() else {
                    return;
                };
                let caster_castle = side - TERRITORY_ID_BASE;
                if caster_castle == ward.owner_castle_id {
                    self.territory_ward_return_to_place(index);
                } else {
                    self.territory_ward_unspawn_me(index);
                    let npc =
                        self.territory_add_ward(ward.territory_id, caster_castle, ward.owner_castle_id, true);
                    if let Some(w) = self.territory.wards.get_mut(index) {
                        w.npc = npc;
                        w.owner_castle_id = caster_castle;
                    }
                    if let Some(t) = self.territory.territories.get_mut(&caster_castle) {
                        t.quest_done[1] += 1;
                    }
                }
            }
            _ => {}
        }
    }
}
