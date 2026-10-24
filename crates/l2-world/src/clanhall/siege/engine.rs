//! `ClanHallSiegeEngine`: the attackers, the guards, the tasks and the three steps of a siege
//! (`prepareOwner`, `startSiege`, `endSiege`), and the hooks the six scripts override.
// Java: mechanics/siege/clanhalls/ClanHallSiegeEngine.java
// Java: mechanics/siege/clanhalls/SiegableHall.java (spawnDoor, addAttacker, removeAttacker,
//       updateNextSiege, updateSiegeZone)

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use tracing::{info, warn};

use super::{
    HallScript, HallSiege, HallSiegeTimer, ScriptState, SiegeStatus, devastated, flag_war, rainbow,
    resistance, schedule,
};
use crate::clan::model::java_map_order;
use crate::clanhall::ClanHallTimer;
use crate::residence::{ClanHallId, ClanHallKind, ResidenceRef, Writes};
use crate::siege::{SiegeClan, SiegeClanType};
use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::World;

/// The hour before the siege date the registration ends (`- 3600000` everywhere in Java).
const PREPARE_BEFORE_MS: i64 = 3_600_000;
/// `SiegeStarts` is scheduled this long after `PrepareOwner`.
const START_AFTER_PREPARE_MS: i64 = 3_600_000;

/// A Java exception a script's code would throw (a `ConcurrentModificationException` of a map
/// changed while it is walked, an index out of bounds, a null pointer). The caller that catches
/// it logs it; whatever came after the throwing statement does not run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ScriptError(pub(crate) &'static str);

pub(crate) type ScriptResult = Result<(), ScriptError>;

impl World {
    // ------------------------------------------------------------------ queries

    pub(crate) fn hall_siege(&self, id: ClanHallId) -> Option<&HallSiege> {
        self.clanhall.siege.halls.get(&id)
    }

    pub(crate) fn hall_siege_mut(&mut self, id: ClanHallId) -> Option<&mut HallSiege> {
        self.clanhall.siege.halls.get_mut(&id)
    }

    /// Whether the hall has a siege engine (its script was loaded).
    pub fn hall_has_siege(&self, id: ClanHallId) -> bool {
        self.hall_siege(id).is_some()
    }

    /// Java `SiegableHall.getSiegeStatus()` (`None` for a hall without an engine).
    pub fn hall_siege_status(&self, id: ClanHallId) -> Option<SiegeStatus> {
        self.hall_siege(id).map(|s| s.status)
    }

    /// Java `SiegableHall.isRegistering()`.
    pub fn hall_is_registering(&self, id: ClanHallId) -> bool {
        self.hall_siege_status(id) == Some(SiegeStatus::Registering)
    }

    /// Java `SiegableHall.isWaitingBattle()`.
    pub fn hall_is_waiting_battle(&self, id: ClanHallId) -> bool {
        self.hall_siege_status(id) == Some(SiegeStatus::WaitingBattle)
    }

    /// Java `SiegableHall.isInSiege()`.
    pub fn hall_is_in_siege(&self, id: ClanHallId) -> bool {
        self.hall_siege_status(id) == Some(SiegeStatus::Running)
    }

    /// Java `SiegableHall.getNextSiegeTime()` (epoch milliseconds).
    pub fn hall_next_siege_time(&self, id: ClanHallId) -> i64 {
        match self.clan_hall(id).map(|h| &h.kind) {
            Some(ClanHallKind::Siegable { next_siege, .. }) => *next_siege,
            _ => 0,
        }
    }

    /// Java `SiegableHall.getSiegeLength()` (milliseconds).
    pub fn hall_siege_length(&self, id: ClanHallId) -> i64 {
        match self.clan_hall(id).map(|h| &h.kind) {
            Some(ClanHallKind::Siegable { siege_length, .. }) => *siege_length,
            _ => 0,
        }
    }

    /// Java `getAttackers().keySet()` in the map's iteration order.
    pub fn hall_siege_attacker_clans(&self, id: ClanHallId) -> Vec<i32> {
        let Some(siege) = self.hall_siege(id) else {
            return Vec::new();
        };
        let keys: Vec<i32> = siege.attackers.iter().map(|a| a.clan_id).collect();
        java_map_order(&keys, siege.attackers_peak.max(keys.len()))
            .into_iter()
            .map(|i| keys[i])
            .collect()
    }

