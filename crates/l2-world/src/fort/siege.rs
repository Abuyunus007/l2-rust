//! The fortress siege (Java `FortSiege`): registration, the countdown, start and end, the siege
//! state of the clans' members, the commanders and the barracks, the flags of the attackers.
// Java: mechanics/siege/FortSiege.java
// Java: mechanics/siege/FortTeleportWhoType.java
// Java: mechanics/siege/Fort.java (resetDoors, setSiegeDate, getSiegeDate)
// Java: managers/FortSiegeManager.java (getSiege(x, y, z), getSieges)
// Java: entity/actor/tasks/player/FameTask.java, entity/actor/Player.java (startFameTask,
//       stopFameTask)
//
// See the module comment of `fort/mod.rs` for what runs side by side with the Java game server.
// Java's `ThreadPool.execute(task)` (run at once on a pool thread) is a plain call here.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ChatType, NpcSay, SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use tracing::{info, warn};

use super::FortTimer;
use crate::residence::{FortId, ResidenceWrite, Writes};
use crate::teleport::TeleportWhereType;
use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::{World, unix_ms_now};

/// The adena the first clan of a siege pays (`addAttacker`).
const REGISTRATION_FEE: i64 = 250_000;

/// Java `FortTeleportWhoType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FortTeleportWhoType {
    All,
    Attacker,
    Owner,
}

/// Java `SiegeClan` of an attacker: the clan and the headquarters flags it has put up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FortSiegeClan {
    pub clan_id: i32,
    /// `_flag`: the object ids of the flag NPCs (filled by the headquarters code, M7-SIEGE-01).
    pub flags: Vec<ObjectId>,
    /// Whether the registration is a row of `fortsiege_clans` (a registration made without
    /// `SiegeDatabaseWrites` exists in this world only).
    pub(crate) written: bool,
}

impl FortSiegeClan {
    fn new(clan_id: i32, written: bool) -> Self {
        Self {
            clan_id,
            flags: Vec::new(),
            written,
        }
    }
}

/// A fortress siege row write with Java's statement (inside [`ResidenceWrite::FortSiege`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FortSiegeWrite {
    /// `FortSiege.saveSiegeDate`: `UPDATE fort SET siegeDate = ? WHERE id = ?`.
    SiegeDate { fort_id: i32, siege_date: i64 },
    /// `FortSiege.saveSiegeClan`: `INSERT INTO fortsiege_clans (clan_id,fort_id) values (?,?)`.
    AddAttacker { fort_id: i32, clan_id: i32 },
    /// `FortSiege.removeSiegeClan`: the clan's row, or every row of the fortress for clan 0.
    RemoveAttackers { fort_id: i32, clan_id: i32 },
    /// `FortSiege.clearSiegeClan`: every row of the fortress, and with an owner every row of the
    /// owner clan.
    ClearSiegeClans {
        fort_id: i32,
        owner_clan_id: Option<i32>,
    },
    /// `Clan.updateBloodOathCountInDB`: `UPDATE clan_data SET blood_oath_count=? WHERE clan_id=?`.
    BloodOathCount { clan_id: i32, count: i32 },
}

/// The siege of one fortress (Java `FortSiege`).
#[derive(Debug, Default)]
pub(crate) struct FortSiege {
    /// `_isInProgress`.
    pub(crate) in_progress: bool,
    /// `_attackerClans`.
    pub(crate) attackers: Vec<FortSiegeClan>,
    /// `_commanders`: the commanders of the siege (spawn indices).
    pub(crate) commanders: Vec<usize>,
    /// `_siegeEnd`.
    pub(crate) siege_end: Option<TimerId>,
    /// `_siegeRestore`.
    pub(crate) siege_restore: Option<TimerId>,
    /// `_siegeStartTask`. Java never nulls it when the task has run, and reads `!= null` as "the
    /// countdown has been started"; the id stays here too (cancelling a timer that ran is a
    /// no-op).
    pub(crate) siege_start_task: Option<TimerId>,
}

impl World {
    // ------------------------------------------------------------------ queries

    /// Java `FortSiege.isInProgress()`.
    pub fn fort_siege_in_progress(&self, fort: FortId) -> bool {
        self.fort.forts.get(&fort).is_some_and(|s| s.siege.in_progress)
    }

    /// Java `getAttackerClans()`: the registered clans (ids).
    pub fn fort_attacker_clans(&self, fort: FortId) -> Vec<i32> {
        self.fort
            .forts
            .get(&fort)
            .map(|s| s.siege.attackers.iter().map(|c| c.clan_id).collect())
            .unwrap_or_default()
    }

    /// Java `getAttackerClan(clanId)`.
    pub fn fort_attacker_clan(&self, fort: FortId, clan_id: i32) -> Option<&FortSiegeClan> {
        self.fort
            .forts
            .get(&fort)?
            .siege
            .attackers
            .iter()
            .find(|c| c.clan_id == clan_id)
    }

    /// Java `checkIsAttacker(clan)`.
    pub fn fort_check_is_attacker(&self, fort: FortId, clan_id: i32) -> bool {
        self.fort_attacker_clan(fort, clan_id).is_some()
    }

