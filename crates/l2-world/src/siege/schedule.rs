//! The siege schedule: the next siege date, the start and end tasks, the registration period.
// Java: mechanics/siege/Siege.java (ScheduleStartSiegeTask, ScheduleEndSiegeTask, startAutoTask,
//       correctSiegeDateTime, setNextSiegeDate, saveSiegeDate, saveCastleSiege,
//       endTimeRegistration)
// Java: managers/CastleManager.java (registerSiegeDate, getSiegeDates)
// Java: network/serverpackets/SiegeInfo.java (the hours of `SiegeHourList`)
// Java: network/clientpackets/RequestSetCastleSiegeTime.java (isSiegeTimeValid)
//
// # Calendars
// Java computes the dates with `Calendar.getInstance()` in the JVM's zone (D-48). The functions
// here take the offset of that zone as a function of the UTC time, so tests can fix it. A week
// starts on Sunday (`Calendar.SUNDAY`, the en_US locale): `set(DAY_OF_WEEK, day)` moves to that
// day of the Sunday-to-Saturday week the date is in, like the fishing championship
// (`fishing/championship.rs`).

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use tracing::{info, warn};

use super::SiegeTimer;
use crate::residence::{CastleId, ResidenceReadThen, ResidenceRef, ResidenceWrite};
use crate::world::World;

const SECOND_MS: i64 = 1000;
const MINUTE_MS: i64 = 60 * SECOND_MS;
const HOUR_MS: i64 = 60 * MINUTE_MS;
const DAY_MS: i64 = 24 * HOUR_MS;

/// The local day number (days since 1970-01-01 in the zone) and the local milliseconds of `utc`.
fn local_day_and_ms(utc: i64, offset_at: &dyn Fn(i64) -> i64) -> (i64, i64) {
    let local = utc + offset_at(utc);
    (local.div_euclid(DAY_MS), local.rem_euclid(DAY_MS))
}

/// Back from local milliseconds to UTC (the offset of the result, like `Calendar` resolving the
/// fields).
fn from_local(local: i64, offset_at: &dyn Fn(i64) -> i64) -> i64 {
    let guess = local - offset_at(local);
    local - offset_at(guess)
}

/// Java `Calendar.add(DAY_OF_MONTH, days)`: the same local time `days` later.
pub(crate) fn add_local_days(utc: i64, days: i64, offset_at: &dyn Fn(i64) -> i64) -> i64 {
    let local = utc + offset_at(utc);
    from_local(local + days * DAY_MS, offset_at)
}

/// Java `Siege.setNextSiegeDate`'s calendar: from the castle's siege date (or now, when it is
/// over) the day of the week and the hour of the schedule, minute and second 0 (the milliseconds
/// stay), and `siege_cycle` weeks later when that is before now.
pub fn next_siege_date(
    siege_date: i64,
    now: i64,
    day: i32,
    hour: i32,
    siege_cycle: i32,
    offset_at: &dyn Fn(i64) -> i64,
) -> i64 {
    let base = if siege_date < now { now } else { siege_date };
    let (local_day, local_ms) = local_day_and_ms(base, offset_at);
    let millis = local_ms.rem_euclid(SECOND_MS);
    // 1970-01-01 was a Thursday: 0 = Sunday ... 6 = Saturday.
    let weekday = (local_day + 4).rem_euclid(7);
    let target_day = local_day + (i64::from(day) - 1 - weekday);
    let mut local = target_day * DAY_MS + i64::from(hour) * HOUR_MS + millis;
    let mut result = from_local(local, offset_at);
    // `if (calendar.before(Calendar.getInstance())) calendar.add(WEEK_OF_YEAR, siegeCycle)`.
    if result < now {
        local += i64::from(siege_cycle) * 7 * DAY_MS;
        result = from_local(local, offset_at);
    }
    result
}

/// `SiegeInfo`'s hours: the siege date with minute and second 0 (the milliseconds stay) at each
/// hour of `SiegeHourList`, in epoch milliseconds.
pub fn siege_hour_times(siege_date: i64, hours: &[i32], offset_at: &dyn Fn(i64) -> i64) -> Vec<i64> {
    let (day, ms) = local_day_and_ms(siege_date, offset_at);
    let millis = ms.rem_euclid(SECOND_MS);
    hours
        .iter()
        .map(|hour| from_local(day * DAY_MS + i64::from(*hour) * HOUR_MS + millis, offset_at))
        .collect()
}

