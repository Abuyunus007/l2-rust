//! Java `OlympiadGameTask` (one stadium's state machine), `OlympiadGameManager.run` (the
//! matchmaking every 30 s) and `OlympiadAnnouncer`.
// Java: mechanics/olympiad/{OlympiadGameTask,OlympiadGameState,OlympiadGameManager,
//       OlympiadAnnouncer}.java

use l2_protocol_game::server::{ChatType, NpcSay, SystemMessage, SystemMessageParam};
use l2_protocol_game::{NpcStringId, SystemMessageId};
use l2_util::ObjectId;
use tracing::warn;

use super::OlympiadTimer;
use super::game::{CompetitionType, OlympiadGame};
use crate::timer::TimerId;
use crate::world::World;

/// `TELEPORT_TO_ARENA_TIMES`.
const TELEPORT_TO_ARENA_TIMES: [i64; 11] = [120, 60, 30, 15, 10, 5, 4, 3, 2, 1, 0];
/// `BATTLE_START_TIME_FIRST`.
const BATTLE_START_TIME_FIRST: [i64; 7] = [60, 50, 40, 30, 20, 10, 0];
/// `BATTLE_START_TIME_SECOND`.
const BATTLE_START_TIME_SECOND: [i64; 7] = [10, 5, 4, 3, 2, 1, 0];
/// `TELEPORT_TO_TOWN_TIMES`.
const TELEPORT_TO_TOWN_TIMES: [i64; 10] = [40, 30, 20, 10, 5, 4, 3, 2, 1, 0];
/// `OlympiadAnnouncer.OLY_MANAGER`.
const OLY_MANAGER: i32 = 31688;

/// Java `OlympiadGameState`, in Java's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum GameState {
    Begin,
    TeleportToArena,
    GameStarted,
    BattleCountdownFirst,
    BattleCountdownSecond,
    BattleStarted,
    BattleInProgress,
    GameCancelled,
    GameStopped,
    TeleportToTown,
    Cleanup,
    #[default]
    Idle,
}

/// One stadium's task (Java `OlympiadGameTask`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GameTask {
    pub state: GameState,
    pub game: Option<OlympiadGame>,
    pub need_announce: bool,
    /// Java `_countDown`: seconds, but milliseconds in `BATTLE_IN_PROGRESS`.
    pub count_down: i64,
    pub(crate) timer: Option<TimerId>,
}

impl GameTask {
    /// Java `isRunning()`.
    pub fn is_running(&self) -> bool {
        self.state != GameState::Idle
    }

    /// Java `isGameStarted()`.
    pub fn is_game_started(&self) -> bool {
        self.state >= GameState::GameStarted && self.state <= GameState::Cleanup
    }

    /// Java `isBattleStarted()`.
    pub fn is_battle_started(&self) -> bool {
        self.state == GameState::BattleInProgress
    }

    /// Java `isBattleFinished()`.
    pub fn is_battle_finished(&self) -> bool {
        self.state == GameState::TeleportToTown
    }

    /// Java `getDelay(times)`.
    fn delay(&mut self, times: &[i64]) -> i64 {
        for &time in &times[..times.len().saturating_sub(1)] {
            if time >= self.count_down {
                continue;
            }
            let delay = self.count_down - time;
            self.count_down = time;
            return delay;
        }
        // "should not happens"
        self.count_down = -1;
        1
    }
}

impl World {
    fn task(&self, index: usize) -> Option<&GameTask> {
        self.olympiad.stadiums.as_ref()?.get(index).map(|s| &s.task)
    }

    fn task_mut(&mut self, index: usize) -> Option<&mut GameTask> {
        self.olympiad
            .stadiums
            .as_mut()?
            .get_mut(index)
            .map(|s| &mut s.task)
    }

    /// Java `attachGame(game)`: the task starts (`ThreadPool.execute(this)`).
    pub(crate) fn olympiad_attach_game(&mut self, index: usize, game: OlympiadGame) {
        let Some(task) = self.task_mut(index) else {
            return;
        };
        if task.state != GameState::Idle {
            warn!("Attempt to overwrite non-finished game in state {:?}", task.state);
            return;
        }
        task.game = Some(game);
        task.state = GameState::Begin;
        task.need_announce = false;
        let timer = self.schedule_olympiad_timer(OlympiadTimer::Task(index), 0);
        if let Some(task) = self.task_mut(index) {
            task.timer = Some(timer);
        }
    }

