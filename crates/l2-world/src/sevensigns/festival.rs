//! The Festival of Darkness (M8-FEST-01): the festival manager that runs every cycle of the
//! competition period, the arenas with their witch and monsters, the signing up at the festival
//! guides, the blood offerings, the scores in `seven_signs_festival` and the accumulated bonus.
// Java: mechanics/sevensigns/SevenSignsFestival.java
// Java: entity/actor/instance/{FestivalGuide,FestivalMonster}.java
// Java: dist/game/data/scripts/handlers/bypass/npc/Festival.java
// Java: mechanics/sevensigns/SevenSigns.java (addFestivalScore)
//
// # Shape
// Java's `FestivalManager.run()` blocks its thread in `wait(...)` between the steps of one round.
// Here the same sequence is a chain of timers on the world's wheel, one [`FestivalStage`] per
// `wait`: the code after a `wait` is the handler of its stage. A round (`run`) starts every
// `AltFestivalCycleLength` (`scheduleAtFixedRate`: a round that overran is followed at once).
// A `Spawn` of Java is a runtime spawn (`add_runtime_spawn`), a `FestivalMonster` an NPC of the
// template type `FestivalMonster` that the world remembers in `monsters` (its offering bonus).
//
// # Side by side (D-39, D-50, D-53)
// The rows of `seven_signs_festival` are global: with `CycleDatabaseWrites = False` nothing is
// written (the scores stay in this world's memory) and the rows are read again when a score table
// is shown (`SSQStatus`, the guide's pages 4, 5 and 6; a read replaces the data in memory).
// Java writes no festival row per player.
//
// # Left out
// - `FestivalMonster.isAutoAttackable(attacker)` (a festival monster is not attackable by another
//   one): monsters never attack monsters here.
// - Archer targeting TODO of Java's header comment (it is a TODO in Java too).

use std::collections::{BTreeMap, HashMap};

use l2_protocol_game::server::{
    ActionFailed, ChatType, CreatureSay, MagicSkillUse, SayBody, SenderField, SystemMessage,
    SystemMessageParam,
};
use l2_protocol_game::{NpcStringId, SystemMessageId};
use l2_util::ObjectId;
use tracing::{debug, info, warn};

use super::festival_data::{
    FESTIVAL_DAWN_CHEST_SPAWNS, FESTIVAL_DAWN_PLAYER_SPAWNS, FESTIVAL_DAWN_PRIMARY_SPAWNS,
    FESTIVAL_DAWN_SECONDARY_SPAWNS, FESTIVAL_DAWN_WITCH_SPAWNS, FESTIVAL_DUSK_CHEST_SPAWNS,
    FESTIVAL_DUSK_PLAYER_SPAWNS, FESTIVAL_DUSK_PRIMARY_SPAWNS, FESTIVAL_DUSK_SECONDARY_SPAWNS,
    FESTIVAL_DUSK_WITCH_SPAWNS,
};
use super::{
    CABAL_DAWN, CABAL_DUSK, CABAL_NULL, SEAL_STONE_BLUE_ID, SEAL_STONE_BLUE_VALUE, SEAL_STONE_GREEN_ID,
    SEAL_STONE_GREEN_VALUE, SEAL_STONE_RED_ID, SEAL_STONE_RED_VALUE, SEVEN_SIGNS_HTML_PATH, SevenSignsThen,
    SevenSignsTimer, SevenSignsWrite, cabal_short_name,
};
use crate::movement::Intention;
use crate::outbox::PersistCommand;
use crate::party::PartyMessageType;
use crate::teleport::{TeleportTarget, TeleportWhereType};
use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::{World, unix_ms_now};

/// `FESTIVAL_COUNT`: do not change without correcting the database and `SevenSigns` itself.
pub const FESTIVAL_COUNT: usize = 5;
/// `FESTIVAL_LEVEL_MAX_*`.
pub const FESTIVAL_LEVEL_MAX_31: i32 = 0;
pub const FESTIVAL_LEVEL_MAX_42: i32 = 1;
pub const FESTIVAL_LEVEL_MAX_53: i32 = 2;
pub const FESTIVAL_LEVEL_MAX_64: i32 = 3;
pub const FESTIVAL_LEVEL_MAX_NONE: i32 = 4;
/// `FESTIVAL_LEVEL_SCORES`: what a festival gives its cabal (500 at most together).
pub const FESTIVAL_LEVEL_SCORES: [i32; FESTIVAL_COUNT] = [60, 70, 100, 120, 150];
/// `FESTIVAL_OFFERING_ID` / `FESTIVAL_OFFERING_VALUE`: the blood offering and its score.
pub const FESTIVAL_OFFERING_ID: i32 = 5901;
pub const FESTIVAL_OFFERING_VALUE: i64 = 5;

const FESTIVAL_MAX_OFFSET_X: i32 = 230;
const FESTIVAL_MAX_OFFSET_Y: i32 = 230;
/// `FESTIVAL_DEFAULT_RESPAWN`: seconds.
const FESTIVAL_DEFAULT_RESPAWN: i32 = 60;
/// The NPCs whose speech the oracles hear (`npcSpawned`).
const DAWN_CHAT_GUIDE: i32 = 31127;
const DUSK_CHAT_GUIDE: i32 = 31137;
const WITCH_MAGIC_SKILL_1: i32 = 2003;
const WITCH_MAGIC_SKILL_2: i32 = 2133;

/// The values of `Feature.ini` the festival reads (`FeatureConfig.ALT_FESTIVAL_*` and
/// `FESTIVAL_WIN_POINTS`), `Default` the Java defaults.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FestivalSettings {
    /// `AltFestivalMinPlayer`.
    pub min_player: i32,
    /// `AltFestivalManagerStart`: ms after the start (or the competition period) of the first round.
    pub manager_start: i64,
    /// `AltFestivalLength`.
    pub length: i64,
    /// `AltFestivalCycleLength`.
    pub cycle_length: i64,
    /// `AltFestivalFirstSpawn`.
    pub first_spawn: i64,
    /// `AltFestivalFirstSwarm`.
    pub first_swarm: i64,
    /// `AltFestivalSecondSpawn`.
    pub second_spawn: i64,
    /// `AltFestivalSecondSwarm`.
    pub second_swarm: i64,
    /// `AltFestivalChestSpawn`.
    pub chest_spawn: i64,
    /// `FestivalOfDarknessWin`: the reputation points of the highest ranked party's clans.
    pub win_points: i32,
}

impl Default for FestivalSettings {
    fn default() -> Self {
        Self {
            min_player: 5,
            manager_start: 120_000,
            length: 1_080_000,
            cycle_length: 2_280_000,
            first_spawn: 120_000,
            first_swarm: 300_000,
            second_spawn: 540_000,
            second_swarm: 720_000,
            chest_spawn: 900_000,
            win_points: 200,
        }
    }
}

impl FestivalSettings {
    /// `FESTIVAL_SIGNUP_TIME`.
    pub fn signup_time(&self) -> i64 {
        self.cycle_length - self.length - 60_000
    }
}

/// One `StatSet` of `_festivalData` (a `seven_signs_festival` row): the best score of one festival
/// and cabal in one Seven Signs cycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FestivalData {
    pub festival_id: i32,
    /// `"dawn"` or `"dusk"`.
    pub cabal: String,
    pub cycle: i32,
    /// The milliseconds the score was set (`0`: never).
    pub date: i64,
    pub score: i32,
    /// The names of the party, separated by commas.
    pub members: String,
}

impl FestivalData {
    /// The blank set `getHighestScoreData` makes when it has no data.
    fn blank() -> Self {
        Self {
            festival_id: 0,
            cabal: String::new(),
            cycle: 0,
            date: 0,
            score: 0,
            members: String::new(),
        }
    }
}

/// The steps of one round of the manager, each after one of Java's `wait(...)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FestivalStage {
    /// After `wait(FESTIVAL_SIGNUP_TIME)`.
    AfterSignup,
    /// After `wait(CYCLE - SIGNUP)` with nobody signed up.
    NoPartyWait,
    /// After `wait(ALT_FESTIVAL_FIRST_SPAWN)`.
    FirstSpawn,
    /// After `wait(FIRST_SWARM - FIRST_SPAWN)`.
    FirstSwarm,
    /// After `wait(SECOND_SPAWN - FIRST_SWARM)`.
    SecondSpawn,
    /// After `wait(SECOND_SWARM - SECOND_SPAWN)`.
    SecondSwarm,
    /// After `wait(CHEST_SPAWN - SECOND_SWARM)`.
    ChestSpawn,
    /// After `wait(LENGTH - elapsedTime)`.
    End,
}

/// The festival timers (inside `SevenSignsTimer::Festival`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FestivalTimer {
    /// The scheduler starts a round.
    ManagerRun,
    /// A `wait` of the round ended.
    Stage(FestivalStage),
}

/// Java `FestivalSpawn`: a place (and an npc id) of the tables.
#[derive(Debug, Clone, Copy)]
struct FestivalSpawn {
    x: i32,
    y: i32,
    z: i32,
    heading: i32,
    npc_id: i32,
}

/// Java `DarknessFestival`: one running arena.
#[derive(Debug, Clone)]
pub(crate) struct DarknessFestival {
    cabal: i32,
    level_range: i32,
    challenge_increased: bool,
    start_location: FestivalSpawn,
    witch_spawn: FestivalSpawn,
    witch_inst: Option<ObjectId>,
    witch_spawn_index: Option<usize>,
    /// `_npcInsts`: the monsters and chests with the runtime spawn of each.
    npc_insts: Vec<(ObjectId, usize)>,
    /// `_participants` (`None`: Java's null after `festivalEnd`).
    participants: Option<Vec<ObjectId>>,
    /// `_originalLocations`: x, y, z and heading before the teleport.
    original_locations: BTreeMap<ObjectId, (i32, i32, i32, i32)>,
}

/// The festival state of the world.
#[derive(Debug, Default)]
pub(crate) struct FestivalWorld {
    /// `_festivalData`: by Seven Signs cycle, then by offset (`festivalId`, plus 5 for Dawn).
    pub(crate) data: BTreeMap<i32, BTreeMap<i32, FestivalData>>,
    /// `_signsCycle`.
    pub(crate) signs_cycle: i32,
    /// `_nextFestivalCycleStart` / `_nextFestivalStart` (epoch ms).
    next_festival_cycle_start: i64,
    next_festival_start: i64,
    /// `_festivalInitialized` / `_festivalInProgress`.
    initialized: bool,
    in_progress: bool,
    /// `_managerInstance != null`.
    manager_exists: bool,
    /// `_managerScheduledTask != null` and not cancelled.
    scheduler_active: bool,
    /// The epoch ms the next round is due at (the fixed rate of the scheduler).
    next_run_at: i64,
    /// The pending `ManagerRun` timer.
    run_timer: Option<TimerId>,
    /// The pending stage timer of the running round.
    stage_timer: Option<TimerId>,
    /// A round is in progress (its stages are on the wheel).
    round_active: bool,
    /// The seconds of the round: `elapsedTime`.
    elapsed: i64,
    dawn_participants: BTreeMap<i32, Option<Vec<ObjectId>>>,
    dusk_participants: BTreeMap<i32, Option<Vec<ObjectId>>>,
    dawn_previous: BTreeMap<i32, Vec<ObjectId>>,
    dusk_previous: BTreeMap<i32, Vec<ObjectId>>,
    dawn_scores: BTreeMap<i32, i64>,
    dusk_scores: BTreeMap<i32, i64>,
    /// `_festivalInstances`: 10 + level range for Dusk, 20 + level range for Dawn.
    instances: BTreeMap<i32, DarknessFestival>,
    /// The `_bonusMultiplier` of each festival monster that has one other than 1.
    monsters: HashMap<ObjectId, i32>,
    /// The names of the characters that were in a registered party (`CharInfoTable.getNameById`).
    names: BTreeMap<ObjectId, String>,
}

