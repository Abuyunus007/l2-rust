//! The games of a stadium: `OlympiadGameNormal` (the classed and non-classed 1 vs 1 matches) and
//! `OlympiadGameTeams` (3 vs 3), with their participants, the removals, the points and the
//! result.
// Java: mechanics/olympiad/{AbstractOlympiadGame,OlympiadGameNormal,OlympiadGameClassed,
//       OlympiadGameNonClassed,OlympiadGameTeams,Participant,OlympiadInfo,CompetitionType}.java
//
// A game is stored in its stadium's task (`Stadium::task::game`) and refers to its players by
// object id. Java's `Participant` keeps a `Player` reference that `updatePlayer()` refreshes;
// here [`Participant::player`] is the id of that reference: set when the game is made, refreshed
// by `updatePlayer` (the id while the player is in the world, else `None`) and cleared by
// `clearPlayers`. Java's `StatSet` of a participant is the noble's entry of `Olympiad.NOBLES`;
// here the stats are read and written there by char id.

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    ExOlympiadMatchResult, ExOlympiadMode, ExOlympiadUserInfo, OlympiadResultEntry, SystemMessage,
    SystemMessageParam,
};
use l2_util::ObjectId;
use tracing::warn;

use super::{OlympiadFight, OlympiadWrite};
use crate::residence::ResidenceRef;
use crate::teleport::TeleportTarget;
use crate::world::World;

/// Java `OlympiadGameTeams.MAX_TEAM_SIZE`.
pub const MAX_TEAM_SIZE: usize = 3;
/// `removals`: the skills whose reuse is 15 minutes or less are enabled again.
const MAX_ENABLED_REUSE_MS: i32 = 900_000;

/// Java `CompetitionType` (without `OTHER`, which no game has).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompetitionType {
    Classed,
    NonClassed,
    Teams,
}

impl CompetitionType {
    /// Java `toString()`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Classed => "classed",
            Self::NonClassed => "non-classed",
            Self::Teams => "teams",
        }
    }
}

/// One fighter of a game (Java `Participant`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Participant {
    pub object_id: i32,
    /// Java `player`: see the module comment.
    pub player: Option<ObjectId>,
    pub name: String,
    pub side: i32,
    pub base_class: i32,
    pub disconnected: bool,
    pub defaulted: bool,
    pub clan_name: String,
    pub clan_id: i32,
    /// Java `new Participant(objId, side)`: the place holder of a team with fewer than three
    /// members (name "-", no stats, an id of the id factory).
    pub filler: bool,
}

/// The noble values a game changes (Java's `StatSet` keys).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NobleStat {
    Points,
    Done,
    Won,
    Lost,
    Drawn,
    DoneWeek,
    DoneWeekClassed,
    DoneWeekNonClassed,
    DoneWeekTeam,
}

/// One game (Java `AbstractOlympiadGame` and its subclasses). A classed or non-classed game has
/// one participant per side, a team game three (with fillers).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OlympiadGame {
    /// Java `_stadiumID`.
    pub stadium_id: usize,
    pub kind: CompetitionType,
    /// Java `_startTime` (unix ms).
    pub start_time: i64,
    pub aborted: bool,
    pub team_one: Vec<Participant>,
    pub team_two: Vec<Participant>,
    /// `_teamOneSize`, `_teamTwoSize`: the real members (1 for a normal game).
    pub team_one_size: usize,
    pub team_two_size: usize,
    /// `_teamOneDefaulted`, `_teamTwoDefaulted` (team games only).
    pub team_one_defaulted: bool,
    pub team_two_defaulted: bool,
    /// `_damageP1`/`_damageP2` (`_damageT1`/`_damageT2`).
    pub damage_one: i32,
    pub damage_two: i32,
}

impl OlympiadGame {
    /// Java `containsParticipant(playerId)`.
    pub fn contains_participant(&self, player_id: i32) -> bool {
        self.team_one[..self.team_one_size]
            .iter()
            .chain(&self.team_two[..self.team_two_size])
            .any(|p| p.object_id == player_id)
    }

    /// Java `getPlayerNames()`.
    pub fn player_names(&self) -> [String; 2] {
        [
            self.team_one.first().map(|p| p.name.clone()).unwrap_or_default(),
            self.team_two.first().map(|p| p.name.clone()).unwrap_or_default(),
        ]
    }

    fn is_teams(&self) -> bool {
        self.kind == CompetitionType::Teams
    }

    /// The real participants (`< _teamOneSize`, then `< _teamTwoSize`).
    fn real(&self) -> impl Iterator<Item = &Participant> {
        self.team_one[..self.team_one_size]
            .iter()
            .chain(&self.team_two[..self.team_two_size])
    }

    fn real_mut(&mut self) -> impl Iterator<Item = &mut Participant> {
        let (one, two) = (self.team_one_size, self.team_two_size);
        self.team_one[..one]
            .iter_mut()
            .chain(self.team_two[..two].iter_mut())
    }
}

impl World {
    /// The game of a stadium.
    pub(crate) fn olympiad_game(&self, index: usize) -> Option<&OlympiadGame> {
        self.olympiad.stadiums.as_ref()?.get(index)?.task.game.as_ref()
    }

    pub(crate) fn olympiad_game_mut(&mut self, index: usize) -> Option<&mut OlympiadGame> {
        self.olympiad
            .stadiums
            .as_mut()?
            .get_mut(index)?
            .task
            .game
            .as_mut()
    }

    /// Java `new Participant(player, side)`.
    fn new_participant(&self, player: ObjectId, side: i32) -> Participant {
        let clan_id = self.clan_id_of(player);
        let clan_name = self.clan(clan_id).map(|c| c.name.clone()).unwrap_or_default();
        Participant {
            object_id: player.0,
            player: Some(player),
            name: self
                .players
                .get(&player)
                .map(|p| p.data.name.clone())
                .unwrap_or_default(),
            side,
            base_class: self.olympiad_base_class(player),
            disconnected: false,
            defaulted: false,
            clan_name,
            clan_id,
            filler: false,
        }
    }

    /// A player is in the world (Java's `World.getPlayer(id) != null`, `isOnline()`).
    fn olympiad_online(&self, id: ObjectId) -> bool {
        self.players.contains_key(&id)
    }

    // ------------------------------------------------------------------------- creating games

    /// `OlympiadGameNormal.createListOfParticipants(list)` on a waiting list: two random online
    /// players leave it. `list` names the list: `None` the non-classed one, `Some(class)` a classed
    /// one.
    fn create_list_of_participants(&mut self, class: Option<i32>) -> Option<[Participant; 2]> {
        loop {
            let len = self.waiting_list(class).map_or(0, Vec::len);
            if len < 2 {
                return None;
            }
            let index = self.random_index(len);
            let one = self.waiting_list_mut(class)?.remove(index);
            if !self.olympiad_online(ObjectId(one)) {
                continue;
            }
            let len = self.waiting_list(class).map_or(0, Vec::len);
            let index = self.random_index(len);
            let two = self.waiting_list_mut(class)?.remove(index);
            if !self.olympiad_online(ObjectId(two)) {
                if let Some(list) = self.waiting_list_mut(class) {
                    list.push(one);
                }
                continue;
            }
            return Some([
                self.new_participant(ObjectId(one), 1),
                self.new_participant(ObjectId(two), 2),
            ]);
        }
    }

    /// `Rnd.get(n)`: 0 to n - 1.
    fn random_index(&mut self, len: usize) -> usize {
        let bound = i32::try_from(len).unwrap_or(i32::MAX);
        usize::try_from(self.rng().below(bound)).unwrap_or(0)
    }

    fn waiting_list(&self, class: Option<i32>) -> Option<&Vec<i32>> {
        match class {
            None => Some(&self.olympiad.non_class_based),
            Some(class) => self.olympiad.class_based.get(&class),
        }
    }

