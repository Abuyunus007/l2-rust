//! `FortUpdater`: the periodic jobs of an owned fortress (the blood oaths, the supply level and the
//! contract with a castle, and the end of the owner's time).
// Java: mechanics/siege/FortUpdater.java, FortUpdaterType.java
// Java: mechanics/siege/Fort.java (load, updateOwnerInDB: where the two tasks are started and
//       cancelled; getTimeTillRebelArmy, getTimeTillNextFortUpdate)
// Java: entity/clan/Clan.java (increaseBloodOathCount)
//
// # The tasks
// `_fortUpdater[0]` is the periodic update (every `FortressPeriodicUpdateFrequency` minutes),
// `_fortUpdater[1]` the owner's time limit (every hour while `FortressMaximumOwnTime` is above 0,
// or once, a minute after the start, when the owner held the fortress too long already). Both
// run on the timer wheel ([`FortTimer::Updater`]).
//
// # Side by side (D-49, D-51)
// Java runs the same jobs on both servers. Here the blood oaths count up in memory and the clan's
// row is written with the switch; the part that moves adena (the fee for the contracted castle,
// the supply level and the fall back to an independent fortress when the clan cannot pay) runs only
// with `SiegeDatabaseWrites`, because it changes the clan warehouse and the castle treasury,
// which the Java game server charges for real (see `residence/functions.rs` for the same rule).

use tracing::warn;

use super::{FortTimer, siege::FortSiegeWrite};
use crate::residence::{CastleId, FortId, Writes};
use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::{World, unix_ms_now};

/// Java `FortUpdaterType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FortUpdaterType {
    /// `PERIODIC_UPDATE`.
    PeriodicUpdate,
    /// `MAX_OWN_TIME`.
    MaxOwnTime,
}

impl FortUpdaterType {
    /// The index in `_fortUpdater`.
    fn slot(self) -> usize {
        match self {
            Self::PeriodicUpdate => 0,
            Self::MaxOwnTime => 1,
        }
    }
}

/// One scheduled `FortUpdater` (`_fortUpdater[i]` with the `_clan` and `_runCount` of the task).
#[derive(Debug, Clone, Copy)]
pub(crate) struct UpdaterTask {
    pub(crate) timer: TimerId,
    /// The world tick the task runs next.
    pub(crate) due_tick: u64,
    /// The `_clan` the task was made for.
    pub(crate) clan: i32,
    pub(crate) run_count: i32,
    /// The period of a `scheduleAtFixedRate` task, `None` for a task that runs once.
    pub(crate) period_ms: Option<u64>,
}

impl World {
    /// Java `scheduleAtFixedRate(new FortUpdater(...), initial, period)` or `schedule(..., initial)`.
    fn fort_schedule_updater(
        &mut self,
        fort: FortId,
        kind: FortUpdaterType,
        clan: i32,
        run_count: i32,
        initial_ms: i64,
        period_ms: Option<u64>,
    ) {
        let due = self.tick + TimerWheel::ms_to_ticks(u64::try_from(initial_ms).unwrap_or(0));
        let timer = self
            .timers
            .schedule_at(due, TimerKind::Fort(FortTimer::Updater { fort, kind }));
        self.fort.state_mut(fort).updaters[kind.slot()] = Some(UpdaterTask {
            timer,
            due_tick: due,
            clan,
            run_count,
            period_ms,
        });
    }

    /// `_fortUpdater[i].cancel(false)`; the field is nulled by the caller where Java does.
    fn fort_cancel_updater(&mut self, fort: FortId, slot: usize, forget: bool) {
        let state = self.fort.state_mut(fort);
        if let Some(task) = state.updaters[slot] {
            self.timers.cancel(task.timer);
            if forget {
                self.fort.state_mut(fort).updaters[slot] = None;
            }
        }
    }

    /// The tasks `Fort.load()` starts for an owned fortress.
    pub(crate) fn fort_start_updaters_on_load(&mut self, fort: FortId) {
        let Some((clan, last_owned, owned_time)) = self
            .fort(fort)
            .filter(|f| f.owner_id != 0)
            .map(|f| (f.owner_id, f.last_owned_time, f.owned_time(unix_ms_now())))
        else {
            return;
        };
        let settings = &self.fort.settings;
        let period = i64::from(settings.fs_update_frq) * 60_000;
        let (max_own_time, hour) = (settings.fs_max_own_time, 3_600_000_i64);
        let run_count = owned_time / (settings.fs_update_frq * 60).max(1);
        let mut initial = unix_ms_now() - last_owned;
        while period > 0 && initial > period {
            initial -= period;
        }
        let initial = period - initial;
        if max_own_time <= 0 || owned_time < max_own_time.saturating_mul(3600) {
            self.fort_schedule_updater(
                fort,
                FortUpdaterType::PeriodicUpdate,
                clan,
                run_count,
                initial,
                Some(u64::try_from(period).unwrap_or(0)),
            );
            if max_own_time > 0 {
                self.fort_schedule_updater(
                    fort,
                    FortUpdaterType::MaxOwnTime,
                    clan,
                    run_count,
                    hour,
                    Some(hour.unsigned_abs()),
                );
            }
        } else {
            // The owner held it too long already: "Schedule owner tasks to remove owner."
            self.fort_schedule_updater(fort, FortUpdaterType::MaxOwnTime, clan, 0, 60_000, None);
        }
    }

