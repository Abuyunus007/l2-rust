//! Duels: the challenge, the countdown, the fight, the end and what a duelist's state changes in
//! the rest of the world (M6-DUEL-01).
// Java: managers/DuelManager.java
// Java: entity/actor/holders/player/Duel.java
// Java: entity/actor/enums/player/DuelResult.java
// Java: network/clientpackets/{RequestDuelStart,RequestDuelAnswerStart,RequestDuelSurrender}.java
// Java: network/serverpackets/{ExDuelAskStart,ExDuelReady,ExDuelStart,ExDuelEnd,ExDuelUpdateUserInfo}.java
// Java: entity/actor/Player.java (canDuel, getNoDuelReason, setInDuel, setDuelState,
//       setStartingDuel, setTeam, isInDuelWith)
//
// # State
// Java keeps a duelist's state on the `Player` (`_isInDuel`, `_duelId`, `_duelState`,
// `_startingDuel`, the `Creature` team). Here it is one [`PlayerDuel`] per player in
// `World::duel` (D-17: no change to `Player`), dropped when the player leaves the world
// ([`World::forget_duel`]). The duels themselves are `Duel`s in `DuelManager._duels`' place.
//
// # Tasks
// Java's three `Runnable`s (`ScheduleStartDuelTask`, `ScheduleDuelTask`, `ScheduleEndDuelTask`)
// are [`DuelTask`]s on the timer wheel (`TimerKind::Duel`). A task of a duel that is gone does
// nothing. The duel's end time and "remaining time" count in world milliseconds
// (`World::world_ms`), not `System.currentTimeMillis()`.
//
// # The party duel arena (M8-INST-01)
// Java moves a party duel to a copy of an Olympiad arena (`InstanceManager.createDynamicInstance`
// of one of the templates 147 to 150, the `OlympiadStadiumZone` spawns), opens its doors when the
// fight starts, destroys the instance when the fight is over (which sends everybody to the exit
// point or the nearest town) and puts every duelist back at the saved position when the duel
// ends (`teleportBack`, which keeps the instance id: the destroy already took them out).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use l2_net::Encode;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    ActionFailed, ExDuelAskStart, ExDuelEnd, ExDuelReady, ExDuelStart, ExDuelUpdateUserInfo, PlaySound,
    SocialAction, SystemMessage, SystemMessageParam,
};
use l2_util::ObjectId;
use tracing::warn;

use crate::effects::SkillFinishType;
use crate::timer::{TimerKind, TimerWheel};
use crate::world::World;
use crate::zones::ZoneId;

/// Java `Duel.PARTY_DUEL_DURATION` (seconds).
const PARTY_DUEL_DURATION: i64 = 300;
/// Java `Duel.PLAYER_DUEL_DURATION` (seconds).
const PLAYER_DUEL_DURATION: i64 = 120;
/// `new ScheduleStartDuelTask(this), 3000`.
const START_DELAY_MS: u64 = 3000;
/// The countdown and the watch task run every second.
const TASK_INTERVAL_MS: u64 = 1000;
/// "give players 20 seconds to complete teleport and get ready (its ought to be 30 on official..)"
const PARTY_TELEPORT_WAIT_MS: u64 = 20_000;
/// `ThreadPool.schedule(this, _duel.isPartyDuel() ? 20000 : 1)`: the 1 ms of a 1 vs 1 duel is
/// one tick here.
const PLAYER_TELEPORT_WAIT_MS: u64 = 1;
/// `ScheduleEndDuelTask` runs 5 seconds after the fight ended (the loser kneels meanwhile).
const END_DELAY_MS: u64 = 5000;
/// `RequestDuelStart`: "Players may not be too far apart."
const DUEL_REQUEST_RANGE: i32 = 250;
/// `Duel.checkEndDuelCondition`: "Are the players too far apart?"
const DUEL_MAX_DISTANCE: i32 = 1600;
/// `SocialAction(objectId, 7)`: the bow of the loser.
const KNEEL_ACTION: i32 = 7;
/// `Team.BLUE` / `Team.RED` ids.
const TEAM_NONE: u8 = 0;
const TEAM_BLUE: u8 = 1;
const TEAM_RED: u8 = 2;
/// The text of `DuelManager.addDuel` and `Duel.isDuelistInPvp`.
const ENGAGED_IN_PVP: &str = "The duel was canceled because a duelist engaged in PvP combat.";

/// Java `Duel.DUELSTATE_*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DuelState {
    /// `DUELSTATE_NODUEL`.
    #[default]
    NoDuel,
    /// `DUELSTATE_DUELLING`.
    Duelling,
    /// `DUELSTATE_DEAD`: defeated or surrendered, frozen until the duel ends.
    Dead,
    /// `DUELSTATE_WINNER`.
    Winner,
    /// `DUELSTATE_INTERRUPTED`: somebody outside the duel hit a duelist.
    Interrupted,
}

/// Java `DuelResult`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuelResult {
    Continue,
    Team1Win,
    Team2Win,
    Team1Surrender,
    Team2Surrender,
    Canceled,
    Timeout,
}

/// What a duel timer does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuelTask {
    /// `ScheduleStartDuelTask`: one step of the countdown.
    Start,
    /// `ScheduleDuelTask`: the once-a-second look at the end conditions.
    Watch,
    /// `ScheduleEndDuelTask(duel, result)`.
    End(DuelResult),
}

/// The payload of `TimerKind::Duel`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuelTimer {
    pub duel: i32,
    pub task: DuelTask,
}

/// The duel part of a `Player` and of the `Creature` team.
#[derive(Debug, Clone, Default)]
struct PlayerDuel {
    /// `_isInDuel`.
    in_duel: bool,
    /// `_duelId`: 0 = not in a duel.
    duel_id: i32,
    /// `_duelState`.
    state: DuelState,
    /// `_startingDuel`.
    starting: bool,
    /// `Creature._team` id.
    team: u8,
}

/// Java `Duel.PlayerCondition`: what a duelist had before the duel.
#[derive(Debug, Clone)]
struct PlayerCondition {
    hp: f64,
    mp: f64,
    cp: f64,
    /// `_debuffs`: skill ids to remove when the duel ends.
    debuffs: BTreeSet<i32>,
    /// `_x`, `_y`, `_z` of a party duelist (`_paDuel`): where `teleportBack` puts him.
    position: Option<(i32, i32, i32)>,
}

/// Java `Duel`.
#[derive(Debug)]
struct Duel {
    /// `_playerA`: the challenger. `None` after `onRemoveFromParty` (Java sets the field null).
    player_a: Option<ObjectId>,
    /// `_playerB`: the challenged.
    player_b: Option<ObjectId>,
    party_duel: bool,
    /// `_duelEndTime` in world ms.
    end_time_ms: i64,
    /// `_surrenderRequest`: 0 none, 1 team 1 gave up, 2 team 2 did.
    surrender_request: i32,
    /// `_countdown`.
    countdown: i32,
    /// `_finished`.
    finished: bool,
    /// `_playerConditions` (Java: a `ConcurrentHashMap`; the order is by object id here).
    conditions: BTreeMap<ObjectId, PlayerCondition>,
    /// `_duelInstanceId`: the arena instance of a party duel, 0 before `teleportPlayers`.
    instance_id: i32,
}

/// `DuelManager`'s tables and the duel state of the players.
#[derive(Debug, Default)]
pub(crate) struct DuelWorld {
    /// `_duels`.
    duels: BTreeMap<i32, Duel>,
    /// `_currentDuelId`.
    current_duel_id: i32,
    players: HashMap<ObjectId, PlayerDuel>,
}

