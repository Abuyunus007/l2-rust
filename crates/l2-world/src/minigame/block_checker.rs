//! Handy's Block Checker: the arena lobby (`HandysBlockCheckerManager`, `ArenaParticipantsHolder`)
//! and the match (`BlockCheckerManager`).
// Java: managers/HandysBlockCheckerManager.java
// Java: managers/games/BlockCheckerManager.java (`StartEvent`, `SpawnRound`, `EndEvent`)
// Java: entity/actor/holders/player/ArenaParticipantsHolder.java
// Java: entity/actor/instance/Block.java (`changeColor`, `increaseTeamPointsAndSend`, `dropItem`)
// Java: dist/game/data/scripts/handlers/skill/effects/FlipBlock.java (the effect's rule)
// Java: dist/game/data/scripts/ai/areas/FantasyIsle/HandysBlockCheckerEvent.java (`onFirstTalk`)
//
// # Teams
// Team 0 is red, team 1 blue (Java's `ArenaParticipantsHolder`); `World::set_team` takes the
// `Team` ids (1 blue, 2 red).
//
// # The match
// `StartEvent` (queued by the last ready vote), then `SpawnRound(16 blocks, 1)`, one minute later
// 20 blocks, one minute later 14 blocks and the end three minutes after that. `EndEvent` pays,
// sends everybody back to Handy and frees the arena. Java queues `SpawnRound` on the thread pool
// from inside `StartEvent` and then sets the players up, which makes a race of the first block
// count and the start time; here the first round is one tick after the players are set up.

use std::collections::{BTreeMap, HashSet};

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    ActionFailed, ExBasicActionList, ExCubeGameAddPlayer, ExCubeGameChangePoints, ExCubeGameChangeTeam,
    ExCubeGameChangeTimeToStart, ExCubeGameCloseUi, ExCubeGameEnd, ExCubeGameExtendedChangePoints,
    ExCubeGameRemovePlayer, ExCubeGameRequestReady, ExCubeGameTeamList, RelationChanged, SystemMessage,
    SystemMessageParam,
};
use l2_util::ObjectId;
use tracing::{error, warn};

use super::{MinigameTimer, TEAM_BLUE, TEAM_NONE, TEAM_RED};
use crate::effects::SkillKey;
use crate::ground_item::GroundItem;
use crate::teleport::TeleportTarget;
use crate::timer::TimerId;
use crate::world::World;
use crate::zones::ZoneId;

/// `_arenaCoordinates`: team 1 x y, team 2 x y, arena centre x y.
const ARENA_COORDINATES: [[i32; 6]; 4] = [
    [-58368, -62745, -57751, -62131, -58053, -62417],
    [-58350, -63853, -57756, -63266, -58053, -63551],
    [-57194, -63861, -56580, -63249, -56886, -63551],
    [-57200, -62727, -56584, -62115, -56850, -62391],
];
/// `Z_COORD`.
const Z_COORD: i32 = -2405;
/// Where everybody goes back to, near Handy.
const BACK_TO_HANDY: (i32, i32, i32) = (-57478, -60367, -2370);
/// The block and the block carrying girl.
const BLOCK_NPC_ID: i32 = 18672;
const GIRL_NPC_ID: i32 = 18676;
/// Handy's Block Checker Bond and Land Mine.
const BOND_ITEM_ID: i32 = 13787;
const LAND_MINE_ITEM_ID: i32 = 13788;
/// The reward (Handy's Block Checker Coupon... `13067`).
const REWARD_ITEM_ID: i32 = 13067;
/// `Block._colorEffect` of a red block; blue is 0.
const RED_COLOR: i32 = 0x53;
/// `HandysBlockCheckerManager.schedulePenaltyRemoval`.
const PENALTY_MS: u64 = 10_000;
/// `ThreadPool.schedule(new SpawnRound(...), 60000)`.
const ROUND_MS: u64 = 60_000;
/// `ThreadPool.schedule(new EndEvent(), 180000)`.
const END_DELAY_MS: u64 = 180_000;
/// `CarryingGirlUnspawn` after 9 seconds.
const GIRL_MS: u64 = 9_000;
/// `_startedTime = now + 300000`.
const MATCH_MS: i64 = 300_000;
/// Skills of the match: freeze, red and blue transformation.
const FREEZE_SKILL: SkillKey = SkillKey { id: 6034, level: 1 };
const RED_TRANSFORMATION_SKILL: SkillKey = SkillKey { id: 6035, level: 1 };
const BLUE_TRANSFORMATION_SKILL: SkillKey = SkillKey { id: 6036, level: 1 };

/// What a Block Checker timer does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockTask {
    /// `removePenalty(objectId)`.
    PenaltyEnd(ObjectId),
    /// `StartEvent.run()` of an arena.
    Start(i32),
    /// `SpawnRound(boxes, round).run()`.
    Spawn { arena: i32, boxes: i32, round: i32 },
    /// `CarryingGirlUnspawn.run()`.
    GirlUnspawn(ObjectId),
    /// `EndEvent.run()`.
    End(i32),
}

