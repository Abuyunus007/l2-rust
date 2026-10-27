//! The spawns of a fortress: the always spawned NPCs, the suspicious merchant, the NPC commanders
//! of peace time, the special envoys of the castles, and the siege guards.
// Java: mechanics/siege/Fort.java (initNpcs, initSiegeNpcs, initNpcCommanders, initSpecialEnvoys,
//       spawnSuspiciousMerchant, despawnSuspiciousMerchant, spawnNpcCommanders,
//       despawnNpcCommanders, spawnSpecialEnvoys, getCastleIdByAmbassador, getCastleByAmbassador,
//       isBorderFortress)
// Java: managers/FortSiegeGuardManager.java (loadSiegeGuard, spawnSiegeGuard, unspawnSiegeGuard)
// Java: entity/spawns/Spawn.java (setRespawnDelay, doSpawn, startRespawn, stopRespawn,
//       getLastSpawn)
//
// A Java `Spawn` that is created and kept (`new Spawn(npcId)` with an amount of one, a place and
// a delay of 60 seconds) is an entry of the runtime spawns here (`respawn.rs`); the fortress holds
// its index. `doSpawn(false)` is `initialize_npc`, `getLastSpawn().deleteMe()` is `delete_npc`.

use std::collections::{BTreeMap, BTreeSet};

use l2_data::Location;
use l2_data::spawn::NpcSpawn;
use l2_util::ObjectId;
use tracing::warn;

use super::load::{FortSiegeGuardRecord, FortSpawnRecord};
use crate::npc::SpawnReport;
use crate::residence::{CastleId, FortId};
use crate::world::World;

/// `fort_spawnlist.spawnType`: always spawned.
pub const SPAWN_TYPE_ALWAYS: i32 = 0;
/// Despawned during a siege (the NPC commanders of peace time).
pub const SPAWN_TYPE_COMMANDER: i32 = 1;
/// Despawned ten minutes before a siege (the suspicious merchant).
pub const SPAWN_TYPE_SIEGE_NPC: i32 = 2;
/// Spawned after the fortress was taken (the envoys of the castles).
pub const SPAWN_TYPE_ENVOY: i32 = 3;

/// The respawn delay Java gives every spawn of the fort tables (`setRespawnDelay(60)`), seconds.
const FORT_RESPAWN_DELAY: i32 = 60;

/// What `Fort` keeps of its spawns.
#[derive(Debug, Default)]
pub(crate) struct FortSpawns {
    /// `_isSuspiciousMerchantSpawned`.
    pub(crate) suspicious_merchant_spawned: bool,
    /// `_siegeNpcs`.
    pub(crate) siege_npcs: Vec<usize>,
    /// `_npcCommanders`.
    pub(crate) npc_commanders: Vec<usize>,
    /// `_specialEnvoys`.
    pub(crate) special_envoys: Vec<usize>,
    /// `_envoyCastles`: envoy NPC id to castle id.
    pub(crate) envoy_castles: BTreeMap<i32, i32>,
    /// `_availableCastles`.
    pub(crate) available_castles: BTreeSet<i32>,
    /// `FortSiegeGuardManager._siegeGuards`: the spawn and its respawn delay (seconds).
    pub(crate) siege_guards: Vec<(usize, i32)>,
    /// The spawns `FortSiege.spawnCommanders` made, kept for the next siege (Java makes new
    /// `Spawn` objects each time; the entries here are reused, which behaves the same).
    pub(crate) commander_spawn_pool: Vec<usize>,
}

impl World {
    /// Java `new Spawn(npcId)` with `setAmount(1)`, `setXYZ`, `setHeading` and
    /// `setRespawnDelay(delay)`: the spawn is made, nothing stands there yet. `delay` is in
    /// seconds (0: no respawn).
    pub(crate) fn fort_new_spawn(
        &mut self,
        npc_id: i32,
        position: (i32, i32, i32),
        heading: i32,
        respawn_delay: i32,
    ) -> usize {
        // `setRespawnDelay(delay)`: `Math.max(1, delay) * 1000`, or none for 0.
        let delay_ms = if respawn_delay == 0 {
            0
        } else {
            respawn_delay.max(1).saturating_mul(1000)
        };
        let index = crate::respawn::RUNTIME_SPAWN_BASE + self.death.runtime_spawns.len();
        self.death.runtime_spawns.push(NpcSpawn {
            npc_id,
            amount: 1,
            location: Location {
                x: position.0,
                y: position.1,
                z: position.2,
            },
            heading,
            respawn_min_ms: delay_ms,
            respawn_max_ms: delay_ms,
            // A new `Spawn` has `_doRespawn = true`.
            respawn_enabled: true,
            chase_range: 0,
            name: None,
            territory: None,
            period_of_day: None,
            ai_data: None,
            group: None,
            source: "fortress".to_owned(),
        });
        index
    }

