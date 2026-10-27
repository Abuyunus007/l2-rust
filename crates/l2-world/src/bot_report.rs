//! The bot report button (M6-FU-09): `RequestActionUse` 65, the reports of the players, their
//! points, the punishments by report count and the daily reset of the points.
// Java: data/BotReportTable.java
// Java: network/clientpackets/RequestActionUse.java (case 65)
// Java: config/GeneralConfig.java (EnableBotReportButton, BotReportPointsResetHour,
// Java:   BotReportDelay, AllowReportsFromSameClanMembers)
//
// # Shape
// Everything `BotReportTable` keeps is in [`BotReportWorld`], keyed by character id (D-11): the
// reports of each reported character, the points of each reporter and the report time of each IP
// address. The reports start from the rows of `bot_reported_char_data` (`World::load_bot_reports`,
// read at startup only when the button is on, like Java), and every new report is a
// `PersistCommand::BotReport` row.
//
// # Differences from Java, with the reason
// - Java rewrites the whole table when the server shuts down (`saveReportedCharData`: `DELETE`, then
//   one `INSERT` per report). The Java game server shares the table (D-39), so a rewrite from this
//   server's memory would erase the reports the other one took meanwhile (D-49). Every report is
//   written when it is made instead (`INSERT .. ON DUPLICATE KEY UPDATE`); the rows are the same
//   ones in the end.
// - The IP hash needs the IPv4 address of the reporter's connection; Java's `hashIp` throws for
//   anything else, and the report is dropped without a word (here too).
//
// # Left out, with the card that brings it
// - Fake players (`isFakePlayer`, `getFakePlayerInfo().isTalkable()`) do not exist in the port.

use std::collections::BTreeMap;
use std::net::IpAddr;

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use tracing::{info, warn};

use crate::effects::SkillKey;
use crate::outbox::PersistCommand;
use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::{World, unix_ms_now};
use crate::zones::ZoneId;

/// `BotReportTable.COLUMN_*`: one row of `bot_reported_char_data`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BotReportRow {
    /// `botId`.
    pub bot_id: i32,
    /// `reporterId`.
    pub reporter_id: i32,
    /// `reportDate`: Unix milliseconds.
    pub report_date: i64,
}

/// One `<punishment>` of `BotReportPunishments.xml` (`PunishHolder`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BotPunishment {
    pub skill_id: i32,
    pub skill_level: i32,
    /// `sysMessageId`, -1 for none.
    pub system_message: i32,
}

/// The values `BotReportTable` reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BotReportSettings {
    /// `GeneralConfig.BOTREPORT_ENABLE` (`EnableBotReportButton`).
    pub enable: bool,
    /// `BOTREPORT_RESETPOINT_HOUR` (`BotReportPointsResetHour`, "HH:MM"): the hour and minute,
    /// `None` when Java's parse fails (the reset then runs every 24 hours).
    pub reset_hour_minute: Option<(i64, i64)>,
    /// `BOTREPORT_REPORT_DELAY` (`BotReportDelay` minutes), in milliseconds.
    pub report_delay_ms: i64,
    /// `BOTREPORT_ALLOW_REPORTS_FROM_SAME_CLAN_MEMBERS`.
    pub allow_reports_from_same_clan_members: bool,
    /// `BotReportPunishments.xml` by `neededReportCount` (negative: from that many on).
    pub punishments: BTreeMap<i32, BotPunishment>,
}

impl Default for BotReportSettings {
    /// The button is off (`EnableBotReportButton` defaults to False in the config class).
    fn default() -> Self {
        Self {
            enable: false,
            reset_hour_minute: Some((0, 0)),
            report_delay_ms: 30 * 60_000,
            allow_reports_from_same_clan_members: false,
            punishments: BTreeMap::new(),
        }
    }
}

/// `BotReportTable.ReporterCharData`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Reporter {
    last_report: i64,
    /// A Java `byte`.
    points: i8,
}

impl Reporter {
    fn new() -> Self {
        Self {
            last_report: 0,
            points: 7,
        }
    }
}

