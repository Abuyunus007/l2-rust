//! Registering for a castle siege: the attacker and defender lists, their `siege_clans` rows, the
//! lord's approval of defenders, the siege window packets and the re-read of the registrations
//! (D-50).
// Java: mechanics/siege/Siege.java (registerAttacker, registerDefender, checkIfCanRegister,
//       checkIfAlreadyRegisteredForSameDay, saveSiegeClan, loadSiegeClan, removeSiegeClan,
//       approveSiegeDefenderClan, clearSiegeClan, clearSiegeWaitingClan, listRegisterClan)
// Java: managers/SiegeManager.java (checkIsRegistered)
// Java: network/clientpackets/{RequestJoinSiege,RequestConfirmSiegeWaitingList,
//       RequestSiegeAttackerList,RequestSiegeDefenderList,RequestSiegeInfo}.java
// Java: network/serverpackets/{SiegeInfo,SiegeAttackerList,SiegeDefenderList}.java

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::siege::{
    DefenderState, SiegeAttackerList, SiegeClan as PacketClan, SiegeDefender, SiegeDefenderList, SiegeInfo,
    SiegeOwnerClan, SiegeTime,
};
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use tracing::warn;

use super::schedule::siege_hour_times;
use super::{ATTACKER, DEFENDER, DEFENDER_NOT_APPROVED, OWNER, SiegeClan, SiegeClanRecord, SiegeClanType};
use crate::clan::ClanAccess;
use crate::residence::{CastleId, CastleRecord, ResidenceReadThen, ResidenceWrite};
use crate::world::World;

/// The client packets of the siege window as the world takes them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SiegeInput {
    /// `RequestSiegeInfo`: Java answers nothing.
    SiegeInfo,
    /// `RequestSiegeAttackerList`.
    AttackerList { castle_id: i32 },
    /// `RequestSiegeDefenderList`.
    DefenderList { castle_id: i32 },
    /// `RequestJoinSiege`.
    JoinSiege {
        castle_id: i32,
        is_attacker: i32,
        is_joining: i32,
    },
    /// `RequestConfirmSiegeWaitingList`.
    ConfirmSiegeWaitingList {
        castle_id: i32,
        clan_id: i32,
        approved: i32,
    },
    /// `RequestSetCastleSiegeTime` (the seconds of the chosen hour).
    SetCastleSiegeTime { castle_id: i32, time: i32 },
}

impl World {
    // ------------------------------------------------------------------ the packets

    /// The siege window packets.
    pub(crate) fn on_siege_input(&mut self, player: ObjectId, input: SiegeInput) {
        match input {
            SiegeInput::SiegeInfo => {}
            SiegeInput::AttackerList { castle_id } => {
                if self.castle(CastleId(castle_id)).is_some() {
                    self.request_siege_reread(
                        CastleId(castle_id),
                        ResidenceReadThen::SiegeAttackerList { player, castle_id },
                    );
                }
                // M7-CHSIEGE-01: `SiegeAttackerList(hall)` of a siegable hall.
            }
            SiegeInput::DefenderList { castle_id } => {
                if self.castle(CastleId(castle_id)).is_some() {
                    self.request_siege_reread(
                        CastleId(castle_id),
                        ResidenceReadThen::SiegeDefenderList { player, castle_id },
                    );
                }
            }
            SiegeInput::JoinSiege {
                castle_id,
                is_attacker,
                is_joining,
            } => {
                if !self.has_clan_access(player, ClanAccess::CastleSiege) {
                    self.send(
                        player,
                        &SystemMessage::new(SystemMessageId::YouAreNotAuthorizedToDoThat),
                    );
                    return;
                }
                if self.clan_of(player).is_none() {
                    return;
                }
                if self.castle(CastleId(castle_id)).is_some() {
                    self.request_siege_reread(
                        CastleId(castle_id),
                        ResidenceReadThen::SiegeJoin {
                            player,
                            castle_id,
                            is_attacker,
                            is_joining,
                        },
                    );
                }
                // M7-CHSIEGE-01: the siegable hall branch (`CHSiegeManager.registerClan`).
            }
            SiegeInput::ConfirmSiegeWaitingList {
                castle_id,
                clan_id,
                approved,
            } => {
                // Check if the player has a clan.
                if self.clan_of(player).is_none() || self.castle(CastleId(castle_id)).is_none() {
                    return;
                }
                self.request_siege_reread(
                    CastleId(castle_id),
                    ResidenceReadThen::SiegeConfirmWaiting {
                        player,
                        castle_id,
                        clan_id,
                        approved,
                    },
                );
            }
            SiegeInput::SetCastleSiegeTime { castle_id, time } => {
                self.on_request_set_castle_siege_time(player, castle_id, time);
            }
        }
    }

