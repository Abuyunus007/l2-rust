//! The recommendation system: the votes (`RequestVoteNew`), the recommendations a player earns by
//! staying online (`RecoGiveTask`) and the daily reset at 6:30 (`DailyResetManager`: Nevit's
//! advent time and the recommendations).
// Java: network/clientpackets/RequestVoteNew.java
// Java: entity/actor/tasks/player/RecoGiveTask.java
// Java: entity/actor/Player.java (giveRecom, incRecomHave, decRecomLeft, setRecomHave,
//       setRecomLeft, storeRecommendationValues, checkRecoBonusTask, stopRecoGiveTask)
// Java: managers/DailyResetManager.java (the schedule, onReset, resetNevit, resetRecommends)
// Java: commons/time/TimeUtil.java (getNextTime)
//
// # State
// | Java | Here |
// |---|---|
// | `Player._recomHave` / `_recomLeft` | `Player::recom_have` / `recom_left` (live, 0 to 255) |
// | the row of `character_reco_bonus` | `PlayerProfile::recom_have` / `recom_left` / `recom_time_left_ms`, which mirror what the database holds |
// | `Player._recoGiveTask`, `_recoTwoHoursGiven` | [`RecoWorld::give`], `TimerKind::RecoGive` |
// | the daily `scheduleAtFixedRate` | [`RecoWorld::daily`], `TimerKind::DailyReset` |
//
// # Database (D-39)
// The world never waits for the database. Java writes `character_reco_bonus` itself, so the
// world hands the same statements to the persistence task as commands, in the order Java runs
// them: `PersistCommand::RecoValues` (a vote, `RecoGiveTask`), `PersistCommand::ResetRecommends`
// (the two `UPDATE`s of the daily reset) and `PersistCommand::PlayerVariables` (`storeMe`).
// `ResetRecommends` is only sent with the Rust-only setting `DailyResetDatabaseWrites`
// (`SideBySide.ini`, default False): the Java game server on the same database runs it itself.
// The mirror in the profile is updated with each of them, because Java reads the row back
// (`loadRecommendations` at `startNevitHourglassTask`).
//
// # What is left out
// - `DailyResetManager`'s start-up decision "run now when the last reset (the global variable
//   `DAILY_TASK_RESET`) is older than today's 6:30": the world has no global variables, and
//   writing that variable from here would be wiped by the Java game server's `storeMe`
//   (`DELETE FROM global_variables`). The Rust server only resets at the next 6:30.
// - `clanLeaderApply` on Wednesdays (clans), `resetDailySkills` (the Wondrous Cubic reuse), the
//   `ON_DAILY_RESET` event and `getAccountVariables().storeMe()`.

use std::collections::HashMap;

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use tracing::info;

use crate::outbox::PersistCommand;
use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::{World, unix_ms_now};

/// `scheduleAtFixedRate(new RecoGiveTask(this), 7200000, 3600000)`: the first run after 2 hours.
const RECO_GIVE_FIRST_MS: u64 = 7_200_000;
/// ... and one an hour after that.
const RECO_GIVE_PERIOD_MS: u64 = 3_600_000;
/// `ThreadPool.scheduleAtFixedRate(this::onReset, startDelay, 86400000)`.
const DAILY_PERIOD_MS: u64 = 86_400_000;
/// `TimeUtil.getNextTime(6, 30)`: 06:30 in milliseconds after local midnight.
const DAILY_RESET_OF_DAY_MS: i64 = (6 * 60 + 30) * 60_000;
/// The recommendations `resetRecommends` takes away (and the ones everybody may give again).
const RESET_REC_HAVE: i32 = 20;
const RESET_REC_LEFT: i32 = 20;
/// `ps.setInt(2, 3600000); // Timer = 1 hour`.
const RESET_TIME_LEFT_MS: i64 = 3_600_000;
/// `Player.setRecomHave` / `setRecomLeft`: `Math.clamp(value, 0, 255)`.
const RECOM_MAX: i32 = 255;

/// Java `Player._recoGiveTask` and `_recoTwoHoursGiven`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RecoGive {
    timer: TimerId,
    /// The tick the timer is due: a fixed rate counts from it, not from the moment it ran.
    due_tick: u64,
    two_hours_given: bool,
}

/// The recommendation tasks of the world (a field of [`ExpWorld`](super::exp::ExpWorld)).
#[derive(Debug, Default)]
pub struct RecoWorld {
    give: HashMap<ObjectId, RecoGive>,
    /// The daily reset: its timer and the tick it is due.
    daily: Option<(TimerId, u64)>,
}