/// Java `RequestSetCastleSiegeTime.isSiegeTimeValid`: the chosen time is one of the hours of
/// `SiegeHourList` on the siege day.
// NOTE(java-quirk): Java compares `Calendar.HOUR` (the 12 hour clock), so 4:00 matches 16:00 of
// the list, and it never compares the milliseconds.
pub(crate) fn is_siege_time_valid(
    siege_date: i64,
    chosen: i64,
    hours: &[i32],
    offset_at: &dyn Fn(i64) -> i64,
) -> bool {
    let (siege_day, _) = local_day_and_ms(siege_date, offset_at);
    let (chosen_day, chosen_ms) = local_day_and_ms(chosen, offset_at);
    let chosen_hour = chosen_ms / HOUR_MS;
    let chosen_minute = (chosen_ms / MINUTE_MS) % 60;
    let chosen_second = (chosen_ms / SECOND_MS) % 60;
    hours.iter().any(|hour| {
        // `cal1.set(HOUR_OF_DAY, hour)` is lenient: 24 is the next day's midnight.
        let total = siege_day * 24 + i64::from(*hour);
        let (day, hour_of_day) = (total.div_euclid(24), total.rem_euclid(24));
        day == chosen_day && hour_of_day % 12 == chosen_hour % 12 && chosen_minute == 0 && chosen_second == 0
    })
}

impl World {
    // ------------------------------------------------------------------ startAutoTask

    /// Java `Siege.startAutoTask()`: corrects a siege date that is over, logs it, loads the siege
    /// clans and arms the start task (1 s).
    pub(crate) fn siege_start_auto_task_at(&mut self, id: CastleId, now: i64) {
        self.siege_correct_siege_date_time(id, now);
        self.log_siege_date(id);
        self.siege_load_siege_clan(id);
        // Schedule siege auto start.
        let old = self.castle_siege(id).and_then(|s| s.start_timer);
        if let Some(old) = old {
            self.timers.cancel(old);
        }
        let timer = self.schedule_siege_timer(SiegeTimer::Start(id), 1000);
        if let Some(siege) = self.castle_siege_mut(id) {
            siege.start_timer = Some(timer);
        }
    }

    /// Java `Siege.correctSiegeDateTime()`: a siege date in the past (the server was down) moves
    /// to the next one and is saved.
    fn siege_correct_siege_date_time(&mut self, id: CastleId, now: i64) {
        let Some(date) = self.castle(id).map(|c| c.siege_date) else {
            return;
        };
        if date < now {
            // Since siege has past reschedule it to the next one.
            self.siege_set_next_siege_date(id, now);
            self.siege_save_siege_date(id);
        }
    }

    /// Java `Siege.setNextSiegeDate()`.
    pub(crate) fn siege_set_next_siege_date(&mut self, id: CastleId, now: i64) {
        let Some(holder) = self.siege_schedule().for_castle(id.0).copied() else {
            return;
        };
        if !holder.siege_enabled {
            return;
        }
        let Some(date) = self.castle(id).map(|c| c.siege_date) else {
            return;
        };
        let cycle = self.sieges.settings.siege_cycle;
        let offset = |t| self.siege_offset_at(t);
        let next = next_siege_date(date, now, holder.day, holder.hour, cycle, &offset);
        if let Some(castle) = self.castle_mut(id) {
            castle.siege_date = next;
        }
        let allowed = self.castle_siege_date_count(next) < holder.max_concurrent;
        if allowed {
            self.sieges.castle_siege_dates.insert(id.0, next);
            self.broadcast_to_all_online_players(
                &SystemMessage::new(SystemMessageId::S1HasAnnouncedTheNextCastleSiegeTime)
                    .with(SystemMessageParam::CastleName(id.0)),
            );
        }
        // Allow (or deny) registration for next siege.
        if let Some(siege) = self.castle_siege_mut(id) {
            siege.registration_over = !allowed;
        }
    }

