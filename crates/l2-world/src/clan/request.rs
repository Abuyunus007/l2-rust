//! Java `RequestPartner`: the invitation between two players that waits for an answer (a clan
//! invitation here; alliances and friends use it too).
// Java: network/holders/RequestPartner.java
//
// Java keeps one `RequestPartner` per player with the partner and the request packet; the
// invitation ends with the answer (`onRequestResponse`) or 15 seconds later. Here the state of
// every player with a request lives in `ClanWorld::requests`, the partner by id.
//
// NOTE(java-quirk): `setOnRequestTimer` schedules `clear` 15 seconds later and never cancels it,
// so the timer of an earlier request also clears a newer one. The timers here are not cancelled
// either.

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;

use super::ClanTimer;
use crate::timer::{TimerKind, TimerWheel};
use crate::world::World;

/// Java `RequestPartner.REQUEST_TIMEOUT` (seconds).
pub const REQUEST_TIMEOUT_SECS: u64 = 15;

/// The packet that started a request (Java keeps the `ClientPacket` itself).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestPacket {
    /// `RequestJoinPledge` with its pledge type.
    JoinPledge { pledge_type: i32 },
}

/// One player's request state (Java `RequestPartner` fields).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RequestPartner {
    pub partner: Option<ObjectId>,
    pub packet: Option<RequestPacket>,
    pub is_requestor: bool,
    pub is_answerer: bool,
}

impl World {
    /// Java `getRequest().isProcessingRequest()`.
    pub fn is_processing_request(&self, id: ObjectId) -> bool {
        self.clans.requests.get(&id).is_some_and(|r| r.partner.is_some())
    }

    /// Java `getRequest().getPartner()`.
    pub fn request_partner(&self, id: ObjectId) -> Option<ObjectId> {
        self.clans.requests.get(&id).and_then(|r| r.partner)
    }

    /// Java `getRequest().getRequestPacket()`.
    pub fn request_packet(&self, id: ObjectId) -> Option<RequestPacket> {
        self.clans.requests.get(&id).and_then(|r| r.packet)
    }

    /// Java `getRequest().setRequest(partner, packet)`: both players wait for the answer. `false`
    /// (with Java's message to `player`) when either is busy.
    pub(crate) fn set_request(
        &mut self,
        player: ObjectId,
        partner: Option<ObjectId>,
        packet: RequestPacket,
    ) -> bool {
        let Some(partner) = partner else {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouHaveInvitedTheWrongTarget),
            );
            return false;
        };
        if self.is_processing_request(partner) {
            let name = self
                .players
                .get(&partner)
                .map(|p| p.data.name.clone())
                .unwrap_or_default();
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::C1IsOnAnotherTaskPleaseTryAgainLater)
                    .with(SystemMessageParam::Text(name)),
            );
            return false;
        }
        if self.is_processing_request(player) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::WaitingForAnotherReply),
            );
            return false;
        }
        let me = self.clans.requests.entry(player).or_default();
        me.partner = Some(partner);
        me.packet = Some(packet);
        self.set_on_request_timer(player, true);
        let other = self.clans.requests.entry(partner).or_default();
        other.partner = Some(player);
        other.packet = Some(packet);
        self.set_on_request_timer(partner, false);
        true
    }

    /// Java `setOnRequestTimer(isRequestor)`.
    fn set_on_request_timer(&mut self, id: ObjectId, is_requestor: bool) {
        let request = self.clans.requests.entry(id).or_default();
        request.is_requestor = is_requestor;
        request.is_answerer = !is_requestor;
        let due = self.tick + TimerWheel::ms_to_ticks(REQUEST_TIMEOUT_SECS * 1000);
        self.timers
            .schedule_at(due, TimerKind::Clan(ClanTimer::RequestClear(id)));
    }

    /// Java `getRequest().onRequestResponse()`: the request of both players ends.
    pub(crate) fn on_request_response(&mut self, id: ObjectId) {
        if let Some(partner) = self.request_partner(id) {
            self.clear_request(partner);
        }
        self.clear_request(id);
    }

    /// Java `RequestPartner.clear()`.
    pub(crate) fn clear_request(&mut self, id: ObjectId) {
        self.clans.requests.remove(&id);
    }
}
