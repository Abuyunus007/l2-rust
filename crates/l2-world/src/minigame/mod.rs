//! The minigames: Handy's Block Checker, Kratei's Cube and the Underground Coliseum
//! (M8-GAMES-01).
// Java: org/l2jmobius/gameserver/managers/HandysBlockCheckerManager.java
// Java: org/l2jmobius/gameserver/managers/games/{BlockCheckerManager,KrateisCubeManager,
//       UndergroundColiseumManager}.java
// Java: org/l2jmobius/gameserver/mechanics/{krateisCube,undergroundColiseum}/*.java
// Java: org/l2jmobius/gameserver/entity/actor/holders/player/ArenaParticipantsHolder.java
// Java: org/l2jmobius/gameserver/entity/actor/instance/{Block,KrateisCubeManager,
//       KrateisMatchManager,UCManager,UCHelper,UCTower}.java
// Java: network/clientpackets/{RequestExCubeGameChangeTeam,RequestExCubeGameReadyAnswer}.java
// Java: entity/actor/Player.java (the event fields: `_isRegisteredOnEvent`, `_krateiArena`,
//       `_ucState`, `_handysBlockCheckerEventArena`, `_canRevive`)
//
// # Ownership (D-10, D-11)
// [`MinigameWorld`] is the world's. The players of an arena, the teams of the coliseum, the
// waiting parties and the NPCs a match made (blocks, towers, watchers) are ids; the objects stay
// in the world's tables. What Java keeps on the `Player` for these games is one
// [`PlayerMinigame`] per player here (D-17: no change to `Player`), dropped when the player
// leaves the world.
//
// # Tasks (D-12)
// Java's `ThreadPool.schedule` and `ThreadPool.execute` are [`MinigameTimer`]s on the timer
// wheel; `execute` is the next tick. The coliseum's `Thread.sleep` countdown is one timer step
// per message. The countdown messages of Kratei's Cube are one timer per second like Java's.
//
// # Persistence (D-15, D-39, D-49..D-53)
// The only rows these games write are the coliseum's: `underground_coliseum` (the best team of
// an arena) and the `UC_START_TIME` / `UC_STOP_TIME` rows of `global_variables`. Both are global
// rows the Java game server writes too, so they go out only with `CycleDatabaseWrites`
// ([`MinigameSettings::cycle_database_writes`], D-53); without it they live in memory and are read
// at start ([`MinigameLoad`]).
//
// # Files
// - `block_checker.rs`: `HandysBlockCheckerManager`, `ArenaParticipantsHolder` and the event
//   engine `BlockCheckerManager`, the two client packets, `Block.changeColor` / `FlipBlock`.
// - `krateis.rs`: `KrateisCubeManager`, `KrateiArena` and the player points.
// - `coliseum.rs`: `UndergroundColiseumManager`, `UCArena`, `UCTeam`, `UCPoint`, `UCRunningTask`.
// - `bypass.rs`: the NPCs: `KrateisCubeManager`, `KrateisMatchManager`, `UCManager`, `UCHelper`.
// - `cron.rs`: `SchedulingPattern`, for `BattleStartTime`.
//
// # Left to the script milestone (D-05)
// `HandysBlockCheckerEvent` (the arena managers' first talk: [`World::block_checker_first_talk`]
// is its body), `KrateisCube` (the monster kill: [`World::krateis_add_points`]),
// `TeleportToUndergroundColiseum` and the `EventItem` / `FlipBlock` handlers (the effect and the
// item call [`World::block_checker_flip_block`] and the arena holders).

mod block_checker;
mod bypass;
mod coliseum;
mod cron;
mod krateis;

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use l2_data::MinigameData;
use l2_protocol_game::server::SystemMessage;
use l2_util::ObjectId;

use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::{World, unix_ms_now};

pub use block_checker::{BlockArenaView, BlockTask};
pub use coliseum::{ColiseumTask, UcPartyState, UcTeamView};
pub use krateis::{KrateisState, KrateisTask};

