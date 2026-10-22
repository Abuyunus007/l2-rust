//! The fishing championship: the five longest fish of the week, the ranking pages and the prizes.
// Java: managers/FishingChampionshipManager.java
// Java: entity/actor/instance/Fisherman.java (onBypassFeedback: FishingChampionship,
//       FishingReward)
// Java: network/clientpackets/RequestExFishRanking.java
// Java: network/GameServer.java and Shutdown.java (the manager is built and saved only with
//       `AltFishChampionshipEnabled`)
//
// # Rows (D-39, D-49)
// The manager reads `fishing_championship` and the global variable `fishChampionshipEnd` at start
// and writes them (`DELETE FROM fishing_championship`, one `INSERT` per fisher) when a period ends
// and at shutdown. The rows are one table for the whole server, and the Java game server rewrites
// them from its own memory, so while it shares the database this server only reads them and keeps
// its changes in memory; `FishingChampionshipDatabaseWrites = True` in `config/SideBySide.ini`
// (`FishingSettings::championship_database_writes`) turns the writes on.

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use tracing::{debug, info};

use super::FishingTimer;
use crate::outbox::PersistCommand;
use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::{World, unix_ms_now};

const HTML_DIR: &str = "data/html/fisherman/championship/";
/// `ThreadPool.schedule(new needRefresh(), 60000)`.
const REFRESH_DELAY_MS: u64 = 60_000;
const DAY_MS: i64 = 86_400_000;
const HOUR_MS: i64 = 3_600_000;

/// One row of `fishing_championship` (Java's private `Fisher`).
#[derive(Debug, Clone, PartialEq)]
pub struct ChampionshipRow {
    /// `player_name`.
    pub name: String,
    /// `fish_length`.
    pub length: f64,
    /// `rewarded`: 0 a fish of the running week, 1 a winner of the last week who has not taken the
    /// prize, 2 a winner who has.
    pub rewarded: i32,
}

/// What `restoreData` reads and `shutdown` writes: the global variable `fishChampionshipEnd` and
/// the rows (the winners first, then the running week).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChampionshipData {
    /// `fishChampionshipEnd`, Unix milliseconds (0 when the variable is not there).
    pub end_date: i64,
    pub rows: Vec<ChampionshipRow>,
}

/// The state of `FishingChampionshipManager`.
#[derive(Debug)]
pub(crate) struct Championship {
    /// The manager exists (Java builds it only with `AltFishChampionshipEnabled`).
    started: bool,
    end_date: i64,
    players_name: Vec<String>,
    fish_length: Vec<String>,
    win_players_name: Vec<String>,
    win_fish_length: Vec<String>,
    tmp_players: Vec<ChampionshipRow>,
    win_players: Vec<ChampionshipRow>,
    min_fish_length: f64,
    need_refresh: bool,
    finish_timer: Option<TimerId>,
}

impl Default for Championship {
    fn default() -> Self {
        Self {
            started: false,
            end_date: 0,
            players_name: Vec::new(),
            fish_length: Vec::new(),
            win_players_name: Vec::new(),
            win_fish_length: Vec::new(),
            tmp_players: Vec::new(),
            win_players: Vec::new(),
            min_fish_length: 0.0,
            // Java: `protected boolean _needRefresh = true;`
            need_refresh: true,
            finish_timer: None,
        }
    }
}

/// Java `String.valueOf(double)` for the lengths (60 to 93): `75.0`, `75.342`.
fn double_text(value: f64) -> String {
    format!("{value:?}")
}

/// Java's two bubble sorts (`refreshResult`, `refreshWinResult`): longest first. Bubble sort with
/// a strict `<` is stable, so a stable sort gives the same order, ties included.
fn sort_longest_first(rows: &mut [ChampionshipRow]) {
    rows.sort_by(|a, b| b.length.partial_cmp(&a.length).unwrap_or(std::cmp::Ordering::Equal));
}