/// Java `TimeUtil.getNextTime(6, 30)`: the next 06:30:00.000 in the server's zone at or after
/// `now_ms`, as Unix milliseconds. `offset_at(utc_ms)` is the zone's offset from UTC at that
/// moment (D-48), so a daylight saving change between now and then is taken into account like
/// `Calendar` does.
pub fn next_daily_reset_ms(now_ms: i64, offset_at: impl Fn(i64) -> i64) -> i64 {
    const DAY_MS: i64 = 86_400_000;
    let local_now = now_ms + offset_at(now_ms);
    let day_start = local_now - local_now.rem_euclid(DAY_MS);
    let to_utc = |local: i64| {
        let guess = local - offset_at(now_ms);
        local - offset_at(guess)
    };
    let today = to_utc(day_start + DAILY_RESET_OF_DAY_MS);
    // "If the target time has already passed today, schedule for the next day."
    if today < now_ms {
        to_utc(day_start + DAY_MS + DAILY_RESET_OF_DAY_MS)
    } else {
        today
    }
}

/// What `RecoGiveTask` gives: 10 the first time (after 2 hours online), then 1.
fn reco_to_give(two_hours_given: bool) -> i32 {
    if two_hours_given { 1 } else { 10 }
}

/// What the database holds of `rec_have` after the daily `UPDATE`s.
fn reset_rec_have(rec_have: i32) -> i32 {
    // `rec_have <= 20` -> 0, else `GREATEST(rec_have-20,0)`.
    (rec_have - RESET_REC_HAVE).max(0)
}

impl World {
    /// Whether the player has a `RecoGiveTask` (tests).
    pub fn has_reco_give_task(&self, id: ObjectId) -> bool {
        self.rewards.reco.give.contains_key(&id)
    }

    /// Whether the daily reset is scheduled (tests).
    pub fn has_daily_reset_task(&self) -> bool {
        self.rewards.reco.daily.is_some()
    }

    // ------------------------------------------------------------------ the counters

    /// Java `Player.setRecomHave(value)`.
    pub(crate) fn set_recom_have(&mut self, id: ObjectId, value: i32) {
        if let Some(player) = self.players.get_mut(&id) {
            player.recom_have = value.clamp(0, RECOM_MAX);
        }
    }

    /// Java `Player.setRecomLeft(value)`.
    pub(crate) fn set_recom_left(&mut self, id: ObjectId, value: i32) {
        if let Some(player) = self.players.get_mut(&id) {
            player.recom_left = value.clamp(0, RECOM_MAX);
        }
    }

    /// Java `Player.storeRecommendationValues()`: the counters now, "without tapping into Nevit
    /// Hourglass task".
    // NOTE(java-quirk): the statement is a `REPLACE` of `(charId,rec_have,rec_left)`, which drops
    // the row, so `time_left` is the column default 0 until the next `storeRecommendations`
    // writes the hourglass time again. The mirror says so too (a paused hourglass writes back
    // what the database holds).
    pub(crate) fn store_recommendation_values(&mut self, id: ObjectId) {
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        let (rec_have, rec_left) = (player.recom_have, player.recom_left);
        let profile = &mut player.data.profile;
        profile.recom_have = rec_have;
        profile.recom_left = rec_left;
        profile.recom_time_left_ms = 0;
        self.outbox.persist(PersistCommand::RecoValues {
            owner: id,
            rec_have,
            rec_left,
        });
    }

    /// Java `PlayerVariables.storeMe()`: the variables the world changed are written now. Java
    /// writes only when something changed.
    fn store_player_variables(&mut self, id: ObjectId) {
        let Some(player) = self.players.get(&id) else {
            return;
        };
        let profile = &player.data.profile;
        let upserts: Vec<(String, String)> = profile
            .variables_changed
            .iter()
            .filter_map(|name| {
                profile
                    .variables
                    .get(name)
                    .map(|value| (name.clone(), value.clone()))
            })
            .collect();
        let removed: Vec<String> = profile.variables_removed.iter().cloned().collect();
        if upserts.is_empty() && removed.is_empty() {
            return;
        }
        self.outbox.persist(PersistCommand::PlayerVariables {
            owner: id,
            upserts,
            removed,
        });
    }

    // ------------------------------------------------------------------ RecoGiveTask