/// Java `Player.UC_STATE_NONE`, `UC_STATE_POINT`, `UC_STATE_ARENA`.
pub const UC_STATE_NONE: u8 = 0;
pub const UC_STATE_POINT: u8 = 1;
pub const UC_STATE_ARENA: u8 = 2;

/// Java `Team.NONE`, `Team.BLUE`, `Team.RED` (the ids `World::set_team` takes).
pub(crate) const TEAM_NONE: u8 = 0;
pub(crate) const TEAM_BLUE: u8 = 1;
pub(crate) const TEAM_RED: u8 = 2;

/// The values the minigames read: `General.ini` (Block Checker), `UndergroundColiseum.ini` and the
/// D-53 switch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinigameSettings {
    /// `EnableBlockCheckerEvent` (`GeneralConfig.ENABLE_BLOCK_CHECKER_EVENT`).
    pub enable_block_checker_event: bool,
    /// `HBCEFairPlay` (`GeneralConfig.HBCE_FAIR_PLAY`).
    pub hbce_fair_play: bool,
    /// `MinBlockCheckerTeamMembers`, already clamped to 1..=6 like Java.
    pub min_block_checker_team_members: i32,
    /// `AllowPartyInSameEvent` (`GeneralConfig.ALLOW_PARTY_IN_SAME_EVENT`).
    pub allow_party_in_same_event: bool,
    /// `UndergroundColiseumConfig.UC_START_TIME` (`BattleStartTime`).
    pub uc_start_time: String,
    /// `UC_TIME_PERIOD` (`BattlePeriod`, hours).
    pub uc_time_period: i32,
    /// `UC_ALLOW_ANNOUNCE` (`AllowAnnouncements`).
    pub uc_allow_announce: bool,
    /// `UC_PARTY_SIZE` (`PartySize`).
    pub uc_party_size: i32,
    /// `UC_RESS_TIME` (`ResurrectionTime`, seconds).
    pub uc_ress_time: i32,
    /// `CycleDatabaseWrites` of `config/SideBySide.ini` (D-53), `game_server::cycle_database_writes`.
    pub cycle_database_writes: bool,
    /// Tests only: the wall clock at world tick 0 (the clock then runs with the ticks).
    pub clock_override_unix_ms: Option<i64>,
}

impl Default for MinigameSettings {
    /// The Java default of each key.
    fn default() -> Self {
        Self {
            enable_block_checker_event: false,
            hbce_fair_play: false,
            min_block_checker_team_members: 2,
            allow_party_in_same_event: true,
            uc_start_time: "0 17 * * *".to_owned(),
            uc_time_period: 2,
            uc_allow_announce: false,
            uc_party_size: 7,
            uc_ress_time: 10,
            cycle_database_writes: false,
            clock_override_unix_ms: None,
        }
    }
}

/// What the game server read from the database before the world starts: Java
/// `UndergroundColiseumManager.load` (`UC_START_TIME`, `UC_STOP_TIME`) and `restoreBestTeams`
/// (`SELECT * FROM underground_coliseum ORDER BY arenaId`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MinigameLoad {
    /// `getLong("UC_START_TIME", 0)`.
    pub uc_start_ms: i64,
    /// `getLong("UC_STOP_TIME", 0)`.
    pub uc_stop_ms: i64,
    /// (`arenaId`, `leader`, `wins`).
    pub best_teams: Vec<(i32, String, i32)>,
}

/// A row change of the minigames, with Java's statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MinigameWrite {
    /// `INSERT INTO underground_coliseum (arenaId, leader, wins) VALUES (?,?,?)`.
    BestTeamInsert {
        arena_id: i32,
        leader: String,
        wins: i32,
    },
    /// `UPDATE underground_coliseum SET leader = ?, wins = ?  WHERE arenaId = ?`.
    // NOTE(java-quirk): `saveBestTeam` binds `team.getArenaId()` as the *leader* (`setInt(1, ...)`),
    // so the leader column of an updated row holds the arena id, not a name. Ported as is.
    BestTeamUpdate { arena_id: i32, wins: i32 },
    /// `REPLACE INTO global_variables (var, value)` for `UC_START_TIME` / `UC_STOP_TIME`.
    GlobalVariable { name: String, value: i64 },
}

