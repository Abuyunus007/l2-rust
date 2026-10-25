//! Fortresses: the fortress table, the owner, the flag pole, the supply level and the state.
//! The siege, the spawns and `FortUpdater` are `fort/` (M7-FORT-01).
// Java: managers/FortManager.java
// Java: mechanics/siege/Fort.java (setOwner, removeOwner, updateOwnerInDB, setOwnerClan,
//       setVisibleFlag, raiseSupplyLeveL, setSupplyLeveL, saveFortVariables, setFortState,
//       updateClansReputation, getOwnedTime, checkIfInZone, getDistance)
// Java: entity/actor/Npc.java (getFort)
//
// The siege of a fortress, `FortUpdater`, the fort spawns (merchants, commanders, envoys) and the
// castle contracts are `fort/` (M7-FORT-01); this file calls them where Java's `Fort` does.

use std::collections::BTreeMap;

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{PlaySound, SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use tracing::warn;

use super::{FortId, ResidenceRef, ResidenceWrite, Writes};
use crate::world::{World, unix_ms_now};

/// A fortress (Java `Fort` without its siege and spawns).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fort {
    pub id: FortId,
    /// `name` of the `fort` row ("Shanty" ... "Monastic").
    pub name: String,
    /// `_fortOwner`: the owning clan's id, 0 for none.
    pub owner_id: i32,
    /// `_siegeDate` (epoch milliseconds), for the siege card.
    pub siege_date: i64,
    /// `_lastOwnedTime` (epoch milliseconds, 0 without an owner).
    pub last_owned_time: i64,
    /// `_fortType`: 0 small (3 commanders), 1 big (4 commanders and a control room).
    pub fort_type: i32,
    /// `_state`: 0 not decided yet, 1 independent, 2 contracted with a castle.
    pub state: i32,
    /// `_castleId`: the castle the fortress is contracted with (0 for none).
    pub castle_id: i32,
    /// `_supplyLeveL`.
    pub supply_level: i32,
    /// `_function`: the current function objects by type (see `functions.rs`).
    pub(crate) functions: BTreeMap<i32, u64>,
    /// `_doors`: the doors inside the fortress's siege zone (`activateInstance`).
    pub doors: Vec<ObjectId>,
    /// `_flagPole`: the static object of type 3 whose name starts with the fortress name.
    pub flag_pole: Option<ObjectId>,
    /// The `FortZone` (Java `getResidenceZone()`).
    pub zone: Option<i32>,
    /// The `SiegeZone` (Java `getZone()`).
    pub siege_zone: Option<i32>,
    /// `_residentialSkills`: `(id, level)`.
    pub residential_skills: Vec<(i32, i32)>,
}

impl Fort {
    /// A fortress with Java's field defaults.
    pub fn new(id: FortId) -> Self {
        Self {
            id,
            name: String::new(),
            owner_id: 0,
            siege_date: 0,
            last_owned_time: 0,
            fort_type: 0,
            state: 0,
            castle_id: 0,
            supply_level: 0,
            functions: BTreeMap::new(),
            doors: Vec::new(),
            flag_pole: None,
            zone: None,
            siege_zone: None,
            residential_skills: Vec::new(),
        }
    }

    /// Java `getFortSize()`: the barracks of the fortress.
    pub fn fort_size(&self) -> i32 {
        if self.fort_type == 0 { 3 } else { 5 }
    }

    /// Java `getOwnedTime()`: seconds since the owner took it (0 without an owner).
    pub fn owned_time(&self, now_ms: i64) -> i32 {
        if self.last_owned_time == 0 {
            0
        } else {
            ((now_ms - self.last_owned_time) / 1000) as i32
        }
    }
}

impl World {
    // ------------------------------------------------------------------ FortManager lookups

    /// Java `FortManager.getFortById(fortId)`.
    pub fn fort(&self, id: FortId) -> Option<&Fort> {
        self.residences.forts.iter().find(|f| f.id == id)
    }

    pub(crate) fn fort_mut(&mut self, id: FortId) -> Option<&mut Fort> {
        self.residences.forts.iter_mut().find(|f| f.id == id)
    }