    /// The rest of `RequestJoinSiege.runImpl` once the registrations were read again.
    pub(crate) fn siege_join_after_reread(
        &mut self,
        player: ObjectId,
        id: CastleId,
        is_attacker: i32,
        is_joining: i32,
    ) {
        if !self.players.contains_key(&player) {
            return;
        }
        let Some(dissolving) = self.clan_of(player).map(|c| c.dissolving_expiry_time) else {
            return;
        };
        if is_joining == 1 {
            if crate::world::unix_ms_now() < dissolving {
                self.send(
                    player,
                    &SystemMessage::new(
                        SystemMessageId::YourClanMayNotRegisterToParticipateInASiegeWhileUnderAGracePeriodOfTheClanSDissolution,
                    ),
                );
                return;
            }
            if is_attacker == 1 {
                self.siege_register_attacker(id, player, false);
            } else {
                self.siege_register_defender(id, player, false);
            }
        } else {
            let clan = self.clan_of(player).map(|c| c.id);
            self.siege_remove_siege_clan_of(id, clan);
        }
        self.send_siege_info(player, id);
    }

    /// The rest of `RequestConfirmSiegeWaitingList.runImpl` once the registrations were read
    /// again.
    pub(crate) fn siege_confirm_after_reread(
        &mut self,
        player: ObjectId,
        id: CastleId,
        clan_id: i32,
        approved: i32,
    ) {
        if !self.players.contains_key(&player) {
            return;
        }
        let Some(owner) = self.castle(id).map(|c| c.owner_id) else {
            return;
        };
        // Check if leader of the clan who owns the castle?
        if owner != self.clan_id_of(player) || !self.is_clan_leader(player) {
            return;
        }
        if self.clan(clan_id).is_none() {
            return;
        }
        let Some(siege) = self.castle_siege(id) else {
            return;
        };
        if !siege.registration_over {
            if approved == 1 {
                if siege.is_defender_waiting(Some(clan_id)) {
                    self.siege_approve_siege_defender_clan(id, clan_id);
                } else {
                    return;
                }
            } else if siege.is_defender_waiting(Some(clan_id)) || siege.is_defender(Some(clan_id)) {
                self.siege_remove_siege_clan(id, clan_id);
            }
        }
        // Update the defender list.
        self.send_siege_defender_list(player, id);
    }

    /// Java `new SiegeInfo(castle, player)`.
    pub fn siege_info_packet(&self, player: ObjectId, id: CastleId) -> Option<SiegeInfo> {
        let castle = self.castle(id)?;
        let clan_id = self.clan_id_of(player);
        let leader = self.is_clan_leader(player);
        let owner_id = castle.owner_id;
        let owner = if owner_id > 0 {
            match self.clan(owner_id) {
                Some(clan) => Some(SiegeOwnerClan {
                    name: clan.name.clone(),
                    leader_name: self.clan_leader_name(owner_id),
                    ally_id: clan.ally_id,
                    ally_name: clan.ally_name.clone().unwrap_or_default(),
                }),
                None => {
                    warn!("Null owner for castle: {}", castle.name);
                    None
                }
            }
        } else {
            None
        };
        let now_secs = i32::try_from(self.siege_now_ms() / 1000).unwrap_or(i32::MAX);
        let time = if !castle.time_registration_over && leader && clan_id == owner_id {
            let offset = |t| self.siege_offset_at(t);
            let hours = siege_hour_times(castle.siege_date, &self.sieges.settings.siege_hour_list, &offset);
            SiegeTime::Hours(
                hours
                    .into_iter()
                    .map(|t| i32::try_from(t / 1000).unwrap_or(i32::MAX))
                    .collect(),
            )
        } else {
            SiegeTime::Date(i32::try_from(castle.siege_date / 1000).unwrap_or(i32::MAX))
        };
        Some(SiegeInfo {
            residence_id: id.0,
            viewer_is_owner_leader: owner_id == clan_id && leader,
            owner_id,
            owner,
            now_secs,
            time,
        })
    }