    /// Java `FortSiegeManager.checkIsRegistered(clan, fort)` for every fortress.
    pub fn fort_clan_is_registered(&self, clan_id: i32) -> bool {
        self.fort
            .forts
            .keys()
            .any(|fort| self.fort_check_is_attacker(*fort, clan_id))
    }

    /// Java `checkIsDefender(clan)`: the clan owns the fortress.
    pub fn fort_check_is_defender(&self, fort: FortId, clan_id: i32) -> bool {
        clan_id != 0 && self.fort(fort).is_some_and(|f| f.owner_id == clan_id)
    }

    /// Java `FortSiege.checkIfInZone(x, y, z)`: the siege runs and the place is in its zone.
    pub fn fort_check_if_in_zone(&self, fort: FortId, x: i32, y: i32, z: i32) -> bool {
        self.fort_siege_in_progress(fort) && self.fort_contains(fort, x, y, z)
    }

    /// Java `FortSiegeManager.getSiege(x, y, z)`: the fortress whose running siege holds the place.
    pub fn fort_siege_at(&self, x: i32, y: i32, z: i32) -> Option<FortId> {
        self.residences
            .forts
            .iter()
            .map(|f| f.id)
            .find(|id| self.fort_check_if_in_zone(*id, x, y, z))
    }

    /// Java `getFlag(clan)`: the headquarters flags of an attacker.
    pub fn fort_siege_flags(&self, fort: FortId, clan_id: i32) -> Vec<ObjectId> {
        self.fort_attacker_clan(fort, clan_id)
            .map(|c| c.flags.clone())
            .unwrap_or_default()
    }

    /// Java `SiegeClan.addFlag(flag)` of an attacker.
    pub fn fort_siege_add_flag(&mut self, fort: FortId, clan_id: i32, flag: ObjectId) -> bool {
        match self
            .fort
            .forts
            .get_mut(&fort)
            .and_then(|s| s.siege.attackers.iter_mut().find(|c| c.clan_id == clan_id))
        {
            Some(clan) => {
                clan.flags.push(flag);
                true
            }
            None => false,
        }
    }

    /// Java `FortSiege.killedFlag(flag)`: the first attacker that has the flag loses it.
    pub fn fort_siege_killed_flag(&mut self, fort: FortId, flag: ObjectId) {
        if let Some(state) = self.fort.forts.get_mut(&fort) {
            for clan in &mut state.siege.attackers {
                if let Some(index) = clan.flags.iter().position(|f| *f == flag) {
                    clan.flags.remove(index);
                    return;
                }
            }
        }
    }

    /// The number of commanders still alive in the siege (`getCommanders().size()`).
    pub fn fort_commanders_alive(&self, fort: FortId) -> usize {
        self.fort.forts.get(&fort).map_or(0, |s| s.siege.commanders.len())
    }

    // ------------------------------------------------------------------ rows

    fn fort_siege_write(&mut self, write: FortSiegeWrite) -> bool {
        self.siege_write(ResidenceWrite::FortSiege(write))
    }

    /// Java `saveSiegeDate()`.
    fn fort_save_siege_date(&mut self, fort: FortId) {
        let Some(siege_date) = self.fort(fort).map(|f| f.siege_date) else {
            return;
        };
        self.fort_siege_write(FortSiegeWrite::SiegeDate {
            fort_id: fort.0,
            siege_date,
        });
    }

    /// Java `saveFortSiege()`: "clear siege date", then save it.
    fn fort_save_fort_siege(&mut self, fort: FortId) {
        if let Some(f) = self.fort_mut(fort) {
            f.siege_date = 0;
        }
        self.fort_save_siege_date(fort);
    }

    /// Java `setSiegeDateTime(merchant)`.
    fn fort_set_siege_date_time(&mut self, fort: FortId, merchant: bool) {
        let minutes = if merchant {
            i64::from(self.fort.settings.siege.suspicious_merchant_respawn_delay)
        } else {
            60
        };
        if let Some(f) = self.fort_mut(fort) {
            f.siege_date = unix_ms_now() + minutes * 60_000;
        }
        self.fort_save_siege_date(fort);
    }

    // ------------------------------------------------------------------ announcements

    /// The members of the clans a siege speaks to: the attackers' first, then the owner's.
    fn fort_siege_audience(&self, fort: FortId) -> Vec<ObjectId> {
        let mut members = Vec::new();
        for clan_id in self.fort_attacker_clans(fort) {
            members.extend(self.clan_online_members(clan_id, 0));
        }
        if let Some(owner) = self.fort(fort).map(|f| f.owner_id).filter(|c| *c != 0) {
            members.extend(self.clan_online_members(owner, 0));
        }
        members
    }

    /// Java `announceToPlayer(sm)`: only the participants hear it.
    pub fn fort_siege_announce(&self, fort: FortId, message: &SystemMessage) {
        for member in self.fort_siege_audience(fort) {
            self.send(member, message);
        }
    }