/// `BlockCheckerManager`: the engine of one arena.
#[derive(Debug, Default)]
pub(crate) struct BlockEngine {
    /// `_redTeamPoints`, `_blueTeamPoints`.
    red_player_points: BTreeMap<ObjectId, i32>,
    blue_player_points: BTreeMap<ObjectId, i32>,
    /// `_redPoints`, `_bluePoints` (start at 15).
    red_points: i32,
    blue_points: i32,
    /// `_spawns`: the block NPCs.
    spawns: Vec<ObjectId>,
    /// `_drops`: the items dropped in the event.
    drops: Vec<ObjectId>,
    /// `_isRedWinner`: kept from match to match, a tie does not change it.
    is_red_winner: bool,
    /// `_startedTime`: when the match ends (`now + 300000`) on the minigame clock.
    started_time_ms: i64,
    /// `_isStarted`.
    started: bool,
    /// `_abnormalEnd`.
    abnormal_end: bool,
    /// `_task`.
    task: Option<TimerId>,
}

impl BlockEngine {
    fn new() -> Self {
        Self {
            red_points: 15,
            blue_points: 15,
            ..Self::default()
        }
    }
}

/// `ArenaParticipantsHolder` and its engine.
#[derive(Debug)]
pub(crate) struct BlockArena {
    red: Vec<ObjectId>,
    blue: Vec<ObjectId>,
    engine: BlockEngine,
}

impl BlockArena {
    fn new() -> Self {
        Self {
            red: Vec::new(),
            blue: Vec::new(),
            engine: BlockEngine::new(),
        }
    }

    /// `getAllPlayers()`: red, then blue.
    fn all(&self) -> Vec<ObjectId> {
        self.red.iter().chain(self.blue.iter()).copied().collect()
    }

    /// `getPlayerTeam(player)`: 0 red, 1 blue, -1 none.
    fn team_of(&self, player: ObjectId) -> i32 {
        if self.red.contains(&player) {
            0
        } else if self.blue.contains(&player) {
            1
        } else {
            -1
        }
    }

    /// `addPlayer(player, team)`.
    fn add(&mut self, player: ObjectId, team: i32) {
        if team == 0 {
            self.red.push(player);
        } else {
            self.blue.push(player);
        }
    }

    /// `removePlayer(player, team)`: the first match.
    fn remove(&mut self, player: ObjectId, team: i32) {
        let list = if team == 0 { &mut self.red } else { &mut self.blue };
        if let Some(index) = list.iter().position(|p| *p == player) {
            list.remove(index);
        }
    }
}

/// `HandysBlockCheckerManager`'s state.
#[derive(Debug, Default)]
pub(crate) struct BlockWorld {
    /// `_arenaPlayers`: empty until `startUpParticipantsQueue`.
    arenas: Vec<BlockArena>,
    /// `_arenaVotes`.
    votes: [i32; 4],
    /// `_arenaStatus`.
    status: [bool; 4],
    /// `_registrationPenalty`.
    penalty: HashSet<ObjectId>,
}

/// The lobby of one arena, for tests and tools.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockArenaView {
    pub red: Vec<ObjectId>,
    pub blue: Vec<ObjectId>,
    pub red_points: i32,
    pub blue_points: i32,
    /// `isStarted()`.
    pub started: bool,
    /// The block NPCs of the match.
    pub blocks: Vec<ObjectId>,
}

fn arena_index(arena: i32) -> Option<usize> {
    usize::try_from(arena).ok().filter(|a| *a < 4)
}

impl World {
    // ------------------------------------------------------------------ the lobby

    /// Java `HandysBlockCheckerManager.startUpParticipantsQueue()`.
    pub fn start_block_checker_queue(&mut self) {
        self.minigames.block.arenas = (0..4).map(|_| BlockArena::new()).collect();
    }

    /// The lobby and the engine of one arena.
    pub fn block_arena(&self, arena: i32) -> Option<BlockArenaView> {
        let holder = self.minigames.block.arenas.get(arena_index(arena)?)?;
        Some(BlockArenaView {
            red: holder.red.clone(),
            blue: holder.blue.clone(),
            red_points: holder.engine.red_points,
            blue_points: holder.engine.blue_points,
            started: holder.engine.started,
            blocks: holder.engine.spawns.clone(),
        })
    }

    /// Java `HandysBlockCheckerManager.arenaIsBeingUsed(arenaId)`.
    pub fn block_arena_is_being_used(&self, arena: i32) -> bool {
        arena_index(arena).is_some_and(|a| self.minigames.block.status[a])
    }

