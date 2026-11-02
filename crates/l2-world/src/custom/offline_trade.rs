//! The custom parts of `config/Custom/OfflineTrade.ini` that `trade/offline.rs` (M6-FU-14) left
//! out: the voiced command `.offline` (`EnableOfflineCommand`) and `OfflineAbnormalEffect`.
// Java: dist/game/data/scripts/handlers/chat/commands/voiced/Offline.java
// Java: network/clientpackets/DlgAnswer.java (the `OFFLINE_TRADE` branch)
// Java: data/sql/OfflineTraderTable.java (enteredOfflineMode: the abnormal effect)
// Java: config/custom/OfflineTradeConfig.java
//
// # Left out
// - `OfflineDisconnectSameAccount` (`CharSelectionInfo`): the character list of the account is
//   built outside the world. In the side by side mode (D-52) the account stays on this server
//   while a trader sells and a login of the account kicks the trader, which is the same result.
// - The restore of a trader (`restoreOfflineTraders`) does not give the abnormal effect back: the
//   effect is a visual only and the restore does not run side by side.

use l2_data::skill::AbnormalVisualEffect;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ActionFailed, ConfirmDlg, SystemMessage, SystemMessageParam};
use l2_util::ObjectId;

use crate::command::LeaveReason;
use crate::world::World;

/// The keys of `OfflineTrade.ini` this module reads that `OfflineSettings` does not hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfflineCommandSettings {
    /// `EnableOfflineCommand`.
    pub enable_offline_command: bool,
    /// `OfflineAbnormalEffect`.
    pub offline_abnormal_effects: Vec<AbnormalVisualEffect>,
}

impl Default for OfflineCommandSettings {
    fn default() -> Self {
        Self {
            enable_offline_command: true,
            offline_abnormal_effects: Vec::new(),
        }
    }
}

/// What the world keeps for the command.
#[derive(Debug, Default)]
pub(crate) struct OfflineCommandWorld {
    pub(crate) settings: OfflineCommandSettings,
    /// The players who answer the `.offline` question (`PlayerAction.OFFLINE_TRADE`).
    asked: std::collections::BTreeSet<ObjectId>,
}

impl World {
    /// Installs the keys of `OfflineTrade.ini` this module reads. Call before players enter.
    pub fn set_offline_command_settings(&mut self, settings: OfflineCommandSettings) {
        self.offline_command.settings = settings;
    }

    /// Java `Player.canLogout()` as far as the world models it.
    fn can_logout(&mut self, id: ObjectId) -> bool {
        !(self.is_changing_class(id)
            || self.enchant_blocks_logout(id)
            || self.minigame_blocks_logout(id)
            || self.festival_blocks_logout(id))
    }

    /// `Offline.onCommand` through `VoicedCommandHandler.getHandler(word)`: registered only with
    /// `EnableOfflineCommand` and a trade or craft enable key.
    pub(crate) fn offline_trade_voiced_handler(&mut self, player: ObjectId, word: &str) -> Option<bool> {
        let s = self.offline_settings();
        let registered = self.offline_command.settings.enable_offline_command
            && (s.offline_trade_enable || s.offline_craft_enable);
        if !registered || !matches!(word, "offline" | "offlinetrade" | "offline_trade") {
            return None;
        }
        if word != "offline" {
            return Some(true);
        }
        if !self.is_in_store_mode(player) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::PrivateStoreAlreadyClosed),
            );
            self.send(player, &ActionFailed);
            return Some(false);
        }
        if self.is_in_instance(player) || self.is_in_vehicle(player) || !self.can_logout(player) {
            self.send(player, &ActionFailed);
            return Some(false);
        }
        self.offline_command.asked.insert(player);
        self.send(
            player,
            &ConfirmDlg::new(SystemMessageId::S13).with(SystemMessageParam::Text(
                "Do you wish to exit and continue trading?".to_owned(),
            )),
        );
        Some(true)
    }

    /// `DlgAnswer`, the `OFFLINE_TRADE` branch of the `S1_3` answers. `true` when the player was
    /// asked.
    pub(crate) fn offline_trade_dlg_answer(&mut self, player: ObjectId, answer: i32) -> bool {
        if !self.offline_command.asked.remove(&player) {
            return false;
        }
        let s = self.offline_settings();
        if answer == 0
            || !self.offline_command.settings.enable_offline_command
            || (!s.offline_trade_enable && !s.offline_craft_enable)
        {
            return true;
        }
        if !self.is_in_store_mode(player) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::PrivateStoreAlreadyClosed),
            );
            self.send(player, &ActionFailed);
            return true;
        }
        if self.is_in_instance(player) || self.is_in_vehicle(player) || !self.can_logout(player) {
            self.send(player, &ActionFailed);
            return true;
        }
        // Remove player from boss zone.
        self.remove_from_boss_zones(player);
        // Unregister from olympiad.
        self.olympiad_unregister_on_leave(player);
        if !self.entered_offline_mode(player) {
            self.leave(player, LeaveReason::Logout);
        }
        true
    }

    /// `OfflineTraderTable.enteredOfflineMode`, after `storeMe()`: the abnormal visual effect.
    pub(crate) fn offline_trade_abnormal_effect(&mut self, id: ObjectId) {
        let count = self.offline_command.settings.offline_abnormal_effects.len();
        if count == 0 {
            return;
        }
        let at = usize::try_from(self.rng().below(count as i32)).unwrap_or(0);
        let effect = self.offline_command.settings.offline_abnormal_effects[at];
        self.start_abnormal_visual_effect(id, &[effect]);
    }

    /// `DlgAnswer` of the `S1_3` question: the `.offline` and `.offlineplay` dialogs of the custom
    /// features (they come before the wedding and the admin question).
    pub(crate) fn custom_play_dlg_answer(&mut self, player: ObjectId, answer: i32) -> bool {
        self.offline_trade_dlg_answer(player, answer) || self.offline_play_dlg_answer(player, answer)
    }
}
