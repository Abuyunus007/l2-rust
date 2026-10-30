//! The war period: the next war date, the start and end tasks with their messages, the war
//! itself (the castle and fortress zones, the war NPCs, the wards), the players' war state, the
//! battlefield channel and the online reward.
// Java: managers/TerritoryWarManager.java (setNextTWDate, setTWStartTimeInMillis,
//       ScheduleStartTWTask, ScheduleEndTWTask, closeTerritoryChannelTask,
//       RewardOnlineParticipants, startTerritoryWar, endTerritoryWar, updatePlayerTWStateFlags,
//       announceToParticipants, Territory.changeNPCsSpawn, territoryCatapultDestroyed)

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use tracing::{info, warn};

use super::{
    TERRITORY_ID_BASE, TerritoryThen, TerritoryTimer, TerritoryWard, TerritoryWrite, WARD_ITEM_BASE,
};
use crate::residence::{CastleId, FortId, ResidenceRef};
use crate::siege::{SiegeZoneSiege, next_siege_date};
use crate::world::World;

/// `Calendar.SUNDAY` and `Calendar.SATURDAY`.
const SUNDAY: i32 = 1;
const SATURDAY: i32 = 7;

const MINUTE_MS: i64 = 60_000;
const HOUR_MS: i64 = 60 * MINUTE_MS;

impl World {
    // ------------------------------------------------------------------ the date

    /// Java `setNextTWDate()`: the date of the global variable when it is still ahead, else the
    /// Saturday 20:00 (the Sunday 22:00 when no castle has an owner) of this week, two weeks on
    /// when that is over; the new date is kept in the variable. Then the start task is armed.
    pub(crate) fn tw_set_next_tw_date(&mut self) {
        let now = self.tw_now_ms();
        let next = self.territory.next_tw_start_date;
        let date = if next > now {
            next
        } else {
            let has_owned_castle = self.has_owned_castle();
            let (day, hour) = if has_owned_castle {
                (SATURDAY, 20)
            } else {
                (SUNDAY, 22)
            };
            let offset = |t| self.siege_offset_at(t);
            // `cal = now`, `set(DAY_OF_WEEK, day)`, `HOUR_OF_DAY`, minute and second 0 (the
            // milliseconds stay), and `add(WEEK_OF_YEAR, 2)` when that is before now.
            let date = next_siege_date(now, now, day, hour, 2, &offset);
            self.territory.next_tw_start_date = date;
            self.territory_write(TerritoryWrite::NextTwStartDate(date));
            date
        };
        self.tw_set_start_time(date);
    }

    /// Java `setTWStartTimeInMillis(time)`: the new start, and the end task (during the war) or
    /// the start task runs again in a second.
    pub fn tw_set_start_time(&mut self, time: i64) {
        self.territory.start_tw_date = time;
        if self.territory.in_progress {
            if let Some(old) = self.territory.end_task.take() {
                self.timers.cancel(old);
            }
            self.territory.end_task = Some(self.schedule_territory_timer(TerritoryTimer::EndTw, 1000));
        } else {
            if let Some(old) = self.territory.start_task.take() {
                self.timers.cancel(old);
            }
            self.territory.start_task = Some(self.schedule_territory_timer(TerritoryTimer::StartTw, 1000));
        }
    }

    // ------------------------------------------------------------------ ScheduleStartTWTask

    fn tw_rearm_start(&mut self, delay_ms: i64) {
        self.territory.start_task = Some(self.schedule_territory_timer(TerritoryTimer::StartTw, delay_ms));
    }

    /// The countdown messages and flags of the start task between 20 minutes and the start.
    fn tw_countdown(&mut self, message: SystemMessageId) {
        self.broadcast_to_all_online_players(&SystemMessage::new(message));
        self.territory.channel_open = true;
        self.territory.registration_over = true;
        self.tw_update_player_state_flags(false);
    }