    /// The points of a player in the running match (`getPlayerPoints`).
    pub fn block_player_points(&self, arena: i32, player: ObjectId, is_red: bool) -> i32 {
        let Some(holder) = arena_index(arena).and_then(|a| self.minigames.block.arenas.get(a)) else {
            return 0;
        };
        let engine = &holder.engine;
        if !engine.red_player_points.contains_key(&player) && !engine.blue_player_points.contains_key(&player)
        {
            return 0;
        }
        let map = if is_red {
            &engine.red_player_points
        } else {
            &engine.blue_player_points
        };
        map.get(&player).copied().unwrap_or(0)
    }

    /// `broadCastPacketToTeam(packet)`: red, then blue.
    fn block_broadcast(&self, arena: usize, packet: &impl l2_net::Encode) {
        let Some(holder) = self.minigames.block.arenas.get(arena) else {
            return;
        };
        for player in holder.all() {
            self.send(player, packet);
        }
    }

    /// `HandysBlockCheckerEvent.onFirstTalk(npc, player)` for the arena manager of `arena`, the
    /// body of the script: the registration answers and, when both teams have the minimum, the
    /// ready question.
    pub fn block_checker_first_talk(&mut self, player: ObjectId, arena: i32) {
        let Some(index) = arena_index(arena) else {
            return;
        };
        if self.minigames.block.arenas.get(index).is_none() {
            return;
        }
        // `eventIsFull(arena)`.
        if self.minigames.block.arenas[index].all().len() == 12 {
            self.minigame_system_message(player, SystemMessageId::YouCannotRegisterBecauseCapacityHasBeenExceeded);
            return;
        }
        if self.block_arena_is_being_used(arena) {
            self.minigame_system_message(player, SystemMessageId::TheMatchIsBeingPreparedPleaseTryAgainLater);
            return;
        }
        if self.block_checker_add_player(player, arena) {
            let holder = &self.minigames.block.arenas[index];
            // Java sends `new ExCubeGameTeamList(red, blue, arena)`, which stores `arena - 1`.
            let list = ExCubeGameTeamList {
                room: arena - 1,
                blue: self.block_cube_players(&holder.blue),
                red: self.block_cube_players(&holder.red),
            };
            self.send(player, &list);
            let minimum = self.minigames.settings.min_block_checker_team_members as usize;
            let holder = &self.minigames.block.arenas[index];
            if holder.blue.len() >= minimum && holder.red.len() >= minimum {
                // `holder.updateEvent()` hands the same holder to the engine: nothing to do.
                self.block_broadcast(index, &ExCubeGameRequestReady);
                self.block_broadcast(index, &ExCubeGameChangeTimeToStart { seconds: 10 });
            }
        }
    }

    fn block_cube_players(&self, players: &[ObjectId]) -> Vec<l2_protocol_game::server::CubePlayer> {
        players
            .iter()
            .map(|p| l2_protocol_game::server::CubePlayer {
                object_id: p.0,
                name: self.minigame_player_name(*p),
            })
            .collect()
    }

    /// Java `HandysBlockCheckerManager.addPlayerToArena(player, arenaId)`.
    pub fn block_checker_add_player(&mut self, player: ObjectId, arena: i32) -> bool {
        let Some(index) = arena_index(arena).filter(|a| *a < self.minigames.block.arenas.len()) else {
            return false;
        };
        for holder in &self.minigames.block.arenas {
            if holder.all().contains(&player) {
                let name = self.minigame_player_name(player);
                self.send(
                    player,
                    &SystemMessage::new(SystemMessageId::C1IsAlreadyRegisteredOnTheMatchWaitingList)
                        .with(SystemMessageParam::Text(name)),
                );
                return false;
            }
        }
        if self.is_cursed_weapon_equipped(player) {
            self.minigame_system_message(player, SystemMessageId::YouCannotRegisterWhileInPossessionOfACursedWeapon);
            return false;
        }
        if self.is_registered_on_event(player) || self.minigame_in_olympiad_mode(player) {
            self.send_message(player, "Could not register you due other event participation.");
            return false;
        }
        if self.minigame_olympiad_registered(player) {
            self.minigame_olympiad_unregister(player);
            self.minigame_system_message(
                player,
                SystemMessageId::ApplicantsForTheOlympiadUndergroundColiseumOrKrateiSCubeMatchesCannotRegister,
            );
        }
        if self.minigames.block.penalty.contains(&player) {
            self.minigame_system_message(player, SystemMessageId::YouMustWait10SecondsBeforeAttemptingToRegisterAgain);
            return false;
        }
        let holder = &mut self.minigames.block.arenas[index];
        let is_red = if holder.blue.len() < holder.red.len() {
            holder.add(player, 1);
            false
        } else {
            holder.add(player, 0);
            true
        };
        let packet = ExCubeGameAddPlayer {
            red_team: is_red,
            object_id: player.0,
            name: self.minigame_player_name(player),
        };
        self.block_broadcast(index, &packet);
        true
    }