fn text(value: &str) -> SystemMessageParam {
    SystemMessageParam::Text(value.to_owned())
}

fn pc_name(value: &str) -> SystemMessageParam {
    SystemMessageParam::PlayerName(value.to_owned())
}

impl World {
    // ------------------------------------------------------------------ player state

    /// Java `Player.isInDuel()`.
    pub fn is_in_duel(&self, id: ObjectId) -> bool {
        self.duel.players.get(&id).is_some_and(|p| p.in_duel)
    }

    /// Java `Player._startingDuel`.
    pub(crate) fn is_starting_duel(&self, id: ObjectId) -> bool {
        self.duel.players.get(&id).is_some_and(|p| p.starting)
    }

    /// Java `Player.getDuelId()`: 0 when not in a duel.
    pub fn duel_id_of(&self, id: ObjectId) -> i32 {
        self.duel.players.get(&id).map_or(0, |p| p.duel_id)
    }

    /// Java `Player.getDuelState()`.
    pub fn duel_state_of(&self, id: ObjectId) -> DuelState {
        self.duel.players.get(&id).map_or(DuelState::NoDuel, |p| p.state)
    }

    /// Java `Player.setDuelState(mode)`.
    pub fn set_duel_state(&mut self, id: ObjectId, state: DuelState) {
        if self.players.contains_key(&id) {
            self.duel.players.entry(id).or_default().state = state;
        }
    }

    /// Java `Creature.getTeam().getId()`: 0 none, 1 blue, 2 red.
    pub fn team_of(&self, id: ObjectId) -> u8 {
        self.duel.players.get(&id).map_or(TEAM_NONE, |p| p.team)
    }

    /// Java `Player.setTeam(team)`: the team, then `broadcastUserInfo()`.
    pub(crate) fn set_team(&mut self, id: ObjectId, team: u8) {
        if !self.players.contains_key(&id) {
            return;
        }
        self.duel.players.entry(id).or_default().team = team;
        self.broadcast_user_info(id);
        if let Some(summon) = self.summon_of(id) {
            self.broadcast_status_update(summon);
        }
    }

    /// Java `Player.setStartingDuel()`.
    fn set_starting_duel(&mut self, id: ObjectId) {
        if self.players.contains_key(&id) {
            self.duel.players.entry(id).or_default().starting = true;
        }
    }

    /// Java `Player.setInDuel(duelId)`: a duel id above 0 puts the player in it, anything else
    /// takes it out. `starting` ends either way.
    pub(crate) fn set_in_duel(&mut self, id: ObjectId, duel_id: i32) {
        if !self.players.contains_key(&id) {
            return;
        }
        if duel_id > 0 {
            let state = self.duel.players.entry(id).or_default();
            state.in_duel = true;
            state.state = DuelState::Duelling;
            state.duel_id = duel_id;
        } else {
            if self.duel_state_of(id) == DuelState::Dead {
                self.enable_all_skills(id);
                self.start_hp_mp_regeneration(id);
            }
            let state = self.duel.players.entry(id).or_default();
            state.in_duel = false;
            state.state = DuelState::NoDuel;
            state.duel_id = 0;
        }
        self.duel.players.entry(id).or_default().starting = false;
    }

    /// Java `Player.isInDuelWith(target)`: both in the same duel.
    pub fn is_in_duel_with(&self, id: ObjectId, target: ObjectId) -> bool {
        self.is_in_duel(id) && self.is_in_duel(target) && self.duel_id_of(id) == self.duel_id_of(target)
    }

    /// The player is gone (a new login is a new `Player` object).
    pub(crate) fn forget_duel(&mut self, id: ObjectId) {
        self.duel.players.remove(&id);
    }

    /// Java `Player.canDuel()`; the `Err` is `getNoDuelReason()`'s message id (its `C1` is the
    /// player).
    // NOTE(java-quirk): Java keeps the reason in a field that `getNoDuelReason` resets; every
    // caller reads it right after `canDuel()` returned false, so a plain `Result` is the same.
    pub fn can_duel(&self, id: ObjectId) -> Result<(), SystemMessageId> {
        use SystemMessageId::{
            C1CannotDuelBecauseC1IsAlreadyEngagedInADuel,
            C1CannotDuelBecauseC1IsCurrentlyEngagedInAPrivateStoreOrManufacture,
            C1CannotDuelBecauseC1IsCurrentlyEngagedInBattle, C1CannotDuelBecauseC1IsCurrentlyFishing,
            C1CannotDuelBecauseC1IsCurrentlyRidingABoatSteedOrStrider,
            C1CannotDuelBecauseC1IsInAChaoticState, C1CannotDuelBecauseC1SHpOrMpIsBelow50,
            C1CannotMakeAChallengeToADuelBecauseC1IsCurrentlyInADuelProhibitedAreaPeacefulZoneSevenSignsZoneNearWaterRestartProhibitedArea as InProhibitedArea,
            ThereIsNoOpponentToReceiveYourChallengeForADuel,
        };
        let Some(player) = self.players.get(&id) else {
            return Err(ThereIsNoOpponentToReceiveYourChallengeForADuel);
        };
        if self.is_in_combat(id) || self.is_jailed(id) {
            return Err(C1CannotDuelBecauseC1IsCurrentlyEngagedInBattle);
        }
        // `getMaxHp() / 2` is an int division.
        let (max_hp, max_mp) = self
            .stats(id)
            .map_or((0, 0), |view| (view.max_hp(), view.max_mp()));
        if self.is_dead(id)
            || self.is_alike_dead(id)
            || player.core.cur_hp < f64::from(max_hp / 2)
            || player.core.cur_mp < f64::from(max_mp / 2)
        {
            return Err(C1CannotDuelBecauseC1SHpOrMpIsBelow50);
        }
        let state = self.duel.players.get(&id);
        if state.is_some_and(|s| s.in_duel || s.starting) {
            return Err(C1CannotDuelBecauseC1IsAlreadyEngagedInADuel);
        }
        // M8: `_inOlympiadMode` (C1_CANNOT_DUEL_BECAUSE_C1_IS_PARTICIPATING_IN_THE_OLYMPIAD).
        if self.is_cursed_weapon_equipped(id) {
            return Err(C1CannotDuelBecauseC1IsInAChaoticState);
        }
        if self.is_in_store_mode(id) {
            return Err(C1CannotDuelBecauseC1IsCurrentlyEngagedInAPrivateStoreOrManufacture);
        }
        // `isMounted() || isInBoat()` (C1_CANNOT_DUEL_BECAUSE_C1_IS_CURRENTLY_RIDING_A_BOAT_STEED_OR_STRIDER).
        if self.is_mounted(id) {
            return Err(C1CannotDuelBecauseC1IsCurrentlyRidingABoatSteedOrStrider);
        }
        // Boats: not modelled.
        if self.is_fishing(id) {
            return Err(C1CannotDuelBecauseC1IsCurrentlyFishing);
        }
        if self.is_inside_zone(id, ZoneId::Pvp)
            || self.is_inside_zone(id, ZoneId::Peace)
            || self.is_inside_zone(id, ZoneId::Siege)
            || self.is_inside_zone(id, ZoneId::NoPvp)
        {
            return Err(InProhibitedArea);
        }
        Ok(())
    }

    /// `getNoDuelReason()` as the message the challenger gets.
    fn no_duel_reason_message(&self, target: ObjectId, reason: SystemMessageId) -> SystemMessage {
        SystemMessage::new(reason).with(pc_name(&self.creature_name(target)))
    }

