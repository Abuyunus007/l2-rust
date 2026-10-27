//! The cycle: the restore at start, the period change on the timer wheel, the new seal owners,
//! the saves, `isDateInSealValidPeriod`, the parts of `EnterWorld` / `onPlayerEnter`, and the D-50
//! re-reads (see the module comment of `sevensigns`).
// Java: mechanics/sevensigns/SevenSigns.java (constructor, isNextPeriodChangeInPast,
//       setCalendarForNextPeriodChange, isDateInSealValidPeriod, restoreSevenSignsData,
//       saveSevenSignsData, saveSevenSignsStatus, resetPlayerData, initializeSeals, resetSeals,
//       calcNewSealOwners, teleLosingCabalFromDungeons, SevenSignsPeriodChange, giveCPMult,
//       removeCPMult, sendCurrentPeriodMsg)
// Java: managers/CastleManager.java (validateTaxes), managers/DailyResetManager.java (onSave)
// Java: network/clientpackets/EnterWorld.java (the Seal of Strife block, sendCurrentPeriodMsg)
// Java: entity/actor/Player.java (onPlayerEnter: the dungeon checks; store: saveSevenSignsData)
// Java: network/serverpackets/SSQInfo.java

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SsqInfo, SystemMessage};
use l2_util::ObjectId;
use tracing::{debug, info, warn};

use super::{
    CABAL_DAWN, CABAL_DUSK, CABAL_NULL, PERIOD_COMP_RECRUITING, PERIOD_COMP_RESULTS, PERIOD_COMPETITION,
    PERIOD_MAJOR_LENGTH, PERIOD_MINOR_LENGTH, PERIOD_SEAL_VALIDATION, PERIOD_START_HOUR, PERIOD_START_MINS,
    SEAL_AVARICE, SEAL_GNOSIS, SEAL_NULL, SEAL_STRIFE, SevenSignsLoad, SevenSignsStatus, SevenSignsThen,
    SevenSignsTimer, SevenSignsWrite, SignsPlayer, THE_VANQUISHED_OF_WAR, THE_VICTOR_OF_WAR, cabal_name,
    cabal_short_name, java_round_f32, period_name, seal_name,
};
use crate::outbox::PersistCommand;
use crate::residence::{CastleId, ResidenceWrite};
use crate::teleport::TeleportWhereType;
use crate::timer::{TimerKind, TimerWheel};
use crate::world::{World, unix_ms_now};

const SECOND_MS: i64 = 1000;
const MINUTE_MS: i64 = 60 * SECOND_MS;
const HOUR_MS: i64 = 60 * MINUTE_MS;
const DAY_MS: i64 = 24 * HOUR_MS;
const WEEK_MS: i64 = 7 * DAY_MS;

/// `DailyResetManager`'s global save: every 30 minutes.
const GLOBAL_SAVE_MS: u64 = 1_800_000;

/// Java `calcContributionScore(blue, green, red)`.
pub fn calc_contribution_score(blue: i64, green: i64, red: i64) -> i64 {
    blue.wrapping_mul(super::BLUE_CONTRIB_POINTS)
        .wrapping_add(green.wrapping_mul(super::GREEN_CONTRIB_POINTS))
        .wrapping_add(red.wrapping_mul(super::RED_CONTRIB_POINTS))
}

/// Java `calcAncientAdenaReward(blue, green, red)`.
pub fn calc_ancient_adena_reward(blue: i64, green: i64, red: i64) -> i64 {
    blue.wrapping_mul(super::SEAL_STONE_BLUE_VALUE)
        .wrapping_add(green.wrapping_mul(super::SEAL_STONE_GREEN_VALUE))
        .wrapping_add(red.wrapping_mul(super::SEAL_STONE_RED_VALUE))
}

/// `new SSQInfo()`: the sky of the winner during the seal validation period (2 Dawn, 1 Dusk,
/// else 0).
pub fn ssq_info_state(status: &SevenSignsStatus) -> i32 {
    if status.active_period == PERIOD_SEAL_VALIDATION {
        match status.cabal_highest_score() {
            CABAL_DAWN => return 2,
            CABAL_DUSK => return 1,
            _ => {}
        }
    }
    0
}

/// The wall clock time `local` (milliseconds of the local zone) as UTC milliseconds.
fn local_to_utc(local: i64, offset_at: &impl Fn(i64) -> i64) -> i64 {
    let guess = local - offset_at(local);
    local - offset_at(guess)
}

/// Java `Calendar.DAY_OF_WEEK` of a local day number (1 Sunday ... 7 Saturday; 1970-01-01 was a
/// Thursday).
fn day_of_week(local_day: i64) -> i64 {
    (local_day + 4).rem_euclid(7) + 1
}

