//! TEMPORARY SHIM, replaced by `trade/request.rs` (M6-TRADE-01) when it is merged: the methods
//! here have the names and signatures of that file. Only `set_pending_invitation` and
//! `is_invitation_pending` (Java `Party`) stay.
//!
//! The pending request between two players: `Player.onTransactionRequest` and friends, which the
//! party invitation uses (trade, friend and clan invitations use them too).
// Java: entity/actor/Player.java (REQUEST_TIMEOUT, setActiveRequester, getActiveRequester,
//       isProcessingRequest, onTransactionRequest, isRequestExpired, onTransactionResponse)
// Java: entity/groups/Party.java (setPendingInvitation, isInvitationRequestExpired)
//
// Java compares the request's expiry with the game tick counter whenever somebody asks; no task
// fires at the end of the 15 seconds. The same is done here with the world's tick counter, which
// is why the pending invitation has no timer on the wheel: there is nothing to run when it ends.

use l2_util::ObjectId;

use crate::timer::TimerWheel;
use crate::world::World;

/// Java `Player.REQUEST_TIMEOUT`: seconds.
pub const REQUEST_TIMEOUT_SECS: u64 = 15;

/// The request state of every player that has one.
#[derive(Debug, Default)]
pub(crate) struct Requests {
    /// Java `_activeRequester`: who asked this player.
    active_requester: std::collections::HashMap<ObjectId, ObjectId>,
    /// Java `_requestExpireTime`: the tick this player's own request ends (it is the requester).
    expire_tick: std::collections::HashMap<ObjectId, u64>,
}

/// `REQUEST_TIMEOUT` in world ticks.
fn timeout_ticks() -> u64 {
    TimerWheel::ms_to_ticks(REQUEST_TIMEOUT_SECS * 1000)
}

impl World {
    /// Java `Player.setActiveRequester(requester)`.
    pub(crate) fn set_active_requester(&mut self, id: ObjectId, requester: Option<ObjectId>) {
        match requester {
            Some(requester) => {
                self.party.requests.active_requester.insert(id, requester);
            }
            None => {
                self.party.requests.active_requester.remove(&id);
            }
        }
    }

    /// Java `Player.getActiveRequester()`: the stored requester is dropped once its request ran out
    /// (`requester.isRequestExpired()`; the trade list exception needs trades).
    // NOTE: a requester who left the world is gone too. Java keeps the stale object, answers
    // through it and may even build a party around a player who is offline.
    pub(crate) fn active_requester(&mut self, id: ObjectId) -> Option<ObjectId> {
        let requester = self.party.requests.active_requester.get(&id).copied()?;
        if !self.players.contains_key(&requester) || self.is_request_expired(requester) {
            return None;
        }
        Some(requester)
    }

    /// Java `Player.isProcessingRequest()`.
    pub(crate) fn is_processing_request(&mut self, id: ObjectId) -> bool {
        self.active_requester(id).is_some()
            || self
                .party
                .requests
                .expire_tick
                .get(&id)
                .is_some_and(|expire| *expire > self.tick)
    }

    /// Java `Player.onTransactionRequest(partner)`: `id` asks `partner`, for 15 seconds.
    pub(crate) fn on_transaction_request(&mut self, id: ObjectId, partner: ObjectId) {
        let expire = self.tick + timeout_ticks();
        self.party.requests.expire_tick.insert(id, expire);
        self.set_active_requester(partner, Some(id));
    }

    /// Java `Player.isRequestExpired()`: `_requestExpireTime <= ticks` (0 when never set).
    pub(crate) fn is_request_expired(&self, id: ObjectId) -> bool {
        self.party
            .requests
            .expire_tick
            .get(&id)
            .is_none_or(|expire| *expire <= self.tick)
    }

    /// Java `Player.onTransactionResponse()`.
    pub(crate) fn on_transaction_response(&mut self, id: ObjectId) {
        self.party.requests.expire_tick.remove(&id);
    }

    /// What `Player.deleteMe` does about a pending request: `setActiveRequester(null)`. The
    /// requests this player made and the ones made to it are dropped with it.
    pub(crate) fn forget_requests(&mut self, id: ObjectId) {
        self.party.requests.active_requester.remove(&id);
        self.party.requests.expire_tick.remove(&id);
        self.party
            .requests
            .active_requester
            .retain(|_, requester| *requester != id);
        self.party.chosen_distribution.remove(&id);
    }

    /// Java `Party.setPendingInvitation(value)`: the timeout starts again whatever the value.
    pub(crate) fn set_pending_invitation(&mut self, party: super::PartyId, value: bool) {
        let timeout = self.tick + timeout_ticks();
        if let Some(p) = self.party.parties.get_mut(&party) {
            p.pending_invitation = value;
            p.pending_invite_timeout = timeout;
        }
    }

    /// Java `Party.getPendingInvitation() && !isInvitationRequestExpired()`.
    pub(crate) fn is_invitation_pending(&self, party: super::PartyId) -> bool {
        self.party
            .parties
            .get(&party)
            .is_some_and(|p| p.pending_invitation && p.pending_invite_timeout > self.tick)
    }
}