/// Java `String.split(",")`: no trailing empty strings, and the whole text when there is no comma
/// (so `""` gives one empty name).
pub(crate) fn java_split_comma(text: &str) -> Vec<String> {
    let mut parts: Vec<String> = text.split(',').map(str::to_owned).collect();
    if parts.len() > 1 {
        while parts.last().is_some_and(String::is_empty) {
            parts.pop();
        }
    }
    parts
}

/// `getFestivalName(festivalId)`.
pub fn festival_name(festival_id: i32) -> &'static str {
    match festival_id {
        FESTIVAL_LEVEL_MAX_31 => "Level 31 or lower",
        FESTIVAL_LEVEL_MAX_42 => "Level 42 or lower",
        FESTIVAL_LEVEL_MAX_53 => "Level 53 or lower",
        FESTIVAL_LEVEL_MAX_64 => "Level 64 or lower",
        _ => "No Level Limit",
    }
}

/// `getMaxLevelForFestival(festivalId)`; `max_level` is `ExperienceData.getMaxLevel()`.
pub fn max_level_for_festival(festival_id: i32, max_level: i32) -> i32 {
    match festival_id {
        FESTIVAL_LEVEL_MAX_31 => 31,
        FESTIVAL_LEVEL_MAX_42 => 42,
        FESTIVAL_LEVEL_MAX_53 => 53,
        FESTIVAL_LEVEL_MAX_64 => 64,
        _ => max_level - 1,
    }
}

/// `isFestivalArcher(npcId)`: the archers and marksmen end in 4 or 9.
fn is_festival_archer(npc_id: i32) -> bool {
    if !(18009..=18108).contains(&npc_id) {
        return false;
    }
    let identifier = npc_id % 10;
    identifier == 4 || identifier == 9
}

/// What the `FestivalGuide` of an NPC id is: the level range, the oracle and the seal stones the
/// registration costs (blue, green, red).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuideInfo {
    pub festival_type: i32,
    pub oracle: i32,
    pub blue: i32,
    pub green: i32,
    pub red: i32,
}

impl GuideInfo {
    /// The constructor's `switch (getId())`.
    pub fn of_npc(npc_id: i32) -> Self {
        let range = |ty, oracle, blue, green, red| Self {
            festival_type: ty,
            oracle,
            blue,
            green,
            red,
        };
        match npc_id {
            31127 | 31132 => range(FESTIVAL_LEVEL_MAX_31, CABAL_DAWN, 900, 540, 270),
            31128 | 31133 => range(FESTIVAL_LEVEL_MAX_42, CABAL_DAWN, 1500, 900, 450),
            31129 | 31134 => range(FESTIVAL_LEVEL_MAX_53, CABAL_DAWN, 3000, 1800, 900),
            31130 | 31135 => range(FESTIVAL_LEVEL_MAX_64, CABAL_DAWN, 4500, 2700, 1350),
            31131 | 31136 => range(FESTIVAL_LEVEL_MAX_NONE, CABAL_DAWN, 6000, 3600, 1800),
            31137 | 31142 => range(FESTIVAL_LEVEL_MAX_31, CABAL_DUSK, 900, 540, 270),
            31138 | 31143 => range(FESTIVAL_LEVEL_MAX_42, CABAL_DUSK, 1500, 900, 450),
            31139 | 31144 => range(FESTIVAL_LEVEL_MAX_53, CABAL_DUSK, 3000, 1800, 900),
            31140 | 31145 => range(FESTIVAL_LEVEL_MAX_64, CABAL_DUSK, 4500, 2700, 1350),
            31141 | 31146 => range(FESTIVAL_LEVEL_MAX_NONE, CABAL_DUSK, 6000, 3600, 1800),
            _ => range(FESTIVAL_LEVEL_MAX_NONE, CABAL_NULL, 0, 0, 0),
        }
    }

    /// `getStoneCount(stoneType)`: -1 for anything but the three seal stones.
    pub fn stone_count(&self, stone_type: i32) -> i32 {
        match stone_type {
            SEAL_STONE_BLUE_ID => self.blue,
            SEAL_STONE_GREEN_ID => self.green,
            SEAL_STONE_RED_ID => self.red,
            _ => -1,
        }
    }
}

/// `FestivalMonster.isAggressive` and the like are asked by the AI by the template type.
pub(crate) const FESTIVAL_MONSTER_TYPE: &str = "FestivalMonster";

fn spawn_row(row: &[i32; 5]) -> FestivalSpawn {
    FestivalSpawn {
        x: row[0],
        y: row[1],
        z: row[2],
        heading: row[3],
        npc_id: row[4],
    }
}

impl World {
    // ------------------------------------------------------------------ the settings and start

    /// The festival settings (tests).
    pub fn festival_settings(&self) -> &FestivalSettings {
        &self.seven_signs.settings.festival
    }

    /// Java `SevenSignsFestival`'s constructor: the rows (`restoreFestivalData`), then, outside
    /// the seal validation period, the manager. Call once after the Seven Signs are loaded.
    pub(crate) fn festival_load(&mut self, rows: Vec<FestivalData>) {
        self.seven_signs.festival.signs_cycle = self.ss_current_cycle();
        self.seven_signs.festival.data.clear();
        self.festival_insert_rows(rows);
        if self.ss_is_seal_validation_period() {
            info!("SevenSignsFestival: Initialization bypassed due to Seal Validation in effect.");
            return;
        }
        self.festival_start_manager();
    }

    /// `restoreFestivalData`'s grouping: Dawn rows are stored at `festivalId + FESTIVAL_COUNT`.
    fn festival_insert_rows(&mut self, rows: Vec<FestivalData>) {
        for row in rows {
            let offset = if row.cabal == "dawn" {
                row.festival_id + FESTIVAL_COUNT as i32
            } else {
                row.festival_id
            };
            self.seven_signs.festival
                .data
                .entry(row.cycle)
                .or_default()
                .insert(offset, row);
        }
    }

    /// The answer of a D-50 read: the rows replace the data in memory (only with
    /// `CycleDatabaseWrites = False`, the caller checks).
    pub(crate) fn festival_apply_read(&mut self, rows: Vec<FestivalData>) {
        self.seven_signs.festival.data.clear();
        self.festival_insert_rows(rows);
        self.seven_signs.festival.signs_cycle = self.ss_current_cycle();
    }

    /// Java `startFestivalManager()`: `new FestivalManager()` (the cycle counter, the next start
    /// times), then the first round after `AltFestivalManagerStart`.
    pub(crate) fn festival_start_manager(&mut self) {
        let settings = self.seven_signs.settings.festival.clone();
        let now = unix_ms_now();
        // NOTE(java-quirk): `new FestivalManager()` drops the old instances without unspawning
        // them, so their witch and monsters would stay for good; they are unspawned here.
        self.festival_unspawn_all();
        self.seven_signs.festival.instances.clear();
        self.seven_signs.status.festival_cycle = self.seven_signs.status.festival_cycle.wrapping_add(1);
        self.seven_signs.festival.next_festival_cycle_start = now + settings.cycle_length;
        self.seven_signs.festival.next_festival_start = now + settings.cycle_length - settings.signup_time();
        self.seven_signs.festival.manager_exists = true;
        // `setNextFestivalStart(ALT_FESTIVAL_MANAGER_START + FESTIVAL_SIGNUP_TIME)`.
        self.seven_signs.festival.next_festival_start = now + settings.manager_start + settings.signup_time();
        self.seven_signs.festival.scheduler_active = true;
        self.seven_signs.festival.next_run_at = now + settings.manager_start;
        self.festival_schedule_run(settings.manager_start);
        info!(
            "SevenSignsFestival: The first Festival of Darkness cycle begins in {} minute(s).",
            settings.manager_start / 60_000
        );
    }

    /// Java `getFestivalManagerSchedule().cancel(false)`: no round after the running one. The
    /// getter starts the manager when there is none (`_managerScheduledTask == null`).
    pub(crate) fn festival_cancel_manager(&mut self) {
        if !self.seven_signs.festival.manager_exists {
            self.festival_start_manager();
        }
        self.seven_signs.festival.scheduler_active = false;
        if let Some(timer) = self.seven_signs.festival.run_timer.take() {
            self.timers.cancel(timer);
        }
    }

    fn festival_timer(&mut self, delay_ms: i64, timer: FestivalTimer) -> TimerId {
        let ms = u64::try_from(delay_ms).unwrap_or(0);
        let due = self.tick + TimerWheel::ms_to_ticks(ms);
        self.timers
            .schedule_at(due, TimerKind::SevenSigns(SevenSignsTimer::Festival(timer)))
    }

    fn festival_schedule_run(&mut self, delay_ms: i64) {
        let timer = self.festival_timer(delay_ms, FestivalTimer::ManagerRun);
        if let Some(old) = self.seven_signs.festival.run_timer.replace(timer) {
            self.timers.cancel(old);
        }
    }

    /// A `wait(ms)`: the stage runs after `ms`. A wait of 0 or less is a stage run at once.
    // NOTE(java-quirk): `wait(0)` waits forever and `wait(-1)` throws (ignored); the first would
    // stop the manager for good, so both go on at once here.
    fn festival_wait(&mut self, ms: i64, stage: FestivalStage) {
        let timer = self.festival_timer(ms.max(0), FestivalTimer::Stage(stage));
        if let Some(old) = self.seven_signs.festival.stage_timer.replace(timer) {
            self.timers.cancel(old);
        }
    }

    /// The festival timers.
    pub(crate) fn on_festival_timer(&mut self, timer: FestivalTimer) {
        match timer {
            FestivalTimer::ManagerRun => {
                self.seven_signs.festival.run_timer = None;
                self.festival_run();
            }
            FestivalTimer::Stage(stage) => {
                self.seven_signs.festival.stage_timer = None;
                self.festival_stage(stage);
            }
        }
    }

    /// `FestivalManager.run()`, up to its first `wait`.
    fn festival_run(&mut self) {
        if self.seven_signs.festival.round_active {
            // The scheduler never runs a task twice at once.
            return;
        }
        // The manager shouldn't be running if Seal Validation is in effect.
        if self.ss_is_seal_validation_period() {
            return self.festival_run_ended();
        }
        let settings = self.seven_signs.settings.festival.clone();
        // If the next period is due to start before the end of this festival cycle, then don't
        // run it.
        if self.seven_signs.next_period_change - unix_ms_now() < settings.cycle_length {
            return self.festival_run_ended();
        } else if self.festival_mins_to_next_festival() == 2 {
            self.festival_send_message_to_all(NpcStringId::TheMainEventWillStartIn2MinutesPleaseRegisterNow);
        }
        self.seven_signs.festival.round_active = true;
        // Stand by until the allowed signup period has elapsed.
        self.festival_wait(settings.signup_time(), FestivalStage::AfterSignup);
    }

