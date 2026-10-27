//! Changing the loot rule of a running party by vote.
// Java: entity/groups/Party.java (requestLootChange, answerLootChangeRequest, finishLootRequest,
//       PARTY_DISTRIBUTION_TYPE_REQUEST_TIMEOUT)
// Java: network/clientpackets/{RequestPartyLootModification,AnswerPartyLootModification}.java
// Java: network/serverpackets/{ExAskModifyPartyLooting,ExSetPartyLooting}.java
//
// The leader asks, every other member answers, and the rule changes when all of them agreed. One
// "no" or 15 seconds without all answers cancel the change. A member who joins during the vote
// cancels it (`addPartyMember`). The 15 s task is a timer of the wheel.

use std::collections::HashSet;

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    ExAskModifyPartyLooting, ExSetPartyLooting, PartyDistributionType, SystemMessage, SystemMessageParam,
};
use l2_util::ObjectId;

use super::PartyId;
use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::World;

/// Java `Party.PARTY_DISTRIBUTION_TYPE_REQUEST_TIMEOUT`: 15 seconds.
const LOOT_REQUEST_TIMEOUT_MS: u64 = 15_000;

/// The open vote: Java `_changeRequestDistributionType`, `_changeDistributionTypeAnswers` and
/// `_changeDistributionTypeRequestTask`.
#[derive(Debug, Clone)]
pub(crate) struct LootChange {
    pub(crate) distribution_type: PartyDistributionType,
    /// The members who said yes.
    pub(crate) answers: HashSet<ObjectId>,
    pub(crate) timer: TimerId,
}

impl World {
    /// Java `RequestPartyLootModification.runImpl`; `distribution_type_id` is the raw int.
    pub(crate) fn on_request_party_loot_modification(&mut self, player: ObjectId, distribution_type_id: i32) {
        let Some(distribution_type) = PartyDistributionType::from_id(distribution_type_id) else {
            return;
        };
        let Some(party) = self.party_of(player) else {
            return;
        };
        if !self.is_party_leader(player) || self.party_distribution_type(party) == Some(distribution_type) {
            return;
        }
        self.party_request_loot_change(party, distribution_type);
    }

    /// Java `AnswerPartyLootModification.runImpl`.
    pub(crate) fn on_answer_party_loot_modification(&mut self, player: ObjectId, answer: i32) {
        if let Some(party) = self.party_of(player) {
            self.party_answer_loot_change_request(party, player, answer == 1);
        }
    }

    /// Java `Party.requestLootChange(type)`.
    pub(crate) fn party_request_loot_change(
        &mut self,
        party: PartyId,
        distribution_type: PartyDistributionType,
    ) {
        let Some(p) = self.party.parties.get(&party) else {
            return;
        };
        if p.loot_change.is_some() {
            return;
        }
        let Some(leader) = p.leader() else {
            return;
        };
        let due = self.tick + TimerWheel::ms_to_ticks(LOOT_REQUEST_TIMEOUT_MS);
        let timer = self.timers.schedule_at(due, TimerKind::PartyLootRequest(party));
        if let Some(p) = self.party.parties.get_mut(&party) {
            p.loot_change = Some(LootChange {
                distribution_type,
                answers: HashSet::new(),
                timer,
            });
        }
        let leader_name = self
            .players
            .get(&leader)
            .map(|l| l.data.name.clone())
            .unwrap_or_default();
        self.broadcast_party_except(
            party,
            leader,
            &ExAskModifyPartyLooting {
                requestor: leader_name,
                distribution_type,
            },
        );
        self.send(
            leader,
            &SystemMessage::new(SystemMessageId::RequestingApprovalForChangingPartyLootToS1).with(
                SystemMessageParam::SystemString(distribution_type.sys_string_id()),
            ),
        );
    }

    /// Java `Party.answerLootChangeRequest(member, answer)`.
    pub(crate) fn party_answer_loot_change_request(
        &mut self,
        party: PartyId,
        member: ObjectId,
        answer: bool,
    ) {
        let member_count = self.party_members(party).len();
        let Some(change) = self
            .party
            .parties
            .get_mut(&party)
            .and_then(|p| p.loot_change.as_mut())
        else {
            return;
        };
        if change.answers.contains(&member) {
            return;
        }
        if !answer {
            self.party_finish_loot_request(party, false);
            return;
        }
        change.answers.insert(member);
        // NOTE(java-quirk): the leader is a member too, so an answer of the leader counts, and the
        // vote is over one answer early then.
        if change.answers.len() + 1 >= member_count {
            self.party_finish_loot_request(party, true);
        }
    }

    /// Java `Party.finishLootRequest(success)`.
    pub(crate) fn party_finish_loot_request(&mut self, party: PartyId, success: bool) {
        let Some(change) = self
            .party
            .parties
            .get_mut(&party)
            .and_then(|p| p.loot_change.take())
        else {
            return;
        };
        self.timers.cancel(change.timer);
        if success {
            self.broadcast_party(
                party,
                &ExSetPartyLooting {
                    result: 1,
                    distribution_type: change.distribution_type,
                },
            );
            if let Some(p) = self.party.parties.get_mut(&party) {
                p.distribution_type = change.distribution_type;
            }
            self.broadcast_party(
                party,
                &SystemMessage::new(SystemMessageId::PartyLootWasChangedToS1).with(
                    SystemMessageParam::SystemString(change.distribution_type.sys_string_id()),
                ),
            );
        } else {
            if let Some(current) = self.party_distribution_type(party) {
                self.broadcast_party(
                    party,
                    &ExSetPartyLooting {
                        result: 0,
                        distribution_type: current,
                    },
                );
            }
            self.broadcast_party(
                party,
                &SystemMessage::new(SystemMessageId::PartyLootChangeWasCancelled),
            );
        }
    }

    /// The 15 s task of the vote.
    pub(crate) fn on_party_loot_request_timer(&mut self, party: PartyId) {
        self.party_finish_loot_request(party, false);
    }
}