/// `BotReportTable.getInstance()`'s fields.
#[derive(Debug, Default)]
pub(crate) struct BotReportWorld {
    pub settings: BotReportSettings,
    /// `_ipRegistry`: the last report time of each hashed IP address.
    ip_registry: BTreeMap<i32, i64>,
    /// `_charRegistry`.
    reporters: BTreeMap<i32, Reporter>,
    /// `_reports`: each reported character to its reporters and the time of their reports
    /// (`ReportedCharData._reporters`).
    reports: BTreeMap<i32, BTreeMap<i32, i64>>,
    /// The timer of the daily points reset.
    reset_timer: Option<TimerId>,
}

/// `hashIp`: the IPv4 octets as a little endian int. `None` for an address Java cannot parse.
fn hash_ip(ip: Option<IpAddr>) -> Option<i32> {
    match ip? {
        IpAddr::V4(v4) => {
            let [a, b, c, d] = v4.octets();
            Some(i32::from(a) | (i32::from(b) << 8) | (i32::from(c) << 16) | (i32::from(d) << 24))
        }
        IpAddr::V6(_) => None,
    }
}

const DAY_MS: i64 = 86_400_000;

/// The moment of `hour:minute` on the local day of `now_ms`, with the seconds and milliseconds of
/// `now_ms`: `Calendar.set(HOUR_OF_DAY, hour)` and `set(MINUTE, minute)` leave them alone.
/// `offset_at(utc_ms)` is the local zone's offset (D-48).
fn same_day_target(now_ms: i64, hour: i64, minute: i64, offset_at: &impl Fn(i64) -> i64) -> i64 {
    let local_now = now_ms + offset_at(now_ms);
    let day_start = local_now - local_now.rem_euclid(DAY_MS);
    let local = day_start + hour * 3_600_000 + minute * 60_000 + local_now.rem_euclid(60_000);
    // The zone's offset at the target moment (a daylight saving change in between).
    let guess = local - offset_at(now_ms);
    local - offset_at(guess)
}

/// `BotReportTable.scheduleResetPointTask`: the delay until the next reset. Today's moment, or
/// tomorrow's when today's has passed.
pub fn next_reset_delay_ms(now_ms: i64, hour: i64, minute: i64, offset_at: impl Fn(i64) -> i64) -> i64 {
    let target = same_day_target(now_ms, hour, minute, &offset_at);
    let target = if target < now_ms { target + DAY_MS } else { target };
    target - now_ms
}

/// `loadReportedCharData`'s `lastResetTime`: today's reset moment, yesterday's when it is still to
/// come.
pub fn last_reset_ms(now_ms: i64, hour: i64, minute: i64, offset_at: impl Fn(i64) -> i64) -> i64 {
    let target = same_day_target(now_ms, hour, minute, &offset_at);
    if now_ms < target { target - DAY_MS } else { target }
}

impl World {
    /// Installs the settings and the punishments (`BotReportTable`'s constructor). A punishment
    /// whose skill does not exist is dropped with a warning, like Java's `addPunishment`.
    pub fn set_bot_report(&mut self, mut settings: BotReportSettings) {
        if let Some(skills) = self.skill_table() {
            settings.punishments.retain(|needed, punishment| {
                let known = skills.get(punishment.skill_id, punishment.skill_level).is_some();
                if !known {
                    warn!(
                        "BotReportTable: Could not add punishment for {needed} report(s): Skill {}-{} does not exist!",
                        punishment.skill_id, punishment.skill_level
                    );
                }
                known
            });
        }
        self.bot_report.settings = settings;
    }

    /// The settings in use (tests).
    pub fn bot_report_settings(&self) -> &BotReportSettings {
        &self.bot_report.settings
    }

    /// `BotReportTable.loadReportedCharData`: the reports of the table, and the points the
    /// reporters have left since the last reset. Call once at startup, only with the button on.
    pub fn load_bot_reports(&mut self, rows: Vec<BotReportRow>) {
        self.load_bot_reports_at(rows, unix_ms_now(), l2_util::time::local_offset_millis);
    }