    /// The end of a round: the scheduler's next one (`scheduleAtFixedRate`), if it was not
    /// cancelled.
    fn festival_run_ended(&mut self) {
        self.seven_signs.festival.round_active = false;
        if !self.seven_signs.festival.scheduler_active {
            return;
        }
        let period = self.seven_signs.settings.festival.cycle_length;
        self.seven_signs.festival.next_run_at += period;
        let delay = (self.seven_signs.festival.next_run_at - unix_ms_now()).max(0);
        self.festival_schedule_run(delay);
    }

    /// The code of `FestivalManager.run()` after each of its `wait`s.
    fn festival_stage(&mut self, stage: FestivalStage) {
        let s = self.seven_signs.settings.festival.clone();
        match stage {
            FestivalStage::AfterSignup => {
                // Clear past participants, they can no longer register their score if not done
                // so already.
                self.seven_signs.festival.dawn_previous.clear();
                self.seven_signs.festival.dusk_previous.clear();
                // Get rid of random monsters that avoided deletion after last festival.
                self.festival_unspawn_all();
                // Start only if participants signed up.
                self.festival_check_signed_up();
            }
            FestivalStage::NoPartyWait => {
                let keys: Vec<i32> = self.seven_signs.festival.instances.keys().copied().collect();
                for key in keys {
                    if self
                        .seven_signs.festival
                        .instances
                        .get(&key)
                        .is_some_and(|i| !i.npc_insts.is_empty())
                    {
                        self.festival_unspawn_mobs(key);
                    }
                }
                self.festival_check_signed_up();
            }
            FestivalStage::FirstSpawn => {
                // Participants can now opt to increase the challenge, if desired.
                self.seven_signs.festival.elapsed = s.first_spawn;
                self.seven_signs.festival.in_progress = true;
                // Sequentially set all festivals to begin, spawn the Festival Witch and notify
                // participants.
                for key in self.festival_instance_keys() {
                    self.festival_start_instance(key);
                    self.festival_instance_say(key, NpcStringId::TheMainEventIsNowStarting);
                }
                // After a short time period, move all idle spawns to the center of the arena.
                self.festival_wait(s.first_swarm - s.first_spawn, FestivalStage::FirstSwarm);
            }
            FestivalStage::FirstSwarm => {
                self.seven_signs.festival.elapsed += s.first_swarm - s.first_spawn;
                for key in self.festival_instance_keys() {
                    self.festival_move_monsters_to_center(key);
                }
                // Stand by until the time comes for the second spawn.
                self.festival_wait(s.second_spawn - s.first_swarm, FestivalStage::SecondSpawn);
            }
            FestivalStage::SecondSpawn => {
                // Spawn an extra set of monsters (archers) on the free platforms with a faster
                // respawn when killed.
                for key in self.festival_instance_keys() {
                    self.festival_spawn_monsters(key, FESTIVAL_DEFAULT_RESPAWN / 2, 2);
                    let end = (s.length - s.second_spawn) / 60_000;
                    if end == 2 {
                        self.festival_instance_say(
                            key,
                            NpcStringId::TheFestivalOfDarknessWillEndInTwoMinutes,
                        );
                    } else {
                        self.festival_instance_say_text(
                            key,
                            &format!("The Festival of Darkness will end in {end} minute(s)."),
                        );
                    }
                }
                self.seven_signs.festival.elapsed += s.second_spawn - s.first_swarm;
                // After another short time period, again move all idle spawns to the center.
                self.festival_wait(s.second_swarm - s.second_spawn, FestivalStage::SecondSwarm);
            }
            FestivalStage::SecondSwarm => {
                for key in self.festival_instance_keys() {
                    self.festival_move_monsters_to_center(key);
                }
                self.seven_signs.festival.elapsed += s.second_swarm - s.second_spawn;
                // Stand by until the time comes for the chests to be spawned.
                self.festival_wait(s.chest_spawn - s.second_swarm, FestivalStage::ChestSpawn);
            }
            FestivalStage::ChestSpawn => {
                // Spawn the festival chests, which enable the team to gain greater rewards for
                // each chest they kill.
                for key in self.festival_instance_keys() {
                    self.festival_spawn_monsters(key, FESTIVAL_DEFAULT_RESPAWN, 3);
                    // FIXME(java): "What is the correct npcString?"
                    self.festival_instance_say_text(
                        key,
                        "The chests have spawned! Be quick, the festival will end soon.",
                    );
                }
                self.seven_signs.festival.elapsed += s.chest_spawn - s.second_swarm;
                // Stand by and wait until it's time to end the festival.
                let rest = s.length - self.seven_signs.festival.elapsed;
                self.festival_wait(rest, FestivalStage::End);
            }
            FestivalStage::End => {
                // Participants can no longer opt to increase the challenge, as the festival will
                // soon close.
                self.seven_signs.festival.in_progress = false;
                // Sequentially begin the ending sequence for all running festivals.
                for key in self.festival_instance_keys() {
                    self.festival_end_instance(key);
                }
                // Clear the participants list for the next round of signups.
                self.seven_signs.festival.dawn_participants.clear();
                self.seven_signs.festival.dusk_participants.clear();
                // Allow signups for the next festival cycle.
                self.seven_signs.festival.initialized = false;
                self.festival_send_message_to_all(NpcStringId::ThatWillDoILlMoveYouToTheOutsideSoon);
                self.festival_run_ended();
            }
        }
    }

    /// `while (_noPartyRegister)` and the initiation after it.
    fn festival_check_signed_up(&mut self) {
        let s = self.seven_signs.settings.festival.clone();
        if self.seven_signs.festival.dusk_participants.is_empty() && self.seven_signs.festival.dawn_participants.is_empty() {
            let now = unix_ms_now();
            self.seven_signs.festival.next_festival_cycle_start = now + s.cycle_length;
            self.seven_signs.festival.next_festival_start = now + s.cycle_length - s.signup_time();
            self.festival_wait(s.cycle_length - s.signup_time(), FestivalStage::NoPartyWait);
            return;
        }
        // INITIATION: create the instances for the festivals in both Oracles, but only if they
        // have participants signed up for them.
        for i in 0..FESTIVAL_COUNT as i32 {
            if self.seven_signs.festival.dusk_participants.get(&i).is_some_and(Option::is_some) {
                let festival = self.festival_new_instance(CABAL_DUSK, i);
                self.seven_signs.festival.instances.insert(10 + i, festival);
                self.festival_init_instance(10 + i);
            }
            if self.seven_signs.festival.dawn_participants.get(&i).is_some_and(Option::is_some) {
                let festival = self.festival_new_instance(CABAL_DAWN, i);
                self.seven_signs.festival.instances.insert(20 + i, festival);
                self.festival_init_instance(20 + i);
            }
        }
        // Prevent future signups while festival is in progress.
        self.seven_signs.festival.initialized = true;
        self.seven_signs.festival.next_festival_start = unix_ms_now() + s.cycle_length;
        self.festival_send_message_to_all(NpcStringId::TheMainEventIsNowStarting);
        // Stand by for a short length of time before starting the festival.
        self.festival_wait(s.first_spawn, FestivalStage::FirstSpawn);
    }

    fn festival_instance_keys(&self) -> Vec<i32> {
        self.seven_signs.festival.instances.keys().copied().collect()
    }

    // ------------------------------------------------------------------ the questions

    /// `isFestivalInitialized()`.
    pub fn festival_initialized(&self) -> bool {
        self.seven_signs.festival.initialized
    }

    /// `isFestivalInProgress()`.
    pub fn festival_in_progress(&self) -> bool {
        self.seven_signs.festival.in_progress
    }

    /// `getCurrentFestivalCycle()`.
    pub fn festival_cycle(&self) -> i32 {
        self.seven_signs.status.festival_cycle
    }

    /// `getMinsToNextCycle()`.
    pub fn festival_mins_to_next_cycle(&self) -> i64 {
        if self.ss_is_seal_validation_period() {
            return -1;
        }
        (self.seven_signs.festival.next_festival_cycle_start - unix_ms_now()) / 60_000
    }

    /// `getMinsToNextFestival()`.
    pub fn festival_mins_to_next_festival(&self) -> i32 {
        if self.ss_is_seal_validation_period() {
            return -1;
        }
        // `(int) (((_nextFestivalStart - now) / 60000) + 1)`.
        (((self.seven_signs.festival.next_festival_start - unix_ms_now()) / 60_000) + 1) as i32
    }

    /// `getTimeToNextFestivalStr()` (the `%festivalMins%` of every NPC page).
    pub fn festival_time_to_next_festival_str(&self) -> String {
        if self.ss_is_seal_validation_period() {
            return "<font color=\"FF0000\">This is the Seal Validation period. Festivals will resume next week.</font>"
                .to_owned();
        }
        format!(
            "<font color=\"FF0000\">The next festival will begin in {} minute(s).</font>",
            self.festival_mins_to_next_festival()
        )
    }

    /// `isParticipant(player)`: in a registered party (`Player.isFestivalParticipant`).
    pub fn is_festival_participant(&self, player: ObjectId) -> bool {
        if self.ss_is_seal_validation_period() {
            return false;
        }
        if !self.seven_signs.festival.manager_exists {
            return false;
        }
        let has = |map: &BTreeMap<i32, Option<Vec<ObjectId>>>| {
            map.values().any(|p| p.as_ref().is_some_and(|p| p.contains(&player)))
        };
        has(&self.seven_signs.festival.dawn_participants) || has(&self.seven_signs.festival.dusk_participants)
    }

    /// `getFestivalForPlayer(player)`: the oracle and the level range, `(-1, -1)` for nobody.
    // NOTE(java-quirk): the loop reads Dusk at `++festivalId`, so it finds Dawn in ranges 0, 2 and 4
    // and Dusk in ranges 1, 3 and 5 only (the other ranges read as "not found").
    pub fn festival_for_player(&self, player: ObjectId) -> (i32, i32) {
        let mut festival_id = 0;
        let has = |map: &BTreeMap<i32, Option<Vec<ObjectId>>>, id: i32| {
            map.get(&id)
                .and_then(Option::as_ref)
                .is_some_and(|p| p.contains(&player))
        };
        while festival_id < FESTIVAL_COUNT as i32 {
            if has(&self.seven_signs.festival.dawn_participants, festival_id) {
                return (CABAL_DAWN, festival_id);
            }
            festival_id += 1;
            if has(&self.seven_signs.festival.dusk_participants, festival_id) {
                return (CABAL_DUSK, festival_id);
            }
            festival_id += 1;
        }
        (-1, -1)
    }