/// The client requests of the Cube game (`ClientInput::Minigame`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MinigameInput {
    /// `RequestExCubeGameChangeTeam` (`arena` is Java's `_arena`, the wire value plus 1).
    CubeChangeTeam { arena: i32, team: i32 },
    /// `RequestExCubeGameReadyAnswer`.
    CubeReadyAnswer { arena: i32, answer: i32 },
}

/// What Java keeps on a `Player` for the minigames.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlayerMinigame {
    /// `_handysBlockCheckerEventArena`: -1 for none.
    pub(crate) block_arena: i32,
    /// `_isRegisteredOnEvent`.
    pub(crate) registered_on_event: bool,
    /// `_krateiArena`.
    pub(crate) krateis_arena: Option<i32>,
    /// `_ucState`.
    pub(crate) uc_state: u8,
    /// `_ucKills`, `_ucDeaths`.
    pub(crate) uc_kills: i32,
    pub(crate) uc_deaths: i32,
    /// `_canRevive`.
    pub(crate) can_revive: bool,
}

impl Default for PlayerMinigame {
    fn default() -> Self {
        Self {
            block_arena: -1,
            registered_on_event: false,
            krateis_arena: None,
            uc_state: UC_STATE_NONE,
            uc_kills: 0,
            uc_deaths: 0,
            can_revive: true,
        }
    }
}

/// The payload of `TimerKind::Minigame`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MinigameTimer {
    Block(BlockTask),
    Krateis(KrateisTask),
    Coliseum(ColiseumTask),
}

/// The minigames of the world.
#[derive(Debug, Default)]
pub struct MinigameWorld {
    pub(crate) settings: MinigameSettings,
    pub(crate) data: Arc<MinigameData>,
    pub(crate) players: HashMap<ObjectId, PlayerMinigame>,
    pub(crate) block: block_checker::BlockWorld,
    pub(crate) krateis: krateis::KrateisWorld,
    pub(crate) coliseum: coliseum::ColiseumWorld,
    /// The team colour of the NPCs a match made (the coliseum's towers), for `NpcInfo`.
    pub(crate) npc_team: BTreeMap<ObjectId, u8>,
    /// The colour effect of the blocks (`Block._colorEffect`), for `NpcInfo`.
    pub(crate) block_color: BTreeMap<ObjectId, i32>,
}

impl World {
    /// Installs the `General.ini` and `UndergroundColiseum.ini` values and the D-53 switch.
    pub fn set_minigame_settings(&mut self, settings: MinigameSettings) {
        self.minigames.settings = settings;
    }

    /// Installs the arenas of `UndergroundColiseum.xml` and `KrateisCube.xml`.
    pub fn set_minigame_data(&mut self, data: Arc<MinigameData>) {
        self.minigames.data = data;
    }

    /// Java `GameServer`'s `KrateisCubeManager.getInstance()` and
    /// `UndergroundColiseumManager.getInstance()` (and the `startUpParticipantsQueue()` of the Block
    /// Checker script with `EnableBlockCheckerEvent`): the arenas are made, the registration cycles
    /// start. Call once, after the spawns and the doors exist.
    pub fn start_minigames(&mut self, load: MinigameLoad) {
        if self.minigames.settings.enable_block_checker_event {
            self.start_block_checker_queue();
        }
        self.start_krateis_cube();
        self.start_underground_coliseum(load);
    }

