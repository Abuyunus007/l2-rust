//! Offline play (`config/Custom/OfflinePlay.ini`, `EnableOfflinePlayCommand`, off by default): a
//! player who is auto playing types `.offlineplay`, the client goes and the character keeps
//! hunting.
// # The character stays without a client
// Like an offline trader (`trade/offline.rs`): the player is in `World::players` without an entry
// in `World::sinks`; the login server hears `PlayerLogout` at once.
//
// # The rows (M9-L01)
// `character_offline_play` and `character_offline_play_group` are written, removed and restored
// with `RestoreAutoPlayOffliners`.
// The save is `storeOfflinePlay` (on `startOfflinePlay`), the removal `removeOfflinePlay` (auto
// play stopped, `CharacterSelect`), the groups are written by `storeOfflineGroups` at shutdown and
// the whole group table is emptied after the restore.
//
// `OfflinePlayDisconnectSameAccount` and `OfflineDisconnectSameAccount` are the hook of the
// character list (`release_offline_same_account`, called by the game server when it builds the
// list). The `AntiFeedManager` `OFFLINE_PLAY` count is in `start_offline_play` and, for the
// `CharacterSelect` branch, in `offline_play_select_refusal`.

use std::collections::{BTreeMap, BTreeSet};
use std::net::IpAddr;

use l2_data::skill::AbnormalVisualEffect;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    ActionFailed, ChatType, ConfirmDlg, CreatureSay, ExShowScreenMessage, LeaveWorld, PartyDistributionType,
    SayBody, SenderField, SystemMessage, SystemMessageParam,
};
use l2_util::ObjectId;
use tracing::{info, warn};

use super::dualbox::{GAME_ID, OFFLINE_PLAY};
use super::java_hash_set_order;
use crate::command::LeaveReason;
use crate::outbox::{LsEvent, PersistCommand};
use crate::player::PlayerData;
use crate::world::World;
use crate::zones::ZoneId;

/// `OfflinePlayTable.TYPE_*`: the `type` column of `character_offline_play`.
pub mod row_type {
    pub const ACTIVE_SOULSHOT: i32 = 0;
    pub const AUTO_SUPPLY_ITEM: i32 = 1;
    pub const AUTO_BUFF: i32 = 2;
    pub const AUTO_SKILL: i32 = 3;
    pub const AUTO_ACTION: i32 = 4;
    pub const AUTO_POTION_ITEM: i32 = 5;
    /// `config/Custom/AutoPotions.ini`.
    pub const CUSTOM_AUTO_POTION_SYSTEM: i32 = 100;
}

/// A `Set.add`.
fn push_unique(list: &mut Vec<i32>, value: i32) {
    if !list.contains(&value) {
        list.push(value);
    }
}

/// One line of `character_offline_play`: `(type, id)`.
pub type OfflinePlayLine = (i32, i32);

/// A write to the offline play tables with Java's statements (`OfflinePlayTable`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OfflinePlayWrite {
    /// `storeOfflinePlay`: the rows of the character are deleted, then these are written.
    Store {
        char_id: ObjectId,
        rows: Vec<OfflinePlayLine>,
    },
    /// `removeOfflinePlay`.
    Remove { char_id: ObjectId },
    /// `storeOfflineGroups`: `(leader, member, distribution type id)`.
    StoreGroups(Vec<(ObjectId, ObjectId, i32)>),
    /// The end of `restoreOfflinePlayers`: `TRUNCATE TABLE character_offline_play_group`.
    ClearGroups,
}

/// A character `restoreOfflinePlayers` brings back: loaded like at `CharacterSelect`, with its rows.
#[derive(Debug)]
pub struct OfflinePlayRestore {
    pub data: PlayerData,
    pub rows: Vec<OfflinePlayLine>,
}

/// One leader of `character_offline_play_group` with its members `(charId, distribution id)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfflinePlayGroup {
    pub leader: ObjectId,
    pub members: Vec<(ObjectId, i32)>,
}

/// Java `OfflinePlayConfig`. `Default` is the Java default of each key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfflinePlaySettings {
    /// `EnableOfflinePlayCommand`.
    pub enable_offline_play_command: bool,
    /// `RestoreAutoPlayOffliners`.
    pub restore_auto_play_offliners: bool,
    /// `OfflinePlayPremium`.
    pub offline_play_premium: bool,
    /// `OfflinePlayLogoutOnDeath`.
    pub offline_play_logout_on_death: bool,
    /// `OfflinePlayDisconnectSameAccount`.
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

