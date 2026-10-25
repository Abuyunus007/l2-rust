//! `CHSiegeManager`: a clan registers for a conquerable hall siege or cancels, and the siege
//! window shows the hall (`SiegeInfo`, `SiegeAttackerList`).
// Java: managers/CHSiegeManager.java (registerClan, unRegisterClan, isClanParticipating,
//       getNearbyClanHall, getSiege)
// Java: network/clientpackets/{RequestJoinSiege,RequestSiegeAttackerList}.java (the hall branches)
// Java: network/serverpackets/{SiegeInfo,SiegeAttackerList}.java (the hall branches)

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::siege::{SiegeAttackerList, SiegeInfo, SiegeOwnerClan, SiegeTime};
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use tracing::warn;

use super::HallScript;
use super::load::{FlagWarRecord, HallSiegeRows};
use crate::residence::{ClanHallId, ResidenceReadThen, ResidenceRef};
use crate::world::{World, unix_ms_now};

/// What the hall code does once the hall was read again (`ResidenceReadThen::HallSiege`, D-50).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HallThen {
    /// `RequestJoinSiege` for a hall: the registration or its cancelling.
    Join {
        player: ObjectId,
        hall_id: i32,
        is_joining: i32,
    },
    /// An event of a script that acts on registrations.
    ScriptEvent {
        player: ObjectId,
        npc: ObjectId,
        script: HallScript,
        event: String,
    },
}

impl World {
    // ------------------------------------------------------------------ the siege window

    /// Whether a siege window request names a conquerable hall (`getSiegableHall(id)`).
    pub(crate) fn is_siegable_hall_id(&self, id: i32) -> bool {
        self.clan_hall(ClanHallId(id))
            .is_some_and(|h| h.is_siegable_hall())
    }

    /// `RequestSiegeAttackerList` for a hall: `SiegeAttackerList(hall)`.
    pub(crate) fn hall_send_attacker_list(&self, player: ObjectId, id: ClanHallId) {
        if let Some(packet) = self.hall_attacker_list_packet(id) {
            self.send(player, &packet);
        }
    }

    /// Java `new SiegeAttackerList(hall)`; `None` where `getSiege()` is null (a NullPointerException).
    pub fn hall_attacker_list_packet(&self, id: ClanHallId) -> Option<SiegeAttackerList> {
        self.hall_siege(id)?;
        Some(SiegeAttackerList {
            residence_id: id.0,
            clans: self
                .hall_siege_attacker_clans(id)
                .into_iter()
                .map(|clan| self.siege_packet_clan(clan))
                .collect(),
        })
    }

    /// Java `new SiegeInfo(hall, player)`.
    pub fn hall_siege_info_packet(&self, player: ObjectId, id: ClanHallId) -> Option<SiegeInfo> {
        let hall = self.clan_hall(id)?;
        let owner_id = hall.owner_id;
        let owner = if owner_id > 0 {
            match self.clan(owner_id) {
                Some(clan) => Some(SiegeOwnerClan {
                    name: clan.name.clone(),
                    leader_name: self.clan_leader_name(owner_id),
                    ally_id: clan.ally_id,
                    ally_name: clan.ally_name.clone().unwrap_or_default(),
                }),
                None => {
                    warn!("Null owner for siegable hall: {}", hall.name);
                    None
                }
            }
        } else {
            None
        };
        let now_secs = i32::try_from(self.siege_now_ms() / 1000).unwrap_or(i32::MAX);
        let next = i32::try_from(self.hall_next_siege_time(id) / 1000).unwrap_or(i32::MAX);
        Some(SiegeInfo {
            residence_id: id.0,
            viewer_is_owner_leader: owner_id == self.clan_id_of(player) && self.is_clan_leader(player),
            owner_id,
            owner,
            now_secs,
            time: SiegeTime::Date(next),
        })
    }

    /// `RequestJoinSiege` for a hall: the hall is read again first (D-50).
    pub(crate) fn hall_join_siege(&mut self, player: ObjectId, id: ClanHallId, is_joining: i32) {
        self.request_residence_reread(
            ResidenceRef::ClanHall(id),
            ResidenceReadThen::HallSiege(HallThen::Join {
                player,
                hall_id: id.0,
                is_joining,
            }),
        );
    }

    /// The rest of `RequestJoinSiege.runImpl` for a hall once the row was read again.
    fn hall_join_after_reread(&mut self, player: ObjectId, id: ClanHallId, is_joining: i32) {
        if !self.players.contains_key(&player) {
            return;
        }
        let Some((clan_id, dissolving)) = self.clan_of(player).map(|c| (c.id, c.dissolving_expiry_time))
        else {
            return;
        };
        if is_joining == 1 {
            if unix_ms_now() < dissolving {
                self.send(
                    player,
                    &SystemMessage::new(
                        SystemMessageId::YourClanMayNotRegisterToParticipateInASiegeWhileUnderAGracePeriodOfTheClanSDissolution,
                    ),
                );
                return;
            }
            self.hall_register_clan(clan_id, id, player);
        } else {
            self.hall_unregister_clan(clan_id, id);
        }
        if let Some(packet) = self.hall_siege_info_packet(player, id) {
            self.send(player, &packet);
        }
    }

