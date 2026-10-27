//! The Grand Olympiad (M8-OLY-01): the periods (competition, validation, the weekly points),
//! the nobles and their points, the registration (classed, non-classed, teams), the games in
//! the stadiums, the buffers, the spectators and the manager NPCs.
// Java: mechanics/olympiad/{Olympiad,OlympiadManager,OlympiadGameManager,OlympiadGameTask,
//       AbstractOlympiadGame,OlympiadGameNormal,OlympiadGameClassed,OlympiadGameNonClassed,
//       OlympiadGameTeams,Participant,OlympiadInfo,OlympiadAnnouncer,CompetitionType,
//       OlympiadGameState}.java
// Java: entity/zone/type/OlympiadStadiumZone.java, entity/actor/instance/OlympiadManager.java
// Java: entity/actor/Player.java (the olympiad and observer fields, enterOlympiadObserverMode,
//       leaveOlympiadObserverMode, leaveObserverMode)
// Java: dist/game/data/scripts/handlers/bypass/npc/{OlympiadManagerLink,OlympiadObservation}.java
// Java: dist/game/data/scripts/handlers/chat/commands/user/OlympiadStat.java
//
// # How it fits together
// - [`OlympiadWorld`] (in `World::olympiad`) is Java's three singletons: `Olympiad` (the period,
//   the nobles by char id, the scheduled tasks), `OlympiadManager` (the waiting lists) and
//   `OlympiadGameManager` (one [`Stadium`] per `OlympiadStadiumZone`, each with its game task).
// - `period.rs`: `Olympiad` (load, `init`, the competition start/end, the weekly change, the
//   period end and the validation end, the hero choice that feeds `Hero`, the passes).
// - `registration.rs`: `OlympiadManager` (register, unregister, `checkNoble`, the disconnect).
// - `game.rs`: the games (`OlympiadGameNormal` for classed and non-classed, `OlympiadGameTeams`),
//   their participants, `validateWinner` and the points.
// - `task.rs`: `OlympiadGameTask` (the state machine of one stadium), `OlympiadGameManager.run`
//   (the 30 s matchmaking) and `OlympiadAnnouncer`.
// - `stadium.rs`: `OlympiadStadiumZone` (who it takes, enter/exit, the broadcasts, the doors and
//   buffers of the stadium's instance).
// - `observer.rs`: the spectators (`enterOlympiadObserverMode`, `leaveOlympiadObserverMode`,
//   `leaveObserverMode`) and the client packets of the observer.
// - `npc.rs`: the manager and buffer pages, the bypasses and `/olympiadstat`.
//
// # Timers
// Every `ThreadPool` task of Java is an [`OlympiadTimer`] on the timer wheel. Java's `Olympiad`
// keeps the competition start task when `init()` runs again, so an old start can still fire; it
// checks `isOlympiadEnd()` first, as here. The competition end's "sleep a minute while games run"
// loop is a timer that checks again every 60 s ([`OlympiadTimer::CompEndWait`]).
//
// # Clocks
// Java keeps the period ends in `System.currentTimeMillis()` and builds the dates with
// `Calendar.getInstance()` in the JVM's zone (D-48). Tests pin both with
// [`OlympiadSettings::clock_override_unix_ms`] and [`OlympiadSettings::zone_offset_override_ms`].
//
// # Side by side (D-39, D-53)
// The period, the validation and the weekly points are global: with `CycleDatabaseWrites = False`
// the Java game server runs them on the same database. This server then
// - reads `olympiad_data`, `olympiad_nobles` and the last period's ranking at start (the game
//   server's loader) and keeps its own copy in memory,
// - runs the same timers and matches in memory but writes none of the global rows: no
//   `olympiad_data`, no whole-table `olympiad_nobles` save, no `TRUNCATE`, no
//   `olympiad_nobles_eom` copy, and it hands out no passes at the period end (Java writes those
//   `character_variables` rows for every noble),
// - reads the rows again when it shows them: the class rankings and the last period's points
//   (Java reads the database there too) and the unclaimed passes before it shows or hands them
//   out (D-50),
// - still writes what Java writes per match: the `olympiad_fights` row of a decided match.
// With the switch on it writes everything where Java does.
//
// # Left out, with the system that brings it
// - `Hero` (the hero choice, `isHero`, `claimHero`, the hero list and the pages, D-53's `heroes`
//   rows): M8-HERO-01. The period end hands the heroes to be to
//   [`World::olympiad_compute_new_heroes`].
// - The admin words (`//checkolympiad`, `//saveolymp`, `//endolympiad`): M8-ADMIN-07, on
//   [`World::olympiad_millis_to_olympiad_end`], [`World::save_olympiad_status`] and
//   [`World::olympiad_manual_select_heroes`].
// - `AntiFeedManager` (`DualboxCheckMaxOlympiadParticipantsPerIP`, default 0 = off) and the
//   `olympiad` log file (`OlympiadLogFights`): not ported.
// - The `ON_OLYMPIAD_MATCH_RESULT` script event and the custom events (`isRegisteredOnEvent`,
//   D-05).

