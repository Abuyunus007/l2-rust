//! What happens to an NPC after it died: the corpse decays, its spawn counts one NPC less and,
//! after the respawn delay, the same NPC comes back at its spawn.
// Java: entity/spawns/Spawn.java (decreaseCount, respawnNpc, stopRespawn, startRespawn,
//       isRespawnEnabled, getLastSpawn, the `_currentCount` / `_scheduledCount` bookkeeping of
//       `initializeNpc`)
// Java: entity/spawns/SpawnGroup.java (onUnitDeath, spawnUnits)
// Java: taskmanagers/{DecayTaskManager,RespawnTaskManager,SpawnGroupTaskManager}.java
// Java: entity/actor/Npc.java (onDecay, deleteMe), entity/actor/Creature.java (onDecay, decayMe)
//
// # How the three Java task managers map to the timer wheel
// Each Java manager polls a map once a second (`scheduleAtFixedRate(this, 0, 1000)`) and runs an
// entry when `currentTime > dueTime`, so an entry fires between 0 and 1 s after its due time.
// Here every entry is one timer on the world's wheel ([`TimerKind::Decay`], [`TimerKind::Respawn`],
// [`TimerKind::GroupSpawn`]) and fires on the tick its due time falls in (delays are rounded up
// to 50 ms ticks, see [`TimerWheel::ms_to_ticks`]). The wall-clock phase of Java's poll cannot be
// reproduced, and a deterministic wheel is what D-12 asks for.
//
// # The life of a spawned NPC
// 1. `World::initialize_npc` (npc.rs) creates it and calls [`World::spawn_registered`]:
//    `_currentCount++`, `_spawnedNpcs.add(npc)`.
// 2. It dies (`death.rs`): `DecayTaskManager.add` -> [`World::add_decay_task`], `corpseTime` s.
// 3. [`World::on_npc_decay`] (Java `Npc.onDecay`): `DeleteObject` for everybody, the object leaves
//    the world, `Spawn.decreaseCount` -> [`World::decrease_count`], which schedules the respawn
//    (`RespawnTaskManager.add`) if `_doRespawn` and `_scheduledCount + _currentCount <
//    _maximumCount`, at `Rnd.get(min, max)` ms (`respawnDelay` +- `respawnRandom`) or `min`.
// 4. [`World::on_respawn_timer`] (Java `Spawn.respawnNpc`): the same object id comes back
//    (Java re-initialises the same `Npc` object), at a fresh place of its territory when it has one.
//
// A minion that is not part of a spawn comes back through `MinionList.MinionRespawnTask`
// instead (`ai/minions.rs`, `TimerKind::MinionRespawn`).
//
// NOTE(java-quirk): Java keeps the `Npc` object between its death and its respawn. Here the
// entry in `World::npcs` is removed when the corpse decays and a new one is made at the respawn;
// what lives on is the object id (and the weapon enchant effect the constructor rolled).

use std::collections::VecDeque;

use l2_data::spawn::SpawnSelection;
use l2_util::ObjectId;
use tracing::debug;

use crate::npc::SpawnReport;
use crate::timer::{TimerKind, TimerWheel};
use crate::world::World;

/// The runtime fields of Java's `Spawn`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SpawnRuntime {
    /// `_currentCount`: NPCs of this spawn in the world, corpses included.
    pub(crate) current_count: i32,
    /// `_scheduledCount`: respawns in progress or standing by.
    pub(crate) scheduled_count: i32,
    /// `_doRespawn`.
    pub(crate) do_respawn: bool,
    /// `_spawnedNpcs`, oldest first.
    pub(crate) spawned: VecDeque<ObjectId>,
}

/// What the respawn of one dead NPC needs (the `Npc` object of Java, minus what the object id
/// and the spawn already say).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PendingRespawn {
    pub(crate) spawn: usize,
    /// `_currentEnchant`, rolled by the constructor.
    pub(crate) enchant: i32,
}

impl World {
    /// The number of spawns of the datapack. Runtime spawns come after them in the index space.
    fn static_spawn_count(&self) -> usize {
        self.npc_world.spawn_data().map_or(0, |s| s.spawns().len())
    }