    /// Java `FortManager.getForts()`, in Java's list order.
    pub fn forts(&self) -> &[Fort] {
        &self.residences.forts
    }

    /// Java `FortManager.getFortByOwner(clan)`.
    pub fn fort_by_owner(&self, clan_id: i32) -> Option<FortId> {
        if clan_id <= 0 {
            return None;
        }
        self.residences
            .forts
            .iter()
            .find(|f| f.owner_id == clan_id)
            .map(|f| f.id)
    }

    /// Java `FortManager.getFort(name)`: ignoring case, the name trimmed.
    pub fn fort_by_name(&self, name: &str) -> Option<FortId> {
        let name = name.trim();
        self.residences
            .forts
            .iter()
            .find(|f| f.name.eq_ignore_ascii_case(name))
            .map(|f| f.id)
    }

    /// Java `FortManager.getFortIndex(fortId)`.
    pub fn fort_index(&self, id: FortId) -> Option<usize> {
        self.residences.forts.iter().position(|f| f.id == id)
    }

    /// Java `Fort.checkIfInZone(x, y, z)`: inside the fortress's siege zone.
    pub fn fort_contains(&self, id: FortId, x: i32, y: i32, z: i32) -> bool {
        let Some(zone) = self.fort(id).and_then(|f| f.siege_zone) else {
            return false;
        };
        self.zoning
            .zones
            .as_ref()
            .and_then(|zones| zones.get_of_type(zone, "SiegeZone"))
            .is_some_and(|zone| zone.is_inside_zone(x, y, z))
    }

    /// Java `FortManager.getFort(x, y, z)` / `getFortIndex(x, y, z)`.
    pub fn fort_at(&self, x: i32, y: i32, z: i32) -> Option<FortId> {
        self.residences
            .forts
            .iter()
            .find(|f| self.fort_contains(f.id, x, y, z))
            .map(|f| f.id)
    }

    /// Java `Fort.getDistance(obj)` (`None` without a siege zone, where Java throws).
    pub fn fort_distance(&self, id: FortId, x: i32, y: i32) -> Option<f64> {
        let zone = self.fort(id)?.siege_zone?;
        let zone = self.zoning.zones.as_ref()?.get_of_type(zone, "SiegeZone")?;
        Some(zone.distance_to_zone(x, y))
    }

    /// Java `FortManager.findNearestFortIndex(obj, maxDistance)`.
    pub fn nearest_fort(&self, x: i32, y: i32, z: i32, max_distance: i64) -> Option<FortId> {
        if let Some(id) = self.fort_at(x, y, z) {
            return Some(id);
        }
        let mut max = max_distance;
        let mut found = None;
        for fort in &self.residences.forts {
            let Some(distance) = self.fort_distance(fort.id, x, y) else {
                continue;
            };
            if (max as f64) > distance {
                max = distance as i64;
                found = Some(fort.id);
            }
        }
        found
    }

    /// Java `Npc.getFort()`: the fortress whose siege zone holds the NPC, else the nearest one.
    pub fn npc_fort(&self, npc: ObjectId) -> Option<FortId> {
        let (x, y, z) = self.position_of(npc)?;
        self.nearest_fort(x, y, z, i64::MAX)
    }

    // ------------------------------------------------------------------ flag, supplies, state

    /// Java `Fort.setVisibleFlag(value)`: the flag on the flag pole (`setMeshIndex(1 / 0)`).
    pub fn fort_set_visible_flag(&mut self, id: FortId, value: bool) {
        if let Some(pole) = self.fort(id).and_then(|f| f.flag_pole) {
            self.set_static_object_mesh_index(pole, i32::from(value));
        }
    }

    /// Java `Fort.setOwnerClan(clan)`: the owner, and the flag shows while there is one.
    fn fort_set_owner_clan(&mut self, id: FortId, clan: Option<i32>) {
        self.fort_set_visible_flag(id, clan.is_some());
        if let Some(fort) = self.fort_mut(id) {
            fort.owner_id = clan.unwrap_or(0);
        }
    }

