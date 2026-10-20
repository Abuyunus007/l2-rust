//! Nevit's blessing: the hunting points a player earns by hunting, the 3 minute blessing they
//! fill, the advent time that limits it per day, and the hourglass bonus of the recommendations.
// Java: entity/actor/holders/player/Nevit.java
// Java: entity/actor/Player.java (the Nevit hourglass: startNevitHourglassTask, getNevitHourglass*,
//       loadRecommendations, storeRecommendations, checkRecoBonusTask)
// Java: entity/actor/tasks/player/RecoBonusTaskEnd.java
// Java: entity/zone/type/PeaceZone.java (onEnter)
// Java: network/serverpackets/{ExNevitAdventEffect,ExNevitAdventPointInfoPacket,
//       ExNevitAdventTimeChange,ExVoteSystemInfo}.java
//
// # State
// Java keeps three scheduled tasks per player and four values in `PlayerVariables`:
//
// | Java | Here |
// |---|---|
// | `Nevit._adventTask` (30 s, once) | [`PlayerNevit::advent`], `TimerKind::NevitAdvent` |
// | `Nevit._nevitEffectTask` (the blessing) | [`PlayerNevit::effect`], `TimerKind::NevitEffectEnd` |
// | `Player._nevitHourglassTask` | [`PlayerNevit::hourglass`], `TimerKind::NevitHourglassEnd` |
// | `PlayerStat._pausedNevitHourglass` | [`PlayerNevit::paused`] |
// | variables `hunting_points`, `hunting_time`, `nevit_b` | `PlayerProfile::variables` (below) |
//
// Remaining times are `end - now` on the world's clock (`tick * TICK`), which is what Java's
// `ScheduledFuture.getDelay` answers.
//
// # Variables and recommendations in the database (D-39)
// `PlayerVariables` keeps its changes in memory and writes them 60 s after the first change, at a
// store, and on logout. The world keeps the values in `PlayerProfile::variables` and the names it
// touched in `variables_changed` / `variables_removed`; the persistence task upserts and deletes
// those names with every store (autosave and logout). `character_reco_bonus` is written the same
// way from `recom_have`, `recom_left` and `recom_time_left_ms` (`reco_store`).
//
// # What is left out
// - The visual aura (`AbnormalVisualEffect.NAVIT_ADVENT`): abnormal visual effects belong to the
//   effect system (M5), so the blessing works but nobody sees its glow.
// - `EffectType.NEVITS_HOURGLASS` (an effect that keeps the hourglass paused) is the
//   `NevitsHourglass` handler (`effects/handlers/nevits_hourglass.rs`, M5-EFX-05).
// - The "hunting" exp bonus is a `TODO` in Java too.
//
// The recommendations (`RecoGiveTask`, `RequestVoteNew`) and the daily reset
// (`DailyResetManager.resetNevit` / `resetRecommends`) are in `recommendations.rs` (M5-FU-03).

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    ExNevitAdventEffect, ExNevitAdventPointInfo, ExNevitAdventTimeChange, SystemMessage,
};
use l2_util::ObjectId;

use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::{TICK, World, unix_ms_now};
use crate::zones::ZoneId;

/// Java `Player.NEVIT_HOURGLASS_BONUS`: the exp bonus in percent by level / 10 and by
/// recommendations / 10.
pub const NEVIT_HOURGLASS_BONUS: [[i32; 10]; 9] = [
    [25, 50, 50, 50, 50, 50, 50, 50, 50, 50],
    [16, 33, 50, 50, 50, 50, 50, 50, 50, 50],
    [12, 25, 37, 50, 50, 50, 50, 50, 50, 50],
    [10, 20, 30, 40, 50, 50, 50, 50, 50, 50],
    [8, 16, 25, 33, 41, 50, 50, 50, 50, 50],
    [7, 14, 21, 28, 35, 42, 50, 50, 50, 50],
    [6, 12, 18, 25, 31, 37, 43, 50, 50, 50],
    [5, 11, 16, 22, 27, 33, 38, 44, 50, 50],
    [5, 10, 15, 20, 25, 30, 35, 40, 45, 50],
];
/// Java `NEVIT_HOURGLASS_ACTIVE`, `_PAUSED` and `_MAINTAIN`: the status `ExVoteSystemInfo` shows.
pub const NEVIT_HOURGLASS_ACTIVE: i32 = 0;
pub const NEVIT_HOURGLASS_PAUSED: i32 = 1;
pub const NEVIT_HOURGLASS_MAINTAIN: i32 = 10;