    fn waiting_list_mut(&mut self, class: Option<i32>) -> Option<&mut Vec<i32>> {
        match class {
            None => Some(&mut self.olympiad.non_class_based),
            Some(class) => self.olympiad.class_based.get_mut(&class),
        }
    }

    fn normal_game(&mut self, index: usize, kind: CompetitionType, pair: [Participant; 2]) -> OlympiadGame {
        let [one, two] = pair;
        // The constructor: `setOlympiadGameId(id)` of both.
        for id in [one.object_id, two.object_id] {
            if let Some(state) = self.player_olympiad_mut(ObjectId(id)) {
                state.game_id = i32::try_from(index).unwrap_or(-1);
            }
        }
        OlympiadGame {
            stadium_id: index,
            kind,
            start_time: 0,
            aborted: false,
            team_one: vec![one],
            team_two: vec![two],
            team_one_size: 1,
            team_two_size: 1,
            team_one_defaulted: false,
            team_two_defaulted: false,
            damage_one: 0,
            damage_two: 0,
        }
    }

    /// `OlympiadGameClassed.createGame(id, classList)`: a random list of the ready classes.
    pub(crate) fn create_classed_game(&mut self, index: usize, ready: &mut Vec<i32>) -> Option<OlympiadGame> {
        while !ready.is_empty() {
            let pick = self.random_index(ready.len());
            let class = ready[pick];
            if self.olympiad.class_based.get(&class).is_none_or(|l| l.len() < 2) {
                ready.remove(pick);
                continue;
            }
            let Some(pair) = self.create_list_of_participants(Some(class)) else {
                ready.remove(pick);
                continue;
            };
            return Some(self.normal_game(index, CompetitionType::Classed, pair));
        }
        None
    }

    /// `OlympiadGameNonClassed.createGame(id, list)`.
    pub(crate) fn create_non_classed_game(&mut self, index: usize) -> Option<OlympiadGame> {
        let pair = self.create_list_of_participants(None)?;
        Some(self.normal_game(index, CompetitionType::NonClassed, pair))
    }

    /// `OlympiadGameTeams.createGame(id, list)`: two random teams whose members are all online.
    pub(crate) fn create_teams_game(&mut self, index: usize) -> Option<OlympiadGame> {
        let mut team_one_players: Vec<ObjectId> = Vec::new();
        let mut team_two_players: Vec<ObjectId> = Vec::new();
        while self.olympiad.teams_based.len() > 1 {
            let pick = self.random_index(self.olympiad.teams_based.len());
            let team_one = self.olympiad.teams_based.remove(pick);
            if team_one.is_empty() {
                continue;
            }
            for id in &team_one {
                let id = ObjectId(*id);
                if !self.olympiad_online(id) {
                    team_one_players.clear();
                    break;
                }
                team_one_players.push(id);
            }
            if team_one_players.is_empty() {
                continue;
            }
            let pick = self.random_index(self.olympiad.teams_based.len());
            let team_two = self.olympiad.teams_based.remove(pick);
            if team_two.is_empty() {
                self.olympiad.teams_based.push(team_one);
                team_one_players.clear();
                continue;
            }
            for id in &team_two {
                let id = ObjectId(*id);
                if !self.olympiad_online(id) {
                    team_two_players.clear();
                    break;
                }
                team_two_players.push(id);
            }
            if team_two_players.is_empty() {
                self.olympiad.teams_based.push(team_one);
                team_one_players.clear();
                continue;
            }
            let t1: Vec<Participant> = team_one_players
                .iter()
                .map(|p| self.new_participant(*p, 1))
                .collect();
            let t2: Vec<Participant> = team_two_players
                .iter()
                .map(|p| self.new_participant(*p, 2))
                .collect();
            return Some(self.teams_game(index, t1, t2));
        }
        None
    }

    /// The `OlympiadGameTeams` constructor: the members get the game id, the missing places get a
    /// filler with an id of the id factory.
    fn teams_game(&mut self, index: usize, one: Vec<Participant>, two: Vec<Participant>) -> OlympiadGame {
        let one_size = one.len().min(MAX_TEAM_SIZE);
        let two_size = two.len().min(MAX_TEAM_SIZE);
        let game_id = i32::try_from(index).unwrap_or(-1);
        let mut team_one = Vec::with_capacity(MAX_TEAM_SIZE);
        let mut team_two = Vec::with_capacity(MAX_TEAM_SIZE);
        let mut one = one.into_iter();
        let mut two = two.into_iter();
        for i in 0..MAX_TEAM_SIZE {
            if i < one_size
                && let Some(par) = one.next()
            {
                if let Some(state) = par.player.and_then(|p| self.player_olympiad_mut(p)) {
                    state.game_id = game_id;
                }
                team_one.push(par);
            } else {
                team_one.push(self.filler_participant(1));
            }
            if i < two_size
                && let Some(par) = two.next()
            {
                if let Some(state) = par.player.and_then(|p| self.player_olympiad_mut(p)) {
                    state.game_id = game_id;
                }
                team_two.push(par);
            } else {
                team_two.push(self.filler_participant(2));
            }
        }
        OlympiadGame {
            stadium_id: index,
            kind: CompetitionType::Teams,
            start_time: 0,
            aborted: false,
            team_one,
            team_two,
            team_one_size: one_size,
            team_two_size: two_size,
            team_one_defaulted: false,
            team_two_defaulted: false,
            damage_one: 0,
            damage_two: 0,
        }
    }

    /// `new Participant(IdManager.getNextId(), side)`.
    fn filler_participant(&mut self, side: i32) -> Participant {
        let object_id = self
            .npc_world
            .ids
            .as_ref()
            .and_then(|ids| ids.allocate().ok())
            .map_or(0, |id| id.0);
        Participant {
            object_id,
            player: None,
            name: "-".to_owned(),
            side,
            base_class: 0,
            disconnected: false,
            defaulted: false,
            clan_name: String::new(),
            clan_id: 0,
            filler: true,
        }
    }

    // ------------------------------------------------------------------------- participants

    /// `updatePlayer()` of one participant.
    fn update_player(&mut self, index: usize, team: usize, slot: usize) -> Option<ObjectId> {
        let id = {
            let game = self.olympiad_game(index)?;
            let par = if team == 0 {
                game.team_one.get(slot)?
            } else {
                game.team_two.get(slot)?
            };
            (par.object_id, par.player)
        };
        let online = self.olympiad_online(ObjectId(id.0));
        let game = self.olympiad_game_mut(index)?;
        let par = if team == 0 {
            game.team_one.get_mut(slot)?
        } else {
            game.team_two.get_mut(slot)?
        };
        if id.1.is_none() || !online {
            par.player = online.then_some(ObjectId(id.0));
        }
        par.player
    }

    /// `Participant.updateStat(statName, increment)`: the value never goes below 0.
    pub(crate) fn update_noble_stat(&mut self, char_id: i32, stat: NobleStat, increment: i32) {
        let Some(noble) = self.olympiad.nobles.get_mut(&char_id) else {
            warn!("Olympiad: no noble stats for {char_id}, {stat:?} not changed");
            return;
        };
        let field = match stat {
            NobleStat::Points => &mut noble.points,
            NobleStat::Done => &mut noble.comp_done,
            NobleStat::Won => &mut noble.comp_won,
            NobleStat::Lost => &mut noble.comp_lost,
            NobleStat::Drawn => &mut noble.comp_drawn,
            NobleStat::DoneWeek => &mut noble.comp_done_week,
            NobleStat::DoneWeekClassed => &mut noble.comp_done_week_classed,
            NobleStat::DoneWeekNonClassed => &mut noble.comp_done_week_non_classed,
            NobleStat::DoneWeekTeam => &mut noble.comp_done_week_team,
        };
        *field = field.wrapping_add(increment).max(0);
    }