    /// Java `ScheduleStartTWTask.run()`.
    pub(crate) fn on_tw_start_task(&mut self) {
        self.territory.start_task = None;
        let remaining = self.territory.start_tw_date - self.tw_now_ms();
        if remaining > 7_200_000 {
            self.territory.registration_over = false;
            // Prepare task for 2h before TW start to end registration.
            self.tw_rearm_start(remaining - 7_200_000);
        } else if remaining > 1_200_000 {
            self.broadcast_to_all_online_players(&SystemMessage::new(
                SystemMessageId::TheTerritoryWarRequestPeriodHasEnded,
            ));
            self.territory.registration_over = true;
            // Prepare task for 20 mins left before TW start.
            self.tw_rearm_start(remaining - 1_200_000);
        } else if remaining > 600_000 {
            self.tw_countdown(
                SystemMessageId::TheTerritoryWarWillBeginIn20MinutesTerritoryRelatedFunctionsIEBattlefieldChannelDisguiseScrollsTransformationsEtcCanNowBeUsed,
            );
            // Prepare task for 10 mins left before TW start.
            self.tw_rearm_start(remaining - 600_000);
        } else if remaining > 300_000 {
            self.tw_countdown(SystemMessageId::TheTerritoryWarBeginsIn10Minutes);
            // Prepare task for 5 mins left before TW start.
            self.tw_rearm_start(remaining - 300_000);
        } else if remaining > 60_000 {
            self.tw_countdown(SystemMessageId::TheTerritoryWarBeginsIn5Minutes);
            // Prepare task for 1 min left before TW start.
            self.tw_rearm_start(remaining - 60_000);
        } else if remaining > 0 {
            self.tw_countdown(SystemMessageId::TheTerritoryWarBeginsIn1Minute);
            // Prepare task for TW start.
            self.tw_rearm_start(remaining);
        } else if remaining + self.territory.settings.war_length_ms > 0 {
            self.territory.channel_open = true;
            self.territory.registration_over = true;
            if self.siege_database_writes() || self.territory.gm_start {
                self.territory.gm_start = false;
                self.tw_start_territory_war();
                // Prepare task for TW end.
                self.territory.end_task = Some(self.schedule_territory_timer(TerritoryTimer::EndTw, 1000));
                self.territory.reward_task =
                    Some(self.schedule_territory_timer(TerritoryTimer::RewardOnline, 60_000));
            } else {
                // D-51: the Java game server runs the real war. This world reads the rows again
                // once that war and its channel (ten minutes after its end) are over.
                info!(
                    "SideBySide: the Territory War runs on the Java game server (SiegeDatabaseWrites = False)."
                );
                let until_closed = remaining + self.territory.settings.war_length_ms + 600_000;
                self.schedule_territory_timer(TerritoryTimer::SideBySideReread, until_closed);
            }
        }
        // NOTE(java-quirk): a start date more than `WarLength` in the past arms nothing; the
        // schedule stays stopped until a GM or a restart sets a new date.
    }

    // ------------------------------------------------------------------ ScheduleEndTWTask

