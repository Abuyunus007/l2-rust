//! `AutoSpawnHandler`: the NPCs of `random_spawn` that come and go on a timer (the Mammon
//! merchants, the Seven Signs preachers, orators and crests, Lilith and Anakim, a few quest NPCs),
//! and `SevenSigns.spawnSevenSignsNPC`, which turns the Seven Signs ones on and off.
// Java: entity/spawns/AutoSpawnHandler.java (registerSpawn, setSpawnActive, AutoSpawner,
//       AutoDespawner, getAutoSpawnInstance, getAutoSpawnInstances)
// Java: mechanics/sevensigns/SevenSigns.java (spawnSevenSignsNPC)
//
// The groups are read once at start (`restoreSpawnData`, read only). A registered group is
// active at once, like Java: its first spawn comes after the initial delay (30 seconds by
// default). Then `spawnSevenSignsNPC` turns the Seven Signs groups off again where the state says
// so. The spawned NPCs have no spawn entry, so they never respawn (Java's `stopRespawn`).
//
// Differences kept small and marked: the group keys are counters, not object ids (Java takes an
// id from `IdManager` per group); several NPCs of one group and the random places are worked out
// before the NPC appears instead of moving it right after the spawn.

use std::collections::BTreeMap;

use l2_protocol_game::server::{ChatType, CreatureSay, SayBody, SenderField};
use l2_util::ObjectId;
use tracing::{debug, info, warn};

use super::{
    ANAKIM_NPC_ID, CABAL_DAWN, CABAL_DUSK, CABAL_NULL, CREST_OF_DAWN_ID, CREST_OF_DUSK_ID, LILITH_NPC_ID,
    MAMMON_BLACKSMITH_ID, MAMMON_MARKETEER_ID, MAMMON_MERCHANT_ID, ORATOR_NPC_ID, PREACHER_NPC_ID,
    SEAL_AVARICE, SEAL_GNOSIS, SevenSignsTimer,
};
use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::World;

/// `DEFAULT_INITIAL_SPAWN`: 30 seconds after registration.
const DEFAULT_INITIAL_SPAWN: i64 = 30_000;
/// `DEFAULT_RESPAWN` / `DEFAULT_DESPAWN`: one hour.
const DEFAULT_RESPAWN: i64 = 3_600_000;
const DEFAULT_DESPAWN: i64 = 3_600_000;
/// How often a random place is tried again when another NPC stands there (Java tries forever).
const MAX_PLACE_TRIES: usize = 100;

/// One `random_spawn` group with its places, as the game server read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoSpawnGroup {
    pub npc_id: i32,
    pub count: i32,
    /// `initialDelay`, `respawnDelay`, `despawnDelay` in milliseconds (`< 0`: the default).
    pub initial_delay: i32,
    pub respawn_delay: i32,
    pub despawn_delay: i32,
    pub broadcast: bool,
    pub random_spawn: bool,
    /// `(x, y, z, heading)`; a heading of -1 is not set.
    pub locations: Vec<(i32, i32, i32, i32)>,
}

/// The tasks of one group (`TimerKind::SevenSigns(SevenSignsTimer::AutoSpawn(..))`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoSpawnTimer {
    /// `AutoSpawner.run()`.
    Spawn(u32),
    /// `AutoDespawner.run()`.
    Despawn(u32),
}

/// Java `AutoSpawnInstance`.
#[derive(Debug, Clone)]
struct AutoSpawnInstance {
    npc_id: i32,
    init_delay: i64,
    res_delay: i64,
    des_delay: i64,
    spawn_count: i32,
    last_loc_index: i32,
    npcs: Vec<ObjectId>,
    locations: Vec<(i32, i32, i32, i32)>,
    spawn_active: bool,
    random_spawn: bool,
    broadcast: bool,
    /// `_runningSpawns.get(objectId)`.
    task: Option<TimerId>,
}

/// `AutoSpawnHandler`'s state.
#[derive(Debug, Default)]
pub(crate) struct AutoSpawnWorld {
    /// `_registeredSpawns`, by key.
    instances: BTreeMap<u32, AutoSpawnInstance>,
    next_key: u32,
}