/// Java `setEndOfChamp`: from `end_date` set minute and second to 0, add 6 days, move to Tuesday
/// of that week and set the hour to 19, all in the server's zone.
///
/// `Calendar.getInstance()` uses the default locale's first day of the week; this takes Sunday
/// (`Calendar.SUNDAY`, the en_US week) as the first day, so `DAY_OF_WEEK = 3` is the Tuesday
/// after the Sunday on or before day + 6. The milliseconds of the second are kept, as Java keeps
/// `MILLISECOND`.
pub(crate) fn end_of_champ(end_date: i64, offset_at: impl Fn(i64) -> i64) -> i64 {
    let offset = offset_at(end_date);
    let local = end_date + offset;
    let day = local.div_euclid(DAY_MS) + 6;
    let millis = local.rem_euclid(1000);
    // 1970-01-01 was a Thursday: 0 = Sunday ... 6 = Saturday.
    let weekday = (day + 4).rem_euclid(7);
    let tuesday = day + (2 - weekday);
    let local_result = tuesday * DAY_MS + 19 * HOUR_MS + millis;
    local_result - offset_at(local_result - offset)
}

/// `(_enddate - System.currentTimeMillis()) / 60000`: the minutes left.
fn minutes_remaining(end_date: i64, now_ms: i64) -> i64 {
    (end_date - now_ms) / 60_000
}

impl World {
    /// Java `new FishingChampionshipManager()` at server start: `restoreData`, the sorted names,
    /// the minimum length, and the end of the period, which runs at once when it is over. Does
    /// nothing without `AltFishChampionshipEnabled`, like `GameServer`.
    pub fn load_fishing_championship(&mut self, data: ChampionshipData) {
        self.load_fishing_championship_at(data, unix_ms_now(), l2_util::time::local_offset_millis);
    }

    /// [`Self::load_fishing_championship`] for a given clock.
    pub fn load_fishing_championship_at(
        &mut self,
        data: ChampionshipData,
        now_ms: i64,
        offset_at: impl Fn(i64) -> i64,
    ) {
        if !self.settings.fishing.championship_enabled {
            return;
        }
        let championship = &mut self.fishing.championship;
        *championship = Championship {
            started: true,
            end_date: data.end_date,
            ..Championship::default()
        };
        for row in data.rows {
            if row.rewarded == 0 {
                championship.tmp_players.push(ChampionshipRow { rewarded: 0, ..row });
            } else if row.rewarded > 0 {
                championship.win_players.push(row);
            }
        }
        self.refresh_win_result();
        self.recalculate_min_length();

        if self.fishing.championship.end_date <= now_ms {
            self.fishing.championship.end_date = now_ms;
            self.championship_finish(now_ms, &offset_at);
        } else {
            let delay = u64::try_from(self.fishing.championship.end_date - now_ms).unwrap_or(0);
            self.schedule_championship_finish(delay);
        }
    }

    fn schedule_championship_finish(&mut self, delay_ms: u64) {
        let due = self.tick + TimerWheel::ms_to_ticks(delay_ms);
        let timer = self
            .timers
            .schedule_at(due, TimerKind::Fishing(FishingTimer::ChampionshipFinish));
        if let Some(old) = self.fishing.championship.finish_timer.replace(timer) {
            self.timers.cancel(old);
        }
    }

    /// Java `finishChamp.run()`: the running week becomes the winners and a new period starts.
    pub(crate) fn on_championship_finish_timer(&mut self) {
        if self.fishing.championship.started {
            self.championship_finish(unix_ms_now(), &l2_util::time::local_offset_millis);
        }
    }

