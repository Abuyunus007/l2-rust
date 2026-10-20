//! Leaving the world: logout, restart, disconnect, kick and shutdown.
// Java: network/clientpackets/{Logout,RequestRestart}.java
// Java: network/Disconnection.java (storeAndDelete, storeAndDeleteWith)
// Java: Shutdown.disconnectAllCharacters, LoginServerThread.doKickPlayer
//
// Every way out stores the character first (`PersistCommand::StorePlayer` with the snapshot, then
// `PlayerLeft`), then the last packet of that way goes out, then the connection hears about it
// through `PacketSink::left_world`.
//
// What decides whether Java lets a player log out or restart (`canLogout`, combat stance,
// enchanting, store mode, class change, festival) is not modelled yet: the answer is always yes.
// When one of those systems arrives, this is where it refuses (RestartResponse(false) +
// ActionFailed for a restart, ActionFailed for a logout).

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{LeaveWorld, RestartResponse, ServerClose, SystemMessage};
use l2_util::ObjectId;
use tracing::info;

use crate::command::LeaveReason;
use crate::outbox::PersistCommand;
use crate::world::World;

impl World {
    /// `Logout.runImpl` / `RequestRestart.runImpl`, once the checks passed.
    pub(crate) fn on_logout(&mut self, id: ObjectId) {
        self.leave(id, LeaveReason::Logout);
    }

    pub(crate) fn on_restart(&mut self, id: ObjectId) {
        self.leave(id, LeaveReason::Restart);
    }

    /// `LoginServerThread.doKickPlayer`: `Disconnection.of(client).storeAndDeleteWith(msg)`.
    pub(crate) fn kick_account(&mut self, account: &str) {
        let found = self
            .players
            .values()
            .find(|p| p.data.account == account)
            .map(|p| p.core.object_id);
        if let Some(id) = found {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::AnotherPersonHasLoggedInWithTheSameAccount),
            );
            self.leave(id, LeaveReason::Kicked);
        }
    }

    /// Removes a player: final store, then `PlayerLeft`, the last packet of this way out, and
    /// finally the session hears about it.
    pub(crate) fn leave(&mut self, id: ObjectId, reason: LeaveReason) {
        if !self.players.contains_key(&id) {
            return;
        }
        // Java `deleteMe`: the Nevit logout listener and `storeRecommendations(false)` come before
        // the store (they change what is stored), `stopAllTimers` ends the vitality task.
        self.nevit_on_leave(id);
        self.forget_vitality(id);
        // `storeMe()` comes next: the snapshot is taken while the effects are still on (with them
        // the buffs to save, `store_snapshot`).
        let Some(snapshot) = self.store_snapshot(id) else {
            return;
        };
        // `Player.deleteMe` stops the toggles while the player is still in view.
        self.stop_all_toggles(id, true);
        // `Player.deleteMe`: "Deals with sudden exit in the middle of transaction."
        self.cancel_transaction_on_leave(id);
        let Some(_player) = self.players.remove(&id) else {
            return;
        };
        if let Some(session) = self.player_sessions.remove(&id) {
            self.sessions.remove(&session);
        }
        // Java: PlayerAutoSaveTaskManager.remove
        if let Some(timer) = self.autosave_timers.remove(&id) {
            self.timers.cancel(timer);
        }
        let account = snapshot.account.clone();
        info!("{} ({account}) left the world: {reason:?}.", snapshot.name);
        self.outbox
            .persist(PersistCommand::StorePlayer(Box::new(snapshot)));
        self.outbox.persist(PersistCommand::PlayerLeft {
            account,
            object_id: id,
            // Java sends `PlayerLogout` when the connection ends (`GameClient.onDisconnection`),
            // not when the player goes back to the character screen.
            logout: reason != LeaveReason::Restart,
        });
        // Java: `deleteMe()` after `storeMe()`: out of everybody's view, out of the world.
        self.decay(id);
        // `Creature.deleteMe`: every effect goes without broadcasting
        // (`stopAllEffectsWithoutExclusions(false, false)`); the messages still reach a client
        // that is connected. Then the `BuffFinishTask` stops with the object.
        self.stop_all_effects_without_exclusions(id, false, false);
        self.forget_effects(id);
        self.forget_combat(id);
        self.forget_death_state(id);
        // Java: the Player object is gone with its `_isInvul` and regeneration task (a new login is
        // a new object, so the invulnerability of the GM does not carry over).
        self.forget_status(id);
        // `Player.stopAllTimers` -> `stopPvpRegTask`; the flag goes with the object.
        self.forget_pvp(id);
        for other in self.players.values_mut() {
            other.known_relations.remove(&id);
        }
        let Some(sink) = self.sinks.remove(&id) else {
            return;
        };
        // The last packet of each way out. A client that disconnected gets none.
        match reason {
            // storeAndDeleteWith(LeaveWorld.STATIC_PACKET)
            LeaveReason::Logout => sink.send(l2_net::Encode::to_bytes(&LeaveWorld)),
            // RequestRestart: client.sendPacket(RestartResponse.valueOf(true))
            LeaveReason::Restart => sink.send(l2_net::Encode::to_bytes(&RestartResponse { result: true })),
            // Shutdown.disconnectAllCharacters: storeAndDeleteWith(ServerClose.STATIC_PACKET)
            LeaveReason::ServerShutdown => sink.send(l2_net::Encode::to_bytes(&ServerClose)),
            // The kick message was sent by `kick_account`.
            LeaveReason::Kicked | LeaveReason::Disconnected => {}
        }
        sink.left_world(reason);
    }
}
