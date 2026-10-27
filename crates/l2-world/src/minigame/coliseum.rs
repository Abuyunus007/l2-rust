//! The Underground Coliseum: the battle period (`UndergroundColiseumManager`), the arenas
//! (`UCArena`), their two teams (`UCTeam`) and the start points (`UCPoint`).
// Java: managers/games/UndergroundColiseumManager.java
// Java: mechanics/undergroundColiseum/{UCArena,UCTeam,UCPoint,UCWaiting,UCRunningTask,UCReward,
//       UCBestTeam}.java
// Java: entity/actor/instance/UCTower.java
// Java: config/UndergroundColiseumConfig.java
//
// # The battle period
// `BattleStartTime` (cron) opens a period of `BattlePeriod` hours. Parties of `PartySize` or more
// register at the arena's curator (`bypass.rs`); two waiting parties fight in the arena: first
// the countdown (one minute), then the teams are split over the start points, the towers are
// spawned and the match lasts ten minutes. A team wins by killing the other team's members after
// its tower is down, or by the kill count when the time is up. The winner stays for the next
// waiting party.
//
// # Parties
// A party is a [`PartyId`]. What Java keeps in `Party._ucState` (`UCWaiting` or `UCTeam`) is
// [`UcPartyState`] in the arena's table.

use std::collections::{BTreeMap, HashMap};

use l2_data::ColiseumArenaDef;
use l2_protocol_game::NpcStringId;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::minigame::{PVP_RECORD_FINISH, PVP_RECORD_START, PVP_RECORD_UPDATE};
use l2_protocol_game::server::{
    ExPvpMatchRecord, ExPvpMatchUserDie, ExShowScreenMessage, PvpMember, SystemMessage,
};
use l2_util::ObjectId;
use tracing::{info, warn};

use super::cron::SchedulingPattern;
use super::{
    MinigameLoad, MinigameTimer, MinigameWrite, TEAM_BLUE, TEAM_NONE, TEAM_RED, UC_STATE_ARENA,
    UC_STATE_NONE, UC_STATE_POINT,
};
use crate::outbox::PersistCommand;
use crate::party::PartyId;
use crate::teleport::TeleportWhereType;
use crate::timer::TimerId;
use crate::world::World;

/// `UCTeam.NOT_DECIDED`, `WIN`, `FAIL`.
const NOT_DECIDED: u8 = 0;
const WIN: u8 = 1;
const FAIL: u8 = 2;
/// `MINUTES_IN_MILISECONDS = 10 * 60000`.
const MATCH_MS: u64 = 600_000;
/// `MINUTES_IN_MILISECONDS - 60000L`: the wait for the next match when none is running.
const WAIT_MS: u64 = 540_000;
/// `PlayerVariables.RESTORE_LOCATION`.
const RESTORE_LOCATION: &str = "RESTORE_LOCATION";
/// The default of `BattleStartTime`, used when the configured pattern does not parse.
const DEFAULT_START_TIME: &str = "0 17 * * *";
/// Arena 5 is the one open to every level from 40; the others use their level ranges. The towers.
const TEAM_COUNT: usize = 2;
/// Players are dealt over the first three start points of the arena.
const POINTS_USED: usize = 3;

/// Java `Party._ucState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UcPartyState {
    /// `UCWaiting`: the party waits in the arena's list.
    Waiting { arena: i32 },
    /// `UCTeam`: the party fights as team `index` (0 blue, 1 red).
    Team { arena: i32, index: usize },
}

/// What a coliseum timer does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColiseumTask {
    /// `UCRegistrationTask(status)`.
    PeriodSwitch(bool),
    /// `UCRunningTask.run()` of an arena.
    ArenaRun(i32),
    /// One step of `prepareStart()`'s sleeping countdown.
    PrepareStep { arena: i32, step: u8 },
    /// The delayed resurrection of `UCTeam.onKill`.
    Resurrect {
        arena: i32,
        team: usize,
        player: ObjectId,
    },
}

/// `UCTeam`.
#[derive(Debug, Default)]
pub(crate) struct UcTeam {
    tower: Option<ObjectId>,
    party: Option<PartyId>,
    kill_count: i32,
    status: u8,
    last_party: Option<PartyId>,
    consecutive_wins: i32,
    register_time_ms: i64,
}

/// `UCWaiting`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct UcWaiting {
    party: PartyId,
    register_ms: i64,
}

/// `UCArena`.
#[derive(Debug)]
pub(crate) struct UcArena {
    def: ColiseumArenaDef,
    /// `_manager`: the curator NPC.
    manager: Option<ObjectId>,
    teams: Vec<UcTeam>,
    /// `UCPoint._players`.
    point_players: Vec<Vec<ObjectId>>,
    waiting: Vec<UcWaiting>,
    battle_now: bool,
    /// `_taskFuture`.
    task: Option<TimerId>,
}

/// `UndergroundColiseumManager`'s state.
#[derive(Debug, Default)]
pub(crate) struct ColiseumWorld {
    arenas: BTreeMap<i32, UcArena>,
    started: bool,
    period_start_ms: i64,
    period_end_ms: i64,
    reg_task: Option<TimerId>,
    /// `_bestTeams`: arena id -> (leader, wins).
    best_teams: BTreeMap<i32, (String, i32)>,
    /// `Party._ucState`.
    party_state: HashMap<PartyId, UcPartyState>,
}

/// A team of an arena, for tests and tools.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UcTeamView {
    pub party: Option<PartyId>,
    pub kill_count: i32,
    /// 0 not decided, 1 win, 2 fail.
    pub status: u8,
    pub consecutive_wins: i32,
    pub tower: Option<ObjectId>,
}

fn chain_ms(ms: i64) -> u64 {
    u64::try_from(ms).unwrap_or(0)
}

