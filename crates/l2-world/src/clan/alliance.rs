//! Alliances: creating and dissolving one, the invitation, leaving, expelling a clan and the
//! alliance window (M6-CLAN-02).
// Java: entity/clan/Clan.java (checkAllyJoinCondition, createAlly, dissolveAlly,
//       broadcastToOnlineAllyMembers)
// Java: network/clientpackets/{RequestJoinAlly,RequestAnswerJoinAlly,AllyLeave,AllyDismiss,
//       RequestDismissAlly,RequestAllyInfo}.java
// Java: network/serverpackets/{AllianceInfo,AskJoinAlly}.java, entity/clan/ClanInfo.java
//
// The alliance is its leading clan: `ally_id` is the id of that clan, and the alliance's name,
// penalty and crest are read from the clans like in Java (there is no alliance object).

use l2_net::Encode;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::pledge::{AllianceClanInfo, AllianceInfo, AskJoinAlly};
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;

use super::model::{
    PENALTY_TYPE_CLAN_DISMISSED, PENALTY_TYPE_CLAN_LEAVED, PENALTY_TYPE_DISMISS_CLAN,
    PENALTY_TYPE_DISSOLVE_ALLY, is_alpha_numeric, java_len,
};
use super::{RequestPacket, java_int_days_ms};
use crate::world::{World, unix_ms_now};
use crate::zones::ZoneId;

impl World {
    fn ally_player_name(&self, id: ObjectId) -> String {
        self.players
            .get(&id)
            .map(|p| p.data.name.clone())
            .unwrap_or_default()
    }