    /// Java `ClanHallSiegeEngine.checkIsAttacker(clan)`.
    pub fn hall_siege_is_attacker(&self, id: ClanHallId, clan_id: i32) -> bool {
        clan_id != 0
            && self
                .hall_siege(id)
                .is_some_and(|s| s.attackers.iter().any(|a| a.clan_id == clan_id))
    }

    /// Java `ClanHallSiegeEngine.getAttackerClan(clanId)`.
    pub fn hall_siege_attacker_clan(&self, id: ClanHallId, clan_id: i32) -> Option<&SiegeClan> {
        self.hall_siege(id)?
            .attackers
            .iter()
            .find(|a| a.clan_id == clan_id)
    }

    pub(crate) fn hall_siege_attacker_clan_mut(
        &mut self,
        id: ClanHallId,
        clan_id: i32,
    ) -> Option<&mut SiegeClan> {
        self.hall_siege_mut(id)?
            .attackers
            .iter_mut()
            .find(|a| a.clan_id == clan_id)
    }

    /// Java `SpecialSiegeGuardAI.getAlly()`: the players a flag war warrior leaves alone.
    pub fn hall_siege_allies_of(&self, npc: ObjectId) -> &[i32] {
        self.clanhall.siege.allies.get(&npc).map_or(&[], Vec::as_slice)
    }

    fn hall_name(&self, id: ClanHallId) -> String {
        self.clan_hall(id).map(|h| h.name.clone()).unwrap_or_default()
    }

    // ------------------------------------------------------------------ attackers

    /// Java `SiegableHall.addAttacker(clan)`: the clan is put in the map (a new `SiegeClan`, so a
    /// clan that was there loses its headquarters). Nothing without an engine.
    pub fn hall_add_attacker(&mut self, id: ClanHallId, clan_id: i32) {
        let Some(siege) = self.hall_siege_mut(id) else {
            return;
        };
        let attacker = SiegeClan::new(clan_id, SiegeClanType::Attacker);
        match siege.attackers.iter_mut().find(|a| a.clan_id == clan_id) {
            Some(existing) => *existing = attacker,
            None => siege.attackers.push(attacker),
        }
        siege.attackers_peak = siege.attackers_peak.max(siege.attackers.len());
    }

    /// Java `SiegableHall.removeAttacker(clan)`.
    pub fn hall_remove_attacker(&mut self, id: ClanHallId, clan_id: i32) {
        if let Some(siege) = self.hall_siege_mut(id) {
            siege.attackers.retain(|a| a.clan_id != clan_id);
        }
    }

    /// Java `getAttackers().clear()`.
    pub fn hall_clear_attackers(&mut self, id: ClanHallId) {
        if let Some(siege) = self.hall_siege_mut(id) {
            siege.attackers.clear();
        }
    }

    /// `ClanTable.destroyClan`: `hall.removeAttacker(clan)` of every conquerable hall.
    pub(crate) fn hall_sieges_remove_attacker(&mut self, clan_id: i32) {
        let halls: Vec<ClanHallId> = self.clanhall.siege.halls.keys().copied().collect();
        for hall in halls {
            self.hall_remove_attacker(hall, clan_id);
        }
    }

    /// Java `CHSiegeManager.isClanParticipating(clan)`.
    pub fn hall_clan_is_participating(&self, clan_id: i32) -> bool {
        self.clanhall
            .siege
            .halls
            .keys()
            .any(|hall| self.hall_siege_is_attacker(*hall, clan_id))
    }

    /// `ClanHallSiegeEngine.saveAttackers` for every hall (`CHSiegeManager.onServerShutDown`): the
    /// rows go out only with `SiegeDatabaseWrites` (D-51).
    pub(crate) fn hall_sieges_shutdown(&mut self) {
        let halls: Vec<ClanHallId> = self.clanhall.siege.halls.keys().copied().collect();
        for hall in halls {
            self.hall_save_attackers(hall);
        }
        if !self.clanhall.siege.halls.is_empty() {
            info!("CHSiegeManager: Siegable hall attacker lists saved!");
        }
    }

