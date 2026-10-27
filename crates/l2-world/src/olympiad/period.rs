//! Java `Olympiad`: the load, the competition periods, the weekly change, the period end (the
//! heroes to be, the monthly copy, the ranks and passes) and the validation end.
// Java: mechanics/olympiad/Olympiad.java

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use tracing::{info, warn};

use super::{
    HeroToBe, NobleStats, OlympiadData, OlympiadLoad, OlympiadSettings, OlympiadTimer, OlympiadWrite,
    UNCLAIMED_OLYMPIAD_PASSES_VAR,
};
use crate::outbox::PersistCommand;
use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::{World, unix_ms_now};

/// Java `Olympiad.HERO_IDS`: the third classes that get a hero.
pub(crate) const HERO_IDS: [i32; 35] = [
    88, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110,
    111, 112, 113, 114, 115, 116, 117, 118, 131, 132, 133, 134,
];

const SECOND_MS: i64 = 1_000;
const MINUTE_MS: i64 = 60 * SECOND_MS;
const HOUR_MS: i64 = 60 * MINUTE_MS;
const DAY_MS: i64 = 24 * HOUR_MS;
/// The game manager and the announcer start 30 s after the competition (`scheduleAtFixedRate`).
const GAME_MANAGER_MS: i64 = 30_000;
const ANNOUNCER_MS: i64 = 500;
/// "wait 1 minutes for end of pending games".
const COMP_END_WAIT_MS: i64 = 60_000;
/// The registration closes ten minutes before the competition ends.
const REGISTRATION_END_MS: i64 = 600_000;

// ------------------------------------------------------------------------------- the calendar

/// Year, month (1 to 12) and day of a day count since 1970-01-01.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// Days since 1970-01-01 of a civil date.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn days_in_month(year: i64, month: i64) -> i64 {
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    days_from_civil(next_year, next_month, 1) - days_from_civil(year, month, 1)
}

/// A Java `Calendar` in the local zone: the day count and the time of day (local ms).
#[derive(Debug, Clone, Copy)]
struct Cal {
    day: i64,
    time_of_day: i64,
}

impl Cal {
    /// `Calendar.getInstance()` at `utc`.
    fn at(utc: i64, offset_at: &dyn Fn(i64) -> i64) -> Self {
        let local = utc + offset_at(utc);
        Self {
            day: local.div_euclid(DAY_MS),
            time_of_day: local.rem_euclid(DAY_MS),
        }
    }

    /// `getTimeInMillis()`: back to UTC with the offset of the result.
    fn millis(self, offset_at: &dyn Fn(i64) -> i64) -> i64 {
        let local = self.day * DAY_MS + self.time_of_day;
        let guess = local - offset_at(local);
        local - offset_at(guess)
    }

    /// `get(DAY_OF_WEEK)`: Sunday is 1, Saturday 7 (1970-01-01 was a Thursday).
    fn day_of_week(self) -> i32 {
        i32::try_from((self.day + 4).rem_euclid(7)).unwrap_or(0) + 1
    }

    /// `set(HOUR_OF_DAY, h)`, `set(MINUTE, m)` (seconds and milliseconds stay).
    fn set_hour_minute(&mut self, hour: i32, minute: i32) {
        let seconds = self.time_of_day.rem_euclid(MINUTE_MS);
        let local = self.day * DAY_MS + i64::from(hour) * HOUR_MS + i64::from(minute) * MINUTE_MS + seconds;
        // Lenient: an hour of 24 is the next day.
        self.day = local.div_euclid(DAY_MS);
        self.time_of_day = local.rem_euclid(DAY_MS);
    }

    /// `set(HOUR_OF_DAY, h)`, `set(MINUTE, 0)`, `set(SECOND, 0)` (the milliseconds stay).
    fn set_hour_zero(&mut self, hour: i32) {
        let millis = self.time_of_day.rem_euclid(SECOND_MS);
        self.time_of_day = i64::from(hour) * HOUR_MS + millis;
    }

    /// `add(DAY_OF_MONTH, days)` (the wall clock time stays).
    fn add_days(&mut self, days: i64) {
        self.day += days;
    }

    /// `add(MONTH, months)`: the day of the month is cut to the length of the new month.
    fn add_months(&mut self, months: i64) {
        let (year, month, dom) = civil_from_days(self.day);
        let index = year * 12 + (month - 1) + months;
        let (new_year, new_month) = (index.div_euclid(12), index.rem_euclid(12) + 1);
        self.day = days_from_civil(new_year, new_month, dom.min(days_in_month(new_year, new_month)));
    }

    /// `set(DAY_OF_MONTH, 1)`.
    fn set_first_of_month(&mut self) {
        let (year, month, _) = civil_from_days(self.day);
        self.day = days_from_civil(year, month, 1);
    }
}

/// Java `DateTimeFormatter.ofPattern("yyyy-MM-dd HH")` (`minutes` false) or
/// `("yyyy-MM-dd HH:mm")` of a local time (ms since the local epoch), the formats of the hero
/// pages.
pub(super) fn format_local_date(local_ms: i64, minutes: bool) -> String {
    let (year, month, day) = civil_from_days(local_ms.div_euclid(DAY_MS));
    let time_of_day = local_ms.rem_euclid(DAY_MS);
    let hour = time_of_day / HOUR_MS;
    if minutes {
        format!(
            "{year:04}-{month:02}-{day:02} {hour:02}:{:02}",
            (time_of_day % HOUR_MS) / MINUTE_MS
        )
    } else {
        format!("{year:04}-{month:02}-{day:02} {hour:02}")
    }
}