    /// Java `Siege.listRegisterClan(player)`: `SiegeInfo`.
    pub(crate) fn send_siege_info(&self, player: ObjectId, id: CastleId) {
        if let Some(packet) = self.siege_info_packet(player, id) {
            self.send(player, &packet);
        }
    }

    /// One clan of the siege lists (`None` when `ClanTable` does not know it).
    fn siege_packet_clan(&self, clan_id: i32) -> Option<PacketClan> {
        let clan = self.clan(clan_id)?;
        Some(PacketClan {
            clan_id: clan.id,
            name: clan.name.clone(),
            leader_name: self.clan_leader_name(clan_id),
            crest_id: clan.crest_id,
            ally_id: clan.ally_id,
            ally_name: clan.ally_name.clone().unwrap_or_default(),
            ally_crest_id: clan.ally_crest_id,
        })
    }

    /// Java `new SiegeAttackerList(castle)`.
    pub fn siege_attacker_list_packet(&self, id: CastleId) -> Option<SiegeAttackerList> {
        let siege = self.castle_siege(id)?;
        Some(SiegeAttackerList {
            residence_id: id.0,
            clans: siege
                .attacker_clans()
                .iter()
                .map(|sc| self.siege_packet_clan(sc.clan_id))
                .collect(),
        })
    }

    /// Java `new SiegeDefenderList(castle)`.
    pub fn siege_defender_list_packet(&self, id: CastleId) -> Option<SiegeDefenderList> {
        let siege = self.castle_siege(id)?;
        let mut clans: Vec<Option<SiegeDefender>> = siege
            .defender_clans()
            .iter()
            .map(|sc| {
                let state = match sc.kind {
                    SiegeClanType::Owner => DefenderState::Owner,
                    SiegeClanType::DefenderPending => DefenderState::DefenderPending,
                    SiegeClanType::Defender => DefenderState::Defender,
                    SiegeClanType::Attacker => DefenderState::Other,
                };
                self.siege_packet_clan(sc.clan_id)
                    .map(|clan| SiegeDefender { clan, state })
            })
            .collect();
        // NOTE(java-quirk): Java throws (and sends nothing) for a waiting clan `ClanTable` does not
        // know; here its record is left out like an unknown approved clan.
        clans.extend(siege.defender_waiting_clans().iter().map(|sc| {
            self.siege_packet_clan(sc.clan_id).map(|clan| SiegeDefender {
                clan,
                state: DefenderState::DefenderPending,
            })
        }));
        Some(SiegeDefenderList {
            castle_id: id.0,
            clans,
        })
    }

    pub(crate) fn send_siege_attacker_list(&self, player: ObjectId, id: CastleId) {
        if let Some(packet) = self.siege_attacker_list_packet(id) {
            self.send(player, &packet);
        }
    }

    pub(crate) fn send_siege_defender_list(&self, player: ObjectId, id: CastleId) {
        if let Some(packet) = self.siege_defender_list_packet(id) {
            self.send(player, &packet);
        }
    }

    // ------------------------------------------------------------------ re-read (D-50)

    /// Reads the castle and every `siege_clans` row again before acting on them (D-50); the
    /// answer comes back as `WorldCommand::ResidenceReread` and goes on with `then`.
    pub fn request_siege_reread(&mut self, id: CastleId, then: ResidenceReadThen) {
        self.outbox
            .persist(crate::outbox::PersistCommand::ResidenceRereadRequest {
                residence_id: id.0,
                kind: crate::residence::ResidenceReadKind::SiegeClans,
                then,
            });
    }