    /// Java `HandysBlockCheckerManager.removePlayer(player, arenaId, team)`.
    pub fn block_checker_remove_player(&mut self, player: ObjectId, arena: i32, team: i32) {
        let Some(index) = arena_index(arena).filter(|a| *a < self.minigames.block.arenas.len()) else {
            return;
        };
        let is_red = team == 0;
        let holder = &mut self.minigames.block.arenas[index];
        holder.remove(player, team);
        self.block_broadcast(
            index,
            &ExCubeGameRemovePlayer {
                red_team: is_red,
                object_id: player.0,
            },
        );
        // End event if there is an empty team.
        let holder = &self.minigames.block.arenas[index];
        let team_size = if is_red { holder.red.len() } else { holder.blue.len() };
        if team_size == 0 {
            self.block_end_event_abnormally(index);
        }
        self.minigames.block.penalty.insert(player);
        self.minigame_schedule(PENALTY_MS, MinigameTimer::Block(BlockTask::PenaltyEnd(player)));
    }

    /// Java `HandysBlockCheckerManager.changePlayerToTeam(player, arena)`.
    // NOTE(java-quirk): a player who is in neither team is "not from red", so he is put in the red
    // team without any registration check.
    pub fn block_checker_change_player_to_team(&mut self, player: ObjectId, arena: i32) {
        let Some(index) = arena_index(arena).filter(|a| *a < self.minigames.block.arenas.len()) else {
            return;
        };
        let holder = &mut self.minigames.block.arenas[index];
        let is_from_red = holder.red.contains(&player);
        if (is_from_red && holder.blue.len() == 6) || (!is_from_red && holder.red.len() == 6) {
            self.send_message(player, "The team is full");
            return;
        }
        let future_team = i32::from(is_from_red);
        holder.add(player, future_team);
        holder.remove(player, if is_from_red { 0 } else { 1 });
        self.block_broadcast(
            index,
            &ExCubeGameChangeTeam {
                object_id: player.0,
                from_red_team: is_from_red,
            },
        );
    }

    /// Java `HandysBlockCheckerManager.increaseArenaVotes(arena)`.
    pub fn block_checker_increase_arena_votes(&mut self, arena: i32) {
        let Some(index) = arena_index(arena).filter(|a| *a < self.minigames.block.arenas.len()) else {
            return;
        };
        let new_votes = self.minigames.block.votes[index] + 1;
        let holder = &self.minigames.block.arenas[index];
        if new_votes as usize > holder.all().len() / 2 && !holder.engine.started {
            // `clearArenaVotes(arena)`.
            self.minigames.block.votes[index] = 0;
            let holder = &self.minigames.block.arenas[index];
            if holder.blue.is_empty() || holder.red.is_empty() {
                return;
            }
            if self.minigames.settings.hbce_fair_play {
                self.block_check_and_shuffle(index);
            }
            // `ThreadPool.execute(holder.getEvent().new StartEvent())`.
            self.minigame_schedule(1, MinigameTimer::Block(BlockTask::Start(index as i32)));
        } else {
            self.minigames.block.votes[index] = new_votes;
        }
    }

    /// Java `ArenaParticipantsHolder.checkAndShuffle()`.
    fn block_check_and_shuffle(&mut self, index: usize) {
        let holder = &self.minigames.block.arenas[index];
        let (red_size, blue_size) = (holder.red.len(), holder.blue.len());
        let (from, count) = if red_size > blue_size + 1 {
            (holder.red.clone(), red_size - (blue_size + 1) + 1)
        } else if blue_size > red_size + 1 {
            (holder.blue.clone(), blue_size - (red_size + 1) + 1)
        } else {
            return;
        };
        self.block_broadcast(
            index,
            &SystemMessage::new(SystemMessageId::TeamMembersWereModifiedBecauseTheTeamsWereUnbalanced),
        );
        // The players are the first ones of the list as it was before any change.
        for player in from.into_iter().take(count) {
            self.block_checker_change_player_to_team(player, index as i32);
        }
    }

    /// Java `HandysBlockCheckerManager.onDisconnect(player)`.
    pub fn block_checker_on_disconnect(&mut self, player: ObjectId) {
        let arena = self.block_checker_arena_of(player);
        let Some(index) = arena_index(arena).filter(|a| *a < self.minigames.block.arenas.len()) else {
            return;
        };
        let team = self.minigames.block.arenas[index].team_of(player);
        self.block_checker_remove_player(player, arena, team);
        if self.team_of(player) != TEAM_NONE {
            self.stop_all_effects(player);
            // Remove team aura.
            self.set_team(player, TEAM_NONE);
            // Remove the event items.
            self.block_destroy_event_items(player);
            self.set_inside(player, ZoneId::Pvp, false);
            // Teleport back.
            self.tele_to_location(
                player,
                TeleportTarget::at(BACK_TO_HANDY.0, BACK_TO_HANDY.1, BACK_TO_HANDY.2),
            );
        }
    }