/// Java `AdventTask`: the advent time grows by 30 s per run, and each run earns 72 points.
const ADVENT_TASK_MS: u64 = 30_000;
const ADVENT_TASK_SECS: i32 = 30;
const ADVENT_TASK_POINTS: i32 = 72;
/// Java `PlayerStat.addLevel`: "Nevit System" gives 2000 points for a new level.
pub(crate) const LEVEL_UP_POINTS: i32 = 2000;
/// Java `Player.loadRecommendations`: the hourglass of a character without a row, one hour.
pub const DEFAULT_HOURGLASS_MS: i64 = 3_600_000;

/// Names of the `character_variables` rows.
const HUNTING_POINTS: &str = "hunting_points";
const HUNTING_TIME: &str = "hunting_time";
const NEVIT_B: &str = "nevit_b";

/// The `.ini` values Nevit reads besides `NevitEnabled` (`WorldSettings::nevit_enabled`).
/// `Default` is the Java default of each key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NevitSettings {
    /// `PlayerConfig.NEVIT_MAX_POINTS` (`NevitMaxPoints`): the points that start the blessing.
    pub max_points: i32,
    /// `PlayerConfig.NEVIT_BONUS_EFFECT_TIME` (`NevitBonusEffectTime`): seconds the blessing lasts.
    pub bonus_effect_time: i32,
    /// `PlayerConfig.NEVIT_ADVENT_TIME` (`NevitAdventTime`): seconds of hunting a day earn points.
    pub advent_time: i32,
    /// `PlayerConfig.NEVIT_IGNORE_ADVENT_TIME` (`NevitIgnoreAdventTime`).
    pub ignore_advent_time: bool,
}

impl Default for NevitSettings {
    fn default() -> Self {
        Self {
            max_points: 7200,
            bonus_effect_time: 180,
            advent_time: 14_400,
            ignore_advent_time: false,
        }
    }
}

/// Java `Player.getNevitHourglassBonus`: the exp bonus in percent of an active hourglass.
pub fn hourglass_bonus(level: i32, recom_have: i32) -> i32 {
    // NOTE: Java indexes the table with `level / 10` and throws from level 90; the highest
    // player level is 85, so the clamp never matters.
    let row = usize::try_from(level / 10)
        .unwrap_or(0)
        .min(NEVIT_HOURGLASS_BONUS.len() - 1);
    let column = usize::try_from((recom_have.min(100) - 1) / 10)
        .unwrap_or(0)
        .min(9);
    NEVIT_HOURGLASS_BONUS[row][column]
}

/// Java `Nevit.calcPercent`: `(int) ((100.0D / NEVIT_MAX_POINTS) * points)`.
pub fn calc_percent(points: i32, max_points: i32) -> i32 {
    ((100.0 / f64::from(max_points)) * f64::from(points)) as i32
}

/// The moment Java's `Nevit.onPlayerLogin` calls the day's start: today 6:30:00.000 in the
/// server's zone, as Unix milliseconds. `offset_ms` is the zone's offset from UTC (D-48).
pub fn nevit_reset_boundary_ms(now_ms: i64, offset_ms: i64) -> i64 {
    const DAY_MS: i64 = 86_400_000;
    let local = now_ms + offset_ms;
    let midnight = local - local.rem_euclid(DAY_MS);
    midnight + (6 * 60 + 30) * 60_000 - offset_ms
}

/// A running scheduled task and the world time (ms) it ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Running {
    timer: TimerId,
    end_ms: u64,
}

/// Java `Player._nevitHourglassTask`: `timer` is `None` once it ran (the task is then `isDone()`
/// but still set, so Java does not start another one).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Hourglass {
    timer: Option<TimerId>,
    end_ms: u64,
}

/// One player's Nevit state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlayerNevit {
    advent: Option<TimerId>,
    effect: Option<Running>,
    hourglass: Option<Hourglass>,
    paused: bool,
}

