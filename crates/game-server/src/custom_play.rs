//! The settings of the custom features of M9-C02 (`config/Custom/PremiumSystem.ini`,
//! `AutoPotions.ini`, `AutoPlay.ini`, `OfflinePlay.ini` and the custom keys of `OfflineTrade.ini`),
//! built from the config the way the Java `*Config.load()` methods read them.
// The generated structs hold the plain keys; the lists and maps (`Map<Integer, Float>`,
// `Set<Integer>`, `List<AbnormalVisualEffect>`, the hexadecimal colour) are read from the ini
// again here.

use std::collections::HashMap;
use std::path::Path;

use l2_config::{GameConfig, IniFile};
use l2_data::skill::AbnormalVisualEffect;
use l2_world::{
    AutoPlayRules, AutoPotionSettings, OfflineCommandSettings, OfflinePlaySettings, PremiumRates,
    PremiumSettings, ShownRates, World,
};
use tracing::warn;

/// All the settings of this module, installed together.
#[derive(Debug, Clone, PartialEq)]
pub struct CustomPlaySettings {
    pub premium: PremiumSettings,
    pub auto_potions: AutoPotionSettings,
    pub auto_play: AutoPlayRules,
    pub offline_play: OfflinePlaySettings,
    pub offline_command: OfflineCommandSettings,
}

impl CustomPlaySettings {
    /// Installs them in the world. Call before players enter.
    pub fn install(self, world: &mut World) {
        world.set_premium_settings(self.premium);
        world.set_auto_potion_settings(self.auto_potions);
        world.set_auto_play_rules(self.auto_play);
        world.set_offline_play_settings(self.offline_play);
        world.set_offline_command_settings(self.offline_command);
    }
}

/// Reads one file of `config/Custom/`, empty (every key at its default) when it cannot be read.
fn custom_ini(config_dir: &Path, file: &str) -> IniFile {
    IniFile::load(config_dir.join(file)).unwrap_or_else(|_| IniFile::empty(file))
}

/// `PremiumSystemConfig.load()`'s `Map<Integer, Float>` of `item,rate;item,rate`.
fn premium_rate_map(ini: &IniFile, key: &str) -> HashMap<i32, f32> {
    let text = ini.get_string(key, "");
    let mut map = HashMap::new();
    if text.is_empty() {
        return map;
    }
    for item in text.split(';') {
        let parts: Vec<&str> = item.split(',').collect();
        if parts.len() != 2 {
            warn!("PremiumSystemConfig.load(): invalid config property -> {key} \"{item}\"");
            continue;
        }
        match (parts[0].parse::<i32>(), parts[1].parse::<f32>()) {
            (Ok(id), Ok(rate)) => {
                map.insert(id, rate);
            }
            _ => {
                if !item.is_empty() {
                    warn!("PremiumSystemConfig.load(): invalid config property -> {key} \"{item}\"");
                }
            }
        }
    }
    map
}

/// `PremiumSystemConfig` (the premium keys) and the server rates the `.premium` page shows.
pub fn premium_settings(config: &GameConfig, config_dir: &Path) -> PremiumSettings {
    let p = &config.premium_system;
    let ini = custom_ini(config_dir, l2_config::game::PremiumSystemConfig::FILE);
    PremiumSettings {
        enabled: p.premium_system_enabled,
        account_wide: p.account_wide_premium,
        pc_cafe_retail_like: p.pc_cafe_retail_like,
        shown_rates: ShownRates {
            rate_xp: config.rates.rate_xp,
            rate_sp: config.rates.rate_sp,
            death_drop_chance: config.rates.rate_death_drop_chance_multiplier,
            death_drop_amount: config.rates.rate_death_drop_amount_multiplier,
            spoil_drop_chance: config.rates.rate_spoil_drop_chance_multiplier,
            spoil_drop_amount: config.rates.rate_spoil_drop_amount_multiplier,
        },
        rates: PremiumRates {
            xp: p.premium_rate_xp,
            sp: p.premium_rate_sp,
            drop_chance: p.premium_rate_drop_chance,
            drop_amount: p.premium_rate_drop_amount,
            spoil_chance: p.premium_rate_spoil_chance,
            spoil_amount: p.premium_rate_spoil_amount,
            quest_xp: p.premium_rate_quest_xp,
            quest_sp: p.premium_rate_quest_sp,
            drop_chance_by_id: premium_rate_map(&ini, "PremiumRateDropChanceByItemId"),
            drop_amount_by_id: premium_rate_map(&ini, "PremiumRateDropAmountByItemId"),
        },
    }
}