    /// `getWeeklyMatchType()`.
    fn weekly_match_type(kind: CompetitionType) -> NobleStat {
        match kind {
            CompetitionType::Classed => NobleStat::DoneWeekClassed,
            CompetitionType::NonClassed => NobleStat::DoneWeekNonClassed,
            CompetitionType::Teams => NobleStat::DoneWeekTeam,
        }
    }

    /// `getDivider()`.
    fn game_divider(&self, kind: CompetitionType) -> i32 {
        match kind {
            CompetitionType::Classed => self.olympiad.settings.divider_classed,
            CompetitionType::NonClassed => self.olympiad.settings.divider_non_classed,
            CompetitionType::Teams => 5,
        }
    }

    /// `getReward()`.
    fn game_reward(&self, kind: CompetitionType) -> Vec<[i32; 2]> {
        match kind {
            CompetitionType::Classed => self.olympiad.settings.classed_reward.clone(),
            CompetitionType::NonClassed => self.olympiad.settings.non_classed_reward.clone(),
            CompetitionType::Teams => self.olympiad.settings.team_reward.clone(),
        }
    }

    // ------------------------------------------------------------------------- broadcasts

    /// `broadcastPacket(packet)`: to the participants (each `updatePlayer()` first).
    pub(crate) fn game_broadcast(&mut self, index: usize, packet: &impl l2_net::Encode) {
        let Some((one, two)) = self
            .olympiad_game(index)
            .map(|g| (g.team_one_size, g.team_two_size))
        else {
            return;
        };
        let bytes = packet.to_bytes();
        for (team, size) in [(0, one), (1, two)] {
            for slot in 0..size {
                if let Some(player) = self.update_player(index, team, slot) {
                    self.send_bytes(player, bytes.clone());
                }
            }
        }
    }

    /// `new ExOlympiadUserInfo(participant)`.
    fn participant_user_info(&self, par: &Participant) -> ExOlympiadUserInfo {
        if let Some(player) = par.player.filter(|p| self.players.contains_key(p))
            && let (Some(p), Some(view)) = (self.players.get(&player), self.stats(player))
        {
            return ExOlympiadUserInfo {
                side: u8::try_from(p.olympiad.side).unwrap_or(0),
                object_id: player.0,
                name: p.data.name.clone(),
                class_id: p.data.class_id,
                cur_hp: p.core.cur_hp as i32,
                max_hp: view.max_hp(),
                cur_cp: p.core.cur_cp as i32,
                max_cp: view.max_cp(),
            };
        }
        ExOlympiadUserInfo {
            side: u8::try_from(par.side).unwrap_or(0),
            object_id: par.object_id,
            name: par.name.clone(),
            class_id: par.base_class,
            cur_hp: 0,
            max_hp: 100,
            cur_cp: 0,
            max_cp: 100,
        }
    }

    /// `sendOlympiadInfo(creature)`.
    pub(crate) fn game_send_olympiad_info(&self, index: usize, to: ObjectId) {
        let Some(game) = self.olympiad_game(index) else {
            return;
        };
        for par in game.team_one.iter().chain(&game.team_two) {
            self.send(to, &self.participant_user_info(par));
        }
    }

    /// `broadcastOlympiadInfo(stadium)`.
    pub(crate) fn game_broadcast_olympiad_info(&mut self, index: usize) {
        let Some(game) = self.olympiad_game(index) else {
            return;
        };
        let packets: Vec<ExOlympiadUserInfo> = game
            .team_one
            .iter()
            .chain(&game.team_two)
            .map(|par| self.participant_user_info(par))
            .collect();
        for packet in packets {
            self.stadium_broadcast(index, &packet);
        }
    }

    // ------------------------------------------------------------------------- the stages

    /// `checkDefaulted(player)`: the message for the opponents, `None` when the player may fight.
    fn check_defaulted_player(&mut self, player: Option<ObjectId>) -> Option<SystemMessage> {
        let cancelled =
            SystemMessageId::YourOpponentMadeHasteWithTheirTailBetweenTheirLegsTheMatchHasBeenCancelled;
        let requirements =
            SystemMessageId::YourOpponentDoesNotMeetTheRequirementsToDoBattleTheMatchHasBeenCancelled;
        let Some(player) = player.filter(|p| self.players.contains_key(p)) else {
            return Some(SystemMessage::new(cancelled));
        };
        // `getClient() == null || isDetached()`: an offline trader has no client.
        if !self.sinks.contains_key(&player) {
            return Some(SystemMessage::new(cancelled));
        }
        // "safety precautions" (`isRegisteredOnEvent()`: the events are scripts, D-05).
        if self.in_observer_mode(player) {
            return Some(SystemMessage::new(requirements));
        }
        let name = self
            .players
            .get(&player)
            .map(|p| p.data.name.clone())
            .unwrap_or_default();
        let own = if self.is_dead(player) {
            Some(
                SystemMessage::new(SystemMessageId::C1IsCurrentlyDeadAndCannotParticipateInTheOlympiad)
                    .with(SystemMessageParam::PlayerName(name)),
            )
        } else if self.olympiad_subclass_active(player) {
            Some(
                SystemMessage::new(
                    SystemMessageId::C1DoesNotMeetTheParticipationRequirementsYouCannotParticipateInTheOlympiadBecauseYouHaveChangedToYourSubClass,
                )
                .with(SystemMessageParam::PlayerName(name)),
            )
        } else if self.is_cursed_weapon_equipped(player) {
            let item = self
                .players
                .get(&player)
                .map_or(0, |p| p.cursed_weapon_equipped_id);
            Some(
                SystemMessage::new(
                    SystemMessageId::C1DoesNotMeetTheParticipationRequirementsTheOwnerOfS2CannotParticipateInTheOlympiad,
                )
                .with(SystemMessageParam::PlayerName(name))
                .with(SystemMessageParam::ItemName(item)),
            )
        } else if !self.is_inventory_under_90(player, true) {
            Some(
                SystemMessage::new(
                    SystemMessageId::C1DoesNotMeetTheParticipationRequirementsYouCannotParticipateInTheOlympiadBecauseYourInventorySlotExceeds80,
                )
                .with(SystemMessageParam::PlayerName(name)),
            )
        } else {
            None
        };
        let own = own?;
        self.send(player, &own);
        Some(SystemMessage::new(requirements))
    }

    /// `checkDefaulted()` of the game: `true` when a side defaulted.
    pub(crate) fn game_check_defaulted(&mut self, index: usize) -> bool {
        let Some(teams) = self.olympiad_game(index).map(OlympiadGame::is_teams) else {
            return true;
        };
        if !teams {
            self.update_player(index, 0, 0);
            self.update_player(index, 1, 0);
            let (one, two) = match self.olympiad_game(index) {
                Some(g) => (g.team_one[0].player, g.team_two[0].player),
                None => return true,
            };
            if let Some(reason) = self.check_defaulted_player(one) {
                if let Some(game) = self.olympiad_game_mut(index) {
                    game.team_one[0].defaulted = true;
                }
                if let Some(two) = two {
                    self.send(two, &reason);
                }
            }
            if let Some(reason) = self.check_defaulted_player(two) {
                if let Some(game) = self.olympiad_game_mut(index) {
                    game.team_two[0].defaulted = true;
                }
                if let Some(one) = one {
                    self.send(one, &reason);
                }
            }
            return self
                .olympiad_game(index)
                .is_none_or(|g| g.team_one[0].defaulted || g.team_two[0].defaulted);
        }
        let Some((one_size, two_size)) = self
            .olympiad_game(index)
            .map(|g| (g.team_one_size, g.team_two_size))
        else {
            return true;
        };
        for (team, size) in [(0usize, one_size), (1usize, two_size)] {
            for slot in (0..size).rev() {
                let player = self.update_player(index, team, slot);
                let Some(reason) = self.check_defaulted_player(player) else {
                    continue;
                };
                let Some(game) = self.olympiad_game_mut(index) else {
                    return true;
                };
                let (own, already) = if team == 0 {
                    game.team_one[slot].defaulted = true;
                    let already = game.team_one_defaulted;
                    game.team_one_defaulted = true;
                    (team, already)
                } else {
                    game.team_two[slot].defaulted = true;
                    let already = game.team_two_defaulted;
                    game.team_two_defaulted = true;
                    (team, already)
                };
                if !already {
                    // Java sends to every participant of the other team, the fillers included.
                    let others: Vec<ObjectId> = if own == 0 {
                        game.team_two.iter().filter_map(|p| p.player).collect()
                    } else {
                        game.team_one.iter().filter_map(|p| p.player).collect()
                    };
                    for other in others {
                        self.send(other, &reason);
                    }
                }
            }
        }
        self.olympiad_game(index)
            .is_none_or(|g| g.team_one_defaulted || g.team_two_defaulted)
    }