    /// Java `updatePlayerSiegeStateFlags(clear)`.
    pub(crate) fn fort_update_player_siege_state_flags(&mut self, fort: FortId, clear: bool) {
        let mut groups: Vec<(u8, Vec<ObjectId>)> = Vec::new();
        for clan_id in self.fort_attacker_clans(fort) {
            groups.push((1, self.clan_online_members(clan_id, 0)));
        }
        if let Some(owner) = self.fort(fort).map(|f| f.owner_id).filter(|c| *c != 0) {
            groups.push((2, self.clan_online_members(owner, 0)));
        }
        let frequency = i64::from(self.fort.settings.fortress_zone_fame_task_frequency) * 1000;
        let points = self.fort.settings.fortress_zone_fame_aquire_points;
        for (state, members) in groups {
            for member in members {
                // The player's siege state is the one the castle sieges set too (M7-SIEGE-01).
                if clear {
                    self.set_siege_state(member, 0);
                    self.set_siege_side(member, 0);
                    self.set_in_siege(member, false);
                    self.fort_stop_fame_task(member);
                } else {
                    let in_zone = self
                        .players
                        .get(&member)
                        .is_some_and(|p| self.fort_check_if_in_zone(fort, p.core.x, p.core.y, p.core.z));
                    self.set_siege_state(member, state);
                    self.set_siege_side(member, fort.0);
                    if in_zone {
                        self.set_in_siege(member, true);
                        self.fort_start_fame_task(member, frequency, points);
                    }
                }
                self.broadcast_user_info(member);
            }
        }
    }

    /// The siege state of a player (`getSiegeState()`): 1 attacker, 2 defender, 0 nothing. One
    /// state for castle and fortress sieges, as in Java ([`World::siege_state_of`]).
    pub fn fort_siege_state_of(&self, player: ObjectId) -> u8 {
        self.siege_state_of(player)
    }

    /// `isInSiege()` ([`World::is_in_siege`]).
    pub fn fort_player_in_siege(&self, player: ObjectId) -> bool {
        self.is_in_siege(player)
    }

    // ------------------------------------------------------------------ fame

    /// Java `Player.startFameTask(delay, fameFixRate)`: the player's one fame task, shared with
    /// the castle sieges (`siege/player.rs`).
    pub(crate) fn fort_start_fame_task(&mut self, player: ObjectId, delay_ms: i64, points: i32) {
        self.start_fame_task(player, delay_ms, points);
    }

    /// Java `Player.stopFameTask()`.
    pub(crate) fn fort_stop_fame_task(&mut self, player: ObjectId) {
        self.stop_fame_task(player);
    }

    /// Forgets what the fortress sieges keep of a player who leaves the world.
    pub(crate) fn fort_player_left(&mut self, player: ObjectId) {
        // The siege state itself goes with `forget_player_siege_state` (M7-SIEGE-01).
        self.fort_stop_fame_task(player);
        // DEVIATION(stale reference): a flag whose bearer leaves the world without having dropped it
        // (the logout teleport put him out of the fortress first) keeps pointing at the old
        // `Player` object in Java; here the bearer is forgotten, so the next login starts clean.
        for flags in self.fort.flags.values_mut() {
            for flag in flags.iter_mut().filter(|f| f.player == Some(player)) {
                flag.player = None;
                flag.worn_item = None;
            }
        }
    }

    // ------------------------------------------------------------------ the tasks

    /// The fortress timers.
    pub(crate) fn on_fort_timer(&mut self, timer: FortTimer) {
        match timer {
            FortTimer::Updater { fort, kind } => self.on_fort_updater(fort, kind),
            FortTimer::StartSiege { fort, time } => self.on_fort_start_siege_task(fort, time),
            FortTimer::EndSiege(fort) => {
                // Java `ScheduleEndSiegeTask.run`.
                if !self.fort_siege_in_progress(fort) {
                    return;
                }
                self.fort.state_mut(fort).siege.siege_end = None;
                self.fort_end_siege(fort);
            }
            FortTimer::SiegeRestore(fort) => {
                // Java `ScheduleSiegeRestore.run`.
                if !self.fort_siege_in_progress(fort) {
                    return;
                }
                self.fort.state_mut(fort).siege.siege_restore = None;
                self.fort_reset_siege(fort);
                self.fort_siege_announce(
                    fort,
                    &SystemMessage::new(SystemMessageId::TheBarracksFunctionHasBeenRestored),
                );
            }
            FortTimer::SuspiciousMerchantSpawn(fort) => {
                // Java `ScheduleSuspiciousMerchantSpawn.run`.
                if self.fort_siege_in_progress(fort) {
                    return;
                }
                self.fort_spawn_suspicious_merchant(fort);
            }
            FortTimer::CommanderCanTalk(npc) => {
                self.fort.silenced_commanders.remove(&npc);
            }
        }
    }

    fn fort_schedule(&mut self, delay_ms: i64, timer: FortTimer) -> TimerId {
        let due = self.tick + TimerWheel::ms_to_ticks(u64::try_from(delay_ms).unwrap_or(0));
        self.timers.schedule_at(due, TimerKind::Fort(timer))
    }