    /// Java `Fort.raiseSupplyLeveL()`: one more, at most `FortressMaxSupplyLevel`.
    pub fn fort_raise_supply_level(&mut self, id: FortId) {
        let max = self.residences.settings.fs_max_supply_level;
        if let Some(fort) = self.fort_mut(id) {
            fort.supply_level = (fort.supply_level + 1).min(max);
        }
    }

    /// Java `Fort.setSupplyLeveL(value)`: ignored above `FortressMaxSupplyLevel`.
    pub fn fort_set_supply_level(&mut self, id: FortId, value: i32) {
        let max = self.residences.settings.fs_max_supply_level;
        if let Some(fort) = self.fort_mut(id)
            && value <= max
        {
            fort.supply_level = value;
        }
    }

    /// Java `Fort.saveFortVariables()`: the supply level row (`FortUpdater` and the owner changes
    /// are [`Writes::Job`]s, D-51).
    pub fn fort_save_variables(&mut self, id: FortId, writes: Writes) {
        let Some(supply_level) = self.fort(id).map(|f| f.supply_level) else {
            return;
        };
        let write = ResidenceWrite::FortSupplyLevel {
            fort_id: id.0,
            supply_level,
        };
        self.write_residence(writes, write);
    }

    /// Java `Fort.setFortState(state, castleId)`: 0 not decided, 1 independent, 2 contracted with
    /// `castle_id`. The reset of an owner change is a [`Writes::Job`]; the owner's choice at an
    /// envoy a [`Writes::Player`].
    pub fn fort_set_state(&mut self, id: FortId, state: i32, castle_id: i32, writes: Writes) {
        let Some(fort) = self.fort_mut(id) else {
            return;
        };
        fort.state = state;
        fort.castle_id = castle_id;
        let write = ResidenceWrite::FortState {
            fort_id: id.0,
            state,
            castle_id,
        };
        self.write_residence(writes, write);
    }

    /// Java `Fort.updateClansReputation(owner, removePoints)`.
    pub fn fort_update_clans_reputation(&mut self, clan_id: i32, remove_points: bool) {
        if self.clan(clan_id).is_none() {
            return;
        }
        if remove_points {
            let points = self.residences.settings.loose_fort_points;
            self.take_reputation_score(clan_id, points);
        } else {
            let points = self.residences.settings.take_fort_points;
            self.add_reputation_score(clan_id, points);
        }
    }

    // ------------------------------------------------------------------ owner

    /// Java `Fort.setOwner(clan, updateClansReputation)`: the end of a fortress siege. `false`
    /// when the clan already has a castle (the rebel army keeps the fortress).
    pub fn fort_set_owner(&mut self, id: FortId, clan_id: i32, update_clans_reputation: bool) -> bool {
        if self.clan(clan_id).is_none() {
            warn!("Fort: Updating Fort owner with null clan!!!");
            return false;
        }
        let Some(old_owner) = self.fort(id).map(|f| f.owner_id) else {
            return false;
        };
        self.fort_siege_announce(
            id,
            &SystemMessage::new(SystemMessageId::TheFortressBattleOfS1HasFinished)
                .with(SystemMessageParam::CastleName(id.0)),
        );
        if old_owner != 0 && old_owner != clan_id {
            // Remove points from old owner.
            self.fort_update_clans_reputation(old_owner, true);
            // The old leader on a wyvern dismounts.
            if let Some(leader) = self.clan(old_owner).and_then(|c| c.leader)
                && self.players.contains_key(&leader)
                && self.mount_type_of(leader) == crate::mount::MountType::Wyvern
            {
                self.dismount(leader);
            }
            if self.fort_siege_in_progress(id) {
                self.fort_update_player_siege_state_flags(id, true);
            }
            self.fort_remove_owner(id, true);
        }
        // Initialize fort state.
        self.fort_set_state(id, 0, 0, Writes::Job);
        // If clan already have castle, don't store him in fortress.
        if self.clan(clan_id).is_some_and(|c| c.castle_id > 0) {
            self.fort_siege_announce(
                id,
                &SystemMessage::new(SystemMessageId::TheRebelArmyRecapturedTheFortress),
            );
            return false;
        }
        // Give points to new owner.
        if update_clans_reputation {
            self.fort_update_clans_reputation(clan_id, false);
        }
        self.fort_spawn_special_envoys(id);
        // If clan have already fortress, remove it.
        // NOTE(java-quirk): a clan that already owns this fortress loses it here and gets it back
        // below.
        if self.clan(clan_id).is_some_and(|c| c.fort_id > 0)
            && let Some(other) = self.fort_by_owner(clan_id)
        {
            self.fort_remove_owner(other, true);
        }
        self.fort_set_supply_level(id, 0);
        self.fort_set_owner_clan(id, Some(clan_id));
        // Update in database.
        self.fort_update_owner_in_db(id, Writes::Job);
        self.fort_save_variables(id, Writes::Job);
        if self.fort_siege_in_progress(id) {
            self.fort_end_siege(id);
        }
        for member in self.clan_online_members(clan_id, 0) {
            self.give_residential_skills(ResidenceRef::Fort(id), member);
            self.send_skill_list(member);
        }
        true
    }