    /// Java `ClanHallSiegeEngine.saveAttackers()`.
    // NOTE(java-quirk): the three flag war and Rainbow Springs scripts keep their own tables and
    // still write this one at shutdown.
    pub(crate) fn hall_save_attackers(&mut self, id: ClanHallId) {
        let clans = self.hall_siege_attacker_clans(id);
        self.siege_write(crate::residence::ResidenceWrite::HallSiege(
            super::HallSiegeWrite::SaveAttackers { hall_id: id.0, clans },
        ));
    }

    // ------------------------------------------------------------------ schedule

    /// Schedules a hall siege timer `delay_ms` from now.
    pub(crate) fn hall_schedule(&mut self, timer: HallSiegeTimer, delay_ms: i64) -> TimerId {
        let delay = u64::try_from(delay_ms.max(0)).unwrap_or(0);
        let due = self.tick + TimerWheel::ms_to_ticks(delay);
        self.timers
            .schedule_at(due, TimerKind::ClanHall(ClanHallTimer::HallSiege(timer)))
    }

    /// `_siegeTask = ThreadPool.schedule(task, delay)`. Java does not cancel the task it replaces.
    pub(crate) fn hall_set_task(&mut self, id: ClanHallId, timer: HallSiegeTimer, delay_ms: i64) {
        let task = self.hall_schedule(timer, delay_ms);
        if let Some(siege) = self.hall_siege_mut(id) {
            siege.task = Some(task);
        }
    }

    /// Java `cancelSiegeTask()`.
    pub fn hall_cancel_siege_task(&mut self, id: ClanHallId) {
        if let Some(task) = self.hall_siege(id).and_then(|s| s.task) {
            self.timers.cancel(task);
        }
    }

    /// The delay of the `PrepareOwner` after a new siege date: `nextSiege - now - 1 hour`.
    fn hall_prepare_delay(&self, id: ClanHallId) -> i64 {
        self.hall_next_siege_time(id) - self.siege_now_ms() - PREPARE_BEFORE_MS
    }

    /// `ThreadPool.schedule(new PrepareOwner(), nextSiege - now - 3600000)`.
    pub(crate) fn hall_arm_prepare_owner(&mut self, id: ClanHallId) {
        let delay = self.hall_prepare_delay(id);
        self.hall_set_task(id, HallSiegeTimer::PrepareOwner(id), delay);
    }

    /// Java `SiegableHall.updateNextSiege()`: the next date from the schedule, written.
    pub fn hall_update_next_siege(&mut self, id: ClanHallId) {
        let Some(config) = self.hall_siege(id).map(|s| s.schedule_config) else {
            return;
        };
        self.hall_update_next_siege_with(id, config);
    }

    /// [`Self::hall_update_next_siege`] with a schedule (the constructor reads the row before the
    /// engine exists).
    pub(crate) fn hall_update_next_siege_with(&mut self, id: ClanHallId, config: [i32; 5]) {
        let now = self.siege_now_ms();
        let next = schedule::next_siege_after(now, config, &|t| self.siege_offset_at(t));
        if let Some(ClanHallKind::Siegable { next_siege, .. }) = self.clan_hall_mut(id).map(|h| &mut h.kind) {
            *next_siege = next;
        }
        self.clan_hall_update_db(id, Writes::Job);
    }

    /// Java `setNextSiegeDate(date)` and `updateSiege()` and `updateDb()` (the GM's new date).
    pub fn hall_set_next_siege_date(&mut self, id: ClanHallId, date: i64) {
        if let Some(ClanHallKind::Siegable { next_siege, .. }) = self.clan_hall_mut(id).map(|h| &mut h.kind) {
            *next_siege = date;
        }
        self.hall_update_siege(id);
        self.clan_hall_update_db(id, Writes::Job);
    }

