//! The "cancel return" of the dispel effects (`CancelReturn.ini`): buffs a dispel took come back
//! after `TimeToReturn` seconds with the time they had left.
// Java: dist/game/data/scripts/handlers/skill/effects/{DispelAll,DispelByCategory,
//       DispelBySlotProbability}.java (the `CancelReturnConfig` branches),
//       config/custom/CancelReturnConfig.java

use std::collections::HashMap;

use l2_util::ObjectId;

use super::SkillKey;
use super::buff_info::BuffInfo;
use super::control::ControlTimer;
use crate::timer::{TimerKind, TimerWheel};
use crate::world::World;

/// A buff that was taken: what Java's task reads from the removed `BuffInfo`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TakenBuff {
    pub skill: SkillKey,
    /// `BuffInfo.getAbnormalTime()` and `getPeriodStartTicks()`: `getTime()` is read again when
    /// the buff returns, so the seconds that passed since count.
    pub abnormal_time: i32,
    pub period_start_tick: u64,
}

impl TakenBuff {
    pub(crate) fn of(info: &BuffInfo) -> Self {
        Self {
            skill: info.skill(),
            abnormal_time: info.abnormal_time(),
            period_start_tick: info.period_start_tick(),
        }
    }

    /// Java `BuffInfo.getTime()` of the removed info at world tick `now`.
    fn time(&self, now: u64) -> i32 {
        let elapsed = now.saturating_sub(self.period_start_tick) / super::buff_info::TICKS_PER_SECOND;
        self.abnormal_time
            .saturating_sub(i32::try_from(elapsed).unwrap_or(i32::MAX))
    }
}

/// One scheduled return (Java's `ThreadPool.schedule` lambda).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PendingReturn {
    pub target: ObjectId,
    /// `DispelAll` also gives the buffs back to a monster; the other two only to players.
    pub monsters_too: bool,
    pub taken: Vec<TakenBuff>,
}

/// The scheduled returns by id (`EffectsWorld::pending_returns`).
pub(crate) type ReturnTable = HashMap<u64, PendingReturn>;

impl World {
    /// The shared `CancelReturnConfig` checks of the three dispel effects: whether the buffs that
    /// `effector` takes from `effected` come back.
    pub(crate) fn cancel_returns(&self, effector: ObjectId, effected: ObjectId) -> bool {
        let settings = &self.settings.effects;
        if !settings.cancel_return_on {
            return false;
        }
        if (self.is_player(effector) && !settings.cancel_return_player)
            || ((self.is_monster(effector) || self.is_raid(effector)) && !settings.cancel_return_mob)
        {
            return false;
        }
        // `!CANCEL_RETURN_PLAYER_OLYS && effected.isPlayer() && effected.isInOlympiadMode()`
        // (M8-OLY-01).
        if !settings.cancel_return_player_olys && self.is_player(effected) && self.is_in_olympiad_mode(effected) {
            return false;
        }
        true
    }

    /// Schedules the return of `taken` to `target` in `TIME_TO_RETURN` milliseconds.
    pub(crate) fn schedule_cancel_return(
        &mut self,
        target: ObjectId,
        monsters_too: bool,
        taken: Vec<TakenBuff>,
    ) {
        self.effects.next_return += 1;
        let id = self.effects.next_return;
        self.effects.pending_returns.insert(
            id,
            PendingReturn {
                target,
                monsters_too,
                taken,
            },
        );
        let delay = u64::try_from(self.settings.effects.cancel_return_time_ms).unwrap_or(0);
        let due = self.tick + TimerWheel::ms_to_ticks(delay);
        self.timers
            .schedule_at(due, TimerKind::Control(ControlTimer::CancelReturn(id)));
    }

    /// The task of the dispel effects: the buffs that are not on the creature again come back with
    /// the time they had left.
    pub(crate) fn on_cancel_return_timer(&mut self, id: u64) {
        let Some(pending) = self.effects.pending_returns.remove(&id) else {
            return;
        };
        let target = pending.target;
        // `effected.isDead()`, a player who is offline, a creature that is neither player nor
        // (for `DispelAll`) monster.
        if self.is_dead(target) {
            return;
        }
        if self.is_player(target) {
            if !self.players.get(&target).is_some_and(|p| p.entered_world) {
                return;
            }
        } else if !(pending.monsters_too && self.is_monster(target)) {
            return;
        }
        let now = self.tick;
        for taken in pending.taken {
            let time_left = taken.time(now);
            if time_left <= 0 {
                continue;
            }
            // `sk == null`: the skill table was replaced.
            if self
                .skill_table()
                .is_none_or(|t| t.get(taken.skill.id, taken.skill.level).is_none())
            {
                continue;
            }
            if self
                .effect_list(target)
                .is_some_and(|l| l.info_by_skill_id(taken.skill.id).is_some())
            {
                continue;
            }
            self.apply_effects(target, target, taken.skill);
            let new_info = self
                .effect_list(target)
                .and_then(|l| l.info_by_skill_id(taken.skill.id))
                .map(|i| i.id());
            if let (Some(info), Some(list)) = (new_info, self.effects.lists.get_mut(&target))
                && let Some(info) = list.get_mut(info)
            {
                info.set_abnormal_time(time_left);
            }
        }
    }
}