impl OfflinePlaySettings {
    /// `GameServer`: "ENABLE_OFFLINE_PLAY_COMMAND && RESTORE_AUTO_PLAY_OFFLINERS" ->
    /// `restoreOfflinePlayers()`.
    pub fn restore_wanted(&self) -> bool {
        self.enable_offline_play_command && self.restore_auto_play_offliners
    }
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
    /// Restored players get the play name colour with `OfflineSetNameColor` of the trade settings
    /// (`restoreOfflinePlayers`).
    restored_color: BTreeSet<ObjectId>,
    /// What `CharacterSelect` needs from an offline character it just released (it is gone from
    /// the world by then): `(premium, client address)`.
    released: BTreeMap<ObjectId, (bool, Option<IpAddr>)>,
}

impl World {
    /// Installs `OfflinePlay.ini`. Call before players enter.
    pub fn set_offline_play_settings(&mut self, settings: OfflinePlaySettings) {
        // `GameServer`: `registerEvent(OFFLINE_PLAY)` with the command.
        if settings.enable_offline_play_command {
            self.anti_feed_register_event(OFFLINE_PLAY);
        }
        self.offline_play.settings = settings;
    }

    /// Whether the rows of the two tables are written: `RestoreAutoPlayOffliners`.
    fn offline_play_writes(&self) -> bool {
        self.offline_play.settings.restore_auto_play_offliners
    }

    /// Java `Player.isOfflinePlay()`.
    pub fn is_offline_play(&self, id: ObjectId) -> bool {
        self.offline_play.players.contains(&id)
    }

    /// The name colour `OfflinePlaySetNameColor` gives a player who plays offline.
    pub(crate) fn offline_play_name_color(&self, id: ObjectId) -> Option<i32> {
        (self.is_offline_play(id)
            && (self.offline_play.settings.offline_play_set_name_color
                || self.offline_play.restored_color.contains(&id)))
        .then_some(self.offline_play.settings.offline_play_name_color)
    }

    /// `DecayTaskManager.add`: whether the dead player is logged out after ten seconds.
    pub(crate) fn offline_play_logout_on_death(&self, id: ObjectId) -> bool {
        self.is_offline_play(id) && self.offline_play.settings.offline_play_logout_on_death
    }

    /// `Player.setAutoPlaying(false)` of an offline player removes its rows
    /// (`RESTORE_AUTO_PLAY_OFFLINERS`).
    pub(crate) fn offline_play_auto_play_stopped(&mut self, id: ObjectId) {
        if self.is_offline_play(id) && self.offline_play_writes() {
            self.offline_play_remove_rows(id);
        }
    }