    /// `getParticipants(oracle, festivalId)`: the list, `None` for Java's null.
    pub fn festival_participants(&self, oracle: i32, festival_id: i32) -> Option<&Vec<ObjectId>> {
        let map = if oracle == CABAL_DAWN {
            &self.seven_signs.festival.dawn_participants
        } else {
            &self.seven_signs.festival.dusk_participants
        };
        map.get(&festival_id).and_then(Option::as_ref)
    }

    /// `getPreviousParticipants(oracle, festivalId)`.
    pub fn festival_previous_participants(&self, oracle: i32, festival_id: i32) -> Option<&Vec<ObjectId>> {
        let map = if oracle == CABAL_DAWN {
            &self.seven_signs.festival.dawn_previous
        } else {
            &self.seven_signs.festival.dusk_previous
        };
        map.get(&festival_id)
    }

    /// `setParticipants(oracle, festivalId, festivalParty)`: the members of the party (`None`
    /// stores Java's null, which still keeps the map from being empty).
    pub(crate) fn festival_set_participants(
        &mut self,
        oracle: i32,
        festival_id: i32,
        party: Option<crate::party::PartyId>,
    ) {
        let participants = party.map(|party| {
            let members = self.party_members(party);
            for member in &members {
                if let Some(p) = self.players.get(member) {
                    self.seven_signs.festival.names.insert(*member, p.data.name.clone());
                }
            }
            members
        });
        let map = if oracle == CABAL_DAWN {
            &mut self.seven_signs.festival.dawn_participants
        } else {
            &mut self.seven_signs.festival.dusk_participants
        };
        map.insert(festival_id, participants);
    }

    /// `getHighestScoreData(oracle, festivalId)`: the best score of this cycle (a blank set where
    /// Java's lookup throws).
    pub fn festival_highest_score_data(&self, oracle: i32, festival_id: i32) -> FestivalData {
        let offset = if oracle == CABAL_DAWN {
            festival_id + 5
        } else {
            festival_id
        };
        self.seven_signs.festival
            .data
            .get(&self.seven_signs.festival.signs_cycle)
            .and_then(|m| m.get(&offset))
            .cloned()
            .unwrap_or_else(FestivalData::blank)
    }

    /// `getHighestScore(oracle, festivalId)`.
    pub fn festival_highest_score(&self, oracle: i32, festival_id: i32) -> i32 {
        self.festival_highest_score_data(oracle, festival_id).score
    }

    /// `getOverallHighestScoreData(festivalId)`: the best score of any cycle.
    pub fn festival_overall_highest_score_data(&self, festival_id: i32) -> Option<FestivalData> {
        let mut result: Option<&FestivalData> = None;
        let mut highest = 0;
        for cycle in self.seven_signs.festival.data.values() {
            for data in cycle.values() {
                if data.festival_id != festival_id {
                    continue;
                }
                if data.score > highest {
                    highest = data.score;
                    result = Some(data);
                }
            }
        }
        result.cloned()
    }

    /// `getAccumulatedBonus(festivalId)`.
    pub fn festival_accumulated_bonus(&self, festival_id: i32) -> i32 {
        usize::try_from(festival_id)
            .ok()
            .and_then(|i| self.seven_signs.status.accumulated_bonus.get(i))
            .copied()
            .unwrap_or(0)
    }

    /// `getTotalAccumulatedBonus()`.
    pub fn festival_total_accumulated_bonus(&self) -> i32 {
        self.seven_signs
            .status
            .accumulated_bonus
            .iter()
            .fold(0i32, |a, b| a.wrapping_add(*b))
    }

    /// The final score of the last round (`getFinalScore`): `None` where Java's map has no entry.
    pub fn festival_final_score(&self, oracle: i32, festival_id: i32) -> Option<i64> {
        let map = if oracle == CABAL_DAWN {
            &self.seven_signs.festival.dawn_scores
        } else {
            &self.seven_signs.festival.dusk_scores
        };
        map.get(&festival_id).copied()
    }

    /// The rows of `seven_signs_festival` as they are in memory.
    pub fn festival_rows(&self) -> Vec<FestivalData> {
        self.seven_signs.festival
            .data
            .values()
            .flat_map(|c| c.values().cloned())
            .collect()
    }

    // ------------------------------------------------------------------ the rows

    /// Java `saveFestivalData(updateSettings)`: `REPLACE INTO` every row, then (with
    /// `updateSettings`) the status row. Global: only with `CycleDatabaseWrites` (D-53).
    pub(crate) fn festival_save(&mut self, update_settings: bool) {
        if !self.seven_signs.settings.cycle_database_writes {
            debug!("SideBySide: seven_signs_festival not saved (CycleDatabaseWrites = False)");
        } else {
            let rows = self.festival_rows();
            self.outbox
                .persist(PersistCommand::SevenSigns(SevenSignsWrite::ReplaceFestival(rows)));
        }
        // Updates Seven Signs DB data also, so call only if really necessary.
        if update_settings {
            self.ss_save_status();
        }
    }

    /// Java `resetFestivalData(updateSettings)`: the start of a new quest event period.
    pub(crate) fn festival_reset_data(&mut self, update_settings: bool) {
        self.seven_signs.status.festival_cycle = 0;
        self.seven_signs.festival.signs_cycle = self.ss_current_cycle();
        // Set all accumulated bonuses back to 0.
        self.seven_signs.status.accumulated_bonus = [0; 5];
        self.seven_signs.festival.dawn_participants.clear();
        self.seven_signs.festival.dawn_previous.clear();
        self.seven_signs.festival.dawn_scores.clear();
        self.seven_signs.festival.dusk_participants.clear();
        self.seven_signs.festival.dusk_previous.clear();
        self.seven_signs.festival.dusk_scores.clear();
        // Set up a new data set for the current cycle of festivals.
        let cycle = self.seven_signs.festival.signs_cycle;
        let mut new_data = BTreeMap::new();
        for i in 0..(FESTIVAL_COUNT as i32 * 2) {
            let festival_id = if i >= FESTIVAL_COUNT as i32 {
                i - FESTIVAL_COUNT as i32
            } else {
                i
            };
            new_data.insert(
                i,
                FestivalData {
                    festival_id,
                    cabal: cabal_short_name(if i >= FESTIVAL_COUNT as i32 {
                        CABAL_DAWN
                    } else {
                        CABAL_DUSK
                    })
                    .to_owned(),
                    cycle,
                    date: 0,
                    score: 0,
                    members: String::new(),
                },
            );
        }
        self.seven_signs.festival.data.insert(cycle, new_data);
        self.festival_save(update_settings);
        // Remove any unused blood offerings from online players.
        let mut ids: Vec<ObjectId> = self.players.keys().copied().collect();
        ids.sort_unstable();
        for id in ids {
            self.destroy_item_by_item_id(id, FESTIVAL_OFFERING_ID, -1, false);
        }
        info!("SevenSignsFestival: Reinitialized engine for next competition period.");
    }

    /// Java `rewardHighestRanked()`: the clans of the best party of each range get reputation.
    pub(crate) fn festival_reward_highest_ranked(&mut self) {
        for range in [
            FESTIVAL_LEVEL_MAX_31,
            FESTIVAL_LEVEL_MAX_42,
            FESTIVAL_LEVEL_MAX_53,
            FESTIVAL_LEVEL_MAX_64,
            FESTIVAL_LEVEL_MAX_NONE,
        ] {
            if let Some(overall) = self.festival_overall_highest_score_data(range) {
                for name in java_split_comma(&overall.members) {
                    self.festival_add_reputation_for_member_clan(&name);
                }
            }
        }
    }

    /// `addReputationPointsForPartyMemberClan(name)`.
    // The offline case is a database query in Java (`GET_CLAN_NAME`); the clan table already knows
    // the members of every clan by name, so it is answered from there (D-15: no query in a tick).
    fn festival_add_reputation_for_member_clan(&mut self, member_name: &str) {
        let points = self.seven_signs.settings.festival.win_points;
        let clan_id = match self.player_by_name(member_name) {
            Some(player) => self.clan_id_of(player),
            None => self
                .clans()
                .find(|c| c.members.iter().any(|m| m.name == member_name))
                .map_or(0, |c| c.id),
        };
        if clan_id == 0 || self.clan(clan_id).is_none() {
            return;
        }
        self.add_reputation_score(clan_id, points);
        let message = SystemMessage::new(
            SystemMessageId::ClanMemberC1WasAnActiveMemberOfTheHighestRankedPartyInTheFestivalOfDarknessS2PointsHaveBeenAddedToYourClanSReputationScore,
        )
        .with(SystemMessageParam::Text(member_name.to_owned()))
        .with(SystemMessageParam::IntNumber(points));
        self.broadcast_to_clan(clan_id, &message);
    }

    // ------------------------------------------------------------------ scores

    /// Java `SevenSigns.addFestivalScore(cabal, amount)`: the same number of points are deducted
    /// from the rival cabal.
    pub(crate) fn ss_add_festival_score(&mut self, cabal: i32, amount: i32) {
        let status = &mut self.seven_signs.status;
        if cabal == CABAL_DUSK {
            status.dusk_festival_score = status.dusk_festival_score.wrapping_add(amount);
            // To prevent negative scores!
            if status.dawn_festival_score >= amount {
                status.dawn_festival_score = status.dawn_festival_score.wrapping_sub(amount);
            }
        } else {
            status.dawn_festival_score = status.dawn_festival_score.wrapping_add(amount);
            if status.dusk_festival_score >= amount {
                status.dusk_festival_score = status.dusk_festival_score.wrapping_sub(amount);
            }
        }
    }

    /// The name of a character for the list of a party (`CharInfoTable.getNameById`, "null" for an
    /// unknown id, which is what `StringJoiner.add(null)` writes).
    fn festival_name_of(&self, id: ObjectId) -> String {
        if let Some(p) = self.players.get(&id) {
            return p.data.name.clone();
        }
        self.seven_signs.festival
            .names
            .get(&id)
            .cloned()
            .unwrap_or_else(|| "null".to_owned())
    }

    /// Java `setFinalScore(player, oracle, festivalId, offeringScore)`: `true` when the score is
    /// the best of this cycle for the cabal.
    pub fn festival_set_final_score(&mut self, oracle: i32, festival_id: i32, offering_score: i64) -> bool {
        let curr_dawn = self.festival_highest_score(CABAL_DAWN, festival_id);
        let curr_dusk = self.festival_highest_score(CABAL_DUSK, festival_id);
        let (this_cabal, other_cabal) = if oracle == CABAL_DAWN {
            self.seven_signs.festival.dawn_scores.insert(festival_id, offering_score);
            (curr_dawn, curr_dusk)
        } else {
            self.seven_signs.festival.dusk_scores.insert(festival_id, offering_score);
            (curr_dusk, curr_dawn)
        };
        // Check if this is the highest score for this level range so far for the player's cabal.
        if offering_score > i64::from(this_cabal) {
            // If the current score is greater than that for the other cabal, then they already
            // have the points from this festival.
            // NOTE(java-quirk): that also refuses a better score of the cabal that leads.
            if this_cabal > other_cabal {
                return false;
            }
            let previous: Vec<ObjectId> = self
                .festival_previous_participants(oracle, festival_id)
                .cloned()
                .unwrap_or_default();
            // Record a string list of the party members involved.
            let members = previous
                .iter()
                .map(|m| self.festival_name_of(*m))
                .collect::<Vec<_>>()
                .join(",");
            // Update the highest scores and party list.
            let offset = if oracle == CABAL_DAWN {
                festival_id + 5
            } else {
                festival_id
            };
            let cycle = self.seven_signs.festival.signs_cycle;
            // NOTE(java-quirk): without a data set for the cycle Java changes a blank copy that
            // is thrown away.
            if let Some(data) = self
                .seven_signs.festival
                .data
                .get_mut(&cycle)
                .and_then(|m| m.get_mut(&offset))
            {
                data.date = unix_ms_now();
                data.score = offering_score as i32;
                data.members = members;
            }
            // Only add the score to the cabal's overall if it's higher than the other cabal's.
            if offering_score > i64::from(other_cabal) {
                let contrib = FESTIVAL_LEVEL_SCORES[usize::try_from(festival_id).unwrap_or(0).min(4)];
                // Give this cabal the festival points, while deducting them from the other.
                self.ss_add_festival_score(oracle, contrib);
            }
            self.festival_save(true);
            return true;
        }
        false
    }