/// `Hero.loadFights`' `from`: `Calendar.getInstance()` with `DAY_OF_MONTH` 1, `HOUR_OF_DAY` 0,
/// `MINUTE` 0 and `MILLISECOND` 0 (D-48). The seconds stay.
// NOTE(java-quirk): `SECOND` is not set, so the cut is the first of the month at 00:00:ss, ss
// being the second the server loaded the hero at.
pub fn hero_fights_cutoff_ms(now_utc_ms: i64, offset_at: &dyn Fn(i64) -> i64) -> i64 {
    let mut cal = Cal::at(now_utc_ms, offset_at);
    cal.set_first_of_month();
    cal.time_of_day = (cal.time_of_day.rem_euclid(MINUTE_MS) / SECOND_MS) * SECOND_MS;
    cal.millis(offset_at)
}

/// The `dayCounter` loops of `init()` and `setNewCompBegin()`: the days from `current_day` to the
/// next day of `OlympiadCompetitionDays`.
fn competition_day_counter(current_day: i32, days: &[i32]) -> i64 {
    let mut found = false;
    let mut counter = 0;
    for i in current_day..8 {
        if days.contains(&i) {
            found = true;
            break;
        }
        counter += 1;
    }
    if !found {
        for i in 1..8 {
            if days.contains(&i) {
                break;
            }
            counter += 1;
        }
    }
    counter
}

/// Java's `numDays, numHours, numMins` of a countdown, as the load and `updateCompStatus` log
/// it.
fn countdown(ms: i64) -> (i64, i64, i64) {
    let secs = (ms / 1000) % 60;
    let mut count = ((ms as f64 / 1000.0) - secs as f64) / 60.0;
    let mins = (count % 60.0) as i64;
    count = (count - mins as f64) / 60.0;
    let hours = (count % 24.0) as i64;
    let days = ((count - hours as f64) / 24.0) as i64;
    (days, hours, mins)
}

impl World {
    /// The `Olympiad.ini` values, the D-53 switch and the test clocks.
    pub fn set_olympiad_settings(&mut self, settings: OlympiadSettings) {
        self.olympiad.settings = settings;
    }

    pub fn olympiad_settings(&self) -> &OlympiadSettings {
        &self.olympiad.settings
    }

    /// `System.currentTimeMillis()` of the Olympiad.
    pub(crate) fn olympiad_now_ms(&self) -> i64 {
        match self.olympiad.settings.clock_override_unix_ms {
            Some(base) => base.saturating_add(self.world_ms()),
            None => unix_ms_now(),
        }
    }

    /// The zone offset at a time (D-48).
    pub(crate) fn olympiad_offset_at(&self, utc: i64) -> i64 {
        self.olympiad
            .settings
            .zone_offset_override_ms
            .unwrap_or_else(|| l2_util::time::local_offset_millis(utc))
    }

    /// [`hero_fights_cutoff_ms`] for the world's clock and zone.
    pub(crate) fn hero_fights_cutoff(&self) -> i64 {
        hero_fights_cutoff_ms(self.olympiad_now_ms(), &|t| self.olympiad_offset_at(t))
    }

    fn olympiad_cal_now(&self) -> Cal {
        let now = self.olympiad_now_ms();
        Cal::at(now, &|t| self.olympiad_offset_at(t))
    }

    fn olympiad_cal_millis(&self, cal: Cal) -> i64 {
        cal.millis(&|t| self.olympiad_offset_at(t))
    }

    /// Schedules an Olympiad timer `delay_ms` from now (a negative delay runs at the next tick).
    pub(crate) fn schedule_olympiad_timer(&mut self, timer: OlympiadTimer, delay_ms: i64) -> TimerId {
        let delay = u64::try_from(delay_ms.max(0)).unwrap_or(0);
        let due = self.tick + TimerWheel::ms_to_ticks(delay).max(1);
        self.timers.schedule_at(due, TimerKind::Olympiad(timer))
    }

    fn cancel_olympiad_timer(&mut self, timer: Option<TimerId>) {
        if let Some(timer) = timer {
            self.timers.cancel(timer);
        }
    }

    /// Sends an Olympiad write to the persistence task.
    pub(crate) fn olympiad_write(&mut self, write: OlympiadWrite) {
        self.outbox.persist(PersistCommand::Olympiad(write));
    }

    /// `CycleDatabaseWrites` (D-53).
    pub(crate) fn olympiad_writes_cycles(&self) -> bool {
        self.olympiad.settings.cycle_database_writes
    }

    // ------------------------------------------------------------------------------- the load