    /// Java `Player.checkRecoBonusTask`: `scheduleAtFixedRate(new RecoGiveTask(this), 7200000,
    /// 3600000)`.
    pub(crate) fn start_reco_give_task(&mut self, id: ObjectId) {
        let due = self.tick + TimerWheel::ms_to_ticks(RECO_GIVE_FIRST_MS);
        let timer = self.timers.schedule_at(due, TimerKind::RecoGive(id));
        let entry = RecoGive {
            timer,
            due_tick: due,
            two_hours_given: false,
        };
        if let Some(old) = self.rewards.reco.give.insert(id, entry) {
            self.timers.cancel(old.timer);
        }
    }

    /// Java `Player.stopRecoGiveTask`.
    pub(crate) fn stop_reco_give_task(&mut self, id: ObjectId) {
        if let Some(task) = self.rewards.reco.give.remove(&id) {
            self.timers.cancel(task.timer);
        }
    }

    /// Java `RecoGiveTask.run`.
    pub(crate) fn on_reco_give_timer(&mut self, id: ObjectId) {
        let Some(task) = self.rewards.reco.give.get(&id).copied() else {
            return;
        };
        if !self.players.contains_key(&id) {
            self.rewards.reco.give.remove(&id);
            return;
        }
        // A fixed rate: the next run is an hour after this one was due.
        let next_due = task.due_tick + TimerWheel::ms_to_ticks(RECO_GIVE_PERIOD_MS);
        let timer = self.timers.schedule_at(next_due, TimerKind::RecoGive(id));
        self.rewards.reco.give.insert(
            id,
            RecoGive {
                timer,
                due_tick: next_due,
                two_hours_given: true,
            },
        );

        // 10 recommendations to give out after 2 hours of being logged in.
        // 1 more recommendation to give out every hour after that.
        let to_give = reco_to_give(task.two_hours_given);
        let left = self.players.get(&id).map_or(0, |p| p.recom_left);
        self.set_recom_left(id, left.wrapping_add(to_give));

        self.send(
            id,
            &SystemMessage::new(SystemMessageId::YouObtainedS1RecommendationS)
                .with(SystemMessageParam::IntNumber(to_give)),
        );
        self.send_user_info(id);

        // Store player recommendations to avoid reseting them with Nevit peace zone check.
        self.store_recommendation_values(id);
    }

    // ------------------------------------------------------------------ RequestVoteNew

