//! Java `TimerExecutor<String>` of a script (`getTimers()`): the newer timers with a parameter
//! set, `onTimerEvent` and `onTimerCancel` (S-API-03).
// Java: mechanics/script/timers/{TimerExecutor,TimerHolder,IEventTimerEvent,IEventTimerCancel}.java
// Java: mechanics/script/Quest.java (onTimerEvent, onTimerCancel, getTimers)
// Java: entity/actor/Npc.java (stopTimerHolders at onDecay), entity/actor/Player.java
//       (stopAllTasks)
//
// # Differences from `QuestTimer` (`timers.rs`)
// - The key is the event text only, compared with the NPC and the player by identity; adding an
//   equal timer replaces (and cancels) the running one.
// - `params` (Java `StatSet`) travel with the timer to the hook.
//
// NOTE(java-quirk): `TimerHolder.run` first calls `onTimerPostExecute`, which cancels the
// one-shot holder that is running; `cancelTimer` finds the task not done yet, cancels it and
// notifies `onTimerCancel`. So a one-shot timer calls `onTimerCancel` and then `onTimerEvent`.
//
// NOTE(java-quirk): `Npc.stopTimerHolders` (at decay) and `Player.stopAllTasks` only cancel the
// tasks (`cancelTask`): the holders stay in the executor, `hasTimer` still says yes and no
// `onTimerCancel` is sent. Adding an equal timer later removes the stale holder silently.
//
// NOTE(java-quirk): `TimerExecutor.cancelTimer(event, npc, player)` returns false even when it
// stopped a timer; here it returns nothing.

use std::collections::BTreeMap;

use l2_util::ObjectId;

use super::api::ScriptNpc;
use super::ctx::ScriptCtx;
use super::setup::ScriptId;
use super::timers::ScriptTimer;
use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::World;

/// Java `StatSet` as timer parameters: named values. Everything is kept as text and read the way
/// `StatSet.getInt` etc. parse it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TimerParams {
    values: BTreeMap<String, String>,
}

impl TimerParams {
    pub fn new() -> Self {
        Self::default()
    }

    /// Java `StatSet.set(name, value)`.
    pub fn set(mut self, name: &str, value: impl ToString) -> Self {
        self.values.insert(name.to_owned(), value.to_string());
        self
    }

    pub fn contains(&self, name: &str) -> bool {
        self.values.contains_key(name)
    }

    /// Java `getString(name)`; Java throws for a missing name, here it is empty.
    pub fn get_string(&self, name: &str) -> String {
        self.values.get(name).cloned().unwrap_or_default()
    }

    /// Java `getString(name, default)`.
    pub fn get_string_or(&self, name: &str, default: &str) -> String {
        self.values
            .get(name)
            .cloned()
            .unwrap_or_else(|| default.to_owned())
    }

    /// Java `getInt(name, default)`.
    pub fn get_int(&self, name: &str, default: i32) -> i32 {
        self.values
            .get(name)
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(default)
    }

    /// Java `getLong(name, default)`.
    pub fn get_long(&self, name: &str, default: i64) -> i64 {
        self.values
            .get(name)
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(default)
    }

    /// Java `getBoolean(name, default)`.
    pub fn get_bool(&self, name: &str, default: bool) -> bool {
        self.values
            .get(name)
            .map_or(default, |v| v.eq_ignore_ascii_case("true"))
    }
}

/// One Java `TimerHolder`.
#[derive(Debug, Clone)]
struct Holder {
    script: ScriptId,
    event: String,
    params: Option<TimerParams>,
    npc: Option<ScriptNpc>,
    player: Option<ObjectId>,
    repeating: bool,
    ticks: u64,
    /// The task on the wheel; `None` once it was cancelled without removing the holder.
    wheel: Option<TimerId>,
}

impl Holder {
    /// Java `isEqual(event, npc, player)`.
    fn is_equal(&self, event: &str, npc: Option<ObjectId>, player: Option<ObjectId>) -> bool {
        self.event == event && self.npc.map(|n| n.object_id) == npc && self.player == player
    }
}

/// The executors of every script.
#[derive(Debug, Default)]
pub(crate) struct ExecTimers {
    next_id: u64,
    /// Ids grow, so the order is Java's insertion order.
    holders: BTreeMap<u64, Holder>,
}

impl ExecTimers {
    fn ids_where(&self, script: ScriptId, f: impl Fn(&Holder) -> bool) -> Vec<u64> {
        self.holders
            .iter()
            .filter(|(_, h)| h.script == script && f(h))
            .map(|(id, _)| *id)
            .collect()
    }
}

impl World {
    /// Java `TimerExecutor.addTimer` / `addRepeatingTimer`: an equal timer is replaced.
    pub(crate) fn exec_add_timer(
        &mut self,
        script: ScriptId,
        event: &str,
        params: Option<TimerParams>,
        time_ms: i64,
        (npc, player): (Option<ScriptNpc>, Option<ObjectId>),
        repeating: bool,
    ) -> bool {
        // `removeAndCancelTimers(timers, holder::isEqual)`.
        let equal = self
            .scripts
            .exec
            .ids_where(script, |h| h.is_equal(event, npc.map(|n| n.object_id), player));
        for id in equal {
            self.exec_cancel(id);
        }
        let ticks = TimerWheel::ms_to_ticks(u64::try_from(time_ms).unwrap_or(0));
        let id = self.scripts.exec.next_id;
        self.scripts.exec.next_id += 1;
        let wheel = self
            .timers
            .schedule_at(self.tick + ticks, TimerKind::Script(ScriptTimer::Exec(id)));
        self.scripts.exec.holders.insert(
            id,
            Holder {
                script,
                event: event.to_owned(),
                params,
                npc,
                player,
                repeating,
                ticks,
                wheel: Some(wheel),
            },
        );
        true
    }