    /// The wall clock (Java `System.currentTimeMillis()`) the minigame schedules read.
    pub(crate) fn minigame_now_ms(&self) -> i64 {
        match self.minigames.settings.clock_override_unix_ms {
            Some(base) => base + self.world_ms(),
            None => unix_ms_now(),
        }
    }

    /// Arms a minigame timer `ms` from now.
    pub(crate) fn minigame_schedule(&mut self, ms: u64, timer: MinigameTimer) -> TimerId {
        let due = self.tick + TimerWheel::ms_to_ticks(ms);
        self.timers.schedule_at(due, TimerKind::Minigame(timer))
    }

    /// The timers of the minigames fire here.
    pub(crate) fn on_minigame_timer(&mut self, timer: MinigameTimer) {
        match timer {
            MinigameTimer::Block(task) => self.on_block_timer(task),
            MinigameTimer::Krateis(task) => self.on_krateis_timer(task),
            MinigameTimer::Coliseum(task) => self.on_coliseum_timer(task),
        }
    }

    /// The client requests of the Cube game.
    pub(crate) fn on_minigame_input(&mut self, player: ObjectId, input: MinigameInput) {
        match input {
            MinigameInput::CubeChangeTeam { arena, team } => {
                self.on_cube_game_change_team(player, arena, team)
            }
            MinigameInput::CubeReadyAnswer { arena, answer } => {
                self.on_cube_game_ready_answer(player, arena, answer)
            }
        }
    }

    // ------------------------------------------------------------------ the player's fields

    pub(crate) fn player_minigame_mut(&mut self, id: ObjectId) -> Option<&mut PlayerMinigame> {
        if !self.players.contains_key(&id) {
            return None;
        }
        Some(self.minigames.players.entry(id).or_default())
    }

    /// Java `Player.isRegisteredOnEvent()`: `_isRegisteredOnEvent || _isOnEvent` (the custom events
    /// that set `_isOnEvent` are not ported).
    pub fn is_registered_on_event(&self, id: ObjectId) -> bool {
        self.minigames
            .players
            .get(&id)
            .is_some_and(|p| p.registered_on_event)
    }

    /// Java `Player.setRegisteredOnEvent(value)`.
    pub fn set_registered_on_event(&mut self, id: ObjectId, value: bool) {
        if let Some(p) = self.player_minigame_mut(id) {
            p.registered_on_event = value;
        }
    }

    /// Java `Player.getBlockCheckerArena()`: -1 outside a Block Checker match.
    pub fn block_checker_arena_of(&self, id: ObjectId) -> i32 {
        self.minigames.players.get(&id).map_or(-1, |p| p.block_arena)
    }

    pub(crate) fn set_block_checker_arena(&mut self, id: ObjectId, arena: i32) {
        if let Some(p) = self.player_minigame_mut(id) {
            p.block_arena = arena;
        }
    }

    /// Java `Player.getKrateiArena()`: the id of the Kratei's Cube arena the player is registered
    /// in.
    pub fn krateis_arena_of(&self, id: ObjectId) -> Option<i32> {
        self.minigames.players.get(&id).and_then(|p| p.krateis_arena)
    }

    /// Java `Player.getUCState()`.
    pub fn uc_state_of(&self, id: ObjectId) -> u8 {
        self.minigames
            .players
            .get(&id)
            .map_or(UC_STATE_NONE, |p| p.uc_state)
    }

    /// Java `Player.setUCState(state)`: also `setRegisteredOnEvent(state != UC_STATE_NONE)`.
    pub(crate) fn set_uc_state(&mut self, id: ObjectId, state: u8) {
        if let Some(p) = self.player_minigame_mut(id) {
            p.registered_on_event = state != UC_STATE_NONE;
            p.uc_state = state;
        }
    }

    /// Java `Player.getUCKills()`, `getUCDeaths()`.
    pub fn uc_kills_deaths_of(&self, id: ObjectId) -> (i32, i32) {
        self.minigames
            .players
            .get(&id)
            .map_or((0, 0), |p| (p.uc_kills, p.uc_deaths))
    }

