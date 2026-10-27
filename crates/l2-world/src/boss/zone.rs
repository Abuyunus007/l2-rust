//! Java `entity/zone/type/BossZone.java` and the zone registry of `GrandBossManager`: the players a
//! boss zone lets in (`grandboss_list`), the rule that sends everybody else out, and the lookups
//! the managers offer (`getZone(creature)`, `getZone(x, y, z)`, `checkIfInZone`).
//
// Players only: Java's `onEnter` also removes the summon of a player who may not enter and
// `onExit` sends a raid boss home, but NPCs and summons have no zone state in the world.

use std::collections::HashMap;

use l2_data::Zone;
use l2_util::ObjectId;
use tracing::{info, warn};

use crate::command::LeaveReason;
use crate::teleport::{TeleportTarget, TeleportWhereType};
use crate::world::{World, unix_ms_now};

/// The zone type name of Java's `BossZone`.
pub(crate) const BOSS_ZONE: &str = "BossZone";

/// Java `BossZone` with its `Settings`.
#[derive(Debug, Clone, Default)]
pub(crate) struct BossZoneState {
    /// `InvadeTime` (`_timeInvade`), milliseconds.
    pub(crate) time_invade: i32,
    /// `oustX`, `oustY`, `oustZ`; (0, 0, 0) sends players to the town.
    pub(crate) oust: (i32, i32, i32),
    /// `_playersAllowed`.
    pub(crate) allowed: Vec<i32>,
    /// `_playerAllowedReEntryTimes`: player to the time until which the re-entry is allowed.
    pub(crate) re_entry: HashMap<i32, i64>,
}

impl BossZoneState {
    fn of(zone: &Zone) -> Self {
        // `Integer.parseInt(value)` of the parameters: a bad number throws while the zone loads.
        Self {
            time_invade: zone.int_param("InvadeTime").unwrap_or(0),
            oust: (
                zone.int_param("oustX").unwrap_or(0),
                zone.int_param("oustY").unwrap_or(0),
                zone.int_param("oustZ").unwrap_or(0),
            ),
            allowed: Vec::new(),
            re_entry: HashMap::new(),
        }
    }

    fn has_oust(&self) -> bool {
        self.oust.0 != 0 && self.oust.1 != 0 && self.oust.2 != 0
    }
}

impl World {
    /// The `BossZone` constructors (`addZone`) and `GrandBossManager.initZones()`: the zones of
    /// the zone table, and the players of `grandboss_list` each of them lets in.
    pub(crate) fn load_boss_zones(&mut self, list: &[(i32, i32)]) {
        self.boss.zones.clear();
        let Some(zones) = self.zoning.zones.clone() else {
            return;
        };
        for zone in zones.all_of_type(BOSS_ZONE) {
            self.boss.zones.insert(zone.id, BossZoneState::of(zone));
        }
        let mut lists: HashMap<i32, Vec<i32>> = self.boss.zones.keys().map(|id| (*id, Vec::new())).collect();
        for (player, zone) in list {
            match lists.get_mut(zone) {
                Some(players) => players.push(*player),
                None => {
                    // NOTE(java-quirk): `zones.get(zone).add(..)` throws a NullPointerException,
                    // the catch is outside the loop and the rows after this one are dropped.
                    warn!("Error while initializing GrandBoss zones: no boss zone {zone}");
                    break;
                }
            }
        }
        info!("GrandBossManager: Initialized {} Grand Boss Zones", self.boss.zones.len());
        for (id, players) in lists {
            if let Some(state) = self.boss.zones.get_mut(&id) {
                state.allowed = players;
            }
        }
    }

    /// The rows of `grandboss_list` as `GrandBossManager.storeMe` writes them: the players of each
    /// zone, zones with nobody left out.
    pub(crate) fn boss_zone_list(&self) -> Vec<(i32, i32)> {
        self.boss
            .zones
            .iter()
            .flat_map(|(zone, state)| state.allowed.iter().map(move |player| (*player, *zone)))
            .collect()
    }

    /// Java `GrandBossManager.getZone(x, y, z)`: the boss zone that holds the point.
    pub fn grand_boss_zone_at(&self, x: i32, y: i32, z: i32) -> Option<i32> {
        let zones = self.zoning.zones.as_ref()?;
        self.boss
            .zones
            .keys()
            .find(|id| zones.get_of_type(**id, BOSS_ZONE).is_some_and(|zone| zone.is_inside_zone(x, y, z)))
            .copied()
    }

    /// Java `GrandBossManager.getZone(creature)`: the boss zone the player stands in
    /// (`isCharacterInZone`).
    pub fn grand_boss_zone_of(&self, id: ObjectId) -> Option<i32> {
        let player = self.players.get(&id)?;
        self.boss
            .zones
            .keys()
            .find(|zone| player.zones.members().iter().any(|(zid, ty)| zid == *zone && ty == BOSS_ZONE))
            .copied()
    }

    /// Java `checkIfInZone(player)`.
    pub fn grand_boss_in_zone(&self, id: ObjectId) -> bool {
        self.players
            .get(&id)
            .is_some_and(|p| self.grand_boss_zone_at(p.core.x, p.core.y, p.core.z).is_some())
    }

    /// Java `BossZone.getAllowedPlayers()`.
    pub fn boss_zone_allowed(&self, zone: i32) -> Option<&[i32]> {
        self.boss.zones.get(&zone).map(|s| s.allowed.as_slice())
    }

