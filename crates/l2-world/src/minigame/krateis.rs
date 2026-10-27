//! Kratei's Cube: the registration cycle (`KrateisCubeManager`) and the arenas (`KrateiArena`).
// Java: managers/games/KrateisCubeManager.java
// Java: mechanics/krateisCube/{KrateiArena,KrateiCubePlayer,KrateisReward,KrateiMsgType}.java
// Java: entity/actor/Player.java (`doDie`: the respawn task and the killer's points)
//
// # The cycle
// Twice an hour (minute 27 and 57 of the local time) registration closes; three minutes later the
// players near the entrance manager (`AltPartyRange`) are put in the waiting room of their arena,
// 11 seconds after that the match starts for twenty minutes: doors that open and close in turns,
// watchers that change, points for the kills (3 per player, 1 per monster), and the rewards by
// rank at the end.
//
// # Java's ConcurrentHashMap
// The players of an arena are a `ConcurrentHashMap<Player, KrateiCubePlayer>`, so Java's loops over
// them have no defined order; ties in the ranking depend on it. Here they go by object id.

use std::collections::BTreeMap;

use l2_data::KrateisArenaDef;
use l2_protocol_game::NpcStringId;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    CC_RECORD_INITIALIZE, CC_RECORD_UPDATE, CcRecordEntry, ChatType, CreatureSay, ExPvpMatchCcMyRecord,
    ExPvpMatchCcRecord, ExPvpMatchCcRetire, SayBody, SenderField, SystemMessage, SystemMessageParam,
};
use l2_util::ObjectId;
use tracing::info;

use super::MinigameTimer;
use crate::effects::SkillKey;
use crate::timer::TimerId;
use crate::world::World;

/// The NPC of the entrance manager (`getAnySpawn(32503)`).
const ENTRANCE_MANAGER_ID: i32 = 32503;
/// The watchers: the quiet and the attacking ones.
const WATCHER_ID: i32 = 18601;
const WATCHER_ROTATED_ID: i32 = 18602;
/// Where everybody goes after the match.
const BACK_TO_FANTASY_ISLE: (i32, i32, i32) = (-70381, -70937, -1428);
/// `final long time = 1200000`.
const MATCH_MS: u64 = 1_200_000;
/// `_timeTask`: the "game will end" countdown starts 11 seconds before the end.
const END_WARNING_MS: u64 = 11_000;
/// `ThreadPool.schedule(this::startEvent, 11000)`.
const START_DELAY_MS: u64 = 11_000;
/// `ThreadPool.schedule(this::prepareEvent, 180000)`.
const PREPARE_DELAY_MS: u64 = 180_000;
/// `ThreadPool.schedule(this::recalcEventTime, 180000)`.
const RECALC_DELAY_MS: u64 = 180_000;
/// `Calendar.MINUTE` the registration closes at, the early and the late one.
const MINUTE_FIRST: i64 = 27;
const MINUTE_SECOND: i64 = 57;

/// Java `KrateisCubeManager.EventState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KrateisState {
    #[default]
    Registration,
    Preparing,
    Started,
}

/// Java `KrateiMsgType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KrateiMsgType {
    Initialized,
    Registration5,
    Registration3,
    Registration1,
    Preparing,
    Started,
}

/// What a Kratei's Cube timer does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KrateisTask {
    /// `closeRegistration()`.
    CloseRegistration,
    /// `prepareEvent()`.
    PrepareEvent,
    /// `startEvent()`.
    StartEvent,
    /// `recalcEventTime()`.
    RecalcEventTime,
    /// `getManagerMessage(type)`.
    Message(KrateiMsgType),
    /// `KrateiArena.endEvent()`.
    EndArena(i32),
    /// `KrateiArena.timeLeftInfo()`.
    TimeLeftInfo(i32),
    /// `KrateiArena.changeWatchers()`.
    ChangeWatchers(i32),
    /// `KrateiArena.changeDoorStatus(active)`.
    ChangeDoors { arena: i32, active: bool },
    /// `MessageTask(players, time, type)`: `kind` 0 is "the match will start in", 1 "the game will
    /// end in".
    Countdown { arena: i32, time: i32, kind: u8 },
    /// `RespawnTask(player, time)`.
    Respawn { arena: i32, player: ObjectId, time: i32 },
}

/// `KrateiCubePlayer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct KrateisPlayer {
    pub(crate) inside: bool,
    pub(crate) register: bool,
    pub(crate) points: i32,
}

/// `KrateiArena`.
#[derive(Debug)]
pub(crate) struct KrateisArena {
    pub(crate) def: KrateisArenaDef,
    /// The `KrateisMatchManager` NPC of the arena.
    pub(crate) manager: Option<ObjectId>,
    pub(crate) players: BTreeMap<ObjectId, KrateisPlayer>,
    watchers: Vec<ObjectId>,
    event_task: Option<TimerId>,
    time_task: Option<TimerId>,
    watcher_task: Option<TimerId>,
    door_task: Option<TimerId>,
    doors_rotation: bool,
    watcher_rotation: bool,
    pub(crate) battle_now: bool,
    pub(crate) active_now: bool,
}