    /// Java `ScheduleStartSiegeTask.run()` for a countdown of `time` seconds.
    fn on_fort_start_siege_task(&mut self, fort: FortId, time: i32) {
        // D-51: the Java game server runs the real sieges; the countdown of this world does
        // nothing while it only holds a copy of the registrations.
        if !self.siege_database_writes() {
            return;
        }
        if self.fort_siege_in_progress(fort) {
            return;
        }
        let minutes = |world: &World, n: i32| {
            world.fort_siege_announce(
                fort,
                &SystemMessage::new(SystemMessageId::S1MinuteSUntilTheFortressBattleStarts)
                    .with(SystemMessageParam::IntNumber(n)),
            );
        };
        let seconds = |world: &World, n: i32| {
            world.fort_siege_announce(
                fort,
                &SystemMessage::new(SystemMessageId::S1SecondSUntilTheFortressBattleStarts)
                    .with(SystemMessageParam::IntNumber(n)),
            );
        };
        // The next task is not kept in `_siegeStartTask` (Java only stores the first one).
        let then = |world: &mut World, time: i32, delay_ms: i64| {
            world.fort_schedule(delay_ms, FortTimer::StartSiege { fort, time });
        };
        match time {
            // 1hr remains: "Prepare task for 10 minutes left."
            3600 => then(self, 600, 3_000_000),
            // 10min remains.
            600 => {
                self.fort_despawn_suspicious_merchant(fort);
                minutes(self, 10);
                then(self, 300, 300_000);
            }
            300 => {
                minutes(self, 5);
                then(self, 60, 240_000);
            }
            60 => {
                minutes(self, 1);
                then(self, 30, 30_000);
            }
            30 => {
                seconds(self, 30);
                then(self, 10, 20_000);
            }
            10 => {
                seconds(self, 10);
                then(self, 5, 5000);
            }
            5 => {
                seconds(self, 5);
                then(self, 1, 4000);
            }
            1 => {
                seconds(self, 1);
                then(self, 0, 1000);
            }
            0 => self.fort_start_siege(fort),
            _ => warn!("Exception: ScheduleStartSiegeTask(): unknown siege time: {time}"),
        }
    }

    // ------------------------------------------------------------------ start and end

    /// Java `startSiege()`.
    pub fn fort_start_siege(&mut self, fort: FortId) {
        if self.fort_siege_in_progress(fort) {
            return;
        }
        // "used admin command admin_startfortsiege"
        if let Some(task) = self.fort.state_mut(fort).siege.siege_start_task {
            self.timers.cancel(task);
            self.fort_despawn_suspicious_merchant(fort);
        }
        self.fort.state_mut(fort).siege.siege_start_task = None;
        if self.fort.state_mut(fort).siege.attackers.is_empty() {
            return;
        }
        // Flag so that same siege instance cannot be started again.
        self.fort.state_mut(fort).siege.in_progress = true;
        // `loadSiegeClan()` reads the rows again; the registrations of this world are the ones
        // the siege uses (D-15: no database inside a tick).
        self.fort_update_player_siege_state_flags(fort, false);
        // Teleport to the closest town.
        self.fort_teleport_player(fort, FortTeleportWhoType::Attacker, TeleportWhereType::Town);
        self.fort_despawn_npc_commanders(fort);
        self.fort_spawn_siege_commanders(fort);
        self.fort_reset_doors(fort);
        self.fort_spawn_siege_guards(fort);
        self.fort_set_visible_flag(fort, false);
        self.fort_zone_set_active(fort, true);
        self.fort_zone_update_status(fort);
        // Schedule a task to prepare auto siege end.
        let length = i64::from(self.fort.settings.siege.siege_length) * 60 * 1000;
        let end = self.fort_schedule(length, FortTimer::EndSiege(fort));
        self.fort.state_mut(fort).siege.siege_end = Some(end);
        self.fort_siege_announce(
            fort,
            &SystemMessage::new(SystemMessageId::TheFortressBattleS1HasBegun)
                .with(SystemMessageParam::CastleName(fort.0)),
        );
        self.fort_save_fort_siege(fort);
        info!(
            "Siege of {} fort started.",
            self.residence_name(crate::residence::ResidenceRef::Fort(fort))
        );
        // `ON_FORT_SIEGE_START` listeners are scripts (D-05).
    }