    /// Java `CastleManager.getSiegeDates(siegeDate)`: the castles whose siege is within a second.
    fn castle_siege_date_count(&self, siege_date: i64) -> i32 {
        let count = self
            .sieges
            .castle_siege_dates
            .values()
            .filter(|date| (*date - siege_date).abs() < 1000)
            .count();
        i32::try_from(count).unwrap_or(i32::MAX)
    }

    /// Java `Siege.saveSiegeDate()`: the start task is armed again (1 s) when there is one, and
    /// the dates are written (D-51: only with `SiegeDatabaseWrites`).
    pub(crate) fn siege_save_siege_date(&mut self, id: CastleId) {
        let old = self.castle_siege(id).and_then(|s| s.start_timer);
        if let Some(old) = old {
            self.timers.cancel(old);
            let timer = self.schedule_siege_timer(SiegeTimer::Start(id), 1000);
            if let Some(siege) = self.castle_siege_mut(id) {
                siege.start_timer = Some(timer);
            }
        }
        let Some(castle) = self.castle(id) else {
            return;
        };
        let write = ResidenceWrite::CastleSiegeDate {
            castle_id: id.0,
            siege_date: castle.siege_date,
            reg_time_end: castle.time_registration_over_date,
            reg_time_over: castle.time_registration_over,
        };
        self.siege_job_write(write);
    }

    /// Java `Siege.saveCastleSiege()`: the next siege date, the registration period ends a day
    /// from now, then the schedule starts again.
    pub(crate) fn siege_save_castle_siege(&mut self, id: CastleId) {
        let now = self.siege_now_ms();
        self.siege_set_next_siege_date(id, now);
        let offset = |t| self.siege_offset_at(t);
        let reg_end = add_local_days(now, 1, &offset);
        if let Some(castle) = self.castle_mut(id) {
            castle.time_registration_over_date = reg_end;
            castle.time_registration_over = false;
        }
        self.siege_save_siege_date(id);
        self.siege_start_auto_task_at(id, now);
    }

    /// Java `Siege.endTimeRegistration(automatic)`.
    pub(crate) fn siege_end_time_registration(&mut self, id: CastleId, automatic: bool) {
        if let Some(castle) = self.castle_mut(id) {
            castle.time_registration_over = true;
        }
        if !automatic {
            self.siege_save_siege_date(id);
        }
    }

    // ------------------------------------------------------------------ the tasks

    /// Java `ScheduleStartSiegeTask.run()`.
    pub(crate) fn on_siege_start_task(&mut self, id: CastleId) {
        let Some(in_progress) = self.castle_siege(id).map(|s| s.in_progress) else {
            return;
        };
        if in_progress {
            return;
        }
        let now = self.siege_now_ms();
        let Some((reg_over, reg_end, siege_date)) = self.castle(id).map(|c| {
            (
                c.time_registration_over,
                c.time_registration_over_date,
                c.siege_date,
            )
        }) else {
            return;
        };
        if !reg_over {
            let reg_remaining = reg_end - now;
            if reg_remaining > 0 {
                self.siege_rearm_start(id, reg_remaining);
                return;
            }
            self.siege_end_time_registration(id, true);
        }
        let remaining = siege_date - now;
        if remaining > 86_400_000 {
            // Prepare task for 24 before siege start to end registration.
            self.siege_rearm_start(id, remaining - 86_400_000);
        } else if remaining > 13_600_000 {
            self.broadcast_to_all_online_players(
                &SystemMessage::new(SystemMessageId::TheRegistrationTermForS1HasEnded)
                    .with(SystemMessageParam::CastleName(id.0)),
            );
            if let Some(siege) = self.castle_siege_mut(id) {
                siege.registration_over = true;
            }
            self.siege_clear_siege_waiting_clan(id);
            // Prepare task for 1 hr left before siege start.
            self.siege_rearm_start(id, remaining - 13_600_000);
        } else if remaining > 600_000 {
            self.siege_rearm_start(id, remaining - 600_000);
        } else if remaining > 300_000 {
            self.siege_rearm_start(id, remaining - 300_000);
        } else if remaining > 10_000 {
            self.siege_rearm_start(id, remaining - 10_000);
        } else if remaining > 0 {
            self.siege_rearm_start(id, remaining);
        } else if self.siege_database_writes() {
            self.start_siege(id);
        } else {
            // D-51: the Java game server runs the real siege; this world reads the castle and its
            // registrations again once that siege is over.
            info!(
                "SideBySide: the siege of {} runs on the Java game server (SiegeDatabaseWrites = False).",
                self.residence_name(ResidenceRef::Castle(id))
            );
            let length_ms = i64::from(self.sieges.settings.siege_length) * MINUTE_MS;
            self.schedule_siege_timer(SiegeTimer::SideBySideReread(id), length_ms + 10 * MINUTE_MS);
        }
    }