    /// Java `Player._canRevive` (`Creature.canRevive()`): Kratei's Cube turns it off in the match.
    pub(crate) fn minigame_can_revive(&self, id: ObjectId) -> bool {
        self.minigames.players.get(&id).is_none_or(|p| p.can_revive)
    }

    // ------------------------------------------------------------------ hooks other systems call

    /// `Player.canLogout()`: "A superior power doesn't allow you to leave." for a registered
    /// player. `true` when the player may not leave (the message is sent).
    pub(crate) fn minigame_blocks_logout(&self, id: ObjectId) -> bool {
        if !self.is_registered_on_event(id) {
            return false;
        }
        self.send_message(id, "A superior power doesn't allow you to leave.");
        true
    }

    /// The player is gone: `Player.deleteMe`'s Block Checker block, and the player's registrations
    /// (a new login is a new `Player` object in Java, so the old one's arena entries cannot be
    /// reached again).
    pub(crate) fn minigame_on_player_left(&mut self, id: ObjectId) {
        // `GeneralConfig.ENABLE_BLOCK_CHECKER_EVENT && _handysBlockCheckerEventArena != -1`.
        if self.minigames.settings.enable_block_checker_event && self.block_checker_arena_of(id) != -1 {
            self.block_checker_on_disconnect(id);
        }
        self.krateis_player_left(id);
        self.minigames.players.remove(&id);
    }

    /// `NpcInfo.getTeam()` of an NPC a match made.
    pub(crate) fn minigame_npc_team(&self, npc: ObjectId) -> u8 {
        self.minigames.npc_team.get(&npc).copied().unwrap_or(TEAM_NONE)
    }

    /// Java `Block.getColorEffect()`: 0x53 for a red block, 0 for a blue one (0 for any other NPC).
    pub fn block_color_of(&self, block: ObjectId) -> i32 {
        self.minigame_npc_color_effect(block)
    }

    /// `NpcInfo.getColorEffect()` of a block.
    pub(crate) fn minigame_npc_color_effect(&self, npc: ObjectId) -> i32 {
        self.minigames.block_color.get(&npc).copied().unwrap_or(0)
    }

    /// `isAutoAttackable` of a block (`Block.isAutoAttackable`) and a coliseum tower
    /// (`UCTower.isAutoAttackable`); `None` for any other NPC.
    pub(crate) fn minigame_npc_auto_attackable(&self, npc: ObjectId, attacker: ObjectId) -> Option<bool> {
        if self.minigames.block_color.contains_key(&npc) {
            // `attacker.isPlayer() ? arena > -1 : true`.
            return Some(!self.is_player(attacker) || self.block_checker_arena_of(attacker) > -1);
        }
        if self.minigames.npc_team.contains_key(&npc) {
            return Some(true);
        }
        None
    }

    /// `Player.doDie(killer)`'s event blocks: the Kratei's Cube respawn countdown and the killer's
    /// points, then the coliseum team's `onKill`.
    pub(crate) fn minigame_on_player_die(&mut self, id: ObjectId, killer: Option<ObjectId>) {
        self.krateis_on_player_die(id, killer);
        self.coliseum_on_player_die(id, killer);
    }

    /// `Npc.doDie` of a coliseum tower: `UCTower.doDie` takes the tower out of its team.
    pub(crate) fn minigame_on_npc_die(&mut self, npc: ObjectId) {
        self.coliseum_tower_died(npc);
    }