mod game;
mod npc;
mod observer;
mod period;
mod registration;
mod stadium;
mod task;

use std::collections::{BTreeMap, HashMap};

use l2_util::ObjectId;

use crate::timer::TimerId;
use crate::world::World;

pub use game::{CompetitionType, OlympiadGame, Participant};
pub use npc::register_bypass_handlers;
pub(crate) use npc::{OLYMPIAD_HOST, olympiad_command_bypass};
pub use observer::OlympiadInput;
pub use task::GameState;

/// Java `Olympiad.OLYMPIAD_HTML_PATH`.
pub const OLYMPIAD_HTML_PATH: &str = "data/html/olympiad/";
/// Java `Olympiad.UNCLAIMED_OLYMPIAD_PASSES_VAR`.
pub const UNCLAIMED_OLYMPIAD_PASSES_VAR: &str = "UNCLAIMED_OLYMPIAD_POINTS";
/// The user command of `OlympiadStat.java` (`/olympiadstat`).
pub const OLYMPIAD_STAT_COMMAND: i32 = 109;

/// The `Olympiad.ini` values the Olympiad reads (Java `OlympiadConfig`), the D-53 switch and the
/// test clocks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OlympiadSettings {
    /// `OlympiadEnabled` (default true).
    pub enabled: bool,
    /// `OlympiadStartTime` (hour, default 18) and `OlympiadMin` (default 0).
    pub start_time: i32,
    pub min: i32,
    /// `OlympiadMaxBuffs` (default 5).
    pub max_buffs: i32,
    /// `OlympiadCPeriod` (ms, default 6 hours).
    pub comp_period_ms: i64,
    /// `OlympiadBattle` (ms, default 5 minutes).
    pub battle_ms: i64,
    /// `OlympiadWPeriod` (ms, default a week).
    pub weekly_period_ms: i64,
    /// `OlympiadVPeriod` (ms, default a day).
    pub validation_period_ms: i64,
    /// `OlympiadStartPoints` (default 10) and `OlympiadWeeklyPoints` (default 10).
    pub start_points: i32,
    pub weekly_points: i32,
    /// `OlympiadClassedParticipants`, `OlympiadNonClassedParticipants` (11),
    /// `OlympiadTeamsParticipants` (6): the waiting list size that starts games.
    pub classed: i32,
    pub non_classed: i32,
    pub teams: i32,
    /// `OlympiadRegistrationDisplayNumber` (default 100).
    pub reg_display: i32,
    /// `OlympiadClassedReward`, `OlympiadNonClassedReward`, `OlympiadTeamReward`.
    pub classed_reward: Vec<[i32; 2]>,
    pub non_classed_reward: Vec<[i32; 2]>,
    pub team_reward: Vec<[i32; 2]>,
    /// `OlympiadCompRewItem` (default 13722, the Olympiad Token).
    pub comp_reward_item: i32,
    /// `OlympiadMinMatchesForPoints` (default 15).
    pub min_matches: i32,
    /// `OlympiadGPPerPoint` (default 1000).
    pub gp_per_point: i32,
    /// `OlympiadHeroPoints` (200) and `OlympiadRank1Points` .. `OlympiadRank5Points`.
    pub hero_points: i32,
    pub rank_points: [i32; 5],
    /// `OlympiadMaxPoints` (default 10).
    pub max_points: i32,
    /// `OlympiadDividerClassed`, `OlympiadDividerNonClassed` (default 5).
    pub divider_classed: i32,
    pub divider_non_classed: i32,
    /// `OlympiadMaxWeeklyMatches` (70), `...NonClassed` (60), `...Classed` (30), `...Team` (10).
    pub max_weekly_matches: i32,
    pub max_weekly_matches_non_classed: i32,
    pub max_weekly_matches_classed: i32,
    pub max_weekly_matches_team: i32,
    /// `OlympiadShowMonthlyWinners` (default true).
    pub show_monthly_winners: bool,
    /// `OlympiadAnnounceGames` (default true).
    pub announce_games: bool,
    /// `OlympiadRestrictedItems`: the items a fighter cannot use (`isOlyRestrictedItem`).
    pub restricted_items: std::collections::BTreeSet<i32>,
    /// `OlympiadDisableBlessedSpiritShots` (default false).
    pub disable_blessed_spiritshots: bool,
    /// `OlympiadWaitTime` (seconds, default 120).
    pub wait_time: i32,
    /// `OlympiadUseCustomPeriodSettings` (false), `OlympiadPeriod` (`MONTH`),
    /// `OlympiadPeriodMultiplier` (1), `OlympiadCompetitionDays` (1..7, Sunday is 1).
    pub use_custom_period_settings: bool,
    pub period: String,
    pub period_multiplier: i32,
    pub competition_days: Vec<i32>,
    /// `CycleDatabaseWrites` of `config/SideBySide.ini` (D-53, `game_server::cycle_database_writes`).
    pub cycle_database_writes: bool,
    /// Tests: the unix time (ms) at world tick 0. `None` reads the system clock.
    pub clock_override_unix_ms: Option<i64>,
    /// Tests: the zone offset (ms) the calendar uses. `None` asks the machine (D-48).
    pub zone_offset_override_ms: Option<i64>,
}

