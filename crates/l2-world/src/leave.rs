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
use crate::snapshot::snapshot_for_store;
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
        let Some(player) = self.players.remove(&id) else {
            return;
        };
        if let Some(session) = self.player_sessions.remove(&id) {
            self.sessions.remove(&session);
        }
        // Java: PlayerAutoSaveTaskManager.remove
        if let Some(timer) = self.autosave_timers.remove(&id) {
            self.timers.cancel(timer);
        }
        let snapshot = snapshot_for_store(&player, self.tick);
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