    /// The rows of [`Self::request_siege_reread`] as the database has them now. A castle whose
    /// siege is in progress keeps its owner and lists (the siege changes them in memory); the
    /// others load their lists from the rows again. Rows this world added without writing them
    /// (`SiegeDatabaseWrites` off) stay.
    pub(crate) fn apply_siege_reread(&mut self, castle: Option<&CastleRecord>, rows: &[SiegeClanRecord]) {
        if let Some(record) = castle
            && !self.is_castle_siege_in_progress(CastleId(record.id))
        {
            self.apply_castle_record(record);
        }
        let unsaved: Vec<(SiegeClanRecord, bool)> = self
            .sieges
            .siege_clan_rows
            .iter()
            .filter(|(row, unsaved)| {
                *unsaved
                    && !rows
                        .iter()
                        .any(|r| r.castle_id == row.castle_id && r.clan_id == row.clan_id)
            })
            .copied()
            .collect();
        self.sieges.siege_clan_rows = rows.iter().map(|r| (*r, false)).chain(unsaved).collect();
        let ids: Vec<CastleId> = self.sieges.sieges.keys().copied().collect();
        for id in ids {
            if !self.is_castle_siege_in_progress(id) {
                self.siege_load_siege_clan(id);
            }
        }
    }

    // ------------------------------------------------------------------ the rows

    /// Runs a `siege_clans` statement: the copy here changes and the row goes out with
    /// `SiegeDatabaseWrites` (D-51).
    fn siege_clans_write(&mut self, write: ResidenceWrite) {
        let saved = self.siege_database_writes();
        let rows = &mut self.sieges.siege_clan_rows;
        match &write {
            ResidenceWrite::SiegeClanInsert {
                clan_id,
                castle_id,
                kind,
            } => rows.push((
                SiegeClanRecord {
                    castle_id: *castle_id,
                    clan_id: *clan_id,
                    kind: *kind,
                },
                !saved,
            )),
            ResidenceWrite::SiegeClanUpdate {
                castle_id,
                clan_id,
                kind,
            } => {
                for (row, unsaved) in rows.iter_mut() {
                    if row.castle_id == *castle_id && row.clan_id == *clan_id {
                        row.kind = *kind;
                        *unsaved |= !saved;
                    }
                }
            }
            ResidenceWrite::SiegeClanDelete { castle_id, clan_id } => {
                rows.retain(|(r, _)| !(r.castle_id == *castle_id && r.clan_id == *clan_id));
            }
            ResidenceWrite::SiegeClansDeleteCastle { castle_id } => {
                rows.retain(|(r, _)| r.castle_id != *castle_id)
            }
            ResidenceWrite::SiegeClansDeleteClan { clan_id } => rows.retain(|(r, _)| r.clan_id != *clan_id),
            ResidenceWrite::SiegeClansDeleteWaiting { castle_id } => {
                rows.retain(|(r, _)| !(r.castle_id == *castle_id && r.kind == DEFENDER_NOT_APPROVED));
            }
            _ => {}
        }
        self.siege_job_write(write);
    }

    /// Java `SiegeManager.checkIsRegistered(clan, castleId)`: a castle owner, or a clan with a
    /// row for the castle.
    pub fn siege_check_is_registered(&self, clan_id: Option<i32>, castle_id: i32) -> bool {
        let Some(clan) = clan_id.and_then(|id| self.clan(id)) else {
            return false;
        };
        if clan.castle_id > 0 {
            return true;
        }
        self.sieges
            .siege_clan_rows
            .iter()
            .any(|(r, _)| r.clan_id == clan.id && r.castle_id == castle_id)
    }

    /// Java `Siege.loadSiegeClan()`: the lists from the rows, the owner first as `OWNER`.
    pub(crate) fn siege_load_siege_clan(&mut self, id: CastleId) {
        let owner = self.castle(id).map_or(0, |c| c.owner_id);
        let rows: Vec<SiegeClanRecord> = self
            .sieges
            .siege_clan_rows
            .iter()
            .filter(|(r, _)| r.castle_id == id.0)
            .map(|(r, _)| *r)
            .collect();
        let Some(siege) = self.castle_siege_mut(id) else {
            return;
        };
        siege.attacker_clans_mut().clear();
        siege.defender_clans_mut().clear();
        siege.defender_waiting_clans.clear();
        // Add castle owner as defender (add owner first so that they are on the top of the
        // defender list).
        if owner > 0 {
            siege
                .defender_clans_mut()
                .push(SiegeClan::new(owner, SiegeClanType::Owner));
        }
        for row in rows {
            match row.kind {
                DEFENDER => siege
                    .defender_clans_mut()
                    .push(SiegeClan::new(row.clan_id, SiegeClanType::Defender)),
                ATTACKER => siege
                    .attacker_clans_mut()
                    .push(SiegeClan::new(row.clan_id, SiegeClanType::Attacker)),
                DEFENDER_NOT_APPROVED => siege
                    .defender_waiting_clans
                    .push(SiegeClan::new(row.clan_id, SiegeClanType::DefenderPending)),
                _ => {}
            }
        }
    }