/// The Nevit state of every player in the world (a field of [`ExpWorld`](super::exp::ExpWorld)).
#[derive(Debug, Default)]
pub struct NevitWorld {
    players: std::collections::HashMap<ObjectId, PlayerNevit>,
}

impl NevitWorld {
    /// The state of a player, if the world created it.
    pub fn get(&self, id: ObjectId) -> Option<&PlayerNevit> {
        self.players.get(&id)
    }
}

impl World {
    /// The world clock in milliseconds (what Java's `getDelay` counts against).
    fn clock_ms(&self) -> u64 {
        self.tick * TICK.as_millis() as u64
    }

    fn nevit_mut(&mut self, id: ObjectId) -> &mut PlayerNevit {
        self.rewards.nevit.players.entry(id).or_default()
    }

    fn schedule_after_ms(&mut self, ms: u64, kind: TimerKind) -> TimerId {
        let due = self.tick + TimerWheel::ms_to_ticks(ms);
        self.timers.schedule_at(due, kind)
    }

    // ------------------------------------------------------------------ variables

    /// An integer player variable; 0 when it is missing or not a number (`getInt(name, 0)`).
    pub(crate) fn player_variable(&self, id: ObjectId, name: &str) -> i32 {
        self.players
            .get(&id)
            .and_then(|p| p.data.profile.variables.get(name))
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(0)
    }

    /// Java `PlayerVariables.set(name, int)`.
    pub(crate) fn set_player_variable(&mut self, id: ObjectId, name: &str, value: i32) {
        if let Some(player) = self.players.get_mut(&id) {
            let profile = &mut player.data.profile;
            profile.variables.insert(name.to_owned(), value.to_string());
            profile.variables_removed.remove(name);
            profile.variables_changed.insert(name.to_owned());
        }
    }

    /// Java `PlayerVariables.remove(name)`.
    pub(crate) fn remove_player_variable(&mut self, id: ObjectId, name: &str) {
        if let Some(player) = self.players.get_mut(&id) {
            let profile = &mut player.data.profile;
            if profile.variables.remove(name).is_some() {
                profile.variables_changed.remove(name);
                profile.variables_removed.insert(name.to_owned());
            }
        }
    }

    // ------------------------------------------------------------------ DailyResetManager

    /// The per-player part of Java `DailyResetManager.resetNevit`: `hunting_time` back to 0 and
    /// the client is told. A running advent task is left alone, like Java.
    pub(crate) fn nevit_reset_advent_time(&mut self, id: ObjectId) {
        self.set_player_variable(id, HUNTING_TIME, 0);
        self.send(id, &ExNevitAdventTimeChange::new(0, true));
    }

    // ------------------------------------------------------------------ Nevit.java: reads

    /// Java `Nevit.getAdventPoints()`.
    pub fn nevit_adventure_points(&self, id: ObjectId) -> i32 {
        if self.settings.nevit_enabled {
            self.player_variable(id, HUNTING_POINTS)
        } else {
            0
        }
    }

    /// Java `Nevit.getAdventTime()`.
    pub fn nevit_advent_time(&self, id: ObjectId) -> i32 {
        if self.settings.nevit_enabled {
            self.player_variable(id, HUNTING_TIME)
        } else {
            0
        }
    }

    /// Java `Nevit.getEffectTime()`: whole seconds the blessing has left.
    fn nevit_effect_time(&self, id: ObjectId) -> i32 {
        let now = self.clock_ms();
        self.rewards
            .nevit
            .players
            .get(&id)
            .and_then(|n| n.effect)
            .map_or(0, |e| {
                i32::try_from(e.end_ms.saturating_sub(now) / 1000).unwrap_or(i32::MAX)
            })
    }

    /// Java `Nevit.isAdventBlessingActive()`.
    pub fn is_advent_blessing_active(&self, id: ObjectId) -> bool {
        let now = self.clock_ms();
        self.rewards
            .nevit
            .players
            .get(&id)
            .and_then(|n| n.effect)
            .is_some_and(|e| e.end_ms > now)
    }

    /// Java `Player.getNevitHourglassTime()`: whole seconds the hourglass has left.
    pub fn nevit_hourglass_time(&self, id: ObjectId) -> i32 {
        let now = self.clock_ms();
        self.rewards
            .nevit
            .players
            .get(&id)
            .and_then(|n| n.hourglass)
            .map_or(0, |h| {
                i32::try_from(h.end_ms.saturating_sub(now) / 1000).unwrap_or(i32::MAX)
            })
    }