    /// Java `endSiege()`.
    pub fn fort_end_siege(&mut self, fort: FortId) {
        if !self.fort_siege_in_progress(fort) {
            return;
        }
        // Flag so that siege instance can be started.
        self.fort.state_mut(fort).siege.in_progress = false;
        // Removes all flags. Note: Remove flag before teleporting players.
        self.fort_remove_siege_flags(fort);
        self.fort_unspawn_flags(fort);
        self.fort_update_player_siege_state_flags(fort, true);
        let owner = self.fort(fort).map(|f| f.owner_id).filter(|c| *c != 0);
        self.fort_zone_banish_foreigners(fort, owner.unwrap_or(-1));
        self.fort_zone_set_active(fort, false);
        self.fort_zone_update_status(fort);
        // Save fort specific data.
        self.fort_save_fort_siege(fort);
        // Clear siege clan from db.
        self.fort_clear_siege_clan(fort);
        // Remove commander from this fort.
        self.fort_remove_siege_commanders(fort);
        self.fort_spawn_npc_commanders(fort);
        // Remove all spawned siege guard from this fort.
        self.fort_unspawn_siege_guards(fort);
        // Respawn door to fort.
        self.fort_reset_doors(fort);
        // Prepare 3hr task for suspicious merchant respawn.
        let delay = i64::from(self.fort.settings.siege.suspicious_merchant_respawn_delay) * 60 * 1000;
        self.fort_schedule(delay, FortTimer::SuspiciousMerchantSpawn(fort));
        // Store suspicious merchant spawn in DB.
        self.fort_set_siege_date_time(fort, true);
        if let Some(task) = self.fort.state_mut(fort).siege.siege_end.take() {
            self.timers.cancel(task);
        }
        if let Some(task) = self.fort.state_mut(fort).siege.siege_restore.take() {
            self.timers.cancel(task);
        }
        let pole_hidden = self
            .fort(fort)
            .and_then(|f| f.flag_pole)
            .and_then(|pole| self.static_objects.get(&pole))
            .is_some_and(|o| o.mesh_index == 0);
        if owner.is_some() && pole_hidden {
            self.fort_set_visible_flag(fort, true);
        }
        info!(
            "Siege of {} fort finished.",
            self.residence_name(crate::residence::ResidenceRef::Fort(fort))
        );
        // `ON_FORT_SIEGE_FINISH` listeners are scripts (D-05).
    }

    /// Java `Fort.endOfSiege(clan)` (`endFortressSiege.run`): the fortress changes hands.
    pub fn fort_end_of_siege(&mut self, fort: FortId, clan_id: i32) {
        self.fort_set_owner(fort, clan_id, true);
    }

    /// `removeFlags()`: every headquarters flag of the attackers goes (`SiegeClan.removeFlags`).
    fn fort_remove_siege_flags(&mut self, fort: FortId) {
        let flags: Vec<ObjectId> = self
            .fort
            .forts
            .get_mut(&fort)
            .map(|s| {
                s.siege
                    .attackers
                    .iter_mut()
                    .flat_map(|c| std::mem::take(&mut c.flags))
                    .collect()
            })
            .unwrap_or_default();
        for flag in flags {
            self.delete_npc(flag);
        }
    }

    /// Java `Fort.resetDoors()`: the doors close, come back to life (`doRevive`) and are
    /// repaired (`setCurrentHp(getMaxHp())`, which tells everybody who sees them).
    pub fn fort_reset_doors(&mut self, fort: FortId) {
        let doors = self.fort(fort).map(|f| f.doors.clone()).unwrap_or_default();
        for door in doors {
            if self.doors.get(&door).is_some_and(|d| d.open) {
                self.close_door(door);
            }
            if self.is_dead(door) {
                self.do_revive(door);
            }
            let max = self.stats(door).map_or(0, |v| v.max_hp());
            if self.door(door).is_some_and(|d| d.current_hp() < f64::from(max)) {
                self.set_door_current_hp(door, f64::from(max));
            }
        }
        // `loadDoorUpgrade()` ("Check for any upgrade the doors may have").
        // NOTE(java-quirk): it selects `fort_doorupgrade` and reads a column `id` that table does
        // not have (it is `doorId`), and nothing inserts a row: it never does anything.
    }

    // ------------------------------------------------------------------ teleports

    /// Java `teleportPlayer(teleportWho, teleportWhere)`.
    pub(crate) fn fort_teleport_player(
        &mut self,
        fort: FortId,
        who: FortTeleportWhoType,
        where_to: TeleportWhereType,
    ) {
        let players: Vec<ObjectId> = match who {
            FortTeleportWhoType::Owner => {
                let owner = self.fort(fort).map(|f| f.owner_id).filter(|c| *c != 0);
                owner
                    .map(|clan| {
                        self.clan_online_members(clan, 0)
                            .into_iter()
                            .filter(|p| self.fort_player_in_siege(*p))
                            .collect()
                    })
                    .unwrap_or_default()
            }
            FortTeleportWhoType::Attacker => self
                .fort_attacker_clans(fort)
                .into_iter()
                .flat_map(|clan| self.clan_online_members(clan, 0))
                .filter(|p| self.fort_player_in_siege(*p))
                .collect(),
            FortTeleportWhoType::All => self.fort_players_in_zone(fort),
        };
        for player in players {
            if self.is_gm(player) || self.is_jailed(player) {
                continue;
            }
            self.tele_to_location_where(player, where_to);
        }
    }

    // ------------------------------------------------------------------ registration