    fn championship_finish(&mut self, now_ms: i64, offset_at: &dyn Fn(i64) -> i64) {
        let championship = &mut self.fishing.championship;
        championship.win_players.clear();
        let finished = std::mem::take(&mut championship.tmp_players);
        for mut fisher in finished {
            fisher.rewarded = 1;
            championship.win_players.push(fisher);
        }

        self.refresh_win_result();
        let end = end_of_champ(self.fishing.championship.end_date, offset_at);
        self.fishing.championship.end_date = end;
        self.championship_shutdown();

        info!("FishingChampionshipManager : new event period start.");
        let delay = u64::try_from(end - now_ms).unwrap_or(0);
        self.schedule_championship_finish(delay);
    }

    /// Java `FishingChampionshipManager.shutdown()`: `fishChampionshipEnd` and the rows, winners
    /// first. Called when a period ends and when the server stops; writes only with
    /// `FishingChampionshipDatabaseWrites` (D-49).
    pub(crate) fn championship_shutdown(&mut self) {
        let championship = &self.fishing.championship;
        if !championship.started || !self.settings.fishing.championship_database_writes {
            return;
        }
        let mut rows = championship.win_players.clone();
        rows.extend(
            championship
                .tmp_players
                .iter()
                .map(|f| ChampionshipRow { rewarded: 0, ..f.clone() }),
        );
        self.outbox
            .persist(PersistCommand::FishingChampionship(Box::new(ChampionshipData {
                end_date: championship.end_date,
                rows,
            })));
    }

    /// Java `needRefresh.run()`.
    pub(crate) fn on_championship_refresh_timer(&mut self) {
        self.fishing.championship.need_refresh = true;
    }

    /// The rows as they are (for the tests and the admin tools).
    pub fn fishing_championship_rows(&self) -> (Vec<ChampionshipRow>, Vec<ChampionshipRow>) {
        let c = &self.fishing.championship;
        (c.tmp_players.clone(), c.win_players.clone())
    }

    /// `_enddate` of the championship.
    pub fn fishing_championship_end(&self) -> i64 {
        self.fishing.championship.end_date
    }

    /// Java `recalculateMinLength`.
    fn recalculate_min_length(&mut self) {
        let championship = &mut self.fishing.championship;
        let mut min = 99_999.0;
        for fisher in &championship.tmp_players {
            if fisher.length < min {
                min = fisher.length;
            }
        }
        championship.min_fish_length = min;
    }

    /// Java `refreshResult`.
    fn refresh_result(&mut self) {
        let championship = &mut self.fishing.championship;
        championship.need_refresh = false;
        championship.players_name.clear();
        championship.fish_length.clear();
        sort_longest_first(&mut championship.tmp_players);
        championship.players_name = championship.tmp_players.iter().map(|f| f.name.clone()).collect();
        championship.fish_length = championship
            .tmp_players
            .iter()
            .map(|f| double_text(f.length))
            .collect();
    }

    /// Java `refreshWinResult`.
    fn refresh_win_result(&mut self) {
        let championship = &mut self.fishing.championship;
        sort_longest_first(&mut championship.win_players);
        championship.win_players_name = championship.win_players.iter().map(|f| f.name.clone()).collect();
        championship.win_fish_length = championship
            .win_players
            .iter()
            .map(|f| double_text(f.length))
            .collect();
    }

