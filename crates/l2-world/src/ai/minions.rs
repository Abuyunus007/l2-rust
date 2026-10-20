//! The life of a master's minions after the first spawn: a dead minion that comes back
//! (`MinionRespawnTask`), the minions that go with their master, and the config values that
//! steer both.
// Java: entity/actor/holders/npc/MinionList.java (onMinionDie, onMasterDie, MinionRespawnTask,
//       initializeNpc)
// Java: entity/actor/Attackable.java (doDie: the minion part), entity/actor/instance/Monster.java
//       (deleteMe)
// Java: config/NpcConfig.java (RaidMinionRespawnTime, CustomMinionsRespawnTime, ForceDeleteMinions)
//
// # How it plugs in
// - The master -> minions index is `AttackableWorld::minions` (`attackable.rs`), Java's
//   `Monster._minionList`; `hasMinions()` is "the master has an entry".
// - A minion that dies tells its master's list ([`World::minions_on_die`], from `death.rs`). The
//   list takes it out and, for a raid boss' minion (`RaidMinionRespawnTime`, 5 minutes) or a minion
//   of the `CustomMinionsRespawnTime` table, schedules the respawn on the timer wheel
//   ([`TimerKind::MinionRespawn`]) instead of Java's `ThreadPool.schedule`.
// - The respawn re-creates the minion with its old object id near the master (Java re-initialises
//   the same `Monster` object) and hands it the master's aggro list.
//
// NOTE(java-quirk): `MinionRespawnTask` skips a minion whose corpse has not decayed
// (`!_minion.isSpawned()`), so a respawn time shorter than the corpse time loses the minion for
// good.
// NOTE(java-quirk): `_minion.getAggroList().putAll(_master.getAggroList())` shares the `AggroInfo`
// objects, so later hits on the master change the minion's numbers too; here the minion gets a
// copy. The creature it attacks first is `getAggroList().keySet().iterator().next()`, a
// `ConcurrentHashMap` key in an order nobody can name; the master's oldest entry is used.

use std::collections::HashMap;

use l2_util::ObjectId;
use tracing::debug;

use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::World;

/// `MinionList.NO_RESPAWN`.
const NO_RESPAWN: i32 = 0;

/// What a pending `MinionRespawnTask` needs (the `Monster` object of Java, minus what its object
/// id says).
#[derive(Debug, Clone, Copy)]
struct PendingMinion {
    master: ObjectId,
    npc_id: i32,
    /// `_currentEnchant`, rolled by the constructor.
    enchant: i32,
    timer: TimerId,
}

/// Java `MinionList._respawnTasks`, by the minion that waits.
#[derive(Debug, Default)]
pub(crate) struct MinionRespawns {
    pending: HashMap<ObjectId, PendingMinion>,
}

impl MinionRespawns {
    /// How many minions wait for their respawn.
    pub(crate) fn len(&self) -> usize {
        self.pending.len()
    }
}

impl World {
    /// Java `Monster.hasMinions()`: the master has a minion list.
    pub(crate) fn has_minions(&self, id: ObjectId) -> bool {
        self.ai.attackable.minions.contains_key(&id)
    }

    /// How many minions wait for their respawn task (for tests and tools).
    pub fn pending_minion_respawns(&self) -> usize {
        self.ai.attackable.minion_respawns.len()
    }

    /// Java `Attackable.doDie`'s minion part: `leader.getMinionList().onMinionDie(mob,
    /// respawnTime)` and `mob.getMinionList().onMasterDie(false)`.
    pub(crate) fn minions_on_die(&mut self, id: ObjectId) {
        if !self.is_monster(id) {
            return;
        }
        if let Some(leader) = self.leader_of(id)
            && self.has_minions(leader)
        {
            // `MINIONS_RESPAWN_TIME.get(getId()) * 1000`, an int product; -1 when the table does
            // not name this NPC.
            let npc_id = self.npcs.get(&id).map_or(0, |n| n.npc_id);
            let respawn_ms = self
                .ai
                .attackable
                .settings
                .minions_respawn_time
                .get(&npc_id)
                .map_or(-1, |seconds| seconds.wrapping_mul(1000));
            self.minion_list_on_minion_die(leader, id, respawn_ms);
        }
        if self.has_minions(id) {
            self.minion_list_on_master_die(id, false);
        }
    }

