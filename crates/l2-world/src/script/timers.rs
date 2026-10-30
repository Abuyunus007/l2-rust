//! Quest timers on the timer wheel (D-12), the delayed kill notification and the despawn of
//! `addSpawn(..., despawnDelay)`.
// Java: mechanics/script/QuestTimer.java, mechanics/script/Quest.java (startQuestTimer,
//       getQuestTimer, cancelQuestTimer, cancelQuestTimers, removeQuestTimer)
// Java: entity/actor/Npc.java (addQuestTimer, stopQuestTimers at onDecay, scheduleDespawn)
// Java: entity/actor/Player.java (addQuestTimer, stopAllTasks)
// Java: entity/actor/Attackable.java (doDie: notifyEventAsyncDelayed(OnAttackableKill, _onKillDelay))

use l2_util::ObjectId;

use super::api::ScriptNpc;
use super::setup::ScriptId;
use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::World;

/// Java `Attackable._onKillDelay`: "L2J uses 5000".
pub const DEFAULT_ON_KILL_DELAY_MS: i32 = 2500;

/// The script tasks on the timer wheel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptTimer {
    /// A quest timer (its id in [`QuestTimers`]).
    Quest(u64),
    /// A holder of a script's `TimerExecutor` (its id in `timer_executor.rs`).
    Exec(u64),
    /// `OnAttackableKill`, `onKillDelay` after the death.
    Kill {
        npc: ScriptNpc,
        killer: ObjectId,
        is_summon: bool,
    },
    /// Java `Npc.scheduleDespawn(delay)`'s `DespawnTask`: `deleteMe()`.
    Despawn(ObjectId),
    /// A task of a tamed beast (`tamed_beast.rs`, S-API-05).
    TamedBeast {
        beast: ObjectId,
        task: super::tamed_beast::TamedBeastTask,
    },
}

/// One running Java `QuestTimer`.
#[derive(Debug, Clone)]
struct QuestTimer {
    script: ScriptId,
    name: String,
    npc: Option<ScriptNpc>,
    player: Option<ObjectId>,
    /// The period of a repeating timer (`scheduleAtFixedRate`), in ticks.
    repeat_ticks: Option<u64>,
    wheel: TimerId,
    /// A `ThreadPool.schedule` task of a script ([`World::schedule_script_event`]): not a
    /// `QuestTimer`, so the name lookups (`getQuestTimer`, `cancelQuestTimer(s)`) do not see it.
    unmerged: bool,
}

/// Every running quest timer, by id; ids grow, so the lowest matching id is Java's first in
/// its per-name list.
#[derive(Debug, Default)]
pub(crate) struct QuestTimers {
    next_id: u64,
    timers: std::collections::BTreeMap<u64, QuestTimer>,
}

impl QuestTimers {
    /// Java `getQuestTimer(name, npc, player)`: `QuestTimer.equals` compares the NPC and the
    /// player by identity, so `None` only matches `None`.
    pub(crate) fn find(
        &self,
        script: ScriptId,
        name: &str,
        npc: Option<ObjectId>,
        player: Option<ObjectId>,
    ) -> Option<u64> {
        self.timers
            .iter()
            .find(|(_, t)| {
                !t.unmerged
                    && t.script == script
                    && t.name == name
                    && t.npc.map(|n| n.object_id) == npc
                    && t.player == player
            })
            .map(|(id, _)| *id)
    }

    /// The number of running quest timers (tests).
    pub(crate) fn len(&self) -> usize {
        self.timers.len()
    }
}

impl World {
    /// Java `startQuestTimer(name, time, npc, player, repeating)`.
    pub(crate) fn start_quest_timer(
        &mut self,
        script: ScriptId,
        name: &str,
        time_ms: i64,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
        repeating: bool,
    ) {
        // "If there exists a timer with this name, allow the timer only if the [npc, player] set
        // is unique."
        if self
            .scripts
            .timers
            .find(script, name, npc.map(|n| n.object_id), player)
            .is_some()
        {
            return;
        }
        self.insert_quest_timer(script, name, time_ms, npc, player, repeating, false);
    }

    /// Java `ThreadPool.schedule(() -> ..., delay)` in a script: a one-shot task that runs the
    /// script's `on_event(name, npc, player)` and never merges with a timer of the same name
    /// (several can run side by side). It stops with its NPC or player like a quest timer
    /// (`Npc.onDecay`, logout), where Java's runnable would still run on the stale object.
    pub(crate) fn schedule_script_event(
        &mut self,
        script: ScriptId,
        name: &str,
        time_ms: i64,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) {
        self.insert_quest_timer(script, name, time_ms, npc, player, false, true);
    }