impl World {
    // ------------------------------------------------------------------ the manager

    /// Java `UndergroundColiseumManager.load()`.
    pub(crate) fn start_underground_coliseum(&mut self, load: MinigameLoad) {
        let now = self.minigame_now_ms();
        self.minigames.coliseum.period_start_ms = load.uc_start_ms;
        self.minigames.coliseum.period_end_ms = load.uc_stop_ms;
        if load.uc_start_ms < now && load.uc_stop_ms < now {
            self.coliseum_generate_new_date();
        }
        // `parseDatapackFile("data/UndergroundColiseum.xml")`.
        let defs: Vec<ColiseumArenaDef> = self.minigames.data.coliseum_arenas().cloned().collect();
        let mut arenas = BTreeMap::new();
        for def in defs {
            let manager = self.minigame_find_npc(def.curator);
            let teams = (0..TEAM_COUNT).map(|_| UcTeam::default()).collect();
            arenas.insert(
                def.id,
                UcArena {
                    def,
                    manager,
                    teams,
                    point_players: vec![Vec::new(); 4],
                    waiting: Vec::new(),
                    battle_now: false,
                    task: None,
                },
            );
        }
        info!(
            "UndergroundColiseumManager: Loaded {} coliseum arenas.",
            arenas.len()
        );
        self.minigames.coliseum.arenas = arenas;

        let (start, end) = (
            self.minigames.coliseum.period_start_ms,
            self.minigames.coliseum.period_end_ms,
        );
        if start < now && end > now {
            self.coliseum_switch_status(true);
        } else {
            let id = self.minigame_schedule(
                chain_ms(start - now),
                MinigameTimer::Coliseum(ColiseumTask::PeriodSwitch(true)),
            );
            self.minigames.coliseum.reg_task = Some(id);
            info!("UndergroundColiseumManager: Starts at {start}");
        }
        // `restoreBestTeams()`.
        self.minigames.coliseum.best_teams = load
            .best_teams
            .into_iter()
            .map(|(arena, leader, wins)| (arena, (leader, wins)))
            .collect();
    }

    /// Java `generateNewDate()`: the next period from the cron pattern; the two variables are
    /// written with `CycleDatabaseWrites` (D-53).
    fn coliseum_generate_new_date(&mut self) {
        let now = self.minigame_now_ms();
        let text = self.minigames.settings.uc_start_time.clone();
        let pattern = SchedulingPattern::parse(&text)
            .or_else(|e| {
                warn!(
                    "UndergroundColiseum: bad BattleStartTime {text:?} ({}); using {DEFAULT_START_TIME}.",
                    e.0
                );
                SchedulingPattern::parse(DEFAULT_START_TIME)
            })
            .unwrap_or_else(|_| SchedulingPattern::never());
        let offset = match self.minigames.settings.clock_override_unix_ms {
            Some(_) => 0,
            None => l2_util::time::local_offset_millis(now),
        };
        let start = pattern.next(now, offset);
        let end = start + i64::from(self.minigames.settings.uc_time_period) * 3_600_000;
        self.minigames.coliseum.period_start_ms = start;
        self.minigames.coliseum.period_end_ms = end;
        for (name, value) in [("UC_START_TIME", start), ("UC_STOP_TIME", end)] {
            self.minigame_write(MinigameWrite::GlobalVariable {
                name: name.to_owned(),
                value,
            });
        }
    }

    /// A row write, only with `CycleDatabaseWrites` (D-53).
    fn minigame_write(&mut self, write: MinigameWrite) {
        if self.minigames.settings.cycle_database_writes {
            self.outbox.persist(PersistCommand::Minigame(write));
        }
    }

    /// Java `switchStatus(isStart)`.
    fn coliseum_switch_status(&mut self, is_start: bool) {
        if let Some(task) = self.minigames.coliseum.reg_task.take() {
            self.timers.cancel(task);
        }
        self.coliseum_set_started(is_start);
        let now = self.minigame_now_ms();
        if is_start {
            let end = self.minigames.coliseum.period_end_ms;
            let id = self.minigame_schedule(
                chain_ms(end - now),
                MinigameTimer::Coliseum(ColiseumTask::PeriodSwitch(false)),
            );
            self.minigames.coliseum.reg_task = Some(id);
            info!("UndergroundColiseumManager: Ends at {end}");
        } else {
            self.coliseum_generate_new_date();
            let start = self.minigames.coliseum.period_start_ms;
            let id = self.minigame_schedule(
                chain_ms(start - now),
                MinigameTimer::Coliseum(ColiseumTask::PeriodSwitch(true)),
            );
            self.minigames.coliseum.reg_task = Some(id);
            info!("UndergroundColiseumManager: Starts at {start}");
        }
    }

    /// Java `setStarted(started)`.
    fn coliseum_set_started(&mut self, started: bool) {
        self.minigames.coliseum.started = started;
        let ids: Vec<i32> = self.minigames.coliseum.arenas.keys().copied().collect();
        for arena in ids {
            self.coliseum_arena_switch_status(arena, started);
        }
        if self.minigames.settings.uc_allow_announce {
            let text = if started {
                "Underground Coliseum has started!"
            } else {
                "Underground Coliseum has stopped!"
            };
            self.broadcast_announcement(text, false);
        }
    }

    /// Java `UndergroundColiseumManager.isStarted()`.
    pub fn coliseum_is_started(&self) -> bool {
        self.minigames.coliseum.started
    }

    /// The battle period (start, end) on the wall clock.
    pub fn coliseum_period(&self) -> (i64, i64) {
        (
            self.minigames.coliseum.period_start_ms,
            self.minigames.coliseum.period_end_ms,
        )
    }

