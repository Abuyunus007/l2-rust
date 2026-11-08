//! The settings of L2Rust's own features (`config/L2R.ini`, not in the Java server). The game server
//! reads the file (`l2_config::L2rConfig`) and maps it here; `//reload l2r` replaces them while the
//! server runs (`ControlRequest::ReloadL2r`, answered by `WorldCommand::L2rReloaded`).
//!
//! l2-world does not depend on l2-config: these types are the world's own copy of the values.

use l2_util::ObjectId;

use crate::world::World;

/// `[Companions]` `CompanionCombatMode`: how the companions fight (D-61).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CompanionCombatMode {
    /// The companions attack what the owner targets.
    Aggressive,
    /// The companions attack only after the owner attacked the monster himself.
    #[default]
    Passive,
}

/// The `[Companions]` section of `L2R.ini`. `Default` is the default of every key.
#[derive(Debug, Clone, PartialEq)]
pub struct CompanionSettings {
    /// `CompanionPAtkMultiplier`: factor on the P. Atk. of companions.
    pub p_atk_multiplier: f64,
    /// `CompanionMAtkMultiplier`: factor on the M. Atk. of companions.
    pub m_atk_multiplier: f64,
    /// `CompanionHpMultiplier`: factor on the max HP of companions.
    pub hp_multiplier: f64,
    /// `CompanionAutoJoinParty`: the companions show in the party window of their owner.
    pub auto_join_party: bool,
    /// `CompanionCombatMode`.
    pub combat_mode: CompanionCombatMode,
    /// `CompanionFeePerLevel`: Adena a companion costs for each level of its owner.
    pub fee_per_level: i64,
    /// `CompanionMaxCount`: the most companions a player may have at once.
    pub max_count: usize,
}

impl Default for CompanionSettings {
    fn default() -> Self {
        Self {
            p_atk_multiplier: 1.0,
            m_atk_multiplier: 1.0,
            hp_multiplier: 1.0,
            auto_join_party: true,
            combat_mode: CompanionCombatMode::Passive,
            fee_per_level: 5_000,
            max_count: 3,
        }
    }
}

/// The `[RiftInvasion]` section of `L2R.ini` (D-64). `Default` is the default of every key.
#[derive(Debug, Clone, PartialEq)]
pub struct RiftInvasionSettings {
    /// `RiftEnabled`.
    pub enabled: bool,
    /// `RiftIntervalMinMinutes` in milliseconds.
    pub interval_min_ms: i64,
    /// `RiftIntervalMaxMinutes` in milliseconds (never below the minimum).
    pub interval_max_ms: i64,
    /// `RiftAnnounceLeadMinutes` in milliseconds.
    pub announce_lead_ms: i64,
    /// `RiftAnnounceReminder`: a reminder one minute before the start.
    pub announce_reminder: bool,
    /// `RiftWaveCount`.
    pub wave_count: usize,
    /// `RiftWaveIntervalSeconds` in milliseconds.
    pub wave_interval_ms: i64,
    /// `RiftDurationMinutes` in milliseconds.
    pub duration_ms: i64,
    /// `RiftMobLevelOffset`.
    pub mob_level_offset: i32,
    /// `RiftScaleWithPlayers`.
    pub scale_with_players: bool,
    /// `RiftScalePerPlayerPercent`.
    pub scale_per_player_percent: i32,
    /// `RiftScaleMaxPercent`.
    pub scale_max_percent: i32,
    /// `RiftParticipationRadius`.
    pub participation_radius: i32,
    /// `RiftMinContributionPercent`.
    pub min_contribution_percent: f64,
    /// `RiftShardItemId` (0: not set).
    pub shard_item_id: i32,
    /// `RiftShardsPerWave`.
    pub shards_per_wave: i64,
    /// `RiftShardsBoss`.
    pub shards_boss: i64,
    /// `RiftTopContributorBonus`.
    pub top_contributor_bonus: i64,
    /// `RiftShardBound`.
    pub shard_bound: bool,
    /// `RiftWeeklyCap` (0: no cap).
    pub weekly_cap: i64,
    /// `RiftWeeklyResetDay`, 1 (Monday) to 7 (Sunday).
    pub weekly_reset_day: i32,
    /// `RiftWeeklyResetHour`.
    pub weekly_reset_hour: i32,
    /// `RiftMobsNormalDrops`.
    pub mobs_normal_drops: bool,
    /// `RiftMobsExpRate`.
    pub mobs_exp_rate: f64,
    /// `RiftMobsSpRate`.
    pub mobs_sp_rate: f64,
    /// `RiftMerchantNpcId` (0: not set).
    pub merchant_npc_id: i32,
    /// `RiftMerchantLoc`.
    pub merchant_loc: (i32, i32, i32),
}