impl Default for OlympiadSettings {
    /// Java's defaults.
    fn default() -> Self {
        Self {
            enabled: true,
            start_time: 18,
            min: 0,
            max_buffs: 5,
            comp_period_ms: 21_600_000,
            battle_ms: 300_000,
            weekly_period_ms: 604_800_000,
            validation_period_ms: 86_400_000,
            start_points: 10,
            weekly_points: 10,
            classed: 11,
            non_classed: 11,
            teams: 6,
            reg_display: 100,
            classed_reward: vec![[13722, 50]],
            non_classed_reward: vec![[13722, 40]],
            team_reward: vec![[13722, 85]],
            comp_reward_item: 13722,
            min_matches: 15,
            gp_per_point: 1000,
            hero_points: 200,
            rank_points: [100, 75, 55, 40, 30],
            max_points: 10,
            divider_classed: 5,
            divider_non_classed: 5,
            max_weekly_matches: 70,
            max_weekly_matches_non_classed: 60,
            max_weekly_matches_classed: 30,
            max_weekly_matches_team: 10,
            show_monthly_winners: true,
            announce_games: true,
            restricted_items: std::collections::BTreeSet::new(),
            disable_blessed_spiritshots: false,
            wait_time: 120,
            use_custom_period_settings: false,
            period: "MONTH".to_owned(),
            period_multiplier: 1,
            competition_days: vec![1, 2, 3, 4, 5, 6, 7],
            cycle_database_writes: false,
            clock_override_unix_ms: None,
            zone_offset_override_ms: None,
        }
    }
}

/// The `olympiad_data` row (Java `_currentCycle`, `_period`, `_olympiadEnd`, `_validationEnd`,
/// `_nextWeeklyChange`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OlympiadData {
    pub current_cycle: i32,
    pub period: i32,
    pub olympiad_end: i64,
    pub validation_end: i64,
    pub next_weekly_change: i64,
}

/// One noble (Java's `StatSet` in `Olympiad.NOBLES`, one `olympiad_nobles` row).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NobleStats {
    pub class_id: i32,
    pub char_name: String,
    pub points: i32,
    pub comp_done: i32,
    pub comp_won: i32,
    pub comp_lost: i32,
    pub comp_drawn: i32,
    pub comp_done_week: i32,
    pub comp_done_week_classed: i32,
    pub comp_done_week_non_classed: i32,
    pub comp_done_week_team: i32,
    /// Java `"to_save"`: a new noble, written with `INSERT` (then `UPDATE`).
    pub to_save: bool,
}

