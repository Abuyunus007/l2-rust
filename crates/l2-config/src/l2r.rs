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

/// `[RiftInvasion]` `RiftZoneChoice`: how the zone of the next rift is picked (D-64).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiftZoneChoice {
    /// Any zone, all alike.
    Random,
    /// A zone that fits the levels of the players online, more likely the more of them fit.
    ByPlayers,
}

impl FromStr for RiftZoneChoice {
    type Err = ();

    /// Case-insensitive: `BY_PLAYERS`, `by_players`, `RANDOM`.
    fn from_str(s: &str) -> Result<Self, ()> {
        if s.eq_ignore_ascii_case("RANDOM") {
            Ok(Self::Random)
        } else if s.eq_ignore_ascii_case("BY_PLAYERS") {
            Ok(Self::ByPlayers)
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
    /// `[RiftInvasion]` (D-64).
    pub rift: RiftInvasionConfig,
}

/// `[RiftInvasion]`: the rift invasions (D-64). An id of 0 means "not set": the feature then logs
/// a warning and stays off.
#[derive(Debug, Clone, PartialEq)]
pub struct RiftInvasionConfig {
    /// `RiftEnabled`, default true.
    pub enabled: bool,
    /// `RiftIntervalMinMinutes`, range 1 to 10080, default 120.
    pub interval_min_minutes: i64,
    /// `RiftIntervalMaxMinutes`, range 1 to 10080, default 240; below the minimum it is the
    /// minimum.
    pub interval_max_minutes: i64,
    /// `RiftAnnounceLeadMinutes`, range 0 to 60, default 5.
    pub announce_lead_minutes: i64,
    /// `RiftAnnounceReminder`: a reminder one minute before the start, default true.
    pub announce_reminder: bool,
    /// `RiftWaveCount`, range 1 to 10, default 3.
    pub wave_count: i32,
    /// `RiftWaveIntervalSeconds`, range 10 to 3600, default 90.
    pub wave_interval_seconds: i64,
    /// `RiftDurationMinutes`, range 1 to 240, default 20.
    pub duration_minutes: i64,
    /// `RiftMobLevelOffset`, range -20 to 20, default 0.
    pub mob_level_offset: i32,
    /// `RiftScaleWithPlayers`, default true.
    pub scale_with_players: bool,
    /// `RiftScalePerPlayerPercent`, range 0 to 100, default 8.
    pub scale_per_player_percent: i32,
    /// `RiftScaleMaxPercent`, range 0 to 1000, default 200.
    pub scale_max_percent: i32,
    /// `RiftParticipationRadius`, range 100 to 10000, default 3000.
    pub participation_radius: i32,
    /// `RiftMinContributionPercent`, range 0.0 to 100.0, default 1.
    pub min_contribution_percent: f64,
    /// `RiftShardItemId`, 0 (not set) or an item id, default 0.
    pub shard_item_id: i32,
    /// `RiftShardsPerWave`, range 0 to 10000, default 5.
    pub shards_per_wave: i64,
    /// `RiftShardsBoss`, range 0 to 10000, default 15.
    pub shards_boss: i64,
    /// `RiftTopContributorBonus`, range 0 to 10000, default 5.
    pub top_contributor_bonus: i64,
    /// `RiftShardBound`, default true.
    pub shard_bound: bool,
    /// `RiftWeeklyCap`, range 0 (no cap) to 1000000, default 400.
    pub weekly_cap: i64,
    /// `RiftWeeklyResetDay`, 1 (Monday) to 7 (Sunday), default 3 (Wednesday).
    pub weekly_reset_day: i32,
    /// `RiftWeeklyResetHour`, range 0 to 23, default 6.
    pub weekly_reset_hour: i32,
    /// `RiftMobsNormalDrops`, default false.
    pub mobs_normal_drops: bool,
    /// `RiftMobsExpRate`: factor on the exp of rift monsters, range 0.0 to 100.0, default 1.0.
    pub mobs_exp_rate: f64,
    /// `RiftMobsSpRate`: factor on the SP of rift monsters, range 0.0 to 100.0, default 1.0.
    pub mobs_sp_rate: f64,
    /// `RiftMerchantNpcId`, 0 (not set) or an NPC id, default 0.
    pub merchant_npc_id: i32,
    /// `RiftMerchantLoc` `x,y,z`, default `83400,148000,-3400` (Giran).
    pub merchant_loc: (i32, i32, i32),
    /// `RiftZoneChoice`, `RANDOM` or `BY_PLAYERS`, default `BY_PLAYERS`.
    pub zone_choice: RiftZoneChoice,
    /// `RiftZoneLevelMargin`: levels a player may be below or above a zone's range and still
    /// count for it, range 0 to 20, default 5.
    pub zone_level_margin: i32,
    /// `RiftAdaptiveLevel`: the monsters take the median level of the players near the rift,
    /// default false.
    pub adaptive_level: bool,
    /// `RiftAdaptiveRange`: how far an adapted level may leave the zone's range, range 0 to 20,
    /// default 5.
    pub adaptive_range: i32,
}

impl RiftInvasionConfig {
    fn from_ini(ini: &IniFile) -> Self {
        let interval_min_minutes = ranged_long(ini, "RiftIntervalMinMinutes", 120, 1, 10_080);
        let mut interval_max_minutes = ranged_long(ini, "RiftIntervalMaxMinutes", 240, 1, 10_080);
        if interval_max_minutes < interval_min_minutes {
            warn!(
                "Config 'RiftIntervalMaxMinutes' in file '{}' is {interval_max_minutes}, below 'RiftIntervalMinMinutes' {interval_min_minutes}, using {interval_min_minutes}.",
                L2rConfig::FILE
            );
            interval_max_minutes = interval_min_minutes;
        }
        Self {
            enabled: ini.get_bool("RiftEnabled", true),
            interval_min_minutes,
            interval_max_minutes,
            announce_lead_minutes: ranged_long(ini, "RiftAnnounceLeadMinutes", 5, 0, 60),
            announce_reminder: ini.get_bool("RiftAnnounceReminder", true),
            wave_count: ranged_long(ini, "RiftWaveCount", 3, 1, 10) as i32,
            wave_interval_seconds: ranged_long(ini, "RiftWaveIntervalSeconds", 90, 10, 3_600),
            duration_minutes: ranged_long(ini, "RiftDurationMinutes", 20, 1, 240),
            mob_level_offset: ranged_long(ini, "RiftMobLevelOffset", 0, -20, 20) as i32,
            scale_with_players: ini.get_bool("RiftScaleWithPlayers", true),
            scale_per_player_percent: ranged_long(ini, "RiftScalePerPlayerPercent", 8, 0, 100) as i32,
            scale_max_percent: ranged_long(ini, "RiftScaleMaxPercent", 200, 0, 1_000) as i32,
            participation_radius: ranged_long(ini, "RiftParticipationRadius", 3_000, 100, 10_000) as i32,
            min_contribution_percent: ranged(ini, "RiftMinContributionPercent", 1.0, 0.0, 100.0),
            shard_item_id: ranged_long(ini, "RiftShardItemId", 0, 0, i64::from(i32::MAX)) as i32,
            shards_per_wave: ranged_long(ini, "RiftShardsPerWave", 5, 0, 10_000),
            shards_boss: ranged_long(ini, "RiftShardsBoss", 15, 0, 10_000),
            top_contributor_bonus: ranged_long(ini, "RiftTopContributorBonus", 5, 0, 10_000),
            shard_bound: ini.get_bool("RiftShardBound", true),
            weekly_cap: ranged_long(ini, "RiftWeeklyCap", 400, 0, 1_000_000),
            weekly_reset_day: ranged_long(ini, "RiftWeeklyResetDay", 3, 1, 7) as i32,
            weekly_reset_hour: ranged_long(ini, "RiftWeeklyResetHour", 6, 0, 23) as i32,
            mobs_normal_drops: ini.get_bool("RiftMobsNormalDrops", false),
            mobs_exp_rate: ranged(ini, "RiftMobsExpRate", 1.0, 0.0, 100.0),
            mobs_sp_rate: ranged(ini, "RiftMobsSpRate", 1.0, 0.0, 100.0),
            merchant_npc_id: ranged_long(ini, "RiftMerchantNpcId", 0, 0, i64::from(i32::MAX)) as i32,
            merchant_loc: location(ini, "RiftMerchantLoc", (83_400, 148_000, -3_400)),
            zone_choice: ini.get_enum("RiftZoneChoice", RiftZoneChoice::ByPlayers),
            zone_level_margin: ranged_long(ini, "RiftZoneLevelMargin", 5, 0, 20) as i32,
            adaptive_level: ini.get_bool("RiftAdaptiveLevel", false),
            adaptive_range: ranged_long(ini, "RiftAdaptiveRange", 5, 0, 20) as i32,
        }
    }
}

impl Default for RiftInvasionConfig {
    fn default() -> Self {
        Self::from_ini(&IniFile::empty(L2rConfig::FILE))
    }
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
            rift: RiftInvasionConfig::from_ini(ini),
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

/// Settings of the login server's `config/L2R.ini` (our own features, D-62).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginL2rConfig {
    /// `[LoginServer]` key `ShowDownServers`: the server list shows the registered game servers
    /// that are down (Java's behaviour), default true. False lists only the servers that are up.
    pub show_down_servers: bool,
}

impl LoginL2rConfig {
    /// File name inside the login server's config directory.
    pub const FILE: &'static str = "L2R.ini";

    pub fn from_ini(ini: &IniFile) -> Self {
        Self {
            show_down_servers: ini.get_bool("ShowDownServers", true),
        }
    }
}

impl Default for LoginL2rConfig {
    fn default() -> Self {
        Self::from_ini(&IniFile::empty(Self::FILE))
    }
}

/// A location `x,y,z`; anything else gives `default` and a warning.
fn location(ini: &IniFile, key: &str, default: (i32, i32, i32)) -> (i32, i32, i32) {
    let Some(text) = ini.get_value(key) else {
        return default;
    };
    let parts: Vec<Option<i32>> = text.split(',').map(|p| p.trim().parse().ok()).collect();
    if let [Some(x), Some(y), Some(z)] = parts[..] {
        return (x, y, z);
    }
    warn!(
        "Config '{key}' in file '{}' is '{text}', not x,y,z, using default: {},{},{}.",
        L2rConfig::FILE,
        default.0,
        default.1,
        default.2
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
                rift: RiftInvasionConfig::default(),
            }
        );
    }