    /// Java `addAccumulatedBonus(festivalId, stoneType, stoneAmount)`.
    pub fn festival_add_accumulated_bonus(&mut self, festival_id: i32, stone_type: i32, stone_amount: i32) {
        let each = match stone_type {
            SEAL_STONE_BLUE_ID => SEAL_STONE_BLUE_VALUE,
            SEAL_STONE_GREEN_ID => SEAL_STONE_GREEN_VALUE,
            SEAL_STONE_RED_ID => SEAL_STONE_RED_VALUE,
            _ => 0,
        } as i32;
        if let Some(bonus) = usize::try_from(festival_id)
            .ok()
            .and_then(|i| self.seven_signs.status.accumulated_bonus.get_mut(i))
        {
            *bonus = bonus.wrapping_add(stone_amount.wrapping_mul(each));
        }
    }

    /// Java `distribAccumulatedBonus(player)`: the player's share of the bonus of the festival his
    /// name is in, when his cabal won.
    // NOTE(java-quirk): nothing is paid out; the share is taken off the accumulated bonus and only
    // the caller's page depends on it.
    pub fn festival_distrib_accumulated_bonus(&mut self, player: ObjectId) -> i32 {
        let player_cabal = self.ss_player_cabal(player.0);
        if player_cabal != self.ss_cabal_highest_score() {
            return 0;
        }
        let name = self
            .players
            .get(&player)
            .map(|p| p.data.name.clone())
            .unwrap_or_default();
        let cycle = self.seven_signs.festival.signs_cycle;
        let found = self.seven_signs.festival.data.get(&cycle).and_then(|m| {
            m.values().find(|d| d.members.contains(&name)).map(|d| {
                (
                    d.festival_id,
                    i32::try_from(java_split_comma(&d.members).len()).unwrap_or(i32::MAX),
                )
            })
        });
        let mut player_bonus = 0;
        if let Some((festival_id, members)) = found
            && let Some(slot) = usize::try_from(festival_id)
                .ok()
                .and_then(|i| self.seven_signs.status.accumulated_bonus.get_mut(i))
        {
            let total = *slot;
            player_bonus = total / members.max(1);
            *slot = total - player_bonus;
        }
        player_bonus
    }

    // ------------------------------------------------------------------ speech

    /// The oracle's chat guides (`npcSpawned`): the NPCs 31127 and 31137.
    fn festival_chat_guides(&self) -> Option<(ObjectId, ObjectId)> {
        let find = |npc_id: i32| {
            self.npcs
                .iter()
                .filter(|(_, n)| n.npc_id == npc_id)
                .map(|(id, _)| *id)
                .max()
        };
        Some((find(DAWN_CHAT_GUIDE)?, find(DUSK_CHAT_GUIDE)?))
    }

    /// `sendMessageToAll(npcString)`: both guides shout it.
    fn festival_send_message_to_all(&mut self, string: NpcStringId) {
        let Some((dawn, dusk)) = self.festival_chat_guides() else {
            return;
        };
        for npc in [dawn, dusk] {
            let mut parameters = Vec::new();
            if string.param_count() == 1 {
                parameters.push(self.festival_mins_to_next_festival().to_string());
            }
            let name = self.creature_name(npc);
            self.broadcast_packet(
                npc,
                &CreatureSay {
                    sender_object_id: npc.0,
                    chat_type: ChatType::NpcShout,
                    sender: SenderField::Name(name),
                    message_id: string.id(),
                    body: SayBody::Parameters(parameters),
                },
            );
        }
    }

    /// `sendMessageToParticipants(npcStringId)` of an arena: the witch says it.
    fn festival_instance_say(&mut self, key: i32, string: NpcStringId) {
        let Some((witch, has_participants)) = self.festival_witch_and_participants(key) else {
            return;
        };
        if !has_participants {
            return;
        }
        let name = self.creature_name(witch);
        self.broadcast_packet(
            witch,
            &CreatureSay {
                sender_object_id: witch.0,
                chat_type: ChatType::NpcGeneral,
                sender: SenderField::Name(name),
                message_id: string.id(),
                body: SayBody::Parameters(Vec::new()),
            },
        );
    }

    /// `sendMessageToParticipants(String)`: the witch says a text, as "Festival Witch".
    fn festival_instance_say_text(&mut self, key: i32, text: &str) {
        let Some((witch, has_participants)) = self.festival_witch_and_participants(key) else {
            return;
        };
        if !has_participants {
            return;
        }
        self.broadcast_packet(
            witch,
            &CreatureSay {
                sender_object_id: witch.0,
                chat_type: ChatType::NpcGeneral,
                sender: SenderField::Name("Festival Witch".to_owned()),
                message_id: -1,
                body: SayBody::Text(text.to_owned()),
            },
        );
    }

    fn festival_witch_and_participants(&self, key: i32) -> Option<(ObjectId, bool)> {
        let instance = self.seven_signs.festival.instances.get(&key)?;
        let witch = instance.witch_inst?;
        Some((witch, instance.participants.as_ref().is_some_and(|p| !p.is_empty())))
    }

    // ------------------------------------------------------------------ the arenas

    fn festival_new_instance(&mut self, cabal: i32, level_range: i32) -> DarknessFestival {
        let index = usize::try_from(level_range).unwrap_or(0).min(4);
        let (witch_row, player_row) = if cabal == CABAL_DAWN {
            (FESTIVAL_DAWN_WITCH_SPAWNS[index], FESTIVAL_DAWN_PLAYER_SPAWNS[index])
        } else {
            (FESTIVAL_DUSK_WITCH_SPAWNS[index], FESTIVAL_DUSK_PLAYER_SPAWNS[index])
        };
        let participants = if cabal == CABAL_DAWN {
            self.seven_signs.festival.dawn_participants.get(&level_range)
        } else {
            self.seven_signs.festival.dusk_participants.get(&level_range)
        }
        .cloned()
        .flatten()
        // FOR TESTING! (Java)
        .unwrap_or_default();
        // `FestivalSpawn(int[])`: "Generate a random heading if no positive one given."
        let witch_heading = if witch_row[3] < 0 {
            self.rng().below(65_536)
        } else {
            witch_row[3]
        };
        let witch_spawn = FestivalSpawn {
            heading: witch_heading,
            ..spawn_row(&witch_row)
        };
        let start_heading = if player_row[3] < 0 {
            self.rng().below(65_536)
        } else {
            player_row[3]
        };
        let start_location = FestivalSpawn {
            x: player_row[0],
            y: player_row[1],
            z: player_row[2],
            heading: start_heading,
            npc_id: -1,
        };
        DarknessFestival {
            cabal,
            level_range,
            challenge_increased: false,
            start_location,
            witch_spawn,
            witch_inst: None,
            witch_spawn_index: None,
            npc_insts: Vec::new(),
            participants: Some(participants),
            original_locations: BTreeMap::new(),
        }
    }

    /// A random place around the start location of an arena (`FESTIVAL_MAX_OFFSET_*`).
    fn festival_random_around(&mut self, center: &FestivalSpawn) -> (i32, i32) {
        let (mut x, mut y) = (center.x, center.y);
        if self.rng().next_bool() {
            x += self.rng().below(FESTIVAL_MAX_OFFSET_X);
            y += self.rng().below(FESTIVAL_MAX_OFFSET_Y);
        } else {
            x -= self.rng().below(FESTIVAL_MAX_OFFSET_X);
            y -= self.rng().below(FESTIVAL_MAX_OFFSET_Y);
        }
        (x, y)
    }

    /// A `new Spawn(npcId)` at a place with `startRespawn()`: the spawn index and the NPC.
    fn festival_spawn(
        &mut self,
        npc_id: i32,
        place: (i32, i32, i32),
        heading: i32,
        respawn_secs: i32,
    ) -> Option<(usize, ObjectId)> {
        // `new Spawn(npcId)` throws for an unknown template or a class Java cannot build.
        let constructible = self
            .npc_world
            .template(npc_id)
            .is_some_and(|t| crate::npc::NpcClass::from_type(&t.npc_type).is_some());
        if !constructible {
            return None;
        }
        // `setRespawnDelay(delay)`: `Math.max(1, delay) * 1000`.
        let delay_ms = respawn_secs.max(1).saturating_mul(1000);
        let index = self.add_runtime_spawn(l2_data::spawn::NpcSpawn {
            npc_id,
            amount: 1,
            location: l2_data::Location {
                x: place.0,
                y: place.1,
                z: place.2,
            },
            heading,
            respawn_min_ms: delay_ms,
            respawn_max_ms: delay_ms,
            // A new `Spawn` has `_doRespawn = true` and the festival calls `startRespawn()`.
            respawn_enabled: true,
            chase_range: 0,
            name: None,
            territory: None,
            period_of_day: None,
            ai_data: None,
            group: None,
            source: "festival".to_owned(),
        });
        let mut report = crate::npc::SpawnReport::default();
        let npc = self.initialize_npc(index, None, &mut report)?;
        self.start_respawn(index);
        Some((index, npc))
    }