    /// Java `getBestTeam(arenaId)`: (leader, wins).
    pub fn coliseum_best_team(&self, arena: i32) -> Option<(String, i32)> {
        self.minigames.coliseum.best_teams.get(&arena).cloned()
    }

    /// Java `updateBestTeam(arenaId, name, wins)`.
    fn coliseum_update_best_team(&mut self, arena: i32, name: &str, wins: i32) {
        let known = self.minigames.coliseum.best_teams.contains_key(&arena);
        self.minigames
            .coliseum
            .best_teams
            .insert(arena, (name.to_owned(), wins));
        if known {
            self.minigame_write(MinigameWrite::BestTeamUpdate {
                arena_id: arena,
                wins,
            });
        } else {
            self.minigame_write(MinigameWrite::BestTeamInsert {
                arena_id: arena,
                leader: name.to_owned(),
                wins,
            });
        }
    }

    /// The party state, `Party.getUCState()`.
    pub fn uc_party_state(&self, party: PartyId) -> Option<UcPartyState> {
        self.minigames.coliseum.party_state.get(&party).copied()
    }

    /// The parties waiting in an arena, in order.
    pub fn coliseum_waiting(&self, arena: i32) -> Vec<PartyId> {
        self.minigames
            .coliseum
            .arenas
            .get(&arena)
            .map(|a| a.waiting.iter().map(|w| w.party).collect())
            .unwrap_or_default()
    }

    /// A team of an arena.
    pub fn coliseum_team(&self, arena: i32, index: usize) -> Option<UcTeamView> {
        let team = self.minigames.coliseum.arenas.get(&arena)?.teams.get(index)?;
        Some(UcTeamView {
            party: team.party,
            kill_count: team.kill_count,
            status: team.status,
            consecutive_wins: team.consecutive_wins,
            tower: team.tower,
        })
    }

    /// Java `UCArena.isBattleNow()`.
    pub fn coliseum_is_battle_now(&self, arena: i32) -> bool {
        self.minigames
            .coliseum
            .arenas
            .get(&arena)
            .is_some_and(|a| a.battle_now)
    }

    pub(crate) fn on_coliseum_timer(&mut self, task: ColiseumTask) {
        match task {
            ColiseumTask::PeriodSwitch(status) => {
                self.minigames.coliseum.reg_task = None;
                self.coliseum_switch_status(status);
            }
            ColiseumTask::ArenaRun(arena) => {
                if let Some(a) = self.minigames.coliseum.arenas.get_mut(&arena) {
                    a.task = None;
                }
                self.coliseum_running_task(arena);
            }
            ColiseumTask::PrepareStep { arena, step } => self.coliseum_prepare_step(arena, step),
            ColiseumTask::Resurrect { arena, team, player } => self.coliseum_resurrect(arena, team, player),
        }
    }

    // ------------------------------------------------------------------ the arena

    /// Java `UCArena.switchStatus(start)`.
    // NOTE(java-quirk): a start while a task is pending takes the stop branch too.
    fn coliseum_arena_switch_status(&mut self, arena_id: i32, start: bool) {
        let Some(arena) = self.minigames.coliseum.arenas.get_mut(&arena_id) else {
            return;
        };
        if arena.task.is_none() && start {
            self.coliseum_run_new_task(arena_id, false);
            return;
        }
        if let Some(task) = arena.task.take() {
            self.timers.cancel(task);
        }
        self.coliseum_generate_winner(arena_id);
        self.coliseum_remove_teams(arena_id);
        for index in 0..TEAM_COUNT {
            self.coliseum_team_clean_up(arena_id, index);
        }
        let doors = self.coliseum_point_doors(arena_id);
        for (point, ids) in doors.iter().enumerate() {
            self.coliseum_set_doors(ids, false);
            if let Some(arena) = self.minigames.coliseum.arenas.get_mut(&arena_id) {
                arena.point_players[point].clear();
            }
        }
        if let Some(arena) = self.minigames.coliseum.arenas.get_mut(&arena_id) {
            arena.battle_now = false;
        }
    }

    /// Java `runNewTask(isFullTime)`.
    fn coliseum_run_new_task(&mut self, arena_id: i32, full_time: bool) {
        let id = self.minigame_schedule(
            if full_time { MATCH_MS } else { WAIT_MS },
            MinigameTimer::Coliseum(ColiseumTask::ArenaRun(arena_id)),
        );
        if let Some(arena) = self.minigames.coliseum.arenas.get_mut(&arena_id) {
            arena.task = Some(id);
        }
    }

    /// Java `runTaskNow()`: the running task runs at once (one tick).
    pub(crate) fn coliseum_run_task_now(&mut self, arena_id: i32) {
        if let Some(task) = self
            .minigames
            .coliseum
            .arenas
            .get_mut(&arena_id)
            .and_then(|a| a.task.take())
        {
            self.timers.cancel(task);
        }
        let id = self.minigame_schedule(1, MinigameTimer::Coliseum(ColiseumTask::ArenaRun(arena_id)));
        if let Some(arena) = self.minigames.coliseum.arenas.get_mut(&arena_id) {
            arena.task = Some(id);
        }
    }

    fn coliseum_point_doors(&self, arena_id: i32) -> Vec<Vec<i32>> {
        self.minigames
            .coliseum
            .arenas
            .get(&arena_id)
            .map(|a| a.def.points.iter().map(|p| p.doors.clone()).collect())
            .unwrap_or_default()
    }

    fn coliseum_set_doors(&mut self, doors: &[i32], open: bool) {
        for door_id in doors {
            let Some(object) = self.door_by_door_id(*door_id).map(|d| d.object_id) else {
                continue;
            };
            if open {
                self.open_door(object);
            } else {
                self.close_door(object);
            }
        }
    }