    /// Java `loadSiegeClan()` for rows read from the database: the registrations of this world
    /// become those rows (a registration this world holds without a row stays).
    pub(crate) fn fort_apply_siege_clan_rows(&mut self, fort: FortId, rows: &[i32]) {
        let state = self.fort.state_mut(fort);
        let mut attackers: Vec<FortSiegeClan> = rows
            .iter()
            .map(|clan| {
                state
                    .siege
                    .attackers
                    .iter()
                    .find(|c| c.clan_id == *clan)
                    .cloned()
                    .map_or_else(
                        || FortSiegeClan::new(*clan, true),
                        |mut c| {
                            c.written = true;
                            c
                        },
                    )
            })
            .collect();
        attackers.extend(state.siege.attackers.iter().filter(|c| !c.written).cloned());
        state.siege.attackers = attackers;
    }

    /// Java `saveSiegeClan(clan)`: the registration row and the clan joins the attackers (at most
    /// `AttackerMaxClans`).
    fn fort_save_siege_clan(&mut self, fort: FortId, clan_id: i32) {
        let max = usize::try_from(self.fort.settings.siege.attacker_max_clans).unwrap_or(0);
        if self.fort_attacker_clans(fort).len() >= max {
            return;
        }
        let written = self.fort_siege_write(FortSiegeWrite::AddAttacker {
            fort_id: fort.0,
            clan_id,
        });
        self.fort
            .state_mut(fort)
            .siege
            .attackers
            .push(FortSiegeClan::new(clan_id, written));
    }

    /// Java `addAttacker(player, checkConditions)`. 0: no clan, 1: not enough adena, 2: not the
    /// right time, 3: already registered, 4: registered.
    pub fn fort_add_attacker(&mut self, fort: FortId, player: ObjectId, check_conditions: bool) -> i32 {
        let clan = self.clan_id_of(player);
        if self.clan(clan).is_none() {
            // Player does not have clan.
            return 0;
        }
        if check_conditions {
            if self.fort_attacker_clans(fort).is_empty() && self.adena_of(player) < REGISTRATION_FEE {
                // Player does not have enough adena to register.
                return 1;
            }
            // "Is not right time to register Fortress now": the Territory War's registration is
            // over before its start, or its channel is open after it (M7-TW-01).
            let now = self.tw_now_ms();
            let tw_start = self.tw_start_time_ms();
            if (now < tw_start && self.is_tw_registration_over())
                || (now > tw_start && self.is_tw_channel_open())
            {
                return 2;
            }
            let forts: Vec<FortId> = self.residences.forts.iter().map(|f| f.id).collect();
            for other in forts {
                if self.fort_check_is_attacker(other, clan) {
                    // Players clan is already registered to siege.
                    return 3;
                }
                let owner_busy = self.fort(other).is_some_and(|f| f.owner_id == clan)
                    && (self.fort_siege_in_progress(other)
                        || self
                            .fort
                            .forts
                            .get(&other)
                            .is_some_and(|s| s.siege.siege_start_task.is_some()));
                if owner_busy {
                    return 3;
                }
            }
        }
        self.fort_save_siege_clan(fort, clan);
        if self.fort_attacker_clans(fort).len() == 1 {
            if check_conditions {
                self.reduce_adena(player, REGISTRATION_FEE, true);
            }
            self.fort_start_auto_task(fort, true);
        }
        // Players clan is successfully registered to siege.
        4
    }

    /// Java `removeAttacker(clan)`.
    pub fn fort_remove_attacker(&mut self, fort: FortId, clan_id: i32) {
        let Some(clan) = self.clan(clan_id) else {
            return;
        };
        if clan.fort_id == fort.0 || !self.fort_check_is_attacker(fort, clan_id) {
            return;
        }
        self.fort_remove_siege_clan(fort, clan_id);
    }

    /// Java `removeSiegeClan(clanId)` (no checks): the row goes, the list is read again, and a
    /// siege without attackers is over.
    fn fort_remove_siege_clan(&mut self, fort: FortId, clan_id: i32) {
        self.fort_siege_write(FortSiegeWrite::RemoveAttackers {
            fort_id: fort.0,
            clan_id,
        });
        // `loadSiegeClan()`: the rows are what is left.
        let state = self.fort.state_mut(fort);
        if clan_id == 0 {
            state.siege.attackers.clear();
        } else {
            state.siege.attackers.retain(|c| c.clan_id != clan_id);
        }
        if !state.siege.attackers.is_empty() {
            return;
        }
        if state.siege.in_progress {
            self.fort_end_siege(fort);
        } else {
            // Clear siege time in DB.
            self.fort_save_fort_siege(fort);
        }
        if let Some(task) = self.fort.state_mut(fort).siege.siege_start_task.take() {
            self.timers.cancel(task);
        }
    }

    /// Java `clearSiegeClan()`: every registration of the fortress is deleted.
    pub fn fort_clear_siege_clan(&mut self, fort: FortId) {
        let owner = self.fort(fort).map(|f| f.owner_id).filter(|c| *c != 0);
        self.fort_siege_write(FortSiegeWrite::ClearSiegeClans {
            fort_id: fort.0,
            owner_clan_id: owner,
        });
        // NOTE(java-quirk): the owner's registrations at the other fortresses are deleted from
        // the table only; the other sieges keep the clan in their lists until they are loaded
        // again.
        self.fort.state_mut(fort).siege.attackers.clear();
        // If siege is in progress, end siege.
        if self.fort_siege_in_progress(fort) {
            self.fort_end_siege(fort);
        }
        // If siege isn't in progress (1hr waiting time till siege starts), cancel waiting time.
        if let Some(task) = self.fort.state_mut(fort).siege.siege_start_task.take() {
            self.timers.cancel(task);
        }
    }