    /// Java `TimerHolder.cancelTimer()` + the removal from the set: the task stops and
    /// `onTimerCancel` is notified when the task was still live.
    fn exec_cancel(&mut self, id: u64) {
        let Some(holder) = self.scripts.exec.holders.remove(&id) else {
            return;
        };
        if let Some(wheel) = holder.wheel {
            self.timers.cancel(wheel);
            self.exec_notify_cancel(&holder);
        }
    }

    fn exec_notify_cancel(&mut self, holder: &Holder) {
        self.with_script(holder.script, |s, ctx| {
            s.on_timer_cancel(
                ctx,
                &holder.event,
                holder.params.as_ref(),
                holder.npc,
                holder.player,
            )
        });
    }

    /// The wheel fired the task of holder `id` (Java `TimerHolder.run`).
    pub(crate) fn on_exec_timer(&mut self, id: u64) {
        let Some(holder) = self.scripts.exec.holders.get(&id).cloned() else {
            return;
        };
        if holder.repeating {
            // `scheduleAtFixedRate`.
            let wheel = self
                .timers
                .schedule_at(self.tick + holder.ticks, TimerKind::Script(ScriptTimer::Exec(id)));
            if let Some(h) = self.scripts.exec.holders.get_mut(&id) {
                h.wheel = Some(wheel);
            }
        } else {
            // `onTimerPostExecute`: removes (and cancels, see the module notes) every equal
            // holder, this one included. The task that is running is off the wheel already.
            let equal = self.scripts.exec.ids_where(holder.script, |h| {
                h.is_equal(&holder.event, holder.npc.map(|n| n.object_id), holder.player)
            });
            for other in equal {
                if other == id {
                    if let Some(h) = self.scripts.exec.holders.remove(&id)
                        && h.wheel.is_some()
                    {
                        self.exec_notify_cancel(&h);
                    }
                } else {
                    self.exec_cancel(other);
                }
            }
        }
        self.with_script(holder.script, |s, ctx| {
            s.on_timer_event(
                ctx,
                &holder.event,
                holder.params.as_ref(),
                holder.npc,
                holder.player,
            )
        });
    }

    /// `Npc.stopTimerHolders` and `Player.stopAllTasks`: only the tasks stop.
    pub(crate) fn exec_stop_tasks_of(&mut self, object: ObjectId) {
        let wheels: Vec<TimerId> = self
            .scripts
            .exec
            .holders
            .values_mut()
            .filter(|h| h.npc.map(|n| n.object_id) == Some(object) || h.player == Some(object))
            .filter_map(|h| h.wheel.take())
            .collect();
        for wheel in wheels {
            self.timers.cancel(wheel);
        }
    }

    /// The number of holders in the executors (tests and admin views).
    pub fn exec_timer_count(&self) -> usize {
        self.scripts.exec.holders.len()
    }
}

impl ScriptCtx<'_> {
    /// Java `getTimers().addTimer(event, params, time, npc, player)`: a one-shot timer; an equal
    /// one (event, npc, player) is replaced. Always `true`, like Java.
    pub fn add_timer(
        &mut self,
        event: &str,
        params: Option<TimerParams>,
        time_ms: i64,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> bool {
        self.world
            .exec_add_timer(self.script, event, params, time_ms, (npc, player), false)
    }

    /// Java `getTimers().addRepeatingTimer(event, time, npc, player)`.
    pub fn add_repeating_timer(
        &mut self,
        event: &str,
        time_ms: i64,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> bool {
        self.world
            .exec_add_timer(self.script, event, None, time_ms, (npc, player), true)
    }

    /// Java `getTimers().hasTimer(event, npc, player)`.
    pub fn has_timer(&self, event: &str, npc: Option<ScriptNpc>, player: Option<ObjectId>) -> bool {
        let npc = npc.map(|n| n.object_id);
        !self
            .world
            .scripts
            .exec
            .ids_where(self.script, |h| h.is_equal(event, npc, player))
            .is_empty()
    }

    /// Java `getTimers().cancelTimers(event)`: every timer of the event. `true` when there was one.
    pub fn cancel_timers(&mut self, event: &str) -> bool {
        let ids = self
            .world
            .scripts
            .exec
            .ids_where(self.script, |h| h.event == event);
        for id in &ids {
            self.world.exec_cancel(*id);
        }
        !ids.is_empty()
    }

    /// Java `getTimers().cancelTimer(event, npc, player)`.
    pub fn cancel_timer(&mut self, event: &str, npc: Option<ScriptNpc>, player: Option<ObjectId>) {
        let npc = npc.map(|n| n.object_id);
        let ids = self
            .world
            .scripts
            .exec
            .ids_where(self.script, |h| h.is_equal(event, npc, player));
        for id in ids {
            self.world.exec_cancel(id);
        }
    }

    /// Java `getTimers().cancelTimersOf(npc)`.
    pub fn cancel_timers_of(&mut self, npc: ScriptNpc) {
        let ids = self
            .world
            .scripts
            .exec
            .ids_where(self.script, |h| h.npc.map(|n| n.object_id) == Some(npc.object_id));
        for id in ids {
            self.world.exec_cancel(id);
        }
    }

    /// Java `getTimers().cancelAllTimers()`.
    pub fn cancel_all_timers(&mut self) {
        let ids = self.world.scripts.exec.ids_where(self.script, |_| true);
        for id in ids {
            self.world.exec_cancel(id);
        }
    }

    /// Java `hasTimers()`: whether the script has asked for timers (and still holds some).
    pub fn has_timers(&self) -> bool {
        !self
            .world
            .scripts
            .exec
            .ids_where(self.script, |_| true)
            .is_empty()
    }
}