    /// Java `PlayerStat.hasPausedNevitHourglass()`.
    pub(crate) fn nevit_hourglass_paused(&self, id: ObjectId) -> bool {
        self.rewards.nevit.players.get(&id).is_some_and(|n| n.paused)
    }

    /// Java `Player.isNevitHourglassActive()`.
    pub fn is_nevit_hourglass_active(&self, id: ObjectId) -> bool {
        self.nevit_hourglass_time(id) > 0 || self.nevit_hourglass_paused(id)
    }

    /// Java `Player.getNevitHourglassStatus()`.
    pub fn nevit_hourglass_status(&self, id: ObjectId) -> i32 {
        if self.nevit_hourglass_paused(id) {
            NEVIT_HOURGLASS_PAUSED
        } else if self.nevit_hourglass_time(id) > 0 {
            NEVIT_HOURGLASS_ACTIVE
        } else {
            NEVIT_HOURGLASS_MAINTAIN
        }
    }

    /// Java `Player.getNevitHourglassBonus()`: only while online with recommendations and an
    /// active hourglass.
    pub fn nevit_hourglass_bonus(&self, id: ObjectId) -> i32 {
        let Some(player) = self.players.get(&id) else {
            return 0;
        };
        if player.recom_have != 0 && self.is_nevit_hourglass_active(id) {
            hourglass_bonus(player.core.level, player.recom_have)
        } else {
            0
        }
    }

    /// Java `Player.getNevitHourglassMultiplier()`.
    pub fn nevit_hourglass_multiplier(&self, id: ObjectId) -> f64 {
        let mut multiplier = 1.0;
        let bonus = f64::from(self.nevit_hourglass_bonus(id));
        if bonus > 0.0 {
            multiplier += bonus / 100.0;
        }
        multiplier
    }

    // ------------------------------------------------------------------ Nevit.java: the login

    /// Java `Nevit.onPlayerLogin`: the daily reset of the advent time, the packets, the blessing
    /// the player logged out with and the message about the points.
    pub(crate) fn nevit_on_player_login(&mut self, id: ObjectId) {
        let now = unix_ms_now();
        let boundary = nevit_reset_boundary_ms(now, l2_util::time::local_offset_millis(now));
        let last_access = self.players.get(&id).map_or(0, |p| p.data.profile.last_access);
        // Reset Nevit's Blessing.
        // NOTE(java-quirk): `getLastAccess()` is milliseconds and is compared with seconds, so the
        // reset only happens for a character whose `lastAccess` is 0 (never logged out).
        if last_access < boundary / 1000 && now > boundary {
            self.set_player_variable(id, HUNTING_TIME, 0);
        }

        // Send Packets.
        let points = self.nevit_adventure_points(id);
        self.send(id, &ExNevitAdventPointInfo { points });
        let time = self.nevit_advent_time(id);
        self.send(id, &ExNevitAdventTimeChange::new(time, true));
        let blessing = self.player_variable(id, NEVIT_B);
        self.start_nevit_effect(id, blessing);

        // Set percent.
        let percent = calc_percent(
            self.player_variable(id, HUNTING_POINTS),
            self.settings.nevit.max_points,
        );
        let message = match percent {
            45..50 => Some(SystemMessageId::YouAreStartingToFeelTheEffectsOfNevitSAdventBlessing),
            50..75 => Some(
                SystemMessageId::YouAreFurtherInfusedWithTheBlessingsOfNevitContinueToBattleEvilWhereverItMayLurk,
            ),
            75.. => Some(SystemMessageId::NevitSAdventBlessingShinesStronglyFromAboveYouCanAlmostSeeHisDivineAura),
            _ => None,
        };
        if let Some(message) = message {
            self.send(id, &SystemMessage::new(message));
        }
    }

    // ------------------------------------------------------------------ Nevit.java: points

    fn set_advent_points(&mut self, id: ObjectId, points: i32) {
        self.set_player_variable(id, HUNTING_POINTS, points);
    }

    fn set_advent_time(&mut self, id: ObjectId, time: i32) {
        self.set_player_variable(id, HUNTING_TIME, time);
    }

