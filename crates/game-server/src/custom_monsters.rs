//! The settings of the custom monster features (M9-C01): `Custom/ChampionMonsters.ini`,
//! `Custom/BossAnnouncements.ini` and `Custom/NpcStatMultipliers.ini`, read where the generated
//! config has no list type.
// Java: config/custom/{ChampionMonstersConfig,BossAnnouncementsConfig,NpcStatMultipliersConfig}.java

use std::collections::BTreeSet;
use std::path::Path;

use l2_config::{GameConfig, IniFile};
use l2_data::{NpcStatMultipliers, StatMultipliers};
use l2_world::custom::{BossAnnouncementSettings, ChampionSettings, CustomSettings};
use l2_world::rewards::drop::ItemHolder;
use tracing::warn;

fn load_ini(config_dir: &Path, file: &str) -> IniFile {
    IniFile::load(config_dir.join(file)).unwrap_or_else(|_| IniFile::empty(file))
}

/// Java `ChampionMonstersConfig.CHAMPION_REWARD_ITEMS`: `ChampionRewardItems = 4356,10;4357,5`.
/// A broken entry is skipped with a warning (Java's `Integer.parseInt` would abort the load).
pub fn parse_champion_reward_items(value: &str) -> Vec<ItemHolder> {
    let mut items = Vec::new();
    for entry in value.split(';') {
        if entry.is_empty() {
            continue;
        }
        let mut parts = entry.split(',');
        let id = parts.next().and_then(|p| p.parse::<i32>().ok());
        let count = parts.next().and_then(|p| p.parse::<i64>().ok());
        match (id, count) {
            (Some(id), Some(count)) => items.push(ItemHolder { id, count }),
            _ => warn!("ChampionMonsters.ini: ChampionRewardItems has an invalid entry: {entry:?}"),
        }
    }
    items
}

/// Java `BossAnnouncementsConfig`: `Raidboss...Announcements = 29001, 29006`.
pub fn parse_id_list(value: &str, key: &str) -> BTreeSet<i32> {
    let mut ids = BTreeSet::new();
    for id in value.split(',') {
        if id.is_empty() {
            continue;
        }
        match id.trim().parse::<i32>() {
            Ok(id) => {
                ids.insert(id);
            }
            Err(_) => warn!("BossAnnouncements.ini: {key} has an invalid id: {id:?}"),
        }
    }
    ids
}

/// The champion and boss announcement settings the world reads.
pub fn custom_settings(config: &GameConfig, config_dir: &Path) -> CustomSettings {
    let c = &config.champion_monsters;
    let champion_ini = load_ini(config_dir, "Custom/ChampionMonsters.ini");
    let b = &config.boss_announcements;
    let boss_ini = load_ini(config_dir, "Custom/BossAnnouncements.ini");
    CustomSettings {
        champion: ChampionSettings {
            enable: c.champion_enable,
            passive: c.champion_passive,
            frequency: c.champion_frequency,
            title: c.champ_title.clone(),
            show_aura: c.show_champion_aura,
            min_level: c.champ_min_level,
            max_level: c.champ_max_level,
            hp: c.champion_hp,
            hp_regen: c.champion_hp_regen,
            rewards_exp_sp: c.champion_rewards_exp_sp,
            rewards_chance: c.champion_rewards_chance,
            rewards_amount: c.champion_rewards_amount,
            adenas_rewards_chance: c.champion_adenas_rewards_chance,
            adenas_rewards_amount: c.champion_adenas_rewards_amount,
            reward_lower_level_item_chance: c.champion_reward_lower_level_item_chance,
            reward_higher_level_item_chance: c.champion_reward_higher_level_item_chance,
            reward_items: parse_champion_reward_items(
                &champion_ini.get_string("ChampionRewardItems", "4356,10"),
            ),
            enable_vitality: c.champion_enable_vitality,
            enable_in_instances: c.champion_enable_in_instances,
        },
        boss_announcements: BossAnnouncementSettings {
            raidboss_spawn_announcements: b.raidboss_spawn_announcements,
            raidboss_defeat_announcements: b.raidboss_defeat_announcements,
            raidboss_instance_announcements: b.raidboss_instance_announcements,
            grandboss_spawn_announcements: b.grandboss_spawn_announcements,
            grandboss_defeat_announcements: b.grandboss_defeat_announcements,
            grandboss_instance_announcements: b.grandboss_instance_announcements,
            raidbosses_excluded_from_spawn_announcements: parse_id_list(
                &boss_ini.get_string("RaidbossExcludedFromSpawnAnnouncements", ""),
                "RaidbossExcludedFromSpawnAnnouncements",
            ),
            raidbosses_excluded_from_defeat_announcements: parse_id_list(
                &boss_ini.get_string("RaidbossExcludedFromDefeatAnnouncements", ""),
                "RaidbossExcludedFromDefeatAnnouncements",
            ),
        },
    }
}