    /// The members of a team's party, in party order.
    fn coliseum_team_members(&self, arena_id: i32, index: usize) -> Vec<ObjectId> {
        self.minigames
            .coliseum
            .arenas
            .get(&arena_id)
            .and_then(|a| a.teams.get(index))
            .and_then(|t| t.party)
            .map(|p| self.party_members(p))
            .unwrap_or_default()
    }

    /// Java `UCArena.broadcastToAll(packet)`.
    fn coliseum_broadcast(&self, arena_id: i32, packet: &impl l2_net::Encode) {
        for index in 0..TEAM_COUNT {
            for member in self.coliseum_team_members(arena_id, index) {
                self.send(member, packet);
            }
        }
    }

    /// Java `UCTeam.setParty(party)`.
    pub(crate) fn coliseum_team_set_party(&mut self, arena_id: i32, index: usize, party: Option<PartyId>) {
        let Some(team) = self
            .minigames
            .coliseum
            .arenas
            .get_mut(&arena_id)
            .and_then(|a| a.teams.get_mut(index))
        else {
            return;
        };
        let old = std::mem::replace(&mut team.party, party);
        if let Some(old) = old {
            self.minigames.coliseum.party_state.remove(&old);
        }
        if let Some(party) = party {
            self.minigames.coliseum.party_state.insert(
                party,
                UcPartyState::Team {
                    arena: arena_id,
                    index,
                },
            );
        }
    }

    /// Java `UCTeam.cleanUp()`.
    fn coliseum_team_clean_up(&mut self, arena_id: i32, index: usize) {
        let Some(team) = self
            .minigames
            .coliseum
            .arenas
            .get_mut(&arena_id)
            .and_then(|a| a.teams.get_mut(index))
        else {
            return;
        };
        let party = team.party.take();
        team.last_party = None;
        team.consecutive_wins = 0;
        team.status = NOT_DECIDED;
        team.kill_count = 0;
        if let Some(party) = party {
            self.minigames.coliseum.party_state.remove(&party);
        }
    }

    fn uc_team(&self, arena_id: i32, index: usize) -> Option<&UcTeam> {
        self.minigames.coliseum.arenas.get(&arena_id)?.teams.get(index)
    }

    /// Java `UCArena.generateWinner()`.
    fn coliseum_generate_winner(&mut self, arena_id: i32) {
        let (Some(blue), Some(red)) = (self.uc_team(arena_id, 0), self.uc_team(arena_id, 1)) else {
            return;
        };
        let (blue_status, red_status) = (blue.status, red.status);
        let (blue_party, red_party) = (blue.party, red.party);
        let (blue_kills, red_kills) = (blue.kill_count, red.kill_count);
        let (blue_time, red_time) = (blue.register_time_ms, red.register_time_ms);
        let mut winner: Option<usize> = None;
        if blue_status == WIN || red_status == WIN {
            winner = Some(if blue_status == WIN { 0 } else { 1 });
        } else if blue_party.is_none() && red_party.is_some() {
            self.coliseum_team_set_status(arena_id, 1, WIN);
            winner = Some(1);
        } else if red_party.is_none() && blue_party.is_some() {
            self.coliseum_team_set_status(arena_id, 0, WIN);
            winner = Some(0);
        } else if red_party.is_some() && blue_party.is_some() {
            if blue_kills > red_kills {
                self.coliseum_team_set_status(arena_id, 0, WIN);
                self.coliseum_team_set_status(arena_id, 1, FAIL);
                winner = Some(0);
            } else if red_kills > blue_kills {
                self.coliseum_team_set_status(arena_id, 0, FAIL);
                self.coliseum_team_set_status(arena_id, 1, WIN);
                winner = Some(1);
            } else if blue_time > red_time {
                self.coliseum_team_set_status(arena_id, 0, FAIL);
                self.coliseum_team_set_status(arena_id, 1, WIN);
                winner = Some(1);
            } else {
                self.coliseum_team_set_status(arena_id, 0, WIN);
                self.coliseum_team_set_status(arena_id, 1, FAIL);
                winner = Some(0);
            }
        }
        self.coliseum_broadcast_record(arena_id, PVP_RECORD_FINISH, winner.map_or(0, |w| w as i32 + 1));
        // `blueTeam.setLastParty(redTeam.getParty())` with the parties as they are now.
        let (blue_party, red_party) = (
            self.uc_team(arena_id, 0).and_then(|t| t.party),
            self.uc_team(arena_id, 1).and_then(|t| t.party),
        );
        if let Some(arena) = self.minigames.coliseum.arenas.get_mut(&arena_id) {
            arena.teams[0].last_party = red_party;
            arena.teams[1].last_party = blue_party;
        }
    }

    /// Java `UCTeam.setStatus(status)`.
    fn coliseum_team_set_status(&mut self, arena_id: i32, index: usize, status: u8) {
        if let Some(team) = self
            .minigames
            .coliseum
            .arenas
            .get_mut(&arena_id)
            .and_then(|a| a.teams.get_mut(index))
        {
            team.status = status;
        }
        if status == WIN {
            self.coliseum_broadcast(
                arena_id,
                &SystemMessage::new(if index == 0 {
                    SystemMessageId::TheBlueTeamIsVictorious
                } else {
                    SystemMessageId::TheRedTeamIsVictorious
                }),
            );
        }
        match status {
            WIN => {
                self.coliseum_increase_consecutive_wins(arena_id, index);
                self.coliseum_compute_reward(arena_id, index);
                self.coliseum_delete_tower(arena_id, index);
            }
            FAIL => self.coliseum_delete_tower(arena_id, index),
            _ => {}
        }
    }