    /// Java `updateSiege()`: the task is armed again for the new date.
    // NOTE(java-quirk): the delay lacks the `- System.currentTimeMillis()` of the constructor's, so
    // it is the date itself, decades away: after a GM changed the date, nothing starts the
    // registration's end by itself.
    pub fn hall_update_siege(&mut self, id: ClanHallId) {
        self.hall_cancel_siege_task(id);
        let delay = self.hall_next_siege_time(id) - PREPARE_BEFORE_MS;
        self.hall_set_task(id, HallSiegeTimer::PrepareOwner(id), delay);
    }

    /// The automatic `PrepareOwner`: only with `SiegeDatabaseWrites` (D-51); without it the
    /// Java game server runs the siege.
    pub(crate) fn on_hall_prepare_owner_timer(&mut self, id: ClanHallId) {
        if !self.siege_database_writes() {
            tracing::debug!("SideBySide: {id} is not prepared (SiegeDatabaseWrites = False).");
            return;
        }
        self.hall_prepare_owner(id);
    }

    pub(crate) fn hall_siege_start_task(&mut self, id: ClanHallId) {
        self.hall_start_siege(id);
    }

    pub(crate) fn hall_siege_end_task(&mut self, id: ClanHallId) {
        self.hall_end_siege(id);
    }

    // ------------------------------------------------------------------ the zone and the doors

    /// Java `SiegableHall.spawnDoor(isDoorWeak)`: a destroyed door comes back with all its HP
    /// (half when weak), an open one closes.
    pub(crate) fn hall_spawn_door(&mut self, id: ClanHallId, weak: bool) {
        for door in self.residence_doors(ResidenceRef::ClanHall(id)) {
            let Some((dead, open, max)) = self.door(door).map(|d| (d.dead, d.open, d.max_hp)) else {
                continue;
            };
            if dead {
                self.revive_door(door);
                let hp = if weak { max / 2 } else { max };
                self.set_door_current_hp(door, f64::from(hp));
            }
            if open {
                self.close_door(door);
            }
        }
    }

    /// Java `ClanHall.openCloseDoor(doorId, open)`.
    pub fn hall_open_close_door(&mut self, id: ClanHallId, door_id: i32, open: bool) {
        if let Some(door) = self.residence_door(ResidenceRef::ClanHall(id), door_id) {
            if open {
                self.open_door(door);
            } else {
                self.close_door(door);
            }
        }
    }

    // ------------------------------------------------------------------ guards

    /// Java `loadGuards()`: the spawns are made from the rows once, later the respawn is started.
    fn hall_load_guards(&mut self, id: ClanHallId) {
        if let Some(guards) = self.hall_siege(id).and_then(|s| s.guards.clone()) {
            for (spawn, _) in guards {
                self.start_respawn(spawn);
            }
            return;
        }
        let rows = self
            .hall_siege(id)
            .map(|s| s.guard_rows.clone())
            .unwrap_or_default();
        let guards: Vec<(usize, i32)> = rows
            .iter()
            .map(|row| {
                // `setAmount(1)`, `setRespawnDelay(delay)`.
                let spawn =
                    self.fort_new_spawn(row.npc_id, (row.x, row.y, row.z), row.heading, row.respawn_delay);
                (spawn, row.respawn_delay)
            })
            .collect();
        if let Some(siege) = self.hall_siege_mut(id) {
            siege.guards = Some(guards);
        }
    }

    /// Java `spawnSiegeGuards()`.
    fn hall_spawn_siege_guards(&mut self, id: ClanHallId) {
        let guards = self
            .hall_siege(id)
            .and_then(|s| s.guards.clone())
            .unwrap_or_default();
        for (spawn, delay) in guards {
            self.fort_do_spawn(spawn);
            if delay == 0 {
                self.stop_respawn(spawn);
            } else {
                self.start_respawn(spawn);
            }
        }
    }

    /// Java `unSpawnSiegeGuards()`.
    fn hall_unspawn_siege_guards(&mut self, id: ClanHallId) {
        let guards = self
            .hall_siege(id)
            .and_then(|s| s.guards.clone())
            .unwrap_or_default();
        for (spawn, _) in guards {
            self.stop_respawn(spawn);
            if let Some(npc) = self.last_spawn(spawn) {
                self.delete_npc(npc);
            }
        }
    }