    /// Java `new Olympiad()`: `load()` (the rows the game server read, see [`OlympiadLoad`]),
    /// then `init()` in the competition period. Nothing when `OlympiadEnabled` is off.
    pub fn load_olympiad(&mut self, load: OlympiadLoad) {
        if !self.olympiad.settings.enabled {
            info!("Olympiad: Disabled.");
            return;
        }
        self.olympiad.loaded = true;
        if !self.olympiad_load(load) {
            return;
        }
        // `OlympiadGameManager` (the stadiums and their arena instances).
        self.olympiad_game_manager();
        if self.olympiad.data.period == 0 {
            self.olympiad_init();
        }
    }

    /// Java `load()`. `false` where Java returns early (an unknown period).
    fn olympiad_load(&mut self, load: OlympiadLoad) -> bool {
        self.olympiad.nobles.clear();
        self.olympiad.data = load.data.unwrap_or(OlympiadData {
            current_cycle: 1,
            period: 0,
            olympiad_end: 0,
            validation_end: 0,
            next_weekly_change: 0,
        });
        let now = self.olympiad_now_ms();
        // Java truncates the nobles before it reads them; the rows read at start are then gone.
        let mut nobles_deleted = false;
        match self.olympiad.data.period {
            0 => {
                if self.olympiad.data.olympiad_end == 0 || self.olympiad.data.olympiad_end < now {
                    self.set_new_olympiad_end();
                } else {
                    self.schedule_weekly_change();
                }
            }
            1 => {
                if self.olympiad.data.validation_end > now {
                    self.load_nobles_rank(&load.classified);
                    let delay = self.millis_to_validation_end();
                    self.olympiad.validation_task =
                        Some(self.schedule_olympiad_timer(OlympiadTimer::ValidationEnd, delay));
                } else {
                    self.olympiad.data.current_cycle += 1;
                    self.olympiad.data.period = 0;
                    self.delete_nobles();
                    nobles_deleted = true;
                    self.set_new_olympiad_end();
                }
            }
            period => {
                warn!("Olympiad System: Omg something went wrong in loading!! Period = {period}");
                return false;
            }
        }

        if !nobles_deleted {
            let max_points = self.olympiad.settings.max_points;
            let weekly = self.olympiad.settings.weekly_points;
            for row in load.nobles {
                let mut stats = row.stats;
                let cap = max_points
                    .wrapping_mul(stats.comp_done)
                    .wrapping_add(weekly.wrapping_mul(4));
                stats.points = stats.points.clamp(0, cap.max(0));
                stats.to_save = false;
                self.olympiad.nobles.insert(row.char_id, stats);
            }
        }

        info!("Olympiad System: Loading....");
        let period = self.olympiad.data.period;
        if period == 0 {
            info!("Olympiad System: Currently in Olympiad Period");
        } else {
            info!("Olympiad System: Currently in Validation Period");
        }
        let milli_to_end = if period == 0 {
            self.millis_to_olympiad_end()
        } else {
            self.millis_to_validation_end()
        };
        let (days, hours, mins) = countdown(milli_to_end);
        info!("Olympiad System: {days} days, {hours} hours and {mins} mins until period ends.");
        if period == 0 {
            let (days, hours, mins) = countdown(self.olympiad_millis_to_week_change());
            info!("Olympiad System: Next weekly change is in {days} days, {hours} hours and {mins} mins.");
        }
        info!("Olympiad System: Loaded {} Nobles", self.olympiad.nobles.len());
        true
    }

    /// Java `loadNoblesRank()`. `classified` is `GET_ALL_CLASSIFIED_NOBLESS`: the char ids of
    /// the last period's copy with enough matches, best first.
    fn load_nobles_rank(&mut self, classified: &[i32]) {
        self.olympiad.nobles_rank.clear();
        let size = classified.len() as f64;
        // `Math.round`.
        let round = |v: f64| (v + 0.5).floor() as i64;
        let mut rank1 = round(size * 0.01);
        let mut rank2 = round(size * 0.10);
        let mut rank3 = round(size * 0.25);
        let mut rank4 = round(size * 0.50);
        if rank1 == 0 {
            rank1 = 1;
            rank2 += 1;
            rank3 += 1;
            rank4 += 1;
        }
        for (index, char_id) in classified.iter().enumerate() {
            let place = i64::try_from(index).unwrap_or(i64::MAX) + 1;
            let rank = if place <= rank1 {
                1
            } else if place <= rank2 {
                2
            } else if place <= rank3 {
                3
            } else if place <= rank4 {
                4
            } else {
                5
            };
            self.olympiad.nobles_rank.insert(*char_id, rank);
        }

        // "Store remaining hero reward points to player variables."
        let ids: Vec<i32> = self.olympiad.nobles.keys().copied().collect();
        for char_id in ids {
            let points = self.noblesse_passes(char_id);
            if points <= 0 {
                continue;
            }
            let player = ObjectId(char_id);
            if self.players.contains_key(&player) {
                self.set_player_variable(player, UNCLAIMED_OLYMPIAD_PASSES_VAR, points);
            } else if self.olympiad_writes_cycles() {
                self.olympiad_write(OlympiadWrite::UnclaimedPoints { char_id, points });
            }
        }
    }