    /// Java `UCTeam.increaseConsecutiveWins()`.
    fn coliseum_increase_consecutive_wins(&mut self, arena_id: i32, index: usize) {
        let Some(team) = self
            .minigames
            .coliseum
            .arenas
            .get_mut(&arena_id)
            .and_then(|a| a.teams.get_mut(index))
        else {
            return;
        };
        team.consecutive_wins += 1;
        let (wins, party) = (team.consecutive_wins, team.party);
        if wins > 1
            && let Some(leader) = party.and_then(|p| self.party_leader(p))
        {
            let name = self.minigame_player_name(leader);
            self.coliseum_update_best_team(arena_id, &name, wins);
        }
    }

    /// Java `UCTeam.computeReward()`.
    fn coliseum_compute_reward(&mut self, arena_id: i32, index: usize) {
        let other = 1 - index;
        let (Some(team), Some(other_team)) = (self.uc_team(arena_id, index), self.uc_team(arena_id, other))
        else {
            return;
        };
        if !(team.last_party.is_none() || team.last_party != other_team.party) {
            return;
        }
        let wins = team.consecutive_wins;
        let modifier = match wins {
            1 => 1.0,
            2 => 1.06,
            3 => 1.12,
            4 => 1.18,
            5 => 1.25,
            6 => 1.27,
            7 => 1.3,
            8 => 1.32,
            9 => 1.35,
            10 => 1.37,
            w if w > 10 => 1.4,
            _ => 1.0,
        };
        let rewards = self
            .minigames
            .coliseum
            .arenas
            .get(&arena_id)
            .map(|a| a.def.rewards.clone())
            .unwrap_or_default();
        if rewards.is_empty() {
            return;
        }
        for member in self.coliseum_team_members(arena_id, index) {
            for reward in &rewards {
                let amount = if reward.use_modifier {
                    (reward.amount as f64 * modifier) as i64
                } else {
                    reward.amount
                };
                self.minigame_give_reward(member, reward.id, amount);
            }
        }
    }

    /// Java `UCTeam.spawnTower()`.
    fn coliseum_spawn_tower(&mut self, arena_id: i32, index: usize) {
        let Some(arena) = self.minigames.coliseum.arenas.get(&arena_id) else {
            return;
        };
        if arena.teams.get(index).is_none_or(|t| t.tower.is_some()) {
            return;
        }
        let Some(tower) = arena.def.towers.get(index).copied() else {
            return;
        };
        let (x, y, z) = tower.place;
        // `setInvul(false)` and `setCurrentHpMp(max, max)` are what a new NPC has.
        let Some(npc) = self.spawn_npc_at(tower.npc_id, x, y, z, 0) else {
            return;
        };
        self.minigames
            .npc_team
            .insert(npc, if index == 1 { TEAM_BLUE } else { TEAM_RED });
        if let Some(team) = self
            .minigames
            .coliseum
            .arenas
            .get_mut(&arena_id)
            .and_then(|a| a.teams.get_mut(index))
        {
            team.tower = Some(npc);
        }
        self.broadcast_info(npc);
    }

    /// Java `UCTeam.deleteTower()`.
    fn coliseum_delete_tower(&mut self, arena_id: i32, index: usize) {
        let Some(tower) = self
            .minigames
            .coliseum
            .arenas
            .get_mut(&arena_id)
            .and_then(|a| a.teams.get_mut(index))
            .and_then(|t| t.tower.take())
        else {
            return;
        };
        self.minigames.npc_team.remove(&tower);
        self.delete_npc(tower);
    }

    /// `UCTower.doDie`: the dead tower leaves its team.
    pub(crate) fn coliseum_tower_died(&mut self, npc: ObjectId) {
        if !self.minigames.npc_team.contains_key(&npc) {
            return;
        }
        let found = self.minigames.coliseum.arenas.iter().find_map(|(id, arena)| {
            arena
                .teams
                .iter()
                .position(|t| t.tower == Some(npc))
                .map(|index| (*id, index))
        });
        if let Some((arena, index)) = found {
            self.coliseum_delete_tower(arena, index);
        }
    }

    /// `UCTower.reduceCurrentHp`: "if (attacker.getTeam() == getTeam()) return;". `true` when the
    /// hit is ignored.
    pub(crate) fn minigame_tower_refuses_damage(&self, target: ObjectId, attacker: Option<ObjectId>) -> bool {
        let Some(&tower_team) = self.minigames.npc_team.get(&target) else {
            return false;
        };
        attacker.is_some_and(|a| self.team_of(self.acting_player(a).unwrap_or(a)) == tower_team)
    }

    /// Java `UCArena.broadcastRecord(type, teamType)`.
    fn coliseum_broadcast_record(&mut self, arena_id: i32, record_type: i32, team_type: i32) {
        let side = |world: &World, index: usize| -> Vec<PvpMember> {
            world
                .coliseum_team_members(arena_id, index)
                .into_iter()
                .map(|member| {
                    let (kills, deaths) = world.uc_kills_deaths_of(member);
                    PvpMember {
                        name: world.minigame_player_name(member),
                        kills,
                        deaths,
                    }
                })
                .collect()
        };
        let kills = |index: usize| self.uc_team(arena_id, index).map_or(0, |t| t.kill_count);
        let packet = ExPvpMatchRecord {
            record_type,
            winner_team: team_type,
            blue_kills: kills(0),
            red_kills: kills(1),
            blue: side(self, 0),
            red: side(self, 1),
        };
        let user_die = (record_type == PVP_RECORD_UPDATE).then(|| self.coliseum_user_die(arena_id));
        for index in 0..TEAM_COUNT {
            for member in self.coliseum_team_members(arena_id, index) {
                self.send(member, &packet);
                if let Some(die) = &user_die {
                    self.send(member, die);
                }
            }
        }
    }

    /// `new ExPVPMatchUserDie(arena)`.
    // NOTE(java-quirk): the packet's "red" kills are the kills of team 0, which is the blue team.
    fn coliseum_user_die(&self, arena_id: i32) -> ExPvpMatchUserDie {
        let kills = |index: usize| self.uc_team(arena_id, index).map_or(0, |t| t.kill_count);
        ExPvpMatchUserDie {
            red_kills: kills(0),
            blue_kills: kills(1),
        }
    }