    /// The definition of spawn `index`: an entry of the datapack's spawn list, or else a spawn an
    /// admin command made ([`World::admin_spawn`]). One index space for both, so `Npc::spawn`
    /// and the spawn runtime work the same for each.
    pub(crate) fn spawn_def(&self, index: usize) -> Option<&l2_data::spawn::NpcSpawn> {
        let static_count = self.static_spawn_count();
        if index < static_count {
            self.npc_world.spawn_data().and_then(|s| s.spawns().get(index))
        } else {
            self.death.runtime_spawns.get(index - static_count)
        }
    }

    /// Java `new Spawn(template)` + `setXYZ` + `setHeading` + `setAmount` + `setRespawnDelay` +
    /// `init()` (+ `stopRespawn()` when the spawn is not permanent or has no delay), as the admin
    /// commands `//spawn`, `//spawn_monster`, `//spawn_once` and `//spawnat` make it. The NPCs
    /// are created now and the returned list is what got a place.
    ///
    /// `respawn_delay` is in seconds (`setRespawnDelay(delay)`: 0 is no respawn, a negative
    /// delay counts as one second). `permanent` is Java's flag: `SpawnData.addNewSpawn` makes
    /// the spawn permanent, which is not written to the datapack here (FOLLOW-UP).
    pub(crate) fn admin_spawn(
        &mut self,
        npc_id: i32,
        position: (i32, i32, i32),
        heading: i32,
        amount: i32,
        respawn_delay: i32,
        permanent: bool,
    ) -> Vec<ObjectId> {
        // `setRespawnDelay(delay, 0)`: `Math.max(1, delay) * 1000` for a delay, 0 for none.
        let delay_ms = if respawn_delay == 0 {
            0
        } else {
            respawn_delay.max(1).saturating_mul(1000)
        };
        let index = self.static_spawn_count() + self.death.runtime_spawns.len();
        self.death.runtime_spawns.push(l2_data::spawn::NpcSpawn {
            npc_id,
            amount,
            location: l2_data::Location {
                x: position.0,
                y: position.1,
                z: position.2,
            },
            heading,
            respawn_min_ms: delay_ms,
            respawn_max_ms: delay_ms,
            // `init()` leaves `_doRespawn = _respawnMinDelay > 0`; `stopRespawn()` follows when
            // `!permanent || respawnTime <= 0` (a negative delay too).
            respawn_enabled: permanent && respawn_delay > 0,
            chase_range: 0,
            name: None,
            territory: None,
            period_of_day: None,
            ai_data: None,
            group: None,
            source: "admin command".to_owned(),
        });
        let mut created = Vec::new();
        let mut report = crate::npc::SpawnReport::default();
        for _ in 0..amount {
            // Java `init()`: the loop stops when a unit cannot be made.
            match self.initialize_npc(index, None, &mut report) {
                Some(id) => created.push(id),
                None => break,
            }
        }
        debug!("admin spawn {index}: npc {npc_id} x{} of {amount}", created.len());
        created
    }

    /// The runtime of spawn `index`. Java creates a `Spawn` with `_doRespawn = true` and
    /// `init()` sets it to `_respawnMinDelay > 0` once the first NPCs stand; the loader
    /// (`NpcSpawn::respawn_enabled`) already computed the value `init()` leaves.
    pub(crate) fn spawn_runtime(&mut self, index: usize) -> &mut SpawnRuntime {
        let enabled = self.spawn_def(index).is_some_and(|s| s.respawn_enabled);
        self.death.spawns.entry(index).or_insert_with(|| SpawnRuntime {
            do_respawn: enabled,
            ..SpawnRuntime::default()
        })
    }

    /// `_currentCount++` and `_spawnedNpcs.add(npc)` at the end of `initializeNpc`.
    pub(crate) fn spawn_registered(&mut self, index: usize, id: ObjectId) {
        let rt = self.spawn_runtime(index);
        rt.current_count += 1;
        rt.spawned.push_back(id);
    }

    /// Java `Spawn.stopRespawn()`.
    pub(crate) fn stop_respawn(&mut self, index: usize) {
        self.spawn_runtime(index).do_respawn = false;
    }