    /// `inv.getItemByItemId(13787) != null` then `destroyItemByItemId(DESTROY, ..., count)` for the
    /// two event items.
    fn block_destroy_event_items(&mut self, player: ObjectId) {
        for item_id in [BOND_ITEM_ID, LAND_MINE_ITEM_ID] {
            if self.inventory_item_count(player, item_id) > 0 {
                self.destroy_item_by_item_id(player, item_id, -1, false);
            }
        }
    }

    /// `RequestExCubeGameChangeTeam.runImpl`.
    pub(crate) fn on_cube_game_change_team(&mut self, player: ObjectId, arena: i32, team: i32) {
        // Do not remove players after start.
        if self.block_arena_is_being_used(arena) {
            return;
        }
        let Some(index) = arena_index(arena).filter(|a| *a < self.minigames.block.arenas.len()) else {
            return;
        };
        match team {
            0 | 1 => self.block_checker_change_player_to_team(player, arena),
            -1 => {
                let team = self.minigames.block.arenas[index].team_of(player);
                // The client sends this twice if the player clicks on exit.
                if team > -1 {
                    self.block_checker_remove_player(player, arena, team);
                }
            }
            _ => warn!("Wrong Cube Game Team ID: {team}"),
        }
    }

    /// `RequestExCubeGameReadyAnswer.runImpl`.
    pub(crate) fn on_cube_game_ready_answer(&mut self, player: ObjectId, arena: i32, answer: i32) {
        if !self.players.contains_key(&player) {
            return;
        }
        match answer {
            // Cancel - Answer No.
            0 => {}
            // OK or Time Over.
            1 => self.block_checker_increase_arena_votes(arena),
            _ => warn!("Unknown Cube Game Answer ID: {answer}"),
        }
    }

    // ------------------------------------------------------------------ the match

    /// `BlockCheckerManager.endEventAbnormally()`.
    fn block_end_event_abnormally(&mut self, index: usize) {
        let engine = &mut self.minigames.block.arenas[index].engine;
        engine.started = false;
        let task = engine.task.take();
        engine.abnormal_end = true;
        if let Some(task) = task {
            self.timers.cancel(task);
        }
        // `ThreadPool.execute(new EndEvent())`.
        self.minigame_schedule(1, MinigameTimer::Block(BlockTask::End(index as i32)));
    }

    pub(crate) fn on_block_timer(&mut self, task: BlockTask) {
        match task {
            BlockTask::PenaltyEnd(player) => {
                self.minigames.block.penalty.remove(&player);
            }
            BlockTask::Start(arena) => self.block_start_event(arena),
            BlockTask::Spawn { arena, boxes, round } => self.block_spawn_round(arena, boxes, round),
            BlockTask::GirlUnspawn(npc) => {
                self.delete_npc(npc);
            }
            BlockTask::End(arena) => self.block_end_event(arena),
        }
    }

    /// `StartEvent.run()`.
    fn block_start_event(&mut self, arena: i32) {
        let Some(index) = arena_index(arena).filter(|a| *a < self.minigames.block.arenas.len()) else {
            error!("Could not set up the arena Id for the Block Checker event, cancelling event...");
            return;
        };
        self.minigames.block.arenas[index].engine.started = true;
        // Spawn the blocks: `ThreadPool.execute(new SpawnRound(16, 1))`.
        self.minigame_schedule(
            1,
            MinigameTimer::Block(BlockTask::Spawn {
                arena,
                boxes: 16,
                round: 1,
            }),
        );
        // Start up player parameters.
        self.block_set_up_players(index);
        // Set the started time.
        self.minigames.block.arenas[index].engine.started_time_ms = self.minigame_now_ms() + MATCH_MS;
    }