/// Java `setCalendarForNextPeriodChange()` on a calendar at `from_ms` (UTC milliseconds): the next
/// change for `period`. The major periods end on Monday at 18:00 local time, the minor ones 15
/// minutes after `from_ms`.
// NOTE(java-quirk): only the hour and the minute are set, so the seconds and the milliseconds of
// the calendar stay (the change is at 18:00:ss of the second the calendar was made).
pub fn next_period_change_ms(period: i32, from_ms: i64, offset_at: impl Fn(i64) -> i64) -> i64 {
    match period {
        PERIOD_SEAL_VALIDATION | PERIOD_COMPETITION => {
            let local = from_ms + offset_at(from_ms);
            let day = local.div_euclid(DAY_MS);
            let ms_of_day = local.rem_euclid(DAY_MS);
            let (hour, minute) = (ms_of_day / HOUR_MS, (ms_of_day % HOUR_MS) / MINUTE_MS);
            // `getDaysToPeriodChange()` (Calendar.MONDAY = 2).
            let num_days = day_of_week(day) - 2;
            let mut days = if num_days < 0 { -num_days } else { 7 - num_days };
            if days == 7
                && (hour < PERIOD_START_HOUR || (hour == PERIOD_START_HOUR && minute < PERIOD_START_MINS))
            {
                days = 0;
            }
            let rest = ms_of_day % MINUTE_MS;
            let target =
                (day + days) * DAY_MS + PERIOD_START_HOUR * HOUR_MS + PERIOD_START_MINS * MINUTE_MS + rest;
            local_to_utc(target, &offset_at)
        }
        PERIOD_COMP_RECRUITING | PERIOD_COMP_RESULTS => from_ms.wrapping_add(PERIOD_MINOR_LENGTH),
        // An unknown period: Java leaves the calendar alone.
        _ => from_ms,
    }
}

/// Java `isNextPeriodChangeInPast()`: the server was off when the period should have changed.
// NOTE(java-quirk): `set(DAY_OF_WEEK, MONDAY)` depends on the locale's first day of the week, but
// together with the "one week back when in the future" step it is the last Monday 18:00 before
// now in every locale. `SECOND` is set to 0, the milliseconds of now stay.
pub fn is_next_period_change_in_past(
    period: i32,
    last_save: i64,
    now_ms: i64,
    offset_at: impl Fn(i64) -> i64,
) -> bool {
    let last_period_change = match period {
        PERIOD_SEAL_VALIDATION | PERIOD_COMPETITION => {
            let local = now_ms + offset_at(now_ms);
            let day = local.div_euclid(DAY_MS);
            let millis = local.rem_euclid(SECOND_MS);
            let since_monday = (day_of_week(day) - 2).rem_euclid(7);
            let target = (day - since_monday) * DAY_MS
                + PERIOD_START_HOUR * HOUR_MS
                + PERIOD_START_MINS * MINUTE_MS
                + millis;
            let mut change = local_to_utc(target, &offset_at);
            // "if we hit next week, just turn back 1 week"
            if now_ms < change {
                change = local_to_utc(target - WEEK_MS, &offset_at);
            }
            change
        }
        // "because of the short duration of this period, just check it from last save"
        PERIOD_COMP_RECRUITING | PERIOD_COMP_RESULTS => last_save.wrapping_add(PERIOD_MINOR_LENGTH),
        _ => now_ms,
    };
    // "because of previous "date" column usage, check only if it already contains usable data"
    last_save > 7 && last_save < last_period_change
}

/// Java `isDateInSealValidPeriod(date)` with `milli_to_change` = `getMilliToPeriodChange()` and
/// `till_date` = `date - now`.
pub(crate) fn date_in_seal_valid_period(period: i32, milli_to_change: i64, till_date: i64) -> bool {
    const CYCLE: i64 = 2 * PERIOD_MAJOR_LENGTH + 2 * PERIOD_MINOR_LENGTH;
    let mut till = till_date;
    while CYCLE < till {
        till -= CYCLE;
    }
    while till < 0 {
        till += CYCLE;
    }
    let (next_quest_start, next_valid_start) = match period {
        PERIOD_COMP_RECRUITING => {
            let valid = milli_to_change + PERIOD_MAJOR_LENGTH;
            (valid + PERIOD_MAJOR_LENGTH + PERIOD_MINOR_LENGTH, valid)
        }
        PERIOD_COMPETITION => (
            milli_to_change + PERIOD_MAJOR_LENGTH + PERIOD_MINOR_LENGTH,
            milli_to_change,
        ),
        PERIOD_COMP_RESULTS => {
            let quest = milli_to_change + PERIOD_MAJOR_LENGTH;
            (quest, quest + PERIOD_MAJOR_LENGTH + PERIOD_MINOR_LENGTH)
        }
        PERIOD_SEAL_VALIDATION => (
            milli_to_change,
            milli_to_change + PERIOD_MAJOR_LENGTH + PERIOD_MINOR_LENGTH,
        ),
        _ => (0, 0),
    };
    !((next_quest_start < till && till < next_valid_start)
        || (next_valid_start < next_quest_start && (till < next_valid_start || next_quest_start < till)))
}

fn status_of_load(status: Option<SevenSignsStatus>) -> SevenSignsStatus {
    // NOTE(java-quirk): without the `id = 0` row Java leaves the seal maps empty and throws a
    // NullPointerException at the first `getSealOwner`; here the seals start unowned.
    status.unwrap_or_default()
}

impl World {
    // ------------------------------------------------------------------ start

    /// Java `SevenSigns`' constructor: the rows, the period's logs, the period change on the wheel
    /// (at once when it was due while the server was off) and the global save. Call once at
    /// startup, after the castles are loaded.
    pub fn load_seven_signs(&mut self, load: SevenSignsLoad) {
        self.load_seven_signs_at(load, unix_ms_now(), l2_util::time::local_offset_millis);
    }