    /// `OfflinePlayTable.removeOfflinePlay(player)`.
    fn offline_play_remove_rows(&mut self, id: ObjectId) {
        self.outbox
            .persist(PersistCommand::OfflinePlay(OfflinePlayWrite::Remove {
                char_id: id,
            }));
    }

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
        self.offline_play.restored_color.remove(&id);
    }

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
        // The dual-box limits of the offline play (`DualboxCheckMaxOfflinePlay(Premium)PerIP`).
        let dualbox = self.settings.custom_misc.dualbox.clone();
        let ip = self.players.get(&id).and_then(|p| p.client_ip);
        let refused = (self.has_premium_status(id)
            && dualbox.max_offlineplay_premium_per_ip > 0
            && !self.anti_feed_try_add_player(OFFLINE_PLAY, id, dualbox.max_offlineplay_premium_per_ip))
            || (dualbox.max_offlineplay_per_ip > 0
                && !self.anti_feed_try_add_player(OFFLINE_PLAY, id, dualbox.max_offlineplay_per_ip));
        if refused {
            let limit = self.anti_feed_limit(ip, dualbox.max_offlineplay_per_ip);
            self.send_message(id, &format!("Only {limit} offline players allowed per IP."));
            return;
        }
        self.anti_feed_remove_player(GAME_ID, id);
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
        // `GameClient.onDisconnection` -> `sendLogout(account)`.
        if let Some(account) = account {
            self.outbox.ls_event(LsEvent::PlayerLogout { account });
        }
        // `if (RESTORE_AUTO_PLAY_OFFLINERS) storeOfflinePlay(this)`.
        if self.offline_play_writes() {
            let rows = self.offline_play_rows(id);
            self.outbox
                .persist(PersistCommand::OfflinePlay(OfflinePlayWrite::Store {
                    char_id: id,
                    rows,
                }));
        }
        tracing::info!("Entered offline play, {id}.");
    }

    /// The lines `OfflinePlayTable.storeOfflinePlay` writes for a character.
    pub(crate) fn offline_play_rows(&self, id: ObjectId) -> Vec<OfflinePlayLine> {
        let mut rows = Vec::new();
        for shot in self.auto_soul_shots(id) {
            rows.push((row_type::ACTIVE_SOULSHOT, shot));
        }
        if let Some(state) = self.auto_play.players.get(&id) {
            let sets = [
                (row_type::AUTO_SUPPLY_ITEM, &state.supply_items),
                (row_type::AUTO_BUFF, &state.buffs),
            ];
            for (kind, ids) in sets {
                rows.extend(java_hash_set_order(ids).into_iter().map(|v| (kind, v)));
            }
            rows.extend(state.skills.iter().map(|v| (row_type::AUTO_SKILL, *v)));
            rows.extend(
                java_hash_set_order(&state.actions)
                    .into_iter()
                    .map(|v| (row_type::AUTO_ACTION, v)),
            );
            if state.potion_item > 0 {
                rows.push((row_type::AUTO_POTION_ITEM, state.potion_item));
            }
        }
        if self.auto_potion_enabled() && self.has_auto_potion(id) {
            rows.push((row_type::CUSTOM_AUTO_POTION_SYSTEM, 0));
        }
        rows
    }

    /// `Shutdown`: "RESTORE_AUTO_PLAY_OFFLINERS && ENABLE_AUTO_ASSIST" ->
    /// `OfflinePlayTable.storeOfflineGroups()`: every auto playing member whose leader auto plays
    /// too. Called before the players are disconnected.
    pub(crate) fn offline_play_store_groups(&mut self) {
        if !self.offline_play_writes() || !self.auto_play.rules.enable_auto_assist {
            return;
        }
        let mut rows = Vec::new();
        let mut ids: Vec<ObjectId> = self.players.keys().copied().collect();
        ids.sort();
        for id in ids {
            if !self.is_auto_playing(id) {
                continue;
            }
            let Some(party) = self.party_of(id) else {
                continue;
            };
            if self.is_party_leader(id) {
                continue;
            }
            let Some(leader) = self.party_leader(party) else {
                continue;
            };
            if self.is_auto_playing(leader) {
                let kind = self
                    .party_distribution_type(party)
                    .map_or(0, PartyDistributionType::id);
                rows.push((leader, id, kind));
            }
        }
        info!("Offline Play Table: Offline play groups stored ({}).", rows.len());
        self.outbox
            .persist(PersistCommand::OfflinePlay(OfflinePlayWrite::StoreGroups(rows)));
    }

    /// Java `OfflinePlayTable.restoreOfflinePlayers`, once the characters are loaded: each one
    /// enters without a client with its auto play settings, plays again, and the groups are put
    /// together.
    pub fn restore_offline_players(
        &mut self,
        players: Vec<OfflinePlayRestore>,
        groups: Vec<OfflinePlayGroup>,
    ) {
        let mut restored = 0;
        for OfflinePlayRestore { data, rows } in players {
            let id = data.object_id;
            if self.players.contains_key(&id) {
                warn!("OfflinePlayTable: {id} is already in the world; not restored.");
                continue;
            }
            self.enter_without_client(data);
            for (kind, value) in rows {
                match kind {
                    row_type::ACTIVE_SOULSHOT => self.add_auto_soul_shot(id, value),
                    row_type::AUTO_SUPPLY_ITEM => {
                        push_unique(&mut self.auto_player_mut(id).supply_items, value)
                    }
                    row_type::AUTO_BUFF => push_unique(&mut self.auto_player_mut(id).buffs, value),
                    row_type::AUTO_SKILL => self.auto_player_mut(id).skills.push(value),
                    row_type::AUTO_ACTION => push_unique(&mut self.auto_player_mut(id).actions, value),
                    row_type::AUTO_POTION_ITEM => self.auto_player_mut(id).potion_item = value,
                    row_type::CUSTOM_AUTO_POTION_SYSTEM if self.auto_potion_enabled() => {
                        self.add_auto_potion(id);
                    }
                    _ => {}
                }
            }
            self.offline_play.players.insert(id);
            self.restore_saved_effects(id);
            self.set_running_effect(id, true);
            if self.offline.settings.offline_set_name_color {
                self.offline_play.restored_color.insert(id);
            }
            let count = self.offline_play.settings.offline_play_abnormal_effects.len();
            if count > 0 {
                let at = usize::try_from(self.rng().below(count as i32)).unwrap_or(0);
                let effect = self.offline_play.settings.offline_play_abnormal_effects[at];
                self.start_abnormal_visual_effect(id, &[effect]);
            }
            self.start_auto_play(id);
            self.start_auto_use_task(id);
            restored += 1;
        }
        info!("OfflinePlayTable: Loaded {restored} offline auto players.");
        // "Restore parties."
        if self.auto_play.rules.enable_auto_assist {
            let mut parties = 0;
            for group in groups {
                if !self.players.contains_key(&group.leader) {
                    continue;
                }
                parties += 1;
                let mut party = None;
                for (member, kind) in group.members {
                    if !self.players.contains_key(&member) {
                        continue;
                    }
                    let party = *party.get_or_insert_with(|| {
                        let kind = PartyDistributionType::from_id(kind)
                            .unwrap_or(PartyDistributionType::FindersKeepers);
                        self.create_party(group.leader, kind)
                    });
                    self.join_party(member, party);
                }
            }
            info!("OfflinePlayTable: Loaded {parties} offline groups.");
        }
        // "Clear all party data in one operation."
        if self.offline_play_writes() {
            self.outbox
                .persist(PersistCommand::OfflinePlay(OfflinePlayWrite::ClearGroups));
        }
    }

    /// `CharSelectionInfo.loadCharacterSelectInfo`: "Disconnect offline trader" (a character in
    /// store mode, `OfflineDisconnectSameAccount`) and "Disconnect offline play"
    /// (`OfflinePlayDisconnectSameAccount`). The account is in use by the new session, so the
    /// login server is not told. `true` when the character was released.
    pub(crate) fn release_offline_same_account(&mut self, id: ObjectId, trader: bool, play: bool) -> bool {
        let offline = !self.sinks.contains_key(&id);
        let release = (trader && offline && self.players.contains_key(&id) && self.is_in_store_mode(id))
            || (play && self.is_offline_play(id));
        let released = release && self.release_offline_trader_quietly(id);
        // Only `CharacterSelect` reads what the release kept.
        self.offline_play.released.remove(&id);
        released
    }

    /// `CharacterSelect`: the `OFFLINE_PLAY` limit for a premium character that was just released
    /// (`World.getPlayer` found it). `Some(page)` is the page to show instead of selecting; `ip` is
    /// the address of the client that selects. Consumes what `release_offline_trader_quietly` kept.
    pub fn offline_play_select_refusal(
        &mut self,
        id: ObjectId,
        ip: Option<IpAddr>,
        access_level: i32,
    ) -> Option<String> {
        let (premium, player_ip) = self.offline_play.released.remove(&id)?;
        let max = self.settings.custom_misc.dualbox.max_offlineplay_premium_per_ip;
        if access_level >= 100 || !premium || max <= 0 {
            return None;
        }
        if self.anti_feed_try_add_client(OFFLINE_PLAY, player_ip, max) {
            return None;
        }
        Some(self.ip_restriction_page(self.anti_feed_limit(ip, max)))
    }

    /// Keeps what `offline_play_select_refusal` needs from a character about to be released.
    pub(crate) fn offline_play_remember_release(&mut self, id: ObjectId) {
        let premium = self.has_premium_status(id);
        let ip = self.players.get(&id).and_then(|p| p.client_ip);
        self.offline_play.released.insert(id, (premium, ip));
    }

    /// `CharacterSelect`: "if (RESTORE_AUTO_PLAY_OFFLINERS && player.isAutoPlaying())
    /// removeOfflinePlay(player)".
    pub(crate) fn offline_play_release_rows(&mut self, id: ObjectId) {
        if self.offline_play.settings.restore_auto_play_offliners && self.is_auto_playing(id) {
            self.offline_play_remove_rows(id);
        }
    }
}