    // ------------------------------------------------------------------ CHSiegeManager

    /// Java `CHSiegeManager.registerClan(clan, hall, player)`.
    pub fn hall_register_clan(&mut self, clan_id: i32, id: ClanHallId, player: ObjectId) {
        let Some((level, castle, hideout)) = self.clan(clan_id).map(|c| (c.level, c.castle_id, c.hideout_id))
        else {
            return;
        };
        let min = self.clanhall.siege.settings.min_clan_level;
        let name = self.clan_hall(id).map(|h| h.name.clone()).unwrap_or_default();
        let owner = self.clan_hall(id).map_or(0, |h| h.owner_id);
        let status = self.hall_siege_status(id);
        if level < min {
            self.send_message(
                player,
                &format!("Only clans of level {min} or higher may register for a castle siege"),
            );
        } else if status == Some(super::SiegeStatus::WaitingBattle) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::TheDeadlineToRegisterForTheSiegeOfS1HasPassed)
                    .with(SystemMessageParam::Text(name)),
            );
        } else if status == Some(super::SiegeStatus::Running) {
            self.send(
                player,
                &SystemMessage::new(
                    SystemMessageId::ThisIsNotTheTimeForSiegeRegistrationAndSoRegistrationAndCancellationCannotBeDone,
                ),
            );
        } else if owner == clan_id {
            self.send(
                player,
                &SystemMessage::new(
                    SystemMessageId::CastleOwningClansAreAutomaticallyRegisteredOnTheDefendingSide,
                ),
            );
        } else if castle != 0 || hideout != 0 {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::AClanThatOwnsACastleCannotParticipateInAnotherSiege),
            );
        } else if !self.hall_has_siege(id) {
            // `hall.getSiege().checkIsAttacker(clan)`: a NullPointerException.
            warn!("CHSiegeManager: {id} has no siege.");
        } else if self.hall_siege_is_attacker(id, clan_id) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouHaveAlreadyRequestedACastleSiege),
            );
        } else if self.hall_clan_is_participating(clan_id) {
            self.send(
                player,
                &SystemMessage::new(
                    SystemMessageId::YourApplicationHasBeenDeniedBecauseYouHaveAlreadySubmittedARequestForAnotherCastleSiege,
                ),
            );
        } else if self.hall_siege(id).is_some_and(|s| {
            i32::try_from(s.attackers.len()).unwrap_or(i32::MAX) >= self.clanhall.siege.settings.max_attackers
        }) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::NoMoreRegistrationsMayBeAcceptedForTheAttackerSide),
            );
        } else {
            self.hall_add_attacker(id, clan_id);
        }
    }

    /// Java `CHSiegeManager.unRegisterClan(clan, hall)`.
    pub fn hall_unregister_clan(&mut self, clan_id: i32, id: ClanHallId) {
        if !self.hall_is_registering(id) {
            return;
        }
        self.hall_remove_attacker(id, clan_id);
    }

    /// The follow-ups of a re-read.
    pub(crate) fn on_hall_siege_reread(&mut self, then: HallThen) {
        match then {
            HallThen::Join {
                player,
                hall_id,
                is_joining,
            } => self.hall_join_after_reread(player, ClanHallId(hall_id), is_joining),
            HallThen::ScriptEvent {
                player,
                npc,
                script,
                event,
            } => {
                if self.npcs.contains_key(&npc) {
                    self.hall_script_run_event(script, player, npc, &event);
                }
            }
        }
    }

    /// The tables of a hall read again (D-50) replace this world's copy.
    pub(crate) fn apply_hall_siege_rows(&mut self, rows: &HallSiegeRows) {
        if let Some(record) = &rows.hall {
            self.apply_siegable_hall_record(record);
        }
        let id = ClanHallId(rows.hall_id);
        match self.hall_siege(id).map(|s| s.script) {
            Some(HallScript::BanditStronghold | HallScript::WildBeastReserve) => {
                super::flag_war::apply_rows(self, id, &rows.flag_war);
            }
            Some(HallScript::RainbowSpringsChateau) => super::rainbow::apply_rows(self, &rows.rainbow),
            _ => {}
        }
    }
}

impl FlagWarRecord {
    /// A row without members, for the tests.
    pub fn new(clan_id: i32, flag: i32, npc: i32) -> Self {
        Self {
            clan_id,
            flag,
            npc,
            members: Vec::new(),
        }
    }
}