    /// Java `MinionList.onMinionDie(minion, respawnTime)`: `respawn_ms` is a delay, -1 for the
    /// default (a raid boss' minions come back after `RaidMinionRespawnTime`, anybody else's do
    /// not) and 0 for never.
    pub(crate) fn minion_list_on_minion_die(&mut self, master: ObjectId, minion: ObjectId, respawn_ms: i32) {
        // Prevent memory leaks.
        if respawn_ms == NO_RESPAWN
            && let Some(npc) = self.npcs.get_mut(&minion)
        {
            npc.leader = None;
        }
        if let Some(list) = self.ai.attackable.minions.get_mut(&master) {
            list.retain(|m| *m != minion);
        }
        let actual = if respawn_ms <= -1 {
            if self.is_raid(master) {
                self.ai.attackable.settings.raid_minion_respawn_timer
            } else {
                NO_RESPAWN
            }
        } else {
            respawn_ms
        };
        if actual > NO_RESPAWN && !self.minion_master_is_alike_dead(master) {
            self.schedule_minion_respawn(master, minion, actual);
        }
    }

    /// Java `_master.isAlikeDead()`; a master that left the world counts as dead.
    fn minion_master_is_alike_dead(&self, master: ObjectId) -> bool {
        !self.npcs.contains_key(&master) || self.is_alike_dead(master)
    }

    /// `ThreadPool.schedule(new MinionRespawnTask(minion), actualRespawnTime)`.
    fn schedule_minion_respawn(&mut self, master: ObjectId, minion: ObjectId, delay_ms: i32) {
        let Some(npc) = self.npcs.get(&minion) else {
            return;
        };
        let (npc_id, enchant) = (npc.npc_id, npc.enchant_effect);
        let ticks = TimerWheel::ms_to_ticks(u64::try_from(delay_ms).unwrap_or(0));
        let due = self.tick + ticks;
        let timer = self.timers.schedule_at(due, TimerKind::MinionRespawn(minion));
        let pending = PendingMinion {
            master,
            npc_id,
            enchant,
            timer,
        };
        // Java keeps both tasks if a minion is told to die twice; the second would find the
        // minion spawned and do nothing, so one entry is the same.
        if let Some(old) = self.ai.attackable.minion_respawns.pending.insert(minion, pending) {
            self.timers.cancel(old.timer);
        }
    }

    /// Java `MinionList.onMasterDie(force)`: a raid boss (or `force`, or `ForceDeleteMinions`)
    /// takes its minions with it, and the respawn tasks of the list end.
    pub(crate) fn minion_list_on_master_die(&mut self, master: ObjectId, force: bool) {
        if !(self.is_raid(master) || force || self.ai.attackable.settings.force_delete_minions) {
            return;
        }
        let minions = self
            .ai
            .attackable
            .minions
            .get_mut(&master)
            .map(std::mem::take)
            .unwrap_or_default();
        for minion in minions {
            // `minion.setLeader(null); minion.deleteMe()`.
            if let Some(npc) = self.npcs.get_mut(&minion) {
                npc.leader = None;
            }
            self.delete_npc(minion);
        }
        let waiting: Vec<ObjectId> = self
            .ai
            .attackable
            .minion_respawns
            .pending
            .iter()
            .filter(|(_, p)| p.master == master)
            .map(|(minion, _)| *minion)
            .collect();
        for minion in waiting {
            if let Some(p) = self.ai.attackable.minion_respawns.pending.remove(&minion) {
                self.timers.cancel(p.timer);
            }
        }
    }

    /// A `MinionRespawnTask` is due.
    pub(crate) fn on_minion_respawn_timer(&mut self, minion: ObjectId) {
        let Some(pending) = self.ai.attackable.minion_respawns.pending.remove(&minion) else {
            return;
        };
        let master = pending.master;
        // Minion can be already spawned or deleted.
        let master_spawned = self.objects.get(&master).is_some_and(|e| e.spawned);
        if self.minion_master_is_alike_dead(master) || !master_spawned || self.npcs.contains_key(&minion) {
            return;
        }
        let Some(templates) = self.npc_world.templates_arc() else {
            return;
        };
        let Some(master_npc_id) = self.npcs.get(&master).map(|n| n.npc_id) else {
            return;
        };
        let (Some(master_template), Some(template)) =
            (templates.get(master_npc_id), templates.get(pending.npc_id))
        else {
            return;
        };
        let Some(minion) =
            self.place_minion(master, master_template, template, Some(minion), pending.enchant)
        else {
            debug!("The respawn of minion {minion} could not be placed.");
            return;
        };
        // Assist master with existing aggro targets.
        let master_aggro = self.aggro_list(master).to_vec();
        if let Some(first) = master_aggro.first().map(|a| a.attacker) {
            self.combat_state_mut(minion).aggro = master_aggro;
            self.set_intention_attack(minion, first);
        }
    }
}