    /// [`Self::load_seven_signs`] for a given clock (`offset_at` gives the zone's offset from UTC
    /// at a UTC millisecond, D-48).
    pub fn load_seven_signs_at(
        &mut self,
        load: SevenSignsLoad,
        now_ms: i64,
        offset_at: impl Fn(i64) -> i64 + Copy,
    ) {
        self.ss_restore(load);
        info!(
            "SevenSigns: Currently in the {} period!",
            period_name(self.ss_current_period()).unwrap_or("null")
        );
        self.ss_initialize_seals();
        let highest = self.ss_cabal_highest_score();
        if self.ss_is_seal_validation_period() {
            if highest == CABAL_NULL {
                info!("SevenSigns: The competition ended with a tie last week.");
            } else {
                info!(
                    "SevenSigns: The {} were victorious last week.",
                    cabal_name(highest)
                );
            }
        } else if highest == CABAL_NULL {
            info!(
                "SevenSigns: The competition, if the current trend continues, will end in a tie this week."
            );
        } else {
            info!(
                "SevenSigns: The {} are in the lead this week.",
                cabal_name(highest)
            );
        }

        // `_nextPeriodChange = Calendar.getInstance()`.
        self.seven_signs.next_period_change = now_ms;
        let status = &self.seven_signs.status;
        let mut milli_to_change = 0;
        if is_next_period_change_in_past(status.active_period, status.last_save, now_ms, offset_at) {
            info!(
                "SevenSigns: Next period change was in the past (server was offline), changing periods now!"
            );
        } else {
            self.ss_set_calendar_for_next_period_change(offset_at);
            milli_to_change = self.seven_signs.next_period_change - now_ms;
        }
        // Schedule a time for the next period change.
        self.ss_schedule_period_change(milli_to_change);
        info!("SevenSigns: Next period begins in {} ms", milli_to_change.max(0));
        // DailyResetManager: the global save every 30 minutes.
        self.ss_schedule_global_save();
    }

    /// Java `restoreSevenSignsData()` (the rows the game server read).
    fn ss_restore(&mut self, load: SevenSignsLoad) {
        self.seven_signs.loaded = true;
        self.seven_signs.players.clear();
        for row in load.players {
            self.seven_signs.players.insert(row.char_id, row);
        }
        self.seven_signs.status = status_of_load(load.status);
    }

    /// `setCalendarForNextPeriodChange()` on `_nextPeriodChange`.
    fn ss_set_calendar_for_next_period_change(&mut self, offset_at: impl Fn(i64) -> i64) {
        let period = self.ss_current_period();
        let from = self.seven_signs.next_period_change;
        self.seven_signs.next_period_change = next_period_change_ms(period, from, offset_at);
        debug!(
            "SevenSigns: Next period change set to {} (epoch ms)",
            self.seven_signs.next_period_change
        );
    }

    /// `ThreadPool.schedule(new SevenSignsPeriodChange(), delay)` (a negative delay runs at once).
    fn ss_schedule_period_change(&mut self, delay_ms: i64) {
        let due = self.tick + TimerWheel::ms_to_ticks(u64::try_from(delay_ms).unwrap_or(0));
        let timer = self
            .timers
            .schedule_at(due, TimerKind::SevenSigns(SevenSignsTimer::PeriodChange));
        if let Some(old) = self.seven_signs.period_timer.replace(timer) {
            self.timers.cancel(old);
        }
    }

    fn ss_schedule_global_save(&mut self) {
        let due = self.tick + TimerWheel::ms_to_ticks(GLOBAL_SAVE_MS);
        let timer = self
            .timers
            .schedule_at(due, TimerKind::SevenSigns(SevenSignsTimer::GlobalSave));
        if let Some(old) = self.seven_signs.save_timer.replace(timer) {
            self.timers.cancel(old);
        }
    }

    /// When the period changes next (epoch milliseconds, Java `_nextPeriodChange`).
    pub fn ss_next_period_change(&self) -> i64 {
        self.seven_signs.next_period_change
    }

    /// Whether the period change is on the wheel (tests).
    pub fn ss_has_period_timer(&self) -> bool {
        self.seven_signs.period_timer.is_some()
    }

    /// The Seven Signs timers.
    pub(crate) fn on_seven_signs_timer(&mut self, timer: SevenSignsTimer) {
        match timer {
            SevenSignsTimer::PeriodChange => {
                self.seven_signs.period_timer = None;
                self.ss_period_change_at(unix_ms_now(), l2_util::time::local_offset_millis);
            }
            SevenSignsTimer::GlobalSave => {
                self.seven_signs.save_timer = None;
                // `DailyResetManager.onSave`: `saveSevenSignsStatus()`; the festival save is the
                // festival's (not ported).
                self.ss_save_status();
                info!("SevenSigns: Data updated.");
                self.ss_schedule_global_save();
            }
            SevenSignsTimer::AutoSpawn(timer) => self.on_auto_spawn_timer(timer),
        }
    }

    // ------------------------------------------------------------------ the period change

