//! The spectators: Java `Player.enterOlympiadObserverMode`, `leaveOlympiadObserverMode`,
//! `leaveObserverMode` and the client packets of the observer.
// Java: entity/actor/Player.java (enterOlympiadObserverMode, leaveOlympiadObserverMode,
//       leaveObserverMode, inObserverMode)
// Java: network/clientpackets/{ObserverReturn,RequestOlympiadObserverEnd,RequestOlympiadMatchList,
//       RequestExOlympiadMatchListRefresh}.java
// Java: network/serverpackets/ExOlympiadMatchList.java
//
// `Player.enterObserverMode(loc)` (the castle observation of the broadcasting towers) is not
// here: nothing starts it yet. `ObserverReturn` still ends any observer mode the way Java does.

use l2_data::stats::AbnormalType;
use l2_protocol_game::server::{ExOlympiadMatchList, ExOlympiadMode, ObservationReturn, OlympiadMatchEntry};
use l2_util::ObjectId;

use super::game::CompetitionType;
use crate::effects::SkillFinishType;
use crate::teleport::TeleportTarget;
use crate::world::World;

/// Java `Player.FALLING_VALIDATION_DELAY`.
const FALLING_VALIDATION_DELAY_MS: i64 = 1000;

/// The client packets of the Olympiad (`ClientInput::Olympiad`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OlympiadInput {
    /// Java `ObserverReturn`.
    ObserverReturn,
    /// Java `RequestOlympiadObserverEnd`.
    ObserverEnd,
    /// Java `RequestOlympiadMatchList`.
    MatchList,
    /// Java `RequestExOlympiadMatchListRefresh`.
    MatchListRefresh,
}

impl World {
    /// The Olympiad client packets.
    pub(crate) fn on_olympiad_input(&mut self, id: ObjectId, input: OlympiadInput) {
        match input {
            OlympiadInput::ObserverReturn => {
                if self.in_observer_mode(id) {
                    self.leave_observer_mode(id);
                }
            }
            OlympiadInput::ObserverEnd => {
                if self.in_observer_mode(id) {
                    self.leave_olympiad_observer_mode(id);
                }
            }
            OlympiadInput::MatchList => {
                // NOTE(java-quirk): Java hands the request to the bypass handler "arenalist",
                // which does not exist, so an observer gets nothing.
            }
            OlympiadInput::MatchListRefresh => {
                let list = self.olympiad_match_list();
                self.send(id, &list);
            }
        }
    }

    /// `new ExOlympiadMatchList()`: the games that started and are not finished.
    pub(crate) fn olympiad_match_list(&mut self) -> ExOlympiadMatchList {
        self.olympiad_game_manager();
        let games = self
            .olympiad
            .stadiums()
            .iter()
            .filter(|s| s.task.is_game_started() && !s.task.is_battle_finished())
            .filter_map(|s| {
                let game = s.task.game.as_ref()?;
                let [player_1, player_2] = game.player_names();
                Some(OlympiadMatchEntry {
                    stadium_id: i32::try_from(game.stadium_id).unwrap_or(0),
                    game_kind: match game.kind {
                        CompetitionType::NonClassed => 1,
                        CompetitionType::Classed => 2,
                        CompetitionType::Teams => -1,
                    },
                    state: if s.task.is_running() { 2 } else { 1 },
                    player_1,
                    player_2,
                })
            })
            .collect();
        ExOlympiadMatchList { games }
    }

    /// Java `enterOlympiadObserverMode(loc, id, instanceId)`.
    pub(crate) fn enter_olympiad_observer_mode(
        &mut self,
        id: ObjectId,
        loc: (i32, i32, i32),
        game_id: i32,
        instance_id: i32,
    ) {
        if let Some(summon) = self.summon_of(id) {
            self.unsummon(summon, Some(id));
        }
        // Remove Hide.
        self.stop_skill_effects_by_abnormal(id, SkillFinishType::Removed, AbnormalType::Hide);
        // The cubics stop without a broadcast.
        self.clear_cubics(id);
        self.olympiad_expel_from_party(id);
        let position = self.position_of(id);
        let sitting = self.is_sitting(id);
        if let Some(state) = self.player_olympiad_mut(id) {
            state.game_id = game_id;
        }
        if sitting {
            self.stand_up(id);
        }
        if let Some(state) = self.player_olympiad_mut(id) {
            if !state.observer_mode
                && let Some(position) = position
            {
                state.last_loc = position;
            }
            state.observer_mode = true;
        }
        self.set_target(id, None);
        self.set_invul(id, true);
        self.set_invisible(id, true);
        self.tele_to_location(
            id,
            TeleportTarget::at(loc.0, loc.1, loc.2).with_instance(instance_id),
        );
        self.send(id, &ExOlympiadMode { mode: 3 });
        self.broadcast_user_info(id);
    }

    /// Java `leaveOlympiadObserverMode()`.
    pub fn leave_olympiad_observer_mode(&mut self, id: ObjectId) {
        let Some(state) = self.player_olympiad(id) else {
            return;
        };
        if state.game_id == -1 {
            return;
        }
        if let Some(state) = self.player_olympiad_mut(id) {
            state.game_id = -1;
            state.observer_mode = false;
        }
        self.set_target(id, None);
        self.send(id, &ExOlympiadMode { mode: 0 });
        self.set_instance_id(id, 0);
        let (x, y, z) = state.last_loc;
        let offset = self.settings.teleport.max_offset_on_teleport;
        self.tele_to_location(id, TeleportTarget::at(x, y, z).with_random_offset(offset));
        if !self.is_gm(id) {
            self.set_invisible(id, false);
            self.set_invul(id, false);
        }
        self.set_intention_idle(id);
        if let Some(state) = self.player_olympiad_mut(id) {
            state.last_loc = (0, 0, 0);
        }
        self.broadcast_user_info(id);
    }

    /// Java `leaveObserverMode()` (`ObserverReturn`).
    pub fn leave_observer_mode(&mut self, id: ObjectId) {
        let Some(state) = self.player_olympiad(id) else {
            return;
        };
        self.set_target(id, None);
        let (x, y, z) = state.last_loc;
        self.tele_to_location(id, TeleportTarget::at(x, y, z));
        if let Some(state) = self.player_olympiad_mut(id) {
            state.last_loc = (0, 0, 0);
        }
        let (x, y, z) = self.position_of(id).unwrap_or((x, y, z));
        self.send(id, &ObservationReturn { x, y, z });
        // `setParalyzed(false)`: only `enterObserverMode` paralyzes, and nothing calls it yet.
        if !self.is_gm(id) {
            self.set_invisible(id, false);
            self.set_invul(id, false);
        }
        self.set_intention_idle(id);
        // `setFalling()`: "Prevent receive falling damage."
        let until = self.world_ms() + FALLING_VALIDATION_DELAY_MS;
        self.set_falling_until(id, until);
        if let Some(state) = self.player_olympiad_mut(id) {
            state.observer_mode = false;
        }
        self.broadcast_user_info(id);
    }
}