    /// Java `Clan.checkAllyJoinCondition(player, target)`: the leader of an alliance may invite
    /// the leader of a clan.
    pub(crate) fn check_ally_join_condition(&self, player: ObjectId, target: Option<ObjectId>) -> bool {
        let not_leader = |world: &Self| {
            world.send(
                player,
                &SystemMessage::new(SystemMessageId::ThisFeatureIsOnlyAvailableToAllianceLeaders),
            );
        };
        let ally_id = self.ally_id_of(player);
        if ally_id == 0 || !self.is_clan_leader(player) || self.clan_id_of(player) != ally_id {
            not_leader(self);
            return false;
        }
        let Some(leader_clan) = self.clan_of(player) else {
            return false;
        };
        let now = unix_ms_now();
        if leader_clan.ally_penalty_expiry_time > now
            && leader_clan.ally_penalty_type == PENALTY_TYPE_DISMISS_CLAN
        {
            self.send(
                player,
                &SystemMessage::new(
                    SystemMessageId::YouMayNotAcceptAnyClanWithinADayAfterExpellingAnotherClan,
                ),
            );
            return false;
        }
        let Some(target) = target.filter(|t| self.players.contains_key(t)) else {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouHaveInvitedTheWrongTarget),
            );
            return false;
        };
        if player == target {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouCannotAskYourselfToApplyToAClan),
            );
            return false;
        }
        let Some(target_clan) = self.clan_of(target) else {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::TheTargetMustBeAClanMember),
            );
            return false;
        };
        if !self.is_clan_leader(target) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::S1IsNotAClanLeader)
                    .with(SystemMessageParam::Text(self.ally_player_name(target))),
            );
            return false;
        }
        if self.ally_id_of(target) != 0 {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::S1ClanIsAlreadyAMemberOfS2Alliance)
                    .with(SystemMessageParam::Text(target_clan.name.clone()))
                    .with(SystemMessageParam::Text(
                        target_clan.ally_name.clone().unwrap_or_default(),
                    )),
            );
            return false;
        }
        if target_clan.ally_penalty_expiry_time > now {
            if target_clan.ally_penalty_type == PENALTY_TYPE_CLAN_LEAVED {
                self.send(
                    player,
                    &SystemMessage::new(
                        SystemMessageId::S1ClanCannotJoinTheAllianceBecauseOneDayHasNotYetPassedSinceTheyLeftAnotherAlliance,
                    )
                    .with(SystemMessageParam::Text(target_clan.name.clone()))
                    .with(SystemMessageParam::Text(
                        target_clan.ally_name.clone().unwrap_or_default(),
                    )),
                );
                return false;
            }
            if target_clan.ally_penalty_type == PENALTY_TYPE_CLAN_DISMISSED {
                self.send(
                    player,
                    &SystemMessage::new(
                        SystemMessageId::AClanThatHasWithdrawnOrBeenExpelledCannotEnterIntoAnAllianceWithinOneDayOfWithdrawalOrExpulsion,
                    ),
                );
                return false;
            }
        }
        if self.is_inside_zone(player, ZoneId::Siege) && self.is_inside_zone(target, ZoneId::Siege) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::TheOpposingClanIsParticipatingInASiegeBattle),
            );
            return false;
        }
        if leader_clan.is_at_war_with(target_clan.id) {
            self.send(
                player,
                &SystemMessage::new(
                    SystemMessageId::YouMayNotAllyWithAClanYouAreCurrentlyAtWarWithThatWouldBeDiabolicalAndTreacherous,
                ),
            );
            return false;
        }
        if i64::try_from(self.clan_allies(ally_id).len()).unwrap_or(i64::MAX)
            >= i64::from(self.clans.settings.alt_max_num_of_clans_in_ally)
        {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouHaveExceededTheLimit),
            );
            return false;
        }
        true
    }

    /// Java `Clan.broadcastToOnlineAllyMembers(packet)`.
    pub(crate) fn broadcast_to_online_ally_members(&self, clan_id: i32, packet: &impl Encode) {
        let ally_id = self.clan(clan_id).map_or(0, |c| c.ally_id);
        for clan in self.clan_allies(ally_id) {
            self.broadcast_to_clan(clan, packet);
        }
    }

    /// Java `Clan.createAlly(player, allyName)`.
    pub(crate) fn create_ally(&mut self, player: ObjectId, ally_name: &str) {
        if !self.players.contains_key(&player) {
            return;
        }
        if !self.is_clan_leader(player) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::OnlyClanLeadersMayCreateAlliances),
            );
            return;
        }
        let clan_id = self.clan_id_of(player);
        let Some(clan) = self.clan(clan_id) else {
            return;
        };
        let now = unix_ms_now();
        let message = if clan.ally_id != 0 {
            Some(SystemMessageId::YouAlreadyBelongToAnotherAlliance)
        } else if clan.level < 5 {
            Some(SystemMessageId::ToCreateAnAllianceYourClanMustBeLevel5OrHigher)
        } else if clan.ally_penalty_expiry_time > now && clan.ally_penalty_type == PENALTY_TYPE_DISSOLVE_ALLY
        {
            Some(SystemMessageId::YouCannotCreateANewAllianceWithin1DayOfDissolution)
        } else if clan.dissolving_expiry_time > now {
            Some(SystemMessageId::AsYouAreCurrentlyScheduleForClanDissolutionNoAllianceCanBeCreated)
        } else if !is_alpha_numeric(ally_name) {
            Some(SystemMessageId::IncorrectAllianceNamePleaseTryAgain)
        } else if java_len(ally_name) > 16 || java_len(ally_name) < 2 {
            Some(SystemMessageId::IncorrectLengthForAnAllianceName)
        } else if self.is_ally_exists(ally_name) {
            Some(SystemMessageId::ThatAllianceNameAlreadyExists)
        } else {
            None
        };
        if let Some(id) = message {
            self.send(player, &SystemMessage::new(id));
            return;
        }
        if let Some(clan) = self.clan_mut(clan_id) {
            clan.ally_id = clan_id;
            clan.ally_name = Some(java_trim(ally_name).to_owned());
            clan.ally_penalty_expiry_time = 0;
            clan.ally_penalty_type = 0;
        }
        self.update_clan_in_db(clan_id);
        self.send_user_info(player);
        // TODO(java): "Need correct message id".
        self.send_message(player, &format!("Alliance {ally_name} has been created."));
    }

    /// Java `Clan.dissolveAlly(player)`.
    pub(crate) fn dissolve_ally(&mut self, player: ObjectId) {
        let clan_id = self.clan_id_of(player);
        let Some(clan) = self.clan(clan_id) else {
            return;
        };
        let ally_id = clan.ally_id;
        if ally_id == 0 {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouAreNotCurrentlyAlliedWithAnyClans),
            );
            return;
        }
        if !self.is_clan_leader(player) || clan_id != ally_id {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::ThisFeatureIsOnlyAvailableToAllianceLeaders),
            );
            return;
        }
        if self.is_inside_zone(player, ZoneId::Siege) {
            self.send(
                player,
                &SystemMessage::new(
                    SystemMessageId::YouCannotDissolveAnAllianceWhileAnAffiliatedClanIsParticipatingInASiegeBattle,
                ),
            );
            return;
        }
        self.broadcast_to_online_ally_members(
            clan_id,
            &SystemMessage::new(SystemMessageId::TheAllianceHasBeenDissolved),
        );

        let now = unix_ms_now();
        for ally in self.clan_allies(ally_id) {
            if ally != clan_id {
                if let Some(c) = self.clan_mut(ally) {
                    c.ally_id = 0;
                    c.ally_name = None;
                    c.ally_penalty_expiry_time = 0;
                    c.ally_penalty_type = 0;
                }
                self.update_clan_in_db(ally);
            }
        }

        if let Some(c) = self.clan_mut(clan_id) {
            c.ally_id = 0;
            c.ally_name = None;
        }
        // NOTE(java-quirk): the ally id is 0 here already, see `change_ally_crest_inner`.
        self.change_ally_crest(clan_id, 0, false);
        let days = self.clans.settings.alt_create_ally_days_when_dissolved;
        if let Some(c) = self.clan_mut(clan_id) {
            c.ally_penalty_expiry_time = now.wrapping_add(java_int_days_ms(days));
            c.ally_penalty_type = PENALTY_TYPE_DISSOLVE_ALLY;
        }
        self.update_clan_in_db(clan_id);
    }

    // ------------------------------------------------------------------ the client's requests

    /// Java `RequestJoinAlly.runImpl`.
    pub(crate) fn on_request_join_ally(&mut self, player: ObjectId, id: i32) {
        if !self.players.contains_key(&player) {
            return;
        }
        // `World.getPlayer(_id)`
        let Some(target) = Some(ObjectId(id)).filter(|t| self.players.contains_key(t)) else {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouHaveInvitedTheWrongTarget),
            );
            return;
        };
        if self.clan_of(player).is_none() {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouAreNotAClanMemberAndCannotPerformThisAction),
            );
            return;
        }
        if !self.check_ally_join_condition(player, Some(target)) {
            return;
        }
        if !self.set_request(player, Some(target), RequestPacket::JoinAlly) {
            return;
        }
        let ally_name = self
            .clan_of(player)
            .and_then(|c| c.ally_name.clone())
            .unwrap_or_default();
        let name = self.ally_player_name(player);
        self.send(
            target,
            &SystemMessage::new(SystemMessageId::S1LeaderS2HasRequestedAnAlliance)
                .with(SystemMessageParam::Text(ally_name.clone()))
                .with(SystemMessageParam::Text(name)),
        );
        self.send(
            target,
            &AskJoinAlly {
                requestor_object_id: player.0,
                requestor_name: ally_name,
            },
        );
    }

    /// Java `RequestAnswerJoinAlly.runImpl`.
    pub(crate) fn on_request_answer_join_ally(&mut self, player: ObjectId, response: i32) {
        let Some(requestor) = self.request_partner(player) else {
            return;
        };
        if response == 0 {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::NoResponseYourEntranceToTheAllianceHasBeenCancelled),
            );
            self.send(
                requestor,
                &SystemMessage::new(SystemMessageId::NoResponseInvitationToJoinAnAllianceHasBeenCancelled),
            );
        } else {
            if self.request_packet(requestor) != Some(RequestPacket::JoinAlly) {
                return; // hax
            }
            let requestor_clan_id = self.clan_id_of(requestor);
            if self.clan(requestor_clan_id).is_none() {
                // `requestor.getClan()` is null: the call on it throws.
                return;
            }
            // we must double check this cause of hack
            if self.check_ally_join_condition(requestor, Some(player)) {
                // TODO(java): "Need correct message id".
                self.send(
                    requestor,
                    &SystemMessage::new(SystemMessageId::ThatPersonHasBeenSuccessfullyAddedToYourFriendList),
                );
                self.send(
                    player,
                    &SystemMessage::new(SystemMessageId::YouHaveAcceptedTheAlliance),
                );
                let clan_id = self.clan_id_of(player);
                let Some((ally_id, ally_name, ally_crest)) = self
                    .clan(requestor_clan_id)
                    .map(|c| (c.ally_id, c.ally_name.clone(), c.ally_crest_id))
                else {
                    return;
                };
                if let Some(c) = self.clan_mut(clan_id) {
                    c.ally_id = ally_id;
                    c.ally_name = ally_name;
                    c.ally_penalty_expiry_time = 0;
                    c.ally_penalty_type = 0;
                }
                self.change_ally_crest(clan_id, ally_crest, true);
                self.update_clan_in_db(clan_id);
            }
        }
        self.on_request_response(player);
    }

    /// Java `AllyLeave.runImpl`.
    pub(crate) fn on_ally_leave(&mut self, player: ObjectId) {
        let clan_id = self.clan_id_of(player);
        let Some(clan) = self.clan(clan_id) else {
            self.send_if_player(
                player,
                SystemMessageId::YouAreNotAClanMemberAndCannotPerformThisAction,
            );
            return;
        };
        if !self.is_clan_leader(player) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::OnlyTheClanLeaderMayApplyForWithdrawalFromTheAlliance),
            );
            return;
        }
        if clan.ally_id == 0 {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouAreNotCurrentlyAlliedWithAnyClans),
            );
            return;
        }
        if clan.id == clan.ally_id {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::AllianceLeadersCannotWithdraw),
            );
            return;
        }
        let now = unix_ms_now();
        if let Some(c) = self.clan_mut(clan_id) {
            c.ally_id = 0;
            c.ally_name = None;
        }
        self.change_ally_crest(clan_id, 0, true);
        let days = self.clans.settings.alt_ally_join_days_when_leaved;
        if let Some(c) = self.clan_mut(clan_id) {
            c.ally_penalty_expiry_time = now.wrapping_add(java_int_days_ms(days));
            c.ally_penalty_type = PENALTY_TYPE_CLAN_LEAVED;
        }
        self.update_clan_in_db(clan_id);
        self.send(
            player,
            &SystemMessage::new(SystemMessageId::YouHaveWithdrawnFromTheAlliance),
        );
    }

    /// Java `AllyDismiss.runImpl`.
    pub(crate) fn on_ally_dismiss(&mut self, player: ObjectId, clan_name: &str) {
        let leader_clan_id = self.clan_id_of(player);
        let Some(leader_clan) = self.clan(leader_clan_id) else {
            self.send_if_player(
                player,
                SystemMessageId::YouAreNotAClanMemberAndCannotPerformThisAction,
            );
            return;
        };
        let leader_ally = leader_clan.ally_id;
        if leader_ally == 0 {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouAreNotCurrentlyAlliedWithAnyClans),
            );
            return;
        }
        if !self.is_clan_leader(player) || leader_clan_id != leader_ally {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::ThisFeatureIsOnlyAvailableToAllianceLeaders),
            );
            return;
        }
        let Some((clan_id, clan_ally)) = self.clan_by_name(clan_name).map(|c| (c.id, c.ally_id)) else {
            self.send(player, &SystemMessage::new(SystemMessageId::ThatClanDoesNotExist));
            return;
        };
        if clan_id == leader_clan_id {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::AllianceLeadersCannotWithdraw),
            );
            return;
        }
        if clan_ally != leader_ally {
            self.send(player, &SystemMessage::new(SystemMessageId::DifferentAlliance));
            return;
        }
        let now = unix_ms_now();
        let settings = &self.clans.settings;
        let (dismiss_days, dismissed_days) = (
            settings.alt_accept_clan_days_when_dismissed,
            settings.alt_ally_join_days_when_dismissed,
        );
        if let Some(c) = self.clan_mut(leader_clan_id) {
            c.ally_penalty_expiry_time = now.wrapping_add(java_int_days_ms(dismiss_days));
            c.ally_penalty_type = PENALTY_TYPE_DISMISS_CLAN;
        }
        self.update_clan_in_db(leader_clan_id);

        if let Some(c) = self.clan_mut(clan_id) {
            c.ally_id = 0;
            c.ally_name = None;
        }
        self.change_ally_crest(clan_id, 0, true);
        if let Some(c) = self.clan_mut(clan_id) {
            c.ally_penalty_expiry_time = now.wrapping_add(java_int_days_ms(dismissed_days));
            c.ally_penalty_type = PENALTY_TYPE_CLAN_DISMISSED;
        }
        self.update_clan_in_db(clan_id);
        self.send(
            player,
            &SystemMessage::new(SystemMessageId::YouHaveSucceededInExpellingTheClan),
        );
    }

    /// Java `RequestDismissAlly.runImpl`.
    pub(crate) fn on_request_dismiss_ally(&mut self, player: ObjectId) {
        if !self.players.contains_key(&player) {
            return;
        }
        if !self.is_clan_leader(player) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::ThisFeatureIsOnlyAvailableToAllianceLeaders),
            );
            return;
        }
        self.dissolve_ally(player);
    }

    /// Java `new AllianceInfo(allianceId)`: `None` when the leading clan is gone (Java throws).
    pub(crate) fn alliance_info(&self, alliance_id: i32) -> Option<AllianceInfo> {
        let leader = self.clan(alliance_id)?;
        let mut allies = Vec::new();
        let (mut total, mut online) = (0i32, 0i32);
        for id in self.clan_allies(alliance_id) {
            let Some(clan) = self.clan(id) else {
                continue;
            };
            let members = i32::try_from(clan.members_count()).unwrap_or(i32::MAX);
            let on = i32::try_from(self.clan_online_members_count(id)).unwrap_or(i32::MAX);
            total = total.wrapping_add(members);
            online = online.wrapping_add(on);
            allies.push(AllianceClanInfo {
                clan_name: clan.name.clone(),
                level: clan.level,
                leader_name: self.clan_leader_name(id),
                total: members,
                online: on,
            });
        }
        Some(AllianceInfo {
            name: leader.ally_name.clone().unwrap_or_default(),
            total,
            online,
            leader_clan: leader.name.clone(),
            leader_player: self.clan_leader_name(alliance_id),
            allies,
        })
    }

    /// Java `RequestAllyInfo.runImpl`.
    pub(crate) fn on_request_ally_info(&mut self, player: ObjectId) {
        if !self.players.contains_key(&player) {
            return;
        }
        let ally_id = self.ally_id_of(player);
        if ally_id <= 0 {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouAreNotCurrentlyAlliedWithAnyClans),
            );
            return;
        }
        let Some(info) = self.alliance_info(ally_id) else {
            return;
        };
        let text = |s: &str| SystemMessageParam::Text(s.to_owned());
        self.send(player, &info);
        // send for player
        self.send(player, &SystemMessage::new(SystemMessageId::AllianceInformation));
        self.send(
            player,
            &SystemMessage::new(SystemMessageId::AllianceNameS1).with(text(&info.name)),
        );
        self.send(
            player,
            &SystemMessage::new(SystemMessageId::AllianceLeaderS2OfS1)
                .with(text(&info.leader_clan))
                .with(text(&info.leader_player)),
        );
        self.send(
            player,
            &SystemMessage::new(SystemMessageId::ConnectionS1TotalS2)
                .with(SystemMessageParam::IntNumber(info.online))
                .with(SystemMessageParam::IntNumber(info.total)),
        );
        self.send(
            player,
            &SystemMessage::new(SystemMessageId::AffiliatedClansTotalS1ClanS).with(
                SystemMessageParam::IntNumber(i32::try_from(info.allies.len()).unwrap_or(i32::MAX)),
            ),
        );
        // The first message of every round is the one the previous round ended with.
        let mut separator = SystemMessage::new(SystemMessageId::ClanInformation);
        for ally in &info.allies {
            self.send(player, &separator);
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::ClanNameS1).with(text(&ally.clan_name)),
            );
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::ClanLeaderS1).with(text(&ally.leader_name)),
            );
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::ClanLevelS1)
                    .with(SystemMessageParam::IntNumber(ally.level)),
            );
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::ConnectionS1TotalS2)
                    .with(SystemMessageParam::IntNumber(ally.online))
                    .with(SystemMessageParam::IntNumber(ally.total)),
            );
            separator = SystemMessage::new(SystemMessageId::Empty4);
        }
        self.send(player, &SystemMessage::new(SystemMessageId::Empty5));
    }

    fn send_if_player(&self, player: ObjectId, id: SystemMessageId) {
        if self.players.contains_key(&player) {
            self.send(player, &SystemMessage::new(id));
        }
    }
}

/// Java `String.trim()`: removes every leading and trailing character up to U+0020.
fn java_trim(text: &str) -> &str {
    text.trim_matches(|c: char| c <= ' ')
}