    /// Java `Siege.saveSiegeClan(clan, typeId, isUpdateRegistration)`.
    fn siege_save_siege_clan(&mut self, id: CastleId, clan_id: i32, type_id: i32, is_update: bool) {
        let Some(clan_castle) = self.clan(clan_id).map(|c| c.castle_id) else {
            return;
        };
        if clan_castle > 0 {
            return;
        }
        let settings = &self.sieges.settings;
        let (defender_max, attacker_max) = (settings.defender_max_clans, settings.attacker_max_clans);
        let Some(siege) = self.castle_siege(id) else {
            return;
        };
        let len = |n: usize| i32::try_from(n).unwrap_or(i32::MAX);
        if type_id == DEFENDER || type_id == DEFENDER_NOT_APPROVED || type_id == OWNER {
            if len(siege.defender_clans().len() + siege.defender_waiting_clans().len()) >= defender_max {
                return;
            }
        } else if len(siege.attacker_clans().len()) >= attacker_max {
            return;
        }
        if !is_update {
            // The primary key is (clan_id, castle_id): Java's INSERT fails for a second row and
            // the lists stay as they are.
            let exists = self
                .sieges
                .siege_clan_rows
                .iter()
                .any(|(r, _)| r.clan_id == clan_id && r.castle_id == id.0);
            if exists {
                warn!(
                    "Exception: saveSiegeClan(Pledge clan, int typeId, boolean isUpdateRegistration): duplicate siege_clans row ({clan_id}, {})",
                    id.0
                );
                return;
            }
            self.siege_clans_write(ResidenceWrite::SiegeClanInsert {
                clan_id,
                castle_id: id.0,
                kind: type_id,
            });
        } else {
            self.siege_clans_write(ResidenceWrite::SiegeClanUpdate {
                castle_id: id.0,
                clan_id,
                kind: type_id,
            });
        }
        let Some(siege) = self.castle_siege_mut(id) else {
            return;
        };
        if type_id == DEFENDER || type_id == OWNER {
            siege
                .defender_clans_mut()
                .push(SiegeClan::new(clan_id, SiegeClanType::Defender));
        } else if type_id == ATTACKER {
            siege
                .attacker_clans_mut()
                .push(SiegeClan::new(clan_id, SiegeClanType::Attacker));
        } else if type_id == DEFENDER_NOT_APPROVED {
            siege
                .defender_waiting_clans
                .push(SiegeClan::new(clan_id, SiegeClanType::DefenderPending));
        }
    }

    // ------------------------------------------------------------------ registering

    /// Java `Siege.registerAttacker(player, force)`.
    pub fn siege_register_attacker(&mut self, id: CastleId, player: ObjectId, force: bool) {
        let Some(clan_id) = self.clan_of(player).map(|c| c.id) else {
            return;
        };
        let Some(owner) = self.castle(id).map(|c| c.owner_id) else {
            return;
        };
        // NOTE(java-quirk): Java throws for an owner id `ClanTable` does not know.
        let ally_id = if owner != 0 {
            self.clan(owner).map_or(0, |c| c.ally_id)
        } else {
            0
        };
        let clan_ally = self.clan(clan_id).map_or(0, |c| c.ally_id);
        if ally_id != 0 && clan_ally == ally_id && !force {
            self.send(
                player,
                &SystemMessage::new(
                    SystemMessageId::YouCannotRegisterAsAnAttackerBecauseYouAreInAnAllianceWithTheCastleOwningClan,
                ),
            );
            return;
        }
        if force {
            if self.siege_check_is_registered(Some(clan_id), id.0) {
                self.send(
                    player,
                    &SystemMessage::new(SystemMessageId::YouHaveAlreadyRequestedACastleSiege),
                );
            } else {
                self.siege_save_siege_clan(id, clan_id, ATTACKER, false);
            }
            return;
        }
        if self.siege_check_if_can_register(id, player, ATTACKER) {
            self.siege_save_siege_clan(id, clan_id, ATTACKER, false);
        }
    }