    /// Java `BossZone.allowPlayerEntry(player, durationInSec)`.
    pub fn allow_boss_zone_entry(&mut self, zone: i32, player: ObjectId, duration_secs: i32) {
        if self.is_gm(player) {
            return;
        }
        let Some(state) = self.boss.zones.get_mut(&zone) else {
            return;
        };
        if !state.allowed.contains(&player.0) {
            state.allowed.push(player.0);
        }
        state
            .re_entry
            .insert(player.0, unix_ms_now() + i64::from(duration_secs) * 1000);
    }

    /// Java `BossZone.removePlayer(player)`.
    pub fn remove_from_boss_zone_lists(&mut self, zone: i32, player: ObjectId) {
        if self.is_gm(player) {
            return;
        }
        if let Some(state) = self.boss.zones.get_mut(&zone) {
            state.allowed.retain(|p| *p != player.0);
            state.re_entry.remove(&player.0);
        }
    }

    /// Java `Player.removeFromBossZone()`: every zone forgets the player.
    pub fn remove_from_boss_zones(&mut self, player: ObjectId) {
        let zones: Vec<i32> = self.boss.zones.keys().copied().collect();
        for zone in zones {
            self.remove_from_boss_zone_lists(zone, player);
        }
    }

    /// Java `BossZone.isPlayerAllowed(player)`: a player who may not be here is sent out.
    pub fn is_boss_zone_player_allowed(&mut self, zone: i32, player: ObjectId) -> bool {
        let Some(state) = self.boss.zones.get(&zone) else {
            return false;
        };
        if self.is_gm(player) || state.allowed.contains(&player.0) {
            return true;
        }
        self.oust_from_boss_zone(zone, player);
        false
    }

    /// The teleport out: `oustX/Y/Z` when all three are set, else the nearest town.
    fn oust_from_boss_zone(&mut self, zone: i32, player: ObjectId) {
        let oust = self.boss.zones.get(&zone).map(|s| (s.has_oust(), s.oust));
        match oust {
            Some((true, (x, y, z))) => self.tele_to_location(player, TeleportTarget::at(x, y, z)),
            _ => self.tele_to_location_where(player, TeleportWhereType::Town),
        }
    }

    /// Java `BossZone.oustAllPlayers()`: everybody inside is sent out and the lists are cleared.
    pub fn oust_all_boss_zone_players(&mut self, zone: i32) {
        let inside: Vec<ObjectId> = self
            .players
            .values()
            .filter(|p| p.zones.members().iter().any(|(zid, ty)| *zid == zone && ty == BOSS_ZONE))
            .map(|p| p.core.object_id)
            .filter(|id| self.sinks.contains_key(id))
            .collect();
        if inside.is_empty() {
            return;
        }
        for player in inside {
            self.oust_from_boss_zone(zone, player);
        }
        if let Some(state) = self.boss.zones.get_mut(&zone) {
            state.re_entry.clear();
            state.allowed.clear();
        }
    }

    /// Java `BossZone.onEnter(creature)` for a player: "teleport out all players who attempt
    /// illegal (re-)entry".
    pub(crate) fn boss_zone_enter(&mut self, player: ObjectId, zone: i32) {
        if self.is_gm(player) {
            return;
        }
        let now = unix_ms_now();
        let started = self.boss.started_ms;
        let Some(state) = self.boss.zones.get_mut(&zone) else {
            return;
        };
        // A player cleared by an AI (or listed before a restart) may come in.
        if state.allowed.contains(&player.0) {
            match state.re_entry.get(&player.0).copied() {
                // Legal null expiration entries: the server booted less than `InvadeTime` ago.
                None => {
                    if started > now - i64::from(state.time_invade) {
                        return;
                    }
                }
                Some(expiration) => {
                    state.re_entry.remove(&player.0);
                    if expiration > now {
                        return;
                    }
                }
            }
            if let Some(index) = state.allowed.iter().position(|p| *p == player.0) {
                state.allowed.remove(index);
            }
        }
        self.oust_from_boss_zone(zone, player);
    }

    /// Java `BossZone.onExit(creature)` for a player who walked out or left the world.
    /// `online` is `Player.isOnline()` at that moment.
    pub(crate) fn boss_zone_exit(&mut self, player: ObjectId, zone: i32, online: bool) {
        if self.is_gm(player) {
            return;
        }
        let now = unix_ms_now();
        let Some(state) = self.boss.zones.get_mut(&zone) else {
            return;
        };
        let listed = state.allowed.contains(&player.0);
        if !online && listed {
            // "mark the time that the player left the zone"
            state
                .re_entry
                .insert(player.0, now + i64::from(state.time_invade));
        } else {
            state.allowed.retain(|p| *p != player.0);
            state.re_entry.remove(&player.0);
        }
    }

    /// `Disconnection` / `deleteMe`: the zones of a player who leaves the world. `Logout`,
    /// `RequestRestart` call `removeFromBossZone()` first (Java), a lost connection does not, so
    /// the player keeps its place for `InvadeTime`.
    pub(crate) fn boss_zone_player_leaving(&mut self, id: ObjectId, reason: LeaveReason) {
        if self.boss.zones.is_empty() {
            return;
        }
        if matches!(reason, LeaveReason::Logout | LeaveReason::Restart) {
            self.remove_from_boss_zones(id);
        }
        let inside: Vec<i32> = self
            .players
            .get(&id)
            .map(|p| {
                p.zones
                    .members()
                    .iter()
                    .filter(|(_, ty)| ty == BOSS_ZONE)
                    .map(|(zid, _)| *zid)
                    .collect()
            })
            .unwrap_or_default();
        for zone in inside {
            self.boss_zone_exit(id, zone, false);
        }
    }
}
