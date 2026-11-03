//! The settings of the custom monster features (M9-C01): `Custom/ChampionMonsters.ini`,
//! `Custom/BossAnnouncements.ini` and `Custom/NpcStatMultipliers.ini`, from the generated config.
// Java: config/custom/{ChampionMonstersConfig,BossAnnouncementsConfig,NpcStatMultipliersConfig}.java

use std::path::Path;

use l2_config::GameConfig;
use l2_data::{NpcStatMultipliers, StatMultipliers};
use l2_world::custom::{BossAnnouncementSettings, ChampionSettings, CustomSettings};
use l2_world::rewards::drop::ItemHolder;

/// The champion and boss announcement settings the world reads.
pub fn custom_settings(config: &GameConfig, _config_dir: &Path) -> CustomSettings {
    let c = &config.champion_monsters;
    let b = &config.boss_announcements;
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
            reward_items: c
                .champion_reward_items
                .iter()
                .map(|item| ItemHolder {
                    id: item.id,
                    count: item.count,
                })
                .collect(),
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
            raidbosses_excluded_from_spawn_announcements: b
                .raidbosses_excluded_from_spawn_announcements
                .clone(),
            raidbosses_excluded_from_defeat_announcements: b
                .raidbosses_excluded_from_defeat_announcements
                .clone(),
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
    fn everything_is_off_with_no_config_files() {
        let dir = tempfile::tempdir().unwrap();
        let config = GameConfig::load(dir.path()).unwrap();
        assert_eq!(npc_stat_multipliers(&config), None);
        assert_eq!(custom_settings(&config, dir.path()), CustomSettings::default());
    }
}