/// One noble of the startup read (`OLYMPIAD_LOAD_NOBLES`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NobleRow {
    pub char_id: i32,
    pub stats: NobleStats,
}

/// What the game server reads for [`World::load_olympiad`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OlympiadLoad {
    /// The `olympiad_data` row; `None` without one (or when the read failed).
    pub data: Option<OlympiadData>,
    /// `olympiad_nobles` joined with `characters`.
    pub nobles: Vec<NobleRow>,
    /// `GET_ALL_CLASSIFIED_NOBLESS`: the char ids of `olympiad_nobles_eom` with enough matches,
    /// best first (the ranking `loadNoblesRank` builds when the server starts in the validation
    /// period).
    pub classified: Vec<i32>,
}

/// A write for the persistence task (`PersistCommand::Olympiad`), in Java's order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OlympiadWrite {
    /// Java `saveNobleData`: one `INSERT` (`insert`, a new noble) or `UPDATE` per noble. Only with
    /// `CycleDatabaseWrites` (D-53).
    SaveNobles(Vec<(i32, NobleStats)>),
    /// Java `saveOlympiadStatus`'s `OLYMPIAD_SAVE_DATA`. Only with `CycleDatabaseWrites`.
    SaveData(OlympiadData),
    /// Java `deleteNobles`: `TRUNCATE olympiad_nobles`. Only with `CycleDatabaseWrites`.
    DeleteNobles,
    /// Java `updateMonthlyData`. Only with `CycleDatabaseWrites`.
    UpdateMonthlyData,
    /// Java `loadNoblesRank` for an offline noble: the `character_variables` row of the unclaimed
    /// points is replaced. Only with `CycleDatabaseWrites`.
    UnclaimedPoints { char_id: i32, points: i32 },
    /// Java `OlympiadGameNormal.saveResults`: the row of one decided match (written as Java does,
    /// whatever the switch says: it is the match of this server's players).
    Fight(OlympiadFight),
}

/// One `olympiad_fights` row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OlympiadFight {
    pub char_one_id: i32,
    pub char_two_id: i32,
    pub char_one_class: i32,
    pub char_two_class: i32,
    pub winner: i32,
    pub start: i64,
    pub time: i64,
    pub classed: bool,
}

/// A read the world asks for (`PersistCommand::OlympiadReread`): Java reads the database at
/// these points (or the world reads a shared row again before it hands items out, D-50).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OlympiadRereadRequest {
    /// Java `getClassLeaderBoard(classId)`.
    ClassLeaderBoard {
        class_id: i32,
        monthly: bool,
        min_matches: i32,
    },
    /// Java `getLastNobleOlympiadPoints(objId)`.
    LastPoints { char_id: i32 },
    /// The `UNCLAIMED_OLYMPIAD_POINTS` variable of a character.
    UnclaimedPasses { char_id: i32 },
}

/// The rows of an [`OlympiadRereadRequest`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OlympiadRereadRows {
    Names(Vec<String>),
    Points(i32),
    /// The variable's text, `None` without the row.
    Passes(Option<String>),
}

/// What the world does with the answer of a read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OlympiadRereadThen {
    pub player: ObjectId,
    /// The manager NPC the page belongs to.
    pub npc: ObjectId,
    pub action: OlympiadRereadAction,
}

/// See [`OlympiadRereadThen`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OlympiadRereadAction {
    /// `olympiad_ranking.htm`.
    Ranking,
    /// `noble_points2.htm`.
    LastPoints,
    /// `olympiadnoble 6`: `noble_settle.htm` or `noble_nopoints2.htm`.
    SettlePage,
    /// `olympiadnoble 10`: the passes are handed out.
    GivePasses,
}

/// The timers of the Olympiad (`TimerKind::Olympiad`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OlympiadTimer {
    /// `_scheduledCompStart`.
    CompStart,
    /// `_scheduledCompEnd`.
    CompEnd,
    /// The competition end waits a minute while games still run (`Thread.sleep(60000)`).
    CompEndWait,
    /// "THE_GRAND_OLYMPIAD_REGISTRATION_PERIOD_HAS_ENDED", ten minutes before the end.
    RegistrationEnd,
    /// `_scheduledOlympiadEnd` (`OlympiadEndTask`).
    OlympiadEnd,
    /// `_scheduledWeeklyTask`.
    Weekly,
    /// `_scheduledValdationTask` (`ValidationEndTask`).
    ValidationEnd,
    /// `_gameManager`: `OlympiadGameManager.run` every 30 s.
    GameManager,
    /// `_gameAnnouncer`: `OlympiadAnnouncer.run` every 500 ms.
    Announcer,
    /// The game task of a stadium (by index).
    Task(usize),
    /// `OlympiadStadiumZone.KickPlayer`.
    KickPlayer(ObjectId),
}

