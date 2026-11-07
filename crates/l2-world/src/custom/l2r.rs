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

/// Every section of `L2R.ini`. One field per feature.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct L2rSettings {
    /// `[Companions]`.
    pub companions: CompanionSettings,
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
}