    /// Java `getNoblessePasses(objectId)`: a one time calculation (the points go to 0).
    fn noblesse_passes(&mut self, char_id: i32) -> i32 {
        if self.olympiad.data.period != 1 || self.olympiad.nobles_rank.is_empty() {
            return 0;
        }
        let Some(&rank) = self.olympiad.nobles_rank.get(&char_id) else {
            return 0;
        };
        let Some(noble) = self.olympiad.nobles.get(&char_id) else {
            return 0;
        };
        if noble.points == 0 {
            return 0;
        }
        let settings = &self.olympiad.settings;
        // Hero point bonus.
        let mut points = if self.olympiad_is_hero_or_unclaimed(char_id) {
            settings.hero_points
        } else {
            0
        };
        // Rank point bonus.
        points += match rank {
            1 => settings.rank_points[0],
            2 => settings.rank_points[1],
            3 => settings.rank_points[2],
            4 => settings.rank_points[3],
            _ => settings.rank_points[4],
        };
        // Win/no win matches point bonus.
        points += if self.competition_won(char_id) > 0 { 10 } else { 5 };
        // This is a one time calculation.
        if let Some(noble) = self.olympiad.nobles.get_mut(&char_id) {
            noble.points = 0;
        }
        points
    }

    /// `Hero.getInstance().isHero(objectId) || isUnclaimedHero(objectId)`.
    pub(crate) fn olympiad_is_hero_or_unclaimed(&self, char_id: i32) -> bool {
        self.is_hero_char(char_id) || self.is_unclaimed_hero(char_id)
    }

    /// `Hero.getInstance().resetData()` and `computeNewHeroes(herosToBe)` (`hero.rs`).
    pub fn olympiad_compute_new_heroes(&mut self, heroes: &[HeroToBe]) {
        self.hero_reset_data();
        self.hero_compute_new_heroes(heroes);
    }

    // ------------------------------------------------------------------------------- init

    /// Java `init()`: the next competition start and end, the period end task, the competition
    /// start task.
    pub(crate) fn olympiad_init(&mut self) {
        if self.olympiad.data.period == 1 {
            return;
        }
        let mut comp_start = self.olympiad_cal_now();
        if self.olympiad.settings.use_custom_period_settings {
            let counter =
                competition_day_counter(comp_start.day_of_week(), &self.olympiad.settings.competition_days);
            if counter > 0 {
                comp_start.add_days(counter);
            }
        }
        comp_start.set_hour_minute(self.olympiad.settings.start_time, self.olympiad.settings.min);
        self.olympiad.comp_start = self.olympiad_cal_millis(comp_start);
        self.olympiad.comp_end = self.olympiad.comp_start + self.olympiad.settings.comp_period_ms;

        let old = self.olympiad.olympiad_end_task.take();
        self.cancel_olympiad_timer(old);
        let delay = self.millis_to_olympiad_end();
        self.olympiad.olympiad_end_task =
            Some(self.schedule_olympiad_timer(OlympiadTimer::OlympiadEnd, delay));

        self.update_comp_status();
    }

    /// Java `updateCompStatus()`.
    fn update_comp_status(&mut self) {
        let milli_to_start = self.millis_to_comp_begin();
        let (days, hours, mins) = countdown(milli_to_start);
        info!("Olympiad System: Competition Period Starts in {days} days, {hours} hours and {mins} mins.");
        info!(
            "Olympiad System: Event starts/started: {}",
            self.olympiad.comp_start
        );
        // NOTE(java-quirk): the task of an earlier `init()` is not cancelled; it checks
        // `isOlympiadEnd()` when it runs.
        self.schedule_olympiad_timer(OlympiadTimer::CompStart, milli_to_start);
    }

    /// The competition start task.
    fn on_comp_start(&mut self) {
        if self.is_olympiad_end() {
            return;
        }
        self.olympiad.in_comp_period = true;
        self.broadcast_to_all_online_players(&SystemMessage::new(
            SystemMessageId::SharpenYourSwordsTightenTheStitchingInYourArmorAndMakeHasteToAGrandOlympiadManagerBattlesInTheGrandOlympiadGamesAreNowTakingPlace,
        ));
        info!("Olympiad System: Olympiad Games have started.");
        self.olympiad.game_manager_task =
            Some(self.schedule_olympiad_timer(OlympiadTimer::GameManager, GAME_MANAGER_MS));
        if self.olympiad.settings.announce_games {
            self.olympiad.announcer_task =
                Some(self.schedule_olympiad_timer(OlympiadTimer::Announcer, GAME_MANAGER_MS));
        }
        let reg_end = self.millis_to_comp_end() - REGISTRATION_END_MS;
        if reg_end > 0 {
            self.schedule_olympiad_timer(OlympiadTimer::RegistrationEnd, reg_end);
        }
        let end = self.millis_to_comp_end();
        self.schedule_olympiad_timer(OlympiadTimer::CompEnd, end);
    }

    /// The competition end task.
    fn on_comp_end(&mut self) {
        if self.is_olympiad_end() {
            return;
        }
        self.olympiad.in_comp_period = false;
        self.broadcast_to_all_online_players(&SystemMessage::new(
            SystemMessageId::MuchCarnageHasBeenLeftForTheCleanupCrewOfTheOlympiadStadiumBattlesInTheGrandOlympiadGamesAreNowOver,
        ));
        info!("Olympiad System: Olympiad games have ended.");
        self.on_comp_end_wait();
    }

