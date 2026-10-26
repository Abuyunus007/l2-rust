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
// enchanting, class change, festival) is not modelled yet: the answer is always yes (a store
// refuses a restart).
// When one of those systems arrives, this is where it refuses (RestartResponse(false) +
// ActionFailed for a restart, ActionFailed for a logout).

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ActionFailed, LeaveWorld, RestartResponse, ServerClose, SystemMessage};
use l2_util::ObjectId;
use tracing::info;

use crate::command::LeaveReason;
use crate::outbox::PersistCommand;
use crate::world::World;

impl World {
    /// `Logout.runImpl` / `RequestRestart.runImpl`, once the checks passed.
    pub(crate) fn on_logout(&mut self, id: ObjectId) {
        // Java `Logout`: `canLogout()` is false while an enchant scroll or attribute stone is in
        // use.
        if self.enchant_blocks_logout(id) {
            self.send(id, &ActionFailed);
            return;
        }
        self.leave(id, LeaveReason::Logout);
    }

    pub(crate) fn on_restart(&mut self, id: ObjectId) {
        // Java `RequestRestart`: an enchant scroll or attribute stone in use.
        if self.enchant_blocks_logout(id) {
            self.send(id, &RestartResponse { result: false });
            self.send(id, &ActionFailed);
            return;
        }
        // Java `RequestRestart`: a player with a store open cannot restart.
        if self.is_in_store_mode(id) {
            self.send_message(id, "Cannot restart while trading.");
            self.send(id, &RestartResponse { result: false });
            return;
        }
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
        // Java `deleteMe`: `zone.onPlayerLogoutInside(this)` of the fortress siege zones, then
        // "Remove combat flag." (M7-FORT-01).
        self.fort_player_leaving(id);
        // `else if (_combatFlagEquippedId)`: a ward carrier drops the ward (M7-TW-01).
        self.territory_player_leaving(id);
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
        // `deleteMe`: `clanMember.setPlayer(null)` (the member keeps the player's values).
        let clan_leave = self.clan_before_leave(id);
        // `deleteMe`: "If a Party is in progress, leave it".
        self.leave_party(id);
        // `deleteMe`: the wyvern's skill goes; `stopAllTimers` stops the feed, stores the food of
        // the mounted pet and ends the rent.
        self.mount_on_leave(id);
        // `deleteMe`: `stopCubics()` (the cubic ids go; `UserInfo` is sent to the leaving client).
        self.stop_cubics(id);
        // `Player.deleteMe`: "Deals with sudden exit in the middle of transaction."
        self.cancel_transaction_on_leave(id);
        // `deleteMe`: "Stop crafting, if in progress" (`requestMakeItemAbort`).
        self.craft_on_player_leave(id);
        // `deleteMe`: `_taskForFish.cancel(false)`.
        self.fishing_on_player_leave(id);
        // `deleteMe`: `clearRefund()` destroys what the player sold to merchants.
        self.clear_refund(id);
        // `Player.deleteMe`: the summon is stored for a restore and unsummoned (Java does it right
        // after the player's `decayMe()`; here before, while the player is still in the tables).
        self.summon_on_owner_leave(id);
        // `deleteMe`: whoever channels a skill at the player stops.
        if self.is_channelized(id) {
            self.abort_channelization(id);
        }
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
        let leaving_name = snapshot.name.clone();
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
        // `deleteMe`: the other members see the player go offline.
        self.clan_after_leave(id, clan_leave);
        // `deleteMe`: `notifyFriends()` (the character is already out of the tables), then the
        // block list goes with the object (M6-FRIEND-01).
        self.friends_notify_offline(id, &leaving_name);
        self.forget_social_lists(id);
        // The flood protectors belong to the client (M6-FU-06).
        self.forget_flood(id, reason);
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
        // The duel state belongs to the Player object, which is gone.
        self.forget_duel(id);
        // `PlayerStat._maxCubicCount` goes with the object.
        self.forget_cubics(id);
        // `_siegeState`, `_siegeSide`, `_isInSiege` and the fame task (M7-SIEGE-01).
        self.forget_player_siege_state(id);
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