    /// Java `OlympiadGameTask.run()`.
    pub(crate) fn olympiad_task_run(&mut self, index: usize) {
        let Some(state) = self.task(index).map(|t| t.state) else {
            return;
        };
        let mut delay = 1;
        let wait_time = i64::from(self.olympiad.settings.wait_time);
        let battle_ms = self.olympiad.settings.battle_ms;
        match state {
            // Game created
            GameState::Begin => {
                if let Some(task) = self.task_mut(index) {
                    task.state = GameState::TeleportToArena;
                    task.count_down = wait_time;
                }
            }
            // Teleport to arena countdown.
            GameState::TeleportToArena => {
                let count_down = self.task(index).map_or(0, |t| t.count_down);
                if count_down > 0 {
                    let message =
                        SystemMessage::new(SystemMessageId::YouWillBeMovedToTheOlympiadStadiumInS1SecondS)
                            .with(SystemMessageParam::IntNumber(
                                i32::try_from(count_down).unwrap_or(0),
                            ));
                    self.game_broadcast(index, &message);
                }
                if let Some(task) = self.task_mut(index) {
                    delay = task.delay(&TELEPORT_TO_ARENA_TIMES);
                    if task.count_down <= 0 {
                        task.state = GameState::GameStarted;
                    }
                }
            }
            // Game start, port players to arena.
            GameState::GameStarted => {
                if !self.olympiad_start_game(index) {
                    if let Some(task) = self.task_mut(index) {
                        task.state = GameState::GameCancelled;
                    }
                } else if let Some(task) = self.task_mut(index) {
                    task.state = GameState::BattleCountdownFirst;
                    task.count_down = BATTLE_START_TIME_FIRST[0];
                    delay = 5;
                }
            }
            // Battle start countdown, first part (60-10).
            GameState::BattleCountdownFirst => {
                self.match_will_start_message(index);
                let mut done = false;
                if let Some(task) = self.task_mut(index) {
                    delay = task.delay(&BATTLE_START_TIME_FIRST);
                    done = task.count_down <= 0;
                }
                if done {
                    self.game_set_players_invul(index, true);
                    // `openDoors()`.
                    self.game_reset_damage(index);
                    self.stadium_open_doors(index);
                    if let Some(task) = self.task_mut(index) {
                        task.state = GameState::BattleCountdownSecond;
                        task.count_down = BATTLE_START_TIME_SECOND[0];
                        delay = task.delay(&BATTLE_START_TIME_SECOND);
                    }
                }
            }
            // Battle start countdown, second part (10-0).
            GameState::BattleCountdownSecond => {
                self.match_will_start_message(index);
                let mut done = false;
                if let Some(task) = self.task_mut(index) {
                    delay = task.delay(&BATTLE_START_TIME_SECOND);
                    done = task.count_down <= 0;
                    if done {
                        task.state = GameState::BattleStarted;
                    }
                }
                if done {
                    self.game_set_players_invul(index, false);
                }
            }
            // Beginning of the battle.
            GameState::BattleStarted => {
                if let Some(task) = self.task_mut(index) {
                    task.count_down = 0;
                    // "Set state first, used in zone update."
                    task.state = GameState::BattleInProgress;
                }
                if !self.olympiad_start_battle(index)
                    && let Some(task) = self.task_mut(index)
                {
                    task.state = GameState::GameStopped;
                }
            }
            // Checks during battle
            GameState::BattleInProgress => {
                let count_down = self.task(index).map_or(0, |t| t.count_down) + 1000;
                let have_winner = self.game_have_winner(index);
                if let Some(task) = self.task_mut(index) {
                    task.count_down = count_down;
                    if have_winner || count_down > battle_ms {
                        task.state = GameState::GameStopped;
                    }
                }
            }
            // Battle cancelled before teleport participants to the stadium
            GameState::GameCancelled => {
                self.olympiad_stop_game(index);
                if let Some(task) = self.task_mut(index) {
                    task.state = GameState::Cleanup;
                }
            }
            // End of the battle.
            GameState::GameStopped => {
                if let Some(task) = self.task_mut(index) {
                    task.state = GameState::TeleportToTown;
                    task.count_down = TELEPORT_TO_TOWN_TIMES[0];
                }
                self.olympiad_stop_game(index);
                if let Some(task) = self.task_mut(index) {
                    delay = task.delay(&TELEPORT_TO_TOWN_TIMES);
                }
            }
            // Teleport to town countdown.
            GameState::TeleportToTown => {
                let count_down = self.task(index).map_or(0, |t| t.count_down);
                if count_down > 0 {
                    let message = SystemMessage::new(SystemMessageId::YouWillBeMovedBackToTownInS1SecondS)
                        .with(SystemMessageParam::IntNumber(
                            i32::try_from(count_down).unwrap_or(0),
                        ));
                    self.game_broadcast(index, &message);
                }
                if let Some(task) = self.task_mut(index) {
                    delay = task.delay(&TELEPORT_TO_TOWN_TIMES);
                    if task.count_down <= 0 {
                        task.state = GameState::Cleanup;
                    }
                }
            }
            // Removals
            GameState::Cleanup => {
                self.olympiad_cleanup_game(index);
                if let Some(task) = self.task_mut(index) {
                    task.state = GameState::Idle;
                    task.game = None;
                    task.timer = None;
                }
                return;
            }
            GameState::Idle => {
                if let Some(task) = self.task_mut(index) {
                    task.timer = None;
                }
                return;
            }
        }
        let timer = self.schedule_olympiad_timer(OlympiadTimer::Task(index), delay * 1000);
        if let Some(task) = self.task_mut(index) {
            task.timer = Some(timer);
        }
    }