/// The Olympiad state of a player (Java `Player._inOlympiadMode`, `_olympiadStart`,
/// `_olympiadGameId`, `_olympiadSide`, `_olyBuffsCount`, `_observerMode`, `_lastLoc`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerOlympiad {
    pub in_olympiad_mode: bool,
    pub olympiad_start: bool,
    /// -1: none.
    pub game_id: i32,
    /// -1: none.
    pub side: i32,
    pub buff_count: i32,
    pub observer_mode: bool,
    /// `_lastLoc`: (0, 0, 0) when unset.
    pub last_loc: (i32, i32, i32),
}

impl Default for PlayerOlympiad {
    fn default() -> Self {
        Self {
            in_olympiad_mode: false,
            olympiad_start: false,
            game_id: -1,
            side: -1,
            buff_count: 0,
            observer_mode: false,
            last_loc: (0, 0, 0),
        }
    }
}

/// One stadium (Java `OlympiadStadiumZone` with its `OlympiadGameTask`).
#[derive(Debug, Clone, PartialEq)]
pub struct Stadium {
    pub zone_id: i32,
    pub name: String,
    /// The dynamic instance of the arena's template (`_zone.setInstanceId`).
    pub instance_id: i32,
    /// The zone's `spawn` points: the first half for side 1, the second half for side 2.
    pub spawns: Vec<(i32, i32, i32)>,
    pub spectator_spawns: Vec<(i32, i32, i32)>,
    pub task: task::GameTask,
}

/// The Olympiad of the world (Java `Olympiad`, `OlympiadManager`, `OlympiadGameManager`).
#[derive(Debug, Default)]
pub struct OlympiadWorld {
    pub(crate) settings: OlympiadSettings,
    /// `load()` ran (the Olympiad is enabled).
    pub(crate) loaded: bool,
    pub(crate) data: OlympiadData,
    /// `NOBLES`, by char id.
    pub(crate) nobles: BTreeMap<i32, NobleStats>,
    /// `NOBLES_RANK`.
    pub(crate) nobles_rank: HashMap<i32, i32>,
    /// `HEROS_TO_BE`.
    pub(crate) heroes_to_be: Vec<HeroToBe>,
    /// `_compStart` and `_compEnd` (unix ms).
    pub(crate) comp_start: i64,
    pub(crate) comp_end: i64,
    /// `_inCompPeriod`.
    pub(crate) in_comp_period: bool,
    pub(crate) olympiad_end_task: Option<TimerId>,
    pub(crate) weekly_task: Option<TimerId>,
    pub(crate) validation_task: Option<TimerId>,
    pub(crate) game_manager_task: Option<TimerId>,
    pub(crate) announcer_task: Option<TimerId>,
    /// `OlympiadManager._nonClassBasedRegisters`.
    pub(crate) non_class_based: Vec<i32>,
    /// `OlympiadManager._classBasedRegisters`: base class -> char ids.
    pub(crate) class_based: BTreeMap<i32, Vec<i32>>,
    /// `OlympiadManager._teamsBasedRegisters`.
    pub(crate) teams_based: Vec<Vec<i32>>,
    /// `OlympiadGameManager._battleStarted`.
    pub(crate) battle_started: bool,
    /// `OlympiadGameManager._tasks`; `None` until the manager is made.
    pub(crate) stadiums: Option<Vec<Stadium>>,
    /// `OlympiadAnnouncer._currentStadium`.
    pub(crate) announcer_stadium: usize,
}

/// One hero to be (Java's `StatSet` of `HEROS_TO_BE`: class, char id, name).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeroToBe {
    pub class_id: i32,
    pub char_id: i32,
    pub char_name: String,
}

impl OlympiadWorld {
    /// Java `Olympiad.getNobleStats(playerId)`.
    pub fn noble(&self, char_id: i32) -> Option<&NobleStats> {
        self.nobles.get(&char_id)
    }