    // ------------------------------------------------------------------ teams and messages

    /// The members of the party `player` is in (just `player` without one).
    fn duel_side(&self, player: Option<ObjectId>, party_duel: bool) -> Vec<ObjectId> {
        match player {
            Some(player) if party_duel => self.party_members_of(player),
            Some(player) => vec![player],
            None => Vec::new(),
        }
    }

    /// Who `broadcastToTeam1` (`player_a`) or `broadcastToTeam2` (`player_b`) sends to: the
    /// party of the duelist in a party duel, the duelist alone otherwise.
    fn duel_team(&self, duel: &Duel, first: bool) -> Vec<ObjectId> {
        let player = if first { duel.player_a } else { duel.player_b };
        self.duel_side(player, duel.party_duel)
    }

    fn send_to_all(&self, to: &[ObjectId], packet: &impl Encode) {
        let bytes = packet.to_bytes();
        for id in to {
            self.send_bytes(*id, bytes.clone());
        }
    }

    /// Java `Duel.broadcastToTeam1(packet)` and `broadcastToTeam2(packet)` for the duel `duel_id`.
    fn duel_broadcast_to_team(&self, duel_id: i32, first: bool, packet: &impl Encode) {
        let Some(duel) = self.duel.duels.get(&duel_id) else {
            return;
        };
        let team = self.duel_team(duel, first);
        self.send_to_all(&team, packet);
    }

    /// Java `DuelManager.broadcastToOppositTeam(player, packet)`.
    pub(crate) fn duel_broadcast_to_opposite_team(&self, player: ObjectId, packet: &impl Encode) {
        if !self.is_in_duel(player) {
            return;
        }
        let Some(duel) = self.duel.duels.get(&self.duel_id_of(player)) else {
            return;
        };
        let (Some(a), Some(b)) = (duel.player_a, duel.player_b) else {
            return;
        };
        let first = if a == player {
            false
        } else if b == player {
            true
        } else if duel.party_duel {
            if self.party_members_of(a).contains(&player) && self.is_in_party(a) {
                false
            } else if self.party_members_of(b).contains(&player) && self.is_in_party(b) {
                true
            } else {
                return;
            }
        } else {
            return;
        };
        let team = self.duel_team(duel, first);
        self.send_to_all(&team, packet);
    }

    /// `new ExDuelUpdateUserInfo(player)`.
    pub(crate) fn duel_update_user_info(&self, id: ObjectId) -> Option<ExDuelUpdateUserInfo> {
        let player = self.players.get(&id)?;
        let view = self.stats(id)?;
        Some(ExDuelUpdateUserInfo {
            name: player.data.name.clone(),
            object_id: id.0,
            class_id: player.data.class_id,
            level: player.core.level,
            current_hp: player.core.cur_hp as i32,
            max_hp: view.max_hp(),
            current_mp: player.core.cur_mp as i32,
            max_mp: view.max_mp(),
            current_cp: player.core.cur_cp as i32,
            max_cp: view.max_cp(),
        })
    }

    // ------------------------------------------------------------------ RequestDuelStart