    /// `StartEvent.setUpPlayers()`.
    fn block_set_up_players(&mut self, index: usize) {
        // Set current arena as being used.
        self.minigames.block.status[index] = true;
        // Initialize packets avoiding create a new one per player.
        let holder = &mut self.minigames.block.arenas[index];
        let half = (holder.engine.spawns.len() / 2) as i32;
        holder.engine.red_points = half;
        holder.engine.blue_points = half;
        let (blue_points, red_points) = (holder.engine.blue_points, holder.engine.red_points);
        let players = holder.all();
        let red = holder.red.clone();
        let initial_points = ExCubeGameChangePoints {
            time_left: 300,
            blue_points,
            red_points,
        };
        for player in players {
            if !self.players.contains_key(&player) {
                continue;
            }
            // Send the secret client packet set up.
            let is_red = red.contains(&player);
            self.send(
                player,
                &ExCubeGameExtendedChangePoints {
                    time_left: 300,
                    blue_points,
                    red_points,
                    red_team: is_red,
                    object_id: player.0,
                    player_points: 0,
                },
            );
            self.send(player, &ActionFailed);
            // Teleport player: team 0 uses the first pair of coordinates, team 1 the second.
            let team_index = if is_red { 0 } else { 1 };
            let x = ARENA_COORDINATES[index][team_index * 2];
            let y = ARENA_COORDINATES[index][team_index * 2 + 1];
            self.tele_to_location(player, TeleportTarget::at(x, y, Z_COORD));
            // Set the player team.
            let engine = &mut self.minigames.block.arenas[index].engine;
            if is_red {
                engine.red_player_points.insert(player, 0);
                self.set_team(player, TEAM_RED);
            } else {
                engine.blue_player_points.insert(player, 0);
                self.set_team(player, TEAM_BLUE);
            }
            self.stop_all_effects(player);
            if let Some(summon) = self.summon_of(player) {
                self.unsummon(summon, Some(player));
            }
            // Give the player start up effects: freeze, then the transformation of the team.
            self.apply_effects(player, player, FREEZE_SKILL);
            self.apply_effects(
                player,
                player,
                if is_red {
                    RED_TRANSFORMATION_SKILL
                } else {
                    BLUE_TRANSFORMATION_SKILL
                },
            );
            // Set the current player arena.
            self.set_block_checker_arena(player, index as i32);
            self.set_inside(player, ZoneId::Pvp, true);
            // Send needed packets.
            self.send(player, &initial_points);
            self.send(player, &ExCubeGameCloseUi);
            self.send(player, &ExBasicActionList::default_list());
            self.block_broadcast_relation_changed(index, player);
        }
    }

    /// `BlockCheckerManager.broadcastRelationChanged(plr)`.
    fn block_broadcast_relation_changed(&self, index: usize, subject: ObjectId) {
        let Some(holder) = self.minigames.block.arenas.get(index) else {
            return;
        };
        for viewer in holder.all() {
            let entry = self.relation_entry(subject, viewer);
            self.send(viewer, &RelationChanged::single(entry));
        }
    }

    /// `SpawnRound.run()`.
    fn block_spawn_round(&mut self, arena: i32, boxes: i32, round: i32) {
        let Some(index) = arena_index(arena).filter(|a| *a < self.minigames.block.arenas.len()) else {
            return;
        };
        if !self.minigames.block.arenas[index].engine.started {
            return;
        }
        let next = match round {
            // Schedule second spawn round.
            1 => Some((ROUND_MS, MinigameTimer::Block(BlockTask::Spawn { arena, boxes: 20, round: 2 }))),
            // Schedule third spawn round.
            2 => Some((ROUND_MS, MinigameTimer::Block(BlockTask::Spawn { arena, boxes: 14, round: 3 }))),
            // Schedule event end count down.
            3 => Some((END_DELAY_MS, MinigameTimer::Block(BlockTask::End(arena)))),
            _ => None,
        };
        if let Some((delay, timer)) = next {
            let id = self.minigame_schedule(delay, timer);
            self.minigames.block.arenas[index].engine.task = Some(id);
        }
        // random % 2 == 0 spawns a red block: the first one is red, then they alternate.
        let center = (ARENA_COORDINATES[index][4], ARENA_COORDINATES[index][5]);
        for i in 0..boxes {
            let x = center.0 + self.rng().get(-400, 400);
            let y = center.1 + self.rng().get(-400, 400);
            match self.spawn_npc_at(BLOCK_NPC_ID, x, y, Z_COORD, 1) {
                Some(block) => {
                    self.minigames
                        .block_color
                        .insert(block, if i % 2 == 0 { RED_COLOR } else { 0 });
                    self.disable_core_ai(block, true);
                    self.minigames.block.arenas[index].engine.spawns.push(block);
                }
                None => warn!("BlockCheckerManager: could not spawn the block {BLOCK_NPC_ID}"),
            }
        }
        // Spawn the block carrying girl.
        if round == 1 || round == 2 {
            let x = center.0 + self.rng().get(-400, 400);
            let y = center.1 + self.rng().get(-400, 400);
            match self.spawn_npc_at(GIRL_NPC_ID, x, y, Z_COORD, 1) {
                // Schedule her deletion after 9 secs of spawn.
                Some(girl) => {
                    self.minigame_schedule(GIRL_MS, MinigameTimer::Block(BlockTask::GirlUnspawn(girl)));
                }
                None => warn!("Could not Spawn Block Checker NPCs! Wrong instance type at npc table?"),
            }
        }
        let engine = &mut self.minigames.block.arenas[index].engine;
        engine.red_points += boxes / 2;
        engine.blue_points += boxes / 2;
        let (blue, red) = (engine.blue_points, engine.red_points);
        let time_left = self.block_time_left(index);
        self.block_broadcast(
            index,
            &ExCubeGameChangePoints {
                time_left,
                blue_points: blue,
                red_points: red,
            },
        );
    }

    /// `(int) ((_startedTime - System.currentTimeMillis()) / 1000)`.
    fn block_time_left(&self, index: usize) -> i32 {
        let started = self.minigames.block.arenas[index].engine.started_time_ms;
        ((started - self.minigame_now_ms()) / 1000) as i32
    }