    /// `while (OlympiadGameManager.getInstance().isBattleStarted()) Thread.sleep(60000)`, then the
    /// rest of the competition end.
    fn on_comp_end_wait(&mut self) {
        if self.olympiad.battle_started {
            self.schedule_olympiad_timer(OlympiadTimer::CompEndWait, COMP_END_WAIT_MS);
            return;
        }
        let manager = self.olympiad.game_manager_task.take();
        self.cancel_olympiad_timer(manager);
        let announcer = self.olympiad.announcer_task.take();
        self.cancel_olympiad_timer(announcer);
        self.save_olympiad_status();
        self.olympiad_init();
    }

    /// Java `getMillisToOlympiadEnd()`.
    pub fn olympiad_millis_to_olympiad_end(&self) -> i64 {
        self.millis_to_olympiad_end()
    }

    fn millis_to_olympiad_end(&self) -> i64 {
        self.olympiad.data.olympiad_end - self.olympiad_now_ms()
    }

    /// Java `manualSelectHeroes()` (`//endolympiad`).
    pub fn olympiad_manual_select_heroes(&mut self) {
        let old = self.olympiad.olympiad_end_task.take();
        self.cancel_olympiad_timer(old);
        self.olympiad.olympiad_end_task = Some(self.schedule_olympiad_timer(OlympiadTimer::OlympiadEnd, 0));
    }

    /// Java `getMillisToValidationEnd()`.
    fn millis_to_validation_end(&self) -> i64 {
        let now = self.olympiad_now_ms();
        if self.olympiad.data.validation_end > now {
            return self.olympiad.data.validation_end - now;
        }
        10
    }

    /// Java `isOlympiadEnd()`.
    pub fn is_olympiad_end(&self) -> bool {
        self.olympiad.data.period != 0
    }

    /// Java `setNewOlympiadEnd()`.
    fn set_new_olympiad_end(&mut self) {
        let message = SystemMessage::new(SystemMessageId::RoundS1OfTheGrandOlympiadGamesHasStarted)
            .with(SystemMessageParam::IntNumber(self.olympiad.data.current_cycle));
        self.broadcast_to_all_online_players(&message);

        let now = self.olympiad_now_ms();
        let weekly = self.olympiad.settings.weekly_period_ms;
        if !self.olympiad.settings.use_custom_period_settings {
            let mut end = self.olympiad_cal_now();
            end.add_months(1);
            end.set_first_of_month();
            // `set(AM_PM, AM)` then `set(HOUR_OF_DAY, 12)`: the hour of day is set last and wins.
            end.set_hour_zero(12);
            self.olympiad.data.olympiad_end = self.olympiad_cal_millis(end);
            self.olympiad.data.next_weekly_change = now + weekly;
        } else {
            let mut end = self.olympiad_cal_now();
            end.set_hour_zero(12);
            let multiplier = self.olympiad.settings.period_multiplier;
            match self.olympiad.settings.period.as_str() {
                "DAY" => {
                    end.add_days(i64::from(multiplier));
                    // "last day is for validation"
                    end.add_days(-1);
                    if multiplier >= 14 {
                        self.olympiad.data.next_weekly_change = now + weekly;
                    } else if multiplier >= 7 {
                        self.olympiad.data.next_weekly_change = now + weekly / 2;
                    } else {
                        warn!("Invalid config value for Config.OLYMPIAD_PERIOD_MULTIPLIER, must be >= 7");
                    }
                }
                "WEEK" => {
                    end.add_days(7 * i64::from(multiplier));
                    end.add_days(-1);
                    if multiplier > 1 {
                        self.olympiad.data.next_weekly_change = now + weekly;
                    } else {
                        self.olympiad.data.next_weekly_change = now + weekly / 2;
                    }
                }
                "MONTH" => {
                    end.add_months(i64::from(multiplier));
                    end.add_days(-1);
                    self.olympiad.data.next_weekly_change = now + weekly;
                }
                _ => {}
            }
            self.olympiad.data.olympiad_end = self.olympiad_cal_millis(end);
        }
        self.schedule_weekly_change();
    }

    /// Java `getMillisToCompBegin()`.
    fn millis_to_comp_begin(&mut self) -> i64 {
        let now = self.olympiad_now_ms();
        if self.olympiad.comp_start < now && self.olympiad.comp_end > now {
            return 10;
        }
        if self.olympiad.comp_start > now {
            return self.olympiad.comp_start - now;
        }
        self.set_new_comp_begin()
    }

    /// Java `setNewCompBegin()`.
    fn set_new_comp_begin(&mut self) -> i64 {
        let mut comp_start = self.olympiad_cal_now();
        let mut current_day = comp_start.day_of_week();
        comp_start.set_hour_minute(self.olympiad.settings.start_time, self.olympiad.settings.min);
        // "Today's competitions ended, start checking from next day."
        if current_day == comp_start.day_of_week() {
            current_day = if current_day == 7 { 1 } else { current_day + 1 };
        }
        if self.olympiad.settings.use_custom_period_settings {
            let counter = competition_day_counter(current_day, &self.olympiad.settings.competition_days);
            if counter > 0 {
                comp_start.add_days(counter);
            }
        }
        // `add(HOUR_OF_DAY, 24)` adds the time, not the wall clock.
        self.olympiad.comp_start = self.olympiad_cal_millis(comp_start) + DAY_MS;
        self.olympiad.comp_end = self.olympiad.comp_start + self.olympiad.settings.comp_period_ms;
        info!("Olympiad System: New Schedule @ {}", self.olympiad.comp_start);
        self.olympiad.comp_start - self.olympiad_now_ms()
    }