    /// Java `SevenSignsPeriodChange.run()`: the period that ended decides what happens, then the
    /// data is saved, the losers leave the dungeons, the sky changes, the NPCs follow and the next
    /// change goes on the wheel. `now_ms` and `offset_at` are the clock of the next schedule.
    pub fn ss_period_change_at(&mut self, now_ms: i64, offset_at: impl Fn(i64) -> i64 + Copy) {
        // Remember the period check here refers to the period just ENDED!
        let period_ended = self.ss_current_period();
        self.seven_signs.status.active_period = period_ended.wrapping_add(1);
        match period_ended {
            PERIOD_COMP_RECRUITING => {
                // FOLLOW-UP(festival): `SevenSignsFestival.startFestivalManager()`.
                self.ss_send_message_to_all(
                    SystemMessageId::SevenSignsTheCompetitionPeriodHasBegunVisitAPriestOfDawnOrPriestessOfDuskToParticipateInTheEvent,
                );
            }
            PERIOD_COMPETITION => {
                self.ss_send_message_to_all(
                    SystemMessageId::SevenSignsTheCompetitionPeriodHasEndedTheNextQuestEventWillStartInOneWeek,
                );
                let comp_winner = self.ss_cabal_highest_score();
                // FOLLOW-UP(festival): stop the festival manager, `rewardHighestRanked()`.
                self.ss_calc_new_seal_owners();
                match comp_winner {
                    CABAL_DAWN => {
                        self.ss_send_message_to_all(SystemMessageId::SevenSignsTheLordsOfDawnHaveWon)
                    }
                    CABAL_DUSK => self
                        .ss_send_message_to_all(SystemMessageId::SevenSignsTheRevolutionariesOfDuskHaveWon),
                    _ => {}
                }
                self.seven_signs.status.previous_winner = comp_winner;
                // Reset Castle ticket buy count.
                let castles: Vec<CastleId> = self.castles().iter().map(|c| c.id).collect();
                for castle in castles {
                    if let Some(c) = self.castle_mut(castle) {
                        c.ticket_buy_count = 0;
                    }
                    self.ss_cycle_residence_write(ResidenceWrite::CastleTicketBuyCount {
                        castle_id: castle.0,
                        count: 0,
                    });
                }
            }
            PERIOD_COMP_RESULTS => {
                // Perform initial Seal Validation set up.
                self.ss_initialize_seals();
                // Buff/Debuff members of the event when Seal of Strife captured.
                self.ss_give_cp_mult(self.ss_seal_owner(SEAL_STRIFE));
                self.ss_send_message_to_all(SystemMessageId::SevenSignsTheSealValidationPeriodHasBegun);
                let winner = self.seven_signs.status.previous_winner;
                info!(
                    "SevenSigns: The {} have won the competition with {} points!",
                    cabal_name(winner),
                    self.ss_current_score(winner)
                );
            }
            PERIOD_SEAL_VALIDATION => {
                // Ensure a cycle restart when this period ends.
                self.seven_signs.status.active_period = PERIOD_COMP_RECRUITING;
                self.ss_send_message_to_all(SystemMessageId::SevenSignsTheSealValidationPeriodHasEnded);
                // Clear Seal of Strife influence.
                self.ss_remove_cp_mult();
                // Reset all data.
                self.ss_reset_player_data();
                self.ss_reset_seals();
                let status = &mut self.seven_signs.status;
                status.current_cycle = status.current_cycle.wrapping_add(1);
                // FOLLOW-UP(festival): `resetFestivalData(false)`.
                status.dawn_stone_score = 0.0;
                status.dusk_stone_score = 0.0;
                status.dawn_festival_score = 0;
                status.dusk_festival_score = 0;
            }
            _ => {}
        }

        // Make sure all Seven Signs data is saved for future use.
        self.ss_save_data_all();
        self.ss_save_status();

        let winner = cabal_short_name(self.ss_cabal_highest_score());
        self.ss_tele_losing_cabal_from_dungeons(winner);

        let sky = SsqInfo {
            state: ssq_info_state(&self.seven_signs.status),
        };
        self.broadcast_to_all_online_players(&sky);
        self.spawn_seven_signs_npc();

        info!(
            "SevenSigns: The {} period has begun!",
            period_name(self.ss_current_period()).unwrap_or("null")
        );
        self.ss_set_calendar_for_next_period_change(offset_at);
        let delay = self.seven_signs.next_period_change - now_ms;
        self.ss_schedule_period_change(delay);
    }

    /// Java `sendMessageToAll(sysMsgId)`.
    fn ss_send_message_to_all(&self, message: SystemMessageId) {
        self.broadcast_to_all_online_players(&SystemMessage::new(message));
    }

    /// Java `initializeSeals()`: only logs.
    fn ss_initialize_seals(&self) {
        let validation = self.ss_is_seal_validation_period();
        for seal in [SEAL_AVARICE, SEAL_GNOSIS, SEAL_STRIFE] {
            let owner = self.ss_seal_owner(seal);
            if owner != CABAL_NULL {
                if validation {
                    info!(
                        "SevenSigns: The {} have won the {}.",
                        cabal_name(owner),
                        seal_name(seal, false)
                    );
                } else {
                    info!(
                        "SevenSigns: The {} is currently owned by {}.",
                        seal_name(seal, false),
                        cabal_name(owner)
                    );
                }
            } else {
                info!("SevenSigns: The {} remains unclaimed.", seal_name(seal, false));
            }
        }
    }

    /// Java `resetPlayerData()`: every member loses cabal, seal and score (the stones and the
    /// Ancient Adena to collect stay).
    fn ss_reset_player_data(&mut self) {
        for p in self.seven_signs.players.values_mut() {
            p.cabal = String::new();
            p.seal = SEAL_NULL;
            p.contribution_score = 0;
        }
    }

    /// Java `resetSeals()`.
    fn ss_reset_seals(&mut self) {
        let status = &mut self.seven_signs.status;
        status.dawn_seal_totals = [0; 4];
        status.dusk_seal_totals = [0; 4];
    }