    /// Java `ScheduleEndTWTask.run()`.
    pub(crate) fn on_tw_end_task(&mut self) {
        self.territory.end_task = None;
        let remaining =
            self.territory.start_tw_date + self.territory.settings.war_length_ms - self.tw_now_ms();
        let minutes = |ms: i64| i32::try_from(ms / 60_000).unwrap_or(i32::MAX);
        let int = |id: SystemMessageId, value: i32| {
            SystemMessage::new(id).with(SystemMessageParam::IntNumber(value))
        };
        let next = if remaining > 3_600_000 {
            let sm = int(SystemMessageId::TheTerritoryWarWillEndInS1HourS, 2);
            self.tw_announce_to_participants(&sm, 0, 0);
            // Prepare task for 1 hr left.
            Some(remaining - 3_600_000)
        } else if remaining > 600_000 {
            let sm = int(
                SystemMessageId::TheTerritoryWarWillEndInS1MinuteS,
                minutes(remaining),
            );
            self.tw_announce_to_participants(&sm, 0, 0);
            // Prepare task for 10 minute left.
            Some(remaining - 600_000)
        } else if remaining > 300_000 {
            let sm = int(
                SystemMessageId::TheTerritoryWarWillEndInS1MinuteS,
                minutes(remaining),
            );
            self.tw_announce_to_participants(&sm, 0, 0);
            // Prepare task for 5 minute left.
            Some(remaining - 300_000)
        } else if remaining > 10_000 {
            let sm = int(
                SystemMessageId::TheTerritoryWarWillEndInS1MinuteS,
                minutes(remaining),
            );
            self.tw_announce_to_participants(&sm, 0, 0);
            // Prepare task for 10 seconds count down.
            Some(remaining - 10_000)
        } else if remaining > 0 {
            let seconds = i32::try_from(remaining / 1000).unwrap_or(i32::MAX);
            let sm = int(SystemMessageId::S1SecondSToTheEndOfTerritoryWar, seconds);
            self.tw_announce_to_participants(&sm, 0, 0);
            // Prepare task for second count down.
            Some(remaining)
        } else {
            self.tw_end_territory_war();
            self.schedule_territory_timer(TerritoryTimer::CloseChannel, 600_000);
            None
        };
        if let Some(delay) = next {
            self.territory.end_task = Some(self.schedule_territory_timer(TerritoryTimer::EndTw, delay));
        }
    }

    /// Java `closeTerritoryChannelTask.run()`.
    pub(crate) fn on_tw_close_channel(&mut self) {
        self.territory.channel_open = false;
        self.territory.disguised.clear();
        self.tw_update_player_state_flags(true);
    }

    /// Java `RewardOnlineParticipants.run()`: a minute of war for every player on a side; the
    /// task ends with the war.
    pub(crate) fn on_tw_reward_online(&mut self) {
        self.territory.reward_task = None;
        if !self.territory.in_progress {
            return;
        }
        let mut players: Vec<ObjectId> = self.players.keys().copied().collect();
        players.sort_unstable();
        for player in players {
            if self.siege_side_of(player) > 0 {
                self.territory_give_point(player, 1000, 6);
            }
        }
        self.territory.reward_task =
            Some(self.schedule_territory_timer(TerritoryTimer::RewardOnline, 60_000));
    }

    /// Side by side (D-51): Sieghardt's war is over. The channel closes like Java's
    /// `closeTerritoryChannelTask`, then the rows are read again and the schedule starts from the
    /// date read.
    pub(crate) fn on_tw_side_by_side_reread(&mut self) {
        self.on_tw_close_channel();
        self.territory_request_reread(TerritoryThen::RestartSchedule);
    }

    // ------------------------------------------------------------------ start and end

    /// The territories whose castle has an owner (`startTerritoryWar` / `endTerritoryWar`).
    fn tw_active_territories(&self) -> Vec<i32> {
        let mut active = Vec::new();
        for t in self.territory.territories.values() {
            match self.castle(CastleId(t.castle_id)) {
                Some(castle) if castle.owner_id > 0 => active.push(t.castle_id),
                Some(_) => {}
                None => warn!("TerritoryWarManager: Castle missing! CastleId: {}", t.castle_id),
            }
        }
        active
    }

    /// A castle's siege zone gets the war as its siege (`setSiegeInstance(this)`,
    /// `setActive`, `updateZoneStatusForCharactersInside`), or loses it.
    fn tw_castle_zone(&mut self, castle: CastleId, active: bool) {
        let Some(zone) = self.castle(castle).and_then(|c| c.siege_zone) else {
            return;
        };
        let settings = self.siege_settings();
        // `TerritoryWarManager.giveFame()` is true, with the castle zone's frequency and amount.
        let fame = (settings.castle_zone_fame_task_frequency > 0).then_some((
            settings.castle_zone_fame_task_frequency,
            settings.castle_zone_fame_aquire_points,
        ));
        if active {
            self.set_siege_zone_siege(
                zone,
                Some(SiegeZoneSiege {
                    siegable: ResidenceRef::Castle(castle),
                    fame,
                }),
            );
            self.set_siege_zone_active(zone, true);
        } else {
            self.set_siege_zone_active(zone, false);
            self.set_siege_zone_siege(zone, None);
        }
        self.castle_doors_set_enemy(castle, active);
    }