/// `AutoPotionsConfig`: the three lists are `getString(key, "0").split(",")`.
pub fn auto_potion_settings(config: &GameConfig, config_dir: &Path) -> AutoPotionSettings {
    let c = &config.auto_potions;
    let ini = custom_ini(config_dir, l2_config::game::AutoPotionsConfig::FILE);
    let list = |key: &str| -> Vec<i32> {
        ini.get_string(key, "0")
            .split(',')
            .filter_map(|s| match s.parse::<i32>() {
                Ok(id) => Some(id),
                Err(_) => {
                    warn!("AutoPotions.ini: {key} has \"{s}\", which is no item id.");
                    None
                }
            })
            .collect()
    };
    AutoPotionSettings {
        enabled: c.auto_potions_enabled,
        in_olympiad: c.auto_potions_in_olympiad,
        min_level: c.auto_potion_min_level,
        cp_enabled: c.auto_cp_enabled,
        hp_enabled: c.auto_hp_enabled,
        mp_enabled: c.auto_mp_enabled,
        cp_percentage: c.auto_cp_percentage,
        hp_percentage: c.auto_hp_percentage,
        mp_percentage: c.auto_mp_percentage,
        cp_item_ids: list("AutoCpItemIds"),
        hp_item_ids: list("AutoHpItemIds"),
        mp_item_ids: list("AutoMpItemIds"),
    }
}

/// `AutoPlayConfig`: the sets are comma separated ids (`DisabledSkillIds`, `DisabledItemIds`,
/// `IgnoredAutoPickItems`).
pub fn auto_play_rules(config: &GameConfig, config_dir: &Path) -> AutoPlayRules {
    let c = &config.auto_play;
    let ini = custom_ini(config_dir, l2_config::game::AutoPlayConfig::FILE);
    let set = |key: &str| -> Vec<i32> {
        let text = ini.get_string(key, "");
        let text = text.trim();
        if text.is_empty() {
            return Vec::new();
        }
        text.split(',')
            .filter_map(|s| match s.trim().parse::<i32>() {
                Ok(id) => Some(id),
                Err(_) => {
                    warn!("AutoPlay.ini: {key} has \"{s}\", which is no id.");
                    None
                }
            })
            .collect()
    };
    AutoPlayRules {
        enable_auto_play: c.enable_auto_play,
        enable_auto_potion: c.enable_auto_potion,
        enable_auto_skill: c.enable_auto_skill,
        enable_auto_item: c.enable_auto_item,
        resume_auto_play: c.resume_auto_play,
        enable_auto_assist: c.enable_auto_assist,
        short_range: c.auto_play_short_range,
        long_range: c.auto_play_long_range,
        auto_play_premium: c.auto_play_premium,
        disabled_auto_skills: set("DisabledSkillIds"),
        disabled_auto_items: set("DisabledItemIds"),
        ignored_auto_pick_items: set("IgnoredAutoPickItems"),
        login_message: c.auto_play_login_message.clone(),
    }
}

/// `Enum.valueOf(AbnormalVisualEffect.class, ave.trim())` for each name of a comma separated list
/// (Java stops the start on a wrong name; the name is skipped with a warning here).
fn abnormal_effects(ini: &IniFile, key: &str) -> Vec<AbnormalVisualEffect> {
    let text = ini.get_string(key, "");
    let text = text.trim();
    if text.is_empty() {
        return Vec::new();
    }
    text.split(',')
        .filter_map(|name| match AbnormalVisualEffect::from_name(name.trim()) {
            Some(effect) => Some(effect),
            None => {
                warn!("{key}: \"{}\" is no abnormal visual effect.", name.trim());
                None
            }
        })
        .collect()
}

/// `Integer.decode("0x" + value)`: a value that does not read as hex falls back to the default.
fn hex_color(ini: &IniFile, key: &str) -> i32 {
    let text = ini.get_string(key, "808080");
    i32::from_str_radix(text.trim().trim_start_matches("0x"), 16).unwrap_or_else(|_| {
        warn!("{key} {text:?} is not a hexadecimal colour; using 808080.");
        0x0080_8080
    })
}