    /// [`Self::load_bot_reports`] for a given clock.
    pub fn load_bot_reports_at(
        &mut self,
        rows: Vec<BotReportRow>,
        now_ms: i64,
        offset_at: impl Fn(i64) -> i64,
    ) {
        if !self.bot_report.settings.enable {
            return;
        }
        let last_reset = self
            .bot_report
            .settings
            .reset_hour_minute
            .map_or(0, |(hour, minute)| last_reset_ms(now_ms, hour, minute, offset_at));
        let table = &mut self.bot_report;
        for row in rows {
            table
                .reports
                .entry(row.bot_id)
                .or_default()
                .insert(row.reporter_id, row.report_date);
            if row.report_date > last_reset {
                match table.reporters.get_mut(&row.reporter_id) {
                    Some(reporter) => reporter.points = reporter.points.wrapping_sub(1),
                    None => {
                        table.reporters.insert(
                            row.reporter_id,
                            Reporter {
                                last_report: 0,
                                points: 6,
                            },
                        );
                    }
                }
            }
        }
        info!(
            "BotReportTable: Loaded {} bot reports",
            self.bot_report.reports.len()
        );
    }

    /// `BotReportTable.scheduleResetPointTask`, the first run. Call once at startup, only with the
    /// button on; the world never starts it by itself, so tests stay independent of the clock.
    pub fn start_bot_report_reset_task(&mut self) {
        self.schedule_bot_report_reset(unix_ms_now(), l2_util::time::local_offset_millis);
    }

    /// [`Self::start_bot_report_reset_task`] for a given clock.
    pub fn schedule_bot_report_reset(&mut self, now_ms: i64, offset_at: impl Fn(i64) -> i64) {
        let delay = match self.bot_report.settings.reset_hour_minute {
            Some((hour, minute)) => next_reset_delay_ms(now_ms, hour, minute, offset_at),
            None => {
                warn!(
                    "BotReportTable: Could not properly schedule bot report points reset task. Scheduled in 24 hours."
                );
                DAY_MS
            }
        };
        let due = self.tick + TimerWheel::ms_to_ticks(u64::try_from(delay).unwrap_or(0));
        let timer = self.timers.schedule_at(due, TimerKind::BotReportReset);
        if let Some(old) = self.bot_report.reset_timer.replace(timer) {
            self.timers.cancel(old);
        }
    }

    /// Whether the daily points reset is scheduled (tests).
    pub fn has_bot_report_reset_task(&self) -> bool {
        self.bot_report.reset_timer.is_some()
    }

    /// The timer of `ResetPointTask`: `resetPointsAndSchedule`.
    pub(crate) fn on_bot_report_reset_timer(&mut self) {
        self.bot_report.reset_timer = None;
        for reporter in self.bot_report.reporters.values_mut() {
            reporter.points = 7;
        }
        self.start_bot_report_reset_task();
    }

    /// `RequestActionUse` case 65: with the button on `BotReportTable.reportBot(player)`, else the
    /// text "This feature is disabled.".
    pub(crate) fn on_bot_report_action(&mut self, player: ObjectId, target: Option<ObjectId>) {
        if self.bot_report.settings.enable {
            self.report_bot(player, target);
        } else {
            self.send_message(player, "This feature is disabled.");
        }
    }

    /// Java `BotReportTable.reportBot(reporter)`: `true` when the report was registered.
    pub fn report_bot(&mut self, reporter: ObjectId, target: Option<ObjectId>) -> bool {
        self.report_bot_at(reporter, target, unix_ms_now())
    }