    /// Java `Nevit.addPoints(value)`: a full bar starts the blessing; during the blessing the
    /// bar stays empty.
    pub fn nevit_add_points(&mut self, id: ObjectId, value: i32) {
        if self.nevit_effect_time(id) > 0 {
            self.set_advent_points(id, 0);
        } else {
            let points = self.nevit_adventure_points(id).wrapping_add(value);
            self.set_advent_points(id, points);
        }

        if self.nevit_adventure_points(id) > self.settings.nevit.max_points {
            self.set_advent_points(id, 0);
            let time = self.settings.nevit.bonus_effect_time;
            self.start_nevit_effect(id, time);
        }

        let points = self.nevit_adventure_points(id);
        let message = match calc_percent(points, self.settings.nevit.max_points) {
            45 => Some(SystemMessageId::YouAreStartingToFeelTheEffectsOfNevitSAdventBlessing),
            50 => Some(
                SystemMessageId::YouAreFurtherInfusedWithTheBlessingsOfNevitContinueToBattleEvilWhereverItMayLurk,
            ),
            75 => Some(SystemMessageId::NevitSAdventBlessingShinesStronglyFromAboveYouCanAlmostSeeHisDivineAura),
            _ => None,
        };
        if let Some(message) = message {
            self.send(id, &SystemMessage::new(message));
        }
        self.send(id, &ExNevitAdventPointInfo { points });
    }

    // ------------------------------------------------------------------ Nevit.java: advent task

    /// Java `Nevit.startAdventTask`: after a gain of exp the advent time starts to run (once the
    /// 30 s task is over, the next gain of exp starts it again).
    pub(crate) fn nevit_start_advent_task(&mut self, id: ObjectId) {
        if self.nevit_mut(id).advent.is_some()
            || self.nevit_advent_time(id) >= self.settings.nevit.advent_time
        {
            return;
        }
        let timer = self.schedule_after_ms(ADVENT_TASK_MS, TimerKind::NevitAdvent(id));
        self.nevit_mut(id).advent = Some(timer);
        let time = self.nevit_advent_time(id);
        self.send(id, &ExNevitAdventTimeChange::new(time, false));
    }

    /// Java `Nevit.AdventTask.run`.
    pub(crate) fn on_nevit_advent_timer(&mut self, id: ObjectId) {
        if !self.players.contains_key(&id) {
            return;
        }
        let advent_time = self.settings.nevit.advent_time;
        let time = self.nevit_advent_time(id).wrapping_add(ADVENT_TASK_SECS);
        self.set_advent_time(id, time);
        if time >= advent_time {
            self.set_advent_time(id, advent_time);
            self.nevit_stop_advent_task(id, true);
        } else {
            self.nevit_add_points(id, ADVENT_TASK_POINTS);
            if time % 60 == 0 {
                self.send(id, &ExNevitAdventTimeChange::new(time, false));
            }
        }
        self.nevit_stop_advent_task(id, false);
    }

    /// Java `Nevit.stopAdventTask(sendPacket)`.
    pub(crate) fn nevit_stop_advent_task(&mut self, id: ObjectId, send_packet: bool) {
        if let Some(timer) = self
            .rewards
            .nevit
            .players
            .get_mut(&id)
            .and_then(|n| n.advent.take())
        {
            self.timers.cancel(timer);
        }
        if send_packet {
            let time = self.nevit_advent_time(id);
            self.send(id, &ExNevitAdventTimeChange::new(time, true));
        }
    }

    // ------------------------------------------------------------------ Nevit.java: the blessing

    /// Java `Nevit.startNevitEffect(timeValue)`: starts the blessing for `time_value` seconds (on
    /// top of what is left of one that runs).
    pub(crate) fn start_nevit_effect(&mut self, id: ObjectId, time_value: i32) {
        let mut time = time_value;
        let running = self.nevit_effect_time(id);
        if running > 0 {
            self.stop_nevit_effect_task(id, false);
            time = time.wrapping_add(running);
        }

        let settings = &self.settings.nevit;
        if (settings.ignore_advent_time || self.nevit_advent_time(id) < settings.advent_time) && time > 0 {
            self.set_player_variable(id, NEVIT_B, time);
            self.send(id, &ExNevitAdventEffect { time_left: time });
            self.send(
                id,
                &SystemMessage::new(
                    SystemMessageId::TheAngelNevitHasBlessedYouFromAboveYouAreImbuedWithFullVitalityAsWellAsAVitalityReplenishingEffectAndShouldYouDieYouWillNotLoseExp,
                ),
            );
            // `startAbnormalVisualEffect(true, NAVIT_ADVENT)`: see the module comment.
            // Java's `time * 1000` is an int multiplication.
            let ms = u64::try_from(time.wrapping_mul(1000)).unwrap_or(0);
            let timer = self.schedule_after_ms(ms, TimerKind::NevitEffectEnd(id));
            let end_ms = self.clock_ms() + ms;
            self.nevit_mut(id).effect = Some(Running { timer, end_ms });
        }
    }