    /// Java `getMillisToCompEnd()`.
    pub(crate) fn millis_to_comp_end(&self) -> i64 {
        self.olympiad.comp_end - self.olympiad_now_ms()
    }

    /// Java `getMillisToWeekChange()`.
    pub fn olympiad_millis_to_week_change(&self) -> i64 {
        let now = self.olympiad_now_ms();
        if self.olympiad.data.next_weekly_change > now {
            return self.olympiad.data.next_weekly_change - now;
        }
        10
    }

    /// Java `scheduleWeeklyChange()`: a fixed rate task.
    fn schedule_weekly_change(&mut self) {
        let old = self.olympiad.weekly_task.take();
        self.cancel_olympiad_timer(old);
        let delay = self.olympiad_millis_to_week_change();
        self.olympiad.weekly_task = Some(self.schedule_olympiad_timer(OlympiadTimer::Weekly, delay));
    }

    /// The weekly task.
    fn on_weekly_change(&mut self) {
        // The next run of the fixed rate task.
        let period = self.olympiad.settings.weekly_period_ms;
        self.olympiad.weekly_task = Some(self.schedule_olympiad_timer(OlympiadTimer::Weekly, period));
        self.add_weekly_points();
        info!("Olympiad System: Added weekly points to nobles");
        self.reset_weekly_matches();
        info!("Olympiad System: Reset weekly matches to nobles");
        self.olympiad.data.next_weekly_change = self.olympiad_now_ms() + period;
    }

    /// Java `addWeeklyPoints()`.
    fn add_weekly_points(&mut self) {
        if self.olympiad.data.period == 1 {
            return;
        }
        let weekly = self.olympiad.settings.weekly_points;
        let max_points = self.olympiad.settings.max_points;
        for noble in self.olympiad.nobles.values_mut() {
            let cap = noble
                .comp_done
                .wrapping_mul(max_points)
                .wrapping_add(weekly.wrapping_mul(4));
            noble.points = noble.points.wrapping_add(weekly).clamp(0, cap.max(0));
        }
    }

    /// Java `resetWeeklyMatches()`.
    fn reset_weekly_matches(&mut self) {
        if self.olympiad.data.period == 1 {
            return;
        }
        for noble in self.olympiad.nobles.values_mut() {
            noble.comp_done_week = 0;
            noble.comp_done_week_classed = 0;
            noble.comp_done_week_non_classed = 0;
            noble.comp_done_week_team = 0;
        }
    }

    // ------------------------------------------------------------------------------- period end

    /// `OlympiadEndTask.run()`.
    fn on_olympiad_end(&mut self) {
        self.olympiad.olympiad_end_task = None;
        let message = SystemMessage::new(SystemMessageId::RoundS1OfTheGrandOlympiadGamesHasNowEnded)
            .with(SystemMessageParam::IntNumber(self.olympiad.data.current_cycle));
        self.broadcast_to_all_online_players(&message);

        let weekly = self.olympiad.weekly_task.take();
        self.cancel_olympiad_timer(weekly);
        self.save_noble_data();

        self.olympiad.data.period = 1;
        self.sort_heroes_to_be();
        let heroes = self.olympiad.heroes_to_be.clone();
        self.olympiad_compute_new_heroes(&heroes);

        self.save_olympiad_status();
        self.update_monthly_data();

        self.olympiad.data.validation_end =
            self.olympiad_now_ms() + self.olympiad.settings.validation_period_ms;

        // `loadNoblesRank()` reads the copy `updateMonthlyData` just made of the nobles.
        // D-53: with the switch off the Java game server hands the passes out (it writes the
        // `character_variables` rows for every noble), so this server does not.
        if self.olympiad_writes_cycles() {
            let classified = self.classified_nobles();
            self.load_nobles_rank(&classified);
        }
        let delay = self.millis_to_validation_end();
        self.olympiad.validation_task =
            Some(self.schedule_olympiad_timer(OlympiadTimer::ValidationEnd, delay));
    }

    /// `GET_ALL_CLASSIFIED_NOBLESS` on the copy of the nobles: enough matches, by points, matches
    /// and wins.
    // NOTE: the database orders ties as it likes; here a tie goes by char id.
    fn classified_nobles(&self) -> Vec<i32> {
        let min = self.olympiad.settings.min_matches;
        let mut list: Vec<(i32, &NobleStats)> = self
            .olympiad
            .nobles
            .iter()
            .filter(|(_, n)| n.comp_done >= min)
            .map(|(id, n)| (*id, n))
            .collect();
        list.sort_by(|(a_id, a), (b_id, b)| {
            b.points
                .cmp(&a.points)
                .then(b.comp_done.cmp(&a.comp_done))
                .then(b.comp_won.cmp(&a.comp_won))
                .then(a_id.cmp(b_id))
        });
        list.into_iter().map(|(id, _)| id).collect()
    }