    /// Java `RequestDuelStart.runImpl`.
    pub(crate) fn on_request_duel_start(&mut self, id: ObjectId, name: &str, party_duel: i32) {
        if !self.players.contains_key(&id) {
            return;
        }
        let Some(target) = self.player_by_name(name) else {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::ThereIsNoOpponentToReceiveYourChallengeForADuel),
            );
            return;
        };
        if id == target {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::ThereIsNoOpponentToReceiveYourChallengeForADuel),
            );
            return;
        }

        // Check if duel is possible.
        if self.can_duel(id).is_err() {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouAreUnableToRequestADuelAtThisTime),
            );
            return;
        }
        if let Err(reason) = self.can_duel(target) {
            self.send(id, &self.no_duel_reason_message(target, reason));
            return;
        }
        // Players may not be too far apart.
        let target_position = self.position_of(target).unwrap_or_default();
        if !self.inside_radius_2d(id, target_position, DUEL_REQUEST_RANGE) {
            let target_name = self.creature_name(target);
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::C1CannotReceiveADuelChallengeBecauseC1IsTooFarAway)
                    .with(text(&target_name)),
            );
            return;
        }

        let player_name = self.creature_name(id);
        if party_duel == 1 {
            self.request_party_duel(id, target, &player_name, party_duel);
        } else if !self.is_processing_request(target) {
            // 1vs1 duel.
            let target_name = self.creature_name(target);
            self.on_transaction_request(id, target);
            self.send(
                target,
                &ExDuelAskStart {
                    requestor_name: player_name.clone(),
                    party_duel,
                },
            );
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::C1HasBeenChallengedToADuel).with(text(&target_name)),
            );
            self.send(
                target,
                &SystemMessage::new(SystemMessageId::C1HasChallengedYouToADuel).with(text(&player_name)),
            );
        } else {
            let target_name = self.creature_name(target);
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::C1IsOnAnotherTaskPleaseTryAgainLater)
                    .with(text(&target_name)),
            );
        }
    }

    /// The party branch of `RequestDuelStart.runImpl`.
    fn request_party_duel(&mut self, id: ObjectId, target: ObjectId, player_name: &str, party_duel: i32) {
        // Player must be in a party & the party leader.
        if !self.is_in_party(id) || !self.is_party_leader(id) {
            self.send_message(
                id,
                "You have to be the leader of a party in order to request a party duel.",
            );
            return;
        }
        // Target must be in a party.
        if !self.is_in_party(target) {
            self.send(
                id,
                &SystemMessage::new(
                    SystemMessageId::SinceThePersonYouChallengedIsNotCurrentlyInAPartyTheyCannotDuelAgainstYourParty,
                ),
            );
            return;
        }
        // Target may not be of the same party.
        if self.is_in_party_with(id, target) {
            self.send_message(id, "This player is a member of your own party.");
            return;
        }

        // Check if every player is ready for a duel.
        for member in self.party_members_of(id) {
            if self.can_duel(member).is_err() {
                self.send_message(id, "Not all the members of your party are ready for a duel.");
                return;
            }
        }

        // The first member of the target's party is its leader ("snatch party leader of
        // targetChar's party").
        let target_members = self.party_members_of(target);
        let party_leader = target_members.first().copied();
        for member in &target_members {
            if self.can_duel(*member).is_err() {
                self.send(
                    id,
                    &SystemMessage::new(
                        SystemMessageId::TheOpposingPartyIsCurrentlyUnableToAcceptAChallengeToADuel,
                    ),
                );
                return;
            }
        }

        // Send request to targetChar's party leader.
        let Some(party_leader) = party_leader else {
            return;
        };
        let leader_name = self.creature_name(party_leader);
        if !self.is_processing_request(party_leader) {
            self.on_transaction_request(id, party_leader);
            self.send(
                party_leader,
                &ExDuelAskStart {
                    requestor_name: player_name.to_owned(),
                    party_duel,
                },
            );
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::C1SPartyHasBeenChallengedToADuel)
                    .with(text(&leader_name)),
            );
            // NOTE(java-quirk): "<challenger>'s party has challenged your party" goes to the player
            // who was named in the request, not to the party leader who got the question.
            self.send(
                target,
                &SystemMessage::new(SystemMessageId::C1SPartyHasChallengedYourPartyToADuel)
                    .with(text(player_name)),
            );
        } else {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::C1IsOnAnotherTaskPleaseTryAgainLater)
                    .with(text(&leader_name)),
            );
        }
    }

    // ------------------------------------------------------------------ RequestDuelAnswerStart

    /// Java `RequestDuelAnswerStart.runImpl`.
    pub(crate) fn on_request_duel_answer_start(&mut self, id: ObjectId, party_duel: i32, response: i32) {
        if !self.players.contains_key(&id) {
            return;
        }
        let Some(requestor) = self.active_requester(id).filter(|r| self.players.contains_key(r)) else {
            return;
        };
        let player_name = self.creature_name(id);
        let requestor_name = self.creature_name(requestor);

        if response == 1 {
            if self.is_in_duel(requestor) {
                self.send(
                    id,
                    &SystemMessage::new(SystemMessageId::C1CannotDuelBecauseC1IsAlreadyEngagedInADuel)
                        .with(text(&requestor_name)),
                );
                return;
            } else if self.is_in_duel(id) {
                self.send(
                    id,
                    &SystemMessage::new(SystemMessageId::YouAreUnableToRequestADuelAtThisTime),
                );
                return;
            }

            let (msg1, msg2) = if party_duel == 1 {
                (
                    SystemMessage::new(
                        SystemMessageId::YouHaveAcceptedC1SChallengeToAPartyDuelTheDuelWillBeginInAFewMoments,
                    )
                    .with(text(&requestor_name)),
                    SystemMessage::new(
                        SystemMessageId::S1HasAcceptedYourChallengeToDuelAgainstTheirPartyTheDuelWillBeginInAFewMoments,
                    )
                    .with(text(&player_name)),
                )
            } else {
                (
                    SystemMessage::new(
                        SystemMessageId::YouHaveAcceptedC1SChallengeADuelTheDuelWillBeginInAFewMoments,
                    )
                    .with(text(&requestor_name)),
                    SystemMessage::new(
                        SystemMessageId::C1HasAcceptedYourChallengeToADuelTheDuelWillBeginInAFewMoments,
                    )
                    .with(text(&player_name)),
                )
            };
            self.send(id, &msg1);
            self.send(requestor, &msg2);

            self.add_duel(requestor, id, party_duel);
        } else if response == -1 {
            self.send(
                requestor,
                &SystemMessage::new(SystemMessageId::C1IsSetToRefuseDuelRequestsAndCannotReceiveADuelRequest)
                    .with(pc_name(&player_name)),
            );
        } else if party_duel == 1 {
            self.send(
                requestor,
                &SystemMessage::new(SystemMessageId::TheOpposingPartyHasDeclinedYourChallengeToADuel),
            );
        } else {
            self.send(
                requestor,
                &SystemMessage::new(SystemMessageId::C1HasDeclinedYourChallengeToADuel)
                    .with(pc_name(&player_name)),
            );
        }

        self.set_active_requester(id, None);
        self.on_transaction_response(requestor);
    }

    // ------------------------------------------------------------------ DuelManager

    /// Java `DuelManager.addDuel(playerA, playerB, partyDuel)`.
    pub(crate) fn add_duel(&mut self, player_a: ObjectId, player_b: ObjectId, party_duel: i32) {
        if !self.players.contains_key(&player_a) || !self.players.contains_key(&player_b) {
            return;
        }
        // Return if a player has PvPFlag.
        if party_duel == 1 {
            // Java dereferences `getParty()` of both: a player who left their party since the
            // challenge makes it throw, and nothing starts.
            if !self.is_in_party(player_a) || !self.is_in_party(player_b) {
                return;
            }
            let members_a = self.party_members_of(player_a);
            let members_b = self.party_members_of(player_b);
            let in_pvp = members_a
                .iter()
                .chain(&members_b)
                .any(|member| self.pvp_flag_of(*member) != 0);
            // A player has PvP flag.
            if in_pvp {
                for member in members_a.iter().chain(&members_b) {
                    self.send_message(*member, ENGAGED_IN_PVP);
                }
                return;
            }
        } else if self.pvp_flag_of(player_a) != 0 || self.pvp_flag_of(player_b) != 0 {
            self.send_message(player_a, ENGAGED_IN_PVP);
            self.send_message(player_b, ENGAGED_IN_PVP);
            return;
        }

        self.duel.current_duel_id += 1;
        let duel_id = self.duel.current_duel_id;
        self.new_duel(duel_id, player_a, player_b, party_duel == 1);
    }

    /// Java `new Duel(playerA, playerB, partyDuel, duelId)`.
    fn new_duel(&mut self, duel_id: i32, player_a: ObjectId, player_b: ObjectId, party_duel: bool) {
        if party_duel {
            for member in self
                .party_members_of(player_a)
                .into_iter()
                .chain(self.party_members_of(player_b))
            {
                self.set_starting_duel(member);
            }
        } else {
            self.set_starting_duel(player_a);
            self.set_starting_duel(player_b);
        }

        let seconds = if party_duel {
            PARTY_DUEL_DURATION
        } else {
            PLAYER_DUEL_DURATION
        };
        let duel = Duel {
            player_a: Some(player_a),
            player_b: Some(player_b),
            party_duel,
            end_time_ms: self.world_ms() + seconds * 1000,
            surrender_request: 0,
            countdown: 5,
            finished: false,
            conditions: BTreeMap::new(),
            instance_id: 0,
        };
        self.duel.duels.insert(duel_id, duel);

        if party_duel {
            // Inform players that they will be ported shortly.
            let message = SystemMessage::new(
                SystemMessageId::InAMomentYouWillBeTransportedToTheSiteWhereTheDuelWillTakePlace,
            );
            self.duel_broadcast_to_team(duel_id, true, &message);
            self.duel_broadcast_to_team(duel_id, false, &message);
        }

        // Schedule duel start.
        self.schedule_duel_task(duel_id, DuelTask::Start, START_DELAY_MS);
    }

    fn schedule_duel_task(&mut self, duel: i32, task: DuelTask, delay_ms: u64) {
        let due = self.tick + TimerWheel::ms_to_ticks(delay_ms);
        self.timers
            .schedule_at(due, TimerKind::Duel(DuelTimer { duel, task }));
    }

    /// How many duels are going on (for tests and tools).
    pub fn duel_count(&self) -> usize {
        self.duel.duels.len()
    }

    /// Java `DuelManager.doSurrender(player)`.
    pub(crate) fn do_duel_surrender(&mut self, player: ObjectId) {
        if !self.players.contains_key(&player) || !self.is_in_duel(player) {
            return;
        }
        let duel_id = self.duel_id_of(player);
        self.duel_surrender(duel_id, player);
    }

    /// Java `DuelManager.onPlayerDefeat(player)`: the player's HP ran out.
    pub(crate) fn duel_on_player_defeat(&mut self, player: ObjectId) {
        if !self.players.contains_key(&player) || !self.is_in_duel(player) {
            return;
        }
        let duel_id = self.duel_id_of(player);
        if self.duel.duels.contains_key(&duel_id) {
            self.duel_player_defeated(duel_id, player);
        }
    }

    /// Java `DuelManager.getDuel(duelId)`'s `getPlayerA()`, `getPlayerB()` and `isPartyDuel()`
    /// (`None` for a duel that does not exist; a player is `None` once `onRemoveFromParty` nulled
    /// it). For the cubics' target choice.
    pub(crate) fn duel_parts(&self, duel_id: i32) -> Option<(Option<ObjectId>, Option<ObjectId>, bool)> {
        self.duel
            .duels
            .get(&duel_id)
            .map(|duel| (duel.player_a, duel.player_b, duel.party_duel))
    }

    /// Java `DuelManager.onBuff(player, buff)`: a debuff cast on a duelist that is removed when
    /// the duel ends. (Java's only caller is the cubic code.)
    pub fn duel_on_buff(&mut self, player: ObjectId, skill_id: i32) {
        if !self.players.contains_key(&player) || !self.is_in_duel(player) {
            return;
        }
        let duel_id = self.duel_id_of(player);
        if let Some(condition) = self
            .duel
            .duels
            .get_mut(&duel_id)
            .and_then(|duel| duel.conditions.get_mut(&player))
        {
            condition.debuffs.insert(skill_id);
        }
    }

    /// Java `DuelManager.onRemoveFromParty(player)`: the player left the party in the middle of a
    /// party duel.
    pub(crate) fn duel_on_remove_from_party(&mut self, player: ObjectId) {
        if !self.players.contains_key(&player) || !self.is_in_duel(player) {
            return;
        }
        let duel_id = self.duel_id_of(player);
        let Some(duel) = self.duel.duels.get_mut(&duel_id) else {
            return;
        };
        // If it isn't a party duel ignore this.
        if !duel.party_duel {
            return;
        }

        // This player is leaving his party during party duel. If he's either playerA or playerB
        // cancel the duel and port the players back.
        if duel.player_a == Some(player) || duel.player_b == Some(player) {
            let players: Vec<(ObjectId, PlayerCondition)> =
                duel.conditions.iter().map(|(id, c)| (*id, c.clone())).collect();
            duel.player_a = None;
            duel.player_b = None;
            // NOTE(java-quirk): only `teleportBack()` and `setInDuel(0)`: the team colour
            // (BLUE/RED) and the opponents' `DUELSTATE` stay, as nothing calls
            // `restorePlayerConditions` for a duel without duelists.
            for (id, condition) in players {
                self.duel_teleport_back(id, &condition);
                self.set_in_duel(id, 0);
            }
        } else {
            // Teleport the player back & delete his PlayerCondition record.
            if let Some(condition) = duel.conditions.remove(&player) {
                self.duel_teleport_back(player, &condition);
            }
            self.set_in_duel(player, 0);
        }
    }

    // ------------------------------------------------------------------ the tasks

    /// A duel timer fired.
    pub(crate) fn on_duel_timer(&mut self, timer: DuelTimer) {
        if !self.duel.duels.contains_key(&timer.duel) {
            return;
        }
        match timer.task {
            DuelTask::Start => self.duel_start_task(timer.duel),
            DuelTask::Watch => self.duel_watch_task(timer.duel),
            DuelTask::End(result) => self.end_duel(timer.duel, result),
        }
    }

    /// `ScheduleStartDuelTask.run`.
    fn duel_start_task(&mut self, duel_id: i32) {
        // start/continue countdown
        let count = self.duel_countdown(duel_id);
        if count == 4 {
            // Save player conditions before teleporting players.
            self.save_player_conditions(duel_id);
            // `teleportPlayers()`: a party duel is moved to an Olympiad arena instance. Java's
            // exceptions there end the task: the duel never starts.
            if !self.duel_teleport_players(duel_id) {
                return;
            }
            let party = self.duel.duels.get(&duel_id).is_some_and(|d| d.party_duel);
            // give players 20 seconds to complete teleport and get ready (its ought to be 30 on
            // official..)
            let wait = if party {
                PARTY_TELEPORT_WAIT_MS
            } else {
                PLAYER_TELEPORT_WAIT_MS
            };
            self.schedule_duel_task(duel_id, DuelTask::Start, wait);
        } else if count > 0 {
            // Duel not started yet - continue countdown.
            self.schedule_duel_task(duel_id, DuelTask::Start, TASK_INTERVAL_MS);
        } else {
            self.start_duel(duel_id);
        }
    }

    /// Java `Duel.countdown()`: the count after this step, and the message to the duelists.
    fn duel_countdown(&mut self, duel_id: i32) -> i32 {
        let Some(duel) = self.duel.duels.get_mut(&duel_id) else {
            return 0;
        };
        duel.countdown -= 1;
        let count = duel.countdown;
        if count > 3 {
            return count;
        }
        // Broadcast countdown to duelists.
        let message = if count > 0 {
            SystemMessage::new(SystemMessageId::TheDuelWillBeginInS1SecondS)
                .with(SystemMessageParam::IntNumber(count))
        } else {
            SystemMessage::new(SystemMessageId::LetTheDuelBegin)
        };
        self.duel_broadcast_to_team(duel_id, true, &message);
        self.duel_broadcast_to_team(duel_id, false, &message);
        count
    }

    /// Java `Duel.savePlayerConditions()`: hp, mp and cp, and the position for a party duel
    /// (`teleportBack`).
    fn save_player_conditions(&mut self, duel_id: i32) {
        let Some(duel) = self.duel.duels.get(&duel_id) else {
            return;
        };
        let party_duel = duel.party_duel;
        let mut members = self.duel_side(duel.player_a, duel.party_duel);
        members.extend(self.duel_side(duel.player_b, duel.party_duel));
        let conditions: Vec<(ObjectId, PlayerCondition)> = members
            .into_iter()
            .filter_map(|id| {
                let core = &self.players.get(&id)?.core;
                Some((
                    id,
                    PlayerCondition {
                        hp: core.cur_hp,
                        mp: core.cur_mp,
                        cp: core.cur_cp,
                        debuffs: BTreeSet::new(),
                        position: party_duel.then_some((core.x, core.y, core.z)),
                    },
                ))
            })
            .collect();
        if let Some(duel) = self.duel.duels.get_mut(&duel_id) {
            duel.conditions.extend(conditions);
        }
    }

    /// Java `Duel.startDuel()`.
    fn start_duel(&mut self, duel_id: i32) {
        let Some(duel) = self.duel.duels.get(&duel_id) else {
            return;
        };
        let party_duel = duel.party_duel;
        let (Some(a), Some(b)) = (duel.player_a, duel.player_b) else {
            self.duel.duels.remove(&duel_id);
            return;
        };
        // NOTE(java-quirk): this early return leaves `_startingDuel` set on the players (only
        // `setInDuel` clears it), so they cannot duel until they are in one.
        if self.is_in_duel(a) || self.is_in_duel(b) {
            self.duel.duels.remove(&duel_id);
            return;
        }

        if party_duel {
            // Set duel state and team.
            let members_a = self.party_members_of(a);
            let members_b = self.party_members_of(b);
            for temp in members_a {
                self.cancel_active_trade(temp);
                self.set_in_duel(temp, duel_id);
                self.set_team(temp, TEAM_BLUE);
                self.broadcast_user_info(temp);
                if let Some(info) = self.duel_update_user_info(temp) {
                    self.duel_broadcast_to_team(duel_id, false, &info);
                }
            }
            for temp in members_b {
                self.cancel_active_trade(temp);
                self.set_in_duel(temp, duel_id);
                self.set_team(temp, TEAM_RED);
                self.broadcast_user_info(temp);
                if let Some(info) = self.duel_update_user_info(temp) {
                    self.duel_broadcast_to_team(duel_id, true, &info);
                }
            }

            // Send duel packets
            let ready = ExDuelReady { party_duel: true };
            let start = ExDuelStart { party_duel: true };
            self.duel_broadcast_to_team(duel_id, true, &ready);
            self.duel_broadcast_to_team(duel_id, false, &ready);
            self.duel_broadcast_to_team(duel_id, true, &start);
            self.duel_broadcast_to_team(duel_id, false, &start);

            // Open arena doors (`InstanceManager.getInstance(getDueldInstanceId()).getDoors()`).
            let instance_id = self.duel.duels.get(&duel_id).map_or(0, |d| d.instance_id);
            let doors: Vec<ObjectId> = self
                .instance(instance_id)
                .map(|i| i.doors.values().copied().collect())
                .unwrap_or_default();
            for door in doors {
                if self.door(door).is_some_and(|d| !d.open) {
                    self.open_door(door);
                }
            }
        } else {
            // set isInDuel() state
            self.set_in_duel(a, duel_id);
            self.set_team(a, TEAM_BLUE);
            self.set_in_duel(b, duel_id);
            self.set_team(b, TEAM_RED);

            // Send duel packets
            let ready = ExDuelReady { party_duel: false };
            let start = ExDuelStart { party_duel: false };
            self.duel_broadcast_to_team(duel_id, true, &ready);
            self.duel_broadcast_to_team(duel_id, false, &ready);
            self.duel_broadcast_to_team(duel_id, true, &start);
            self.duel_broadcast_to_team(duel_id, false, &start);

            if let Some(info) = self.duel_update_user_info(b) {
                self.duel_broadcast_to_team(duel_id, true, &info);
            }
            if let Some(info) = self.duel_update_user_info(a) {
                self.duel_broadcast_to_team(duel_id, false, &info);
            }
            self.broadcast_user_info(a);
            self.broadcast_user_info(b);
        }

        // play sound
        let sound = PlaySound::with_unknown1(1, "B04_S01");
        self.duel_broadcast_to_team(duel_id, true, &sound);
        self.duel_broadcast_to_team(duel_id, false, &sound);

        // start duelling task
        self.schedule_duel_task(duel_id, DuelTask::Watch, TASK_INTERVAL_MS);
    }

    /// `ScheduleDuelTask.run`.
    fn duel_watch_task(&mut self, duel_id: i32) {
        match self.check_end_duel_condition(duel_id) {
            DuelResult::Canceled => {
                // do not schedule duel end if it was interrupted
                self.set_duel_finished(duel_id);
                self.end_duel(duel_id, DuelResult::Canceled);
            }
            DuelResult::Continue => {
                self.schedule_duel_task(duel_id, DuelTask::Watch, TASK_INTERVAL_MS);
            }
            _ => {
                self.set_duel_finished(duel_id);
                self.play_kneel_animation(duel_id);
                // `new ScheduleEndDuelTask(_duel, _duel.checkEndDuelCondition())`: the conditions
                // are read once more.
                let result = self.check_end_duel_condition(duel_id);
                self.schedule_duel_task(duel_id, DuelTask::End(result), END_DELAY_MS);
                // `InstanceManager.destroyInstance(getDueldInstanceId())`: the arena goes (0, the
                // instance of a 1 vs 1 duel, is never destroyed).
                let instance_id = self.duel.duels.get(&duel_id).map_or(0, |d| d.instance_id);
                self.destroy_instance(instance_id);
            }
        }
    }

    fn set_duel_finished(&mut self, duel_id: i32) {
        if let Some(duel) = self.duel.duels.get_mut(&duel_id) {
            duel.finished = true;
        }
    }

    /// Java `Duel.checkEndDuelCondition()`: did a situation occur in which the duel has to end?
    fn check_end_duel_condition(&mut self, duel_id: i32) -> DuelResult {
        let Some(duel) = self.duel.duels.get(&duel_id) else {
            return DuelResult::Canceled;
        };
        // one of the players might leave during duel
        let (Some(a), Some(b)) = (duel.player_a, duel.player_b) else {
            return DuelResult::Canceled;
        };
        if !self.players.contains_key(&a) || !self.players.contains_key(&b) {
            return DuelResult::Canceled;
        }

        // got a duel surrender request?
        if duel.surrender_request != 0 {
            return if duel.surrender_request == 1 {
                DuelResult::Team1Surrender
            } else {
                DuelResult::Team2Surrender
            };
        }
        // duel timed out
        if duel.end_time_ms - self.world_ms() <= 0 {
            return DuelResult::Timeout;
        }
        let party_duel = duel.party_duel;
        // Has a player been declared winner yet?
        if self.duel_state_of(a) == DuelState::Winner {
            // If there is a Winner already there should be no more fighting going on.
            self.duel_stop_fighting(duel_id);
            return DuelResult::Team1Win;
        }
        if self.duel_state_of(b) == DuelState::Winner {
            self.duel_stop_fighting(duel_id);
            return DuelResult::Team2Win;
        }

        // More end duel conditions for 1on1 duels.
        if !party_duel {
            // Duel was interrupted e.g.: player was attacked by mobs / other players.
            if self.duel_state_of(a) == DuelState::Interrupted
                || self.duel_state_of(b) == DuelState::Interrupted
            {
                return DuelResult::Canceled;
            }
            // Are the players too far apart?
            let b_position = self.position_of(b).unwrap_or_default();
            if !self.inside_radius_2d(a, b_position, DUEL_MAX_DISTANCE) {
                return DuelResult::Canceled;
            }
            // Did one of the players engage in PvP combat?
            if self.is_duelist_in_pvp(duel_id, true) {
                return DuelResult::Canceled;
            }
            // Is one of the players in a Siege, Peace or PvP zone?
            for player in [a, b] {
                if self.is_inside_zone(player, ZoneId::Peace)
                    || self.is_inside_zone(player, ZoneId::NoPvp)
                    || self.is_inside_zone(player, ZoneId::Siege)
                    || self.is_inside_zone(player, ZoneId::Pvp)
                {
                    return DuelResult::Canceled;
                }
            }
        }
        DuelResult::Continue
    }

    /// Java `Duel.isDuelistInPvp(sendMessage)`: only for 1 vs 1 duels.
    fn is_duelist_in_pvp(&self, duel_id: i32, send_message: bool) -> bool {
        let Some(duel) = self.duel.duels.get(&duel_id) else {
            return false;
        };
        // Party duels take place in arenas - should be no other players there.
        if duel.party_duel {
            return false;
        }
        let (Some(a), Some(b)) = (duel.player_a, duel.player_b) else {
            return false;
        };
        if self.pvp_flag_of(a) != 0 || self.pvp_flag_of(b) != 0 {
            if send_message {
                self.send_message(a, ENGAGED_IN_PVP);
                self.send_message(b, ENGAGED_IN_PVP);
            }
            return true;
        }
        false
    }

    /// Java `Duel.stopFighting()`: "Stops all players from attacking. Used for duel timeout /
    /// interrupt."
    fn duel_stop_fighting(&mut self, duel_id: i32) {
        let Some(duel) = self.duel.duels.get(&duel_id) else {
            return;
        };
        let mut members = self.duel_side(duel.player_a, duel.party_duel);
        members.extend(self.duel_side(duel.player_b, duel.party_duel));
        for temp in members {
            self.abort_cast(temp);
            self.set_intention_active(temp);
            self.set_target(temp, None);
            self.send(temp, &ActionFailed);
            if let Some(summon) = self.summon_of(temp).filter(|s| !self.is_dead(*s)) {
                self.abort_cast(summon);
                self.set_intention_active(summon);
                self.set_creature_target(summon, None);
                self.send(summon, &ActionFailed);
            }
        }
    }

    /// Java `Duel.getLooser()`.
    fn duel_looser(&self, duel_id: i32) -> Option<ObjectId> {
        let duel = self.duel.duels.get(&duel_id)?;
        let (a, b) = (duel.player_a?, duel.player_b?);
        if !duel.finished {
            return None;
        }
        if self.duel_state_of(a) == DuelState::Winner {
            Some(b)
        } else if self.duel_state_of(b) == DuelState::Winner {
            Some(a)
        } else {
            None
        }
    }

    /// Java `Duel.playKneelAnimation()`: "Playback the bow animation for all loosers".
    fn play_kneel_animation(&mut self, duel_id: i32) {
        let Some(looser) = self.duel_looser(duel_id) else {
            return;
        };
        let party = self.duel.duels.get(&duel_id).is_some_and(|d| d.party_duel);
        let members = if party && self.is_in_party(looser) {
            self.party_members_of(looser)
        } else {
            vec![looser]
        };
        for temp in members {
            self.broadcast_to_self_and_visible(
                temp,
                &SocialAction {
                    object_id: temp.0,
                    action_id: KNEEL_ACTION,
                },
            );
        }
    }

    /// Java `Duel.endDuel(result)`: "The duel has reached a state in which it can no longer
    /// continue".
    fn end_duel(&mut self, duel_id: i32, result: DuelResult) {
        let Some(duel) = self.duel.duels.get(&duel_id) else {
            return;
        };
        let party_duel = duel.party_duel;
        let (Some(a), Some(b)) = (duel.player_a, duel.player_b) else {
            // clean up
            self.duel.duels.remove(&duel_id);
            return;
        };

        // inform players of the result
        let won = |winner: &str| {
            let id = if party_duel {
                SystemMessageId::C1SPartyHasWonTheDuel
            } else {
                SystemMessageId::C1HasWonTheDuel
            };
            SystemMessage::new(id).with(text(winner))
        };
        let tie = SystemMessage::new(SystemMessageId::TheDuelHasEndedInATie);
        let message = match result {
            DuelResult::Team1Win | DuelResult::Team2Surrender => {
                self.restore_player_conditions(duel_id, false);
                Some(won(&self.creature_name(a)))
            }
            DuelResult::Team1Surrender | DuelResult::Team2Win => {
                self.restore_player_conditions(duel_id, false);
                Some(won(&self.creature_name(b)))
            }
            DuelResult::Canceled => {
                self.duel_stop_fighting(duel_id);
                // Don't restore hp, mp, cp.
                self.restore_player_conditions(duel_id, true);
                // TODO(java): "is there no other message for a canceled duel?"
                Some(tie)
            }
            DuelResult::Timeout => {
                self.duel_stop_fighting(duel_id);
                // hp,mp,cp seem to be restored in a timeout too...
                self.restore_player_conditions(duel_id, false);
                Some(tie)
            }
            // Java's `switch` has no case for it.
            DuelResult::Continue => None,
        };
        if let Some(message) = message {
            self.duel_broadcast_to_team(duel_id, true, &message);
            self.duel_broadcast_to_team(duel_id, false, &message);
        }

        // Send end duel packet.
        let end = ExDuelEnd { party_duel };
        self.duel_broadcast_to_team(duel_id, true, &end);
        self.duel_broadcast_to_team(duel_id, false, &end);

        // clean up
        self.duel.duels.remove(&duel_id);
    }

    /// Java `Duel.restorePlayerConditions(abnormalDuelEnd)`.
    fn restore_player_conditions(&mut self, duel_id: i32, abnormal_duel_end: bool) {
        let Some(duel) = self.duel.duels.get(&duel_id) else {
            return;
        };
        let mut members = self.duel_side(duel.player_a, duel.party_duel);
        members.extend(self.duel_side(duel.player_b, duel.party_duel));
        let conditions: Vec<(ObjectId, PlayerCondition)> =
            duel.conditions.iter().map(|(id, c)| (*id, c.clone())).collect();

        // update isInDuel() state for all players
        for temp in members {
            self.set_in_duel(temp, 0);
            self.set_team(temp, TEAM_NONE);
            self.broadcast_user_info(temp);
        }

        // if it is an abnormal DuelEnd do not restore hp, mp, cp
        if abnormal_duel_end {
            return;
        }

        // restore player conditions
        for (id, condition) in conditions {
            self.restore_condition(id, &condition);
        }
    }

    /// Java `PlayerCondition.restoreCondition()`.
    fn restore_condition(&mut self, id: ObjectId, condition: &PlayerCondition) {
        if !self.players.contains_key(&id) {
            return;
        }
        self.set_current_hp(id, condition.hp, true);
        self.set_current_mp(id, condition.mp, true);
        self.set_current_cp(id, condition.cp, true);

        // `if (_paDuel) teleportBack()`.
        self.duel_teleport_back(id, condition);

        // Debuff removal
        for skill_id in &condition.debuffs {
            self.stop_skill_effects(id, SkillFinishType::Removed, *skill_id);
        }
    }

    /// Java `PlayerCondition.teleportBack()`: a party duelist goes back to where he stood
    /// (`teleToLocation(x, y, z)`).
    // NOTE(java-quirk): this overload keeps the instance id; a player who leaves the party during
    // the fight is put at his old position inside the arena instance.
    fn duel_teleport_back(&mut self, id: ObjectId, condition: &PlayerCondition) {
        if let Some((x, y, z)) = condition.position
            && self.players.contains_key(&id)
        {
            self.tele_to_location(id, crate::teleport::TeleportTarget::at(x, y, z));
        }
    }

    /// Java `Duel.teleportPlayers()`: a party duel gets a new instance of a random Olympiad arena
    /// (templates 147 to 150), the Olympiad buffers of the arena go, and each party is put on a
    /// spawn of its half of the stadium's spawn list. `false` where Java throws (the task that
    /// called it ends).
    fn duel_teleport_players(&mut self, duel_id: i32) -> bool {
        let Some((party_duel, player_a, player_b)) = self
            .duel
            .duels
            .get(&duel_id)
            .map(|d| (d.party_duel, d.player_a, d.player_b))
        else {
            return true;
        };
        if !party_duel {
            return true;
        }
        let template = self.rng().get(147, 150);
        let instance_id = self.create_dynamic_instance(template);
        if let Some(duel) = self.duel.duels.get_mut(&duel_id) {
            duel.instance_id = instance_id;
        }
        // `getZone(getNpcs().stream().findFirst().get(), OlympiadStadiumZone.class)`.
        let npcs: Vec<ObjectId> = self
            .instance(instance_id)
            .map(|i| i.npcs.keys().copied().collect())
            .unwrap_or_default();
        let Some(first) = npcs.first().copied() else {
            warn!(
                "Duel {duel_id}: arena instance {instance_id} has no NPC (java.util.NoSuchElementException)."
            );
            return false;
        };
        let spawns = self.position_of(first).and_then(|(x, y, z)| {
            let zones = self.zoning.zones.as_ref()?;
            zones
                .zone_at(x, y, z, "OlympiadStadiumZone")
                .map(|zone| zone.spawns.spawns.clone())
        });
        let Some(spawns) = spawns else {
            warn!("Duel {duel_id}: Unable to find a party duel arena!");
            return false;
        };

        // Remove Olympiad buffers.
        for buffer in npcs {
            let is_manager = self
                .npcs
                .get(&buffer)
                .and_then(|n| self.npc_world.template(n.npc_id))
                .is_some_and(|t| t.npc_type == "OlympiadManager");
            if is_manager && self.objects.get(&buffer).is_some_and(|e| e.spawned) {
                // `buffer.decayMe()`: out of view; the NPC stays in the instance (and in the
                // NPC table) until the instance goes.
                self.decay_keep_entry(buffer);
            }
        }

        let len = i32::try_from(spawns.len()).unwrap_or(i32::MAX);
        let first_index = self.rng().below(len / 2);
        let Some(spawn1) = usize::try_from(first_index)
            .ok()
            .and_then(|i| spawns.get(i))
            .copied()
        else {
            warn!("Duel {duel_id}: the arena has no spawn (java.lang.IndexOutOfBoundsException).");
            return false;
        };
        for temp in self.duel_side(player_a, true) {
            self.tele_to_location(
                temp,
                crate::teleport::TeleportTarget::at(spawn1.x, spawn1.y, spawn1.z).with_instance(instance_id),
            );
        }
        // NOTE(java-quirk): `Rnd.get(size / 2, size)` includes `size`: one roll in `size / 2 + 1`
        // is past the end of the list, Java throws there and the challenged party is never moved
        // (nor does the duel start).
        let second_index = self.rng().get(len / 2, len);
        let Some(spawn2) = usize::try_from(second_index)
            .ok()
            .and_then(|i| spawns.get(i))
            .copied()
        else {
            warn!(
                "Duel {duel_id}: java.lang.IndexOutOfBoundsException: Index {second_index} out of bounds for length {len}"
            );
            return false;
        };
        for temp in self.duel_side(player_b, true) {
            self.tele_to_location(
                temp,
                crate::teleport::TeleportTarget::at(spawn2.x, spawn2.y, spawn2.z).with_instance(instance_id),
            );
        }
        true
    }

    /// Java `Duel.doSurrender(player)`: "Register a surrender request".
    fn duel_surrender(&mut self, duel_id: i32, player: ObjectId) {
        let Some(duel) = self.duel.duels.get(&duel_id) else {
            return;
        };
        // Already received a surrender request.
        if duel.surrender_request != 0 {
            return;
        }
        let party_duel = duel.party_duel;
        let (a, b) = (duel.player_a, duel.player_b);
        // `stopFighting()` reads both duelists: a duel without them throws in Java.
        let (Some(a), Some(b)) = (a, b) else {
            return;
        };

        // stop the fight
        self.duel_stop_fighting(duel_id);

        // TODO(java): "Can every party member cancel a party duel? or only the party leaders?"
        let (request, losers, winners) = if party_duel {
            let members_a = self.party_members_of(a);
            let members_b = self.party_members_of(b);
            if members_a.contains(&player) {
                (1, members_a, members_b)
            } else if members_b.contains(&player) {
                (2, members_b, members_a)
            } else {
                return;
            }
        } else if player == a {
            (1, vec![a], vec![b])
        } else if player == b {
            (2, vec![b], vec![a])
        } else {
            return;
        };
        if let Some(duel) = self.duel.duels.get_mut(&duel_id) {
            duel.surrender_request = request;
        }
        for temp in losers {
            self.set_duel_state(temp, DuelState::Dead);
        }
        for temp in winners {
            self.set_duel_state(temp, DuelState::Winner);
        }
    }

    /// Java `Duel.onPlayerDefeat(player)`: "This function is called whenever a player was defeated
    /// in a duel".
    fn duel_player_defeated(&mut self, duel_id: i32, player: ObjectId) {
        // Set player as defeated.
        self.set_duel_state(player, DuelState::Dead);

        let Some(duel) = self.duel.duels.get(&duel_id) else {
            return;
        };
        // Java dereferences both duelists below: a duel without them throws.
        let (Some(a), Some(b)) = (duel.player_a, duel.player_b) else {
            return;
        };
        if duel.party_duel {
            let team_defeated = !self
                .party_members_of(player)
                .into_iter()
                .any(|temp| self.duel_state_of(temp) == DuelState::Duelling);
            if team_defeated {
                let winner = if self.party_members_of(a).contains(&player) {
                    b
                } else {
                    a
                };
                for temp in self.party_members_of(winner) {
                    self.set_duel_state(temp, DuelState::Winner);
                }
            }
        } else {
            if player != a && player != b {
                warn!("Error in onPlayerDefeat(): player is not part of this 1vs1 duel");
            }
            if a == player {
                self.set_duel_state(b, DuelState::Winner);
            } else {
                self.set_duel_state(a, DuelState::Winner);
            }
        }
    }

    // ------------------------------------------------------------------ damage (status.rs)

    /// The duel part of `PlayerStatus.reduceHp` for a hit by `attacker`: `true` when the damage is
    /// ignored (the duelist is already beaten or has won).
    pub(crate) fn duel_ignores_damage(&mut self, player: ObjectId, attacker: ObjectId) -> bool {
        // `attacker.asPlayer()`: a summon is its owner.
        let Some(attacker_player) = self.acting_player(attacker) else {
            return false;
        };
        if !self.is_in_duel(player) {
            return false;
        }
        match self.duel_state_of(player) {
            DuelState::Dead | DuelState::Winner => return true,
            _ => {}
        }
        // Cancel duel if player got hit by another player, that is not part of the duel.
        if self.duel_id_of(attacker_player) != self.duel_id_of(player) {
            self.set_duel_state(player, DuelState::Interrupted);
        }
        false
    }

    /// The duel part of `PlayerStatus.reduceHp` when the damage would kill the player: a duelist
    /// is beaten, not killed. `true` when it applies and the HP stays at 1.
    pub(crate) fn duel_defeats_instead_of_dying(
        &mut self,
        player: ObjectId,
        attacker: Option<ObjectId>,
    ) -> bool {
        if !self.is_in_duel(player) {
            return false;
        }
        self.disable_all_skills(player);
        self.stop_hp_mp_regeneration(player);
        if let Some(attacker) = attacker {
            self.set_intention_active(attacker);
            self.send(attacker, &ActionFailed);
            self.set_target(attacker, None);
            self.abort_attack(attacker);
        }
        // let the DuelManager know of his defeat
        self.duel_on_player_defeat(player);
        true
    }

    /// The duel part of `NpcStatus.reduceHp` and `SummonStatus.reduceHp` for a hit on `victim`.
    pub(crate) fn duel_interrupt_on_hit(&mut self, victim: ObjectId, attacker: ObjectId) {
        if self.is_summon(victim) {
            self.duel_interrupt_on_summon_hit(victim, attacker);
        } else {
            self.duel_interrupt_on_npc_hit(attacker);
        }
    }

    /// The duel part of `SummonStatus.reduceHp`: whoever hits a summon (its player, for a summon
    /// attacker) leaves his duel unless the summon's owner is in the same one.
    fn duel_interrupt_on_summon_hit(&mut self, summon: ObjectId, attacker: ObjectId) {
        let Some(attacker_player) = self.acting_player(attacker) else {
            return;
        };
        let owner_duel = self.summon_owner(summon).map(|owner| self.duel_id_of(owner));
        if owner_duel != Some(self.duel_id_of(attacker_player)) {
            self.set_duel_state(attacker_player, DuelState::Interrupted);
        }
    }

    /// The duel part of `NpcStatus.reduceHp`: a duelist who hits an NPC leaves the duel.
    pub(crate) fn duel_interrupt_on_npc_hit(&mut self, attacker: ObjectId) {
        // `attacker.asPlayer()`: a summon is its owner.
        if let Some(owner) = self.acting_player(attacker)
            && self.is_in_duel(owner)
        {
            self.set_duel_state(owner, DuelState::Interrupted);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_player_is_in_no_duel() {
        let state = PlayerDuel::default();
        assert!(!state.in_duel);
        assert_eq!(state.state, DuelState::NoDuel);
        assert_eq!(state.team, TEAM_NONE);
        assert_eq!(state.duel_id, 0);
    }
}