    /// Java `Nevit.NevitEffectEnd.run`.
    pub(crate) fn on_nevit_effect_end(&mut self, id: ObjectId) {
        if !self.players.contains_key(&id) {
            return;
        }
        if self.settings.nevit.ignore_advent_time {
            self.set_advent_time(id, 0);
        }
        self.remove_player_variable(id, NEVIT_B);
        self.send(id, &ExNevitAdventEffect { time_left: 0 });
        let points = self.nevit_adventure_points(id);
        self.send(id, &ExNevitAdventPointInfo { points });
        self.send(
            id,
            &SystemMessage::new(
                SystemMessageId::NevitSAdventBlessingHasEndedContinueYourJourneyAndYouWillSurelyMeetHisFavorAgainSometimeSoon,
            ),
        );
        // `stopAbnormalVisualEffect(true, NAVIT_ADVENT)`: see the module comment.
        self.stop_nevit_effect_task(id, false);
    }

    /// Java `Nevit.stopNevitEffectTask(saveTime)`: with `save_time` the seconds left are kept in
    /// the variable `nevit_b` (logout).
    pub(crate) fn stop_nevit_effect_task(&mut self, id: ObjectId, save_time: bool) {
        if self
            .rewards
            .nevit
            .players
            .get(&id)
            .is_none_or(|n| n.effect.is_none())
        {
            return;
        }
        if save_time {
            let time = self.nevit_effect_time(id);
            if time > 0 {
                self.set_player_variable(id, NEVIT_B, time);
            } else {
                self.remove_player_variable(id, NEVIT_B);
            }
        }
        if let Some(effect) = self.nevit_mut(id).effect.take() {
            self.timers.cancel(effect.timer);
        }
    }

    // ------------------------------------------------------------------ the hourglass

    /// Java `Player.loadRecommendations()`, the milliseconds part: what the database holds is what
    /// the last store wrote (`PlayerProfile::recom_time_left_ms`).
    fn stored_hourglass_ms(&self, id: ObjectId) -> i64 {
        self.players
            .get(&id)
            .map_or(0, |p| p.data.profile.recom_time_left_ms)
    }

    /// Java `Player.stopNevitHourglassTask`.
    pub(crate) fn nevit_stop_hourglass_task(&mut self, id: ObjectId) {
        if let Some(hourglass) = self.nevit_mut(id).hourglass.take()
            && let Some(timer) = hourglass.timer
        {
            self.timers.cancel(timer);
        }
    }

    /// Java `Player.startNevitHourglassTask`: starts the hourglass with the time the database
    /// holds, unless there is one, and tells the client.
    pub(crate) fn start_nevit_hourglass_task(&mut self, id: ObjectId) {
        if self.nevit_mut(id).hourglass.is_some() {
            return;
        }
        // Load data. `loadRecommendations` also sets the counters of the player from the row (the
        // daily reset relies on it: the row says 20 to give).
        if let Some(player) = self.players.get_mut(&id) {
            player.recom_have = player.data.profile.recom_have.clamp(0, 255);
            player.recom_left = player.data.profile.recom_left.clamp(0, 255);
        }
        let task_time = self.stored_hourglass_ms(id);
        if task_time > 0 {
            // If player have some timeleft, start bonus task.
            let ms = u64::try_from(task_time).unwrap_or(0);
            let timer = self.schedule_after_ms(ms, TimerKind::NevitHourglassEnd(id));
            let end_ms = self.clock_ms() + ms;
            self.nevit_mut(id).hourglass = Some(Hourglass {
                timer: Some(timer),
                end_ms,
            });
        }
        self.send_vote_system_info(id);
    }