    /// Java `Siege.registerDefender(player, force)`.
    pub fn siege_register_defender(&mut self, id: CastleId, player: ObjectId, force: bool) {
        let Some((owner, name)) = self.castle(id).map(|c| (c.owner_id, c.name.clone())) else {
            return;
        };
        if owner <= 0 {
            self.send_message(
                player,
                &format!("You cannot register as a defender because {name} is owned by NPC."),
            );
            return;
        }
        let clan_id = self.clan_of(player).map(|c| c.id);
        if force {
            if self.siege_check_is_registered(clan_id, id.0) {
                self.send(
                    player,
                    &SystemMessage::new(SystemMessageId::YouHaveAlreadyRequestedACastleSiege),
                );
            } else if let Some(clan_id) = clan_id {
                self.siege_save_siege_clan(id, clan_id, DEFENDER_NOT_APPROVED, false);
            }
            return;
        }
        if self.siege_check_if_can_register(id, player, DEFENDER_NOT_APPROVED)
            && let Some(clan_id) = clan_id
        {
            self.siege_save_siege_clan(id, clan_id, DEFENDER_NOT_APPROVED, false);
        }
    }

    /// Java `Siege.checkIfCanRegister(player, typeId)`: the message of the first rule that fails.
    fn siege_check_if_can_register(&mut self, id: CastleId, player: ObjectId, type_id: i32) -> bool {
        let clan = self.clan_of(player).map(|c| (c.id, c.level, c.castle_id));
        let Some((owner, siege)) = self.castle(id).map(|c| c.owner_id).zip(self.castle_siege(id)) else {
            return false;
        };
        let settings = &self.sieges.settings;
        let len = |n: usize| i32::try_from(n).unwrap_or(i32::MAX);
        let message = if siege.registration_over {
            Some(
                SystemMessage::new(SystemMessageId::TheDeadlineToRegisterForTheSiegeOfS1HasPassed)
                    .with(SystemMessageParam::CastleName(id.0)),
            )
        } else if siege.in_progress {
            Some(SystemMessage::new(
                SystemMessageId::ThisIsNotTheTimeForSiegeRegistrationAndSoRegistrationAndCancellationCannotBeDone,
            ))
        } else {
            match clan {
                None => Some(SystemMessage::new(
                    SystemMessageId::OnlyClansOfLevel5OrHigherMayRegisterForACastleSiege,
                )),
                Some((_, level, _)) if level < settings.siege_clan_min_level => Some(SystemMessage::new(
                    SystemMessageId::OnlyClansOfLevel5OrHigherMayRegisterForACastleSiege,
                )),
                Some((clan_id, _, _)) if clan_id == owner => Some(SystemMessage::new(
                    SystemMessageId::CastleOwningClansAreAutomaticallyRegisteredOnTheDefendingSide,
                )),
                Some((_, _, castle_id)) if castle_id > 0 => Some(SystemMessage::new(
                    SystemMessageId::AClanThatOwnsACastleCannotParticipateInAnotherSiege,
                )),
                Some((clan_id, _, _)) if self.siege_check_is_registered(Some(clan_id), id.0) => {
                    Some(SystemMessage::new(SystemMessageId::YouHaveAlreadyRequestedACastleSiege))
                }
                Some((clan_id, _, _)) if self.siege_check_if_already_registered_for_same_day(id, clan_id) => {
                    Some(SystemMessage::new(
                        SystemMessageId::YourApplicationHasBeenDeniedBecauseYouHaveAlreadySubmittedARequestForAnotherCastleSiege,
                    ))
                }
                Some(_) if type_id == ATTACKER && len(siege.attacker_clans().len()) >= settings.attacker_max_clans => {
                    Some(SystemMessage::new(
                        SystemMessageId::NoMoreRegistrationsMayBeAcceptedForTheAttackerSide,
                    ))
                }
                Some(_)
                    if (type_id == DEFENDER || type_id == DEFENDER_NOT_APPROVED || type_id == OWNER)
                        && len(siege.defender_clans().len() + siege.defender_waiting_clans().len())
                            >= settings.defender_max_clans =>
                {
                    Some(SystemMessage::new(
                        SystemMessageId::NoMoreRegistrationsMayBeAcceptedForTheDefenderSide,
                    ))
                }
                Some(_) => None,
            }
        };
        match message {
            Some(message) => {
                self.send(player, &message);
                false
            }
            None => true,
        }
    }