/// `OfflinePlayConfig`.
pub fn offline_play_settings(config: &GameConfig, config_dir: &Path) -> OfflinePlaySettings {
    let c = &config.offline_play;
    let ini = custom_ini(config_dir, l2_config::game::OfflinePlayConfig::FILE);
    OfflinePlaySettings {
        enable_offline_play_command: c.enable_offline_play_command,
        restore_auto_play_offliners: c.restore_auto_play_offliners,
        offline_play_premium: c.offline_play_premium,
        offline_play_logout_on_death: c.offline_play_logout_on_death,
        offline_play_disconnect_same_account: c.offline_play_disconnect_same_account,
        offline_play_login_message: c.offline_play_login_message.clone(),
        offline_play_set_name_color: c.offline_play_set_name_color,
        offline_play_name_color: hex_color(&ini, "OfflinePlayNameColor"),
        offline_play_abnormal_effects: abnormal_effects(&ini, "OfflinePlayAbnormalEffect"),
    }
}

/// The keys of `OfflineTrade.ini` that `l2_world::OfflineSettings` does not hold.
pub fn offline_command_settings(config: &GameConfig, config_dir: &Path) -> OfflineCommandSettings {
    let ini = custom_ini(config_dir, l2_config::game::OfflineTradeConfig::FILE);
    OfflineCommandSettings {
        enable_offline_command: config.offline_trade.enable_offline_command,
        offline_abnormal_effects: abnormal_effects(&ini, "OfflineAbnormalEffect"),
    }
}

/// Every setting of this module.
pub fn custom_play_settings(config: &GameConfig, config_dir: &Path) -> CustomPlaySettings {
    CustomPlaySettings {
        premium: premium_settings(config, config_dir),
        auto_potions: auto_potion_settings(config, config_dir),
        auto_play: auto_play_rules(config, config_dir),
        offline_play: offline_play_settings(config, config_dir),
        offline_command: offline_command_settings(config, config_dir),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_switch_every_feature_off() {
        let config = GameConfig::default();
        let dir = Path::new("no-such-config-dir");
        let settings = custom_play_settings(&config, dir);
        assert!(!settings.premium.enabled);
        assert!(!settings.auto_potions.enabled);
        assert!(!settings.auto_play.enable_auto_play);
        assert!(!settings.offline_play.enable_offline_play_command);
        // `EnableOfflineCommand` is on by default but needs OfflineTradeEnable / OfflineCraftEnable.
        assert!(settings.offline_command.enable_offline_command);
        assert_eq!(settings.offline_play.offline_play_name_color, 0x0080_8080);
        assert_eq!(settings.auto_potions.hp_item_ids, vec![0]);
    }

    #[test]
    fn the_premium_item_maps_follow_the_file() {
        let ini = IniFile::parse(
            "PremiumSystem.ini",
            "PremiumRateDropChanceByItemId = 57,2;6656,1.5;bad;1,x\n",
        )
        .unwrap();
        let map = premium_rate_map(&ini, "PremiumRateDropChanceByItemId");
        assert_eq!(map.get(&57), Some(&2.0));
        assert_eq!(map.get(&6656), Some(&1.5));
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn the_shipped_files_are_read() {
        // The repo's own snapshot of the configs (`dist/`, D-55).
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/game/config");
        if !dir.join("Custom/AutoPotions.ini").exists() {
            println!("skipped: dist/game/config is not there");
            return;
        }
        let config = GameConfig::load(&dir).unwrap();
        let settings = custom_play_settings(&config, &dir);
        // The shipped file lists two CP potions; Java walks them in hash order.
        assert_eq!(settings.auto_potions.cp_item_ids, vec![5592, 5591]);
        assert_eq!(settings.auto_potions.hp_item_ids, vec![1540, 1539, 1061, 1060]);
        assert_eq!(settings.premium.rates.drop_chance_by_id.get(&57), Some(&2.0));
        assert_eq!(settings.auto_play.disabled_auto_skills, vec![42]);
        assert_eq!(settings.auto_play.ignored_auto_pick_items, vec![8190, 8689]);
        assert!(!settings.premium.enabled && !settings.auto_play.enable_auto_play);
    }
}