impl World {
    /// Java `AutoSpawnHandler.restoreSpawnData()`: every group of `random_spawn` is registered and
    /// active. Call once at startup, after the NPC templates.
    pub fn load_auto_spawns(&mut self, groups: Vec<AutoSpawnGroup>) {
        for g in groups {
            let key = self.register_auto_spawn(
                g.npc_id,
                i64::from(g.initial_delay),
                i64::from(g.respawn_delay),
                i64::from(g.despawn_delay),
            );
            if let Some(inst) = self.seven_signs.auto_spawn.instances.get_mut(&key) {
                inst.spawn_count = g.count;
                inst.broadcast = g.broadcast;
                inst.random_spawn = g.random_spawn;
                inst.locations = g.locations;
            }
        }
        info!(
            "AutoSpawnHandler: Loaded {} handlers in total.",
            self.seven_signs.auto_spawn.instances.len()
        );
    }

    /// The number of registered groups (Java `size()`).
    pub fn auto_spawn_count(&self) -> usize {
        self.seven_signs.auto_spawn.instances.len()
    }

    /// The NPCs the groups of `npc_id` have in the world now (tests, the admin pages).
    pub fn auto_spawn_npcs(&self, npc_id: i32) -> Vec<ObjectId> {
        self.seven_signs
            .auto_spawn
            .instances
            .values()
            .filter(|i| i.npc_id == npc_id)
            .flat_map(|i| i.npcs.iter().copied())
            .collect()
    }

    /// Java `getAutoSpawnInstance(npcId, false)` for the admin pages: the first group of
    /// `npc_id` as (its first NPC, `getTimeToNextSpawn` in milliseconds). `None` when no group
    /// is registered.
    pub fn auto_spawn_first_info(&self, npc_id: i32) -> Option<(Option<ObjectId>, i64)> {
        let inst = self
            .seven_signs
            .auto_spawn
            .instances
            .values()
            .find(|i| i.npc_id == npc_id)?;
        // A group without a running task answers 0.
        let millis = inst.task.map_or(0, |task| {
            self.timers.due_of(task).map_or(0, |due| {
                let ticks = i64::try_from(due).unwrap_or(i64::MAX) - i64::try_from(self.tick).unwrap_or(0);
                ticks.saturating_mul(i64::try_from(crate::world::TICK.as_millis()).unwrap_or(50))
            })
        });
        Some((inst.npcs.first().copied(), millis))
    }

    /// Whether the groups of `npc_id` are active (tests): one entry per group.
    pub fn auto_spawn_active(&self, npc_id: i32) -> Vec<bool> {
        self.seven_signs
            .auto_spawn
            .instances
            .values()
            .filter(|i| i.npc_id == npc_id)
            .map(|i| i.spawn_active)
            .collect()
    }

    /// Java `registerSpawn(npcId, initialDelay, respawnDelay, despawnDelay)`: the group is
    /// registered and set active.
    fn register_auto_spawn(&mut self, npc_id: i32, init: i64, res: i64, des: i64) -> u32 {
        let world = &mut self.seven_signs.auto_spawn;
        world.next_key = world.next_key.wrapping_add(1);
        let key = world.next_key;
        world.instances.insert(
            key,
            AutoSpawnInstance {
                npc_id,
                init_delay: if init < 0 { DEFAULT_INITIAL_SPAWN } else { init },
                res_delay: if res < 0 { DEFAULT_RESPAWN } else { res },
                des_delay: if des < 0 { DEFAULT_DESPAWN } else { des },
                spawn_count: 1,
                last_loc_index: -1,
                npcs: Vec::new(),
                locations: Vec::new(),
                spawn_active: false,
                random_spawn: false,
                broadcast: false,
                task: None,
            },
        );
        self.set_auto_spawn_active(key, true);
        key
    }

    fn auto_spawn_timer(&mut self, delay_ms: i64, timer: AutoSpawnTimer) -> TimerId {
        let due = self.tick + TimerWheel::ms_to_ticks(u64::try_from(delay_ms).unwrap_or(0));
        self.timers
            .schedule_at(due, TimerKind::SevenSigns(SevenSignsTimer::AutoSpawn(timer)))
    }