/// `NpcStatMultipliers.ini` for the NPC loader: `None` unless `EnableNpcStatMultipliers` is on.
pub fn npc_stat_multipliers(config: &GameConfig) -> Option<NpcStatMultipliers> {
    let m = &config.npc_stat_multipliers;
    if !m.enable_npc_stat_multipliers {
        return None;
    }
    Some(NpcStatMultipliers {
        monster: StatMultipliers {
            hp: m.monster_hp_multiplier,
            mp: m.monster_mp_multiplier,
            p_atk: m.monster_patk_multiplier,
            m_atk: m.monster_matk_multiplier,
            p_def: m.monster_pdef_multiplier,
            m_def: m.monster_mdef_multiplier,
            aggro_range: m.monster_agrro_range_multiplier,
            clan_help_range: m.monster_clan_help_range_multiplier,
        },
        raidboss: StatMultipliers {
            hp: m.raidboss_hp_multiplier,
            mp: m.raidboss_mp_multiplier,
            p_atk: m.raidboss_patk_multiplier,
            m_atk: m.raidboss_matk_multiplier,
            p_def: m.raidboss_pdef_multiplier,
            m_def: m.raidboss_mdef_multiplier,
            aggro_range: m.raidboss_agrro_range_multiplier,
            clan_help_range: m.raidboss_clan_help_range_multiplier,
        },
        guard: StatMultipliers {
            hp: m.guard_hp_multiplier,
            mp: m.guard_mp_multiplier,
            p_atk: m.guard_patk_multiplier,
            m_atk: m.guard_matk_multiplier,
            p_def: m.guard_pdef_multiplier,
            m_def: m.guard_mdef_multiplier,
            aggro_range: m.guard_agrro_range_multiplier,
            clan_help_range: m.guard_clan_help_range_multiplier,
        },
        defender: StatMultipliers {
            hp: m.defender_hp_multiplier,
            mp: m.defender_mp_multiplier,
            p_atk: m.defender_patk_multiplier,
            m_atk: m.defender_matk_multiplier,
            p_def: m.defender_pdef_multiplier,
            m_def: m.defender_mdef_multiplier,
            aggro_range: m.defender_agrro_range_multiplier,
            clan_help_range: m.defender_clan_help_range_multiplier,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reward_items_follow_java_syntax() {
        assert_eq!(
            parse_champion_reward_items("4356,10;4357,5;"),
            vec![
                ItemHolder { id: 4356, count: 10 },
                ItemHolder { id: 4357, count: 5 }
            ]
        );
        assert!(parse_champion_reward_items("").is_empty());
        assert!(parse_champion_reward_items("abc").is_empty());
    }

    #[test]
    fn id_lists_skip_empty_entries_and_trim() {
        assert!(parse_id_list("", "k").is_empty());
        assert_eq!(
            parse_id_list("29001, 29006,", "k"),
            BTreeSet::from([29001, 29006])
        );
    }

    #[test]
    fn everything_is_off_with_no_config_files() {
        let dir = tempfile::tempdir().unwrap();
        let config = GameConfig::load(dir.path()).unwrap();
        assert_eq!(npc_stat_multipliers(&config), None);
        assert_eq!(custom_settings(&config, dir.path()), CustomSettings::default());
    }
}