    /// Java `RecoBonusTaskEnd.run`.
    pub(crate) fn on_nevit_hourglass_end(&mut self, id: ObjectId) {
        if !self.players.contains_key(&id) {
            return;
        }
        if let Some(hourglass) = self.nevit_mut(id).hourglass.as_mut() {
            hourglass.timer = None;
        }
        self.send_vote_system_info(id);
    }

    /// Java `Player.storeRecommendations(cancelRecoTask)`: the hourglass time the next store
    /// writes to `character_reco_bonus.time_left`. A paused hourglass writes back what the
    /// database holds.
    pub(crate) fn store_recommendations(&mut self, id: ObjectId, cancel_reco_task: bool) {
        let now = self.clock_ms();
        let mut task_end: i64 = 0;
        let mut cancelled = None;
        if let Some(nevit) = self.rewards.nevit.players.get_mut(&id)
            && let Some(hourglass) = nevit.hourglass
            && let Some(timer) = hourglass.timer
        {
            task_end = i64::try_from(hourglass.end_ms.saturating_sub(now)).unwrap_or(i64::MAX);
            if cancel_reco_task {
                cancelled = Some(timer);
                nevit.hourglass = None;
            }
        }
        if let Some(timer) = cancelled {
            self.timers.cancel(timer);
        }
        let paused = self.nevit_hourglass_paused(id);
        if let Some(player) = self.players.get_mut(&id) {
            let (rec_have, rec_left) = (player.recom_have, player.recom_left);
            let profile = &mut player.data.profile;
            profile.recom_have = rec_have;
            profile.recom_left = rec_left;
            profile.recom_time_left_ms = if paused {
                profile.recom_time_left_ms
            } else {
                task_end
            };
            profile.reco_store = true;
        }
    }

    // ------------------------------------------------------------------ enter, zone, leave

    /// Java `Player.checkRecoBonusTask` (the part that is not the recommendation timer): the
    /// hourglass starts and is stored.
    pub(crate) fn check_reco_bonus_task(&mut self, id: ObjectId) {
        if let Some(p) = self.players.get_mut(&id) {
            p.recom_have = p.data.profile.recom_have;
            p.recom_left = p.data.profile.recom_left;
            p.hourglass_ms = p.data.profile.recom_time_left_ms;
        }
        // Create bonus task.
        self.start_nevit_hourglass_task(id);
        // Create task to give new recommendations.
        self.start_reco_give_task(id);
        // Store new data.
        self.store_recommendations(id, false);
    }

    /// Java `PeaceZone.onEnter`, "Nevit Turn Off on Peace Zone": the advent time and the hourglass
    /// stop.
    pub(crate) fn nevit_on_peace_zone_enter(&mut self, id: ObjectId) {
        // NOTE: not for the zone a player logs in to. Java's source runs this block there too
        // (`Player.onPlayerEnter` -> `revalidateZone(true)`), but the recorded Java session
        // (`fixtures/captures/java-gs-session.jsonl`, a login inside a peace zone) has no
        // `ExNevitAdventTimeChange` from it, and its second `ExVoteSystemInfo` says "not paused"
        // (status 10). The capture wins until the difference is explained.
        if !self.settings.nevit_enabled || !self.players.get(&id).is_some_and(|p| p.entered_world) {
            return;
        }
        self.nevit_stop_advent_task(id, true);
        // Stop Nevit's Hourglass.
        self.store_recommendations(id, true);
        self.nevit_mut(id).paused = true;
    }

    /// The Nevit part of Java's `PlayerStat.addExpAndSp` before the bonuses: "Start Nevit's
    /// Hourglass".
    pub(crate) fn nevit_on_exp_gain_start_hourglass(&mut self, id: ObjectId, add_to_exp: f64) {
        if self.settings.nevit_enabled && add_to_exp > 0.0 && !self.is_inside_zone(id, ZoneId::Peace) {
            self.start_nevit_hourglass_task(id);
            // `getEffectList().getFirstEffect(EffectType.NEVITS_HOURGLASS) == null`.
            if !self.is_affected_by_effect_type(id, crate::effects::EffectType::NevitsHourglass) {
                self.nevit_mut(id).paused = false;
            }
        }
    }