    /// `ClanTable.destroyClan`: `siege.removeAttacker(clan)` of every fortress siege (a clan with a
    /// fortress is not registered anywhere). The rows go as `writes` says.
    pub(crate) fn fort_sieges_remove_attacker(&mut self, clan_id: i32, writes: Writes) {
        let forts: Vec<FortId> = self.fort.forts.keys().copied().collect();
        for fort in forts {
            if !self.fort_check_is_attacker(fort, clan_id) {
                continue;
            }
            if self.clan(clan_id).is_some_and(|c| c.fort_id == fort.0) {
                continue;
            }
            // `removeSiegeClan(clanId)`.
            let write = ResidenceWrite::FortSiege(FortSiegeWrite::RemoveAttackers {
                fort_id: fort.0,
                clan_id,
            });
            self.write_residence(writes, write);
            let state = self.fort.state_mut(fort);
            state.siege.attackers.retain(|c| c.clan_id != clan_id);
            if state.siege.attackers.is_empty() {
                if state.siege.in_progress {
                    self.fort_end_siege(fort);
                } else {
                    self.fort_save_fort_siege(fort);
                }
                if let Some(task) = self.fort.state_mut(fort).siege.siege_start_task.take() {
                    self.timers.cancel(task);
                }
            }
        }
    }

    // ------------------------------------------------------------------ the countdown

    /// Java `checkAutoTask()`: at startup, and for a fortress whose date has come.
    pub fn fort_check_auto_task(&mut self, fort: FortId) {
        if self.fort.state_mut(fort).siege.siege_start_task.is_some() {
            return;
        }
        let Some(siege_date) = self.fort(fort).map(|f| f.siege_date) else {
            return;
        };
        let delay = siege_date - unix_ms_now();
        if delay < 0 {
            // Siege time in past.
            self.fort_save_fort_siege(fort);
            // Remove all clans.
            self.fort_clear_siege_clan(fort);
            // Spawn suspicious merchant immediately.
            self.on_fort_timer(FortTimer::SuspiciousMerchantSpawn(fort));
            return;
        }
        // `loadSiegeClan()`: the registrations are in this world (the game server read them).
        if self.fort_attacker_clans(fort).is_empty() {
            // No attackers - waiting for suspicious merchant spawn.
            self.fort_schedule(delay, FortTimer::SuspiciousMerchantSpawn(fort));
            return;
        }
        // Preparing start siege task.
        let start = |world: &mut World, time: i32, after_ms: i64| {
            let task = world.fort_schedule(after_ms, FortTimer::StartSiege { fort, time });
            world.fort.state_mut(fort).siege.siege_start_task = Some(task);
        };
        // More than hour, how this can happens ? Spawn suspicious merchant.
        if delay > 3_600_000 {
            self.on_fort_timer(FortTimer::SuspiciousMerchantSpawn(fort));
            start(self, 3600, delay - 3_600_000);
        }
        // NOTE(java-quirk): this is an `if`, not an `else if`: with more than an hour left both
        // the 3600 and the 600 task are scheduled, and the countdown runs twice from 10 minutes
        // on (the announcements come twice; `startSiege` ignores the second call).
        if delay > 600_000 {
            // More than 10 min, spawn suspicious merchant.
            self.on_fort_timer(FortTimer::SuspiciousMerchantSpawn(fort));
            start(self, 600, delay - 600_000);
        } else if delay > 300_000 {
            start(self, 300, delay - 300_000);
        } else if delay > 60_000 {
            start(self, 60, delay - 60_000);
        } else {
            // Lower than 1 min, set to 1 min.
            start(self, 60, 0);
        }
        info!(
            "Siege of {} fort: {siege_date}",
            self.residence_name(crate::residence::ResidenceRef::Fort(fort))
        );
    }

    /// Java `startAutoTask(setTime)`: the first clan registered.
    pub fn fort_start_auto_task(&mut self, fort: FortId, set_time: bool) {
        if self.fort.state_mut(fort).siege.siege_start_task.is_some() {
            return;
        }
        if set_time {
            self.fort_set_siege_date_time(fort, false);
        }
        if let Some(owner) = self.fort(fort).map(|f| f.owner_id).filter(|c| *c != 0) {
            self.broadcast_to_clan(
                owner,
                &SystemMessage::new(SystemMessageId::AFortressIsUnderAttack),
            );
        }
        // Execute siege auto start.
        let task = self.fort_schedule(0, FortTimer::StartSiege { fort, time: 3600 });
        self.fort.state_mut(fort).siege.siege_start_task = Some(task);
    }