    /// "THE_MATCH_WILL_START_IN_S1_SECOND_S" to the stadium while the countdown runs.
    fn match_will_start_message(&mut self, index: usize) {
        let count_down = self.task(index).map_or(0, |t| t.count_down);
        if count_down > 0 {
            let message = SystemMessage::new(SystemMessageId::TheMatchWillStartInS1SecondS).with(
                SystemMessageParam::IntNumber(i32::try_from(count_down).unwrap_or(0)),
            );
            self.stadium_broadcast(index, &message);
        }
    }

    /// `startGame()`: "check for defaulted, port players to arena, announce game".
    fn olympiad_start_game(&mut self, index: usize) -> bool {
        // "Checking for opponents and teleporting to arena."
        if self.game_check_defaulted(index) {
            return false;
        }
        self.stadium_close_doors(index);
        let needs_buffers = self
            .olympiad_game(index)
            .is_some_and(|g| g.kind != CompetitionType::Teams);
        if needs_buffers {
            self.stadium_spawn_buffers(index);
        }
        let spawns = self
            .olympiad
            .stadiums()
            .get(index)
            .map(|s| s.spawns.clone())
            .unwrap_or_default();
        if !self.game_port_players_to_arena(index, &spawns) {
            return false;
        }
        self.game_removals(index);
        if let Some(task) = self.task_mut(index) {
            task.need_announce = true;
        }
        // "inform manager"
        self.olympiad.battle_started = true;
        true
    }

    /// `startBattle()`: "last checks, remove buffers, start competition itself".
    fn olympiad_start_battle(&mut self, index: usize) -> bool {
        let needs_buffers = self
            .olympiad_game(index)
            .is_some_and(|g| g.kind != CompetitionType::Teams);
        if needs_buffers {
            self.stadium_delete_buffers(index);
        }
        if self.game_check_battle_status(index) && self.game_make_competition_start(index) {
            // "game successfully started"
            self.game_broadcast_olympiad_info(index);
            self.stadium_broadcast(
                index,
                &SystemMessage::new(SystemMessageId::TheMatchHasStartedFight),
            );
            self.stadium_update_zone_status(index);
            return true;
        }
        false
    }

    /// `stopGame()`: "winner's validations".
    fn olympiad_stop_game(&mut self, index: usize) {
        self.game_validate_winner(index);
        self.game_clean_effects(index);
        self.game_set_players_invul(index, true);
        self.stadium_update_zone_status(index);
    }

    /// `cleanupGame()`: "port players back, closing doors, etc".
    fn olympiad_cleanup_game(&mut self, index: usize) {
        self.game_set_players_invul(index, false);
        self.game_players_status_back(index);
        self.game_port_players_back(index);
        self.game_clear_players(index);
        self.stadium_close_doors(index);
    }

    // ------------------------------------------------------------------------- the manager

    /// Java `OlympiadGameManager.run()`.
    pub(crate) fn olympiad_game_manager_run(&mut self) {
        if self.is_olympiad_end() {
            return;
        }
        if self.olympiad.in_comp_period {
            let mut ready_classed = self.has_enough_registered_classed();
            let mut ready_non_classed = self.has_enough_registered_non_classed();
            let mut ready_teams = self.has_enough_registered_teams();
            if ready_classed.is_none() && !ready_non_classed && !ready_teams {
                return;
            }
            // "set up the games queue"
            let count = self.olympiad.stadiums().len();
            for i in 0..count {
                let running = self.task(i).is_some_and(GameTask::is_running);
                if !running {
                    // "Fair arena distribution": 0,2,4,6,8.. arenas checked for classed or teams
                    // first.
                    if (ready_classed.is_some() || ready_teams) && i % 2 == 0 {
                        // "0,4,8.. arenas checked for teams first"
                        if ready_teams && i % 4 == 0 {
                            if let Some(game) = self.create_teams_game(i) {
                                self.olympiad_attach_game(i, game);
                                continue;
                            }
                            ready_teams = false;
                        }
                        // "if no ready teams found check for classed"
                        if let Some(ready) = ready_classed.as_mut() {
                            if let Some(game) = self.create_classed_game(i, ready) {
                                self.olympiad_attach_game(i, game);
                                continue;
                            }
                            ready_classed = None;
                        }
                    }
                    // "1,3,5,7,9.. arenas used for non-classed."
                    if ready_non_classed {
                        if let Some(game) = self.create_non_classed_game(i) {
                            self.olympiad_attach_game(i, game);
                            continue;
                        }
                        ready_non_classed = false;
                    }
                }
                // "Stop generating games if no more participants."
                if ready_classed.is_none() && !ready_non_classed && !ready_teams {
                    break;
                }
            }
        } else if self.olympiad_all_tasks_finished() {
            // "not in competition period"
            self.clear_registered();
            self.olympiad.battle_started = false;
        }
    }

