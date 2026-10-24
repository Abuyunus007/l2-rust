//! The siege state of a player: the side (`_siegeState`, `_siegeSide`), whether the player is in
//! the siege (`_isInSiege`, `_isInHideoutSiege`), the fame of the siege zone, the siege friends
//! and the siege skills of a clan leader. The fortress, conquerable hall and Territory War cards
//! set the same state through these functions.
// Java: entity/actor/Player.java (setSiegeState, getSiegeState, setSiegeSide, getSiegeSide,
//       isRegisteredOnThisSiegeField, isSiegeFriend, isOnSameSiegeSideWith, setInSiege,
//       isInSiege, setInHideoutSiege, isInHideoutSiege, startFameTask, stopFameTask)
// Java: entity/actor/tasks/player/FameTask.java
// Java: managers/SiegeManager.java (addSiegeSkills, removeSiegeSkills)
// Java: data/xml/SkillData.java (getSiegeSkills)

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;

use super::SiegeTimer;
use crate::residence::CastleId;
use crate::timer::TimerId;
use crate::world::World;
use crate::zones::ZoneId;

/// Seal of Ruler, Build Headquarters (`SkillData.getSiegeSkills`).
const SIEGE_SKILLS: [i32; 2] = [246, 247];
/// Build Advanced Headquarters, for a noble.
const NOBLE_SIEGE_SKILL: i32 = 326;
/// The two skills of a castle owner's leader (844, 845).
const CASTLE_SIEGE_SKILLS: [i32; 2] = [844, 845];

/// The siege fields of one player.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlayerSiegeState {
    /// `_siegeState`: 1 attacker, 2 defender, 0 not involved.
    pub state: u8,
    /// `_siegeSide`: the residence id of the siege (81..89 for Territory War).
    pub side: i32,
    /// `_isInSiege`.
    pub in_siege: bool,
    /// `_isInHideoutSiege`.
    pub in_hideout_siege: bool,
    /// `_fameTask`: the timer, its delay and the fame each run gives.
    pub(crate) fame: Option<(TimerId, i64, i32)>,
}

impl World {
    fn siege_player(&self, id: ObjectId) -> Option<&PlayerSiegeState> {
        self.sieges.players.get(&id)
    }

    fn siege_player_mut(&mut self, id: ObjectId) -> Option<&mut PlayerSiegeState> {
        if !self.players.contains_key(&id) {
            return None;
        }
        Some(self.sieges.players.entry(id).or_default())
    }

    /// Java `getSiegeState()`: 1 attacker, 2 defender, 0 not involved (0 for anything that is no
    /// player).
    pub fn siege_state_of(&self, id: ObjectId) -> u8 {
        self.siege_player(id).map_or(0, |s| s.state)
    }

    /// Java `setSiegeState(state)`.
    pub fn set_siege_state(&mut self, id: ObjectId, state: u8) {
        if let Some(s) = self.siege_player_mut(id) {
            s.state = state;
        }
    }

    /// Java `getSiegeSide()`.
    pub fn siege_side_of(&self, id: ObjectId) -> i32 {
        self.siege_player(id).map_or(0, |s| s.side)
    }

    /// Java `setSiegeSide(value)`.
    pub fn set_siege_side(&mut self, id: ObjectId, side: i32) {
        if let Some(s) = self.siege_player_mut(id) {
            s.side = side;
        }
    }

    /// Java `isInSiege()`.
    pub fn is_in_siege(&self, id: ObjectId) -> bool {
        self.siege_player(id).is_some_and(|s| s.in_siege)
    }

    /// Java `setInSiege(value)`.
    pub fn set_in_siege(&mut self, id: ObjectId, value: bool) {
        if let Some(s) = self.siege_player_mut(id) {
            s.in_siege = value;
        }
    }

    /// Java `isInHideoutSiege()`.
    pub fn is_in_hideout_siege(&self, id: ObjectId) -> bool {
        self.siege_player(id).is_some_and(|s| s.in_hideout_siege)
    }

    /// Java `setInHideoutSiege(value)` (the conquerable halls, M7-CHSIEGE-01).
    pub fn set_in_hideout_siege(&mut self, id: ObjectId, value: bool) {
        if let Some(s) = self.siege_player_mut(id) {
            s.in_hideout_siege = value;
        }
    }