    /// Java `newFish(player, lureId)`: a fish was caught; if it is long enough it joins the
    /// ranking.
    pub(crate) fn championship_new_fish(&mut self, id: ObjectId, lure_id: i32) {
        if !self.settings.fishing.championship_enabled {
            return;
        }
        let Some(name) = self.players.get(&id).map(|p| p.data.name.clone()) else {
            return;
        };
        let mut len = f64::from(self.rng().get(60, 89)) + (f64::from(self.rng().get(0, 1000)) / 1000.0);
        if (8484..=8486).contains(&lure_id) {
            len += f64::from(self.rng().get(0, 3000)) / 1000.0;
        }

        self.send(
            id,
            &SystemMessage::new(SystemMessageId::YouCaughtAFishS1InLength)
                .with(SystemMessageParam::Text(double_text(len))),
        );

        let registered = SystemMessage::new(SystemMessageId::BecauseOfTheSizeOfFishCaughtYouWillBeRegisteredInTheRanking);
        let championship = &mut self.fishing.championship;
        let existing = championship
            .tmp_players
            .iter()
            .position(|f| f.name.eq_ignore_ascii_case(&name));
        if championship.tmp_players.len() < 5 {
            if let Some(index) = existing {
                if championship.tmp_players[index].length < len {
                    championship.tmp_players[index].length = len;
                    self.send(id, &registered);
                    self.recalculate_min_length();
                }
                return;
            }
            championship.tmp_players.push(ChampionshipRow {
                name,
                length: len,
                rewarded: 0,
            });
            self.send(id, &registered);
            self.recalculate_min_length();
        } else if championship.min_fish_length < len {
            if let Some(index) = existing {
                if championship.tmp_players[index].length < len {
                    championship.tmp_players[index].length = len;
                    self.send(id, &registered);
                    self.recalculate_min_length();
                }
                return;
            }

            // The shortest fish goes (the first of equal ones).
            let mut min_index = None;
            let mut min_len = 99_999.0;
            for (index, fisher) in championship.tmp_players.iter().enumerate() {
                if fisher.length < min_len {
                    min_index = Some(index);
                    min_len = fisher.length;
                }
            }
            if let Some(index) = min_index {
                championship.tmp_players.remove(index);
            }
            championship.tmp_players.push(ChampionshipRow {
                name,
                length: len,
                rewarded: 0,
            });
            self.send(id, &registered);
            self.recalculate_min_length();
        }
    }

    /// Java `getWinnerName(par)` and the other three getters: the name or length at place `par`
    /// (1 based) of a list, else `none`.
    fn place(list: &[String], par: usize, none: &str) -> String {
        if par >= 1 && list.len() >= par {
            list[par - 1].clone()
        } else {
            none.to_owned()
        }
    }

    /// The five table rows of the ranking pages. Java builds them on a `String` that starts as
    /// `null`.
    // NOTE(java-quirk): `String str = null; str += "<tr>..."` makes the text start with "null", so
    // the page shows the word before the table.
    fn championship_table(&self, names: &[String], lengths: &[String], none_length: &str) -> String {
        let mut table = String::from("null");
        for x in 1..=5 {
            table.push_str(&format!("<tr><td width=70 align=center>{x}</td>"));
            table.push_str(&format!(
                "<td width=110 align=center>{}</td>",
                Self::place(names, x, "None")
            ));
            table.push_str(&format!(
                "<td width=80 align=center>{}</td></tr>",
                Self::place(lengths, x, none_length)
            ));
        }
        table
    }

    /// The `%prize...%` replacements the two ranking pages share.
    fn replace_prizes(&self, html: &mut String, table: &str) {
        let settings = &self.settings.fishing;
        let item_name = self
            .item_templates()
            .and_then(|t| t.get(settings.championship_reward_item).map(|t| t.name.clone()))
            .unwrap_or_default();
        let prizes = settings.championship_rewards;
        *html = html
            .replace("%TABLE%", table)
            .replace("%prizeItem%", &item_name)
            .replace("%prizeFirst%", &prizes[0].to_string())
            .replace("%prizeTwo%", &prizes[1].to_string())
            .replace("%prizeThree%", &prizes[2].to_string())
            .replace("%prizeFour%", &prizes[3].to_string())
            .replace("%prizeFive%", &prizes[4].to_string());
    }