    fn siege_rearm_start(&mut self, id: CastleId, delay_ms: i64) {
        let timer = self.schedule_siege_timer(SiegeTimer::Start(id), delay_ms);
        if let Some(siege) = self.castle_siege_mut(id) {
            siege.start_timer = Some(timer);
        }
    }

    /// Java `ScheduleEndSiegeTask.run()`: the countdown messages to both sides, then `endSiege`.
    pub(crate) fn on_siege_end_task(&mut self, id: CastleId) {
        let Some((in_progress, end)) = self.castle_siege(id).map(|s| (s.in_progress, s.siege_end_date))
        else {
            return;
        };
        if !in_progress {
            return;
        }
        let remaining = end - self.siege_now_ms();
        let minutes = |ms: i64| i32::try_from(ms / 60_000).unwrap_or(i32::MAX);
        let message = |id: SystemMessageId, value: i32| {
            SystemMessage::new(id).with(SystemMessageParam::IntNumber(value))
        };
        if remaining > 3_600_000 {
            self.siege_announce_to_player(
                id,
                &message(SystemMessageId::S1HourSUntilCastleSiegeConclusion, 2),
                true,
            );
            // Prepare task for 1 hr left.
            self.schedule_siege_timer(SiegeTimer::End(id), remaining - 3_600_000);
        } else if remaining > 600_000 {
            let sm = message(
                SystemMessageId::S1MinuteSUntilCastleSiegeConclusion,
                minutes(remaining),
            );
            self.siege_announce_to_player(id, &sm, true);
            // Prepare task for 10 minute left.
            self.schedule_siege_timer(SiegeTimer::End(id), remaining - 600_000);
        } else if remaining > 300_000 {
            let sm = message(
                SystemMessageId::S1MinuteSUntilCastleSiegeConclusion,
                minutes(remaining),
            );
            self.siege_announce_to_player(id, &sm, true);
            // Prepare task for 5 minute left.
            self.schedule_siege_timer(SiegeTimer::End(id), remaining - 300_000);
        } else if remaining > 10_000 {
            let sm = message(
                SystemMessageId::S1MinuteSUntilCastleSiegeConclusion,
                minutes(remaining),
            );
            self.siege_announce_to_player(id, &sm, true);
            // Prepare task for 10 seconds count down.
            self.schedule_siege_timer(SiegeTimer::End(id), remaining - 10_000);
        } else if remaining > 0 {
            let seconds = i32::try_from(remaining / 1000).unwrap_or(i32::MAX);
            let sm = message(SystemMessageId::ThisCastleSiegeWillEndInS1SecondS, seconds);
            self.siege_announce_to_player(id, &sm, true);
            // Prepare task for second count down.
            self.schedule_siege_timer(SiegeTimer::End(id), remaining);
        } else {
            self.end_siege(id);
        }
    }

    /// D-51: the siege of the Java game server is over; the castle (owner, dates) and the
    /// registrations are read again, then the schedule starts again from the new date.
    pub(crate) fn on_side_by_side_reread_timer(&mut self, id: CastleId) {
        if self.castle_siege(id).is_some_and(|s| s.in_progress) {
            return;
        }
        self.request_siege_reread(id, ResidenceReadThen::SiegeRestartSchedule { castle_id: id.0 });
    }

    /// The follow-up of [`Self::on_side_by_side_reread_timer`].
    pub(crate) fn siege_restart_schedule(&mut self, id: CastleId) {
        let enabled = self
            .siege_schedule()
            .for_castle(id.0)
            .is_some_and(|s| s.siege_enabled);
        if !enabled {
            return;
        }
        let now = self.siege_now_ms();
        self.siege_start_auto_task_at(id, now);
    }