impl KrateisArena {
    fn new(def: KrateisArenaDef, manager: Option<ObjectId>) -> Self {
        Self {
            def,
            manager,
            players: BTreeMap::new(),
            watchers: Vec::new(),
            event_task: None,
            time_task: None,
            watcher_task: None,
            door_task: None,
            doors_rotation: false,
            watcher_rotation: false,
            battle_now: false,
            active_now: false,
        }
    }
}

/// `KrateisCubeManager`'s state.
#[derive(Debug, Default)]
pub(crate) struct KrateisWorld {
    pub(crate) arenas: BTreeMap<i32, KrateisArena>,
    /// `_manager`: the entrance manager.
    manager: Option<ObjectId>,
    /// `_nextMatchTime`.
    next_match_ms: i64,
    /// `_registerActive`.
    register_active: bool,
    /// `_isHalfAnHour`.
    is_half_an_hour: bool,
    state: KrateisState,
    period_task: Option<TimerId>,
    event_task: Option<TimerId>,
    msg_task: Option<TimerId>,
}

impl World {
    /// Java `KrateisCubeManager.load()`: the arenas, then the first registration period if the
    /// entrance manager is spawned.
    pub(crate) fn start_krateis_cube(&mut self) {
        let defs: Vec<KrateisArenaDef> = self.minigames.data.krateis_arenas().cloned().collect();
        let mut arenas = BTreeMap::new();
        for def in defs {
            let manager = self.minigame_find_npc(def.manager);
            arenas.insert(def.id, KrateisArena::new(def, manager));
        }
        info!("KrateisCubeManager: Loaded {} arena templates.", arenas.len());
        self.minigames.krateis.arenas = arenas;
        match self.minigame_find_npc(ENTRANCE_MANAGER_ID) {
            Some(manager) => {
                self.minigames.krateis.manager = Some(manager);
                self.krateis_recalc_event_time();
            }
            None => info!("KrateisCubeManager: Event can't be started because npc not found!"),
        }
    }

    /// The state of the cycle (Java `isRegisterTime`, `isActive`, `isPreparing`).
    pub fn krateis_state(&self) -> KrateisState {
        self.minigames.krateis.state
    }

    /// Java `KrateisCubeManager.isRegisterTime()`.
    pub fn krateis_is_register_time(&self) -> bool {
        self.minigames.krateis.register_active
    }

    /// Java `KrateisCubeManager.getNextMatchTime()`.
    pub fn krateis_next_match_time(&self) -> i64 {
        self.minigames.krateis.next_match_ms
    }

    /// The players of an arena and their points (`KrateiArena.getPlayers()`).
    pub fn krateis_arena_players(&self, arena: i32) -> Vec<(ObjectId, i32)> {
        self.minigames
            .krateis
            .arenas
            .get(&arena)
            .map(|a| a.players.iter().map(|(p, v)| (*p, v.points)).collect())
            .unwrap_or_default()
    }

    /// Java `KrateiArena.isBattleNow()`, `isActiveNow()`.
    pub fn krateis_arena_flags(&self, arena: i32) -> (bool, bool) {
        self.minigames
            .krateis
            .arenas
            .get(&arena)
            .map_or((false, false), |a| (a.battle_now, a.active_now))
    }

    fn krateis_cancel(&mut self, task: Option<TimerId>) {
        if let Some(task) = task {
            self.timers.cancel(task);
        }
    }

    // ------------------------------------------------------------------ the cycle

    /// Java `KrateisCubeManager.recalcEventTime()`.
    pub fn krateis_recalc_event_time(&mut self) {
        let krateis = &mut self.minigames.krateis;
        krateis.register_active = true;
        let (period, event) = (krateis.period_task.take(), krateis.event_task.take());
        self.krateis_cancel(period);
        self.krateis_cancel(event);

        // The local wall clock: minutes, seconds and milliseconds of the machine's zone.
        let now = self.minigame_now_ms();
        let offset = match self.minigames.settings.clock_override_unix_ms {
            Some(_) => 0,
            None => l2_util::time::local_offset_millis(now),
        };
        let local = now + offset;
        let hour_ms = 3_600_000;
        let minute = local.rem_euclid(hour_ms) / 60_000;
        let floor_hour = local - local.rem_euclid(hour_ms);
        let target_local = if minute >= MINUTE_SECOND {
            // `calendar.add(HOUR, 1); calendar.set(MINUTE, 27)`: the seconds stay.
            floor_hour + hour_ms + MINUTE_FIRST * 60_000 + local.rem_euclid(60_000)
        } else if minute < MINUTE_FIRST {
            self.minigames.krateis.is_half_an_hour = true;
            floor_hour + MINUTE_FIRST * 60_000 + local.rem_euclid(1000)
        } else {
            self.minigames.krateis.is_half_an_hour = false;
            floor_hour + MINUTE_SECOND * 60_000 + local.rem_euclid(1000)
        };
        let mut next = target_local - offset;
        if next < now {
            next += 86_400_000;
        }
        self.minigames.krateis.next_match_ms = next;
        let last_time = next - now;
        let id = self.minigame_schedule(
            u64::try_from(last_time).unwrap_or(0),
            MinigameTimer::Krateis(KrateisTask::CloseRegistration),
        );
        self.minigames.krateis.event_task = Some(id);
        self.krateis_manager_message(KrateiMsgType::Initialized);

        let minutes = last_time / 1000 / 60;
        let message = if minutes >= 5 {
            Some((KrateiMsgType::Registration5, 300_000))
        } else if (3..5).contains(&minutes) {
            Some((KrateiMsgType::Registration3, 180_000))
        } else if (1..3).contains(&minutes) {
            Some((KrateiMsgType::Registration1, 60_000))
        } else {
            None
        };
        if let Some((kind, before)) = message {
            let id = self.minigame_schedule(
                u64::try_from(last_time - before).unwrap_or(0),
                MinigameTimer::Krateis(KrateisTask::Message(kind)),
            );
            self.minigames.krateis.msg_task = Some(id);
        }
        if self.minigames.krateis.state != KrateisState::Started {
            self.minigames.krateis.state = KrateisState::Registration;
        }
    }