    /// `portPlayerToArena(par, loc, id)`.
    fn port_player_to_arena(&mut self, index: usize, par: &Participant, loc: (i32, i32, i32)) -> bool {
        let Some(player) = par.player.filter(|p| self.olympiad_online(*p)) else {
            return false;
        };
        let Some((x, y, z)) = self.position_of(player) else {
            return false;
        };
        let instance_id = self.olympiad.stadiums().get(index).map_or(0, |s| s.instance_id);
        let max_buffs = self.olympiad.settings.max_buffs;
        if let Some(state) = self.player_olympiad_mut(player) {
            // `setLastLocation()`.
            state.last_loc = (x, y, z);
        }
        if self.is_sitting(player) {
            self.stand_up(player);
        }
        self.set_target(player, None);
        if let Some(state) = self.player_olympiad_mut(player) {
            state.game_id = i32::try_from(index).unwrap_or(-1);
            state.in_olympiad_mode = true;
            state.olympiad_start = false;
            state.side = par.side;
            state.buff_count = max_buffs;
        }
        self.tele_to_location(
            player,
            TeleportTarget::at(loc.0, loc.1, loc.2).with_instance(instance_id),
        );
        self.send(player, &ExOlympiadMode { mode: 2 });
        // "FIXME: Make sure player has no buffs."
        self.olympiad_clean_effects(player);
        true
    }

    /// `portPlayersToArena(spawns)`.
    pub(crate) fn game_port_players_to_arena(&mut self, index: usize, spawns: &[(i32, i32, i32)]) -> bool {
        let Some(game) = self.olympiad_game(index).cloned() else {
            return false;
        };
        if spawns.is_empty() {
            warn!(
                "portPlayersToArena: the stadium {index} has no spawn (java.lang.IndexOutOfBoundsException)"
            );
            return false;
        }
        let offset = spawns.len() / 2;
        let mut result = true;
        if game.is_teams() {
            for (i, par) in game.team_one[..game.team_one_size].iter().enumerate() {
                let Some(loc) = spawns.get(i).copied() else {
                    warn!("portPlayersToArena: java.lang.IndexOutOfBoundsException: Index {i}");
                    return false;
                };
                result &= self.port_player_to_arena(index, par, loc);
            }
            for (i, par) in game.team_two[..game.team_two_size].iter().enumerate() {
                let Some(loc) = spawns.get(i + offset).copied() else {
                    warn!(
                        "portPlayersToArena: java.lang.IndexOutOfBoundsException: Index {}",
                        i + offset
                    );
                    return false;
                };
                result &= self.port_player_to_arena(index, par, loc);
            }
        } else {
            result &= self.port_player_to_arena(index, &game.team_one[0], spawns[0]);
            let Some(loc) = spawns.get(offset).copied() else {
                return false;
            };
            result &= self.port_player_to_arena(index, &game.team_two[0], loc);
        }
        result
    }

    /// `removals(player, removeParty)`.
    fn olympiad_removals(&mut self, player: Option<ObjectId>, remove_party: bool) {
        let Some(player) = player.filter(|p| self.players.contains_key(p)) else {
            return;
        };
        // Remove Buffs.
        self.stop_all_effects_except_those_that_last_through_death(player);
        // Remove Clan Skills.
        let clan_id = self.clan_id_of(player);
        if clan_id != 0 {
            self.clan_remove_skill_effects(clan_id, player);
            if let Some(castle) = self.castle_by_owner(clan_id) {
                self.remove_residential_skills(ResidenceRef::Castle(castle), player);
            }
            if let Some(fort) = self.fort_by_owner(clan_id) {
                self.remove_residential_skills(ResidenceRef::Fort(fort), player);
            }
        }
        // Abort casting if player casting.
        self.abort_attack(player);
        self.abort_cast(player);
        // Force the character to be visible.
        self.set_invisible(player, false);
        // Heal Player fully.
        self.olympiad_heal_full(player);
        // Remove Summon's Buffs.
        if let Some(summon) = self.summon_of(player) {
            self.stop_all_effects_except_those_that_last_through_death(summon);
            self.abort_attack(summon);
            self.abort_cast(summon);
            if self.is_pet(summon) {
                self.unsummon(summon, Some(player));
            }
        }
        // Stop any cubic that has been given by other player.
        self.stop_cubics_by_others(player);
        // Remove player from his party.
        if remove_party {
            self.olympiad_expel_from_party(player);
        }
        // Remove Agathion: the agathions are not modelled (`getAgathionId()` is 0).
        let mut changed = Vec::new();
        self.check_item_restriction(player, &mut changed);
        // Remove shot automation.
        self.disable_auto_shots_all(player);
        // Discharge any active shots.
        if let Some(weapon) =
            self.paperdoll_object(player, l2_protocol_game::server::charscreen::paperdoll::RHAND)
        {
            self.un_charge_all_shots(weapon);
        }
        // "enable skills with cool time <= 15 minutes"
        let skills: Vec<l2_data::SkillTemplate> = match (self.core_of(player), self.data.stat_data()) {
            (Some(core), Some(data)) => core
                .skills
                .iter()
                .filter_map(|(id, level)| data.skills.get(*id, *level))
                .filter(|s| s.reuse_delay <= MAX_ENABLED_REUSE_MS)
                .cloned()
                .collect(),
            _ => Vec::new(),
        };
        for skill in &skills {
            self.enable_skill(player, skill);
        }
        self.send_skill_list(player);
        let cool_time = self.skill_cool_time(player);
        self.send(player, &cool_time);
    }

    /// CP, HP and MP to the maximum.
    fn olympiad_heal_full(&mut self, player: ObjectId) {
        let Some(view) = self.stats(player) else {
            return;
        };
        let (cp, hp, mp) = (
            f64::from(view.max_cp()),
            f64::from(view.max_hp()),
            f64::from(view.max_mp()),
        );
        self.set_current_cp(player, cp, true);
        self.set_current_hp(player, hp, true);
        self.set_current_mp(player, mp, true);
    }

    /// `removals()` of the game.
    pub(crate) fn game_removals(&mut self, index: usize) {
        let Some(game) = self.olympiad_game(index).cloned() else {
            return;
        };
        if game.is_teams() {
            for par in game.team_one[..game.team_one_size].iter().rev() {
                self.olympiad_removals(par.player, false);
            }
            for par in game.team_two[..game.team_two_size].iter().rev() {
                self.olympiad_removals(par.player, false);
            }
        } else {
            if game.aborted {
                return;
            }
            self.olympiad_removals(game.team_one[0].player, true);
            self.olympiad_removals(game.team_two[0].player, true);
        }
    }