    /// The tasks `Fort.updateOwnerInDB()` starts for a new owner, and stops for none.
    pub(crate) fn fort_restart_updaters(&mut self, fort: FortId, owner: Option<i32>) {
        match owner {
            Some(clan) => {
                self.fort_cancel_updater(fort, 0, false);
                self.fort_cancel_updater(fort, 1, false);
                let settings = &self.fort.settings;
                let period = i64::from(settings.fs_update_frq) * 60_000;
                let max_own_time = settings.fs_max_own_time;
                self.fort_schedule_updater(
                    fort,
                    FortUpdaterType::PeriodicUpdate,
                    clan,
                    0,
                    period,
                    Some(u64::try_from(period).unwrap_or(0)),
                );
                if max_own_time > 0 {
                    self.fort_schedule_updater(
                        fort,
                        FortUpdaterType::MaxOwnTime,
                        clan,
                        0,
                        3_600_000,
                        Some(3_600_000),
                    );
                }
                // NOTE(java-quirk): with `FortressMaximumOwnTime` off, Java keeps the old
                // `_fortUpdater[1]` future of an earlier owner (it was cancelled, not nulled).
            }
            None => {
                self.fort_cancel_updater(fort, 0, true);
                self.fort_cancel_updater(fort, 1, true);
            }
        }
    }

    /// Java `getTimeTillNextFortUpdate()`: seconds to the next periodic update (0 without one).
    pub fn fort_time_till_next_update(&self, fort: FortId) -> i64 {
        let Some(task) = self.fort.forts.get(&fort).and_then(|s| s.updaters[0]) else {
            return 0;
        };
        let ticks = task.due_tick.saturating_sub(self.tick);
        i64::try_from(ticks).unwrap_or(0) * crate::world::TICK.as_millis() as i64 / 1000
    }

    /// Java `getTimeTillRebelArmy()`: seconds until the owner's time is over.
    pub fn fort_time_till_rebel_army(&self, fort: FortId) -> i32 {
        let Some(f) = self.fort(fort) else {
            return 0;
        };
        if f.last_owned_time == 0 {
            return 0;
        }
        let limit = i64::from(self.fort.settings.fs_max_own_time) * 3_600_000;
        // The Java expression is an `int` division of a `long` sum: a `long` result narrowed.
        (((f.last_owned_time + limit) - unix_ms_now()) / 1000) as i32
    }

    /// `FortUpdater.run()`.
    pub(crate) fn on_fort_updater(&mut self, fort: FortId, kind: FortUpdaterType) {
        let slot = kind.slot();
        let Some(mut task) = self.fort.forts.get(&fort).and_then(|s| s.updaters[slot]) else {
            return;
        };
        // `scheduleAtFixedRate`: the next run is on the wheel before this one starts.
        if let Some(period) = task.period_ms {
            let due = self.tick + TimerWheel::ms_to_ticks(period);
            task.timer = self
                .timers
                .schedule_at(due, TimerKind::Fort(FortTimer::Updater { fort, kind }));
            task.due_tick = due;
        }
        if kind == FortUpdaterType::PeriodicUpdate {
            task.run_count += 1;
        }
        self.fort.state_mut(fort).updaters[slot] = Some(task);
        let owner = self.fort(fort).map_or(0, |f| f.owner_id);
        if owner == 0 || owner != task.clan {
            return;
        }
        match kind {
            FortUpdaterType::PeriodicUpdate => self.fort_periodic_update(fort, owner),
            FortUpdaterType::MaxOwnTime => {
                let owned = self.fort(fort).map_or(0, |f| f.owned_time(unix_ms_now()));
                if i64::from(owned) > i64::from(self.fort.settings.fs_max_own_time) * 3600 {
                    self.fort_remove_owner(fort, true);
                    self.fort_set_state(fort, 0, 0, Writes::Job);
                }
            }
        }
    }

    /// The `PERIODIC_UPDATE` case.
    fn fort_periodic_update(&mut self, fort: FortId, clan: i32) {
        // `getOwnerClan().increaseBloodOathCount()`.
        let add = self.fort.settings.fs_blood_oath_count;
        if let Some(c) = self.clan_mut(clan) {
            c.blood_oath_count += add;
            let count = c.blood_oath_count;
            self.siege_write(crate::residence::ResidenceWrite::FortSiege(
                FortSiegeWrite::BloodOathCount { clan_id: clan, count },
            ));
        }
        let (state, castle) = self.fort(fort).map_or((0, 0), |f| (f.state, f.castle_id));
        // The part that moves adena runs where Java's does: this world is the one that charges
        // (D-51).
        if state == 2 && self.siege_database_writes() {
            let fee = i64::from(self.fort.settings.fs_fee_for_castle);
            if self.clan_warehouse_adena(clan) >= fee {
                self.clan_warehouse_destroy_adena(clan, fee);
                if self.castle(CastleId(castle)).is_some() {
                    self.castle_add_to_treasury_no_tax(CastleId(castle), fee, Writes::Job);
                } else {
                    warn!("{fort}: the contracted castle {castle} does not exist.");
                }
                self.fort_raise_supply_level(fort);
            } else {
                self.fort_set_state(fort, 1, 0, Writes::Job);
            }
        }
        self.fort_save_variables(fort, Writes::Job);
    }
}