    /// Java `closeRegistration()`.
    fn krateis_close_registration(&mut self) {
        let period = self.minigames.krateis.period_task.take();
        self.krateis_cancel(period);
        self.minigames.krateis.register_active = false;
        self.krateis_manager_message(KrateiMsgType::Preparing);
        self.minigames.krateis.state = KrateisState::Preparing;
        let id = self.minigame_schedule(
            PREPARE_DELAY_MS,
            MinigameTimer::Krateis(KrateisTask::PrepareEvent),
        );
        self.minigames.krateis.period_task = Some(id);
    }

    /// Java `prepareEvent()`.
    pub fn krateis_prepare_event(&mut self) {
        let krateis = &mut self.minigames.krateis;
        krateis.register_active = false;
        let (period, event, msg) = (
            krateis.period_task.take(),
            krateis.event_task.take(),
            krateis.msg_task.take(),
        );
        self.krateis_cancel(period);
        self.krateis_cancel(event);
        self.krateis_cancel(msg);

        let range = f64::from(self.settings.party.alt_party_range);
        let manager_pos = self.minigames.krateis.manager.and_then(|m| self.position_of(m));
        let mut is_active = false;
        let ids: Vec<i32> = self.minigames.krateis.arenas.keys().copied().collect();
        for arena_id in ids {
            let Some(arena) = self.minigames.krateis.arenas.get(&arena_id) else {
                continue;
            };
            if arena.players.len() < arena.def.min_players.max(0) as usize {
                continue;
            }
            let registered: Vec<ObjectId> = arena
                .players
                .iter()
                .filter(|(_, p)| p.register)
                .map(|(id, _)| *id)
                .collect();
            let mut count = 0;
            let mut invalid = Vec::new();
            for player in registered {
                let inside = match (manager_pos, self.position_of(player)) {
                    (Some(m), Some(p)) => crate::target::distance_3d(p, m) <= range,
                    _ => false,
                };
                if inside {
                    if let Some(p) = self
                        .minigames
                        .krateis
                        .arenas
                        .get_mut(&arena_id)
                        .and_then(|a| a.players.get_mut(&player))
                    {
                        p.inside = true;
                        p.register = false;
                    }
                    count += 1;
                } else {
                    invalid.push(player);
                }
            }
            for player in invalid {
                self.krateis_remove_player(arena_id, player);
            }
            if count > 1 {
                if let Some(arena) = self.minigames.krateis.arenas.get_mut(&arena_id) {
                    arena.battle_now = true;
                }
                self.krateis_teleport_to_battle(arena_id);
                self.krateis_wait_time_info(arena_id);
                is_active = true;
            } else if let Some(arena) = self.minigames.krateis.arenas.get_mut(&arena_id) {
                // Match will not start, revert players to their registered state.
                for p in arena.players.values_mut() {
                    if p.inside {
                        p.inside = false;
                        p.register = true;
                    }
                }
            }
        }
        if is_active {
            let id = self.minigame_schedule(START_DELAY_MS, MinigameTimer::Krateis(KrateisTask::StartEvent));
            self.minigames.krateis.event_task = Some(id);
            self.krateis_manager_message(KrateiMsgType::Started);
        } else {
            self.krateis_recalc_event_time();
        }
    }

    /// Java `startEvent()`.
    pub fn krateis_start_event(&mut self) {
        let event = self.minigames.krateis.event_task.take();
        self.krateis_cancel(event);
        let ids: Vec<i32> = self
            .minigames
            .krateis
            .arenas
            .iter()
            .filter(|(_, a)| a.battle_now)
            .map(|(id, _)| *id)
            .collect();
        for arena in ids {
            self.krateis_arena_start_event(arena);
            self.minigames.krateis.state = KrateisState::Started;
        }
        let id = self.minigame_schedule(
            RECALC_DELAY_MS,
            MinigameTimer::Krateis(KrateisTask::RecalcEventTime),
        );
        self.minigames.krateis.period_task = Some(id);
    }

    /// Java `abortEvent()`.
    pub fn krateis_abort_event(&mut self) {
        let ids: Vec<i32> = self
            .minigames
            .krateis
            .arenas
            .iter()
            .filter(|(_, a)| a.battle_now)
            .map(|(id, _)| *id)
            .collect();
        for arena in ids {
            self.krateis_arena_end_event(arena);
        }
        if !self.minigames.krateis.register_active {
            self.krateis_recalc_event_time();
        }
    }