    /// A fortress's siege zone gets the war as its siege, or loses it.
    fn tw_fort_zone(&mut self, fort: FortId, active: bool) {
        if active {
            self.territory.fort_zones.insert(fort.0);
            self.fort_zone_set_active(fort, true);
            self.fort_zone_update_status(fort);
        } else {
            self.fort_zone_set_active(fort, false);
            self.fort_zone_update_status(fort);
            self.territory.fort_zones.remove(&fort.0);
        }
    }

    /// Java `startTerritoryWar()`.
    pub(crate) fn tw_start_territory_war(&mut self) {
        let active = self.tw_active_territories();
        if active.len() < 2 {
            return;
        }
        self.territory.in_progress = true;
        self.tw_update_player_state_flags(false);
        for castle_id in &active {
            let fort_id = self.territory.territories.get(castle_id).map_or(0, |t| t.fort_id);
            let castle = CastleId(*castle_id);
            if self.castle(castle).is_some() {
                self.territory_change_npcs_spawn(*castle_id, 2, true);
                // Spawn door.
                self.castle_spawn_door(castle, false);
                self.tw_castle_zone(castle, true);
            } else {
                warn!("TerritoryWarManager: Castle missing! CastleId: {castle_id}");
            }
            let fort = FortId(fort_id);
            if self.fort(fort).is_some() {
                self.territory_change_npcs_spawn(*castle_id, 1, true);
                // Spawn door.
                self.fort_reset_doors(fort);
                self.tw_fort_zone(fort, true);
            } else {
                warn!("TerritoryWarManager: Fort missing! FortId: {fort_id}");
            }
            // The wards of the territory become the war's.
            let has_owner = self
                .territory
                .territories
                .get(castle_id)
                .is_some_and(|t| t.owner_clan.is_some());
            let places = self
                .territory
                .territories
                .get(castle_id)
                .map_or(0, |t| t.ward_places.len());
            for index in 0..places {
                let Some(place) = self
                    .territory
                    .territories
                    .get(castle_id)
                    .and_then(|t| t.ward_places.get(index))
                    .cloned()
                else {
                    continue;
                };
                if place.npc.is_none() || !has_owner {
                    continue;
                }
                if !place.npc.is_some_and(|n| self.npcs.contains_key(&n)) {
                    // `ward.getNpc().getSpawn().doSpawn(false)`.
                    self.territory_respawn_ward_place(*castle_id, index);
                }
                let npc = self
                    .territory
                    .territories
                    .get(castle_id)
                    .and_then(|t| t.ward_places.get(index))
                    .and_then(|p| p.npc);
                self.territory.wards.push(TerritoryWard {
                    territory_id: place.npc_id,
                    item_id: place.npc_id + WARD_ITEM_BASE,
                    owner_castle_id: *castle_id,
                    location: (place.x, place.y, place.z, 0),
                    old_location: None,
                    player: None,
                    item: None,
                    npc,
                });
            }
            if let Some(t) = self.territory.territories.get_mut(castle_id) {
                // Killed NPCs and captured wards.
                t.quest_done = [0, 0];
            }
        }
        self.territory.participant_points.clear();
        if self.territory.settings.return_wards_when_tw_starts {
            self.tw_return_wards();
        }
        self.broadcast_to_all_online_players(&SystemMessage::new(SystemMessageId::TerritoryWarHasBegun));
    }

    /// `RETURN_WARDS_WHEN_TW_STARTS`: every ward held by another castle goes home.
    fn tw_return_wards(&mut self) {
        let count = self.territory.wards.len();
        for index in 0..count {
            let Some(ward) = self.territory.wards.get(index).cloned() else {
                continue;
            };
            let home = ward.territory_id - TERRITORY_ID_BASE;
            if ward.owner_castle_id == home {
                continue;
            }
            self.territory_ward_unspawn_me(index);
            let npc = self.territory_add_ward(ward.territory_id, home, ward.owner_castle_id, false);
            if let Some(w) = self.territory.wards.get_mut(index) {
                w.npc = npc;
                w.owner_castle_id = home;
            }
        }
    }

