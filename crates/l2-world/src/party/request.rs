//! The pending invitation of a party (Java `Party.setPendingInvitation` and
//! `isInvitationRequestExpired`). The request between two players (`getActiveRequester`,
//! `onTransactionRequest` ...) is `trade/request.rs`, shared with trade, friend and duel requests.
// Java: entity/groups/Party.java (setPendingInvitation, isInvitationRequestExpired)
//
// Java compares the timeout with the game tick counter whenever somebody asks; no task fires at
// the end of the 15 seconds. The same is done here, so the pending invitation has no timer on the
// wheel: there is nothing to run when it ends.

use super::PartyId;
use crate::trade::{GAME_TICKS_PER_SECOND_I64, REQUEST_TIMEOUT};
use crate::world::World;

impl World {
    /// Java `Party.setPendingInvitation(value)`: the timeout starts again whatever the value.
    pub(crate) fn set_pending_invitation(&mut self, party: PartyId, value: bool) {
        let timeout = i64::from(self.game_ticks()) + i64::from(REQUEST_TIMEOUT) * GAME_TICKS_PER_SECOND_I64;
        if let Some(p) = self.party.parties.get_mut(&party) {
            p.pending_invitation = value;
            p.pending_invite_timeout = timeout;
        }
    }

    /// Java `Party.getPendingInvitation() && !isInvitationRequestExpired()`.
    pub(crate) fn is_invitation_pending(&self, party: PartyId) -> bool {
        let now = i64::from(self.game_ticks());
        self.party
            .parties
            .get(&party)
            .is_some_and(|p| p.pending_invitation && p.pending_invite_timeout > now)
    }
}