    #[test]
    fn the_login_side_shows_down_servers_unless_told_not_to() {
        assert!(LoginL2rConfig::default().show_down_servers);
        let ini = IniFile::parse(LoginL2rConfig::FILE, "[LoginServer]\nShowDownServers = False\n").unwrap();
        assert!(!LoginL2rConfig::from_ini(&ini).show_down_servers);
    }

    #[test]
    fn rift_defaults_are_the_documented_ones() {
        let r = RiftInvasionConfig::default();
        assert!(r.enabled && r.announce_reminder && r.scale_with_players && r.shard_bound);
        assert!(!r.mobs_normal_drops);
        assert_eq!((r.interval_min_minutes, r.interval_max_minutes), (120, 240));
        assert_eq!(
            (r.announce_lead_minutes, r.wave_count, r.wave_interval_seconds),
            (5, 3, 90)
        );
        assert_eq!((r.duration_minutes, r.mob_level_offset), (20, 0));
        assert_eq!((r.scale_per_player_percent, r.scale_max_percent), (8, 200));
        assert_eq!((r.participation_radius, r.min_contribution_percent), (3_000, 1.0));
        assert_eq!((r.shard_item_id, r.merchant_npc_id), (0, 0));
        assert_eq!(
            (r.shards_per_wave, r.shards_boss, r.top_contributor_bonus),
            (5, 15, 5)
        );
        assert_eq!(
            (r.weekly_cap, r.weekly_reset_day, r.weekly_reset_hour),
            (400, 3, 6)
        );
        assert_eq!((r.mobs_exp_rate, r.mobs_sp_rate), (1.0, 1.0));
        assert_eq!(r.merchant_loc, (83_400, 148_000, -3_400));
        assert_eq!(r.zone_choice, RiftZoneChoice::ByPlayers);
        assert_eq!(
            (r.zone_level_margin, r.adaptive_level, r.adaptive_range),
            (5, false, 5)
        );
    }