    /// Java `calcNewSealOwners()`: 35% of a cabal's members must have chosen a seal to take it,
    /// 10% to keep it.
    fn ss_calc_new_seal_owners(&mut self) {
        let percent = |proportion: i32, members: i32| {
            let members = if members == 0 { 1 } else { members };
            java_round_f32((proportion as f32 / members as f32) * 100.0)
        };
        let dawn_members = self.ss_total_members(CABAL_DAWN);
        let dusk_members = self.ss_total_members(CABAL_DUSK);
        let highest = self.ss_cabal_highest_score();
        for seal in [SEAL_AVARICE, SEAL_GNOSIS, SEAL_STRIFE] {
            let status = &self.seven_signs.status;
            let prev_owner = status.seal_owner(seal);
            let dawn_percent = percent(status.seal_proportion(seal, CABAL_DAWN), dawn_members);
            let dusk_percent = percent(status.seal_proportion(seal, CABAL_DUSK), dusk_members);
            let new_owner = match (prev_owner, highest) {
                (CABAL_NULL, CABAL_DAWN) if dawn_percent >= 35 => CABAL_DAWN,
                (CABAL_NULL, CABAL_DUSK) if dusk_percent >= 35 => CABAL_DUSK,
                (CABAL_NULL, _) => CABAL_NULL,
                (CABAL_DAWN, CABAL_NULL | CABAL_DAWN) if dawn_percent >= 10 => CABAL_DAWN,
                (CABAL_DAWN, CABAL_DUSK) if dusk_percent >= 35 => CABAL_DUSK,
                (CABAL_DAWN, CABAL_DUSK) if dawn_percent >= 10 => CABAL_DAWN,
                (CABAL_DAWN, _) => CABAL_NULL,
                (CABAL_DUSK, CABAL_NULL | CABAL_DUSK) if dusk_percent >= 10 => CABAL_DUSK,
                (CABAL_DUSK, CABAL_DAWN) if dawn_percent >= 35 => CABAL_DAWN,
                (CABAL_DUSK, CABAL_DAWN) if dusk_percent >= 10 => CABAL_DUSK,
                (CABAL_DUSK, _) => CABAL_NULL,
                // An owner value Java does not know: `newSealOwner` stays CABAL_NULL.
                _ => CABAL_NULL,
            };
            if let Some(owner) = usize::try_from(seal)
                .ok()
                .and_then(|i| self.seven_signs.status.seal_owners.get_mut(i))
            {
                *owner = new_owner;
            }
            // Alert all online players to new seal status.
            let message = match (seal, new_owner) {
                (SEAL_AVARICE, CABAL_DAWN) => {
                    Some(SystemMessageId::SevenSignsTheLordsOfDawnHaveObtainedTheSealOfAvarice)
                }
                (SEAL_AVARICE, CABAL_DUSK) => {
                    Some(SystemMessageId::SevenSignsTheRevolutionariesOfDuskHaveObtainedTheSealOfAvarice)
                }
                (SEAL_GNOSIS, CABAL_DAWN) => {
                    Some(SystemMessageId::SevenSignsTheLordsOfDawnHaveObtainedTheSealOfGnosis)
                }
                (SEAL_GNOSIS, CABAL_DUSK) => {
                    Some(SystemMessageId::SevenSignsTheRevolutionariesOfDuskHaveObtainedTheSealOfGnosis)
                }
                (SEAL_STRIFE, CABAL_DAWN) => {
                    Some(SystemMessageId::SevenSignsTheLordsOfDawnHaveObtainedTheSealOfStrife)
                }
                (SEAL_STRIFE, CABAL_DUSK) => {
                    Some(SystemMessageId::SevenSignsTheRevolutionariesOfDuskHaveObtainedTheSealOfStrife)
                }
                _ => None,
            };
            if let Some(message) = message {
                self.ss_send_message_to_all(message);
            }
            if seal == SEAL_STRIFE {
                self.ss_validate_taxes(new_owner);
            }
        }
    }

    /// Java `CastleManager.validateTaxes(sealStrifeOwner)`: no castle may tax more than 5% (Dusk
    /// owns the Seal of Strife), 25% (Dawn) or 15% (nobody). The rows are a cycle's (D-53).
    fn ss_validate_taxes(&mut self, strife_owner: i32) {
        let max_tax = match strife_owner {
            CABAL_DUSK => 5,
            CABAL_DAWN => 25,
            _ => 15,
        };
        let over: Vec<CastleId> = self
            .castles()
            .iter()
            .filter(|c| c.tax_percent > max_tax)
            .map(|c| c.id)
            .collect();
        for castle in over {
            if let Some(c) = self.castle_mut(castle) {
                c.tax_percent = max_tax;
                c.tax_rate = f64::from(max_tax) / 100.0;
            }
            self.ss_cycle_residence_write(ResidenceWrite::CastleTaxPercent {
                castle_id: castle.0,
                tax_percent: max_tax,
            });
        }
    }

    /// A castle row the cycle changes: only with `CycleDatabaseWrites` (D-53).
    fn ss_cycle_residence_write(&mut self, write: ResidenceWrite) {
        if self.seven_signs.settings.cycle_database_writes {
            self.residence_write(write);
        } else {
            debug!("SideBySide: not written (CycleDatabaseWrites = False): {write:?}");
        }
    }