    /// Java `Spawn.startRespawn()`.
    pub(crate) fn start_respawn(&mut self, index: usize) {
        self.spawn_runtime(index).do_respawn = true;
    }

    /// Java `Spawn.isRespawnEnabled()`.
    pub fn is_respawn_enabled(&self, index: usize) -> bool {
        match self.death.spawns.get(&index) {
            Some(rt) => rt.do_respawn,
            // Not touched yet: the value `init()` leaves.
            None => self.spawn_def(index).is_some_and(|s| s.respawn_enabled),
        }
    }

    /// Java `Spawn.getLastSpawn()`: `_spawnedNpcs.peekLast()`.
    pub fn last_spawn(&self, index: usize) -> Option<ObjectId> {
        self.death.spawns.get(&index)?.spawned.back().copied()
    }

    /// Java `Spawn._currentCount` of spawn `index`.
    pub fn spawn_current_count(&self, index: usize) -> i32 {
        self.death.spawns.get(&index).map_or(0, |rt| rt.current_count)
    }

    /// Java `Spawn._scheduledCount` of spawn `index`.
    pub fn spawn_scheduled_count(&self, index: usize) -> i32 {
        self.death.spawns.get(&index).map_or(0, |rt| rt.scheduled_count)
    }

    // ------------------------------------------------------------------ DecayTaskManager

    /// Java `DecayTaskManager.add(creature)`: an NPC's corpse stays `corpseTime` seconds; a
    /// player is disconnected after an hour of being dead when `DisconnectAfterDeath` is on.
    /// Adding again replaces the old due time.
    pub(crate) fn add_decay_task(&mut self, id: ObjectId) {
        let delay_secs: i64 = if let Some(npc) = self.npcs.get(&id) {
            let mut delay = self
                .npc_world
                .template(npc.npc_id)
                .map_or(i64::from(self.settings.death.default_corpse_time), |t| {
                    i64::from(t.corpse_time)
                });
            // `isAttackable() && (isSpoiled() || isSeeded())` adds `SpoiledCorpseExtendTime`
            // (seeds: the manor, later).
            if self.is_attackable_npc(id) && self.is_spoiled(id) {
                delay += i64::from(self.settings.death.spoiled_corpse_extend_time);
            }
            delay
        } else if self.players.contains_key(&id) {
            // Offline play: not ported.
            if self.settings.death.disconnect_after_death {
                3600
            } else {
                i64::from(self.settings.death.default_corpse_time)
            }
        } else {
            return;
        };
        // `DECAY_SCHEDULES.put(creature, now + delay * 1000)`.
        let ms = u64::try_from(delay_secs.saturating_mul(1000)).unwrap_or(0);
        let due = self.tick + TimerWheel::ms_to_ticks(ms);
        let timer = self.timers.schedule_at(due, TimerKind::Decay(id));
        self.death.decay_due.insert(id, due);
        if let Some(old) = self.death.decay.insert(id, timer) {
            self.timers.cancel(old);
        }
    }

    /// Java `DecayTaskManager.getRemainingTime(creature)`, in milliseconds: the time left of the
    /// decay task, negative when it is overdue, `i64::MAX` (`Long.MAX_VALUE`) without a task.
    pub fn decay_remaining_ms(&self, id: ObjectId) -> i64 {
        match self.death.decay_due.get(&id) {
            Some(&due) => {
                let ticks =
                    i64::try_from(due).unwrap_or(i64::MAX) - i64::try_from(self.tick).unwrap_or(i64::MAX);
                ticks.saturating_mul(i64::try_from(crate::world::TICK.as_millis()).unwrap_or(50))
            }
            None => i64::MAX,
        }
    }

    /// Java `DecayTaskManager.cancel(creature)`.
    pub(crate) fn cancel_decay_task(&mut self, id: ObjectId) -> bool {
        self.death.decay_due.remove(&id);
        match self.death.decay.remove(&id) {
            Some(timer) => {
                self.timers.cancel(timer);
                true
            }
            None => false,
        }
    }

    /// Whether a decay task is waiting for `id`. For tests and tools.
    pub fn has_decay_task(&self, id: ObjectId) -> bool {
        self.death.decay.contains_key(&id)
    }