    /// Java `setSpawnActive(spawnInst, isActive)`.
    // NOTE: Java leaves an older spawn task running when a group is set active twice (only the
    // Black Marketeer of Mammon, which has no `random_spawn` rows); here the old one is replaced.
    fn set_auto_spawn_active(&mut self, key: u32, active: bool) {
        let Some(init) = self
            .seven_signs
            .auto_spawn
            .instances
            .get(&key)
            .map(|i| i.init_delay)
        else {
            return;
        };
        if active {
            let timer = self.auto_spawn_timer(init, AutoSpawnTimer::Spawn(key));
            let old = self
                .seven_signs
                .auto_spawn
                .instances
                .get_mut(&key)
                .and_then(|i| i.task.replace(timer));
            if let Some(old) = old {
                self.timers.cancel(old);
            }
        } else {
            let old = self
                .seven_signs
                .auto_spawn
                .instances
                .get_mut(&key)
                .and_then(|i| i.task.take());
            if let Some(old) = old {
                self.timers.cancel(old);
            }
            // `ThreadPool.schedule(new AutoDespawner(objectId), 0)`.
            self.auto_despawn(key);
        }
        if let Some(inst) = self.seven_signs.auto_spawn.instances.get_mut(&key) {
            inst.spawn_active = active;
        }
    }

    pub(crate) fn on_auto_spawn_timer(&mut self, timer: AutoSpawnTimer) {
        match timer {
            AutoSpawnTimer::Spawn(key) => self.auto_spawner_run(key),
            AutoSpawnTimer::Despawn(key) => self.auto_despawn(key),
        }
    }

    /// Java `AutoSpawner.run()`.
    fn auto_spawner_run(&mut self, key: u32) {
        let Some(inst) = self.seven_signs.auto_spawn.instances.get(&key).cloned() else {
            return;
        };
        // `scheduleAtFixedRate(rs, init, res)` when there is a despawn delay, else one run.
        if inst.des_delay > 0 {
            let timer = self.auto_spawn_timer(inst.res_delay, AutoSpawnTimer::Spawn(key));
            if let Some(i) = self.seven_signs.auto_spawn.instances.get_mut(&key) {
                i.task = Some(timer);
            }
        } else if let Some(i) = self.seven_signs.auto_spawn.instances.get_mut(&key) {
            i.task = None;
        }
        // If the spawn is not scheduled to be active, cancel the spawn task.
        if !inst.spawn_active {
            return;
        }
        // If there are no set co-ordinates, cancel the spawn task.
        if inst.locations.is_empty() {
            info!("AutoSpawnHandler: No location co-ords specified for spawn instance (Object ID = {key}).");
            return;
        }
        let count = i32::try_from(inst.locations.len()).unwrap_or(i32::MAX);
        let mut index = self.rng().below(count);
        // If random spawning is disabled, the spawn at the next set of co-ordinates after the last.
        if !inst.random_spawn {
            index = inst.last_loc_index.wrapping_add(1);
            if index == count {
                index = 0;
            }
            if let Some(i) = self.seven_signs.auto_spawn.instances.get_mut(&key) {
                i.last_loc_index = index;
            }
        }
        let place = usize::try_from(index)
            .ok()
            .and_then(|i| inst.locations.get(i))
            .copied();
        let Some((mut x, mut y, mut z, heading)) = place else {
            return;
        };
        // A random spawn does not land on another NPC (Java teleports it on until it is alone).
        if inst.random_spawn {
            for _ in 0..MAX_PLACE_TRIES {
                if !self.npc_within(x, y, z, 5) {
                    break;
                }
                let pick = self.rng().below(count);
                if let Some(&(nx, ny, nz, _)) = usize::try_from(pick).ok().and_then(|i| inst.locations.get(i))
                {
                    (x, y, z) = (nx, ny, nz);
                }
            }
        }
        // `newSpawn.setHeading(heading)` only when it is not -1; a `Spawn` starts with 0.
        let heading = if heading == -1 { 0 } else { heading };
        let mut spawned: Vec<ObjectId> = Vec::new();
        if inst.spawn_count == 1 {
            if let Some(id) = self.spawn_npc_at(inst.npc_id, x, y, z, heading) {
                spawned.push(id);
            }
        } else {
            for _ in 0..inst.spawn_count.max(0) {
                // "To prevent spawning of more than one NPC in the exact same spot, move it
                // slightly by a small random offset."
                let (dx, dy) = (self.rng().below(50), self.rng().below(50));
                if let Some(id) = self.spawn_npc_at(inst.npc_id, x + dx, y + dy, z, heading) {
                    spawned.push(id);
                }
            }
        }
        let Some(&last) = spawned.last() else {
            warn!(
                "AutoSpawnHandler: An error occurred while initializing spawn instance (Object ID = {key}): NPC {} could not spawn.",
                inst.npc_id
            );
            return;
        };
        if let Some(i) = self.seven_signs.auto_spawn.instances.get_mut(&key) {
            i.npcs.extend(spawned.iter().copied());
        }
        if inst.broadcast {
            let name = self
                .npc_template(last)
                .map(|t| t.name.clone())
                .unwrap_or_default();
            let town = self
                .zoning
                .map_regions
                .as_ref()
                .map(|r| {
                    let (lx, ly, _) = self.position_of(last).unwrap_or((x, y, z));
                    r.closest_town_name(lx, ly).to_owned()
                })
                .unwrap_or_default();
            self.broadcast_to_all_online_players(&CreatureSay {
                sender_object_id: 0,
                chat_type: ChatType::Announcement,
                sender: SenderField::Name(String::new()),
                message_id: -1,
                body: SayBody::Text(format!("The {name} has spawned near {town}!")),
            });
        }
        // If there is no despawn time, do not create a despawn task.
        if inst.des_delay > 0 {
            self.auto_spawn_timer(inst.des_delay - 1000, AutoSpawnTimer::Despawn(key));
        }
    }