    /// `cleanEffects(player)`: "prevent players kill each other".
    pub(crate) fn olympiad_clean_effects(&mut self, player: ObjectId) {
        if !self.players.contains_key(&player) {
            return;
        }
        if let Some(state) = self.player_olympiad_mut(player) {
            state.olympiad_start = false;
        }
        self.set_target(player, None);
        self.abort_attack(player);
        self.abort_cast(player);
        self.set_intention_idle(player);
        if self.is_dead(player) {
            self.set_dead(player, false);
        }
        self.stop_all_effects_except_those_that_last_through_death(player);
        self.clear_souls(player);
        self.clear_charges(player);
        // `setAgathionId(0)`: the agathions are not modelled.
        if let Some(summon) = self.summon_of(player)
            && !self.is_dead(summon)
        {
            self.set_target(summon, None);
            self.abort_attack(summon);
            self.abort_cast(summon);
            self.set_intention_idle(summon);
            self.stop_all_effects_except_those_that_last_through_death(summon);
        }
        self.olympiad_heal_full(player);
        self.start_hp_mp_regeneration(player);
    }

    /// `cleanEffects()` of the game: the participants still in this game.
    pub(crate) fn game_clean_effects(&mut self, index: usize) {
        for player in self.game_players_in_game(index) {
            self.olympiad_clean_effects(player);
        }
    }

    /// The participants with a player that did not default or disconnect and are still in this
    /// game (`getOlympiadGameId() == _stadiumID`), in Java's loop order.
    fn game_players_in_game(&self, index: usize) -> Vec<ObjectId> {
        let Some(game) = self.olympiad_game(index) else {
            return Vec::new();
        };
        let game_id = i32::try_from(index).unwrap_or(-1);
        let ok = |par: &Participant| {
            par.player
                .filter(|p| !par.defaulted && !par.disconnected && self.olympiad_game_id(*p) == game_id)
        };
        if game.is_teams() {
            game.team_one[..game.team_one_size]
                .iter()
                .rev()
                .chain(game.team_two[..game.team_two_size].iter().rev())
                .filter_map(ok)
                .collect()
        } else {
            [&game.team_one[0], &game.team_two[0]]
                .into_iter()
                .filter_map(ok)
                .collect()
        }
    }

    /// `makeCompetitionStart()`.
    pub(crate) fn game_make_competition_start(&mut self, index: usize) -> bool {
        let now = self.olympiad_now_ms();
        let Some(game) = self.olympiad_game_mut(index) else {
            return false;
        };
        game.start_time = now;
        if game.aborted {
            return false;
        }
        let players: Vec<Option<ObjectId>> = game.real().map(|p| p.player).collect();
        if players.iter().any(Option::is_none) {
            // A normal game checks both first; a team game stops at the first missing one,
            // after the ones before it started (the same here: nobody is missing then).
            return false;
        }
        for player in players.into_iter().flatten() {
            if let Some(state) = self.player_olympiad_mut(player) {
                state.olympiad_start = true;
            }
            self.update_effect_icons(player, false);
        }
        true
    }

    /// `portPlayersBack()`.
    pub(crate) fn game_port_players_back(&mut self, index: usize) {
        let Some(game) = self.olympiad_game(index).cloned() else {
            return;
        };
        let list: Vec<ObjectId> = if game.is_teams() {
            game.team_one[..game.team_one_size]
                .iter()
                .rev()
                .chain(game.team_two[..game.team_two_size].iter().rev())
                .filter(|p| !p.defaulted && !p.disconnected)
                .filter_map(|p| p.player)
                .collect()
        } else {
            [&game.team_one[0], &game.team_two[0]]
                .into_iter()
                .filter(|p| !p.defaulted && !p.disconnected)
                .filter_map(|p| p.player)
                .collect()
        };
        for player in list {
            self.olympiad_port_player_back(player);
        }
    }

    /// `portPlayerBack(player)`.
    fn olympiad_port_player_back(&mut self, player: ObjectId) {
        let Some(loc) = self.player_olympiad(player).map(|s| s.last_loc) else {
            return;
        };
        if loc.0 == 0 && loc.1 == 0 {
            return;
        }
        self.death.pending_revive.remove(&player);
        self.set_instance_id(player, 0);
        self.tele_to_location(player, TeleportTarget::at(loc.0, loc.1, loc.2));
        if let Some(state) = self.player_olympiad_mut(player) {
            state.last_loc = (0, 0, 0);
        }
    }

    /// `playersStatusBack()`.
    pub(crate) fn game_players_status_back(&mut self, index: usize) {
        for player in self.game_players_in_game(index) {
            self.olympiad_player_status_back(player);
        }
    }

    /// `playerStatusBack(player)`.
    fn olympiad_player_status_back(&mut self, player: ObjectId) {
        if self.is_transformed(player) {
            self.untransform(player);
        }
        if self.is_in_olympiad_mode(player) {
            self.send(player, &ExOlympiadMode { mode: 0 });
        }
        if let Some(state) = self.player_olympiad_mut(player) {
            state.in_olympiad_mode = false;
            state.olympiad_start = false;
            state.side = -1;
            state.game_id = -1;
        }
        // Add Clan Skills.
        let clan_id = self.clan_id_of(player);
        if clan_id != 0 {
            self.clan_add_skill_effects(clan_id, player);
            if let Some(castle) = self.castle_by_owner(clan_id) {
                self.give_residential_skills(ResidenceRef::Castle(castle), player);
            }
            if let Some(fort) = self.fort_by_owner(clan_id) {
                self.give_residential_skills(ResidenceRef::Fort(fort), player);
            }
            self.send_skill_list(player);
        }
        // "heal again after adding clan skills"
        self.olympiad_heal_full(player);
        self.start_hp_mp_regeneration(player);
    }

    /// `clearPlayers()`: the fillers give their ids back.
    pub(crate) fn game_clear_players(&mut self, index: usize) {
        let fillers: Vec<i32> = match self.olympiad_game_mut(index) {
            Some(game) => {
                for par in game.team_one.iter_mut().chain(game.team_two.iter_mut()) {
                    par.player = None;
                }
                game.team_one
                    .iter()
                    .chain(&game.team_two)
                    .filter(|p| p.filler)
                    .map(|p| p.object_id)
                    .collect()
            }
            None => return,
        };
        if let Some(ids) = self.npc_world.ids.as_ref() {
            for id in fillers {
                ids.release(ObjectId(id));
            }
        }
    }

    /// `handleDisconnect(player)`.
    pub(crate) fn olympiad_handle_disconnect(&mut self, index: usize, player: ObjectId) {
        if let Some(game) = self.olympiad_game_mut(index)
            && let Some(par) = game.real_mut().find(|p| p.object_id == player.0)
        {
            par.disconnected = true;
        }
    }

    /// `checkBattleStatus()`.
    pub(crate) fn game_check_battle_status(&self, index: usize) -> bool {
        let Some(game) = self.olympiad_game(index) else {
            return false;
        };
        if game.aborted {
            return false;
        }
        if game.is_teams() {
            let all_gone = |team: &[Participant]| team.iter().all(|p| p.disconnected);
            return !all_gone(&game.team_one[..game.team_one_size])
                && !all_gone(&game.team_two[..game.team_two_size]);
        }
        let ok = |par: &Participant| par.player.is_some() && !par.disconnected;
        ok(&game.team_one[0]) && ok(&game.team_two[0])
    }

    /// `haveWinner()`.
    pub(crate) fn game_have_winner(&self, index: usize) -> bool {
        if !self.game_check_battle_status(index) {
            return true;
        }
        let Some(game) = self.olympiad_game(index) else {
            return true;
        };
        let game_id = i32::try_from(index).unwrap_or(-1);
        if game.is_teams() {
            let lost = |team: &[Participant]| {
                team.iter()
                    .filter(|p| !p.disconnected)
                    .filter_map(|p| p.player)
                    .filter(|p| self.olympiad_game_id(*p) == game_id)
                    .all(|p| self.is_dead(p))
            };
            return lost(&game.team_one[..game.team_one_size]) || lost(&game.team_two[..game.team_two_size]);
        }
        // Java's `catch (Exception)` around each: a missing player has lost.
        let lost = |par: &Participant| match par.player {
            Some(p) if self.players.contains_key(&p) && self.olympiad_game_id(p) == game_id => {
                self.is_dead(p)
            }
            _ => true,
        };
        lost(&game.team_one[0]) || lost(&game.team_two[0])
    }