    // ------------------------------------------------------------------ commanders

    /// Java `spawnCommanders()`: the commanders of the siege stand at their places and do not
    /// respawn.
    fn fort_spawn_siege_commanders(&mut self, fort: FortId) {
        self.fort.state_mut(fort).siege.commanders.clear();
        let list = self.fort.commander_lists.get(&fort).cloned().unwrap_or_default();
        for (index, holder) in list.iter().enumerate() {
            // The spawn objects are made once and reused for every siege.
            let existing = self
                .fort
                .state_mut(fort)
                .spawns
                .commander_spawn_pool
                .get(index)
                .copied();
            let spawn = match existing {
                Some(spawn) => spawn,
                None => {
                    let spawn = self.fort_new_spawn(
                        holder.npc_id,
                        (holder.x, holder.y, holder.z),
                        holder.heading,
                        60,
                    );
                    self.fort.state_mut(fort).spawns.commander_spawn_pool.push(spawn);
                    spawn
                }
            };
            self.fort_do_spawn(spawn);
            self.stop_respawn(spawn);
            self.fort.state_mut(fort).siege.commanders.push(spawn);
        }
    }

    /// Java `removeCommanders()`.
    fn fort_remove_siege_commanders(&mut self, fort: FortId) {
        let commanders = std::mem::take(&mut self.fort.state_mut(fort).siege.commanders);
        for spawn in commanders {
            self.fort_despawn(spawn);
        }
    }

    /// Java `resetSiege()`: "Reload commanders and repair doors."
    pub(crate) fn fort_reset_siege(&mut self, fort: FortId) {
        self.fort_remove_siege_commanders(fort);
        self.fort_spawn_siege_commanders(fort);
        self.fort_reset_doors(fort);
    }

    /// Java `killedCommander(instance)`: a commander of the siege died.
    pub(crate) fn fort_killed_commander(&mut self, fort: FortId, npc: ObjectId) {
        if self.fort.state_mut(fort).siege.commanders.is_empty() {
            return;
        }
        let Some(spawn) = self.npcs.get(&npc).and_then(|n| n.spawn) else {
            let npc_id = self.npcs.get(&npc).map_or(0, |n| n.npc_id);
            warn!(
                "FortSiege.killedCommander(): killed commander, but commander not registered for fortress. NpcId: {npc_id} FortId: {}",
                fort.0
            );
            return;
        };
        let npc_id = self.npcs.get(&npc).map_or(0, |n| n.npc_id);
        let spawn_npc_id = self.spawn_def(spawn).map_or(0, |s| s.npc_id);
        let list = self.fort.commander_lists.get(&fort).cloned().unwrap_or_default();
        for holder in list.iter().filter(|h| h.npc_id == spawn_npc_id) {
            let string = match holder.message_id {
                1 => Some(NpcStringId::YouMayHaveBrokenOurArrowsButYouWillNeverBreakOurWillArchersRetreat),
                2 => Some(NpcStringId::AiieeeeCommandCenterThisIsGuardUnitWeNeedBackupRightAway),
                3 => Some(NpcStringId::AtLastTheMagicFieldThatProtectsTheFortressHasWeakenedVolunteersStandBack),
                4 => Some(
                    NpcStringId::IFeelSoMuchGriefThatICanTEvenTakeCareOfMyselfThereIsnTAnyReasonForMeToStayHereAnyLonger,
                ),
                _ => None,
            };
            if let Some(string) = string {
                self.broadcast_packet(
                    npc,
                    &NpcSay {
                        object_id: npc.0,
                        text_type: ChatType::NpcShout as i32,
                        npc_id,
                        npc_string: string as i32,
                        text: String::new(),
                        parameters: Vec::new(),
                    },
                );
            }
        }
        let state = self.fort.state_mut(fort);
        state.siege.commanders.retain(|c| *c != spawn);
        if state.siege.commanders.is_empty() {
            // Spawn fort flags.
            self.fort_spawn_flags(fort);
            // Cancel door/commanders respawn.
            if let Some(task) = self.fort.state_mut(fort).siege.siege_restore {
                self.timers.cancel(task);
            }
            // Open doors in main building.
            // TODO(java): this also opens the control room door of a big fort.
            let doors = self.fort(fort).map(|f| f.doors.clone()).unwrap_or_default();
            for door in doors {
                if self.doors.get(&door).is_some_and(|d| d.template.show_hp) {
                    continue;
                }
                self.open_door(door);
            }
            self.fort_siege_announce(fort, &SystemMessage::new(SystemMessageId::AllBarracksAreOccupied));
        } else {
            self.fort_siege_announce(
                fort,
                &SystemMessage::new(SystemMessageId::TheBarracksHaveBeenSeized),
            );
            if self.fort.state_mut(fort).siege.siege_restore.is_none() {
                let delay = i64::from(self.fort.settings.siege.count_down_length) * 60 * 1000;
                let task = self.fort_schedule(delay, FortTimer::SiegeRestore(fort));
                self.fort.state_mut(fort).siege.siege_restore = Some(task);
            }
        }
    }
}
