//! Settings of the features of L2Rust that the Java server does not have (`config/L2R.ini`).
//! Every feature gets its own `[Section]`; the keys are read like the other `.ini` files, and a
//! value out of its range gives the default with a warning (never a panic, never a clamp).

use std::str::FromStr;

use tracing::warn;

use crate::IniFile;

/// `[Companions]`: how the companions (D-61) of a player fight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompanionCombatMode {
    /// The companions attack what the owner targets.
    Aggressive,
    /// The companions attack only after the owner attacked the monster himself.
    Passive,
}

impl FromStr for CompanionCombatMode {
    type Err = ();

    /// Case-insensitive: `Passive`, `passive` and `PASSIVE` are the same mode.
    fn from_str(s: &str) -> Result<Self, ()> {
        if s.eq_ignore_ascii_case("AGGRESSIVE") {
            Ok(Self::Aggressive)
        } else if s.eq_ignore_ascii_case("PASSIVE") {
            Ok(Self::Passive)
        } else {
            Err(())
        }
    }
}

/// Settings of `config/L2R.ini`.
#[derive(Debug, Clone, PartialEq)]
pub struct L2rConfig {
    /// `[Companions]` key `CompanionPAtkMultiplier`, range 0.0 to 10.0, default 1.0.
    pub companion_p_atk_multiplier: f64,
    /// `[Companions]` key `CompanionMAtkMultiplier`, range 0.0 to 10.0, default 1.0.
    pub companion_m_atk_multiplier: f64,
    /// `[Companions]` key `CompanionHpMultiplier`, range 0.1 to 10.0, default 1.0.
    pub companion_hp_multiplier: f64,
    /// `[Companions]` key `CompanionAutoJoinParty`, default true.
    pub companion_auto_join_party: bool,
    /// `[Companions]` key `CompanionCombatMode`, `AGGRESSIVE` or `PASSIVE`, default `PASSIVE`.
    pub companion_combat_mode: CompanionCombatMode,
    /// `[Companions]` key `CompanionFeePerLevel`: Adena a companion costs for each level of its
    /// owner, range 0 to 1,000,000, default 5,000.
    pub companion_fee_per_level: i64,
    /// `[Companions]` key `CompanionMaxCount`: the most companions a player may have at once,
    /// range 1 to 8, default 3.
    pub companion_max_count: i32,
}

impl L2rConfig {
    /// File name inside the config directory.
    pub const FILE: &'static str = "L2R.ini";

    /// Reads the values of every section. Missing or invalid values give the defaults.
    pub fn from_ini(ini: &IniFile) -> Self {
        Self {
            companion_p_atk_multiplier: ranged(ini, "CompanionPAtkMultiplier", 1.0, 0.0, 10.0),
            companion_m_atk_multiplier: ranged(ini, "CompanionMAtkMultiplier", 1.0, 0.0, 10.0),
            companion_hp_multiplier: ranged(ini, "CompanionHpMultiplier", 1.0, 0.1, 10.0),
            companion_auto_join_party: ini.get_bool("CompanionAutoJoinParty", true),
            companion_combat_mode: ini.get_enum("CompanionCombatMode", CompanionCombatMode::Passive),
            companion_fee_per_level: ranged_long(ini, "CompanionFeePerLevel", 5_000, 0, 1_000_000),
            companion_max_count: ranged_long(ini, "CompanionMaxCount", 3, 1, 8) as i32,
        }
    }
}

impl Default for L2rConfig {
    /// Every key missing: the defaults of the file.
    fn default() -> Self {
        Self::from_ini(&IniFile::empty(Self::FILE))
    }
}

/// A double that must lie in `min..=max`; anything else (NaN included) gives `default` and a
/// warning that names the key, the value, the range and the default.
fn ranged(ini: &IniFile, key: &str, default: f64, min: f64, max: f64) -> f64 {
    let value = ini.get_double(key, default);
    if (min..=max).contains(&value) {
        return value;
    }
    warn!(
        "Config '{key}' in file '{}' is {value}, outside the range {min} to {max}, using default: {default}.",
        L2rConfig::FILE
    );
    default
}