    /// Whether `player` and `target` may be in the same party invitation: the `isRegisteredOnEvent`
    /// block of `RequestJoinParty`. `Err` carries the message Java sends to the requestor.
    pub(crate) fn minigame_party_invite_check(
        &self,
        requestor: ObjectId,
        target: ObjectId,
    ) -> Result<(), &'static str> {
        if !(self.is_registered_on_event(requestor) || self.is_registered_on_event(target)) {
            return Ok(());
        }
        if !self.minigames.settings.allow_party_in_same_event {
            return Err("Event paticipants cannot be invited to parties.");
        }
        if !(self.instance_id_of(requestor) == self.instance_id_of(target)
            && self.is_registered_on_event(requestor)
            && self.is_registered_on_event(target))
        {
            return Err("Event paticipants cannot be invited to parties.");
        }
        if self.team_of(requestor) != self.team_of(target) {
            return Err("You cannot be invited to a party of another team.");
        }
        Ok(())
    }

    /// The `getUCState() != UC_STATE_NONE` message of `RequestOustPartyMember` and
    /// `RequestWithDrawalParty`: `true` when the party command is refused.
    pub(crate) fn minigame_refuses_party_leave(&self, id: ObjectId) -> bool {
        if self.uc_state_of(id) == UC_STATE_NONE {
            return false;
        }
        self.send_message(
            id,
            "You can't dismiss party member when you are in Underground Coliseum.",
        );
        true
    }

    // ------------------------------------------------------------------ the olympiad (M8-OLY-01)

    /// Java `Player.isInOlympiadMode()`.
    // M8-OLY-01: the olympiad is not ported yet; this answers false until it is.
    pub(crate) fn minigame_in_olympiad_mode(&self, _id: ObjectId) -> bool {
        false
    }

    /// Java `OlympiadManager.getInstance().isRegistered(player)`.
    // M8-OLY-01: the olympiad is not ported yet; this answers false until it is.
    pub(crate) fn minigame_olympiad_registered(&self, _id: ObjectId) -> bool {
        false
    }

    /// Java `OlympiadManager.getInstance().unRegisterNoble(player)`.
    // M8-OLY-01: nothing to unregister until the olympiad is ported.
    pub(crate) fn minigame_olympiad_unregister(&mut self, _id: ObjectId) {}

    // ------------------------------------------------------------------ small helpers

    /// `creature.teleToLocation(x, y, z, true)`: with `MaxOffsetOnTeleport`.
    pub(crate) fn minigame_tele(&mut self, id: ObjectId, place: (i32, i32, i32)) {
        let offset = self.settings.teleport.max_offset_on_teleport;
        self.tele_to_location(
            id,
            crate::teleport::TeleportTarget::at(place.0, place.1, place.2).with_random_offset(offset),
        );
    }

    pub(crate) fn minigame_player_name(&self, id: ObjectId) -> String {
        self.players
            .get(&id)
            .map(|p| p.data.name.clone())
            .unwrap_or_default()
    }

    /// Sends a plain system message id.
    pub(crate) fn minigame_system_message(&self, to: ObjectId, id: l2_protocol_game::SystemMessageId) {
        self.send(to, &SystemMessage::new(id));
    }

    /// `player.getVariables().set(name, value)`.
    pub(crate) fn minigame_set_variable(&mut self, id: ObjectId, name: &str, value: String) {
        if let Some(player) = self.players.get_mut(&id) {
            let profile = &mut player.data.profile;
            profile.variables.insert(name.to_owned(), value);
            profile.variables_removed.remove(name);
            profile.variables_changed.insert(name.to_owned());
        }
    }

    /// `player.getVariables().getString(name, "")`.
    pub(crate) fn minigame_variable(&self, id: ObjectId, name: &str) -> String {
        self.players
            .get(&id)
            .and_then(|p| p.data.profile.variables.get(name).cloned())
            .unwrap_or_default()
    }

    /// The live NPC with template `npc_id` that has the lowest object id (Java
    /// `SpawnTable.getAnySpawn(npcId).getLastSpawn()`).
    pub(crate) fn minigame_find_npc(&self, npc_id: i32) -> Option<ObjectId> {
        self.npcs
            .iter()
            .filter(|(_, npc)| npc.npc_id == npc_id && !npc.decayed)
            .map(|(id, _)| *id)
            .min()
    }
}