    /// `festivalInit()`: the participants are teleported in, the witch comes.
    fn festival_init_instance(&mut self, key: i32) {
        let Some(instance) = self.seven_signs.festival.instances.get(&key) else {
            return;
        };
        let participants = instance.participants.clone().unwrap_or_default();
        let start = instance.start_location;
        let witch = instance.witch_spawn;
        // Teleport all players to arena and notify them.
        for participant in &participants {
            if !self.players.contains_key(participant) {
                continue;
            }
            let (px, py, pz) = self.position_of(*participant).unwrap_or((0, 0, 0));
            let heading = self.heading_of(*participant);
            if let Some(i) = self.seven_signs.festival.instances.get_mut(&key) {
                i.original_locations.insert(*participant, (px, py, pz, heading));
            }
            // Randomize the spawn point around the specific centerpoint for each player.
            let (x, y) = self.festival_random_around(&start);
            self.set_intention_idle(*participant);
            let offset = self.settings.teleport.max_offset_on_teleport;
            self.tele_to_location(
                *participant,
                TeleportTarget::at(x, y, start.z).with_random_offset(offset),
            );
            // Remove all buffs from all participants on entry. Works like the skill Cancel.
            self.stop_all_effects_except_those_that_last_through_death(*participant);
            // Remove any stray blood offerings in inventory.
            self.destroy_item_by_item_id(*participant, FESTIVAL_OFFERING_ID, -1, true);
        }
        // Spawn the festival witch for this arena.
        match self.festival_spawn(witch.npc_id, (witch.x, witch.y, witch.z), witch.heading, 1) {
            Some((index, npc)) => {
                if let Some(i) = self.seven_signs.festival.instances.get_mut(&key) {
                    i.witch_inst = Some(npc);
                    i.witch_spawn_index = Some(index);
                }
                // Make it appear as though the Witch has apparated there. And another one...:D
                for skill in [WITCH_MAGIC_SKILL_1, WITCH_MAGIC_SKILL_2] {
                    self.festival_witch_skill_effect(npc, skill);
                }
            }
            None => warn!(
                "SevenSignsFestival: Error while spawning Festival Witch ID {}",
                witch.npc_id
            ),
        }
        // Send a message to all participants from the witch.
        self.festival_instance_say(key, NpcStringId::TheMainEventWillStartIn2MinutesPleaseRegisterNow);
    }

    /// `new MagicSkillUse(witch, witch, skill, 1, 1, 0)` to everybody who sees the witch.
    fn festival_witch_skill_effect(&self, witch: ObjectId, skill: i32) {
        let Some((x, y, z)) = self.position_of(witch) else {
            return;
        };
        let at = l2_protocol_game::server::SkillPosition { x, y, z };
        self.broadcast_packet(
            witch,
            &MagicSkillUse {
                creature_id: witch.0,
                target_id: witch.0,
                skill_id: skill,
                skill_level: 1,
                hit_time: 1,
                reuse_delay: 0,
                creature: at,
                ground_location: None,
                target: at,
            },
        );
    }

    /// `festivalStart()`.
    fn festival_start_instance(&mut self, key: i32) {
        self.festival_spawn_monsters(key, FESTIVAL_DEFAULT_RESPAWN, 0);
    }

    /// `spawnFestivalMonsters(respawnDelay, spawnType)`: 0 the primary monsters, 1 the same
    /// without the archers (the increased challenge), 2 the secondary monsters (archers), 3 the
    /// chests.
    fn festival_spawn_monsters(&mut self, key: i32, respawn_delay: i32, spawn_type: i32) {
        let Some(instance) = self.seven_signs.festival.instances.get(&key) else {
            return;
        };
        let (cabal, range) = (instance.cabal, usize::try_from(instance.level_range).unwrap_or(0).min(4));
        let dawn = cabal == CABAL_DAWN;
        let table: &[[i32; 5]] = match spawn_type {
            0 | 1 => {
                if dawn {
                    FESTIVAL_DAWN_PRIMARY_SPAWNS[range]
                } else {
                    FESTIVAL_DUSK_PRIMARY_SPAWNS[range]
                }
            }
            2 => {
                if dawn {
                    FESTIVAL_DAWN_SECONDARY_SPAWNS[range]
                } else {
                    FESTIVAL_DUSK_SECONDARY_SPAWNS[range]
                }
            }
            3 => {
                if dawn {
                    FESTIVAL_DAWN_CHEST_SPAWNS[range]
                } else {
                    FESTIVAL_DUSK_CHEST_SPAWNS[range]
                }
            }
            _ => return,
        };
        for row in table {
            let mut current = spawn_row(row);
            // Only spawn archers/marksmen if specified to do so.
            if spawn_type == 1 && is_festival_archer(current.npc_id) {
                continue;
            }
            // `FestivalSpawn(int[])`: a negative heading is a random one; the spawn gets its own
            // random heading all the same.
            if current.heading < 0 {
                current.heading = self.rng().below(65_536);
            }
            let heading = self.rng().below(65_536);
            let Some((index, npc)) =
                self.festival_spawn(current.npc_id, (current.x, current.y, current.z), heading, respawn_delay)
            else {
                warn!("SevenSignsFestival: Error while spawning NPC ID {}", current.npc_id);
                continue;
            };
            // Set the offering bonus to 2x or 5x the amount per kill, if this spawn is part of an
            // increased challenge or is a festival chest.
            if spawn_type == 1 {
                self.seven_signs.festival.monsters.insert(npc, 2);
            } else if spawn_type == 3 {
                self.seven_signs.festival.monsters.insert(npc, 5);
            }
            if let Some(i) = self.seven_signs.festival.instances.get_mut(&key) {
                i.npc_insts.push((npc, index));
            }
        }
    }

    /// `moveMonstersToCenter()`: the monsters that are idle or active run to the center.
    fn festival_move_monsters_to_center(&mut self, key: i32) {
        let Some(instance) = self.seven_signs.festival.instances.get(&key) else {
            return;
        };
        let start = instance.start_location;
        let mobs: Vec<ObjectId> = instance.npc_insts.iter().map(|(npc, _)| *npc).collect();
        for mob in mobs {
            if !self.npcs.contains_key(&mob) || self.is_dead(mob) {
                continue;
            }
            // Only move monsters that are idle or doing their usual functions.
            let intention = self.intention_of(mob);
            if intention != Intention::Idle && intention != Intention::Active {
                continue;
            }
            // Random X and Y coords around the player start location, up to half of the maximum
            // allowed offset are generated to prevent the mobs from all moving to the exact same
            // place.
            let (x, y) = self.festival_random_around(&start);
            // `Location(x, y, z, Rnd.get(65536))`: the heading is drawn and not used by the move.
            let _heading = self.rng().below(65_536);
            self.set_running(mob, true);
            self.npc_set_intention_move_to(mob, x, y, start.z);
        }
    }

    /// `increaseChallenge()` of the arena: once per round.
    fn festival_instance_increase_challenge(&mut self, key: i32) -> bool {
        let Some(instance) = self.seven_signs.festival.instances.get_mut(&key) else {
            return false;
        };
        if instance.challenge_increased {
            return false;
        }
        // Set this flag to true to make sure that this can only be done once.
        instance.challenge_increased = true;
        // Spawn more festival monsters, but this time with a twist.
        self.festival_spawn_monsters(key, FESTIVAL_DEFAULT_RESPAWN, 1);
        true
    }

    /// `increaseChallenge(oracle, festivalId)`: `false` where Java would throw (no running arena).
    pub fn festival_increase_challenge(&mut self, oracle: i32, festival_id: i32) -> bool {
        match self.festival_instance_key(oracle, festival_id) {
            Some(key) => self.festival_instance_increase_challenge(key),
            None => false,
        }
    }

    /// The witch of a running arena (tests and tools).
    pub fn festival_witch(&self, oracle: i32, festival_id: i32) -> Option<ObjectId> {
        let key = self.festival_instance_key(oracle, festival_id)?;
        self.seven_signs.festival.instances.get(&key)?.witch_inst
    }

    /// The monsters and chests of a running arena (tests and tools).
    pub fn festival_monsters(&self, oracle: i32, festival_id: i32) -> Vec<ObjectId> {
        self.festival_instance_key(oracle, festival_id)
            .and_then(|key| self.seven_signs.festival.instances.get(&key))
            .map(|i| i.npc_insts.iter().map(|(npc, _)| *npc).collect())
            .unwrap_or_default()
    }

    /// `getFestivalInstance(oracle, festivalIdValue)`: the key of the arena, `None` without a
    /// running festival or participants.
    fn festival_instance_key(&self, oracle: i32, festival_id: i32) -> Option<i32> {
        if !self.seven_signs.festival.initialized {
            return None;
        }
        let key = festival_id + if oracle == CABAL_DUSK { 10 } else { 20 };
        self.seven_signs.festival.instances.contains_key(&key).then_some(key)
    }

    /// `festivalEnd()`: the party is sent out, the arena emptied.
    fn festival_end_instance(&mut self, key: i32) {
        let Some(instance) = self.seven_signs.festival.instances.get(&key) else {
            return;
        };
        let participants = instance.participants.clone();
        let (cabal, range) = (instance.cabal, instance.level_range);
        if let Some(participants) = participants.filter(|p| !p.is_empty()) {
            for participant in &participants {
                if !self.players.contains_key(participant) {
                    continue;
                }
                self.festival_relocate_player(key, *participant, false);
                self.send_message(
                    *participant,
                    "The festival has ended. Your party leader must now register your score before the next festival takes place.",
                );
            }
            if cabal == CABAL_DAWN {
                self.seven_signs.festival.dawn_previous.insert(range, participants);
            } else {
                self.seven_signs.festival.dusk_previous.insert(range, participants);
            }
        }
        if let Some(instance) = self.seven_signs.festival.instances.get_mut(&key) {
            instance.participants = None;
        }
        self.festival_unspawn_mobs(key);
    }

    /// `unspawnMobs()`: the witch and the monsters leave and do not come back.
    fn festival_unspawn_mobs(&mut self, key: i32) {
        let Some(instance) = self.seven_signs.festival.instances.get_mut(&key) else {
            return;
        };
        let witch = instance.witch_inst.take().zip(instance.witch_spawn_index.take());
        let mobs = std::mem::take(&mut instance.npc_insts);
        if let Some((npc, index)) = witch {
            self.stop_respawn(index);
            self.delete_npc(npc);
        }
        for (npc, index) in mobs {
            self.stop_respawn(index);
            self.delete_npc(npc);
            self.seven_signs.festival.monsters.remove(&npc);
        }
    }

    fn festival_unspawn_all(&mut self) {
        for key in self.festival_instance_keys() {
            self.festival_unspawn_mobs(key);
        }
    }

    /// `relocatePlayer(participant, isRemoving)`: back to the place he came from, or to the town.
    fn festival_relocate_player(&mut self, key: i32, participant: ObjectId, is_removing: bool) {
        let original = self
            .seven_signs.festival
            .instances
            .get_mut(&key)
            .and_then(|i| {
                if is_removing {
                    i.original_locations.remove(&participant)
                } else {
                    i.original_locations.get(&participant).copied()
                }
            });
        self.set_intention_idle(participant);
        match original {
            Some((x, y, z, _)) => {
                let offset = self.settings.teleport.max_offset_on_teleport;
                self.tele_to_location(participant, TeleportTarget::at(x, y, z).with_random_offset(offset));
            }
            // If an exception occurs, just move the player to the nearest town.
            None => self.tele_to_location_where(participant, TeleportWhereType::Town),
        }
        self.send_message(participant, "You have been removed from the festival arena.");
    }