    /// Java `teleLosingCabalFromDungeons(compWinner)`.
    fn ss_tele_losing_cabal_from_dungeons(&mut self, comp_winner: &str) {
        let mut ids: Vec<ObjectId> = self.players.keys().copied().collect();
        ids.sort_unstable();
        let validation_or_results = self.ss_is_seal_validation_period() || self.ss_is_comp_results_period();
        for id in ids {
            if self.is_gm(id) || !self.is_in_7s_dungeon(id) {
                continue;
            }
            let cabal = self.seven_signs.players.get(&id.0).map(|p| p.cabal.clone());
            if validation_or_results {
                if cabal.as_deref() != Some(comp_winner) {
                    self.tele_to_location_where(id, TeleportWhereType::Town);
                    self.set_in_7s_dungeon(id, false);
                    self.send_message(
                        id,
                        "You have been teleported to the nearest town due to the beginning of the Seal Validation period.",
                    );
                }
            }
            // NOTE(java-quirk): `!cabal.isEmpty()` sends away the members who did sign, with the
            // message for those who did not.
            else if cabal.as_deref().is_none_or(|c| !c.is_empty()) {
                self.tele_to_location_where(id, TeleportWhereType::Town);
                self.set_in_7s_dungeon(id, false);
                self.send_message(
                    id,
                    "You have been teleported to the nearest town because you have not signed for any cabal.",
                );
            }
        }
    }

    /// Java `giveCPMult(strifeOwner)`: "The Victor of War" for the members of the cabal that owns
    /// the Seal of Strife, "The Vanquished of War" for the other members.
    pub(crate) fn ss_give_cp_mult(&mut self, strife_owner: i32) {
        let mut ids: Vec<ObjectId> = self.players.keys().copied().collect();
        ids.sort_unstable();
        for id in ids {
            let cabal = self.ss_player_cabal(id.0);
            if cabal == CABAL_NULL {
                continue;
            }
            let skill = if cabal == strife_owner {
                THE_VICTOR_OF_WAR
            } else {
                THE_VANQUISHED_OF_WAR
            };
            self.add_skill(id, skill, 1);
        }
    }

    /// Java `removeCPMult()`.
    // NOTE: Java's `Player.removeSkill(skill)` also deletes the skill's `character_skills` row,
    // which these passive skills never have; no row is written here.
    pub(crate) fn ss_remove_cp_mult(&mut self) {
        let mut ids: Vec<ObjectId> = self.players.keys().copied().collect();
        ids.sort_unstable();
        for id in ids {
            self.remove_skill(id, THE_VICTOR_OF_WAR, true);
            self.remove_skill(id, THE_VANQUISHED_OF_WAR, true);
        }
    }

    // ------------------------------------------------------------------ saves

    fn ss_write(&mut self, write: SevenSignsWrite) {
        self.outbox.persist(PersistCommand::SevenSigns(write));
    }

    /// Java `saveSevenSignsData()`: every member (a period change, the shutdown). Global: only
    /// with `CycleDatabaseWrites` (D-53).
    pub(crate) fn ss_save_data_all(&mut self) {
        if !self.seven_signs.settings.cycle_database_writes {
            debug!("SideBySide: seven_signs not saved (CycleDatabaseWrites = False)");
            return;
        }
        let rows: Vec<SignsPlayer> = self.seven_signs.players.values().cloned().collect();
        self.ss_write(SevenSignsWrite::UpdateAllPlayers(rows));
    }

    /// Java `saveSevenSignsData(objectId)`: one member (per player, always written).
    pub(crate) fn ss_save_data(&mut self, char_id: i32) {
        if let Some(row) = self.seven_signs.players.get(&char_id).cloned() {
            self.ss_write(SevenSignsWrite::UpdatePlayer(row));
        }
    }

    /// Java `saveSevenSignsStatus()`: `_lastSave` is now and the row is written. Global: only with
    /// `CycleDatabaseWrites` (D-53); without it the row and `_lastSave` stay.
    pub(crate) fn ss_save_status(&mut self) {
        if !self.seven_signs.settings.cycle_database_writes {
            debug!("SideBySide: seven_signs_status not saved (CycleDatabaseWrites = False)");
            return;
        }
        self.seven_signs.status.last_save = unix_ms_now();
        let status = self.seven_signs.status.clone();
        self.ss_write(SevenSignsWrite::UpdateStatus(status));
    }

    /// Java `Shutdown.saveData`: "Save Seven Signs data before closing." (both global).
    pub(crate) fn seven_signs_shutdown(&mut self) {
        self.ss_save_data_all();
        info!("SevenSigns: Seven Signs data saved.");
        self.ss_save_status();
        info!("SevenSigns: Seven Signs status saved.");
    }

    // ------------------------------------------------------------------ the questions of others

    /// Java `isDateInSealValidPeriod(date)`: whether `date_ms` falls into a seal validation or a
    /// results period.
    pub fn ss_is_date_in_seal_valid_period(&self, date_ms: i64, now_ms: i64) -> bool {
        if !self.seven_signs.loaded {
            // No cycle to measure against (a bare world): every date is valid.
            return true;
        }
        date_in_seal_valid_period(
            self.ss_current_period(),
            self.seven_signs.next_period_change - now_ms,
            date_ms - now_ms,
        )
    }

    // ------------------------------------------------------------------ entering the world

