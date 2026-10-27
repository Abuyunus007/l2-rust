//! The client packets of the party: invite, answer, leave, expel and the leader change.
// Java: network/clientpackets/RequestJoinParty.java
// Java: network/clientpackets/RequestAnswerJoinParty.java
// Java: network/clientpackets/RequestWithDrawalParty.java
// Java: network/clientpackets/RequestOustPartyMember.java
// Java: network/clientpackets/RequestChangePartyLeader.java

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    ActionFailed, AskJoinParty, JoinParty, PartyDistributionType, SystemMessage, SystemMessageParam,
};
use l2_util::ObjectId;

use super::{MAX_MEMBERS, PartyMessageType};
use crate::world::World;

fn text(value: &str) -> SystemMessageParam {
    SystemMessageParam::Text(value.to_owned())
}

/// The id of the `/partyinfo` user command (Java `PartyInfo.COMMAND_IDS`).
pub(crate) const PARTY_INFO_COMMAND: i32 = 81;

impl World {
    fn party_plain_message(&self, to: ObjectId, id: SystemMessageId) {
        self.send(to, &SystemMessage::new(id));
    }

    fn party_player_name(&self, id: ObjectId) -> String {
        self.players
            .get(&id)
            .map(|p| p.data.name.clone())
            .unwrap_or_default()
    }

    /// Java `RequestJoinParty.runImpl`. `distribution_type_id` is the raw int of the packet.
    pub(crate) fn on_request_join_party(
        &mut self,
        requestor: ObjectId,
        name: &str,
        distribution_type_id: i32,
    ) {
        if !self.players.contains_key(&requestor) {
            return;
        }
        let Some(target) = self.player_by_name(name) else {
            self.party_plain_message(
                requestor,
                SystemMessageId::YouMustFirstSelectAUserToInviteToYourParty,
            );
            return;
        };
        // `target.getClient() == null || isDetached()`: an offline trader.
        if !self.sinks.contains_key(&target) {
            self.send_message(requestor, "Player is in offline mode.");
            return;
        }
        let target_name = self.party_player_name(target);
        if self.is_party_banned(requestor) {
            self.party_plain_message(
                requestor,
                SystemMessageId::YouHaveBeenReportedAsAnIllegalProgramUserSoParticipatingInAPartyIsNotAllowed,
            );
            self.send(requestor, &ActionFailed);
            return;
        }
        if self.is_party_banned(target) {
            self.send(
                requestor,
                &SystemMessage::new(
                    SystemMessageId::C1HasBeenReportedAsAnIllegalProgramUserAndCannotJoinAParty,
                )
                .with(text(&target_name)),
            );
            return;
        }
        if !self.is_visible_for(target, requestor) {
            self.party_plain_message(requestor, SystemMessageId::ThatIsAnIncorrectTarget);
            return;
        }
        // Event participants (`isRegisteredOnEvent`): there are no events.
        if self.is_in_party(target) {
            self.send(
                requestor,
                &SystemMessage::new(SystemMessageId::C1IsAMemberOfAnotherPartyAndCannotBeInvited)
                    .with(text(&target_name)),
            );
            return;
        }
        // Java `BlockList.isBlocked(target, requestor)`: the target refuses messages or blocks the
        // requestor.
        if self.is_blocked(target, requestor) {
            self.send(
                requestor,
                &SystemMessage::new(SystemMessageId::S1HasPlacedYouOnHisHerIgnoreList)
                    .with(text(&target_name)),
            );
            return;
        }
        if target == requestor {
            self.party_plain_message(requestor, SystemMessageId::YouHaveInvitedTheWrongTarget);
            return;
        }
        // Cursed weapons, jail (`PunishmentManager`) and the olympiad rule: not modelled.
        if self.is_processing_request(requestor) {
            self.party_plain_message(requestor, SystemMessageId::WaitingForAnotherReply);
            return;
        }
        if self.is_processing_request(target) {
            self.send(
                requestor,
                &SystemMessage::new(SystemMessageId::C1IsOnAnotherTaskPleaseTryAgainLater)
                    .with(text(&target_name)),
            );
            return;
        }
        if self.is_in_party(requestor) && !self.is_party_leader(requestor) {
            self.party_plain_message(requestor, SystemMessageId::OnlyTheLeaderCanGiveOutInvitations);
            return;
        }

        self.send(
            requestor,
            &SystemMessage::new(SystemMessageId::C1HasBeenInvitedToTheParty).with(text(&target_name)),
        );
        if !self.is_in_party(requestor) {
            self.create_new_party_invitation(target, requestor, distribution_type_id);
        } else {
            // "You cannot invite a player when you are in the Dimensional Rift": no rift.
            self.add_target_to_party(target, requestor);
        }
    }