    /// `updateParticipants(player, festivalParty)`: a member left (or went away from) the party.
    pub(crate) fn festival_update_participants(
        &mut self,
        player: ObjectId,
        party: Option<crate::party::PartyId>,
    ) {
        if !self.is_festival_participant(player) {
            return;
        }
        let (oracle, festival_id) = self.festival_for_player(player);
        if festival_id <= -1 {
            return;
        }
        if self.seven_signs.festival.initialized
            && let Some(key) = self.festival_instance_key(oracle, festival_id)
        {
            match party {
                None => {
                    let members: Vec<ObjectId> = self
                        .festival_participants(oracle, festival_id)
                        .cloned()
                        .unwrap_or_default();
                    for member in members {
                        if self.players.contains_key(&member) {
                            self.festival_relocate_player(key, member, true);
                        }
                    }
                }
                Some(_) => self.festival_relocate_player(key, player, true),
            }
        }
        self.festival_set_participants(oracle, festival_id, party);
        // Check on disconnect if min player in party.
        if let Some(party) = party {
            let min = self.seven_signs.settings.festival.min_player;
            let count = i32::try_from(self.party_members(party).len()).unwrap_or(i32::MAX);
            if count < min {
                // Under minimum count.
                self.festival_update_participants(player, None);
                // NOTE(java-quirk): the player is out of the party already, so this does nothing.
                self.remove_party_member(party, player, PartyMessageType::Expelled);
            }
        }
    }

    // ------------------------------------------------------------------ the monsters

    /// `FestivalMonster.doItemDrop(lastAttacker)`: a blood offering for the leader of the killer's
    /// party. The normal drop follows (the caller).
    /// `false` when the monster gives no drop at all.
    // NOTE(java-quirk): a killer who is no player (a summon too) or has no party ends the method
    // before `super.doItemDrop`, so the monster drops nothing then.
    pub(crate) fn festival_monster_drop(&mut self, monster: ObjectId, last_attacker: Option<ObjectId>) -> bool {
        if !self.is_festival_monster(monster) {
            return true;
        }
        let Some(attacker) = last_attacker.filter(|a| self.players.contains_key(a)) else {
            return false;
        };
        let Some(party) = self.party_of(attacker) else {
            return false;
        };
        let Some(leader) = self.party_leader(party) else {
            return false;
        };
        let bonus = i64::from(self.seven_signs.festival.monsters.get(&monster).copied().unwrap_or(1));
        self.add_item(leader, FESTIVAL_OFFERING_ID, bonus, false);
        true
    }

    /// The festival modifier of `Formulas.calcHpRegen` / `calcMpRegen`: `Some` for a participant
    /// while a festival is in progress (`calcFestivalRegenModifier`).
    // NOTE(java-quirk): `getFestivalForPlayer` finds only some ranges (see there), and `-1` makes
    // the modifier 0, which stops the regeneration of everybody who is in another range.
    pub(crate) fn festival_regen_modifier(&self, player: ObjectId) -> Option<f64> {
        if !(self.seven_signs.festival.in_progress && self.is_festival_participant(player)) {
            return None;
        }
        let (oracle, festival_id) = self.festival_for_player(player);
        // If the player isn't found in the festival, leave the regen rate as it is.
        if festival_id < 0 {
            return Some(0.0);
        }
        let index = usize::try_from(festival_id).unwrap_or(0).min(4);
        let center = if oracle == CABAL_DAWN {
            FESTIVAL_DAWN_PLAYER_SPAWNS[index]
        } else {
            FESTIVAL_DUSK_PLAYER_SPAWNS[index]
        };
        let (x, y, _) = self.position_of(player)?;
        // Check the distance between the player and the player spawn point, in the center of the
        // arena.
        let dist = crate::target::distance_2d((x, y, 0), (center[0], center[1], 0));
        // Maximum decreased regen of ~ -65%.
        Some(1.0 - (dist * 0.0005))
    }

    /// `FestivalMonster.isAggressive()` is always true.
    pub(crate) fn is_festival_monster(&self, npc: ObjectId) -> bool {
        self.npc_template(npc)
            .is_some_and(|t| t.npc_type == FESTIVAL_MONSTER_TYPE)
    }

    // ------------------------------------------------------------------ logout and restart

    /// `Player.canLogout()`'s festival part: `false` (with Java's message) while the festival
    /// runs; before it, the party hears the player is out of the upcoming festival.
    pub(crate) fn festival_blocks_logout(&mut self, player: ObjectId) -> bool {
        if !self.is_festival_participant(player) {
            return false;
        }
        if self.seven_signs.festival.initialized {
            self.send_message(player, "You cannot log out while you are a participant in a Festival.");
            return true;
        }
        self.festival_tell_party_removed(player);
        false
    }

    /// `RequestRestart`'s festival part.
    pub(crate) fn festival_blocks_restart(&mut self, player: ObjectId) -> bool {
        if !self.is_festival_participant(player) {
            return false;
        }
        if self.seven_signs.festival.initialized {
            self.send_message(player, "You cannot restart while you are a participant in a festival.");
            return true;
        }
        self.festival_tell_party_removed(player);
        false
    }

    fn festival_tell_party_removed(&mut self, player: ObjectId) {
        if let Some(party) = self.party_of(player) {
            let name = self
                .players
                .get(&player)
                .map(|p| p.data.name.clone())
                .unwrap_or_default();
            self.broadcast_party(
                party,
                &SystemMessage::text(&format!("{name} has been removed from the upcoming Festival.")),
            );
        }
    }

    // ------------------------------------------------------------------ the SSQStatus page

