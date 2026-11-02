//! Offline play (`config/Custom/OfflinePlay.ini`, `EnableOfflinePlayCommand`, off by default): a
//! player who is auto playing types `.offlineplay`, the client goes and the character keeps
//! hunting.
// Java: dist/game/data/scripts/handlers/chat/commands/voiced/OfflinePlay.java
// Java: network/clientpackets/DlgAnswer.java (the `OFFLINE_PLAY` branch)
// Java: entity/actor/Player.java (startOfflinePlay, isOfflinePlay, setOfflinePlay)
// Java: taskmanagers/DecayTaskManager.java (`OfflinePlayLogoutOnDeath`)
// Java: config/custom/OfflinePlayConfig.java
//
// # The character stays without a client
// Like an offline trader (`trade/offline.rs`): the player is in `World::players` without an entry
// in `World::sinks`. `OfflineTradeDatabaseWrites` (D-52) decides the account hold too: only with
// it does the login server hear `PlayerLogout` at once; without it the account stays on this
// server, a login of that account anywhere kicks the character (`release_offline_account`) and
// its final `PlayerLeft` frees the account.
//
// # Left out
// - `RestoreAutoPlayOffliners` (`OfflinePlayTable`: the `character_offline_play` rows, the restore
//   at startup and `storeOfflineGroups` at shutdown). Java's own start would restore the same
//   character while this server plays it (D-52), so it needs an owner decision on a switch; the
//   offline players of this server live until it stops.
// - `OfflinePlayDisconnectSameAccount` (the character list of the account would store and delete
//   the character): the character list is built outside the world; the account hold above already
//   removes the character when the account logs in.
// - `AntiFeedManager` (`DualboxCheckConfig`): the dual-box counters are another feature.

use std::collections::BTreeSet;

use l2_data::skill::AbnormalVisualEffect;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    ActionFailed, ChatType, ConfirmDlg, CreatureSay, ExShowScreenMessage, LeaveWorld, SayBody, SenderField,
    SystemMessage, SystemMessageParam,
};
use l2_util::ObjectId;

use crate::command::LeaveReason;
use crate::outbox::LsEvent;
use crate::world::World;
use crate::zones::ZoneId;

/// Java `OfflinePlayConfig`. `Default` is the Java default of each key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfflinePlaySettings {
    /// `EnableOfflinePlayCommand`.
    pub enable_offline_play_command: bool,
    /// `RestoreAutoPlayOffliners` (see the module comment: not ported).
    pub restore_auto_play_offliners: bool,
    /// `OfflinePlayPremium`.
    pub offline_play_premium: bool,
    /// `OfflinePlayLogoutOnDeath`.
    pub offline_play_logout_on_death: bool,
    /// `OfflinePlayDisconnectSameAccount` (see the module comment: not ported).
    pub offline_play_disconnect_same_account: bool,
    /// `OfflinePlayLoginMessage`.
    pub offline_play_login_message: String,
    /// `OfflinePlaySetNameColor`.
    pub offline_play_set_name_color: bool,
    /// `OfflinePlayNameColor` (`Integer.decode("0x" + value)`, default 808080).
    pub offline_play_name_color: i32,
    /// `OfflinePlayAbnormalEffect`.
    pub offline_play_abnormal_effects: Vec<AbnormalVisualEffect>,
}

impl Default for OfflinePlaySettings {
    fn default() -> Self {
        Self {
            enable_offline_play_command: false,
            restore_auto_play_offliners: true,
            offline_play_premium: false,
            offline_play_logout_on_death: true,
            offline_play_disconnect_same_account: false,
            offline_play_login_message: String::new(),
            offline_play_set_name_color: false,
            offline_play_name_color: 0x0080_8080,
            offline_play_abnormal_effects: Vec::new(),
        }
    }
}

/// What the world keeps for the offline play.
#[derive(Debug, Default)]
pub(crate) struct OfflinePlayWorld {
    pub(crate) settings: OfflinePlaySettings,
    /// The players who play without a client (`Player._offlinePlay`).
    players: BTreeSet<ObjectId>,
    /// The players who answer the `.offlineplay` question (`PlayerAction.OFFLINE_PLAY`).
    asked: BTreeSet<ObjectId>,
}

impl World {
    /// Installs `OfflinePlay.ini`. Call before players enter.
    pub fn set_offline_play_settings(&mut self, settings: OfflinePlaySettings) {
        self.offline_play.settings = settings;
    }

    /// Java `Player.isOfflinePlay()`.
    pub fn is_offline_play(&self, id: ObjectId) -> bool {
        self.offline_play.players.contains(&id)
    }

    /// The name colour `OfflinePlaySetNameColor` gives a player who plays offline.
    pub(crate) fn offline_play_name_color(&self, id: ObjectId) -> Option<i32> {
        (self.is_offline_play(id) && self.offline_play.settings.offline_play_set_name_color)
            .then_some(self.offline_play.settings.offline_play_name_color)
    }

    /// `DecayTaskManager.add`: whether the dead player is logged out after ten seconds.
    pub(crate) fn offline_play_logout_on_death(&self, id: ObjectId) -> bool {
        self.is_offline_play(id) && self.offline_play.settings.offline_play_logout_on_death
    }

    /// `Player.setAutoPlaying(false)` of an offline player removes its rows
    /// (`RESTORE_AUTO_PLAY_OFFLINERS`); the rows are not written (see the module comment).
    pub(crate) fn offline_play_auto_play_stopped(&mut self, _id: ObjectId) {}