    // ------------------------------------------------------------------ the players

    /// The members of every attacker clan who are online, with the clan list in Java's order.
    pub(crate) fn hall_attacker_players(&self, id: ClanHallId) -> Vec<ObjectId> {
        let mut players = Vec::new();
        for clan in self.hall_siege_attacker_clans(id) {
            if self.clan(clan).is_none() {
                continue;
            }
            players.extend(self.clan_online_members(clan, 0));
        }
        players
    }

    /// `pc.setSiegeState(state); pc.broadcastUserInfo(); pc.setInHideoutSiege(in_siege)` for the
    /// online members of the attacker clans.
    fn hall_set_attackers_siege_state(&mut self, id: ClanHallId, state: u8, in_siege: bool) {
        for pc in self.hall_attacker_players(id) {
            self.set_siege_state(pc, state);
            self.broadcast_user_info(pc);
            self.set_in_hideout_siege(pc, in_siege);
        }
    }

    // ------------------------------------------------------------------ prepareOwner

    /// Java `prepareOwner()`: the owner is an attacker like the others, the hall is free, the
    /// registration is over.
    pub fn hall_prepare_owner(&mut self, id: ClanHallId) {
        let result = match self.hall_siege(id).map(|s| s.script) {
            Some(HallScript::BanditStronghold | HallScript::WildBeastReserve) => {
                flag_war::prepare_owner(self, id)
            }
            Some(HallScript::RainbowSpringsChateau) => rainbow::prepare_owner(self, id),
            Some(_) => {
                self.base_prepare_owner(id);
                Ok(())
            }
            None => Ok(()),
        };
        if let Err(e) = result {
            warn!("{id}: prepareOwner failed: {}", e.0);
        }
    }

    fn base_prepare_owner(&mut self, id: ClanHallId) {
        let owner = self.clan_hall(id).map_or(0, |h| h.owner_id);
        if owner > 0 {
            self.hall_add_attacker(id, owner);
        }
        self.clan_hall_free(id);
        self.residence_banish_foreigners(ResidenceRef::ClanHall(id));
        self.hall_broadcast_registration_ended(id);
        self.hall_set_status(id, SiegeStatus::WaitingBattle);
        self.hall_set_task(id, HallSiegeTimer::SiegeStarts(id), START_AFTER_PREPARE_MS);
    }

    /// `THE_REGISTRATION_TERM_FOR_S1_HAS_ENDED` to everybody.
    pub(crate) fn hall_broadcast_registration_ended(&self, id: ClanHallId) {
        self.broadcast_to_all_online_players(
            &SystemMessage::new(SystemMessageId::TheRegistrationTermForS1HasEnded)
                .with(SystemMessageParam::Text(self.hall_name(id))),
        );
    }

    /// `THE_SIEGE_OF_S1_HAS_BEEN_CANCELED_DUE_TO_LACK_OF_INTEREST` to everybody.
    pub(crate) fn hall_broadcast_lack_of_interest(&self, id: ClanHallId) {
        self.broadcast_to_all_online_players(
            &SystemMessage::new(SystemMessageId::TheSiegeOfS1HasBeenCanceledDueToLackOfInterest)
                .with(SystemMessageParam::Text(self.hall_name(id))),
        );
    }

    /// Java `SiegableHall.updateSiegeStatus(status)`.
    pub(crate) fn hall_set_status(&mut self, id: ClanHallId, status: SiegeStatus) {
        if let Some(siege) = self.hall_siege_mut(id) {
            siege.status = status;
        }
    }

    // ------------------------------------------------------------------ startSiege

    /// Java `startSiege()` of the hall's script.
    pub fn hall_start_siege(&mut self, id: ClanHallId) {
        let result = match self.hall_siege(id).map(|s| s.script) {
            Some(HallScript::FortressOfTheDead) => self.dead_start_siege(id),
            Some(HallScript::BanditStronghold | HallScript::WildBeastReserve) => {
                flag_war::start_siege(self, id)
            }
            Some(HallScript::RainbowSpringsChateau) => rainbow::start_siege(self, id),
            Some(_) => self.base_start_siege(id),
            None => Ok(()),
        };
        if let Err(e) = result {
            warn!("{id}: startSiege failed: {}", e.0);
        }
    }