    /// Java `endTerritoryWar()`.
    pub(crate) fn tw_end_territory_war(&mut self) {
        self.territory.in_progress = false;
        let active = self.tw_active_territories();
        self.tw_update_player_state_flags(true);
        let count = self.territory.wards.len();
        for index in 0..count {
            self.territory_ward_unspawn_me(index);
        }
        self.territory.wards.clear();
        if active.len() > 1 {
            for castle_id in &active {
                let fort_id = self.territory.territories.get(castle_id).map_or(0, |t| t.fort_id);
                let castle = CastleId(*castle_id);
                if self.castle(castle).is_some() {
                    self.castle_spawn_door(castle, false);
                    self.territory_change_npcs_spawn(*castle_id, 2, false);
                    self.tw_castle_zone(castle, false);
                } else {
                    warn!("TerritoryWarManager: Castle missing! CastleId: {castle_id}");
                }
                let fort = FortId(fort_id);
                if self.fort(fort).is_some() {
                    self.territory_change_npcs_spawn(*castle_id, 1, false);
                    self.tw_fort_zone(fort, false);
                } else {
                    warn!("TerritoryWarManager: Fort missing! FortId: {fort_id}");
                }
                // NOTE(java-quirk): `t.getHQ().deleteMe()` leaves the territory's `_territoryHQ`
                // set, so the next war's outpost condition still finds it.
                if let Some(hq) = self.territory.territories.get(castle_id).and_then(|t| t.hq) {
                    self.delete_npc(hq);
                }
                let spawn_wards = self.territory.settings.spawn_wards_when_tw_is_not_in_progress;
                let places = self
                    .territory
                    .territories
                    .get(castle_id)
                    .map_or(0, |t| t.ward_places.len());
                for index in 0..places {
                    let Some(npc) = self
                        .territory
                        .territories
                        .get(castle_id)
                        .and_then(|t| t.ward_places.get(index))
                        .and_then(|p| p.npc)
                    else {
                        continue;
                    };
                    let spawned = self.npcs.contains_key(&npc);
                    if !spawned && spawn_wards {
                        self.territory_respawn_ward_place(*castle_id, index);
                    } else if spawned && !spawn_wards {
                        // `decayMe()`: the place keeps the NPC.
                        self.delete_npc(npc);
                    }
                }
            }
        }
        let flags: Vec<ObjectId> = self.territory.clan_flags.values().copied().collect();
        for flag in flags {
            self.delete_npc(flag);
        }
        self.territory.clan_flags.clear();
        // NOTE(java-quirk): the rows of the registrations go, the lists in memory stay (until a
        // restart, or here the next read of the rows).
        let clans: Vec<(i32, i32)> = self
            .territory
            .registered_clans
            .iter()
            .flat_map(|(c, list)| list.iter().map(move |r| (*c, r.id)))
            .collect();
        let mercs: Vec<(i32, i32)> = self
            .territory
            .registered_mercenaries
            .iter()
            .flat_map(|(c, list)| list.iter().map(move |r| (*c, r.id)))
            .collect();
        for (castle_id, registered_id) in clans.into_iter().chain(mercs) {
            self.territory_write(TerritoryWrite::RegistrationDelete {
                castle_id,
                registered_id,
            });
        }
        // Change next TW date.
        self.tw_set_next_tw_date();
        self.broadcast_to_all_online_players(&SystemMessage::new(SystemMessageId::TerritoryWarHasEnded));
    }

    // ------------------------------------------------------------------ the players