    /// Java `getManagerMessage(state)`: the entrance manager shouts.
    fn krateis_manager_message(&mut self, kind: KrateiMsgType) {
        let Some(manager) = self.minigames.krateis.manager else {
            return;
        };
        let last_time = self.minigames.krateis.next_match_ms - self.minigame_now_ms();
        let (string, parameter) = match kind {
            KrateiMsgType::Initialized => (
                NpcStringId::YouHaveS1MinuteSToRegisterForTheMatch,
                Some(
                    if self.minigames.krateis.is_half_an_hour {
                        27
                    } else {
                        57
                    }
                    .to_string(),
                ),
            ),
            KrateiMsgType::Registration5 => {
                let id = self.minigame_schedule(
                    u64::try_from(last_time - 180_000).unwrap_or(0),
                    MinigameTimer::Krateis(KrateisTask::Message(KrateiMsgType::Registration3)),
                );
                self.minigames.krateis.msg_task = Some(id);
                (
                    NpcStringId::ThereAre5MinutesRemainingToRegisterForKrateiSCubeMatch,
                    None,
                )
            }
            KrateiMsgType::Registration3 => {
                let id = self.minigame_schedule(
                    u64::try_from(last_time - 60_000).unwrap_or(0),
                    MinigameTimer::Krateis(KrateisTask::Message(KrateiMsgType::Registration1)),
                );
                self.minigames.krateis.msg_task = Some(id);
                (
                    NpcStringId::ThereAre3MinutesRemainingToRegisterForKrateiSCubeMatch,
                    None,
                )
            }
            KrateiMsgType::Registration1 => (
                NpcStringId::ThereAre1MinutesRemainingToRegisterForKrateiSCubeMatch,
                None,
            ),
            KrateiMsgType::Preparing => (NpcStringId::TheMatchWillBeginInS1MinuteS, Some("3".to_owned())),
            KrateiMsgType::Started => (NpcStringId::TheMatchWillBeginShortly, None),
        };
        let name = self.creature_name(manager);
        let say = CreatureSay {
            sender_object_id: manager.0,
            chat_type: ChatType::NpcShout,
            sender: SenderField::Name(name),
            message_id: string as i32,
            body: SayBody::Parameters(parameter.into_iter().collect()),
        };
        self.broadcast_packet(manager, &say);
    }

    pub(crate) fn on_krateis_timer(&mut self, task: KrateisTask) {
        match task {
            KrateisTask::CloseRegistration => {
                // The task is spent.
                self.minigames.krateis.event_task = None;
                self.krateis_close_registration();
            }
            KrateisTask::PrepareEvent => {
                self.minigames.krateis.period_task = None;
                self.krateis_prepare_event();
            }
            KrateisTask::StartEvent => {
                self.minigames.krateis.event_task = None;
                self.krateis_start_event();
            }
            KrateisTask::RecalcEventTime => {
                self.minigames.krateis.period_task = None;
                self.krateis_recalc_event_time();
            }
            KrateisTask::Message(kind) => {
                self.minigames.krateis.msg_task = None;
                self.krateis_manager_message(kind);
            }
            KrateisTask::EndArena(arena) => {
                if let Some(a) = self.minigames.krateis.arenas.get_mut(&arena) {
                    a.event_task = None;
                }
                self.krateis_arena_end_event(arena);
            }
            KrateisTask::TimeLeftInfo(arena) => self.krateis_time_left_info(arena),
            KrateisTask::ChangeWatchers(arena) => self.krateis_change_watchers(arena),
            KrateisTask::ChangeDoors { arena, active } => self.krateis_change_door_status(arena, active),
            KrateisTask::Countdown { arena, time, kind } => self.krateis_countdown(arena, time, kind),
            KrateisTask::Respawn { arena, player, time } => self.krateis_respawn_step(arena, player, time),
        }
    }

    // ------------------------------------------------------------------ the registration

    /// Java `KrateiArena.addRegisterPlayer(player)`.
    pub fn krateis_add_register_player(&mut self, arena_id: i32, player: ObjectId) -> bool {
        if !self.players.contains_key(&player) {
            return false;
        }
        let Some(arena) = self.minigames.krateis.arenas.get_mut(&arena_id) else {
            return false;
        };
        if arena.players.contains_key(&player) {
            return false;
        }
        arena.players.insert(
            player,
            KrateisPlayer {
                inside: false,
                register: true,
                points: 0,
            },
        );
        if let Some(p) = self.player_minigame_mut(player) {
            p.krateis_arena = Some(arena_id);
            p.registered_on_event = true;
        }
        true
    }

    /// Java `KrateiArena.removePlayer(player)`.
    pub fn krateis_remove_player(&mut self, arena_id: i32, player: ObjectId) -> bool {
        let Some(arena) = self.minigames.krateis.arenas.get_mut(&arena_id) else {
            return false;
        };
        if arena.players.remove(&player).is_none() {
            return false;
        }
        if let Some(p) = self.player_minigame_mut(player) {
            p.krateis_arena = None;
            p.registered_on_event = false;
        }
        true
    }

    /// The player is gone (a new login is a new `Player` in Java, so the old key could never be
    /// registered or counted again).
    pub(crate) fn krateis_player_left(&mut self, player: ObjectId) {
        for arena in self.minigames.krateis.arenas.values_mut() {
            arena.players.remove(&player);
        }
    }