    /// `ValidationEndTask.run()`.
    fn on_validation_end(&mut self) {
        self.olympiad.validation_task = None;
        self.broadcast_announcement("Olympiad Validation Period has ended", false);
        self.olympiad.data.period = 0;
        self.olympiad.data.current_cycle += 1;
        self.delete_nobles();
        self.set_new_olympiad_end();
        self.olympiad_init();
    }

    /// Java `saveNobleData()`: every noble, `INSERT` for the new ones. Only with
    /// `CycleDatabaseWrites` (D-53).
    fn save_noble_data(&mut self) {
        if self.olympiad.nobles.is_empty() || !self.olympiad_writes_cycles() {
            return;
        }
        let rows: Vec<(i32, NobleStats)> = self
            .olympiad
            .nobles
            .iter()
            .map(|(id, n)| (*id, n.clone()))
            .collect();
        for noble in self.olympiad.nobles.values_mut() {
            noble.to_save = false;
        }
        self.olympiad_write(OlympiadWrite::SaveNobles(rows));
    }

    /// Java `saveOlympiadStatus()` (the competition end, the shutdown, `//saveolymp`). Only with
    /// `CycleDatabaseWrites` (D-53).
    pub fn save_olympiad_status(&mut self) {
        if !self.olympiad.loaded {
            return;
        }
        self.save_noble_data();
        if self.olympiad_writes_cycles() {
            let data = self.olympiad.data;
            self.olympiad_write(OlympiadWrite::SaveData(data));
        }
    }

    /// Java `updateMonthlyData()`. Only with `CycleDatabaseWrites` (D-53).
    fn update_monthly_data(&mut self) {
        if self.olympiad_writes_cycles() {
            self.olympiad_write(OlympiadWrite::UpdateMonthlyData);
        }
    }

    /// Java `deleteNobles()`: the table (only with `CycleDatabaseWrites`, D-53) and the memory.
    fn delete_nobles(&mut self) {
        if self.olympiad_writes_cycles() {
            self.olympiad_write(OlympiadWrite::DeleteNobles);
        }
        self.olympiad.nobles.clear();
    }

    /// Java `sortHerosToBe()`: for each hero class the best noble with enough matches and a win
    /// (Java's `OLYMPIAD_GET_HEROS` reads the table `saveNobleData` just wrote, which is the
    /// memory; the soul hounds share one hero).
    // NOTE(java-quirk): `HEROS_TO_BE` is never cleared, so the heroes of earlier periods stay in
    // the list.
    // NOTE: the database orders ties as it likes; here a tie goes by char id.
    fn sort_heroes_to_be(&mut self) {
        if self.olympiad.data.period != 1 {
            return;
        }
        let min = self.olympiad.settings.min_matches;
        let mut soul_hounds: Vec<(i32, NobleStats)> = Vec::new();
        for class_id in HERO_IDS {
            let best = self
                .olympiad
                .nobles
                .iter()
                .filter(|(_, n)| n.class_id == class_id && n.comp_done >= min && n.comp_won > 0)
                .min_by(|(a_id, a), (b_id, b)| {
                    b.points
                        .cmp(&a.points)
                        .then(b.comp_done.cmp(&a.comp_done))
                        .then(b.comp_won.cmp(&a.comp_won))
                        .then(a_id.cmp(b_id))
                })
                .map(|(id, n)| (*id, n.clone()));
            let Some((char_id, noble)) = best else {
                continue;
            };
            info!("Hero {},{char_id},{class_id}", noble.char_name);
            // "Male & Female Soulhounds rank as one hero class"
            if class_id == 132 || class_id == 133 {
                soul_hounds.push((char_id, noble));
            } else {
                self.olympiad.heroes_to_be.push(HeroToBe {
                    class_id,
                    char_id,
                    char_name: noble.char_name,
                });
            }
        }
        let winner = match soul_hounds.as_slice() {
            [] => None,
            [one] => Some(one.clone()),
            [hero1, hero2, ..] => {
                let (a, b) = (&hero1.1, &hero2.1);
                Some(if a.points > b.points {
                    hero1.clone()
                } else if b.points > a.points {
                    hero2.clone()
                } else if a.comp_done > b.comp_done {
                    hero1.clone()
                } else if b.comp_done > a.comp_done {
                    hero2.clone()
                } else if a.comp_won > b.comp_won {
                    hero1.clone()
                } else {
                    hero2.clone()
                })
            }
        };
        if let Some((char_id, noble)) = winner {
            self.olympiad.heroes_to_be.push(HeroToBe {
                class_id: noble.class_id,
                char_id,
                char_name: noble.char_name,
            });
        }
    }

    // ------------------------------------------------------------------------------- timers