    /// Java `Fort.removeOwner(updateDB)`: the owner loses the fortress (residential skills,
    /// `PledgeShowInfoUpdate`), the flag comes down, the supply level and the functions go.
    pub fn fort_remove_owner(&mut self, id: FortId, update_db: bool) {
        self.fort_remove_owner_with(id, update_db, Writes::Job);
    }

    /// [`Self::fort_remove_owner`] with its rows written as `writes` says.
    pub(crate) fn fort_remove_owner_with(&mut self, id: FortId, update_db: bool, writes: Writes) {
        let Some(clan_id) = self.fort(id).map(|f| f.owner_id).filter(|c| *c != 0) else {
            return;
        };
        for member in self.clan_online_members(clan_id, 0) {
            self.remove_residential_skills(ResidenceRef::Fort(id), member);
            self.send_skill_list(member);
        }
        if let Some(clan) = self.clan_mut(clan_id) {
            clan.fort_id = 0;
        }
        self.broadcast_clan_info_update(clan_id);
        self.fort_set_owner_clan(id, None);
        self.fort_set_supply_level(id, 0);
        self.fort_save_variables(id, writes);
        self.residence_remove_all_functions_with(ResidenceRef::Fort(id), writes);
        if update_db {
            self.fort_update_owner_in_db(id, writes);
        }
    }

    /// Java `Fort.updateOwnerInDB()`: the owner row (D-51: only with `SiegeDatabaseWrites`), and
    /// for an owner its fortress id, the victory message to everybody, `PledgeShowInfoUpdate`
    /// and the `Siege_Victory` sound to its members.
    fn fort_update_owner_in_db(&mut self, id: FortId, writes: Writes) {
        let Some(owner) = self.fort(id).map(|f| f.owner_id) else {
            return;
        };
        let clan = self.clan(owner).map(|c| (c.id, c.name.clone()));
        let last_owned_time = if clan.is_some() { unix_ms_now() } else { 0 };
        if let Some(fort) = self.fort_mut(id) {
            fort.last_owned_time = last_owned_time;
        }
        let written = self.write_residence(
            writes,
            ResidenceWrite::FortOwner {
                fort_id: id.0,
                owner: clan.as_ref().map_or(0, |c| c.0),
                last_owned_time,
            },
        );
        self.set_residence_owner_source(ResidenceRef::Fort(id), written);
        // Announce to clan members.
        if let Some((clan_id, name)) = clan {
            if let Some(c) = self.clan_mut(clan_id) {
                // Set has fort flag for new owner.
                c.fort_id = id.0;
            }
            let message = SystemMessage::new(SystemMessageId::S1IsVictoriousInTheFortressBattleOfS2)
                .with(SystemMessageParam::Text(name))
                .with(SystemMessageParam::CastleName(id.0));
            let players: Vec<ObjectId> = self.players.keys().copied().collect();
            for player in players {
                self.send(player, &message);
            }
            self.broadcast_clan_info_update(clan_id);
            self.broadcast_to_clan(clan_id, &PlaySound::with_unknown1(1, "Siege_Victory"));
            // The two `FortUpdater` tasks are cancelled and scheduled again.
            self.fort_restart_updaters(id, Some(clan_id));
        } else {
            self.fort_restart_updaters(id, None);
        }
    }
}