    // ------------------------------------------------------------------ the arena

    /// `KrateiArena.getParticipants()`: the players sorted by points, the highest first.
    fn krateis_participants(&self, arena_id: i32) -> Vec<CcRecordEntry> {
        let Some(arena) = self.minigames.krateis.arenas.get(&arena_id) else {
            return Vec::new();
        };
        let mut list: Vec<(ObjectId, i32)> = arena.players.iter().map(|(p, v)| (*p, v.points)).collect();
        // A stable sort by points, highest first.
        list.sort_by_key(|entry| std::cmp::Reverse(entry.1));
        list.into_iter()
            .map(|(player, points)| CcRecordEntry {
                name: self.minigame_player_name(player),
                points,
            })
            .collect()
    }

    fn krateis_arena_players_ids(&self, arena_id: i32) -> Vec<ObjectId> {
        self.minigames
            .krateis
            .arenas
            .get(&arena_id)
            .map(|a| a.players.keys().copied().collect())
            .unwrap_or_default()
    }

    /// Java `KrateiArena.startEvent()`.
    fn krateis_arena_start_event(&mut self, arena_id: i32) {
        let Some(arena) = self.minigames.krateis.arenas.get_mut(&arena_id) else {
            return;
        };
        let tasks = [
            arena.event_task.take(),
            arena.time_task.take(),
            arena.watcher_task.take(),
            arena.door_task.take(),
        ];
        for task in tasks {
            self.krateis_cancel(task);
        }
        if let Some(arena) = self.minigames.krateis.arenas.get_mut(&arena_id) {
            arena.doors_rotation = true;
        }
        self.krateis_change_door_status(arena_id, true);

        let watcher_places = self
            .minigames
            .krateis
            .arenas
            .get(&arena_id)
            .map(|a| a.def.watcher_locations.clone())
            .unwrap_or_default();
        if !watcher_places.is_empty() {
            let mut watchers = Vec::new();
            for (x, y, z) in watcher_places {
                if let Some(npc) = self.spawn_npc_at(WATCHER_ID, x, y, z, 0) {
                    self.disable_core_ai(npc, true);
                    self.set_immobilized(npc, true);
                    watchers.push(npc);
                }
            }
            if let Some(arena) = self.minigames.krateis.arenas.get_mut(&arena_id) {
                arena.watchers.extend(watchers);
                arena.watcher_rotation = true;
            }
        }
        self.krateis_teleport_all_players(arena_id);
        let rotation_ms = self.krateis_param_ms(arena_id, "watcherRotationTime");
        let end = self.minigame_schedule(MATCH_MS, MinigameTimer::Krateis(KrateisTask::EndArena(arena_id)));
        let time = self.minigame_schedule(
            MATCH_MS - END_WARNING_MS,
            MinigameTimer::Krateis(KrateisTask::TimeLeftInfo(arena_id)),
        );
        let watcher = self.minigame_schedule(
            rotation_ms,
            MinigameTimer::Krateis(KrateisTask::ChangeWatchers(arena_id)),
        );
        if let Some(arena) = self.minigames.krateis.arenas.get_mut(&arena_id) {
            arena.event_task = Some(end);
            arena.time_task = Some(time);
            arena.watcher_task = Some(watcher);
            arena.active_now = true;
        }
    }

    /// `getParams().getInt(name)`.
    fn krateis_param(&self, arena_id: i32, name: &str) -> i32 {
        self.minigames
            .krateis
            .arenas
            .get(&arena_id)
            .map_or(0, |a| a.def.param(name))
    }

    /// `getParams().getInt(name) * 1000` as a delay.
    fn krateis_param_ms(&self, arena_id: i32, name: &str) -> u64 {
        u64::try_from(self.krateis_param(arena_id, name)).unwrap_or(0) * 1000
    }

    /// Java `KrateiArena.endEvent()`.
    pub fn krateis_arena_end_event(&mut self, arena_id: i32) {
        let Some(arena) = self.minigames.krateis.arenas.get_mut(&arena_id) else {
            return;
        };
        let tasks = [
            arena.event_task.take(),
            arena.watcher_task.take(),
            arena.door_task.take(),
        ];
        arena.watcher_rotation = false;
        for task in tasks {
            self.krateis_cancel(task);
        }
        self.krateis_change_door_status(arena_id, false);

        let watchers = self
            .minigames
            .krateis
            .arenas
            .get_mut(&arena_id)
            .map(|a| std::mem::take(&mut a.watchers))
            .unwrap_or_default();
        for npc in watchers {
            self.delete_npc(npc);
        }

        let participants = self.krateis_arena_players_ids(arena_id);
        if !participants.is_empty() {
            for player in &participants {
                self.krateis_remove_all_effects(*player);
                self.minigame_tele(*player, BACK_TO_FANTASY_ISLE);
                if let Some(p) = self.player_minigame_mut(*player) {
                    p.krateis_arena = None;
                    p.registered_on_event = false;
                    p.can_revive = true;
                }
                self.send(*player, &ExPvpMatchCcRetire);
            }
            self.krateis_check_reward_players(arena_id);
        }
        if let Some(arena) = self.minigames.krateis.arenas.get_mut(&arena_id) {
            arena.players.clear();
            arena.battle_now = false;
            arena.active_now = false;
        }
        // `KrateisCubeManager.setIsActivate(false)`.
        self.minigames.krateis.state = KrateisState::Registration;
    }