    /// Java `updatePlayerTWStateFlags(clear)`: the registered clans' and mercenaries' and the
    /// owners' online members get their war side (and the attacker state during the war), or
    /// lose them.
    pub(crate) fn tw_update_player_state_flags(&mut self, clear: bool) {
        // `twQuest.setOnEnterWorld(_isTWInProgress)` of `TerritoryWarSuperClass` (S-TW-01): the
        // script "TerritoryWarSuperClass" (when installed) hears it as an event.
        let event = if self.territory.in_progress {
            "setOnEnterWorld 1"
        } else {
            "setOnEnterWorld 0"
        };
        if let Some(script) = self.scripts.by_name("TerritoryWarSuperClass") {
            self.script_notify_event(script, event, None, None);
        }
        let min_level = self.territory.settings.player_min_level;
        let clans: Vec<(i32, i32)> = self
            .territory
            .registered_clans
            .iter()
            .flat_map(|(c, list)| list.iter().map(move |r| (*c, r.id)))
            .collect();
        for (castle_id, clan) in clans {
            for player in self.clan_online_members(clan, 0) {
                self.tw_update_player_state(player, castle_id, clear, Some(min_level));
            }
        }
        let mercs: Vec<(i32, i32)> = self
            .territory
            .registered_mercenaries
            .iter()
            .flat_map(|(c, list)| list.iter().map(move |r| (*c, r.id)))
            .collect();
        for (castle_id, id) in mercs {
            let player = ObjectId(id);
            if !self.players.contains_key(&player) {
                continue;
            }
            // The mercenaries are not checked for their level here.
            self.tw_update_player_state(player, castle_id, clear, None);
        }
        let owners: Vec<(i32, i32)> = self
            .territory
            .territories
            .values()
            .filter_map(|t| t.owner_clan.map(|c| (t.castle_id, c)))
            .collect();
        for (castle_id, clan) in owners {
            for player in self.clan_online_members(clan, 0) {
                self.tw_update_player_state(player, castle_id, clear, Some(min_level));
            }
        }
    }

    fn tw_update_player_state(
        &mut self,
        player: ObjectId,
        castle_id: i32,
        clear: bool,
        min_level: Option<i32>,
    ) {
        if clear {
            self.set_siege_state(player, 0);
            if !self.territory.channel_open {
                self.set_siege_side(player, 0);
            }
        } else {
            if let Some(min_level) = min_level
                && (self.tw_level_of(player) < min_level || self.tw_class_level_of(player) < 2)
            {
                return;
            }
            if self.territory.in_progress {
                self.set_siege_state(player, 1);
            }
            self.set_siege_side(player, TERRITORY_ID_BASE + castle_id);
        }
        self.broadcast_user_info(player);
    }

    /// `player.getLevel()`.
    pub(crate) fn tw_level_of(&self, player: ObjectId) -> i32 {
        self.players.get(&player).map_or(0, |p| p.core.level)
    }

    /// `player.getPlayerClass().level()`.
    pub(crate) fn tw_class_level_of(&self, player: ObjectId) -> i32 {
        self.players
            .get(&player)
            .and_then(|p| u16::try_from(p.data.class_id).ok())
            .map_or(0, |c| l2_data::ClassId(c).class_level())
    }

    /// Java `announceToParticipants(sm, exp, sp)`: the owners' and the registered clans' online
    /// members and the mercenaries (not of a registered clan) get the packet, and the exp and SP.
    pub(crate) fn tw_announce_to_participants(&mut self, packet: &impl l2_net::Encode, exp: i64, sp: i64) {
        let bytes = packet.to_bytes();
        let send = |world: &mut World, player: ObjectId| {
            world.send_bytes(player, bytes.clone());
            if exp > 0 || sp > 0 {
                world.add_exp_and_sp(player, exp as f64, sp as f64, false);
            }
        };
        // Broadcast to clan members.
        let owners: Vec<i32> = self
            .territory
            .territories
            .values()
            .filter_map(|t| t.owner_clan)
            .collect();
        for clan in owners {
            for member in self.clan_online_members(clan, 0) {
                send(self, member);
            }
        }
        let clans: Vec<i32> = self
            .territory
            .registered_clans
            .values()
            .flatten()
            .map(|r| r.id)
            .collect();
        for clan in clans {
            for member in self.clan_online_members(clan, 0) {
                send(self, member);
            }
        }
        // Broadcast to mercenaries.
        let mercs: Vec<i32> = self
            .territory
            .registered_mercenaries
            .values()
            .flatten()
            .map(|r| r.id)
            .collect();
        for id in mercs {
            let player = ObjectId(id);
            if !self.players.contains_key(&player) {
                continue;
            }
            let clan = self.clan_of(player).map(|c| c.id);
            if clan.is_none_or(|c| !self.territory_check_clan_registered(-1, Some(c))) {
                send(self, player);
            }
        }
    }