    /// A decay timer fired: Java `creature.onDecay()`.
    pub(crate) fn on_decay_timer(&mut self, id: ObjectId) {
        self.death.decay.remove(&id);
        self.death.decay_due.remove(&id);
        if self.players.contains_key(&id) {
            self.on_player_decay(id);
        } else if self.is_summon(id) {
            // `Summon.onDecay()`.
            self.summon_on_decay(id);
        } else {
            self.on_npc_decay(id);
        }
    }

    /// Java `Creature.onDecay()` of a player: with `DisconnectAfterDeath` the character is stored
    /// and disconnected with a message (`Disconnection.storeAndDeleteWith`).
    // NOTE: Java looks the text up in `SendMessageLocalisationData` (an English default); the
    // translations are not ported.
    fn on_player_decay(&mut self, id: ObjectId) {
        if self.settings.death.disconnect_after_death {
            self.send_message(
                id,
                "60 min. have passed after the death of your character, so you were disconnected from the game.",
            );
            self.leave(id, crate::command::LeaveReason::Kicked);
        }
        // Without the option Java would `decayMe()` a player; nothing adds a decay task for one
        // then.
    }

    // ------------------------------------------------------------------ Npc.onDecay

    /// Java `Npc.onDecay()` (with `Creature.onDecay` and `decayMe`): the corpse (or a deleted NPC)
    /// leaves the world, and its spawn counts it out and schedules the respawn. Does nothing for
    /// an NPC that is gone already (`_isDecayed`).
    pub(crate) fn on_npc_decay(&mut self, id: ObjectId) {
        let Some(npc) = self.npcs.get_mut(&id) else {
            return;
        };
        if npc.decayed {
            return;
        }
        npc.decayed = true;
        let (spawn, enchant) = (npc.spawn, npc.enchant_effect);
        // `Creature.decayMe`: an attackable forgets its attackers and its AI stops; then
        // `WorldObject.decayMe`: out of view for everybody, out of the object table.
        self.cancel_decay_task(id);
        self.decay(id);
        self.forget_npc_ai(id);
        // The spoiler and the sweep items go with the corpse; the object id comes back on
        // respawn (`Attackable.onSpawn` clears them).
        self.death.spoil.remove(&id);
        self.forget_combat(id);
        self.forget_status(id);
        // `ZoneRegion.removeFromZones`: NPCs have no zone state.
        // Decrease its spawn counter. `!RaidBossSpawnManager.isDefined(getId())`: raid bosses
        // are not spawned from the spawn files.
        if let Some(index) = spawn {
            self.decrease_count(index, id, enchant);
        }
        // `WalkingManager.onDeath` (routes: M4-AI-01), the summoner's list (summons: M5), quest
        // timers and script value (scripts: D-05).
        self.npcs.remove(&id);
    }

    // ------------------------------------------------------------------ Spawn.decreaseCount

    /// Java `Spawn.decreaseCount(oldNpc)`.
    pub(crate) fn decrease_count(&mut self, index: usize, old: ObjectId, enchant: i32) {
        let (min, max, amount, group) = {
            let Some(spawn) = self.spawn_def(index) else {
                return;
            };
            (
                spawn.respawn_min_ms,
                spawn.respawn_max_ms,
                spawn.amount,
                spawn.group,
            )
        };
        let schedule = {
            let rt = self.spawn_runtime(index);
            // Sanity check.
            if rt.current_count <= 0 {
                return;
            }
            // Decrease the current number of Npc of this Spawn, remove this NPC from list of
            // spawned.
            rt.current_count -= 1;
            rt.spawned.retain(|id| *id != old);
            // Check if respawn is possible to prevent multiple respawning caused by lag.
            if rt.do_respawn && rt.scheduled_count + rt.current_count < amount {
                rt.scheduled_count += 1;
                true
            } else {
                false
            }
        };
        if schedule {
            // `hasRespawnRandom()`: min != max.
            let delay_ms = if min == max { min } else { self.rng().get(min, max) };
            let ms = u64::try_from(delay_ms).unwrap_or(0);
            let due = self.tick + TimerWheel::ms_to_ticks(ms);
            self.timers.schedule_at(due, TimerKind::Respawn(old));
            self.death.respawns.insert(
                old,
                PendingRespawn {
                    spawn: index,
                    enchant,
                },
            );
        }
        // Notify the spawn group, it rolls the replacement itself.
        if let Some(group) = group {
            self.group_unit_death(group, index);
        }
    }