    /// `RequestSetCastleSiegeTime.runImpl`: the castle lord picks the hour of the siege.
    pub(crate) fn on_request_set_castle_siege_time(
        &mut self,
        player: ObjectId,
        castle_id: i32,
        time_secs: i32,
    ) {
        let id = CastleId(castle_id);
        // `_time = readInt(); _time *= 1000;`
        let time = i64::from(time_secs) * 1000;
        let Some((owner, reg_over, siege_date)) = self
            .castle(id)
            .map(|c| (c.owner_id, c.time_registration_over, c.siege_date))
        else {
            warn!("[C]RequestSetCastleSiegeTime: activeChar: {player} castle: null castleId: {castle_id}");
            return;
        };
        if !self.players.contains_key(&player) {
            return;
        }
        if owner > 0 && owner != self.clan_id_of(player) {
            warn!(
                "[C]RequestSetCastleSiegeTime: {player} castleId: {castle_id} is trying to change siege date of not his own castle!"
            );
        } else if !self.is_clan_leader(player) {
            warn!(
                "[C]RequestSetCastleSiegeTime: {player} castleId: {castle_id} is trying to change siege date but is not clan leader!"
            );
        } else if !reg_over {
            let hours = self.sieges.settings.siege_hour_list.clone();
            let offset = |t| self.siege_offset_at(t);
            if is_siege_time_valid(siege_date, time, &hours, &offset) {
                if let Some(castle) = self.castle_mut(id) {
                    castle.siege_date = time;
                    castle.time_registration_over = true;
                }
                self.siege_save_siege_date(id);
                self.broadcast_to_all_online_players(
                    &SystemMessage::new(SystemMessageId::S1HasAnnouncedTheNextCastleSiegeTime)
                        .with(SystemMessageParam::CastleName(castle_id)),
                );
                self.send_siege_info(player, id);
            } else {
                warn!(
                    "[C]RequestSetCastleSiegeTime: {player} castleId: {castle_id} is trying to an invalid time ({time})!"
                );
            }
        } else {
            warn!(
                "[C]RequestSetCastleSiegeTime: {player} castleId: {castle_id} is trying to change siege date but currently not possible!"
            );
        }
    }
}

use l2_util::ObjectId;

#[cfg(test)]
mod tests {
    use super::*;

    fn utc(_: i64) -> i64 {
        0
    }

    /// 2026-10-21 (a Wednesday) 12:34:56.789 UTC.
    const WEDNESDAY: i64 = 1_792_586_096_789;

    #[test]
    fn the_next_siege_is_the_sunday_of_the_week_two_weeks_on() {
        // The week's Sunday (2026-10-18) 16:00 is before now: two weeks later, the millis stay.
        let next = next_siege_date(0, WEDNESDAY, 1, 16, 2, &utc);
        assert_eq!(next, 1_793_548_800_789);
        // A siege date in the future keeps its week.
        let later = next_siege_date(next, WEDNESDAY, 1, 20, 2, &utc);
        assert_eq!(later, next + 4 * HOUR_MS);
        // Saturday of the same week is still ahead.
        let saturday = next_siege_date(0, WEDNESDAY, 7, 16, 2, &utc);
        assert_eq!(saturday, 1_792_857_600_789);
    }

    #[test]
    fn the_zone_moves_the_local_hour() {
        let berlin = |_: i64| HOUR_MS;
        let next = next_siege_date(0, WEDNESDAY, 1, 16, 2, &berlin);
        assert_eq!(next, 1_793_548_800_789 - HOUR_MS);
    }

    #[test]
    fn the_siege_hours_and_their_check_follow_java() {
        let date = 1_793_548_800_789; // a Sunday 16:00:00.789
        let hours = siege_hour_times(date, &[16, 20], &utc);
        assert_eq!(hours, vec![date, date + 4 * HOUR_MS]);
        let exact = date - 789 + 4 * HOUR_MS;
        assert!(is_siege_time_valid(date, exact, &[16, 20], &utc));
        // The 12 hour clock: 8:00 matches 20:00.
        assert!(is_siege_time_valid(date, exact - 12 * HOUR_MS, &[16, 20], &utc));
        assert!(!is_siege_time_valid(date, exact + MINUTE_MS, &[16, 20], &utc));
        assert!(!is_siege_time_valid(date, exact + DAY_MS, &[16, 20], &utc));
        assert!(!is_siege_time_valid(date, exact, &[], &utc));
    }
}