    /// Java `spawnDat.doSpawn(false)`.
    pub(crate) fn fort_do_spawn(&mut self, spawn: usize) -> Option<ObjectId> {
        let mut report = SpawnReport::default();
        self.initialize_npc(spawn, None, &mut report)
    }

    /// `spawnDat.doSpawn(false); spawnDat.startRespawn();`.
    pub(crate) fn fort_spawn_with_respawn(&mut self, spawn: usize) {
        self.fort_do_spawn(spawn);
        self.start_respawn(spawn);
    }

    /// `spawnDat.stopRespawn(); spawnDat.getLastSpawn().deleteMe();`
    pub(crate) fn fort_despawn(&mut self, spawn: usize) {
        self.stop_respawn(spawn);
        if let Some(npc) = self.last_spawn(spawn) {
            self.delete_npc(npc);
        }
    }

    // ------------------------------------------------------------------ the Fort constructor

    /// Java `initNpcs()`: the NPCs that always stand there are made and spawned.
    pub(crate) fn fort_init_npcs(&mut self, rows: &[FortSpawnRecord]) {
        for row in rows.iter().filter(|r| r.spawn_type == SPAWN_TYPE_ALWAYS) {
            let spawn =
                self.fort_new_spawn(row.npc_id, (row.x, row.y, row.z), row.heading, FORT_RESPAWN_DELAY);
            self.fort_spawn_with_respawn(spawn);
        }
    }

    /// `initSiegeNpcs()`, `initNpcCommanders()` and `initSpecialEnvoys()`: the spawns are made and
    /// kept. Nothing stands there yet.
    pub(crate) fn fort_init_spawn_lists(&mut self, fort: FortId, rows: &[FortSpawnRecord]) {
        let mut spawns = super::spawns::FortSpawns::default();
        for row in rows {
            let make = |world: &mut World| {
                world.fort_new_spawn(row.npc_id, (row.x, row.y, row.z), row.heading, FORT_RESPAWN_DELAY)
            };
            match row.spawn_type {
                SPAWN_TYPE_SIEGE_NPC => spawns.siege_npcs.push(make(self)),
                SPAWN_TYPE_COMMANDER => spawns.npc_commanders.push(make(self)),
                SPAWN_TYPE_ENVOY => {
                    spawns.special_envoys.push(make(self));
                    spawns.envoy_castles.insert(row.npc_id, row.castle_id);
                    spawns.available_castles.insert(row.castle_id);
                }
                _ => {}
            }
        }
        let state = self.fort.state_mut(fort);
        // Keep the guards and the pool if they were loaded already.
        spawns.siege_guards = std::mem::take(&mut state.spawns.siege_guards);
        spawns.commander_spawn_pool = std::mem::take(&mut state.spawns.commander_spawn_pool);
        state.spawns = spawns;
    }

    // ------------------------------------------------------------------ Fort's spawn methods

    /// Java `spawnSuspiciousMerchant()`.
    pub(crate) fn fort_spawn_suspicious_merchant(&mut self, fort: FortId) {
        let state = self.fort.state_mut(fort);
        if state.spawns.suspicious_merchant_spawned {
            return;
        }
        state.spawns.suspicious_merchant_spawned = true;
        for spawn in state.spawns.siege_npcs.clone() {
            self.fort_spawn_with_respawn(spawn);
        }
    }