    // ------------------------------------------------------------------ the match

    /// Java `UCRunningTask.run()`.
    fn coliseum_running_task(&mut self, arena_id: i32) {
        self.coliseum_generate_winner(arena_id);
        self.coliseum_remove_teams(arena_id);

        let mut winner: Option<usize> = None;
        for index in 0..TEAM_COUNT {
            match self.uc_team(arena_id, index).map(|t| t.status) {
                Some(WIN) => winner = Some(index),
                Some(FAIL) => self.coliseum_team_clean_up(arena_id, index),
                _ => {}
            }
        }
        let doors = self.coliseum_point_doors(arena_id);
        for (point, ids) in doors.iter().enumerate() {
            self.coliseum_set_doors(ids, false);
            if let Some(arena) = self.minigames.coliseum.arenas.get_mut(&arena_id) {
                arena.point_players[point].clear();
            }
        }

        if let Some(winner) = winner {
            let waiting = self
                .minigames
                .coliseum
                .arenas
                .get(&arena_id)
                .and_then(|a| a.waiting.first().copied());
            if let Some(waiting) = waiting {
                let other = 1 - winner;
                self.coliseum_team_set_party(arena_id, other, Some(waiting.party));
                if let Some(arena) = self.minigames.coliseum.arenas.get_mut(&arena_id) {
                    arena.teams[other].register_time_ms = waiting.register_ms;
                    arena.waiting.remove(0);
                }
                self.coliseum_prepare_start(arena_id);
                return;
            }
            self.coliseum_team_clean_up(arena_id, winner);
        }

        let waiting: Vec<UcWaiting> = self
            .minigames
            .coliseum
            .arenas
            .get(&arena_id)
            .map(|a| a.waiting.iter().take(2).copied().collect())
            .unwrap_or_default();
        let waiting_count = self
            .minigames
            .coliseum
            .arenas
            .get(&arena_id)
            .map_or(0, |a| a.waiting.len());
        if waiting_count >= 2 {
            for (index, entry) in waiting.iter().enumerate() {
                self.coliseum_team_set_party(arena_id, index, Some(entry.party));
                if let Some(arena) = self.minigames.coliseum.arenas.get_mut(&arena_id) {
                    arena.teams[index].register_time_ms = entry.register_ms;
                }
            }
            if let Some(arena) = self.minigames.coliseum.arenas.get_mut(&arena_id) {
                arena.waiting.drain(..waiting.len());
            }
            self.coliseum_prepare_start(arena_id);
            return;
        }
        if let Some(arena) = self.minigames.coliseum.arenas.get_mut(&arena_id) {
            arena.battle_now = false;
        }
        self.coliseum_run_new_task(arena_id, false);
    }

    /// Java `UCArena.removeTeams()`.
    fn coliseum_remove_teams(&mut self, arena_id: i32) {
        for index in 0..TEAM_COUNT {
            for player in self.coliseum_team_members(arena_id, index) {
                self.set_team(player, TEAM_NONE);
                if let Some(p) = self.player_minigame_mut(player) {
                    p.uc_kills = 0;
                    p.uc_deaths = 0;
                }
                self.set_uc_state(player, UC_STATE_NONE);
                if self.is_dead(player) {
                    self.coliseum_res_player(player);
                }
                let restore = self.minigame_variable(player, RESTORE_LOCATION);
                if restore.is_empty() {
                    self.tele_to_location_where(player, TeleportWhereType::Town);
                } else {
                    let parts: Vec<i32> = restore.split(';').filter_map(|v| v.parse().ok()).collect();
                    if let [x, y, z] = parts[..] {
                        self.minigame_tele(player, (x, y, z));
                    } else {
                        warn!("UCArena: bad RESTORE_LOCATION {restore:?} of {player}");
                    }
                }
            }
        }
    }

    /// Java `UCTeam.resPlayer(player)`.
    fn coliseum_res_player(&mut self, player: ObjectId) {
        self.restore_exp(player, 100.0);
        self.do_revive(player);
        let Some((hp, mp, cp)) = self.stats(player).map(|s| (s.max_hp(), s.max_mp(), s.max_cp())) else {
            return;
        };
        self.set_current_hp_mp(player, f64::from(hp), f64::from(mp));
        self.set_current_cp(player, f64::from(cp), true);
    }

    /// Java `UCArena.prepareStart()`: the countdown starts.
    fn coliseum_prepare_start(&mut self, arena_id: i32) {
        if let Some(arena) = self.minigames.coliseum.arenas.get_mut(&arena_id) {
            arena.battle_now = true;
        }
        self.coliseum_prepare_step(arena_id, 0);
    }