impl Default for RiftInvasionSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_min_ms: 120 * 60_000,
            interval_max_ms: 240 * 60_000,
            announce_lead_ms: 5 * 60_000,
            announce_reminder: true,
            wave_count: 3,
            wave_interval_ms: 90_000,
            duration_ms: 20 * 60_000,
            mob_level_offset: 0,
            scale_with_players: true,
            scale_per_player_percent: 8,
            scale_max_percent: 200,
            participation_radius: 3_000,
            min_contribution_percent: 1.0,
            shard_item_id: 0,
            shards_per_wave: 5,
            shards_boss: 15,
            top_contributor_bonus: 5,
            shard_bound: true,
            weekly_cap: 400,
            weekly_reset_day: 3,
            weekly_reset_hour: 6,
            mobs_normal_drops: false,
            mobs_exp_rate: 1.0,
            mobs_sp_rate: 1.0,
            merchant_npc_id: 0,
            merchant_loc: (83_400, 148_000, -3_400),
        }
    }
}

impl RiftInvasionSettings {
    /// Why the rift invasions cannot run with these settings (an id that is not set), `None`
    /// when they can. `RiftEnabled = false` is not a problem: the feature is simply off.
    pub fn missing_setting(&self) -> Option<&'static str> {
        if self.shard_item_id <= 0 {
            Some("RiftShardItemId is 0 (not set)")
        } else if self.merchant_npc_id <= 0 {
            Some("RiftMerchantNpcId is 0 (not set)")
        } else {
            None
        }
    }

    /// The rift shard while it is bound (`RiftShardBound`): not tradeable, droppable, sold in a
    /// private store, mailed, sent by freight or put into the clan warehouse.
    pub fn bound_item(&self) -> Option<i32> {
        (self.shard_bound && self.shard_item_id > 0).then_some(self.shard_item_id)
    }
}

/// Every section of `L2R.ini`. One field per feature.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct L2rSettings {
    /// `[Companions]`.
    pub companions: CompanionSettings,
    /// `[RiftInvasion]`.
    pub rift: RiftInvasionSettings,
}

impl World {
    /// The settings of `L2R.ini`.
    pub fn l2r_settings(&self) -> &L2rSettings {
        &self.custom_scripts.l2r
    }

    /// Installs the settings of `L2R.ini` (at the start and for `//reload l2r`).
    pub fn set_l2r_settings(&mut self, settings: L2rSettings) {
        self.custom_scripts.l2r = settings;
    }

    /// The answer to `//reload l2r`: the settings are replaced and the GM is told so.
    pub(crate) fn on_l2r_reloaded(&mut self, requester: ObjectId, settings: L2rSettings) {
        self.set_l2r_settings(settings);
        self.send_sys_message(requester, "L2R.ini reloaded.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_shipped_file() {
        let settings = L2rSettings::default();
        assert_eq!(settings.companions, CompanionSettings::default());
        assert_eq!(settings.companions.p_atk_multiplier, 1.0);
        assert_eq!(settings.companions.m_atk_multiplier, 1.0);
        assert_eq!(settings.companions.hp_multiplier, 1.0);
        assert!(settings.companions.auto_join_party);
        assert_eq!(settings.companions.combat_mode, CompanionCombatMode::Passive);
    }

    #[test]
    fn a_rift_needs_its_ids_and_binds_its_shard() {
        let mut rift = RiftInvasionSettings::default();
        assert!(rift.missing_setting().is_some());
        assert_eq!(rift.bound_item(), None);
        rift.shard_item_id = 4405;
        assert!(rift.missing_setting().unwrap().contains("RiftMerchantNpcId"));
        assert_eq!(rift.bound_item(), Some(4405));
        rift.merchant_npc_id = 50_030;
        assert_eq!(rift.missing_setting(), None);
        rift.shard_bound = false;
        assert_eq!(rift.bound_item(), None);
    }
}