    /// `RequestJoinParty.addTargetToParty`.
    fn add_target_to_party(&mut self, target: ObjectId, requestor: ObjectId) {
        let Some(party) = self.party_of(requestor) else {
            return;
        };
        // Summary of people already in party and people that get invitation.
        if !self.is_party_leader(requestor) {
            self.party_plain_message(requestor, SystemMessageId::OnlyTheLeaderCanGiveOutInvitations);
            return;
        }
        if self.party_members(party).len() >= MAX_MEMBERS {
            self.party_plain_message(requestor, SystemMessageId::ThePartyIsFull);
            return;
        }
        if self.is_invitation_pending(party) {
            self.party_plain_message(requestor, SystemMessageId::WaitingForAnotherReply);
            return;
        }
        if !self.is_processing_request(target) {
            self.on_transaction_request(requestor, target);
            // In case a leader change has happened, use the party's mode.
            if let Some(distribution_type) = self.party_distribution_type(party) {
                self.send(
                    target,
                    &AskJoinParty {
                        requestor_name: self.party_player_name(requestor),
                        distribution_type,
                    },
                );
            }
            self.set_pending_invitation(party, true);
        } else {
            self.send(
                requestor,
                &SystemMessage::new(SystemMessageId::C1IsOnAnotherTaskPleaseTryAgainLater)
                    .with(text(&self.party_player_name(target))),
            );
        }
    }

    /// `RequestJoinParty.createNewParty`: the party itself is made when the answer is yes.
    fn create_new_party_invitation(
        &mut self,
        target: ObjectId,
        requestor: ObjectId,
        distribution_type_id: i32,
    ) {
        // NOTE(java-quirk): "C1 has been invited" went out before this, so an unknown loot rule
        // ends the invitation without a word.
        let Some(distribution_type) = PartyDistributionType::from_id(distribution_type_id) else {
            return;
        };
        if !self.is_processing_request(target) {
            self.send(
                target,
                &AskJoinParty {
                    requestor_name: self.party_player_name(requestor),
                    distribution_type,
                },
            );
            self.set_active_requester(target, Some(requestor));
            self.on_transaction_request(requestor, target);
            self.party
                .chosen_distribution
                .insert(requestor, distribution_type);
        } else {
            self.party_plain_message(requestor, SystemMessageId::WaitingForAnotherReply);
        }
    }

    /// Java `RequestAnswerJoinParty.runImpl`: `response` is 1 (yes), 0 (no) or -1 (the client
    /// refuses party requests).
    pub(crate) fn on_request_answer_join_party(&mut self, player: ObjectId, response: i32) {
        if !self.players.contains_key(&player) {
            return;
        }
        let Some(requestor) = self.active_requester(player) else {
            return;
        };
        // NOTE: Java keeps the stale requester object of a player who logged out and goes on:
        // `new Party(requestor, ...)` then builds a party around an offline leader. The request
        // ends here instead.
        if !self.players.contains_key(&requestor) {
            self.set_active_requester(player, None);
            return;
        }
        self.send(requestor, &JoinParty { response });

        match response {
            // Party disabled by the player's client config.
            -1 => {
                self.send(
                    requestor,
                    &SystemMessage::new(
                        SystemMessageId::C1IsSetToRefusePartyRequestsAndCannotReceiveAPartyRequest,
                    )
                    .with(SystemMessageParam::PlayerName(self.party_player_name(player))),
                );
            }
            // Party cancelled by the player: the client says it.
            0 => {}
            // Party accepted by the player.
            1 => {
                if let Some(party) = self.party_of(requestor) {
                    if self.party_members(party).len() >= MAX_MEMBERS {
                        let full = SystemMessage::new(SystemMessageId::ThePartyIsFull);
                        self.send(player, &full);
                        self.send(requestor, &full);
                        // NOTE(java-quirk): returns without ending the request, which then runs
                        // out by itself.
                        return;
                    }
                    self.join_party(player, party);
                } else {
                    // NOTE: `getPartyDistributionType()` is null in Java if the inviter never
                    // chose one, which fails later; the default rule stands in.
                    let distribution_type = self
                        .party
                        .chosen_distribution
                        .get(&requestor)
                        .copied()
                        .unwrap_or(PartyDistributionType::FindersKeepers);
                    let party = self.create_party(requestor, distribution_type);
                    self.join_party(player, party);
                }
                // The party matching room of the requestor learns of the new member.
                self.party_room_on_party_join(requestor, player);
            }
            _ => {}
        }

        if let Some(party) = self.party_of(requestor) {
            // If the party is null, there is no need of decreasing.
            self.set_pending_invitation(party, false);
        }
        self.set_active_requester(player, None);
        self.on_transaction_response(requestor);
    }