    /// `EnterWorld`'s "Updating Seal of Strife Buff/Debuff".
    pub(crate) fn ss_enter_world_strife(&mut self, id: ObjectId) {
        let strife = self.ss_seal_owner(SEAL_STRIFE);
        if self.ss_is_seal_validation_period() && strife != CABAL_NULL {
            let cabal = self.ss_player_cabal(id.0);
            if cabal != CABAL_NULL {
                let skill = if cabal == strife {
                    THE_VICTOR_OF_WAR
                } else {
                    THE_VANQUISHED_OF_WAR
                };
                self.add_skill(id, skill, 1);
            }
        } else {
            self.remove_skill(id, THE_VICTOR_OF_WAR, true);
            self.remove_skill(id, THE_VANQUISHED_OF_WAR, true);
        }
    }

    /// Java `sendCurrentPeriodMsg(player)`: the message of the period (`EnterWorld`).
    pub(crate) fn ss_current_period_message(&self) -> Option<SystemMessage> {
        let id = match self.ss_current_period() {
            PERIOD_COMP_RECRUITING => SystemMessageId::SevenSignsPreparationsHaveBegunForTheNextQuestEvent,
            PERIOD_COMPETITION => {
                SystemMessageId::SevenSignsTheQuestEventPeriodHasBegunSpeakWithAPriestOfDawnOrDuskPriestessIfYouWishToParticipateInTheEvent
            }
            PERIOD_COMP_RESULTS => SystemMessageId::SevenSignsQuestEventHasEndedResultsAreBeingTallied,
            PERIOD_SEAL_VALIDATION => {
                SystemMessageId::SevenSignsThisIsTheSealValidationPeriodANewQuestEventPeriodBeginsNextMonday
            }
            _ => return None,
        };
        Some(SystemMessage::new(id))
    }

    /// `Player.onPlayerEnter()`: a player left in a dungeon he may not be in goes to town.
    pub(crate) fn ss_on_player_enter(&mut self, id: ObjectId) {
        if self.is_gm(id) || !self.is_in_7s_dungeon(id) {
            return;
        }
        let cabal = self.ss_player_cabal(id.0);
        if self.ss_is_seal_validation_period() || self.ss_is_comp_results_period() {
            if cabal != self.ss_cabal_highest_score() {
                self.tele_to_location_where(id, TeleportWhereType::Town);
                self.set_in_7s_dungeon(id, false);
                self.send_message(
                    id,
                    "You have been teleported to the nearest town due to the beginning of the Seal Validation period.",
                );
            }
        } else if cabal == CABAL_NULL {
            self.tele_to_location_where(id, TeleportWhereType::Town);
            self.set_in_7s_dungeon(id, false);
            self.send_message(
                id,
                "You have been teleported to the nearest town because you have not signed for any cabal.",
            );
        }
    }

    // ------------------------------------------------------------------ D-50 re-reads

    /// Applies what the session read at `CharacterSelect` (`PlayerProfile::seven_signs_refresh`):
    /// only with `CycleDatabaseWrites = False`, when the Java game server may have changed the
    /// rows. Called from `on_entered` before anything reads the member's cabal.
    pub(crate) fn ss_refresh_player(&mut self, id: ObjectId) {
        let Some(refresh) = self
            .players
            .get_mut(&id)
            .and_then(|p| p.data.profile.seven_signs_refresh.take())
        else {
            return;
        };
        if self.seven_signs.settings.cycle_database_writes {
            return;
        }
        match refresh.player {
            Some(row) if row.char_id == id.0 => {
                self.seven_signs.players.insert(id.0, row);
            }
            Some(_) => {}
            None => {
                self.seven_signs.players.remove(&id.0);
            }
        }
        if let Some(status) = refresh.status {
            self.ss_apply_status_read(status);
        }
    }

    /// Asks the persistence task for every Seven Signs row (D-50); the answer comes back as
    /// `WorldCommand::SevenSignsLoaded` and goes on with `then`.
    pub(crate) fn ss_request_read(&mut self, then: SevenSignsThen) {
        self.outbox
            .persist(PersistCommand::SevenSignsReadRequest { then });
    }

    /// The answer to `PersistCommand::SevenSignsReadRequest`. A failed read changes nothing and
    /// refuses the follow-up (D-50).
    pub(crate) fn on_seven_signs_loaded(&mut self, result: Result<SevenSignsLoad, ()>, then: SevenSignsThen) {
        let Ok(load) = result else {
            warn!("SevenSigns: the rows could not be read again; {then:?} is refused.");
            return;
        };
        if !self.seven_signs.settings.cycle_database_writes {
            // The members who are online here keep the rows this server has (it writes them).
            let online: std::collections::HashSet<i32> = self.players.keys().map(|p| p.0).collect();
            self.seven_signs
                .players
                .retain(|char_id, _| online.contains(char_id));
            for row in load.players {
                if !online.contains(&row.char_id) {
                    self.seven_signs.players.insert(row.char_id, row);
                }
            }
            if let Some(status) = load.status {
                self.ss_apply_status_read(status);
            }
        }
        match then {
            SevenSignsThen::Nothing => {}
            SevenSignsThen::Status { player, page } => self.ss_send_status(player, page),
        }
    }