    /// Java `ClanHallSiegeEngine.startSiege()`.
    pub(crate) fn base_start_siege(&mut self, id: ClanHallId) -> ScriptResult {
        let empty = self.hall_siege(id).is_none_or(|s| s.attackers.is_empty());
        // The Fortress of Resistance has no attacker list.
        if empty && id.0 != super::FORTRESS_RESISTANCE {
            self.hall_on_siege_ends(id)?;
            self.hall_clear_attackers(id);
            self.hall_update_next_siege(id);
            // NOTE(java-quirk): Java schedules the next `PrepareOwner` with the siege date itself as
            // the delay (no `- currentTimeMillis`), so it never runs, and the status stays
            // WAITING_BATTLE: the hall is stuck until the server restarts.
            let delay = self.hall_next_siege_time(id);
            self.hall_set_task(id, HallSiegeTimer::PrepareOwner(id), delay);
            self.hall_set_status(id, SiegeStatus::WaitingBattle);
            self.hall_broadcast_lack_of_interest(id);
            return Ok(());
        }
        self.hall_spawn_door(id, false);
        self.hall_load_guards(id);
        self.hall_spawn_siege_guards(id);
        self.hall_update_siege_zone(id, true);
        self.hall_set_attackers_siege_state(id, 1, true);
        self.hall_set_status(id, SiegeStatus::Running);
        self.hall_on_siege_starts(id);
        let length = self.hall_siege_length(id);
        self.hall_set_task(id, HallSiegeTimer::SiegeEnds(id), length);
        Ok(())
    }

    // ------------------------------------------------------------------ endSiege

    /// Java `endSiege()` of the hall's script.
    pub fn hall_end_siege(&mut self, id: ClanHallId) {
        let script = self.hall_siege(id).map(|s| s.script);
        if matches!(
            script,
            Some(
                HallScript::BanditStronghold
                    | HallScript::WildBeastReserve
                    | HallScript::RainbowSpringsChateau
            )
        ) {
            // `if (_hall.getOwnerId() > 0) { clan.setHideoutId(0); _hall.free(); }`
            let owner = self.clan_hall(id).map_or(0, |h| h.owner_id);
            if owner > 0 {
                if let Some(clan) = self.clan_mut(owner) {
                    clan.hideout_id = 0;
                }
                self.clan_hall_free(id);
            }
        }
        if script.is_some()
            && let Err(e) = self.base_end_siege(id)
        {
            warn!("{id}: endSiege failed: {}", e.0);
        }
    }

    /// Java `ClanHallSiegeEngine.endSiege()`.
    fn base_end_siege(&mut self, id: ClanHallId) -> ScriptResult {
        let name = self.hall_name(id);
        self.broadcast_to_all_online_players(
            &SystemMessage::new(SystemMessageId::TheS1SiegeHasFinished)
                .with(SystemMessageParam::Text(name.clone())),
        );
        let winner = self.hall_winner(id);
        let accomplished = self.hall_siege(id).is_some_and(|s| s.mission_accomplished);
        let message = match winner {
            Some(winner) if accomplished => {
                self.clan_hall_set_owner(id, winner);
                if let Some(clan) = self.clan_mut(winner) {
                    clan.hideout_id = id.0;
                }
                let winner_name = self.clan(winner).map(|c| c.name.clone()).unwrap_or_default();
                SystemMessage::new(SystemMessageId::ClanS1IsVictoriousOverS2SCastleSiege)
                    .with(SystemMessageParam::Text(winner_name))
            }
            _ => SystemMessage::new(SystemMessageId::TheSiegeOfS1HasEndedInADraw),
        };
        self.broadcast_to_all_online_players(&message.with(SystemMessageParam::Text(name)));
        if let Some(siege) = self.hall_siege_mut(id) {
            siege.mission_accomplished = false;
        }
        self.hall_update_siege_zone(id, false);
        self.hall_update_next_siege(id);
        self.hall_spawn_door(id, false);
        self.residence_banish_foreigners(ResidenceRef::ClanHall(id));
        self.hall_set_attackers_siege_state(id, 0, false);
        // Update pvp flag for winners when siege zone becomes inactive.
        if let Some(zone) = self.hall_siege_zone(id) {
            for player in self.siege_zone_players(zone) {
                self.start_pvp_flag(player);
            }
        }
        self.hall_clear_attackers(id);
        self.hall_on_siege_ends(id)?;
        self.hall_arm_prepare_owner(id);
        self.hall_set_status(id, SiegeStatus::Registering);
        self.hall_unspawn_siege_guards(id);
        Ok(())
    }