/// [`ranged`] for a whole number.
fn ranged_long(ini: &IniFile, key: &str, default: i64, min: i64, max: i64) -> i64 {
    let value = ini.get_long(key, default);
    if (min..=max).contains(&value) {
        return value;
    }
    warn!(
        "Config '{key}' in file '{}' is {value}, outside the range {min} to {max}, using default: {default}.",
        L2rConfig::FILE
    );
    default
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
# Companions
[Companions]
CompanionPAtkMultiplier = 2.5
CompanionMAtkMultiplier = 0.5
CompanionHpMultiplier = 3
CompanionAutoJoinParty = false
CompanionCombatMode = AGGRESSIVE
CompanionFeePerLevel = 2000
CompanionMaxCount = 5
";

    #[test]
    fn defaults_come_from_an_empty_file() {
        let config = L2rConfig::default();
        assert_eq!(
            config,
            L2rConfig {
                companion_p_atk_multiplier: 1.0,
                companion_m_atk_multiplier: 1.0,
                companion_hp_multiplier: 1.0,
                companion_auto_join_party: true,
                companion_combat_mode: CompanionCombatMode::Passive,
                companion_fee_per_level: 5_000,
                companion_max_count: 3,
            }
        );
    }

    #[test]
    fn reads_all_five_values_from_a_sample() {
        let config = L2rConfig::from_ini(&IniFile::parse(L2rConfig::FILE, SAMPLE).unwrap());
        assert_eq!(config.companion_p_atk_multiplier, 2.5);
        assert_eq!(config.companion_m_atk_multiplier, 0.5);
        assert_eq!(config.companion_hp_multiplier, 3.0);
        assert!(!config.companion_auto_join_party);
        assert_eq!(config.companion_combat_mode, CompanionCombatMode::Aggressive);
        assert_eq!(config.companion_fee_per_level, 2_000);
        assert_eq!(config.companion_max_count, 5);
    }

    #[test]
    fn fee_and_count_out_of_range_fall_back_to_the_default() {
        let text = "CompanionFeePerLevel = -1\nCompanionMaxCount = 9\n";
        let config = L2rConfig::from_ini(&IniFile::parse(L2rConfig::FILE, text).unwrap());
        assert_eq!(
            (config.companion_fee_per_level, config.companion_max_count),
            (5_000, 3)
        );
        let text = "CompanionFeePerLevel = 0\nCompanionMaxCount = 8\n";
        let config = L2rConfig::from_ini(&IniFile::parse(L2rConfig::FILE, text).unwrap());
        assert_eq!(
            (config.companion_fee_per_level, config.companion_max_count),
            (0, 8)
        );
    }

    #[test]
    fn combat_mode_is_case_insensitive() {
        assert_eq!(
            "passive".parse::<CompanionCombatMode>(),
            Ok(CompanionCombatMode::Passive)
        );
        assert_eq!(
            "Aggressive".parse::<CompanionCombatMode>(),
            Ok(CompanionCombatMode::Aggressive)
        );
        assert_eq!("sometimes".parse::<CompanionCombatMode>(), Err(()));
    }

    #[test]
    fn out_of_range_multipliers_fall_back_to_the_default() {
        let text =
            "CompanionPAtkMultiplier = 10.5\nCompanionMAtkMultiplier = -1\nCompanionHpMultiplier = 0.05\n";
        let config = L2rConfig::from_ini(&IniFile::parse(L2rConfig::FILE, text).unwrap());
        assert_eq!(config.companion_p_atk_multiplier, 1.0);
        assert_eq!(config.companion_m_atk_multiplier, 1.0);
        assert_eq!(config.companion_hp_multiplier, 1.0);
    }

    #[test]
    fn range_edges_are_inside() {
        let text =
            "CompanionPAtkMultiplier = 0\nCompanionHpMultiplier = 10\nCompanionMAtkMultiplier = 10.0\n";
        let config = L2rConfig::from_ini(&IniFile::parse(L2rConfig::FILE, text).unwrap());
        assert_eq!(config.companion_p_atk_multiplier, 0.0);
        assert_eq!(config.companion_hp_multiplier, 10.0);
        assert_eq!(config.companion_m_atk_multiplier, 10.0);
    }

    #[test]
    fn unknown_combat_mode_falls_back_to_passive() {
        let config =
            L2rConfig::from_ini(&IniFile::parse(L2rConfig::FILE, "CompanionCombatMode = BERSERK\n").unwrap());
        assert_eq!(config.companion_combat_mode, CompanionCombatMode::Passive);
    }
}