    /// Java `RequestWithDrawalParty.runImpl`.
    pub(crate) fn on_request_withdrawal_party(&mut self, player: ObjectId) {
        let Some(party) = self.party_of(player) else {
            return;
        };
        // Underground Coliseum and dimensional rift messages: neither exists.
        self.remove_party_member(party, player, PartyMessageType::Left);
        // Party matching room (`PartyMatchDetail`, `ExClosePartyRoom`).
        self.party_room_on_party_withdraw(player);
    }

    /// Java `RequestOustPartyMember.runImpl`.
    pub(crate) fn on_request_oust_party_member(&mut self, player: ObjectId, name: &str) {
        let Some(party) = self.party_of(player) else {
            return;
        };
        if !self.is_party_leader(player) {
            return;
        }
        // Underground Coliseum and dimensional rift messages: neither exists.
        if let Some(member) = self.party_member_by_name(party, name) {
            self.remove_party_member(party, member, PartyMessageType::Expelled);
        }
    }

    /// Java `RequestChangePartyLeader.runImpl`.
    pub(crate) fn on_request_change_party_leader(&mut self, player: ObjectId, name: &str) {
        let Some(party) = self.party_of(player) else {
            return;
        };
        let new_leader = self.party_member_by_name(party, name);
        if self.is_party_leader(player) {
            if new_leader.is_some() {
                self.set_party_leader(party, new_leader);
            } else {
                self.party_plain_message(
                    player,
                    SystemMessageId::YouMayOnlyTransferPartyLeadershipToAnotherMemberOfTheParty,
                );
            }
        } else if new_leader.is_some() {
            self.party_plain_message(
                player,
                SystemMessageId::OnlyTheLeaderOfThePartyCanTransferPartyLeadershipToAnotherPlayer,
            );
        }
    }

    /// Java `PartyInfo.onCommand` (`/partyinfo`): the loot rule, the leader and the member count.
    pub(crate) fn user_command_party_info(&mut self, player: ObjectId) {
        self.party_plain_message(player, SystemMessageId::PartyInformation);
        if let Some(party) = self.party_of(player) {
            let rule = match self.party_distribution_type(party) {
                Some(PartyDistributionType::FindersKeepers) => {
                    Some(SystemMessageId::LootingMethodFindersKeepers)
                }
                Some(PartyDistributionType::Random) => Some(SystemMessageId::LootingMethodRandom),
                Some(PartyDistributionType::RandomIncludingSpoil) => {
                    Some(SystemMessageId::LootingMethodRandomIncludingSpoil)
                }
                Some(PartyDistributionType::ByTurn) => Some(SystemMessageId::LootingMethodByTurn),
                Some(PartyDistributionType::ByTurnIncludingSpoil) => {
                    Some(SystemMessageId::LootingMethodByTurnIncludingSpoil)
                }
                None => None,
            };
            if let Some(rule) = rule {
                self.party_plain_message(player, rule);
            }
            if !self.is_party_leader(player) {
                let leader = self
                    .party_leader(party)
                    .map(|l| self.party_player_name(l))
                    .unwrap_or_default();
                self.send(
                    player,
                    &SystemMessage::new(SystemMessageId::PartyLeaderC1)
                        .with(SystemMessageParam::PlayerName(leader)),
                );
            }
            // NOTE: Java's own custom line ("TODO: Custom?").
            let count = self.party_members(party).len();
            self.send_message(player, &format!("Members: {count}/{MAX_MEMBERS}"));
        }
        self.party_plain_message(player, SystemMessageId::Empty3);
    }
}