    /// [`Self::report_bot`] for a given clock.
    pub fn report_bot_at(&mut self, reporter: ObjectId, target: Option<ObjectId>, now_ms: i64) -> bool {
        let Some(bot) = target else {
            return false;
        };
        // `!bot.isPlayer() && !bot.isFakePlayer()`, or the reporter itself.
        if !self.players.contains_key(&bot) || bot == reporter {
            return false;
        }
        if self.is_inside_zone(bot, ZoneId::Peace) || self.is_inside_zone(bot, ZoneId::Pvp) {
            self.report_tell(
                reporter,
                SystemMessageId::YouCannotReportACharacterWhoIsInAPeaceZoneOrABattlefield,
            );
            return false;
        }
        // `bot.isInOlympiadMode()` (M8-OLY-01).
        if self.is_in_olympiad_mode(bot) {
            self.send(
                reporter,
                &SystemMessage::new(
                    SystemMessageId::ThisCharacterCannotMakeAReportYouCannotMakeAReportWhileLocatedInsideAPeaceZoneOrABattlefieldWhileYouAreAnOpposingClanMemberDuringAClanWarOrWhileParticipatingInTheOlympiad,
                ),
            );
            return false;
        }
        if let (Some(bot_clan), Some(reporter_clan)) = (self.clan_of(bot), self.clan_of(reporter))
            && bot_clan.is_at_war_with(reporter_clan.id)
        {
            self.report_tell(
                reporter,
                SystemMessageId::YouCannotReportWhenAClanWarHasBeenDeclared,
            );
            return false;
        }
        // NOTE(java-quirk): `PlayerStat.setStartingExp` is never called, so the starting exp is 0
        // and only a character without any exp "has not acquired any exp after connecting".
        if self.players.get(&bot).is_some_and(|p| p.data.exp == 0) {
            self.report_tell(
                reporter,
                SystemMessageId::YouCannotReportACharacterWhoHasNotAcquiredAnyExpAfterConnecting,
            );
            return false;
        }

        let settings = &self.bot_report.settings;
        let delay = settings.report_delay_ms;
        let same_clan_allowed = settings.allow_reports_from_same_clan_members;
        let table = &self.bot_report;
        // `_reports.containsKey(reporterId)`: a reported character cannot report.
        if table.reports.contains_key(&reporter.0) {
            self.report_tell(
                reporter,
                SystemMessageId::YouHaveBeenReportedAsAnIllegalProgramUserAndCannotReportOtherUsers,
            );
            return false;
        }
        let ip_client = self.players.get(&reporter).and_then(|p| p.client_ip);
        let Some(ip) = hash_ip(ip_client) else {
            // Java: `Integer.parseInt` of a part that is not a number throws.
            warn!("BotReportTable: no IPv4 address for {reporter}; the report is dropped.");
            return false;
        };
        // `timeHasPassed(_ipRegistry, ip)`.
        if table
            .ip_registry
            .get(&ip)
            .is_some_and(|&time| now_ms - time <= delay)
        {
            self.report_tell(
                reporter,
                SystemMessageId::ThisCharacterCannotMakeAReportTheTargetHasAlreadyBeenReportedByEitherYourClanOrAllianceOrHasAlreadyBeenReportedFromYourCurrentIp,
            );
            return false;
        }
        if let Some(reported) = table.reports.get(&bot.0) {
            if reported.contains_key(&reporter.0) {
                // NOTE(java-quirk): the message id is C1_PURCHASED_S3_S2_S, no parameters.
                self.report_tell(reporter, SystemMessageId::C1PurchasedS3S2S);
                return false;
            }
            if !same_clan_allowed
                && self
                    .clan_of(reporter)
                    .is_some_and(|clan| reported.keys().any(|&id| clan.is_member(id)))
            {
                self.report_tell(
                    reporter,
                    SystemMessageId::ThisCharacterCannotMakeAReportTheTargetHasAlreadyBeenReportedByEitherYourClanOrAllianceOrHasAlreadyBeenReportedFromYourCurrentIp,
                );
                return false;
            }
        }
        if let Some(rep) = table.reporters.get(&reporter.0) {
            if rep.points == 0 {
                self.report_tell(
                    reporter,
                    SystemMessageId::YouHaveUsedAllAvailablePointsPointsAreResetEverydayAtNoon,
                );
                return false;
            }
            let reuse = now_ms - rep.last_report;
            if reuse < delay {
                // NOTE(java-quirk): the text is FIVE_YEARS_HAVE_PASSED_SINCE_THIS_CHARACTER_S_CREATION,
                // whose parameters are the minutes since the last report and the points left.
                self.send(
                    reporter,
                    &SystemMessage::new(SystemMessageId::FiveYearsHavePassedSinceThisCharacterSCreation)
                        .with(SystemMessageParam::IntNumber((reuse / 60_000) as i32))
                        .with(SystemMessageParam::IntNumber(i32::from(rep.points))),
                );
                return false;
            }
        }

        // The report counts.
        let table = &mut self.bot_report;
        let count = {
            let reported = table.reports.entry(bot.0).or_default();
            reported.insert(reporter.0, now_ms);
            reported.len() as i32
        };
        let rep = table.reporters.entry(reporter.0).or_insert_with(Reporter::new);
        // `registerReport`.
        rep.points = rep.points.wrapping_sub(1);
        rep.last_report = now_ms;
        let points_left = rep.points;
        table.ip_registry.insert(ip, now_ms);
        self.outbox.persist(PersistCommand::BotReport(BotReportRow {
            bot_id: bot.0,
            reporter_id: reporter.0,
            report_date: now_ms,
        }));

        let bot_name = self
            .players
            .get(&bot)
            .map(|p| p.data.name.clone())
            .unwrap_or_default();
        self.send(
            reporter,
            &SystemMessage::new(SystemMessageId::C1WasReportedAsABot)
                .with(SystemMessageParam::Text(bot_name.clone())),
        );
        self.send(
            reporter,
            &SystemMessage::new(
                SystemMessageId::YouHaveUsedAReportPointOnC1YouHaveS2PointsRemainingOnThisAccount,
            )
            .with(SystemMessageParam::Text(bot_name))
            .with(SystemMessageParam::IntNumber(i32::from(points_left))),
        );
        self.handle_bot_report(bot, count);
        true
    }