    /// Java `isAllTasksFinished()`.
    pub fn olympiad_all_tasks_finished(&self) -> bool {
        !self.olympiad.stadiums().iter().any(|s| s.task.is_running())
    }

    /// Java `notifyCompetitorDamage(player, damage)`.
    pub(crate) fn olympiad_notify_competitor_damage(&mut self, player: ObjectId, damage: i32) {
        let Ok(index) = usize::try_from(self.olympiad_game_id(player)) else {
            return;
        };
        if index >= self.olympiad.stadiums().len() {
            return;
        }
        self.game_add_damage(index, player, damage);
    }

    /// The `Player.onHit` part (`sendDamageMessage`): damage dealt to a fighter of the same game
    /// counts for the result.
    pub(crate) fn olympiad_on_damage_dealt(&mut self, attacker: ObjectId, target: ObjectId, damage: i32) {
        if self.same_olympiad_game(attacker, target) {
            self.olympiad_notify_competitor_damage(attacker, damage);
        }
    }

    /// Java `OlympiadAnnouncer.run()`.
    pub(crate) fn olympiad_announcer_run(&mut self) {
        let count = self.olympiad.stadiums().len();
        for _ in 0..count {
            if self.olympiad.announcer_stadium >= count {
                self.olympiad.announcer_stadium = 0;
            }
            let current = self.olympiad.announcer_stadium;
            let announce = match self.task_mut(current) {
                Some(task) if task.game.is_some() => {
                    // `needAnnounce()` clears the flag.
                    let need = task.need_announce;
                    task.need_announce = false;
                    need.then(|| task.game.as_ref().map(|g| (g.kind, g.stadium_id)))
                        .flatten()
                }
                _ => None,
            };
            if let Some((kind, stadium_id)) = announce {
                let string = match kind {
                    CompetitionType::NonClassed => {
                        NpcStringId::OlympiadClassFreeIndividualMatchIsGoingToBeginInArenaS1InAMoment
                    }
                    CompetitionType::Classed => {
                        NpcStringId::OlympiadClassIndividualMatchIsGoingToBeginInArenaS1InAMoment
                    }
                    CompetitionType::Teams => {
                        NpcStringId::OlympiadClassFreeTeamMatchIsGoingToBeginInArenaS1InAMoment
                    }
                };
                let arena = (stadium_id + 1).to_string();
                let mut managers: Vec<ObjectId> = self
                    .npcs
                    .iter()
                    .filter(|(_, n)| n.npc_id == OLY_MANAGER)
                    .map(|(id, _)| *id)
                    .collect();
                managers.sort_unstable();
                for manager in managers {
                    self.broadcast_packet(
                        manager,
                        &NpcSay {
                            object_id: manager.0,
                            text_type: ChatType::NpcShout as i32,
                            npc_id: OLY_MANAGER,
                            npc_string: string as i32,
                            text: String::new(),
                            parameters: vec![arena.clone()],
                        },
                    );
                }
                return;
            }
            self.olympiad.announcer_stadium += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_teleport_countdown_follows_java() {
        let mut task = GameTask {
            count_down: 120,
            ..GameTask::default()
        };
        let mut seen = vec![task.count_down];
        let mut delays = Vec::new();
        for _ in 0..12 {
            delays.push(task.delay(&TELEPORT_TO_ARENA_TIMES));
            seen.push(task.count_down);
        }
        assert_eq!(delays, [60, 30, 15, 5, 5, 1, 1, 1, 1, 1, 1, 1]);
        assert_eq!(seen, [120, 60, 30, 15, 10, 5, 4, 3, 2, 1, -1, -1, -1]);
    }

    #[test]
    fn the_game_states_keep_java_order() {
        assert!(GameState::GameStarted < GameState::Cleanup);
        let task = GameTask {
            state: GameState::TeleportToTown,
            ..GameTask::default()
        };
        assert!(task.is_game_started() && task.is_battle_finished() && task.is_running());
        assert!(!GameTask::default().is_running());
    }
}