    /// Java `despawnSuspiciousMerchant()`.
    pub(crate) fn fort_despawn_suspicious_merchant(&mut self, fort: FortId) {
        let state = self.fort.state_mut(fort);
        if !state.spawns.suspicious_merchant_spawned {
            return;
        }
        state.spawns.suspicious_merchant_spawned = false;
        for spawn in state.spawns.siege_npcs.clone() {
            self.fort_despawn(spawn);
        }
    }

    /// Java `spawnNpcCommanders()`.
    pub(crate) fn fort_spawn_npc_commanders(&mut self, fort: FortId) {
        for spawn in self.fort.state_mut(fort).spawns.npc_commanders.clone() {
            self.fort_spawn_with_respawn(spawn);
        }
    }

    /// Java `despawnNpcCommanders()`.
    pub(crate) fn fort_despawn_npc_commanders(&mut self, fort: FortId) {
        for spawn in self.fort.state_mut(fort).spawns.npc_commanders.clone() {
            self.fort_despawn(spawn);
        }
    }

    /// Java `spawnSpecialEnvoys()`.
    pub(crate) fn fort_spawn_special_envoys(&mut self, fort: FortId) {
        for spawn in self.fort.state_mut(fort).spawns.special_envoys.clone() {
            self.fort_spawn_with_respawn(spawn);
        }
    }

    /// Java `getCastleIdByAmbassador(npcId)`: -1 for an NPC that is no envoy of the fortress.
    pub fn fort_castle_id_by_ambassador(&self, fort: FortId, npc_id: i32) -> i32 {
        self.fort
            .forts
            .get(&fort)
            .and_then(|s| s.spawns.envoy_castles.get(&npc_id))
            .copied()
            .unwrap_or(-1)
    }

    /// Java `getCastleByAmbassador(npcId)`.
    pub fn fort_castle_by_ambassador(&self, fort: FortId, npc_id: i32) -> Option<CastleId> {
        let id = CastleId(self.fort_castle_id_by_ambassador(fort, npc_id));
        self.castle(id).map(|c| c.id)
    }

    /// Java `isBorderFortress()`: the envoys of more than one castle.
    pub fn fort_is_border_fortress(&self, fort: FortId) -> bool {
        self.fort
            .forts
            .get(&fort)
            .is_some_and(|s| s.spawns.available_castles.len() > 1)
    }

    /// Java `getContractedCastle()`.
    pub fn fort_contracted_castle(&self, fort: FortId) -> Option<CastleId> {
        let id = self.fort(fort)?.castle_id;
        self.castle(CastleId(id)).map(|c| c.id)
    }

    // ------------------------------------------------------------------ the siege guards

    /// Java `FortSiegeGuardManager.loadSiegeGuard()`.
    pub(crate) fn fort_load_siege_guards(&mut self, fort: FortId, rows: &[FortSiegeGuardRecord]) {
        let mut guards = Vec::with_capacity(rows.len());
        for row in rows {
            let spawn =
                self.fort_new_spawn(row.npc_id, (row.x, row.y, row.z), row.heading, row.respawn_delay);
            guards.push((spawn, row.respawn_delay));
        }
        self.fort.state_mut(fort).spawns.siege_guards = guards;
    }

    /// Java `FortSiegeGuardManager.spawnSiegeGuard()`.
    pub(crate) fn fort_spawn_siege_guards(&mut self, fort: FortId) {
        for (spawn, delay) in self.fort.state_mut(fort).spawns.siege_guards.clone() {
            self.fort_do_spawn(spawn);
            // `getRespawnDelay() == 0`.
            if delay == 0 {
                self.stop_respawn(spawn);
            } else {
                self.start_respawn(spawn);
            }
        }
    }

    /// Java `FortSiegeGuardManager.unspawnSiegeGuard()`: the guards are killed (`doDie` by
    /// themselves), not deleted.
    pub(crate) fn fort_unspawn_siege_guards(&mut self, fort: FortId) {
        for (spawn, _) in self.fort.state_mut(fort).spawns.siege_guards.clone() {
            self.stop_respawn(spawn);
            if let Some(npc) = self.last_spawn(spawn) {
                if self.npcs.contains_key(&npc) {
                    self.do_die(npc, Some(npc));
                } else {
                    warn!("{fort}: the siege guard {npc} is gone already.");
                }
            }
        }
    }
}