    /// `World.getFirstVisibleObjectInRange(npc, Npc.class, radius) != null` for a place.
    fn npc_within(&self, x: i32, y: i32, z: i32, radius: i32) -> bool {
        self.npcs.values().any(|n| {
            crate::target::distance_3d((n.core.x, n.core.y, n.core.z), (x, y, z)) <= f64::from(radius)
        })
    }

    /// Java `AutoDespawner.run()`: the group's NPCs leave the world.
    fn auto_despawn(&mut self, key: u32) {
        let Some(npcs) = self
            .seven_signs
            .auto_spawn
            .instances
            .get_mut(&key)
            .map(|i| std::mem::take(&mut i.npcs))
        else {
            info!("AutoSpawnHandler: No spawn registered for object ID = {key}.");
            return;
        };
        for npc in npcs {
            self.delete_npc(npc);
        }
    }

    /// `getAutoSpawnInstance(npcId, false)`: the first group of the NPC.
    fn auto_spawn_first(&self, npc_id: i32) -> Option<u32> {
        self.seven_signs
            .auto_spawn
            .instances
            .iter()
            .find(|(_, i)| i.npc_id == npc_id)
            .map(|(k, _)| *k)
    }

    /// `getAutoSpawnInstances(npcId)`: every group of the NPC.
    fn auto_spawn_all(&self, npc_id: i32) -> Vec<u32> {
        self.seven_signs
            .auto_spawn
            .instances
            .iter()
            .filter(|(_, i)| i.npc_id == npc_id)
            .map(|(k, _)| *k)
            .collect()
    }

    fn auto_spawn_is_active(&self, key: u32) -> bool {
        self.seven_signs
            .auto_spawn
            .instances
            .get(&key)
            .is_some_and(|i| i.spawn_active)
    }

    /// `if (!isSpawnActive()) setSpawnActive(inst, true)`.
    fn auto_spawn_activate_if_idle(&mut self, key: u32) {
        if !self.auto_spawn_is_active(key) {
            self.set_auto_spawn_active(key, true);
        }
    }

    fn auto_spawn_set_all(&mut self, keys: &[u32], active: bool) {
        for &key in keys {
            self.set_auto_spawn_active(key, active);
        }
    }

    fn auto_spawn_set_opt(&mut self, key: Option<u32>, active: bool) {
        if let Some(key) = key {
            self.set_auto_spawn_active(key, active);
        }
    }