    /// A respawn timer fired: Java `RespawnTaskManager.run`'s `spawn.respawnNpc(npc);
    /// spawn._scheduledCount--`.
    pub(crate) fn on_respawn_timer(&mut self, old: ObjectId) {
        let Some(pending) = self.death.respawns.remove(&old) else {
            return;
        };
        // `respawnNpc`: `if (_doRespawn) initializeNpc(oldNpc)`.
        if self.spawn_runtime(pending.spawn).do_respawn {
            let mut report = SpawnReport::default();
            if self
                .initialize_npc(pending.spawn, Some((old, pending.enchant)), &mut report)
                .is_none()
            {
                debug!(
                    "Spawn {}: the respawn of {old} could not be placed.",
                    pending.spawn
                );
            }
        }
        self.spawn_runtime(pending.spawn).scheduled_count -= 1;
    }

    /// Whether `old` waits for its respawn. For tests and tools.
    pub fn has_pending_respawn(&self, old: ObjectId) -> bool {
        self.death.respawns.contains_key(&old)
    }

    // ------------------------------------------------------------------ SpawnGroup.onUnitDeath

    /// Java `SpawnGroup.onUnitDeath(spawn)`: an `all` group's slot respawns itself; in the other
    /// groups a new entry is rolled (ignoring which one died), reserved at once and
    /// materialised after the rolled entry's respawn delay.
    // NOTE: the group's `_active` flag (cleared by the unspawn admin commands) is always true.
    pub(crate) fn group_unit_death(&mut self, group_index: usize, spawn_index: usize) {
        let Some(spawns) = self.npc_world.spawn_data().cloned() else {
            return;
        };
        let Some(group) = spawns.groups().get(group_index) else {
            return;
        };
        if group.selection == SpawnSelection::All {
            // ALL slots respawn themselves with their fixed species, the unit stays reserved.
            return;
        }
        // `getEntryBySlot`: identity of the slot.
        let Some(dead_entry) = group.entries.iter().position(|e| e.slots.contains(&spawn_index)) else {
            return;
        };
        {
            let Some(state) = self.death.groups.get_mut(&group_index) else {
                return;
            };
            state.entry_alive[dead_entry] -= 1;
            state.alive -= 1;
        }
        let rolled = self.entry_roll(group.entries.len());
        let reserved = self
            .death
            .groups
            .get_mut(&group_index)
            .is_some_and(|state| state.try_reserve(rolled, 1));
        if !reserved {
            return;
        }
        // Reserve now, materialize later. The delay comes from the rolled entry, not from the
        // dead one.
        let entry = &group.entries[rolled];
        let (delay, random) = (i64::from(entry.respawn_delay), i64::from(entry.respawn_random));
        let min = (delay - random).max(1) * 1000;
        let max = (delay + random).max(1) * 1000;
        let ms = if random > 0 {
            i64::from(self.rng().get(
                i32::try_from(min).unwrap_or(i32::MAX),
                i32::try_from(max).unwrap_or(i32::MAX),
            ))
        } else {
            min
        };
        let due = self.tick + TimerWheel::ms_to_ticks(u64::try_from(ms).unwrap_or(0));
        self.timers.schedule_at(
            due,
            TimerKind::GroupSpawn {
                group: group_index,
                entry: rolled,
            },
        );
    }

    /// A group spawn timer fired: Java `SpawnGroupTaskManager.run`'s `group.spawnUnits(entry, 1)`.
    pub(crate) fn on_group_spawn_timer(&mut self, group_index: usize, entry: usize) {
        let Some(mut state) = self.death.groups.remove(&group_index) else {
            return;
        };
        let slots = state.slots.get(entry).cloned().unwrap_or_default();
        let mut report = SpawnReport::default();
        self.spawn_units(&mut state, &slots, entry, 1, &mut report);
        self.death.groups.insert(group_index, state);
    }
}