    /// Java `isRegisteredOnThisSiegeField(value)`: the side, or any territory (81..89).
    pub fn is_registered_on_this_siege_field(&self, id: ObjectId, value: i32) -> bool {
        let side = self.siege_side_of(id);
        side == value || (81..=89).contains(&side)
    }

    /// Java `isOnSameSiegeSideWith(target)`.
    pub fn is_on_same_siege_side_with(&self, id: ObjectId, target: ObjectId) -> bool {
        let state = self.siege_state_of(id);
        state > 0
            && self.is_inside_zone(id, ZoneId::Siege)
            && state == self.siege_state_of(target)
            && self.siege_side_of(id) == self.siege_side_of(target)
    }

    /// Java `isSiegeFriend(target)`: both on the same side of the same castle siege in the siege
    /// zone; attackers only before the first owner change, or while the castle has no owner.
    pub fn is_siege_friend(&self, id: ObjectId, target: ObjectId) -> bool {
        let state = self.siege_state_of(id);
        // If i'm natural or not in siege zone, not friends.
        if state == 0 || !self.is_inside_zone(id, ZoneId::Siege) {
            return false;
        }
        let side = self.siege_side_of(id);
        let Some(castle) = self.castle(CastleId(side)) else {
            return false;
        };
        // If target isn't a player, is self.
        let Some(target) = self.acting_player(target) else {
            return false;
        };
        if target == id {
            return false;
        }
        // If target isn't on same siege or not on same state, not friends.
        if self.siege_side_of(target) != side || state != self.siege_state_of(target) {
            return false;
        }
        if state == 1 {
            // Check first castle mid victory.
            if !castle.first_mid_victory {
                return true;
            }
            // Attackers are considered friends only if castle has no owner.
            return self.clan(castle.owner_id).is_none();
        }
        // Both are defenders, friends.
        true
    }

    /// The siege state of a player that leaves the world (the fame task stops).
    pub(crate) fn forget_player_siege_state(&mut self, id: ObjectId) {
        if let Some(state) = self.sieges.players.remove(&id)
            && let Some((timer, _, _)) = state.fame
        {
            self.timers.cancel(timer);
        }
    }

    // ------------------------------------------------------------------ fame

    /// Java `startFameTask(delay, fameFixRate)`: a level 40+ player of the second class or later
    /// gets `fame` every `delay_ms` while in the siege (`EnableFameSystem`).
    pub fn start_fame_task(&mut self, id: ObjectId, delay_ms: i64, fame: i32) {
        if !self.sieges.settings.fame_system_enabled {
            return;
        }
        let Some(level) = self.players.get(&id).map(|p| p.core.level) else {
            return;
        };
        let class_level = self
            .players
            .get(&id)
            .and_then(|p| u16::try_from(p.data.class_id).ok())
            .map_or(0, |c| l2_data::ClassId(c).class_level());
        if level < 40 || class_level < 2 {
            return;
        }
        if self.sieges.players.get(&id).is_some_and(|s| s.fame.is_some()) {
            return;
        }
        let timer = self.schedule_siege_timer(SiegeTimer::Fame(id), delay_ms);
        if let Some(s) = self.siege_player_mut(id) {
            s.fame = Some((timer, delay_ms, fame));
        }
    }

    /// Java `stopFameTask()`.
    pub fn stop_fame_task(&mut self, id: ObjectId) {
        if let Some((timer, _, _)) = self.sieges.players.get_mut(&id).and_then(|s| s.fame.take()) {
            self.timers.cancel(timer);
        }
    }

    /// Whether the player's fame task runs.
    pub fn has_fame_task(&self, id: ObjectId) -> bool {
        self.siege_player(id).is_some_and(|s| s.fame.is_some())
    }