    /// Java `changeWatchers()`.
    fn krateis_change_watchers(&mut self, arena_id: i32) {
        let Some(arena) = self.minigames.krateis.arenas.get_mut(&arena_id) else {
            return;
        };
        // A task cancelled by `endEvent` in the same tick still comes out of the wheel: it must
        // not bring the watchers back.
        if !arena.active_now {
            return;
        }
        arena.watcher_task = None;
        let old = std::mem::take(&mut arena.watchers);
        let places = arena.def.watcher_locations.clone();
        let rotated = arena.watcher_rotation;
        for npc in old {
            self.delete_npc(npc);
        }
        let mut watchers = Vec::new();
        for (x, y, z) in places {
            if let Some(npc) =
                self.spawn_npc_at(if rotated { WATCHER_ROTATED_ID } else { WATCHER_ID }, x, y, z, 0)
            {
                self.disable_core_ai(npc, true);
                self.set_immobilized(npc, true);
                watchers.push(npc);
            }
        }
        let delay = self.krateis_param_ms(arena_id, "watcherRotationTime");
        let task = self.minigame_schedule(
            delay,
            MinigameTimer::Krateis(KrateisTask::ChangeWatchers(arena_id)),
        );
        if let Some(arena) = self.minigames.krateis.arenas.get_mut(&arena_id) {
            arena.watchers = watchers;
            arena.watcher_rotation = !rotated;
            arena.watcher_task = Some(task);
        }
    }

    /// Java `changeDoorStatus(active)`.
    // NOTE(java-quirk): the task that closes the doors at the end schedules itself again every
    // `doorRotationTime` seconds, and nothing cancels it until the next match starts.
    fn krateis_change_door_status(&mut self, arena_id: i32, active: bool) {
        let Some(arena) = self.minigames.krateis.arenas.get_mut(&arena_id) else {
            return;
        };
        let spent = arena.door_task.take();
        self.krateis_cancel(spent);
        let Some(arena) = self.minigames.krateis.arenas.get_mut(&arena_id) else {
            return;
        };
        arena.doors_rotation = !arena.doors_rotation;
        if !arena.def.doors_a.is_empty() && !arena.def.doors_b.is_empty() {
            let (a, b) = if arena.doors_rotation {
                (arena.def.doors_a.clone(), arena.def.doors_b.clone())
            } else {
                (arena.def.doors_b.clone(), arena.def.doors_a.clone())
            };
            if active {
                self.krateis_set_doors(&a, true);
                self.krateis_set_doors(&b, false);
            } else {
                self.krateis_set_doors(&a, false);
                self.krateis_set_doors(&b, false);
            }
        }
        let delay = self.krateis_param_ms(arena_id, "doorRotationTime");
        let task = self.minigame_schedule(
            delay,
            MinigameTimer::Krateis(KrateisTask::ChangeDoors {
                arena: arena_id,
                active,
            }),
        );
        if let Some(arena) = self.minigames.krateis.arenas.get_mut(&arena_id) {
            arena.door_task = Some(task);
        }
    }