    // ------------------------------------------------------------------ the script hooks

    /// `getWinner()`: the clan that wins, if `ClanTable` knows it.
    pub(crate) fn hall_winner(&self, id: ClanHallId) -> Option<i32> {
        let siege = self.hall_siege(id)?;
        let winner = match &siege.state {
            ScriptState::Resistance(s) => resistance::winner(s),
            ScriptState::Devastated(s) => devastated::winner(s),
            ScriptState::Dead(s) => super::dead::winner(s),
            ScriptState::FlagWar(s) => s.winner,
            ScriptState::Rainbow(s) => s.winner,
        };
        winner.filter(|clan| self.clan(*clan).is_some())
    }

    /// `onSiegeStarts()`.
    fn hall_on_siege_starts(&mut self, id: ClanHallId) {
        match self.hall_siege(id).map(|s| s.script) {
            Some(HallScript::FortressOfResistance) => resistance::on_siege_starts(self, id),
            Some(HallScript::DevastatedCastle) => devastated::on_siege_starts(self, id),
            Some(HallScript::BanditStronghold | HallScript::WildBeastReserve) => {
                flag_war::on_siege_starts(self, id);
            }
            _ => {}
        }
    }

    /// `onSiegeEnds()`.
    pub(crate) fn hall_on_siege_ends(&mut self, id: ClanHallId) -> ScriptResult {
        match self.hall_siege(id).map(|s| s.script) {
            Some(HallScript::FortressOfResistance) => {
                resistance::on_siege_ends(self, id);
                Ok(())
            }
            Some(HallScript::BanditStronghold | HallScript::WildBeastReserve) => {
                flag_war::on_siege_ends(self, id)
            }
            Some(HallScript::RainbowSpringsChateau) => {
                rainbow::on_siege_ends(self, id);
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// `EnterWorld`: a member of an attacker clan of a hall in siege logs in on the attacking side.
    pub(crate) fn hall_siege_on_enter_world(&mut self, player: ObjectId, clan_id: i32) {
        let halls: Vec<ClanHallId> = self.clanhall.siege.halls.keys().copied().collect();
        for hall in halls {
            if !self.hall_is_in_siege(hall) {
                continue;
            }
            if self.hall_siege_is_attacker(hall, clan_id) {
                self.set_siege_state(player, 1);
                self.set_siege_side(player, hall.0);
                self.set_in_hideout_siege(player, true);
            }
        }
    }

    /// Java `getInnerSpawnLoc(player)` of the hall's script: where a participant restarts inside.
    pub(crate) fn hall_inner_spawn_loc(
        &self,
        hall: ClanHallId,
        player: ObjectId,
    ) -> Option<l2_data::Location> {
        match self.hall_siege(hall).map(|s| s.script) {
            Some(HallScript::BanditStronghold | HallScript::WildBeastReserve) => {
                // An exception (`ArrayIndexOutOfBoundsException`) ends the restart without a point.
                self.flag_war_inner_spawn_loc(hall, player).ok().flatten()
            }
            _ => None,
        }
    }

    /// The `getFlag`, `getInnerSpawnLoc`, `canPlantFlag` and `doorIsAutoAttackable` of the script.
    pub(crate) fn hall_door_is_auto_attackable(&self, id: ClanHallId) -> bool {
        // Bandit Stronghold and Wild Beast Reserve answer false, the others keep the default.
        !matches!(
            self.hall_siege(id).map(|s| s.script),
            Some(HallScript::BanditStronghold | HallScript::WildBeastReserve)
        )
    }
}