    /// Java `PlayerStat.setPausedNevitHourglassStatus(value)`: the value, then `ExVoteSystemInfo`
    /// (the `NevitsHourglass` effect pauses the hourglass while it lasts).
    pub(crate) fn set_nevit_hourglass_paused(&mut self, id: ObjectId, value: bool) {
        self.nevit_mut(id).paused = value;
        self.send_vote_system_info(id);
    }

    /// The Nevit part of Java's `PlayerStat.addExpAndSp` after the bonuses.
    pub(crate) fn nevit_on_exp_gain_start_advent(&mut self, id: ObjectId, add_to_exp: f64) {
        if self.settings.nevit_enabled && add_to_exp > 0.0 && !self.is_inside_zone(id, ZoneId::Peace) {
            self.nevit_start_advent_task(id);
        }
    }

    /// Java `Player.deleteMe`: the logout listener of `Nevit` (the blessing keeps its time in
    /// `nevit_b`, the advent task stops), then `storeRecommendations(false)`, then the tasks are
    /// cancelled. Call before the final snapshot.
    pub(crate) fn nevit_on_leave(&mut self, id: ObjectId) {
        if self.settings.nevit_enabled {
            self.stop_nevit_effect_task(id, true);
            self.nevit_stop_advent_task(id, false);
            // Recommendations must be saved before task (timer) is canceled.
            self.store_recommendations(id, false);
        }
        // `stopAllTimers` -> `stopRecoGiveTask`.
        self.stop_reco_give_task(id);
        if let Some(nevit) = self.rewards.nevit.players.remove(&id) {
            for timer in [
                nevit.advent,
                nevit.effect.map(|e| e.timer),
                nevit.hourglass.and_then(|h| h.timer),
            ]
            .into_iter()
            .flatten()
            {
                self.timers.cancel(timer);
            }
        }
    }

    /// Java `Player.autoSave`: `storeRecommendations(false)`. Call before the snapshot.
    pub(crate) fn nevit_on_autosave(&mut self, id: ObjectId) {
        if self.settings.nevit_enabled {
            self.store_recommendations(id, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hourglass_bonus_follows_the_java_table() {
        // level 40-49 is row 4, 25 recommendations is column 2 (24 / 10).
        assert_eq!(hourglass_bonus(45, 25), 25);
        assert_eq!(hourglass_bonus(1, 1), 25);
        assert_eq!(hourglass_bonus(85, 100), 50);
        // More than 100 recommendations count as 100.
        assert_eq!(hourglass_bonus(85, 250), 50);
        assert_eq!(hourglass_bonus(10, 11), 33);
    }

    #[test]
    fn the_percent_of_the_bar_is_cut_to_a_whole_number() {
        assert_eq!(calc_percent(0, 7200), 0);
        assert_eq!(calc_percent(72, 7200), 1);
        assert_eq!(calc_percent(3240, 7200), 45);
        assert_eq!(calc_percent(3600, 7200), 50);
        assert_eq!(calc_percent(5400, 7200), 75);
        assert_eq!(calc_percent(7199, 7200), 99);
    }

    #[test]
    fn the_daily_boundary_is_half_past_six_local_time() {
        // 2026-10-19 12:00:00 UTC; in UTC the boundary is 06:30 the same day.
        let noon = 1_792_411_200_000;
        assert_eq!(nevit_reset_boundary_ms(noon, 0), noon - (5 * 60 + 30) * 60_000);
        // At UTC+2 the local day is already 14:00 and the boundary is 06:30 local = 04:30 UTC.
        assert_eq!(
            nevit_reset_boundary_ms(noon, 2 * 3_600_000),
            noon - (7 * 60 + 30) * 60_000
        );
        // Just after local midnight the boundary is the one of the new day, later than now.
        let after_midnight = noon - 12 * 3_600_000 + 60_000;
        assert!(nevit_reset_boundary_ms(after_midnight, 0) > after_midnight);
    }

    #[test]
    fn the_default_settings_are_the_java_defaults() {
        let s = NevitSettings::default();
        assert_eq!(
            (s.max_points, s.bonus_effect_time, s.advent_time),
            (7200, 180, 14_400)
        );
        assert!(!s.ignore_advent_time);
    }
}