    /// Java `showMidResult(player)`: the ranking of the running week, rebuilt at most once a
    /// minute.
    pub(crate) fn championship_show_mid_result(&mut self, id: ObjectId) {
        if !self.players.contains_key(&id) {
            return;
        }
        if self.fishing.championship.need_refresh {
            if let Some(html) = self.html_from_file(id, 0, &format!("{HTML_DIR}fish_event003.htm")) {
                self.send_html(id, &html);
            }
            self.refresh_result();
            let due = self.tick + TimerWheel::ms_to_ticks(REFRESH_DELAY_MS);
            self.timers
                .schedule_at(due, TimerKind::Fishing(FishingTimer::ChampionshipRefresh));
            return;
        }

        let Some(mut html) = self.html_from_file(id, 0, &format!("{HTML_DIR}fish_event002.htm")) else {
            return;
        };
        let c = &self.fishing.championship;
        let table = self.championship_table(&c.players_name, &c.fish_length, "0");
        self.replace_prizes(&mut html.html, &table);
        self.send_html(id, &html);
    }

    /// Java `showChampScreen(player, npc)`: the winners of the last week and the time left.
    pub(crate) fn championship_show_champ_screen(&mut self, id: ObjectId, npc: ObjectId) {
        let Some(mut html) = self.html_from_file(id, 0, &format!("{HTML_DIR}fish_event001.htm")) else {
            return;
        };
        let c = &self.fishing.championship;
        let table = self.championship_table(&c.win_players_name, &c.win_fish_length, "0");
        self.replace_prizes(&mut html.html, &table);
        html.html = html
            .html
            .replace(
                "%refresh%",
                &minutes_remaining(self.fishing.championship.end_date, unix_ms_now()).to_string(),
            )
            .replace("%objectId%", &npc.0.to_string());
        self.send_html(id, &html);
    }

    /// Java `isWinner(playerName)`: an exact match.
    pub(crate) fn championship_is_winner(&self, name: &str) -> bool {
        self.fishing.championship.win_players_name.iter().any(|n| n == name)
    }

    /// Java `getReward(player)`.
    pub(crate) fn championship_get_reward(&mut self, id: ObjectId) {
        let Some(name) = self.players.get(&id).map(|p| p.data.name.clone()) else {
            return;
        };
        let settings = self.settings.fishing.clone();
        let count = self.fishing.championship.win_players.len();
        for index in 0..count {
            let championship = &mut self.fishing.championship;
            let fisher = &championship.win_players[index];
            if !(fisher.name.eq_ignore_ascii_case(&name) && fisher.rewarded != 2) {
                continue;
            }
            let mut reward_count = 0;
            for (place, winner) in championship.win_players_name.iter().enumerate() {
                if winner.eq_ignore_ascii_case(&name)
                    && let Some(&reward) = settings.championship_rewards.get(place)
                {
                    reward_count = reward;
                }
            }
            championship.win_players[index].rewarded = 2;
            if reward_count > 0 {
                self.add_item(id, settings.championship_reward_item, i64::from(reward_count), true);
                if let Some(html) =
                    self.html_from_file(id, 0, &format!("{HTML_DIR}fish_event_reward001.htm"))
                {
                    self.send_html(id, &html);
                }
            }
        }
    }

    /// Java `Fisherman.sendHtml`.
    fn fisherman_send_html(&mut self, player: ObjectId, npc: ObjectId, name: &str) {
        if let Some(html) = self.html_from_file(player, npc.0, &format!("{HTML_DIR}{name}")) {
            self.send_html(player, &html);
        }
    }