    /// Java `RequestVoteNew.runImpl`.
    pub(crate) fn on_request_vote_new(&mut self, id: ObjectId, target_id: i32) {
        let Some(player) = self.players.get(&id) else {
            return;
        };
        let Some(target) = player.target else {
            self.send(id, &SystemMessage::new(SystemMessageId::SelectTarget));
            return;
        };
        if !self.players.contains_key(&target) {
            self.send(id, &SystemMessage::new(SystemMessageId::ThatIsAnIncorrectTarget));
            return;
        }
        if target.0 != target_id {
            return;
        }
        if target == id {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouCannotRecommendYourself),
            );
            return;
        }
        if player.recom_left <= 0 {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouAreOutOfRecommendationsTryAgainLater),
            );
            return;
        }
        let target_have = self.players.get(&target).map_or(0, |p| p.recom_have);
        if target_have >= RECOM_MAX {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YourSelectedTargetCanNoLongerReceiveARecommendation),
            );
            return;
        }

        // `player.giveRecom(target)`: `target.incRecomHave()` and `decRecomLeft()`.
        if let Some(p) = self.players.get_mut(&target)
            && p.recom_have < RECOM_MAX
        {
            p.recom_have += 1;
        }
        if let Some(p) = self.players.get_mut(&id)
            && p.recom_left > 0
        {
            p.recom_left -= 1;
        }

        let name_of = |world: &World, who: ObjectId| {
            world
                .players
                .get(&who)
                .map(|p| p.data.name.clone())
                .unwrap_or_default()
        };
        let left = self.players.get(&id).map_or(0, |p| p.recom_left);
        let target_name = name_of(self, target);
        self.send(
            id,
            &SystemMessage::new(SystemMessageId::YouHaveRecommendedC1YouHaveS2RecommendationsLeft)
                .with(SystemMessageParam::PlayerName(target_name))
                .with(SystemMessageParam::IntNumber(left)),
        );
        let player_name = name_of(self, id);
        self.send(
            target,
            &SystemMessage::new(SystemMessageId::YouHaveBeenRecommendedByC1)
                .with(SystemMessageParam::PlayerName(player_name)),
        );

        // NOTE(java-quirk): only the target's `UserInfo` is refreshed; the player keeps seeing the
        // old number of recommendations left in it until the next `UserInfo`.
        self.broadcast_user_info(target);

        if self.settings.nevit_enabled {
            self.send_vote_system_info(id);
            self.send_vote_system_info(target);
        }

        // Store player recommendations to avoid reseting them with Nevit peace zone check.
        self.store_recommendation_values(id);
        self.store_recommendation_values(target);
    }

    // ------------------------------------------------------------------ DailyResetManager

    /// Java `DailyResetManager`'s constructor, the schedule: the first run at the next 06:30 in
    /// the server's zone, then every 24 hours. Call once at startup; the world never starts
    /// it by itself, so tests stay independent of the wall clock.
    pub fn start_daily_reset_task(&mut self) {
        let now = unix_ms_now();
        self.start_daily_reset_task_at(now, l2_util::time::local_offset_millis);
    }

    /// [`Self::start_daily_reset_task`] for a given clock (`offset_at` as in
    /// [`next_daily_reset_ms`]).
    pub fn start_daily_reset_task_at(&mut self, now_ms: i64, offset_at: impl Fn(i64) -> i64) {
        let next = next_daily_reset_ms(now_ms, offset_at);
        info!("DailyResetManager: Next schedule at {next} (Unix ms).");
        // `Math.max(0, nextResetTime - currentTime)`
        let delay = u64::try_from(next.saturating_sub(now_ms)).unwrap_or(0);
        let due = self.tick + TimerWheel::ms_to_ticks(delay);
        let timer = self.timers.schedule_at(due, TimerKind::DailyReset);
        if let Some((old, _)) = self.rewards.reco.daily.replace((timer, due)) {
            self.timers.cancel(old);
        }
    }

    /// The timer of `scheduleAtFixedRate(this::onReset, startDelay, 86400000)`.
    pub(crate) fn on_daily_reset_timer(&mut self) {
        // NOTE: a fixed period of 24 hours, like Java: after a daylight saving change the run
        // moves by an hour.
        if let Some((_, due)) = self.rewards.reco.daily {
            let next_due = due + TimerWheel::ms_to_ticks(DAILY_PERIOD_MS);
            let timer = self.timers.schedule_at(next_due, TimerKind::DailyReset);
            self.rewards.reco.daily = Some((timer, next_due));
        }
        self.daily_reset();
    }

    /// Java `DailyResetManager.onReset`, the parts that are ported (see the module comment).
    pub fn daily_reset(&mut self) {
        info!("Starting reset of daily tasks...");
        // Wednesday weekly tasks (`Calendar.getInstance()`: the local day; 1970-01-01 was a
        // Thursday).
        let local_day = l2_util::time::local_now_millis().div_euclid(86_400_000);
        if (local_day + 4).rem_euclid(7) == 3 {
            self.clan_leader_apply();
        }
        // Daily tasks.
        self.reset_nevit();
        self.reset_recommends();
        // Store player variables.
        for id in self.sorted_player_ids() {
            self.store_player_variables(id);
        }
        info!("Daily tasks reset completed.");
    }

    /// The ids of the players in the world, in a fixed order (Java walks a hash map).
    fn sorted_player_ids(&self) -> Vec<ObjectId> {
        let mut ids: Vec<ObjectId> = self.players.keys().copied().collect();
        ids.sort();
        ids
    }

    /// Java `DailyResetManager.resetNevit`: the advent time of everybody online starts again.
    fn reset_nevit(&mut self) {
        if !self.settings.nevit_enabled {
            return;
        }
        for id in self.sorted_player_ids() {
            self.nevit_reset_advent_time(id);
        }
    }

    /// Java `DailyResetManager.resetRecommends`.
    fn reset_recommends(&mut self) {
        // The two `UPDATE`s run in the database first. They touch every row, so with the Java game
        // server on the same database (`DailyResetDatabaseWrites` off, D-39) they are left to it:
        // run twice they would take 40 recommendations away. The rows of the players in the world
        // change either way, which the mirror in their profile follows (their own rows are
        // written by the normal store).
        if self.settings.daily_reset_database_writes {
            self.outbox.persist(PersistCommand::ResetRecommends);
        }
        let ids = self.sorted_player_ids();
        for id in &ids {
            if let Some(player) = self.players.get_mut(id) {
                let profile = &mut player.data.profile;
                profile.recom_have = reset_rec_have(profile.recom_have);
                profile.recom_left = RESET_REC_LEFT;
                profile.recom_time_left_ms = RESET_TIME_LEFT_MS;
            }
        }

        for &id in &ids {
            // NOTE(java-quirk): the players in the world get 0 to give, not 20 like their row.
            // Without Nevit nothing reads the row back, so they must log in again to recommend.
            self.set_recom_left(id, 0);
            let have = self.players.get(&id).map_or(0, |p| p.recom_have);
            self.set_recom_have(id, have - RESET_REC_HAVE);
            self.send_vote_system_info(id);
            self.broadcast_user_info(id);
        }

        // Refresh reco bonus for online players: the hourglass starts again with the row
        // (`loadRecommendations` also reads the counters back, so these get their 20).
        if self.settings.nevit_enabled {
            for &id in &ids {
                self.nevit_stop_hourglass_task(id);
                self.start_nevit_hourglass_task(id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: i64 = 3_600_000;
    /// 2026-10-19 00:00:00 UTC.
    const MIDNIGHT: i64 = 1_792_368_000_000;
    const SIX_THIRTY: i64 = 6 * HOUR + HOUR / 2;

    #[test]
    fn the_next_reset_is_half_past_six_local_time() {
        // Before 6:30 UTC: the same day.
        assert_eq!(next_daily_reset_ms(MIDNIGHT + HOUR, |_| 0), MIDNIGHT + SIX_THIRTY);
        // After 6:30 UTC: the next day.
        assert_eq!(
            next_daily_reset_ms(MIDNIGHT + 7 * HOUR, |_| 0),
            MIDNIGHT + 24 * HOUR + SIX_THIRTY
        );
        // Exactly 6:30:00.000 is not "before" now: today, delay 0.
        assert_eq!(
            next_daily_reset_ms(MIDNIGHT + SIX_THIRTY, |_| 0),
            MIDNIGHT + SIX_THIRTY
        );
        // One millisecond later it is tomorrow.
        assert_eq!(
            next_daily_reset_ms(MIDNIGHT + SIX_THIRTY + 1, |_| 0),
            MIDNIGHT + 24 * HOUR + SIX_THIRTY
        );
    }

    #[test]
    fn the_zone_moves_the_reset() {
        // UTC+2: 6:30 local is 4:30 UTC. At 03:00 UTC (05:00 local) the reset is 1.5 h away.
        let plus_two = |_| 2 * HOUR;
        let now = MIDNIGHT + 3 * HOUR;
        assert_eq!(next_daily_reset_ms(now, plus_two), MIDNIGHT + 4 * HOUR + HOUR / 2);
        // At 05:00 UTC (07:00 local) it is tomorrow's.
        assert_eq!(
            next_daily_reset_ms(MIDNIGHT + 5 * HOUR, plus_two),
            MIDNIGHT + 24 * HOUR + 4 * HOUR + HOUR / 2
        );
        // UTC-5 (local day started at 05:00 UTC): 6:30 local is 11:30 UTC. At 23:00 UTC on the
        // 19th it is 18:00 local, so the next reset is 11:30 UTC on the 20th.
        let minus_five = |_| -5 * HOUR;
        assert_eq!(
            next_daily_reset_ms(MIDNIGHT + 23 * HOUR, minus_five),
            MIDNIGHT + 24 * HOUR + 11 * HOUR + HOUR / 2
        );
    }

    #[test]
    fn a_daylight_saving_change_before_the_reset_is_followed() {
        // The offset is UTC+1 until 03:00 UTC, UTC+2 after it (clocks go forward at 03:00 UTC).
        // At 00:30 UTC (01:30 local) the next 6:30 local is at UTC+2: 04:30 UTC.
        let change = MIDNIGHT + 3 * HOUR;
        let offset = move |utc: i64| if utc < change { HOUR } else { 2 * HOUR };
        assert_eq!(
            next_daily_reset_ms(MIDNIGHT + HOUR / 2, offset),
            MIDNIGHT + 4 * HOUR + HOUR / 2
        );
    }

    #[test]
    fn the_gift_is_ten_the_first_time_then_one() {
        assert_eq!(reco_to_give(false), 10);
        assert_eq!(reco_to_give(true), 1);
    }

    #[test]
    fn the_reset_takes_20_but_not_below_0() {
        assert_eq!([0, 5, 20, 21, 45, 255].map(reset_rec_have), [0, 0, 0, 1, 25, 235]);
    }
}