    /// An Olympiad timer fired.
    pub(crate) fn on_olympiad_timer(&mut self, timer: OlympiadTimer) {
        match timer {
            OlympiadTimer::CompStart => self.on_comp_start(),
            OlympiadTimer::CompEnd => self.on_comp_end(),
            OlympiadTimer::CompEndWait => self.on_comp_end_wait(),
            OlympiadTimer::RegistrationEnd => self.broadcast_to_all_online_players(&SystemMessage::new(
                SystemMessageId::TheGrandOlympiadRegistrationPeriodHasEnded,
            )),
            OlympiadTimer::OlympiadEnd => self.on_olympiad_end(),
            OlympiadTimer::Weekly => self.on_weekly_change(),
            OlympiadTimer::ValidationEnd => self.on_validation_end(),
            OlympiadTimer::GameManager => {
                self.olympiad.game_manager_task =
                    Some(self.schedule_olympiad_timer(OlympiadTimer::GameManager, GAME_MANAGER_MS));
                self.olympiad_game_manager_run();
            }
            OlympiadTimer::Announcer => {
                self.olympiad.announcer_task =
                    Some(self.schedule_olympiad_timer(OlympiadTimer::Announcer, ANNOUNCER_MS));
                self.olympiad_announcer_run();
            }
            OlympiadTimer::Task(index) => self.olympiad_task_run(index),
            OlympiadTimer::KickPlayer(player) => self.stadium_kick_player(player),
        }
    }

    /// Java `Shutdown`: "Olympiad System: Data saved".
    pub(crate) fn olympiad_shutdown(&mut self) {
        self.save_olympiad_status();
        // `Hero.getInstance().shutdown()`.
        self.hero_shutdown();
    }

    // ------------------------------------------------------------------------------- the nobles

    /// Java `getNoblePoints(objId)`.
    pub fn noble_points(&self, char_id: i32) -> i32 {
        self.olympiad.nobles.get(&char_id).map_or(0, |n| n.points)
    }

    /// Java `getCompetitionDone(objId)`.
    pub fn competition_done(&self, char_id: i32) -> i32 {
        self.olympiad.nobles.get(&char_id).map_or(0, |n| n.comp_done)
    }

    /// Java `getCompetitionWon(objId)`.
    pub fn competition_won(&self, char_id: i32) -> i32 {
        self.olympiad.nobles.get(&char_id).map_or(0, |n| n.comp_won)
    }

    /// Java `getCompetitionLost(objId)`.
    pub fn competition_lost(&self, char_id: i32) -> i32 {
        self.olympiad.nobles.get(&char_id).map_or(0, |n| n.comp_lost)
    }

    /// Java `getRemainingWeeklyMatches(objId)`.
    pub fn remaining_weekly_matches(&self, char_id: i32) -> i32 {
        let done = self.olympiad.nobles.get(&char_id).map_or(0, |n| n.comp_done_week);
        (self.olympiad.settings.max_weekly_matches - done).max(0)
    }

    /// Java `getRemainingWeeklyMatchesClassed(objId)`.
    pub fn remaining_weekly_matches_classed(&self, char_id: i32) -> i32 {
        let done = self
            .olympiad
            .nobles
            .get(&char_id)
            .map_or(0, |n| n.comp_done_week_classed);
        (self.olympiad.settings.max_weekly_matches_classed - done).max(0)
    }

    /// Java `getRemainingWeeklyMatchesNonClassed(objId)`.
    pub fn remaining_weekly_matches_non_classed(&self, char_id: i32) -> i32 {
        let done = self
            .olympiad
            .nobles
            .get(&char_id)
            .map_or(0, |n| n.comp_done_week_non_classed);
        (self.olympiad.settings.max_weekly_matches_non_classed - done).max(0)
    }

    /// Java `getRemainingWeeklyMatchesTeam(objId)`.
    pub fn remaining_weekly_matches_team(&self, char_id: i32) -> i32 {
        let done = self
            .olympiad
            .nobles
            .get(&char_id)
            .map_or(0, |n| n.comp_done_week_team);
        (self.olympiad.settings.max_weekly_matches_team - done).max(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utc(_: i64) -> i64 {
        0
    }

    #[test]
    fn the_calendar_knows_the_weekday_and_the_month_end() {
        // 2026-10-27 is a Tuesday (DAY_OF_WEEK 3).
        let day = days_from_civil(2026, 10, 27);
        let cal = Cal { day, time_of_day: 0 };
        assert_eq!(cal.day_of_week(), 3);
        let mut jan = Cal {
            day: days_from_civil(2027, 1, 31),
            time_of_day: 5 * HOUR_MS + 123,
        };
        jan.add_months(1);
        assert_eq!(civil_from_days(jan.day), (2027, 2, 28));
        jan.set_first_of_month();
        jan.set_hour_zero(12);
        assert_eq!(
            jan.millis(&utc),
            days_from_civil(2027, 2, 1) * DAY_MS + 12 * HOUR_MS + 123
        );
    }

    #[test]
    fn the_day_counter_finds_the_next_competition_day() {
        assert_eq!(competition_day_counter(3, &[1, 2, 3, 4, 5, 6, 7]), 0);
        assert_eq!(competition_day_counter(3, &[5]), 2);
        // From Friday to Monday: the rest of the week, then the start of the next one.
        assert_eq!(competition_day_counter(6, &[2]), 2 + 1);
        assert_eq!(competition_day_counter(3, &[]), 5 + 7);
    }

    #[test]
    fn the_countdown_splits_like_java() {
        assert_eq!(countdown(((2 * 24 + 3) * 60 + 4) * 60_000 + 5_000), (2, 3, 4));
    }
}