    // ------------------------------------------------------------------ the war NPCs

    /// Java `Territory.changeNPCsSpawn(type, isSpawn)`: the NPCs of one type come or go.
    pub(crate) fn territory_change_npcs_spawn(&mut self, castle_id: i32, spawn_type: i32, spawn: bool) {
        if !(0..=3).contains(&spawn_type) {
            warn!("Territory: wrong type({spawn_type}) for NPCs spawn change!");
            return;
        }
        let count = self
            .territory
            .territories
            .get(&castle_id)
            .map_or(0, |t| t.spawn_list.len());
        for index in 0..count {
            let Some(s) = self
                .territory
                .territories
                .get(&castle_id)
                .and_then(|t| t.spawn_list.get(index))
                .cloned()
            else {
                continue;
            };
            if s.spawn_type != spawn_type {
                continue;
            }
            // `setNPC(npc)` deletes the NPC that was there.
            if let Some(old) = s.npc {
                self.delete_npc(old);
            }
            let npc = if spawn {
                // `spawnNPC(npcId, location)`: a spawn that does not respawn.
                self.spawn_npc_at(s.npc_id, s.x, s.y, s.z, s.heading)
            } else {
                None
            };
            if spawn && npc.is_none() {
                warn!("TerritoryWarManager: could not spawn NPC {}", s.npc_id);
            }
            if let Some(slot) = self
                .territory
                .territories
                .get_mut(&castle_id)
                .and_then(|t| t.spawn_list.get_mut(index))
            {
                slot.npc = npc;
            }
        }
    }

    /// Java `territoryCatapultDestroyed(castleId)`: the castle NPCs of the war go and the castle's
    /// doors open (the "For the Sake of the Territory" quests call it, D-05).
    pub fn territory_catapult_destroyed(&mut self, castle_id: i32) {
        if self.territory.territories.contains_key(&castle_id) {
            self.territory_change_npcs_spawn(castle_id, 2, false);
        }
        let doors = self
            .castle(CastleId(castle_id))
            .map(|c| c.doors.clone())
            .unwrap_or_default();
        for door in doors {
            self.open_door(door);
        }
    }

    // ------------------------------------------------------------------ the mercenary captains

    /// `MercenaryCaptain`'s constructor: the hourly shout of each captain of an owned territory.
    pub(crate) fn territory_start_captain_timers(&mut self) {
        let captains: Vec<ObjectId> = self
            .territory
            .territories
            .values()
            .filter(|t| t.owner_clan.is_some())
            .flat_map(|t| t.spawn_list.iter())
            .filter(|s| crate::residence::npc::MERCENARY_CAPTAINS.contains(&s.npc_id))
            .filter_map(|s| s.npc)
            .collect();
        for npc in captains {
            let timer = self.schedule_territory_timer(TerritoryTimer::CaptainSay(npc), HOUR_MS);
            self.territory.captain_timers.insert(npc, timer);
        }
    }

    /// The repeating `say` quest timer of a captain.
    pub(crate) fn on_captain_say_timer(&mut self, npc: ObjectId) {
        self.territory.captain_timers.remove(&npc);
        if !self.npcs.contains_key(&npc) {
            return;
        }
        let timer = self.schedule_territory_timer(TerritoryTimer::CaptainSay(npc), HOUR_MS);
        self.territory.captain_timers.insert(npc, timer);
        self.captain_say(npc);
    }
}