    #[test]
    fn rift_zone_choice_and_adaptive_level_are_read() {
        let text = "RiftZoneChoice = random\nRiftZoneLevelMargin = 21\nRiftAdaptiveLevel = true\nRiftAdaptiveRange = 3\n";
        let r = L2rConfig::from_ini(&IniFile::parse(L2rConfig::FILE, text).unwrap()).rift;
        assert_eq!(r.zone_choice, RiftZoneChoice::Random);
        // 21 is out of range: the default.
        assert_eq!(
            (r.zone_level_margin, r.adaptive_level, r.adaptive_range),
            (5, true, 3)
        );
        let r =
            L2rConfig::from_ini(&IniFile::parse(L2rConfig::FILE, "RiftZoneChoice = SOMEWHERE\n").unwrap())
                .rift;
        assert_eq!(r.zone_choice, RiftZoneChoice::ByPlayers);
    }

    #[test]
    fn rift_values_are_read_and_checked() {
        let text = "\
[RiftInvasion]
RiftEnabled = false
RiftIntervalMinMinutes = 30
RiftIntervalMaxMinutes = 10
RiftWaveCount = 11
RiftShardItemId = 4405
RiftMerchantNpcId = 50030
RiftMerchantLoc = 1, -2 ,3
RiftMinContributionPercent = 2.5
RiftWeeklyResetDay = 0
";
        let r = L2rConfig::from_ini(&IniFile::parse(L2rConfig::FILE, text).unwrap()).rift;
        assert!(!r.enabled);
        // The maximum below the minimum is the minimum.
        assert_eq!((r.interval_min_minutes, r.interval_max_minutes), (30, 30));
        // Out of range: the defaults.
        assert_eq!((r.wave_count, r.weekly_reset_day), (3, 3));
        assert_eq!((r.shard_item_id, r.merchant_npc_id), (4405, 50_030));
        assert_eq!(r.merchant_loc, (1, -2, 3));
        assert_eq!(r.min_contribution_percent, 2.5);
        let bad = "RiftMerchantLoc = 1,2\n";
        let r = L2rConfig::from_ini(&IniFile::parse(L2rConfig::FILE, bad).unwrap()).rift;
        assert_eq!(r.merchant_loc, (83_400, 148_000, -3_400));
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