    /// `addDamage(player, damage)`.
    pub(crate) fn game_add_damage(&mut self, index: usize, player: ObjectId, damage: i32) {
        let Some(game) = self.olympiad_game(index) else {
            return;
        };
        if game.is_teams() {
            let one = game.team_one[..game.team_one_size]
                .iter()
                .rev()
                .find(|p| p.object_id == player.0)
                .map(|p| !p.disconnected);
            let two = game.team_two[..game.team_two_size]
                .iter()
                .rev()
                .find(|p| p.object_id == player.0)
                .map(|p| !p.disconnected);
            if let Some(game) = self.olympiad_game_mut(index) {
                match (one, two) {
                    (Some(true), _) => game.damage_one = game.damage_one.wrapping_add(damage),
                    (Some(false), _) => {}
                    (None, Some(true)) => game.damage_two = game.damage_two.wrapping_add(damage),
                    _ => {}
                }
            }
            return;
        }
        let (Some(one), Some(two)) = (game.team_one[0].player, game.team_two[0].player) else {
            return;
        };
        if player == one {
            if !self.is_invul(two)
                && let Some(game) = self.olympiad_game_mut(index)
            {
                game.damage_one = game.damage_one.wrapping_add(damage);
            }
        } else if player == two
            && !self.is_invul(one)
            && let Some(game) = self.olympiad_game_mut(index)
        {
            game.damage_two = game.damage_two.wrapping_add(damage);
        }
    }

    /// `resetDamage()`.
    pub(crate) fn game_reset_damage(&mut self, index: usize) {
        if let Some(game) = self.olympiad_game_mut(index) {
            game.damage_one = 0;
            game.damage_two = 0;
        }
    }

    /// `makePlayersInvul()` / `removePlayersInvul()`: every participant with a player (the
    /// fillers have none).
    pub(crate) fn game_set_players_invul(&mut self, index: usize, invul: bool) {
        let players: Vec<ObjectId> = match self.olympiad_game(index) {
            Some(game) => game
                .team_one
                .iter()
                .chain(&game.team_two)
                .filter_map(|p| p.player)
                .collect(),
            None => return,
        };
        for player in players {
            self.set_invul(player, invul);
        }
    }

    // ------------------------------------------------------------------------- the result

    fn result_entry(&self, par: &Participant, damage: i32, current: i32, diff: i32) -> OlympiadResultEntry {
        OlympiadResultEntry {
            name: par.name.clone(),
            clan_name: par.clan_name.clone(),
            clan_id: par.clan_id,
            class_id: par.base_class,
            damage,
            current_points: current,
            diff_points: diff,
        }
    }

    /// `addPointsToParticipant(par, points)`.
    fn add_points_to_participant(&mut self, index: usize, par: &Participant, points: i32) {
        self.update_noble_stat(par.object_id, NobleStat::Points, points);
        let message = SystemMessage::new(SystemMessageId::C1HasEarnedS2PointsInTheGrandOlympiadGames)
            .with(SystemMessageParam::Text(par.name.clone()))
            .with(SystemMessageParam::IntNumber(points));
        self.game_broadcast(index, &message);
    }

    /// `removePointsFromParticipant(par, points)`.
    fn remove_points_from_participant(&mut self, index: usize, par: &Participant, points: i32) {
        self.update_noble_stat(par.object_id, NobleStat::Points, points.wrapping_neg());
        let message = SystemMessage::new(SystemMessageId::C1HasLostS2PointsInTheGrandOlympiadGames)
            .with(SystemMessageParam::Text(par.name.clone()))
            .with(SystemMessageParam::IntNumber(points));
        self.game_broadcast(index, &message);
    }