    /// `EndEvent.run()`.
    fn block_end_event(&mut self, arena: i32) {
        let Some(index) = arena_index(arena).filter(|a| *a < self.minigames.block.arenas.len()) else {
            return;
        };
        if !self.minigames.block.arenas[index].engine.abnormal_end {
            self.block_reward_players(index);
        }
        self.block_set_players_back(index);
        self.block_clear(index);
        let engine = &mut self.minigames.block.arenas[index].engine;
        engine.started = false;
        engine.abnormal_end = false;
    }

    /// `EndEvent.rewardPlayers()`: a tie pays nothing.
    fn block_reward_players(&mut self, index: usize) {
        let engine = &self.minigames.block.arenas[index].engine;
        if engine.red_points == engine.blue_points {
            return;
        }
        let red_wins = engine.red_points > engine.blue_points;
        self.minigames.block.arenas[index].engine.is_red_winner = red_wins;
        if red_wins {
            self.block_reward_as_winner(index, true);
            self.block_reward_as_loser(index, false);
        } else {
            self.block_reward_as_winner(index, false);
            self.block_reward_as_loser(index, true);
        }
        let team = if red_wins { "Red Team" } else { "Blue Team" };
        self.block_broadcast(
            index,
            &SystemMessage::new(SystemMessageId::TheC1TeamHasWon)
                .with(SystemMessageParam::Text(team.to_owned())),
        );
    }

    /// `EndEvent.rewardAsWinner(isRed)`: 2 coupons for 10 points or more, 8 for the best and 5 for
    /// the second best.
    fn block_reward_as_winner(&mut self, index: usize, is_red: bool) {
        let engine = &mut self.minigames.block.arenas[index].engine;
        let points = if is_red {
            &mut engine.red_player_points
        } else {
            &mut engine.blue_player_points
        };
        // Main give; the players under 10 points leave the list.
        points.retain(|_, value| *value >= 10);
        let entries: Vec<(ObjectId, i32)> = points.iter().map(|(p, v)| (*p, *v)).collect();
        for (player, _) in &entries {
            self.add_item(*player, REWARD_ITEM_ID, 2, true);
        }
        let (mut first, mut second) = (0, 0);
        let (mut winner1, mut winner2): (Option<ObjectId>, Option<ObjectId>) = (None, None);
        for (player, value) in &entries {
            if *value > first {
                // Move old data.
                second = first;
                winner2 = winner1;
                // Set new data.
                first = *value;
                winner1 = Some(*player);
            } else if *value > second {
                second = *value;
                winner2 = Some(*player);
            }
        }
        if let Some(winner) = winner1 {
            self.add_item(winner, REWARD_ITEM_ID, 8, true);
        }
        if let Some(winner) = winner2 {
            self.add_item(winner, REWARD_ITEM_ID, 5, true);
        }
    }

    /// `EndEvent.rewardAsLooser(isRed)`: 2 coupons for 10 points or more.
    fn block_reward_as_loser(&mut self, index: usize, is_red: bool) {
        let engine = &self.minigames.block.arenas[index].engine;
        let points = if is_red {
            &engine.red_player_points
        } else {
            &engine.blue_player_points
        };
        let winners: Vec<ObjectId> = points
            .iter()
            .filter(|(_, value)| **value >= 10)
            .map(|(player, _)| *player)
            .collect();
        for player in winners {
            self.add_item(player, REWARD_ITEM_ID, 2, true);
        }
    }

    /// `EndEvent.setPlayersBack()`.
    fn block_set_players_back(&mut self, index: usize) {
        let holder = &self.minigames.block.arenas[index];
        let players = holder.all();
        let end = ExCubeGameEnd {
            red_team_win: holder.engine.is_red_winner,
        };
        for player in players {
            if !self.players.contains_key(&player) {
                continue;
            }
            self.stop_all_effects(player);
            // Remove team aura.
            self.set_team(player, TEAM_NONE);
            // Set default arena.
            self.set_block_checker_arena(player, -1);
            // Untransform.
            self.untransform(player);
            // Remove the event items.
            self.block_destroy_event_items(player);
            self.block_broadcast_relation_changed(index, player);
            // Teleport back.
            self.tele_to_location(
                player,
                TeleportTarget::at(BACK_TO_HANDY.0, BACK_TO_HANDY.1, BACK_TO_HANDY.2),
            );
            self.set_inside(player, ZoneId::Pvp, false);
            // Send end packet.
            self.send(player, &end);
            self.broadcast_user_info(player);
        }
    }