    /// Every noble, by char id.
    pub fn nobles(&self) -> &BTreeMap<i32, NobleStats> {
        &self.nobles
    }

    /// The period row as the world keeps it.
    pub fn data(&self) -> OlympiadData {
        self.data
    }

    /// Java `inCompPeriod()`.
    pub fn in_comp_period(&self) -> bool {
        self.in_comp_period
    }

    /// Java `getCompStart()` / `_compEnd` (unix ms).
    pub fn comp_start(&self) -> i64 {
        self.comp_start
    }

    pub fn comp_end(&self) -> i64 {
        self.comp_end
    }

    /// The stadiums, empty until `OlympiadGameManager` is made.
    pub fn stadiums(&self) -> &[Stadium] {
        self.stadiums.as_deref().unwrap_or(&[])
    }

    /// `HEROS_TO_BE`.
    pub fn heroes_to_be(&self) -> &[HeroToBe] {
        &self.heroes_to_be
    }

    /// The waiting lists: non-classed, classed by base class, teams.
    pub fn registered_non_classed(&self) -> &[i32] {
        &self.non_class_based
    }

    pub fn registered_classed(&self) -> &BTreeMap<i32, Vec<i32>> {
        &self.class_based
    }

    pub fn registered_teams(&self) -> &[Vec<i32>] {
        &self.teams_based
    }
}

impl World {
    /// The Olympiad of the world.
    pub fn olympiad(&self) -> &OlympiadWorld {
        &self.olympiad
    }

    /// Java `Player.isInOlympiadMode()`.
    pub fn is_in_olympiad_mode(&self, id: ObjectId) -> bool {
        self.players
            .get(&id)
            .is_some_and(|p| p.olympiad.in_olympiad_mode)
    }

    /// Java `Player.isOlympiadStart()`.
    pub fn is_olympiad_start(&self, id: ObjectId) -> bool {
        self.players.get(&id).is_some_and(|p| p.olympiad.olympiad_start)
    }

    /// Java `Player.getOlympiadGameId()` (-1 for none and for a creature that is no player).
    pub fn olympiad_game_id(&self, id: ObjectId) -> i32 {
        self.players.get(&id).map_or(-1, |p| p.olympiad.game_id)
    }

    /// Java `Player.getOlympiadSide()`.
    pub fn olympiad_side(&self, id: ObjectId) -> i32 {
        self.players.get(&id).map_or(-1, |p| p.olympiad.side)
    }

    /// Java `Player.getOlympiadBuffCount()`.
    pub fn olympiad_buff_count(&self, id: ObjectId) -> i32 {
        self.players.get(&id).map_or(0, |p| p.olympiad.buff_count)
    }

    /// Java `Player.inObserverMode()`.
    pub fn in_observer_mode(&self, id: ObjectId) -> bool {
        self.players.get(&id).is_some_and(|p| p.olympiad.observer_mode)
    }

    /// The player state of a player.
    pub fn player_olympiad(&self, id: ObjectId) -> Option<PlayerOlympiad> {
        self.players.get(&id).map(|p| p.olympiad)
    }

    pub(crate) fn player_olympiad_mut(&mut self, id: ObjectId) -> Option<&mut PlayerOlympiad> {
        self.players.get_mut(&id).map(|p| &mut p.olympiad)
    }

    /// Two players of the same running game (`isInOlympiadMode()` of both and the same
    /// `getOlympiadGameId()`), the test Java repeats in the PvP and skill checks.
    pub fn same_olympiad_game(&self, a: ObjectId, b: ObjectId) -> bool {
        self.is_in_olympiad_mode(a)
            && self.is_in_olympiad_mode(b)
            && self.olympiad_game_id(a) == self.olympiad_game_id(b)
    }

    /// Java `ItemTemplate.isOlyRestrictedItem()`.
    pub fn is_oly_restricted_item(&self, item_id: i32) -> bool {
        self.olympiad.settings.restricted_items.contains(&item_id)
    }

    /// `OlympiadConfig.OLYMPIAD_DISABLE_BLESSED_SPIRITSHOTS`.
    pub fn olympiad_disables_blessed_spiritshots(&self) -> bool {
        self.olympiad.settings.disable_blessed_spiritshots
    }
}