    /// The status row as the Java game server wrote it replaces this world's (D-50, D-53). A new
    /// period moves the period change on the wheel and the Seven Signs NPCs follow a new state.
    pub(crate) fn ss_apply_status_read(&mut self, status: SevenSignsStatus) {
        let before = (
            self.ss_current_period(),
            self.seven_signs.status.seal_owners,
            self.ss_cabal_highest_score(),
        );
        self.seven_signs.status = status;
        let after = (
            self.ss_current_period(),
            self.seven_signs.status.seal_owners,
            self.ss_cabal_highest_score(),
        );
        if before.0 != after.0 {
            let now = unix_ms_now();
            let status = &self.seven_signs.status;
            // A 15 minute period ends 15 minutes after the Java GS saved its change; a week long
            // one at the next Monday 18:00.
            self.seven_signs.next_period_change = match status.active_period {
                PERIOD_COMP_RECRUITING | PERIOD_COMP_RESULTS if status.last_save > 7 => {
                    status.last_save.wrapping_add(PERIOD_MINOR_LENGTH)
                }
                period => next_period_change_ms(period, now, l2_util::time::local_offset_millis),
            };
            self.ss_schedule_period_change(self.seven_signs.next_period_change - now);
        }
        if before != after {
            self.spawn_seven_signs_npc();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utc(_ms: i64) -> i64 {
        0
    }

    /// 2026-01-05 (a Monday) `hour:min:sec.250` UTC.
    fn monday(hour: i64, min: i64, sec: i64) -> i64 {
        1_767_571_200_000 + hour * HOUR_MS + min * MINUTE_MS + sec * SECOND_MS + 250
    }

    #[test]
    fn the_major_periods_end_on_monday_at_18() {
        // Monday noon: today 18:00 (the seconds of the calendar stay).
        assert_eq!(
            next_period_change_ms(PERIOD_COMPETITION, monday(12, 0, 7), utc),
            monday(18, 0, 7)
        );
        // Monday 18:30: next Monday.
        assert_eq!(
            next_period_change_ms(PERIOD_SEAL_VALIDATION, monday(18, 30, 0), utc),
            monday(18, 0, 0) + WEEK_MS
        );
        // Sunday 10:00: tomorrow.
        assert_eq!(
            next_period_change_ms(PERIOD_COMPETITION, monday(10, 0, 0) - DAY_MS, utc),
            monday(18, 0, 0)
        );
        // Wednesday: five days on.
        assert_eq!(
            next_period_change_ms(PERIOD_COMPETITION, monday(9, 0, 0) + 2 * DAY_MS, utc),
            monday(18, 0, 0) + WEEK_MS
        );
        // The 15 minute periods.
        assert_eq!(
            next_period_change_ms(PERIOD_COMP_RESULTS, monday(18, 0, 0), utc),
            monday(18, 15, 0)
        );
        // A zone two hours ahead: 18:00 local is 16:00 UTC.
        let plus_two = |_ms: i64| 2 * HOUR_MS;
        assert_eq!(
            next_period_change_ms(PERIOD_COMPETITION, monday(12, 0, 0), plus_two),
            monday(16, 0, 0)
        );
    }

    #[test]
    fn a_change_missed_while_offline_is_seen() {
        // Saved on Monday 17:00, now Monday 19:00: the 18:00 change was missed.
        assert!(is_next_period_change_in_past(
            PERIOD_COMPETITION,
            monday(17, 0, 0),
            monday(19, 0, 0),
            utc
        ));
        // Saved at 18:30, now Tuesday: nothing missed.
        assert!(!is_next_period_change_in_past(
            PERIOD_COMPETITION,
            monday(18, 30, 0),
            monday(10, 0, 0) + DAY_MS,
            utc
        ));
        // A results period saved 20 minutes ago is over.
        assert!(is_next_period_change_in_past(
            PERIOD_COMP_RESULTS,
            monday(18, 0, 0),
            monday(18, 20, 0),
            utc
        ));
        // The old date column (7 or less) never counts.
        assert!(!is_next_period_change_in_past(
            PERIOD_COMPETITION,
            1,
            monday(19, 0, 0),
            utc
        ));
    }

    #[test]
    fn the_seal_validation_window_follows_the_period() {
        // In the competition with a day left: tomorrow + 1 hour is in the validation period,
        // tomorrow - 1 hour is not, and so is 8 days on (the next competition).
        let left = DAY_MS;
        assert!(date_in_seal_valid_period(
            PERIOD_COMPETITION,
            left,
            DAY_MS + HOUR_MS
        ));
        assert!(!date_in_seal_valid_period(
            PERIOD_COMPETITION,
            left,
            DAY_MS - HOUR_MS
        ));
        assert!(!date_in_seal_valid_period(PERIOD_COMPETITION, left, 9 * DAY_MS));
        // In the validation period everything up to its end is valid.
        assert!(date_in_seal_valid_period(PERIOD_SEAL_VALIDATION, left, HOUR_MS));
        assert!(!date_in_seal_valid_period(
            PERIOD_SEAL_VALIDATION,
            left,
            2 * DAY_MS
        ));
    }

    #[test]
    fn contributions_and_rewards_count_by_colour() {
        assert_eq!(calc_contribution_score(1, 1, 1), 18);
        assert_eq!(calc_ancient_adena_reward(2, 0, 3), 36);
    }

    #[test]
    fn the_sky_is_the_winners_in_the_validation_period_only() {
        let mut s = SevenSignsStatus {
            active_period: PERIOD_SEAL_VALIDATION,
            dawn_stone_score: 10.0,
            ..SevenSignsStatus::default()
        };
        assert_eq!(ssq_info_state(&s), 2);
        s.dawn_stone_score = 0.0;
        s.dusk_stone_score = 10.0;
        assert_eq!(ssq_info_state(&s), 1);
        s.active_period = PERIOD_COMPETITION;
        assert_eq!(ssq_info_state(&s), 0);
    }
}