    #[allow(clippy::too_many_arguments)]
    fn insert_quest_timer(
        &mut self,
        script: ScriptId,
        name: &str,
        time_ms: i64,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
        repeating: bool,
        unmerged: bool,
    ) {
        let ticks = TimerWheel::ms_to_ticks(u64::try_from(time_ms).unwrap_or(0));
        let id = self.scripts.timers.next_id;
        self.scripts.timers.next_id += 1;
        let wheel = self
            .timers
            .schedule_at(self.tick + ticks, TimerKind::Script(ScriptTimer::Quest(id)));
        self.scripts.timers.timers.insert(
            id,
            QuestTimer {
                script,
                name: name.to_owned(),
                npc,
                player,
                repeat_ticks: repeating.then_some(ticks),
                wheel,
                unmerged,
            },
        );
    }

    /// Java `cancelQuestTimer(name, npc, player)`.
    pub(crate) fn cancel_quest_timer(
        &mut self,
        script: ScriptId,
        name: &str,
        npc: Option<ObjectId>,
        player: Option<ObjectId>,
    ) {
        let ids: Vec<u64> = self
            .scripts
            .timers
            .timers
            .iter()
            .filter(|(_, t)| {
                !t.unmerged
                    && t.script == script
                    && t.name == name
                    && t.npc.map(|n| n.object_id) == npc
                    && t.player == player
            })
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            self.remove_quest_timer(id);
        }
    }

    /// Java `cancelQuestTimers(name)`: every timer of the name.
    pub(crate) fn cancel_quest_timers(&mut self, script: ScriptId, name: &str) {
        let ids: Vec<u64> = self
            .scripts
            .timers
            .timers
            .iter()
            .filter(|(_, t)| !t.unmerged && t.script == script && t.name == name)
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            self.remove_quest_timer(id);
        }
    }

    /// Java `Npc.stopQuestTimers()` (at `onDecay`) and `Player.stopAllTasks()`: the timers bound
    /// to an object stop.
    pub(crate) fn script_cancel_timers_of(&mut self, object: ObjectId) {
        let ids: Vec<u64> = self
            .scripts
            .timers
            .timers
            .iter()
            .filter(|(_, t)| t.npc.map(|n| n.object_id) == Some(object) || t.player == Some(object))
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            self.remove_quest_timer(id);
        }
        // `Npc.stopTimerHolders()` / `Player.stopAllTasks()` of the `TimerExecutor` holders.
        self.exec_stop_tasks_of(object);
    }

    fn remove_quest_timer(&mut self, id: u64) {
        if let Some(timer) = self.scripts.timers.timers.remove(&id) {
            self.timers.cancel(timer.wheel);
        }
    }

    /// The script tasks of the timer wheel.
    pub(crate) fn on_script_timer(&mut self, timer: ScriptTimer) {
        match timer {
            ScriptTimer::Quest(id) => self.on_quest_timer(id),
            ScriptTimer::Exec(id) => self.on_exec_timer(id),
            ScriptTimer::Kill {
                npc,
                killer,
                is_summon,
            } => self.script_deliver_kill(npc, killer, is_summon),
            ScriptTimer::Despawn(npc) => {
                self.delete_npc(npc);
            }
            ScriptTimer::TamedBeast { beast, task } => self.on_tamed_beast_timer(beast, task),
        }
        self.script_drain();
    }

    /// Java `QuestTimer.ScheduleTimerTask.run`: a one-shot timer is cancelled first, then
    /// `notifyEvent(name, npc, player)`.
    fn on_quest_timer(&mut self, id: u64) {
        let Some(timer) = self.scripts.timers.timers.get(&id).cloned() else {
            return;
        };
        match timer.repeat_ticks {
            None => {
                self.scripts.timers.timers.remove(&id);
            }
            Some(ticks) => {
                let wheel = self
                    .timers
                    .schedule_at(self.tick + ticks, TimerKind::Script(ScriptTimer::Quest(id)));
                if let Some(t) = self.scripts.timers.timers.get_mut(&id) {
                    t.wheel = wheel;
                }
            }
        }
        self.script_notify_event(timer.script, &timer.name, timer.npc, timer.player);
    }

    /// Java `Npc.scheduleDespawn(delay)`.
    pub(crate) fn schedule_script_despawn(&mut self, npc: ObjectId, delay_ms: i64) {
        let ticks = TimerWheel::ms_to_ticks(u64::try_from(delay_ms).unwrap_or(0));
        self.timers
            .schedule_at(self.tick + ticks, TimerKind::Script(ScriptTimer::Despawn(npc)));
    }

    /// The running quest timers of a script, for `//quest_info`: name, NPC, player, repeating.
    pub(crate) fn quest_timers_of(
        &self,
        script: ScriptId,
    ) -> Vec<(String, Option<ScriptNpc>, Option<ObjectId>, bool)> {
        self.scripts
            .timers
            .timers
            .values()
            .filter(|t| t.script == script)
            .map(|t| (t.name.clone(), t.npc, t.player, t.repeat_ticks.is_some()))
            .collect()
    }

    /// The folder of a script's pages (`Script::path`).
    pub(crate) fn script_path_of(&self, script: ScriptId) -> &'static str {
        self.scripts.script(script).map_or("", |s| s.path())
    }

    /// The number of running quest timers (tests and admin views).
    pub fn quest_timer_count(&self) -> usize {
        self.scripts.timers.len()
    }
}