    /// `ON_PLAYER_LOGIN` of `OfflinePlay`: the login message.
    pub(crate) fn offline_play_on_enter(&mut self, id: ObjectId) {
        let settings = &self.offline_play.settings;
        if settings.enable_offline_play_command && !settings.offline_play_login_message.is_empty() {
            let text = settings.offline_play_login_message.clone();
            self.send_announcement_say(id, "OfflinePlay", &text);
        }
    }

    /// The player leaves the world: it plays no more.
    pub(crate) fn offline_play_on_leave(&mut self, id: ObjectId) {
        self.offline_play.players.remove(&id);
        self.offline_play.asked.remove(&id);
    }

    /// Whether the player was an offline player (it is not any more); used by `leave` for the
    /// account hold of D-52.
    pub(crate) fn offline_play_on_delete(&mut self, id: ObjectId) -> bool {
        self.offline_play.players.remove(&id)
    }

    /// `new CreatureSay(null, ChatType.ANNOUNCEMENT, name, text)`.
    pub(crate) fn send_announcement_say(&self, to: ObjectId, name: &str, text: &str) {
        self.send(
            to,
            &CreatureSay {
                sender_object_id: 0,
                chat_type: ChatType::Announcement,
                sender: SenderField::Name(name.to_owned()),
                message_id: -1,
                body: SayBody::Text(text.to_owned()),
            },
        );
    }

    /// `OfflinePlay.onCommand` through `VoicedCommandHandler.getHandler(word)`.
    pub(crate) fn offline_play_voiced_handler(&mut self, player: ObjectId, word: &str) -> Option<bool> {
        if !self.offline_play.settings.enable_offline_play_command || word != "offlineplay" {
            return None;
        }
        let refuse = |world: &mut World, text: &str| {
            world.send(player, &ExShowScreenMessage::text(text, 5000));
            world.send_message(player, text);
            world.send(player, &ActionFailed);
            Some(false)
        };
        if self.offline_play.settings.offline_play_premium && !self.has_premium_status(player) {
            return refuse(self, "This command is only available to premium players.");
        }
        if !self.is_auto_playing(player) {
            return refuse(self, "You need to enable auto play before exiting.");
        }
        if self.is_in_vehicle(player) || self.is_inside_zone(player, ZoneId::Peace) {
            self.send(
                player,
                &ExShowScreenMessage::text("You may not log out from this location.", 5000),
            );
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouMayNotLogOutFromThisLocation),
            );
            self.send(player, &ActionFailed);
            return Some(false);
        }
        if self.is_registered_on_event(player) {
            return refuse(self, "Cannot use this command while registered on an event.");
        }
        self.offline_play.asked.insert(player);
        self.send(
            player,
            &ConfirmDlg::new(SystemMessageId::S13).with(SystemMessageParam::Text(
                "Do you wish to exit and continue auto play?".to_owned(),
            )),
        );
        Some(true)
    }

    /// `DlgAnswer`, the `OFFLINE_PLAY` branch of the `S1_3` answers. `true` when the player was
    /// asked (the answer is used).
    pub(crate) fn offline_play_dlg_answer(&mut self, player: ObjectId, answer: i32) -> bool {
        if !self.offline_play.asked.remove(&player) {
            return false;
        }
        if answer == 0 || !self.offline_play.settings.enable_offline_play_command {
            return true;
        }
        if self.offline_play.settings.offline_play_premium && !self.has_premium_status(player) {
            self.send_message(player, "This command is only available to premium players.");
            return true;
        }
        if !self.is_auto_playing(player) {
            self.send_message(player, "You need to enable auto play before exiting.");
            return true;
        }
        if self.is_in_vehicle(player) || self.is_inside_zone(player, ZoneId::Peace) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouMayNotLogOutFromThisLocation),
            );
            return true;
        }
        if self.is_registered_on_event(player) {
            self.send_message(player, "Cannot use this command while registered on an event.");
            return true;
        }
        // Unregister from olympiad.
        self.olympiad_unregister_on_leave(player);
        self.start_offline_play(player);
        true
    }

    /// Java `Player.startOfflinePlay()`.
    pub fn start_offline_play(&mut self, id: ObjectId) {
        if !self.players.contains_key(&id) {
            return;
        }
        // `sendPacket(LeaveWorld.STATIC_PACKET)`, `_client.setDetached(true)`: the connection goes,
        // the player stays.
        let account = self.players.get(&id).map(|p| p.data.account.clone());
        if let Some(sink) = self.sinks.remove(&id) {
            sink.send(l2_net::Encode::to_bytes(&LeaveWorld));
            sink.left_world(LeaveReason::Logout);
        }
        if let Some(session) = self.player_sessions.remove(&id) {
            self.sessions.remove(&session);
        }
        self.offline_play.players.insert(id);
        let count = self.offline_play.settings.offline_play_abnormal_effects.len();
        if count > 0 {
            let at = usize::try_from(self.rng().below(count as i32)).unwrap_or(0);
            let effect = self.offline_play.settings.offline_play_abnormal_effects[at];
            self.start_abnormal_visual_effect(id, &[effect]);
        }
        self.broadcast_user_info(id);
        // `GameClient.onDisconnection` -> `sendLogout(account)`: only with the database switch
        // (D-52); see the module comment.
        if self.offline.settings.database_writes
            && let Some(account) = account
        {
            self.outbox.ls_event(LsEvent::PlayerLogout { account });
        }
        tracing::info!("Entered offline play, {id}.");
    }

    /// The offline players of an account, for `release_offline_account`.
    pub(crate) fn offline_play_of_account(&self, account: &str) -> Vec<ObjectId> {
        self.offline_play
            .players
            .iter()
            .copied()
            .filter(|id| self.players.get(id).is_some_and(|p| p.data.account == account))
            .collect()
    }
}