    /// `changeDoorsStatus(doors, isOpen)`; a door id the world does not know is skipped.
    fn krateis_set_doors(&mut self, doors: &[i32], open: bool) {
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

    /// `addEffects(player)`: full vitals and the buffs of the arena.
    pub(crate) fn krateis_add_effects(&mut self, arena_id: i32, player: ObjectId) {
        self.krateis_fill_vitals(player);
        let summon = self.summon_of(player);
        if let Some(summon) = summon {
            self.krateis_fill_vitals(summon);
        }
        let buffs = self
            .minigames
            .krateis
            .arenas
            .get(&arena_id)
            .map(|a| a.def.buffs.clone())
            .unwrap_or_default();
        for (id, level) in buffs {
            let key = SkillKey::new(id, level);
            self.apply_effects(player, player, key);
            if let Some(summon) = summon {
                self.apply_effects(summon, summon, key);
            }
        }
        self.broadcast_user_info(player);
        if let Some(summon) = summon {
            self.broadcast_info(summon);
        }
    }

    fn krateis_fill_vitals(&mut self, id: ObjectId) {
        let Some((hp, mp, cp)) = self.stats(id).map(|s| (s.max_hp(), s.max_mp(), s.max_cp())) else {
            return;
        };
        self.set_current_hp(id, f64::from(hp), true);
        self.set_current_cp(id, f64::from(cp), true);
        self.set_current_mp(id, f64::from(mp), true);
    }

    /// `removeAllEffects(player)`.
    fn krateis_remove_all_effects(&mut self, player: ObjectId) {
        self.stop_all_effects(player);
        let summon = self.summon_of(player);
        if let Some(summon) = summon {
            self.stop_all_effects(summon);
        }
        self.broadcast_user_info(player);
        if let Some(summon) = summon {
            self.broadcast_info(summon);
        }
    }

    /// `respawnEffects(player)`.
    fn krateis_respawn_effects(&mut self, player: ObjectId) {
        self.krateis_remove_all_effects(player);
        if self.is_dead(player) {
            self.do_revive_with_power(player, 100.0);
        }
        self.broadcast_status_update(player);
        self.broadcast_user_info(player);
    }

    pub(crate) fn krateis_random_place(&mut self, arena_id: i32, battle: bool) -> Option<(i32, i32, i32)> {
        let places = self.minigames.krateis.arenas.get(&arena_id).map(|a| {
            if battle {
                a.def.battle_locations.clone()
            } else {
                a.def.wait_locations.clone()
            }
        })?;
        if places.is_empty() {
            return None;
        }
        let index = self.rng().below(places.len() as i32) as usize;
        places.get(index).copied()
    }

    /// `teleportAllPlayers()`.
    fn krateis_teleport_all_players(&mut self, arena_id: i32) {
        let players = self.krateis_arena_players_ids(arena_id);
        if players.is_empty() {
            return;
        }
        let record = ExPvpMatchCcRecord {
            state: CC_RECORD_UPDATE,
            players: self.krateis_participants(arena_id),
        };
        for player in players {
            if let Some(place) = self.krateis_random_place(arena_id, true) {
                self.minigame_tele(player, place);
            }
            self.krateis_add_effects(arena_id, player);
            let points = self.krateis_points_of(arena_id, player);
            self.send(player, &ExPvpMatchCcMyRecord { points });
            self.send(player, &record);
            if let Some(p) = self.player_minigame_mut(player) {
                p.can_revive = false;
            }
        }
    }

    /// `teleportToBattle(null)`: everybody to the waiting room.
    fn krateis_teleport_to_battle(&mut self, arena_id: i32) {
        for player in self.krateis_arena_players_ids(arena_id) {
            self.krateis_teleport_to_waiting_room(arena_id, player, false);
        }
    }

    /// `teleportToWaitingRoom(player, restoreEffects)`.
    pub(crate) fn krateis_teleport_to_waiting_room(
        &mut self,
        arena_id: i32,
        player: ObjectId,
        restore: bool,
    ) {
        let registered = self
            .minigames
            .krateis
            .arenas
            .get(&arena_id)
            .is_some_and(|a| a.players.contains_key(&player));
        if !registered {
            return;
        }
        if restore {
            self.krateis_respawn_effects(player);
        }
        if let Some(place) = self.krateis_random_place(arena_id, false) {
            self.minigame_tele(player, place);
        }
    }

    /// `KrateiArena.getPoints(player)`.
    pub fn krateis_points_of(&self, arena_id: i32, player: ObjectId) -> i32 {
        self.minigames
            .krateis
            .arenas
            .get(&arena_id)
            .and_then(|a| a.players.get(&player))
            .map_or(0, |p| p.points)
    }

    /// Java `KrateiArena.addPoints(player, isPlayer)`: 3 points for a player and 1 for a monster
    /// (the `pointPerPlayer`, `pointPerMob` parameters). The `KrateisCube` script calls it with
    /// `false` for a monster kill.
    pub fn krateis_add_points(&mut self, player: ObjectId, is_player: bool) {
        let Some(arena_id) = self.krateis_arena_of(player) else {
            return;
        };
        let Some(arena) = self.minigames.krateis.arenas.get_mut(&arena_id) else {
            return;
        };
        let add = arena.def.param(if is_player {
            "pointPerPlayer"
        } else {
            "pointPerMob"
        });
        let Some(entry) = arena.players.get_mut(&player) else {
            return;
        };
        entry.points += add;
        let points = entry.points;
        self.send(player, &ExPvpMatchCcMyRecord { points });
        let participants = self.krateis_participants(arena_id);
        if !participants.is_empty() {
            let record = ExPvpMatchCcRecord {
                state: CC_RECORD_UPDATE,
                players: participants,
            };
            for p in self.krateis_arena_players_ids(arena_id) {
                self.send(p, &record);
            }
        }
    }

    /// `timeLeftInfo()`.
    fn krateis_time_left_info(&mut self, arena_id: i32) {
        if let Some(arena) = self.minigames.krateis.arenas.get_mut(&arena_id) {
            arena.time_task = None;
            if arena.players.is_empty() {
                return;
            }
        } else {
            return;
        }
        self.minigame_schedule(
            1,
            MinigameTimer::Krateis(KrateisTask::Countdown {
                arena: arena_id,
                time: 10,
                kind: 1,
            }),
        );
    }

    /// `waitTimeInfo()`.
    fn krateis_wait_time_info(&mut self, arena_id: i32) {
        if self
            .minigames
            .krateis
            .arenas
            .get(&arena_id)
            .is_some_and(|a| !a.players.is_empty())
        {
            self.minigame_schedule(
                1,
                MinigameTimer::Krateis(KrateisTask::Countdown {
                    arena: arena_id,
                    time: 10,
                    kind: 0,
                }),
            );
        }
    }

    /// `MessageTask.run()`: one message a second.
    fn krateis_countdown(&mut self, arena_id: i32, time: i32, kind: u8) {
        let players = self.krateis_arena_players_ids(arena_id);
        if players.is_empty() || time <= 0 {
            return;
        }
        let id = match kind {
            0 => Some(SystemMessageId::TheMatchWillStartInS1SecondS),
            1 => Some(SystemMessageId::TheGameWillEndInS1SecondS2),
            _ => None,
        };
        if let Some(id) = id {
            let message = SystemMessage::new(id).with(SystemMessageParam::IntNumber(time));
            for player in players {
                self.send(player, &message);
            }
        }
        if time - 1 > 0 {
            self.minigame_schedule(
                1000,
                MinigameTimer::Krateis(KrateisTask::Countdown {
                    arena: arena_id,
                    time: time - 1,
                    kind,
                }),
            );
        }
    }

    /// `Player.doDie`'s Kratei's Cube blocks: the respawn countdown for the dead player and the
    /// points for a killer who is in an arena.
    pub(crate) fn krateis_on_player_die(&mut self, id: ObjectId, killer: Option<ObjectId>) {
        let Some(arena_id) = self.krateis_arena_of(id) else {
            return;
        };
        // `arena.setRespawnTask(this)`.
        self.minigame_schedule(
            1,
            MinigameTimer::Krateis(KrateisTask::Respawn {
                arena: arena_id,
                player: id,
                time: self.krateis_param(arena_id, "respawnTime"),
            }),
        );
        // `killer.isPlayable()`: the owner of a summon gets the points.
        if let Some(killer) = killer.and_then(|k| self.acting_player(k)) {
            self.krateis_add_points(killer, true);
        }
    }

    /// `RespawnTask.run()`.
    fn krateis_respawn_step(&mut self, arena_id: i32, player: ObjectId, time: i32) {
        if !self.players.contains_key(&player) || time <= 0 {
            return;
        }
        self.send(
            player,
            &SystemMessage::new(SystemMessageId::ResurrectionWillTakePlaceInTheWaitingRoomAfterS1Seconds)
                .with(SystemMessageParam::IntNumber(time)),
        );
        let time = time - 1;
        if time > 0 {
            self.minigame_schedule(
                1000,
                MinigameTimer::Krateis(KrateisTask::Respawn {
                    arena: arena_id,
                    player,
                    time,
                }),
            );
        } else {
            self.krateis_respawn_effects(player);
            if let Some(place) = self.krateis_random_place(arena_id, false) {
                self.minigame_tele(player, place);
            }
        }
    }

    /// `checkRewardPlayers(players)`.
    fn krateis_check_reward_players(&mut self, arena_id: i32) {
        let Some(arena) = self.minigames.krateis.arenas.get(&arena_id) else {
            return;
        };
        let def = arena.def.clone();
        let size = arena.players.len() as i32;
        let mut list: Vec<(ObjectId, i32)> = arena.players.iter().map(|(p, v)| (*p, v.points)).collect();
        // `SortPointInfo`: the highest points first.
        list.sort_by_key(|entry| std::cmp::Reverse(entry.1));
        let record = ExPvpMatchCcRecord {
            state: CC_RECORD_INITIALIZE,
            players: self.krateis_participants(arena_id),
        };
        let mut dif = 0.05_f64;
        let mut pos = 0;
        for (player, points) in list {
            if !self.players.contains_key(&player) {
                continue;
            }
            pos += 1;
            self.send(player, &record);
            if points < def.param("minPoints") {
                continue;
            }
            let count = (f64::from(points) * dif * (1.0 + f64::from(size / pos) * 0.04)) as i32;
            dif -= 0.0016;
            if count <= 0 {
                continue;
            }
            let exp = i64::from(count) * i64::from(def.param("expModifier"));
            let sp = i64::from(count) * i64::from(def.param("spModifier"));
            self.add_exp_and_sp(player, exp as f64, sp as f64, false);
            for reward in &def.rewards {
                let amount = if reward.use_modifier {
                    i64::from(count)
                } else {
                    reward.amount
                };
                self.minigame_give_reward(player, reward.id, amount);
            }
        }
    }

    /// The reward switch both games share: -100 PC cafe points, -200 clan reputation, -300 fame,
    /// above 0 an item.
    pub(crate) fn minigame_give_reward(&mut self, player: ObjectId, id: i32, amount: i64) {
        match id {
            -100 => {
                let mut amount = amount;
                let current = i64::from(self.pc_cafe_points(player));
                let max = i64::from(self.pc_cafe_max_points());
                if current + amount > max {
                    amount = max - current;
                }
                self.send(
                    player,
                    &SystemMessage::new(SystemMessageId::YouHaveAcquiredS1PcCafePoints)
                        .with(SystemMessageParam::IntNumber(amount as i32)),
                );
                self.set_pc_cafe_points(player, (current + amount) as i32);
                let points = self.pc_cafe_points(player);
                self.send_pc_cafe_point_info(player, points, amount as i32, 1);
            }
            -200 => {
                let clan = self.clan_id_of(player);
                if clan != 0 {
                    self.add_reputation_score(clan, amount as i32);
                }
            }
            -300 => {
                let fame = self.players.get(&player).map_or(0, |p| p.data.profile.fame);
                self.set_fame(player, fame.wrapping_add(amount as i32));
                // NOTE(java-quirk): the fame reward says "You have acquired $s1 reputation".
                self.send(
                    player,
                    &SystemMessage::new(SystemMessageId::YouHaveAcquiredS1Reputation)
                        .with(SystemMessageParam::IntNumber(amount as i32)),
                );
                self.send_user_info(player);
            }
            item if item > 0 => {
                self.add_item(player, item, amount, true);
            }
            _ => {}
        }
    }
}