    /// `EndEvent.clearMe()`: the garbage collector and the arena free setter.
    fn block_clear(&mut self, index: usize) {
        let holder = &mut self.minigames.block.arenas[index];
        holder.red.clear();
        holder.blue.clear();
        holder.engine.blue_player_points.clear();
        holder.engine.red_player_points.clear();
        self.minigames.block.status[index] = false;
        let spawns = std::mem::take(&mut self.minigames.block.arenas[index].engine.spawns);
        for block in spawns {
            self.minigames.block_color.remove(&block);
            self.delete_npc(block);
        }
        let drops = std::mem::take(&mut self.minigames.block.arenas[index].engine.drops);
        for item in drops {
            // A player has it (the loot privilege is still on) or it is gone: it is deleted later.
            let free = self
                .ground
                .items
                .get(&item)
                .is_some_and(|ground| ground.owner.is_none());
            if free {
                self.decay_ground_item(item);
            }
        }
    }

    // ------------------------------------------------------------------ the blocks

    /// Java `FlipBlock.onStart(effector, effected)`: `player` hit `block` with the flip effect. A
    /// red player flips a red... a block of the other colour than his team's: team 0 turns the
    /// blocks that are 0x00 (blue), team 1 those that are 0x53 (red).
    pub fn block_checker_flip_block(&mut self, player: ObjectId, block: ObjectId) {
        let Some(&color) = self.minigames.block_color.get(&block) else {
            return;
        };
        let arena = self.block_checker_arena_of(player);
        let Some(index) = arena_index(arena).filter(|a| *a < self.minigames.block.arenas.len()) else {
            return;
        };
        let team = self.minigames.block.arenas[index].team_of(player);
        if (team == 0 && color == 0x00) || (team == 1 && color == RED_COLOR) {
            self.block_change_color(index, player, block, team);
        }
    }

    /// `Block.changeColor(attacker, holder, team)`.
    fn block_change_color(&mut self, index: usize, attacker: ObjectId, block: ObjectId, team: i32) {
        let color = self.minigame_npc_color_effect(block);
        // Change color, tell everybody who knows the block, count the point.
        self.minigames
            .block_color
            .insert(block, if color == RED_COLOR { 0x00 } else { RED_COLOR });
        self.broadcast_info(block);
        self.block_increase_team_points_and_send(index, attacker, team);
        // 30% chance to drop the event items.
        let random = self.rng().below(100);
        if random > 69 && random <= 84 {
            self.block_drop_item(index, BOND_ITEM_ID, block, attacker);
        } else if random > 84 {
            self.block_drop_item(index, LAND_MINE_ITEM_ID, block, attacker);
        }
    }

    /// `Block.increaseTeamPointsAndSend(player, team, eng)`.
    fn block_increase_team_points_and_send(&mut self, index: usize, player: ObjectId, team: i32) {
        // `increasePlayerPoints(player, team)`.
        let engine = &mut self.minigames.block.arenas[index].engine;
        if team == 0 {
            *engine.red_player_points.entry(player).or_insert(0) += 1;
            engine.red_points += 1;
            engine.blue_points -= 1;
        } else {
            *engine.blue_player_points.entry(player).or_insert(0) += 1;
            engine.blue_points += 1;
            engine.red_points -= 1;
        }
        let (blue, red) = (engine.blue_points, engine.red_points);
        let time_left = self.block_time_left(index);
        let is_red = self.minigames.block.arenas[index].red.contains(&player);
        let player_points = self.block_player_points(index as i32, player, is_red);
        self.block_broadcast(
            index,
            &ExCubeGameChangePoints {
                time_left,
                blue_points: blue,
                red_points: red,
            },
        );
        self.block_broadcast(
            index,
            &ExCubeGameExtendedChangePoints {
                time_left,
                blue_points: blue,
                red_points: red,
                red_team: is_red,
                object_id: player.0,
                player_points,
            },
        );
    }

    /// `Block.dropItem(id, eng, player)`: `ItemManager.createItem(LOOT, id, 1, player, this)` and
    /// `dropMe` next to the block.
    fn block_drop_item(&mut self, index: usize, item_id: i32, block: ObjectId, player: ObjectId) {
        let Some(templates) = self.item_templates() else {
            return;
        };
        let Some(template) = templates.get(item_id) else {
            error!("Item doesn't exist so cannot be dropped. Item ID: {item_id}");
            return;
        };
        let Some(id) = self.allocate_item_id() else {
            return;
        };
        let data = self.new_item_data(template, id, 1);
        let mut item = GroundItem::new(data);
        // `ItemManager.createItem` with `LOOT`: the loot privilege of the dropper.
        if !self.settings.ground.auto_loot && !self.settings.ground.auto_loot_item_ids.contains(&item_id) {
            self.set_loot_owner(&mut item, player);
        }
        let Some((bx, by, bz)) = self.position_of(block) else {
            self.release_item_id(id);
            return;
        };
        let x = bx + self.rng().below(50);
        let y = by + self.rng().below(50);
        if self.drop_me(item, Some(block), x, y, bz) {
            self.minigames.block.arenas[index].engine.drops.push(id);
        } else {
            self.release_item_id(id);
        }
    }
}