    /// One message of `prepareStart()` (Java sleeps between them) and, at the end, the check.
    fn coliseum_prepare_step(&mut self, arena_id: i32, step: u8) {
        // (message, parameter, display ms, ms to the next step).
        const STEPS: [(NpcStringId, &str, i32, u64); 8] = [
            (
                NpcStringId::MatchBeginsInS1MinuteSPleaseGatherAroundTheAdministrator,
                "1",
                5000,
                30_000,
            ),
            (NpcStringId::S1SecondSRemaining, "30", 5000, 20_000),
            (NpcStringId::S1SecondSRemaining, "10", 3000, 5000),
            (NpcStringId::S1SecondSRemaining, "5", 1000, 1000),
            (NpcStringId::S1SecondSRemaining, "4", 1000, 1000),
            (NpcStringId::S1SecondSRemaining, "3", 1000, 1000),
            (NpcStringId::S1SecondSRemaining, "2", 1000, 1000),
            (NpcStringId::S1SecondSRemaining, "1", 1000, 1000),
        ];
        if let Some((string, param, time, next)) = STEPS.get(usize::from(step)) {
            self.coliseum_broadcast(
                arena_id,
                &ExShowScreenMessage::npc_string(*string, 2, *time, &[param]),
            );
            self.minigame_schedule(
                *next,
                MinigameTimer::Coliseum(ColiseumTask::PrepareStep {
                    arena: arena_id,
                    step: step + 1,
                }),
            );
            return;
        }
        // The check at the end of the countdown.
        let range = f64::from(self.settings.party.alt_party_range);
        let manager_pos = self
            .minigames
            .coliseum
            .arenas
            .get(&arena_id)
            .and_then(|a| a.manager)
            .and_then(|m| self.position_of(m));
        let party_size = usize::try_from(self.minigames.settings.uc_party_size).unwrap_or(0);
        let mut is_valid = true;
        for index in 0..TEAM_COUNT {
            let Some(party) = self.uc_team(arena_id, index).and_then(|t| t.party) else {
                is_valid = false;
                continue;
            };
            let members = self.party_members(party);
            if members.len() < party_size {
                is_valid = false;
                continue;
            }
            for member in members {
                let far = match (self.position_of(member), manager_pos) {
                    (Some(p), Some(m)) => crate::target::distance_3d(p, m) > range,
                    _ => true,
                };
                if far {
                    is_valid = false;
                    break;
                }
            }
        }
        if !is_valid {
            self.coliseum_broadcast(
                arena_id,
                &ExShowScreenMessage::npc_string(
                    NpcStringId::TheMatchIsAutomaticallyCanceledBecauseYouAreTooFarFromTheAdmissionManager,
                    2,
                    5000,
                    &[],
                ),
            );
            for index in 0..TEAM_COUNT {
                self.coliseum_team_set_party(arena_id, index, None);
                if let Some(arena) = self.minigames.coliseum.arenas.get_mut(&arena_id) {
                    arena.teams[index].register_time_ms = 0;
                }
            }
            if let Some(arena) = self.minigames.coliseum.arenas.get_mut(&arena_id) {
                arena.battle_now = false;
            }
            self.coliseum_run_new_task(arena_id, false);
            return;
        }
        self.coliseum_run_new_task(arena_id, true);
        self.coliseum_split_members_and_teleport(arena_id);
        self.coliseum_start_fight(arena_id);
    }

    /// Java `splitMembersAndTeleport()`.
    fn coliseum_split_members_and_teleport(&mut self, arena_id: i32) {
        if let Some(arena) = self.minigames.coliseum.arenas.get_mut(&arena_id) {
            for players in &mut arena.point_players {
                players.clear();
            }
        }
        self.coliseum_broadcast_record(arena_id, PVP_RECORD_START, 0);
        for index in 0..TEAM_COUNT {
            let mut point = 0;
            for member in self.coliseum_team_members(arena_id, index) {
                self.set_uc_state(member, UC_STATE_POINT);
                self.coliseum_point_teleport_player(arena_id, point, member);
                point += 1;
                if point >= POINTS_USED {
                    point = 0;
                }
            }
        }
        self.coliseum_broadcast_record(arena_id, PVP_RECORD_UPDATE, 0);
    }

    /// Java `UCPoint.teleportPlayer(player)`.
    fn coliseum_point_teleport_player(&mut self, arena_id: i32, point: usize, player: ObjectId) {
        let Some(place) = self
            .minigames
            .coliseum
            .arenas
            .get(&arena_id)
            .and_then(|a| a.def.points.get(point))
            .map(|p| p.place)
        else {
            return;
        };
        if let Some((x, y, z)) = self.position_of(player) {
            self.minigame_set_variable(player, RESTORE_LOCATION, format!("{x};{y};{z}"));
        }
        if self.is_dead(player) {
            self.coliseum_res_player(player);
        }
        self.minigame_tele(player, place);
        if let Some(arena) = self.minigames.coliseum.arenas.get_mut(&arena_id) {
            arena.point_players[point].push(player);
        }
    }

    /// Java `startFight()`.
    fn coliseum_start_fight(&mut self, arena_id: i32) {
        for index in 0..TEAM_COUNT {
            self.coliseum_spawn_tower(arena_id, index);
            for member in self.coliseum_team_members(arena_id, index) {
                self.set_team(member, if index == 1 { TEAM_BLUE } else { TEAM_RED });
            }
        }
    }

    // ------------------------------------------------------------------ deaths

    /// `Player.doDie`: "if the party is a coliseum team, `UCTeam.onKill(this, pk)`".
    pub(crate) fn coliseum_on_player_die(&mut self, id: ObjectId, killer: Option<ObjectId>) {
        let Some(killer) = killer.and_then(|k| self.acting_player(k)) else {
            return;
        };
        let Some(UcPartyState::Team { arena, index }) = self
            .party_of(id)
            .and_then(|p| self.minigames.coliseum.party_state.get(&p).copied())
        else {
            return;
        };
        self.coliseum_team_on_kill(arena, index, id, killer);
    }