    /// Java `FameTask.run()`, then the next run (`scheduleAtFixedRate`).
    pub(crate) fn on_fame_timer(&mut self, id: ObjectId) {
        let Some((_, delay, fame)) = self.siege_player(id).and_then(|s| s.fame) else {
            return;
        };
        let timer = self.schedule_siege_timer(SiegeTimer::Fame(id), delay);
        if let Some(s) = self.siege_player_mut(id) {
            s.fame = Some((timer, delay, fame));
        }
        if self.is_dead(id) && !self.sieges.settings.fame_for_dead_players {
            return;
        }
        // A player without a client is in offline mode (`isDetached`).
        if !self.sinks.contains_key(&id) && !self.sieges.settings.offline_fame {
            return;
        }
        let current = self.players.get(&id).map_or(0, |p| p.data.profile.fame);
        self.set_fame(id, current.saturating_add(fame));
        self.send(
            id,
            &SystemMessage::new(SystemMessageId::YouHaveAcquiredS1Reputation)
                .with(SystemMessageParam::IntNumber(fame)),
        );
        self.send_user_info(id);
    }

    // ------------------------------------------------------------------ siege skills

    /// Java `SkillData.getSiegeSkills(isNoble, hasCastle)`: `(id, level 1)`.
    fn siege_skills_of(&self, player: ObjectId) -> Vec<i32> {
        let noble = self.players.get(&player).is_some_and(|p| p.data.profile.noble);
        let has_castle = self.clan_of(player).is_some_and(|c| c.castle_id > 0);
        let mut skills = SIEGE_SKILLS.to_vec();
        if noble {
            skills.push(NOBLE_SIEGE_SKILL);
        }
        if has_castle {
            skills.extend(CASTLE_SIEGE_SKILLS);
        }
        skills
    }

    /// Java `SiegeManager.addSiegeSkills(player)`: `addSkill(skill, false)` (not stored).
    pub fn add_siege_skills(&mut self, player: ObjectId) {
        for skill in self.siege_skills_of(player) {
            self.add_skill(player, skill, 1);
        }
    }

    /// Java `SiegeManager.removeSiegeSkills(player)`: `removeSkill(skill)` (the effects go).
    pub fn remove_siege_skills(&mut self, player: ObjectId) {
        for skill in self.siege_skills_of(player) {
            self.remove_skill(player, skill, true);
        }
    }

    /// The relation `UserInfo` writes: the clan leader bit, then the siege state (attacker
    /// `0x180`, or `0x1000` for a Territory War registration; defender `0x80`).
    pub fn user_info_relation(&self, player: ObjectId) -> i32 {
        let mut relation = if self.is_clan_leader(player) { 0x40 } else { 0 };
        match self.siege_state_of(player) {
            1 if self.registered_territory_id(player) == 0 => relation |= 0x180,
            1 => relation |= 0x1000,
            2 => relation |= 0x80,
            _ => {}
        }
        relation
    }

    /// `EnterWorld`'s castle sieges: a member of an attacker (defender) clan of a siege in
    /// progress logs in on that side.
    pub(crate) fn siege_on_enter_world(&mut self, player: ObjectId, clan_id: i32) {
        let sides: Vec<(i32, u8)> = self
            .castle_sieges()
            .filter(|s| s.in_progress)
            .filter_map(|s| {
                if s.is_attacker(Some(clan_id)) {
                    Some((s.castle_id.0, 1))
                } else if s.is_defender(Some(clan_id)) {
                    Some((s.castle_id.0, 2))
                } else {
                    None
                }
            })
            .collect();
        for (castle, state) in sides {
            self.set_siege_state(player, state);
            self.set_siege_side(player, castle);
        }
    }

    /// `EnterWorld`: "Attacker or spectator logging in to a siege zone" goes to the town (not a
    /// GM, nor a defender in the siege).
    pub(crate) fn siege_enter_world_zone_check(&mut self, player: ObjectId) {
        if !self.is_gm(player)
            && self.is_inside_zone(player, ZoneId::Siege)
            && (!self.is_in_siege(player) || self.siege_state_of(player) < 2)
        {
            self.tele_to_location_where(player, crate::teleport::TeleportWhereType::Town);
        }
    }

    /// The Territory War territory a player is registered for (Java
    /// `TerritoryWarManager.getRegisteredTerritoryId(player)`): 0 until M7-TW-01.
    // M7-TW-01: the registrations of Territory War.
    pub fn registered_territory_id(&self, _player: ObjectId) -> i32 {
        0
    }
}