    /// Java `Siege.checkIfAlreadyRegisteredForSameDay(clan)`: the clan is on a list of another
    /// castle whose siege is on the same day of the week.
    pub fn siege_check_if_already_registered_for_same_day(&self, id: CastleId, clan_id: i32) -> bool {
        let offset = |t| self.siege_offset_at(t);
        let weekday = |date: i64| {
            let local = date + offset(date);
            (local.div_euclid(86_400_000) + 4).rem_euclid(7)
        };
        let Some(own_day) = self.castle(id).map(|c| weekday(c.siege_date)) else {
            return false;
        };
        self.sieges.sieges.values().any(|siege| {
            siege.castle_id != id
                && self
                    .castle(siege.castle_id)
                    .is_some_and(|c| weekday(c.siege_date) == own_day)
                && (siege.is_attacker(Some(clan_id))
                    || siege.is_defender(Some(clan_id))
                    || siege.is_defender_waiting(Some(clan_id)))
        })
    }

    /// Java `Siege.approveSiegeDefenderClan(clanId)`.
    pub(crate) fn siege_approve_siege_defender_clan(&mut self, id: CastleId, clan_id: i32) {
        if clan_id <= 0 {
            return;
        }
        self.siege_save_siege_clan(id, clan_id, DEFENDER, true);
        self.siege_load_siege_clan(id);
    }

    /// Java `Siege.removeSiegeClan(clanId)`.
    pub fn siege_remove_siege_clan(&mut self, id: CastleId, clan_id: i32) {
        if clan_id <= 0 {
            return;
        }
        self.siege_clans_write(ResidenceWrite::SiegeClanDelete {
            castle_id: id.0,
            clan_id,
        });
        self.siege_load_siege_clan(id);
    }

    /// Java `Siege.removeSiegeClan(clan)`: not the castle's own clan, and only a registered one.
    pub fn siege_remove_siege_clan_of(&mut self, id: CastleId, clan_id: Option<i32>) {
        let Some(clan) = clan_id.and_then(|c| self.clan(c)) else {
            return;
        };
        let (clan_id, castle) = (clan.id, clan.castle_id);
        if castle == id.0 || !self.siege_check_is_registered(Some(clan_id), id.0) {
            return;
        }
        self.siege_remove_siege_clan(id, clan_id);
    }

    /// Java `Siege.clearSiegeClan()`: the castle's rows and, for an owned castle, every row of the
    /// owner; the lists are empty.
    pub fn siege_clear_siege_clan(&mut self, id: CastleId) {
        self.siege_clans_write(ResidenceWrite::SiegeClansDeleteCastle { castle_id: id.0 });
        let owner = self.castle(id).map_or(0, |c| c.owner_id);
        if owner > 0 {
            self.siege_clans_write(ResidenceWrite::SiegeClansDeleteClan { clan_id: owner });
        }
        if let Some(siege) = self.castle_siege_mut(id) {
            siege.attacker_clans_mut().clear();
            siege.defender_clans_mut().clear();
            siege.defender_waiting_clans.clear();
        }
    }

    /// Java `Siege.clearSiegeWaitingClan()`.
    pub(crate) fn siege_clear_siege_waiting_clan(&mut self, id: CastleId) {
        self.siege_clans_write(ResidenceWrite::SiegeClansDeleteWaiting { castle_id: id.0 });
        if let Some(siege) = self.castle_siege_mut(id) {
            siege.defender_waiting_clans.clear();
        }
    }
}