    /// Java `SevenSigns.spawnSevenSignsNPC()`: the Mammon NPCs, Lilith or Anakim, the crests, the
    /// orators and the preachers as the period and the seals say.
    // NOTE(java-quirk): Java throws a NullPointerException where a group is missing from
    // `random_spawn` (the Mammon merchant, blacksmith, Lilith, Anakim); here it is skipped.
    pub fn spawn_seven_signs_npc(&mut self) {
        let merchant = self.auto_spawn_first(MAMMON_MERCHANT_ID);
        let blacksmith = self.auto_spawn_first(MAMMON_BLACKSMITH_ID);
        let lilith = self.auto_spawn_first(LILITH_NPC_ID);
        let anakim = self.auto_spawn_first(ANAKIM_NPC_ID);
        let crests_of_dawn = self.auto_spawn_all(CREST_OF_DAWN_ID);
        let crests_of_dusk = self.auto_spawn_all(CREST_OF_DUSK_ID);
        let orators = self.auto_spawn_all(ORATOR_NPC_ID);
        let preachers = self.auto_spawn_all(PREACHER_NPC_ID);
        let marketeers = self.auto_spawn_all(MAMMON_MARKETEER_ID);
        let announce = self.seven_signs.settings.announce_mammon_spawn;
        let highest = self.ss_cabal_highest_score();
        debug!(
            "SevenSigns: spawning the Seven Signs NPCs (period {})",
            self.ss_current_period()
        );

        if self.ss_is_seal_validation_period() || self.ss_is_comp_results_period() {
            self.auto_spawn_set_all(&marketeers, true);

            let gnosis = self.ss_seal_owner(SEAL_GNOSIS);
            if gnosis == highest && gnosis != CABAL_NULL {
                if let Some(key) = blacksmith {
                    if !announce && let Some(i) = self.seven_signs.auto_spawn.instances.get_mut(&key) {
                        i.broadcast = false;
                    }
                    self.auto_spawn_activate_if_idle(key);
                }
                for &key in orators.iter().chain(preachers.iter()) {
                    self.auto_spawn_activate_if_idle(key);
                }
            } else {
                self.auto_spawn_set_opt(blacksmith, false);
                self.auto_spawn_set_all(&orators, false);
                self.auto_spawn_set_all(&preachers, false);
            }

            let avarice = self.ss_seal_owner(SEAL_AVARICE);
            if avarice == highest && avarice != CABAL_NULL {
                if let Some(key) = merchant {
                    if !announce && let Some(i) = self.seven_signs.auto_spawn.instances.get_mut(&key) {
                        i.broadcast = false;
                    }
                    self.auto_spawn_activate_if_idle(key);
                }
                match highest {
                    CABAL_DAWN => {
                        if let Some(key) = lilith {
                            self.auto_spawn_activate_if_idle(key);
                        }
                        self.auto_spawn_set_opt(anakim, false);
                        for &key in &crests_of_dawn {
                            self.auto_spawn_activate_if_idle(key);
                        }
                        self.auto_spawn_set_all(&crests_of_dusk, false);
                    }
                    CABAL_DUSK => {
                        if let Some(key) = anakim {
                            self.auto_spawn_activate_if_idle(key);
                        }
                        self.auto_spawn_set_opt(lilith, false);
                        for &key in &crests_of_dusk {
                            self.auto_spawn_activate_if_idle(key);
                        }
                        self.auto_spawn_set_all(&crests_of_dawn, false);
                    }
                    _ => {}
                }
            } else {
                self.auto_spawn_set_opt(merchant, false);
                self.auto_spawn_set_opt(lilith, false);
                self.auto_spawn_set_opt(anakim, false);
                self.auto_spawn_set_all(&crests_of_dawn, false);
                self.auto_spawn_set_all(&crests_of_dusk, false);
            }
        } else {
            self.auto_spawn_set_opt(merchant, false);
            self.auto_spawn_set_opt(blacksmith, false);
            self.auto_spawn_set_opt(lilith, false);
            self.auto_spawn_set_opt(anakim, false);
            self.auto_spawn_set_all(&crests_of_dawn, false);
            self.auto_spawn_set_all(&crests_of_dusk, false);
            self.auto_spawn_set_all(&orators, false);
            self.auto_spawn_set_all(&preachers, false);
            self.auto_spawn_set_all(&marketeers, false);
        }
    }
}