    /// Java `UCTeam.onKill(player, killer)`.
    fn coliseum_team_on_kill(&mut self, arena_id: i32, index: usize, player: ObjectId, killer: ObjectId) {
        let Some(team) = self.uc_team(arena_id, index) else {
            return;
        };
        if team.party.is_none() {
            return;
        }
        let party = team.party;
        if self.party_of(player).is_some() && self.party_of(player) == self.party_of(killer) {
            return;
        }
        let other = 1 - index;
        if let Some(arena) = self.minigames.coliseum.arenas.get_mut(&arena_id) {
            arena.teams[other].kill_count += 1;
        }
        if let Some(p) = self.player_minigame_mut(player) {
            p.uc_deaths += 1;
        }
        if let Some(p) = self.player_minigame_mut(killer) {
            p.uc_kills += 1;
        }
        let die = self.coliseum_user_die(arena_id);
        self.coliseum_broadcast(arena_id, &die);

        if self.uc_state_of(player) == UC_STATE_POINT {
            // `point.checkPlayer(player)`: the first point that holds the player opens its doors and
            // moves everybody of it to the arena state.
            let found = self.minigames.coliseum.arenas.get(&arena_id).and_then(|a| {
                a.point_players
                    .iter()
                    .position(|players| players.contains(&player))
                    .map(|point| (point, a.point_players[point].clone()))
            });
            if let Some((point, players)) = found {
                let doors = self
                    .coliseum_point_doors(arena_id)
                    .get(point)
                    .cloned()
                    .unwrap_or_default();
                self.coliseum_set_doors(&doors, true);
                for pl in players {
                    self.set_uc_state(pl, UC_STATE_ARENA);
                }
            }
        }

        let tower = self.uc_team(arena_id, index).and_then(|t| t.tower);
        if tower.is_none() {
            let all_dead = party
                .map(|p| self.party_members(p))
                .unwrap_or_default()
                .into_iter()
                .all(|member| self.is_dead(member));
            if all_dead {
                self.coliseum_team_set_status(arena_id, index, FAIL);
                self.coliseum_team_set_status(arena_id, other, WIN);
                self.coliseum_run_task_now(arena_id);
            }
            return;
        }
        let delay = u64::try_from(self.minigames.settings.uc_ress_time).unwrap_or(0) * 1000;
        self.minigame_schedule(
            delay,
            MinigameTimer::Coliseum(ColiseumTask::Resurrect {
                arena: arena_id,
                team: index,
                player,
            }),
        );
    }

    /// The delayed task of `UCTeam.onKill`.
    fn coliseum_resurrect(&mut self, arena_id: i32, index: usize, player: ObjectId) {
        if self.uc_team(arena_id, index).and_then(|t| t.tower).is_none() {
            return;
        }
        if !self.is_dead(player) {
            return;
        }
        let Some(tower) = self
            .minigames
            .coliseum
            .arenas
            .get(&arena_id)
            .and_then(|a| a.def.towers.get(index))
            .map(|t| t.place)
        else {
            return;
        };
        self.coliseum_res_player(player);
        let (x, y, z) = tower;
        let tx = x + self.rng().get(2, 50);
        let ty = y + self.rng().get(10, 100);
        self.minigame_tele(player, (tx, ty, z));
        if let Some(summon) = self.summon_of(player) {
            self.abort_attack(summon);
            self.abort_cast(summon);
            if !self.is_dead(summon) {
                if let Some((hp, mp)) = self.stats(summon).map(|s| (s.max_hp(), s.max_mp())) {
                    self.set_current_hp(summon, f64::from(hp), true);
                    self.set_current_mp(summon, f64::from(mp), true);
                }
                let sx = x + self.rng().get(2, 50);
                let sy = y + self.rng().get(10, 100);
                self.minigame_tele(summon, (sx, sy, z));
            }
        }
    }

    // ------------------------------------------------------------------ the registration

    /// Registers a party in the arena's waiting list (`UCManager`'s "register" after its checks):
    /// `new UCWaiting(party, arena)`, `waiting.setParty(true)`, `hasRegisterdNow()` and the start
    /// of the match task when two parties wait and no match runs.
    pub(crate) fn coliseum_register_party(&mut self, arena_id: i32, party: PartyId) {
        let now = self.minigame_now_ms();
        let Some(arena) = self.minigames.coliseum.arenas.get_mut(&arena_id) else {
            return;
        };
        arena.waiting.push(UcWaiting {
            party,
            register_ms: now,
        });
        self.minigames
            .coliseum
            .party_state
            .insert(party, UcPartyState::Waiting { arena: arena_id });
        let start = self
            .minigames
            .coliseum
            .arenas
            .get(&arena_id)
            .is_some_and(|a| a.waiting.len() >= 2 && !a.battle_now);
        if start {
            self.coliseum_run_task_now(arena_id);
        }
    }

    /// `UCManager`'s "cancel": the waiting entry goes (`waiting.setParty(false)`, `clean()`,
    /// `getWaitingList().remove(waiting)`).
    pub(crate) fn coliseum_cancel_waiting(&mut self, party: PartyId) -> bool {
        let Some(UcPartyState::Waiting { arena }) = self.minigames.coliseum.party_state.get(&party).copied()
        else {
            return false;
        };
        self.minigames.coliseum.party_state.remove(&party);
        if let Some(arena) = self.minigames.coliseum.arenas.get_mut(&arena) {
            arena.waiting.retain(|w| w.party != party);
        }
        true
    }

    pub(crate) fn coliseum_arena_def(&self, arena: i32) -> Option<ColiseumArenaDef> {
        self.minigames.coliseum.arenas.get(&arena).map(|a| a.def.clone())
    }

    pub(crate) fn coliseum_arena_has_party(&self, arena: i32, party: PartyId) -> bool {
        self.minigames
            .coliseum
            .arenas
            .get(&arena)
            .is_some_and(|a| a.teams.iter().any(|t| t.party == Some(party)))
    }

    pub(crate) fn coliseum_arena_teams_taken(&self, arena: i32) -> bool {
        self.minigames
            .coliseum
            .arenas
            .get(&arena)
            .is_some_and(|a| a.teams.iter().any(|t| t.party.is_some()))
    }

    pub(crate) fn coliseum_waiting_len(&self, arena: i32) -> usize {
        self.minigames
            .coliseum
            .arenas
            .get(&arena)
            .map_or(0, |a| a.waiting.len())
    }
}