    /// `BotReportTable.handleReport`: the punishment of exactly this many reports, then the range
    /// punishments (negative keys) this count reaches.
    fn handle_bot_report(&mut self, bot: ObjectId, count: i32) {
        let punishments = &self.bot_report.settings.punishments;
        let mut due: Vec<BotPunishment> = punishments.get(&count).copied().into_iter().collect();
        // NOTE(java-quirk): Java walks a `ConcurrentHashMap`; the order here is by key.
        due.extend(
            punishments
                .iter()
                .filter(|(key, _)| **key < 0 && key.unsigned_abs() as i32 <= count)
                .map(|(_, p)| *p),
        );
        for punishment in due {
            self.punish_bot(bot, punishment);
        }
    }

    /// `BotReportTable.punishBot`.
    fn punish_bot(&mut self, bot: ObjectId, punishment: BotPunishment) {
        self.apply_effects(
            bot,
            bot,
            SkillKey::new(punishment.skill_id, punishment.skill_level),
        );
        if punishment.system_message > -1
            && let Some(id) = SystemMessageId::from_id(punishment.system_message)
        {
            self.send(bot, &SystemMessage::new(id));
        }
    }

    fn report_tell(&self, to: ObjectId, id: SystemMessageId) {
        self.send(to, &SystemMessage::new(id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: i64 = 3_600_000;
    const MIN: i64 = 60_000;
    /// 2024-01-10T12:00:30.250Z.
    const NOON: i64 = 1_704_888_000_000 + 30_250;

    #[test]
    fn the_ip_hash_is_the_octets_little_endian() {
        let ip = Some(IpAddr::from([1, 2, 3, 4]));
        assert_eq!(hash_ip(ip), Some(1 | (2 << 8) | (3 << 16) | (4 << 24)));
        // The top octet makes a negative int, like Java's `<< 24`.
        assert_eq!(hash_ip(Some(IpAddr::from([0, 0, 0, 200]))), Some(200_i32 << 24));
        assert_eq!(hash_ip(Some("::1".parse().unwrap())), None);
        assert_eq!(hash_ip(None), None);
    }

    /// `scheduleResetPointTask`: today's moment (with the seconds of now) when it is to come,
    /// else tomorrow's.
    #[test]
    fn the_reset_is_the_next_hour_and_minute_in_the_local_zone() {
        // UTC: 12:00:30.250 now, the reset at 18:00.
        assert_eq!(next_reset_delay_ms(NOON, 18, 0, |_| 0), 6 * HOUR);
        // 06:00 has passed: tomorrow.
        assert_eq!(next_reset_delay_ms(NOON, 6, 0, |_| 0), 18 * HOUR);
        // The zone is UTC+2: it is 14:00:30.250 there, the reset at 18:30 is 4.5 h away.
        assert_eq!(
            next_reset_delay_ms(NOON, 18, 30, |_| 2 * HOUR),
            4 * HOUR + 30 * MIN
        );
    }

    /// `loadReportedCharData`: the reset of today, or of yesterday when today's is to come.
    #[test]
    fn the_last_reset_is_today_or_yesterday() {
        assert_eq!(last_reset_ms(NOON, 6, 0, |_| 0), NOON - 6 * HOUR);
        assert_eq!(last_reset_ms(NOON, 18, 0, |_| 0), NOON + 6 * HOUR - DAY_MS);
    }
}
