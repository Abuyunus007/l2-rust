//! The clan packets of the client.
// Java: network/clientpackets/{RequestJoinPledge,RequestAnswerJoinPledge,RequestWithdrawalPledge,
//       RequestOustPledgeMember,RequestPledgeMemberList,RequestPledgeInfo,RequestPledgePower,
//       RequestPledgePowerGradeList,RequestPledgeMemberPowerInfo,RequestPledgeMemberInfo,
//       RequestPledgeSetMemberPowerGrade,RequestPledgeReorganizeMember,
//       RequestPledgeSetAcademyMaster}.java
// Java: entity/clan/Clan.java (checkClanJoinCondition)

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::pledge::{
    AskJoinPledge, JoinPledge, ManagePledgePower, PledgeShowMemberListDelete,
};
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use tracing::warn;

use super::model::{ClanAccess, ClanPrivilegesRef, SUBUNIT_ACADEMY};
use super::{ClanWrite, RequestPacket, days_ms};
use crate::world::{World, unix_ms_now};

impl World {
    /// Java `Clan.checkClanJoinCondition(player, target, pledgeType)`.
    pub(crate) fn check_clan_join_condition(
        &self,
        player: ObjectId,
        target: Option<ObjectId>,
        pledge_type: i32,
    ) -> bool {
        let clan_id = self.clan_id_of(player);
        let Some(clan) = self.clan(clan_id) else {
            return false;
        };
        if !self.has_clan_access(player, ClanAccess::InviteMember) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouAreNotAuthorizedToDoThat),
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
        let now = unix_ms_now();
        if clan.char_penalty_expiry_time > now {
            self.send(
                player,
                &SystemMessage::new(
                    SystemMessageId::AfterAClanMemberIsDismissedFromAClanTheClanMustWaitAtLeastADayBeforeAcceptingANewMember,
                ),
            );
            return false;
        }
        let target_state = self.player_clan_state(target);
        let target_name = self
            .players
            .get(&target)
            .map(|p| p.data.name.clone())
            .unwrap_or_default();
        if target_state.clan_id != 0 {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::S1IsAlreadyAMemberOfAnotherClan)
                    .with(SystemMessageParam::Text(target_name)),
            );
            return false;
        }
        if target_state.clan_join_expiry_time > now {
            self.send(
                player,
                &SystemMessage::new(
                    SystemMessageId::C1CannotJoinTheClanBecauseOneDayHasNotYetPassedSinceTheyLeftAnotherClan,
                )
                .with(SystemMessageParam::Text(target_name)),
            );
            return false;
        }
        let target_level = self.players.get(&target).map_or(0, |p| p.core.level);
        if (target_level > 40 || self.class_level_of(target) >= 2) && pledge_type == SUBUNIT_ACADEMY {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::S1DoesNotMeetTheRequirementsToJoinAClanAcademy)
                    .with(SystemMessageParam::Text(target_name)),
            );
            self.send(
                player,
                &SystemMessage::new(
                    SystemMessageId::ToJoinAClanAcademyCharactersMustBeLevel40OrBelowNotBelongAnotherClanAndNotYetCompletedTheir2ndClassTransfer,
                ),
            );
            return false;
        }
        if self.sub_pledge_members_count(clan_id, pledge_type) >= clan.max_members(pledge_type) {
            if pledge_type == 0 {
                self.send(
                    player,
                    &SystemMessage::new(
                        SystemMessageId::S1IsFullAndCannotAcceptAdditionalClanMembersAtThisTime,
                    )
                    .with(SystemMessageParam::Text(clan.name.clone())),
                );
            } else {
                self.send(
                    player,
                    &SystemMessage::new(
                        SystemMessageId::TheAcademyRoyalGuardOrderOfKnightsIsFullAndCannotAcceptNewMembersAtThisTime,
                    ),
                );
            }
            return false;
        }
        true
    }

    /// Java `getPlayerClass().level()`: 0 for the first class, 1 after the first transfer, ...
    fn class_level_of(&self, id: ObjectId) -> i32 {
        let Some(class_id) = self.players.get(&id).map(|p| p.data.class_id) else {
            return 0;
        };
        u16::try_from(class_id).map_or(0, |c| l2_data::ClassId(c).class_level())
    }

    /// Java `RequestJoinPledge.runImpl`.
    pub(crate) fn on_request_join_pledge(&mut self, player: ObjectId, target: i32, pledge_type: i32) {
        let clan_id = self.clan_id_of(player);
        if self.clan(clan_id).is_none() {
            return;
        }
        // `World.getPlayer(_target)`
        let target = Some(ObjectId(target)).filter(|t| self.players.contains_key(t));
        if target.is_none() {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouHaveInvitedTheWrongTarget),
            );
            return;
        }
        if !self.check_clan_join_condition(player, target, pledge_type) {
            return;
        }
        if !self.set_request(player, target, RequestPacket::JoinPledge { pledge_type }) {
            return;
        }
        let Some((target, clan)) = target.zip(self.clan(clan_id)) else {
            return;
        };
        let packet = AskJoinPledge {
            requestor_object_id: player.0,
            sub_pledge_name: clan.sub_pledge(pledge_type).map(|s| s.name.clone()),
            pledge_type,
            pledge_name: clan.name.clone(),
        };
        self.send(target, &packet);
    }

    /// Java `RequestAnswerJoinPledge.runImpl`.
    ///
    /// NOTE: Java keeps the requestor's `Player` object after it logged off and still lets the
    /// invited player join with it; here a requestor that left the world has no clan state, so
    /// the join conditions fail for it.
    pub(crate) fn on_request_answer_join_pledge(&mut self, player: ObjectId, answer: i32) {
        let Some(requestor) = self.request_partner(player) else {
            return;
        };
        let requestor_name = self
            .players
            .get(&requestor)
            .map(|p| p.data.name.clone())
            .unwrap_or_default();
        if answer == 0 {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouDidnTRespondToS1SInvitationJoiningHasBeenCancelled)
                    .with(SystemMessageParam::Text(requestor_name)),
            );
            let name = self
                .players
                .get(&player)
                .map(|p| p.data.name.clone())
                .unwrap_or_default();
            self.send(
                requestor,
                &SystemMessage::new(SystemMessageId::S1DidNotRespondInvitationToTheClanHasBeenCancelled)
                    .with(SystemMessageParam::Text(name)),
            );
        } else {
            // `requestor.getRequest().getRequestPacket() instanceof RequestJoinPledge`
            let Some(RequestPacket::JoinPledge { pledge_type }) = self.request_packet(requestor) else {
                return; // hax
            };
            let clan_id = self.clan_id_of(requestor);
            if self.clan(clan_id).is_none() {
                // `requestor.getClan()` is null: the call on it throws.
                return;
            }
            // "we must double check this cause during response time conditions can be changed,
            // i.e. another player could join clan"
            if self.check_clan_join_condition(requestor, Some(player), pledge_type) {
                self.join_clan(player, clan_id, pledge_type);
            }
        }
        self.on_request_response(player);
    }

    /// The accepted half of `RequestAnswerJoinPledge`.
    fn join_clan(&mut self, player: ObjectId, clan_id: i32, pledge_type: i32) {
        self.send(player, &JoinPledge { pledge_id: clan_id });
        let level = self.players.get(&player).map_or(0, |p| p.core.level);
        if let Some(s) = self.player_clan_state_mut(player) {
            s.pledge_type = pledge_type;
            if pledge_type == SUBUNIT_ACADEMY {
                s.power_grade = 9; // Academy
                s.lvl_joined_academy = level;
            } else {
                s.power_grade = 5; // new member starts at 5, not confirmed
            }
        }
        self.add_clan_member(clan_id, player);
        // `setClanPrivileges(clan.getRankPrivs(grade))`: the rank's own object (see
        // `ClanPrivilegesRef`).
        let grade = self.player_clan_state(player).power_grade;
        let has_rank = self.clan(clan_id).is_some_and(|c| c.has_rank(grade));
        if let Some(s) = self.player_clan_state_mut(player) {
            s.privileges = if has_rank {
                ClanPrivilegesRef::Rank(grade)
            } else {
                ClanPrivilegesRef::Own(0)
            };
        }
        self.send(player, &SystemMessage::new(SystemMessageId::EnteredTheClan));
        let name = self
            .players
            .get(&player)
            .map(|p| p.data.name.clone())
            .unwrap_or_default();
        self.broadcast_to_clan(
            clan_id,
            &SystemMessage::new(SystemMessageId::S1HasJoinedTheClan).with(SystemMessageParam::Text(name)),
        );
        // Residential skills of the clan's castle or fortress: M7.
        self.send_skill_list(player);
        if let Some(add) = self.pledge_member_add_of_player(player) {
            self.broadcast_to_other_clan_members(clan_id, &add, player);
        }
        self.broadcast_clan_info_update(clan_id);
        // "this activates the clan tab on the new member"
        self.send_pledge_show_member_list_all(clan_id, player);
        if let Some(s) = self.player_clan_state_mut(player) {
            s.clan_join_expiry_time = 0;
        }
        self.broadcast_user_info(player);
    }

    /// Java `RequestWithdrawalPledge.runImpl`.
    pub(crate) fn on_request_withdrawal_pledge(&mut self, player: ObjectId) {
        let clan_id = self.clan_id_of(player);
        if self.clan(clan_id).is_none() {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouAreNotAClanMemberAndCannotPerformThisAction),
            );
            return;
        }
        if self.is_clan_leader(player) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::AClanLeaderCannotWithdrawFromTheirOwnClan),
            );
            return;
        }
        if self.is_in_combat(player) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouCannotLeaveAClanWhileEngagedInCombat),
            );
            return;
        }
        let name = self
            .players
            .get(&player)
            .map(|p| p.data.name.clone())
            .unwrap_or_default();
        let join_days = self.clans.settings.alt_clan_join_days;
        self.remove_clan_member(
            clan_id,
            player,
            unix_ms_now().saturating_add(days_ms(join_days)),
            true,
        );
        self.broadcast_to_clan(
            clan_id,
            &SystemMessage::new(SystemMessageId::S1HasWithdrawnFromTheClan)
                .with(SystemMessageParam::Text(name.clone())),
        );
        // Remove the Player From the Member list.
        self.broadcast_to_clan(clan_id, &PledgeShowMemberListDelete { player_name: name });
        self.send(
            player,
            &SystemMessage::new(SystemMessageId::YouHaveWithdrawnFromTheClan),
        );
        self.send(
            player,
            &SystemMessage::new(
                SystemMessageId::AfterLeavingOrHavingBeenDismissedFromAClanYouMustWaitAtLeastADayBeforeJoiningAnotherClan,
            ),
        );
    }

    /// Java `RequestOustPledgeMember.runImpl`.
    pub(crate) fn on_request_oust_pledge_member(&mut self, player: ObjectId, target: &str) {
        let clan_id = self.clan_id_of(player);
        if self.clan(clan_id).is_none() {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouAreNotAClanMemberAndCannotPerformThisAction),
            );
            return;
        }
        if !self.has_clan_access(player, ClanAccess::RemoveMember) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouAreNotAuthorizedToDoThat),
            );
            return;
        }
        let my_name = self
            .players
            .get(&player)
            .map(|p| p.data.name.clone())
            .unwrap_or_default();
        if super::model::java_equals_ignore_case(&my_name, target) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouCannotDismissYourself),
            );
            return;
        }
        let Some(member) = self.clan_member_by_name(clan_id, target) else {
            warn!("Target ({target}) is not member of the clan");
            return;
        };
        if member.online && self.is_in_combat(member.object_id) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::AClanMemberMayNotBeDismissedDuringCombat),
            );
            return;
        }
        // This also updates the database.
        let join_days = self.clans.settings.alt_clan_join_days;
        let penalty = unix_ms_now().saturating_add(days_ms(join_days));
        self.remove_clan_member(clan_id, member.object_id, penalty, true);
        if let Some(clan) = self.clan_mut(clan_id) {
            clan.char_penalty_expiry_time = penalty;
        }
        self.update_clan_in_db(clan_id);

        self.broadcast_to_clan(
            clan_id,
            &SystemMessage::new(SystemMessageId::ClanMemberS1HasBeenExpelled)
                .with(SystemMessageParam::Text(member.name.clone())),
        );
        self.send(
            player,
            &SystemMessage::new(SystemMessageId::YouHaveSucceededInExpellingTheClanMember),
        );
        self.send(
            player,
            &SystemMessage::new(
                SystemMessageId::AfterAClanMemberIsDismissedFromAClanTheClanMustWaitAtLeastADayBeforeAcceptingANewMember,
            ),
        );
        // Remove the Player From the Member list.
        self.broadcast_to_clan(
            clan_id,
            &PledgeShowMemberListDelete {
                player_name: target.to_owned(),
            },
        );
        // `member.isOnline()` after the removal: the member keeps its player.
        if member.online && self.players.contains_key(&member.object_id) {
            self.send(
                member.object_id,
                &SystemMessage::new(
                    SystemMessageId::YouHaveRecentlyBeenDismissedFromAClanYouAreNotAllowedToJoinAnotherClanFor24Hours,
                ),
            );
        }
    }

    /// Java `RequestPledgeMemberList.runImpl`.
    pub(crate) fn on_request_pledge_member_list(&mut self, player: ObjectId) {
        let clan_id = self.clan_id_of(player);
        if self.clan(clan_id).is_some() {
            self.send_pledge_show_member_list_all(clan_id, player);
        }
    }

    /// Java `RequestPledgeInfo.runImpl`.
    pub(crate) fn on_request_pledge_info(&mut self, player: ObjectId, clan_id: i32) {
        // "we have no clan data ?!? should not happen"
        if let Some(clan) = self.clan(clan_id) {
            self.send(player, &Self::pledge_info(clan));
        }
    }

    /// Java `RequestPledgePower.runImpl`.
    pub(crate) fn on_request_pledge_power(&mut self, player: ObjectId, rank: i32, action: i32, privs: i32) {
        let clan_id = self.clan_id_of(player);
        if action == 2 {
            if self.is_clan_leader(player) {
                let mut privs = privs;
                if rank == 9 {
                    // The rights below cannot be bestowed upon Academy members: leaves only
                    // ACCESS_WAREHOUSE, HALL_OPEN_DOOR, CASTLE_OPEN_DOOR.
                    privs &= ClanAccess::AccessWarehouse.mask()
                        | ClanAccess::HallOpenDoor.mask()
                        | ClanAccess::CastleOpenDoor.mask();
                }
                self.set_rank_privs(clan_id, rank, privs);
            }
        } else if let Some(clan) = self.clan(clan_id) {
            // `ManagePledgePower` writes nothing for an action other than 1. Without a clan Java's
            // `writeImpl` throws on the null clan (nothing is sent).
            if action == 1 {
                let packet = ManagePledgePower {
                    privileges: clan.rank_privs(rank),
                };
                self.send(player, &packet);
            }
        }
    }

    /// Java `RequestPledgePowerGradeList.runImpl`.
    pub(crate) fn on_request_pledge_power_grade_list(&mut self, player: ObjectId) {
        if let Some(clan) = self.clan_of(player) {
            let packet = Self::pledge_power_grade_list(clan);
            self.send(player, &packet);
        }
    }

    /// Java `RequestPledgeMemberPowerInfo.runImpl`.
    pub(crate) fn on_request_pledge_member_power_info(&mut self, player: ObjectId, name: &str) {
        let clan_id = self.clan_id_of(player);
        if let Some(member) = self.clan_member_by_name(clan_id, name) {
            let packet = self.pledge_receive_power_info(clan_id, &member);
            self.send(player, &packet);
        }
    }

    /// Java `RequestPledgeMemberInfo.runImpl`.
    pub(crate) fn on_request_pledge_member_info(&mut self, player: ObjectId, name: &str) {
        let clan_id = self.clan_id_of(player);
        if let Some(member) = self.clan_member_by_name(clan_id, name)
            && let Some(packet) = self.pledge_receive_member_info(clan_id, &member)
        {
            self.send(player, &packet);
        }
    }

    /// Java `RequestPledgeSetMemberPowerGrade.runImpl`.
    pub(crate) fn on_request_pledge_set_member_power_grade(
        &mut self,
        player: ObjectId,
        name: &str,
        power_grade: i32,
    ) {
        let clan_id = self.clan_id_of(player);
        let Some(clan) = self.clan(clan_id) else {
            return;
        };
        let leader_id = clan.leader_id();
        if !self.has_clan_access(player, ClanAccess::ModifyRanks) {
            return;
        }
        let Some(member) = self.clan_member_by_name(clan_id, name) else {
            return;
        };
        if member.object_id.0 == leader_id {
            return;
        }
        if member.pledge_type == SUBUNIT_ACADEMY {
            // Also checked from client side.
            self.send_message(player, "You cannot change academy member grade");
            return;
        }
        self.set_member_power_grade(clan_id, member.object_id, power_grade);
        // "An online member keeps the privileges of the old rank until refreshed": a copy of the
        // rank's privileges (`clone()`).
        if member.online {
            let mask = self.clan(clan_id).map_or(0, |c| c.rank_privs(power_grade));
            if let Some(s) = self.player_clan_state_mut(member.object_id) {
                s.privileges = ClanPrivilegesRef::Own(mask);
            }
            self.send_user_info(member.object_id);
        }
        self.broadcast_clan_status(clan_id);
        if let Some(info) = self.clan_member_info(clan_id, member.object_id) {
            let packet = self.pledge_receive_power_info(clan_id, &info);
            self.send(player, &packet);
        }
    }

    /// Java `ClanMember.setPowerGrade(powerGrade)`: the player's value when online, the row
    /// otherwise.
    fn set_member_power_grade(&mut self, clan_id: i32, id: ObjectId, power_grade: i32) {
        let online = self.clan_member_info(clan_id, id).is_some_and(|m| m.online);
        if let Some(m) = self.clan_mut(clan_id).and_then(|c| c.member_mut(id)) {
            m.power_grade = power_grade;
        }
        if online {
            if let Some(s) = self.player_clan_state_mut(id) {
                s.power_grade = power_grade;
            }
        } else {
            // Db save if char not logged in.
            self.clan_write(ClanWrite::CharPowerGrade {
                char_id: id.0,
                power_grade,
            });
        }
    }

    /// Java `ClanMember.setPledgeType(pledgeType)`.
    fn set_member_pledge_type(&mut self, clan_id: i32, id: ObjectId, pledge_type: i32) {
        let online = self.clan_member_info(clan_id, id).is_some_and(|m| m.online);
        if let Some(m) = self.clan_mut(clan_id).and_then(|c| c.member_mut(id)) {
            m.pledge_type = pledge_type;
        }
        if online {
            if let Some(s) = self.player_clan_state_mut(id) {
                s.pledge_type = pledge_type;
            }
        } else {
            // Db save if char not logged in.
            self.clan_write(ClanWrite::CharSubPledge {
                char_id: id.0,
                pledge_type,
            });
        }
    }

    /// Java `RequestPledgeReorganizeMember.runImpl`.
    pub(crate) fn on_request_pledge_reorganize_member(
        &mut self,
        player: ObjectId,
        is_member_selected: i32,
        member_name: &str,
        new_pledge_type: i32,
        selected_member: &str,
    ) {
        if is_member_selected == 0 {
            return;
        }
        let clan_id = self.clan_id_of(player);
        let Some(clan) = self.clan(clan_id) else {
            return;
        };
        let leader_id = clan.leader_id();
        if !self.has_clan_access(player, ClanAccess::ModifyRanks) {
            return;
        }
        let Some(member1) = self
            .clan_member_by_name(clan_id, member_name)
            .filter(|m| m.object_id.0 != leader_id)
        else {
            return;
        };
        let Some(member2) = self
            .clan_member_by_name(clan_id, selected_member)
            .filter(|m| m.object_id.0 != leader_id)
        else {
            return;
        };
        let old_pledge_type = member1.pledge_type;
        if old_pledge_type == new_pledge_type {
            return;
        }
        self.set_member_pledge_type(clan_id, member1.object_id, new_pledge_type);
        self.set_member_pledge_type(clan_id, member2.object_id, old_pledge_type);
        self.broadcast_clan_status(clan_id);
    }

    /// Java `RequestPledgeSetAcademyMaster.runImpl`: links an academy member and its sponsor, or
    /// ends the link.
    pub(crate) fn on_request_pledge_set_academy_master(
        &mut self,
        player: ObjectId,
        set: i32,
        curr_player_name: &str,
        target_player_name: &str,
    ) {
        let clan_id = self.clan_id_of(player);
        if self.clan(clan_id).is_none() {
            return;
        }
        if !self.has_clan_access(player, ClanAccess::DismissMentee) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouDoNotHaveTheRightToDismissAnApprentice),
            );
            return;
        }
        let (Some(current), Some(target)) = (
            self.clan_member_by_name(clan_id, curr_player_name),
            self.clan_member_by_name(clan_id, target_player_name),
        ) else {
            return;
        };
        let (apprentice, sponsor) = if current.pledge_type == SUBUNIT_ACADEMY {
            (current, target)
        } else {
            (target, current)
        };
        let message = if set == 0 {
            // "test: do we get the current sponsor & apprentice from this packet or no?"
            self.set_link(clan_id, apprentice.object_id, apprentice.online, None, Some(0));
            self.set_link(clan_id, sponsor.object_id, sponsor.online, Some(0), None);
            self.save_apprentice_and_sponsor(apprentice.object_id, 0, 0);
            self.save_apprentice_and_sponsor(sponsor.object_id, 0, 0);
            SystemMessageId::S2ClanMemberC1SApprenticeHasBeenRemoved
        } else {
            if apprentice.sponsor != 0
                || sponsor.apprentice != 0
                || apprentice.apprentice != 0
                || sponsor.sponsor != 0
            {
                // TODO retail message
                self.send_message(player, "Remove previous connections first.");
                return;
            }
            self.set_link(
                clan_id,
                apprentice.object_id,
                apprentice.online,
                None,
                Some(sponsor.object_id.0),
            );
            self.set_link(
                clan_id,
                sponsor.object_id,
                sponsor.online,
                Some(apprentice.object_id.0),
                None,
            );
            // "saving to database even if online, since both must match"
            self.save_apprentice_and_sponsor(apprentice.object_id, 0, sponsor.object_id.0);
            self.save_apprentice_and_sponsor(sponsor.object_id, apprentice.object_id.0, 0);
            SystemMessageId::S2HasBeenDesignatedAsTheApprenticeOfClanMemberS1
        };
        let sm = SystemMessage::new(message)
            .with(SystemMessageParam::Text(sponsor.name.clone()))
            .with(SystemMessageParam::Text(apprentice.name.clone()));
        // `sponsor != player && sponsor != apprentice` (the players, null when offline).
        let sponsor_player = sponsor.online.then_some(sponsor.object_id);
        let apprentice_player = apprentice.online.then_some(apprentice.object_id);
        if sponsor_player != Some(player) && sponsor_player != apprentice_player {
            self.send(player, &sm);
        }
        if let Some(id) = sponsor_player {
            self.send(id, &sm);
        }
        if let Some(id) = apprentice_player {
            self.send(id, &sm);
        }
    }

    /// The link half of `RequestPledgeSetAcademyMaster`: an online member's player gets the new
    /// value (`setApprentice` / `setSponsor`), an offline member `setApprenticeAndSponsor(a, s)`
    /// with the other value 0.
    fn set_link(
        &mut self,
        clan_id: i32,
        id: ObjectId,
        online: bool,
        apprentice: Option<i32>,
        sponsor: Option<i32>,
    ) {
        if online {
            if let Some(s) = self.player_clan_state_mut(id) {
                if let Some(a) = apprentice {
                    s.apprentice = a;
                }
                if let Some(sp) = sponsor {
                    s.sponsor = sp;
                }
            }
        } else if let Some(m) = self.clan_mut(clan_id).and_then(|c| c.member_mut(id)) {
            m.apprentice = apprentice.unwrap_or(0);
            m.sponsor = sponsor.unwrap_or(0);
        }
    }

    /// Java `ClanMember.saveApprenticeAndSponsor(apprentice, sponsor)`.
    fn save_apprentice_and_sponsor(&mut self, id: ObjectId, apprentice: i32, sponsor: i32) {
        self.clan_write(ClanWrite::CharApprenticeSponsor {
            char_id: id.0,
            apprentice,
            sponsor,
        });
    }
}