    /// Java `Fisherman.onBypassFeedback`: `FishSkillList`, `FishingChampionship` and
    /// `FishingReward`. `false` for every other command (the caller hands it to the normal NPC
    /// code).
    pub(crate) fn fisherman_bypass_feedback(&mut self, npc: ObjectId, player: ObjectId, command: &str) -> bool {
        let enabled = self.settings.fishing.championship_enabled;
        if command.eq_ignore_ascii_case("FishSkillList") {
            // FOLLOW-UP: `showFishSkillList` needs the fishing skill tree, which is not loaded.
            debug!("{player}: FishSkillList needs the fishing skill tree (not loaded).");
            true
        } else if command.starts_with("FishingChampionship") {
            if enabled {
                self.championship_show_champ_screen(player, npc);
            } else {
                self.fisherman_send_html(player, npc, "no_fish_event001.htm");
            }
            true
        } else if command.starts_with("FishingReward") {
            if enabled {
                let name = self
                    .players
                    .get(&player)
                    .map(|p| p.data.name.clone())
                    .unwrap_or_default();
                if self.championship_is_winner(&name) {
                    self.championship_get_reward(player);
                } else {
                    self.fisherman_send_html(player, npc, "no_fish_event_reward001.htm");
                }
            } else {
                self.fisherman_send_html(player, npc, "no_fish_event001.htm");
            }
            true
        } else {
            false
        }
    }

    /// Java `RequestExFishRanking.runImpl`.
    pub(crate) fn on_request_ex_fish_ranking(&mut self, id: ObjectId) {
        if self.settings.fishing.championship_enabled {
            self.championship_show_mid_result(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const UTC: fn(i64) -> i64 = |_| 0;

    /// 2026-10-19 (a Monday) 00:00:00 UTC.
    const MONDAY: i64 = 1_792_368_000_000;

    fn row(name: &str, length: f64) -> ChampionshipRow {
        ChampionshipRow {
            name: name.into(),
            length,
            rewarded: 0,
        }
    }

    #[test]
    fn lengths_print_like_java() {
        assert_eq!(double_text(75.0), "75.0");
        assert_eq!(double_text(75.342), "75.342");
        assert_eq!(double_text(0.0), "0.0");
    }

    #[test]
    fn bubble_sort_is_a_stable_sort_longest_first() {
        let mut rows = vec![row("a", 61.0), row("b", 70.5), row("c", 61.0), row("d", 80.0)];
        sort_longest_first(&mut rows);
        let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["d", "b", "a", "c"]);
    }

    #[test]
    fn the_period_ends_on_the_tuesday_after_6_days_at_19_00() {
        // Monday 2026-10-19 12:34:56.789 + 6 days = Sunday 25th; Sunday-first weeks: the Tuesday
        // of the week that starts on that Sunday is the 27th.
        let from = MONDAY + 12 * HOUR_MS + 34 * 60_000 + 56_789;
        let end = end_of_champ(from, UTC);
        assert_eq!(end, MONDAY + 8 * DAY_MS + 19 * HOUR_MS + 789);
        // Tuesday + 6 days = Monday: the Tuesday of that week is the next day.
        let from = MONDAY + DAY_MS + 3 * HOUR_MS;
        let end = end_of_champ(from, UTC);
        assert_eq!(end, MONDAY + 8 * DAY_MS + 19 * HOUR_MS);
        // Saturday + 6 days = Friday: the Tuesday before it.
        let from = MONDAY + 5 * DAY_MS;
        let end = end_of_champ(from, UTC);
        assert_eq!(end, MONDAY + 8 * DAY_MS + 19 * HOUR_MS);
    }

    #[test]
    fn the_period_uses_the_local_zone() {
        // UTC+2 all year: 2026-10-19 23:00 UTC is already Tuesday the 20th 01:00 locally.
        let plus_two: fn(i64) -> i64 = |_| 2 * HOUR_MS;
        let from = MONDAY + 23 * HOUR_MS;
        let end = end_of_champ(from, plus_two);
        // Local: Tuesday 20th + 6 = Monday 26th -> Tuesday 27th 19:00 local = 17:00 UTC.
        assert_eq!(end, MONDAY + 8 * DAY_MS + 17 * HOUR_MS);
    }

    #[test]
    fn minutes_left_truncate_toward_zero() {
        assert_eq!(minutes_remaining(120_000, 0), 2);
        assert_eq!(minutes_remaining(179_999, 0), 2);
        assert_eq!(minutes_remaining(0, 59_999), 0);
    }
}