    /// The best scores of the five festivals for `SSQStatus` page 2.
    pub(crate) fn festival_high_scores(&self) -> Vec<l2_protocol_game::server::ssq::FestivalHighScore> {
        (0..FESTIVAL_COUNT as i32)
            .map(|i| {
                let dusk = self.festival_highest_score_data(CABAL_DUSK, i);
                let dawn = self.festival_highest_score_data(CABAL_DAWN, i);
                l2_protocol_game::server::ssq::FestivalHighScore {
                    dusk_score: i64::from(dusk.score),
                    dusk_members: java_split_comma(&dusk.members),
                    dawn_score: i64::from(dawn.score),
                    dawn_members: java_split_comma(&dawn.members),
                }
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------------- the guide

/// `Festival.java`'s `calculateDate(milliFromEpoch)`: `year/month/day` of the local date, with the
/// month counted from 0 (Java's `Calendar.MONTH`).
fn calculate_date(millis: i64) -> String {
    let local = millis + l2_util::time::local_offset_millis(millis);
    let days = local.div_euclid(86_400_000);
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { year + 1 } else { year };
    // `Calendar.get(MONTH)` is 0 based.
    format!("{year}/{}/{day}", month - 1)
}

impl World {
    /// The festival guide an NPC is (`None` for any other NPC; the template type decides).
    fn festival_guide_of(&self, npc: ObjectId) -> Option<GuideInfo> {
        let n = self.npcs.get(&npc)?;
        let template = self.npc_world.template(n.npc_id)?;
        (template.npc_type == "FestivalGuide").then(|| GuideInfo::of_npc(n.npc_id))
    }

    /// `FestivalGuide.showChatWindow(player, value, suffix, isDescription)`.
    pub(crate) fn festival_show_chat_window(
        &mut self,
        npc: ObjectId,
        player: ObjectId,
        value: i32,
        suffix: Option<&str>,
        is_description: bool,
    ) {
        let Some(guide) = self.festival_guide_of(npc) else {
            return;
        };
        let mut filename = format!(
            "{SEVEN_SIGNS_HTML_PATH}festival/{}",
            if is_description { "desc_" } else { "festival_" }
        );
        match suffix {
            Some(suffix) => filename.push_str(&format!("{value}{suffix}.htm")),
            None => filename.push_str(&format!("{value}.htm")),
        }
        let Some(mut html) = self.html_from_file(player, npc.0, &filename) else {
            return;
        };
        let mut page = html.html.clone();
        let replace = |page: &mut String, key: &str, with: &str| *page = page.replace(key, with);
        replace(&mut page, "%objectId%", &npc.0.to_string());
        replace(&mut page, "%festivalType%", festival_name(guide.festival_type));
        replace(
            &mut page,
            "%cycleMins%",
            &self.festival_mins_to_next_cycle().to_string(),
        );
        if !is_description && suffix.is_some_and(|s| format!("{value}{s}") == "2b") {
            replace(
                &mut page,
                "%minFestivalPartyMembers%",
                &self.seven_signs.settings.festival.min_player.to_string(),
            );
        }
        // If the stats or bonus table is required, construct them.
        if value == 5 {
            let table = self.festival_stats_table();
            replace(&mut page, "%statsTable%", &table);
        }
        if value == 6 {
            let table = self.festival_bonus_table();
            replace(&mut page, "%bonusTable%", &table);
        }
        // Festival's fee.
        if value == 1 {
            replace(&mut page, "%blueStoneNeeded%", &guide.blue.to_string());
            replace(&mut page, "%greenStoneNeeded%", &guide.green.to_string());
            replace(&mut page, "%redStoneNeeded%", &guide.red.to_string());
        }
        html.html = page;
        self.send_html(player, &html);
        // ActionFailed, "to avoid that the client wait another packet".
        self.send(player, &ActionFailed);
    }

    /// `FestivalGuide.getStatsTable()`.
    fn festival_stats_table(&self) -> String {
        let mut table = String::new();
        // Get the scores for each of the festival level ranges (types).
        for i in 0..FESTIVAL_COUNT as i32 {
            let dawn_score = self.festival_highest_score(CABAL_DAWN, i);
            let dusk_score = self.festival_highest_score(CABAL_DUSK, i);
            let winning = if dawn_score > dusk_score {
                "Children of Dawn"
            } else if dawn_score == dusk_score {
                "None"
            } else {
                "Children of Dusk"
            };
            table.push_str(&format!(
                "<tr><td width=\"100\" align=\"center\">{}</td><td align=\"center\" width=\"35\">{dusk_score}</td><td align=\"center\" width=\"35\">{dawn_score}</td><td align=\"center\" width=\"130\">{winning}</td></tr>",
                festival_name(i)
            ));
        }
        table
    }

    /// `FestivalGuide.getBonusTable()`.
    fn festival_bonus_table(&self) -> String {
        let mut table = String::new();
        for i in 0..FESTIVAL_COUNT as i32 {
            table.push_str(&format!(
                "<tr><td align=\"center\" width=\"150\">{}</td><td align=\"center\" width=\"150\">{}</td></tr>",
                festival_name(i),
                self.festival_accumulated_bonus(i)
            ));
        }
        table
    }

    /// Case 4 of `Festival.java`: the page with the best scores of the guide's range.
    fn festival_scores_page(&mut self, npc: ObjectId, player: ObjectId) {
        let Some(guide) = self.festival_guide_of(npc) else {
            return;
        };
        let dawn = self.festival_highest_score_data(CABAL_DAWN, guide.festival_type);
        let dusk = self.festival_highest_score_data(CABAL_DUSK, guide.festival_type);
        let overall = self.festival_overall_highest_score_data(guide.festival_type);
        let mut page = String::from(
            "<html><body>Festival Guide:<br>These are the top scores of the week, for the ",
        );
        page.push_str(&format!("{} festival.<br>", festival_name(guide.festival_type)));
        if dawn.score > 0 {
            page.push_str(&format!(
                "Dawn: {}. Score {}<br>{}<br>",
                calculate_date(dawn.date),
                dawn.score,
                dawn.members
            ));
        } else {
            page.push_str("Dawn: No record exists. Score 0<br>");
        }
        if dusk.score > 0 {
            page.push_str(&format!(
                "Dusk: {}. Score {}<br>{}<br>",
                calculate_date(dusk.date),
                dusk.score,
                dusk.members
            ));
        } else {
            page.push_str("Dusk: No record exists. Score 0<br>");
        }
        match overall.filter(|o| o.score > 0) {
            Some(overall) => {
                let side = if overall.cabal == "dawn" {
                    "Children of Dawn"
                } else {
                    "Children of Dusk"
                };
                page.push_str(&format!(
                    "Consecutive top scores: {}. Score {}<br>Affilated side: {}<br>{}<br>",
                    calculate_date(overall.date),
                    overall.score,
                    side,
                    overall.members
                ));
            }
            None => page.push_str("Consecutive top scores: No record exists. Score 0<br>"),
        }
        page.push_str(&format!(
            "<a action=\"bypass -h npc_{}_Chat 0\">Go back.</a></body></html>",
            npc.0
        ));
        if let Some(message) = l2_protocol_game::server::NpcHtmlMessage::new(npc.0, &page) {
            self.send_html(player, &message);
        }
    }

    /// What the pages 4, 5 and 6 show once the rows were read again (D-50).
    pub(crate) fn festival_show_page(&mut self, npc: ObjectId, player: ObjectId, value: i32) {
        if !self.players.contains_key(&player) || !self.npcs.contains_key(&npc) {
            return;
        }
        if value == 4 {
            self.festival_scores_page(npc, player);
        } else {
            self.festival_show_chat_window(npc, player, value, None, false);
        }
    }

    /// The bypass `Festival` / `FestivalDesc` (`Festival.java`, registered as a bypass handler).
    pub(crate) fn festival_bypass(&mut self, player: ObjectId, command: &str, target: Option<ObjectId>) -> bool {
        let Some(npc) = target else {
            return false;
        };
        let Some(guide) = self.festival_guide_of(npc) else {
            return false;
        };
        let lower = command.to_lowercase();
        if lower.starts_with("festivaldesc") {
            let Some(value) = command.get(13..).and_then(|v| v.parse::<i32>().ok()) else {
                warn!("Exception in Festival: bad command {command}");
                return false;
            };
            self.festival_show_chat_window(npc, player, value, None, true);
            return true;
        }
        let Some(value) = command.get(9..10).and_then(|v| v.parse::<i32>().ok()) else {
            warn!("Exception in Festival: bad command {command}");
            return false;
        };
        match value {
            1 => {
                // Become a participant.
                // Check if the festival period is active, if not then don't allow registration.
                if self.ss_is_seal_validation_period() {
                    self.festival_show_chat_window(npc, player, 2, Some("a"), false);
                    return true;
                }
                // Check if a festival is in progress, then don't allow registration yet.
                if self.seven_signs.festival.initialized {
                    self.send_message(player, "You cannot sign up while a festival is in progress.");
                    return true;
                }
                // Check if the player is in a formed party already.
                let Some(party) = self.party_of(player) else {
                    self.festival_show_chat_window(npc, player, 2, Some("b"), false);
                    return true;
                };
                // Check if the player is the party leader.
                if self.party_leader(party) != Some(player) {
                    self.festival_show_chat_window(npc, player, 2, Some("c"), false);
                    return true;
                }
                // Check to see if the party has at least 5 members.
                let members = i32::try_from(self.party_members(party).len()).unwrap_or(i32::MAX);
                if members < self.seven_signs.settings.festival.min_player {
                    self.festival_show_chat_window(npc, player, 2, Some("b"), false);
                    return true;
                }
                // Check if all the party members are in the required level range.
                let max_level = max_level_for_festival(guide.festival_type, self.java_max_level().unwrap_or(86));
                if self.party_level(party).unwrap_or(0) > max_level {
                    self.festival_show_chat_window(npc, player, 2, Some("d"), false);
                    return true;
                }
                // Check to see if the player has already signed up.
                if self.is_festival_participant(player) {
                    self.festival_set_participants(guide.oracle, guide.festival_type, Some(party));
                    self.festival_show_chat_window(npc, player, 2, Some("f"), false);
                    return true;
                }
                self.festival_show_chat_window(npc, player, 1, None, false);
            }
            2 => {
                // Seal stones.
                let Some(stone_type) = command.get(11..).and_then(|v| v.parse::<i32>().ok()) else {
                    warn!("Exception in Festival: bad command {command}");
                    return false;
                };
                let stone_count = guide.stone_count(stone_type);
                if stone_count <= 0 {
                    return false;
                }
                if !self.destroy_item_by_item_id(player, stone_type, i64::from(stone_count), true) {
                    return false;
                }
                let party = self.party_of(player);
                self.festival_set_participants(guide.oracle, guide.festival_type, party);
                self.festival_add_accumulated_bonus(guide.festival_type, stone_type, stone_count);
                self.festival_show_chat_window(npc, player, 2, Some("e"), false);
            }
            3 => return self.festival_score_registration(npc, player, guide),
            4..=6 => {
                // The score tables: with the rows of the Java game server (D-50, D-53).
                if self.seven_signs.settings.cycle_database_writes {
                    self.festival_show_page(npc, player, value);
                } else {
                    self.ss_request_read(SevenSignsThen::FestivalPage {
                        player,
                        npc,
                        page: value,
                    });
                }
            }
            8 => {
                // Increase the festival challenge.
                let Some(party) = self.party_of(player) else {
                    return true;
                };
                if !self.seven_signs.festival.in_progress {
                    return true;
                }
                if self.party_leader(party) != Some(player) {
                    self.festival_show_chat_window(npc, player, 8, Some("a"), false);
                    return true;
                }
                if self.festival_increase_challenge(guide.oracle, guide.festival_type) {
                    self.festival_show_chat_window(npc, player, 8, Some("b"), false);
                } else {
                    self.festival_show_chat_window(npc, player, 8, Some("c"), false);
                }
            }
            9 => {
                // Leave the festival.
                let Some(party) = self.party_of(player) else {
                    return true;
                };
                if self.party_leader(party) == Some(player) {
                    self.festival_update_participants(player, None);
                } else {
                    let count = i32::try_from(self.party_members(party).len()).unwrap_or(i32::MAX);
                    if count > self.seven_signs.settings.festival.min_player {
                        self.remove_party_member(party, player, PartyMessageType::Expelled);
                    } else {
                        self.send_message(
                            player,
                            "Only the party leader can leave a festival when a party has minimum number of members.",
                        );
                    }
                }
            }
            0 => {
                // Distribute accumulated bonus.
                if !self.ss_is_seal_validation_period() {
                    self.send_message(player, "Bonuses cannot be paid during the competition period.");
                    return true;
                }
                if self.festival_distrib_accumulated_bonus(player) > 0 {
                    self.festival_show_chat_window(npc, player, 0, Some("a"), false);
                } else {
                    self.festival_show_chat_window(npc, player, 0, Some("b"), false);
                }
            }
            _ => self.festival_show_chat_window(npc, player, value, None, false),
        }
        true
    }

    /// Case 3 of `Festival.java`: the party leader registers the blood offerings as a score.
    fn festival_score_registration(&mut self, npc: ObjectId, player: ObjectId, guide: GuideInfo) -> bool {
        // Check if the festival period is active, if not then don't register the score.
        if self.ss_is_seal_validation_period() {
            self.festival_show_chat_window(npc, player, 3, Some("a"), false);
            return true;
        }
        // Check if a festival is in progress, if it is don't register the score.
        if self.seven_signs.festival.in_progress {
            self.send_message(player, "You cannot register a score while a festival is in progress.");
            return true;
        }
        // Check if the player is in a party.
        if self.party_of(player).is_none() {
            self.festival_show_chat_window(npc, player, 3, Some("b"), false);
            return true;
        }
        let previous = self
            .festival_previous_participants(guide.oracle, guide.festival_type)
            .cloned();
        // Check if there are any past participants, and if this player was the party leader in
        // the festival.
        let Some(previous) = previous.filter(|p| !p.is_empty() && p.contains(&player)) else {
            self.festival_show_chat_window(npc, player, 3, Some("b"), false);
            return true;
        };
        if previous.first() != Some(&player) {
            self.festival_show_chat_window(npc, player, 3, Some("b"), false);
            return true;
        }
        // Check if the player collected any blood offerings during the festival.
        let offerings = self.inventory_item_count(player, FESTIVAL_OFFERING_ID);
        if offerings <= 0 {
            self.send_message(player, "You do not have any blood offerings to contribute.");
            return true;
        }
        let offering_score = offerings * FESTIVAL_OFFERING_VALUE;
        if !self.destroy_item_by_item_id(player, FESTIVAL_OFFERING_ID, -1, false) {
            return true;
        }
        let is_highest = self.festival_set_final_score(guide.oracle, guide.festival_type, offering_score);
        self.send(
            player,
            &SystemMessage::new(SystemMessageId::YourContributionScoreHasIncreasedByS1)
                .with(SystemMessageParam::LongNumber(offering_score)),
        );
        if is_highest {
            self.festival_show_chat_window(npc, player, 3, Some("c"), false);
        } else {
            self.festival_show_chat_window(npc, player, 3, Some("d"), false);
        }
        true
    }
}

/// The handler of `bypass Festival ...` and `bypass FestivalDesc ...`.
pub fn festival_bypass_handler(world: &mut World, player: ObjectId, command: &str, target: Option<ObjectId>) -> bool {
    world.festival_bypass(player, command, target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_is_javas() {
        assert_eq!(java_split_comma(""), vec![String::new()]);
        assert_eq!(java_split_comma("a,b"), vec!["a".to_owned(), "b".to_owned()]);
        assert_eq!(java_split_comma("a,b,,"), vec!["a".to_owned(), "b".to_owned()]);
    }

    #[test]
    fn the_guides_know_their_range_and_fee() {
        let g = GuideInfo::of_npc(31133);
        assert_eq!((g.festival_type, g.oracle, g.blue, g.green, g.red), (1, CABAL_DAWN, 1500, 900, 450));
        assert_eq!(g.stone_count(SEAL_STONE_GREEN_ID), 900);
        assert_eq!(g.stone_count(57), -1);
        let none = GuideInfo::of_npc(1);
        assert_eq!((none.oracle, none.blue), (CABAL_NULL, 0));
    }

    #[test]
    fn the_archers_end_in_four_or_nine() {
        assert!(!is_festival_archer(18004));
        assert!(is_festival_archer(18019));
        assert!(is_festival_archer(18104));
        assert!(!is_festival_archer(18010));
        assert!(!is_festival_archer(18109));
    }

    #[test]
    fn the_dates_count_the_month_from_zero() {
        // 2026-01-07 12:00 UTC.
        let utc = 1_767_571_200_000 + 2 * 86_400_000 + 12 * 3_600_000;
        let text = calculate_date(utc);
        assert!(text.starts_with("2026/0/") || text.starts_with("2026/1/"), "{text}");
    }

    #[test]
    fn the_level_ranges_have_their_names() {
        assert_eq!(festival_name(0), "Level 31 or lower");
        assert_eq!(festival_name(4), "No Level Limit");
        assert_eq!(max_level_for_festival(3, 86), 64);
        assert_eq!(max_level_for_festival(4, 86), 85);
    }
}