    /// `rewardParticipant(player, reward)`.
    fn reward_participant(&mut self, player: Option<ObjectId>, reward: &[[i32; 2]]) {
        let Some(player) = player.filter(|p| self.olympiad_online(*p)) else {
            return;
        };
        for [item_id, count] in reward {
            if self
                .add_item(player, *item_id, i64::from(*count), false)
                .is_none()
            {
                continue;
            }
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouHaveEarnedS2S1S)
                    .with(SystemMessageParam::ItemName(*item_id))
                    .with(SystemMessageParam::IntNumber(*count)),
            );
        }
    }

    fn win_message(&mut self, index: usize, name: &str) {
        let message = SystemMessage::new(SystemMessageId::CongratulationsC1YouWinTheMatch)
            .with(SystemMessageParam::Text(name.to_owned()));
        self.stadium_broadcast(index, &message);
    }

    fn tie_message(&mut self, index: usize) {
        self.stadium_broadcast(
            index,
            &SystemMessage::new(SystemMessageId::ThereIsNoVictorTheMatchEndsInATie),
        );
    }

    fn broadcast_result(
        &mut self,
        index: usize,
        tie: bool,
        winside: i32,
        list1: Vec<OlympiadResultEntry>,
        list2: Vec<OlympiadResultEntry>,
    ) {
        let result = if winside == 1 {
            ExOlympiadMatchResult::new(tie, winside, list1, list2)
        } else {
            ExOlympiadMatchResult::new(tie, winside, list2, list1)
        };
        self.stadium_broadcast(index, &result);
    }

    /// `validateWinner(stadium)`.
    pub(crate) fn game_validate_winner(&mut self, index: usize) {
        let Some(game) = self.olympiad_game(index).cloned() else {
            return;
        };
        if game.aborted {
            return;
        }
        if game.is_teams() {
            self.validate_winner_teams(index, &game);
        } else {
            self.validate_winner_normal(index, &game);
        }
    }

    /// `OlympiadGameNormal.validateWinner`.
    fn validate_winner_normal(&mut self, index: usize, game: &OlympiadGame) {
        let max_points = self.olympiad.settings.max_points;
        let divider = self.game_divider(game.kind);
        let reward = self.game_reward(game.kind);
        let weekly = Self::weekly_match_type(game.kind);
        let (one, two) = (game.team_one[0].clone(), game.team_two[0].clone());
        let (damage_one, damage_two) = (game.damage_one, game.damage_two);
        let mut tie = false;
        let mut winside = 0;
        let mut list1 = Vec::new();
        let mut list2 = Vec::new();

        let one_crash = one.player.is_none() || one.disconnected;
        let two_crash = two.player.is_none() || two.disconnected;
        let one_points = self.noble_points(one.object_id);
        let two_points = self.noble_points(two.object_id);
        let Some(mut point_diff) = one_points.min(two_points).checked_div(divider) else {
            warn!("Olympiad: validateWinner: java.lang.ArithmeticException: / by zero");
            return;
        };
        if point_diff <= 0 {
            point_diff = 1;
        } else if point_diff > max_points {
            point_diff = max_points;
        }

        // "Check for if a player defaulted before battle started."
        if one.defaulted || two.defaulted {
            if one.defaulted {
                let points = (one_points / 3).min(max_points);
                self.remove_points_from_participant(index, &one, points);
                list1.push(self.result_entry(&one, damage_one, one_points - points, -points));
                winside = 2;
            }
            if two.defaulted {
                let points = (two_points / 3).min(max_points);
                self.remove_points_from_participant(index, &two, points);
                list2.push(self.result_entry(&two, damage_two, two_points - points, -points));
                if winside == 2 {
                    tie = true;
                } else {
                    winside = 1;
                }
            }
            self.broadcast_result(index, tie, winside, list1, list2);
            return;
        }

        // "Create results for players if a player crashed."
        if one_crash || two_crash {
            if two_crash && !one_crash {
                self.win_message(index, &one.name);
                self.update_noble_stat(one.object_id, NobleStat::Won, 1);
                self.add_points_to_participant(index, &one, point_diff);
                list1.push(self.result_entry(&one, damage_one, one_points + point_diff, point_diff));
                self.update_noble_stat(two.object_id, NobleStat::Lost, 1);
                self.remove_points_from_participant(index, &two, point_diff);
                list2.push(self.result_entry(&two, damage_two, two_points - point_diff, -point_diff));
                winside = 1;
                self.reward_participant(one.player, &reward);
            } else if one_crash && !two_crash {
                self.win_message(index, &two.name);
                self.update_noble_stat(two.object_id, NobleStat::Won, 1);
                self.add_points_to_participant(index, &two, point_diff);
                list2.push(self.result_entry(&two, damage_two, two_points + point_diff, point_diff));
                self.update_noble_stat(one.object_id, NobleStat::Lost, 1);
                self.remove_points_from_participant(index, &one, point_diff);
                list1.push(self.result_entry(&one, damage_one, one_points - point_diff, -point_diff));
                winside = 2;
                self.reward_participant(two.player, &reward);
            } else {
                self.tie_message(index);
                self.update_noble_stat(one.object_id, NobleStat::Lost, 1);
                self.remove_points_from_participant(index, &one, point_diff);
                list1.push(self.result_entry(&one, damage_one, one_points - point_diff, -point_diff));
                self.update_noble_stat(two.object_id, NobleStat::Lost, 1);
                self.remove_points_from_participant(index, &two, point_diff);
                list2.push(self.result_entry(&two, damage_two, two_points - point_diff, -point_diff));
                tie = true;
            }
            for par in [&one, &two] {
                self.update_noble_stat(par.object_id, NobleStat::Done, 1);
                self.update_noble_stat(par.object_id, NobleStat::DoneWeek, 1);
                self.update_noble_stat(par.object_id, weekly, 1);
            }
            self.broadcast_result(index, tie, winside, list1, list2);
            return;
        }

        // "Calculate Fight time"
        let fight_time = self.olympiad_now_ms() - game.start_time;
        let hp_of = |world: &World, par: &Participant| -> f64 {
            match par.player.filter(|p| world.players.contains_key(p)) {
                Some(p) if !world.is_dead(p) => {
                    let hp = world
                        .players
                        .get(&p)
                        .map_or(0.0, |pl| pl.core.cur_hp + pl.core.cur_cp);
                    if hp < 0.5 { 0.0 } else { hp }
                }
                _ => 0.0,
            }
        };
        let one_hp = hp_of(self, &one);
        let two_hp = hp_of(self, &two);
        // "If players crashed, search if they've relogged."
        let one_player = self.update_player(index, 0, 0);
        let two_player = self.update_player(index, 1, 0);
        let one_online = one_player.is_some_and(|p| self.olympiad_online(p));
        let two_online = two_player.is_some_and(|p| self.olympiad_online(p));

        if !one_online && !two_online {
            // NOTE(java-quirk): no points move and the result names no tie.
            self.update_noble_stat(one.object_id, NobleStat::Drawn, 1);
            self.update_noble_stat(two.object_id, NobleStat::Drawn, 1);
            self.tie_message(index);
        } else if !two_online
            || (two_hp == 0.0 && one_hp != 0.0)
            || (damage_one > damage_two && two_hp != 0.0 && one_hp != 0.0)
        {
            self.win_message(index, &one.name);
            self.update_noble_stat(one.object_id, NobleStat::Won, 1);
            self.update_noble_stat(two.object_id, NobleStat::Lost, 1);
            self.add_points_to_participant(index, &one, point_diff);
            list1.push(self.result_entry(&one, damage_one, one_points + point_diff, point_diff));
            self.remove_points_from_participant(index, &two, point_diff);
            list2.push(self.result_entry(&two, damage_two, two_points - point_diff, -point_diff));
            winside = 1;
            self.save_results(&one, &two, 1, game.start_time, fight_time, game.kind);
            self.reward_participant(one_player, &reward);
        } else if !one_online
            || (one_hp == 0.0 && two_hp != 0.0)
            || (damage_two > damage_one && one_hp != 0.0 && two_hp != 0.0)
        {
            self.win_message(index, &two.name);
            self.update_noble_stat(two.object_id, NobleStat::Won, 1);
            self.update_noble_stat(one.object_id, NobleStat::Lost, 1);
            self.add_points_to_participant(index, &two, point_diff);
            list2.push(self.result_entry(&two, damage_two, two_points + point_diff, point_diff));
            self.remove_points_from_participant(index, &one, point_diff);
            list1.push(self.result_entry(&one, damage_one, one_points - point_diff, -point_diff));
            winside = 2;
            self.save_results(&one, &two, 2, game.start_time, fight_time, game.kind);
            self.reward_participant(two_player, &reward);
        } else {
            self.save_results(&one, &two, 0, game.start_time, fight_time, game.kind);
            self.tie_message(index);
            let value = (one_points / divider).min(max_points);
            self.remove_points_from_participant(index, &one, value);
            list1.push(self.result_entry(&one, damage_one, one_points - value, -value));
            let value = (two_points / divider).min(max_points);
            self.remove_points_from_participant(index, &two, value);
            list2.push(self.result_entry(&two, damage_two, two_points - value, -value));
            tie = true;
        }
        for par in [&one, &two] {
            self.update_noble_stat(par.object_id, NobleStat::Done, 1);
            self.update_noble_stat(par.object_id, NobleStat::DoneWeek, 1);
            self.update_noble_stat(par.object_id, weekly, 1);
        }
        self.broadcast_result(index, tie, winside, list1, list2);
    }

    /// `saveResults(one, two, winner, startTime, fightTime, type)`.
    fn save_results(
        &mut self,
        one: &Participant,
        two: &Participant,
        winner: i32,
        start: i64,
        time: i64,
        kind: CompetitionType,
    ) {
        self.olympiad_write(OlympiadWrite::Fight(OlympiadFight {
            char_one_id: one.object_id,
            char_two_id: two.object_id,
            char_one_class: one.base_class,
            char_two_class: two.base_class,
            winner,
            start,
            time,
            classed: kind == CompetitionType::Classed,
        }));
    }

    /// `OlympiadGameTeams.validateWinner`.
    fn validate_winner_teams(&mut self, index: usize, game: &OlympiadGame) {
        let max_points = self.olympiad.settings.max_points;
        let divider = self.game_divider(game.kind);
        let reward = self.game_reward(game.kind);
        let weekly = Self::weekly_match_type(game.kind);
        let one: Vec<Participant> = game.team_one[..game.team_one_size].to_vec();
        let two: Vec<Participant> = game.team_two[..game.team_two_size].to_vec();
        let (damage_one, damage_two) = (game.damage_one, game.damage_two);
        let mut tie = false;
        let mut winside = 0;
        let mut list1 = Vec::new();
        let mut list2 = Vec::new();
        let one_crash = one.iter().all(|p| p.disconnected);
        let two_crash = two.iter().all(|p| p.disconnected);

        // "Check for if a team defaulted before battle started"
        if game.team_one_defaulted || game.team_two_defaulted {
            if game.team_one_defaulted {
                for par in one.iter().rev() {
                    let current = self.noble_points(par.object_id);
                    let points = current / divider;
                    let val = (current / 3).min(max_points);
                    self.remove_points_from_participant(index, par, val);
                    // NOTE(java-quirk): the "current points" are the divided points minus the loss.
                    list1.push(self.result_entry(par, damage_one, points - val, -val));
                }
                winside = 2;
            }
            if game.team_two_defaulted {
                for par in two.iter().rev() {
                    let current = self.noble_points(par.object_id);
                    let points = current / divider;
                    let val = (current / 3).min(max_points);
                    self.remove_points_from_participant(index, par, val);
                    list2.push(self.result_entry(par, damage_two, points - val, -val));
                }
                if winside == 2 {
                    tie = true;
                } else {
                    winside = 1;
                }
            }
            self.broadcast_result(index, tie, winside, list1, list2);
            return;
        }

        // "points to be dedicted in case of losing"
        let capped = |world: &World, par: &Participant| {
            let points = world.noble_points(par.object_id) / divider;
            if points <= 0 {
                1
            } else if points > max_points {
                max_points
            } else {
                points
            }
        };
        let mut points_one: Vec<i32> = one.iter().map(|p| capped(self, p)).collect();
        let mut points_two: Vec<i32> = two.iter().map(|p| capped(self, p)).collect();
        let max_one = points_one.clone();
        let max_two = points_two.clone();
        let mut total_one: i32 = points_one.iter().sum();
        let mut total_two: i32 = points_two.iter().sum();
        // "Choose minimum sum"
        let team = i32::try_from(MAX_TEAM_SIZE).unwrap_or(3);
        let min = (total_one.min(total_two) / team) * team;
        // "calculating coefficients and trying to correct total number of points for each team"
        let divider_one = f64::from(total_one) / f64::from(min);
        let divider_two = f64::from(total_two) / f64::from(min);
        total_one = min;
        total_two = min;
        for p in &mut points_one {
            *p = ((f64::from(*p) / divider_one) as i32).max(1);
            total_one -= *p;
        }
        for p in points_two.iter_mut().rev() {
            *p = ((f64::from(*p) / divider_two) as i32).max(1);
            total_two -= *p;
        }
        // "compensating remaining points, first team from begin to end, second from end to begin"
        for i in 0..points_one.len() {
            if total_one <= 0 {
                break;
            }
            if points_one[i] < max_one[i] {
                points_one[i] += 1;
                total_one -= 1;
            }
        }
        for i in (0..points_two.len()).rev() {
            if total_two <= 0 {
                break;
            }
            if points_two[i] < max_two[i] {
                points_two[i] += 1;
                total_two -= 1;
            }
        }

        let win_points = min / team;
        let done = |world: &mut World| {
            for par in one.iter().rev().chain(two.iter().rev()) {
                world.update_noble_stat(par.object_id, NobleStat::Done, 1);
                world.update_noble_stat(par.object_id, NobleStat::DoneWeek, 1);
                world.update_noble_stat(par.object_id, weekly, 1);
            }
        };

        // "Create results for players if a team crashed"
        if one_crash || two_crash {
            if two_crash && !one_crash {
                self.win_message(index, &one[0].name);
                self.team_loses(index, &two, &points_two, damage_two, &mut list2);
                self.team_wins(index, &one, win_points, damage_one, &mut list1);
                for par in &one {
                    self.reward_participant(par.player, &reward);
                }
                winside = 1;
            } else if one_crash && !two_crash {
                self.win_message(index, &two[0].name);
                self.team_loses(index, &one, &points_one, damage_one, &mut list1);
                self.team_wins(index, &two, win_points, damage_two, &mut list2);
                winside = 2;
                for par in &two {
                    self.reward_participant(par.player, &reward);
                }
            } else {
                self.tie_message(index);
                for (i, par) in one.iter().enumerate().rev() {
                    self.update_noble_stat(par.object_id, NobleStat::Lost, 1);
                    self.remove_points_from_participant(index, par, points_one[i]);
                    let current = self.noble_points(par.object_id);
                    list1.push(self.result_entry(par, damage_one, current - points_one[i], -points_one[i]));
                }
                for (i, par) in two.iter().enumerate().rev() {
                    self.update_noble_stat(par.object_id, NobleStat::Lost, 1);
                    self.remove_points_from_participant(index, par, points_two[i]);
                    // NOTE(java-quirk): the second team's entries show the first team's points.
                    let Some(&shown) = points_one.get(i) else {
                        warn!(
                            "Exception on validateWinner(): java.lang.ArrayIndexOutOfBoundsException: Index {i}"
                        );
                        return;
                    };
                    let current = self.noble_points(par.object_id);
                    list2.push(self.result_entry(par, damage_two, current - shown, -shown));
                }
                tie = true;
            }
            done(self);
            self.broadcast_result(index, tie, winside, list1, list2);
            return;
        }

        let team_hp = |world: &mut World, team_index: usize, team: &[Participant]| -> f64 {
            let mut total = 0.0;
            for (slot, par) in team.iter().enumerate().rev() {
                if !par.disconnected
                    && let Some(p) = par.player.filter(|p| world.players.contains_key(p))
                    && !world.is_dead(p)
                {
                    let hp = world
                        .players
                        .get(&p)
                        .map_or(0.0, |pl| pl.core.cur_hp + pl.core.cur_cp);
                    if hp >= 0.5 {
                        total += hp;
                    }
                }
                world.update_player(index, team_index, slot);
            }
            total
        };
        let one_hp = team_hp(self, 0, &one);
        let two_hp = team_hp(self, 1, &two);

        if (two_hp == 0.0 && one_hp != 0.0) || (damage_one > damage_two && two_hp != 0.0 && one_hp != 0.0) {
            self.win_message(index, &one[0].name);
            self.team_loses(index, &two, &points_two, damage_two, &mut list2);
            self.team_wins(index, &one, win_points, damage_one, &mut list1);
            winside = 1;
            for slot in 0..one.len() {
                let player = self.olympiad_game(index).and_then(|g| g.team_one[slot].player);
                self.reward_participant(player, &reward);
            }
        } else if (one_hp == 0.0 && two_hp != 0.0)
            || (damage_two > damage_one && one_hp != 0.0 && two_hp != 0.0)
        {
            self.win_message(index, &two[0].name);
            self.team_loses(index, &one, &points_one, damage_one, &mut list1);
            self.team_wins(index, &two, win_points, damage_two, &mut list2);
            winside = 2;
            for slot in 0..two.len() {
                let player = self.olympiad_game(index).and_then(|g| g.team_two[slot].player);
                self.reward_participant(player, &reward);
            }
        } else {
            self.tie_message(index);
            for (par, list, damage) in one
                .iter()
                .map(|p| (p, 1, damage_one))
                .chain(two.iter().map(|p| (p, 2, damage_two)))
            {
                self.update_noble_stat(par.object_id, NobleStat::Drawn, 1);
                let points = (self.noble_points(par.object_id) / divider).min(max_points);
                self.remove_points_from_participant(index, par, points);
                let current = self.noble_points(par.object_id);
                let entry = self.result_entry(par, damage, current - points, -points);
                if list == 1 {
                    list1.push(entry);
                } else {
                    list2.push(entry);
                }
            }
            tie = true;
        }
        done(self);
        self.broadcast_result(index, tie, winside, list1, list2);
    }

    /// The losing team of `OlympiadGameTeams.validateWinner`: lost, the points go.
    fn team_loses(
        &mut self,
        index: usize,
        team: &[Participant],
        points: &[i32],
        damage: i32,
        list: &mut Vec<OlympiadResultEntry>,
    ) {
        for (i, par) in team.iter().enumerate() {
            self.update_noble_stat(par.object_id, NobleStat::Lost, 1);
            let value = points.get(i).copied().unwrap_or(0);
            self.remove_points_from_participant(index, par, value);
            let current = self.noble_points(par.object_id);
            list.push(self.result_entry(par, damage, current - value, -value));
        }
    }

    /// The winning team of `OlympiadGameTeams.validateWinner`: won, `min / MAX_TEAM_SIZE` points
    /// each.
    fn team_wins(
        &mut self,
        index: usize,
        team: &[Participant],
        points: i32,
        damage: i32,
        list: &mut Vec<OlympiadResultEntry>,
    ) {
        for par in team {
            self.update_noble_stat(par.object_id, NobleStat::Won, 1);
            self.add_points_to_participant(index, par, points);
            let current = self.noble_points(par.object_id);
            list.push(self.result_entry(par, damage, current + points, points));
        }
    }
}
