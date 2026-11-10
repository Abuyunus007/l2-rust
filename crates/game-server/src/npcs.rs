//! Hands the world the NPC templates and spawn list and starts the spawns, where Java does
//! (`GameServer`: `SpawnData.getInstance()` then `DayNightSpawnManager...notifyChangeMode()`).

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use l2_config::{CompanionCombatMode as L2rCombatMode, GameConfig, L2rConfig};
use l2_data::Datapack;
use l2_world::npc::{NpcSettings, WaterCheck};
use l2_world::{
    AiSettings, CompanionCombatMode, CompanionSettings, IdFactory, L2rSettings, RiftInvasionSettings, World,
};
use tracing::info;

/// The NPC-related configuration values.
pub fn npc_settings(config: &GameConfig, _config_dir: &Path) -> NpcSettings {
    let random = &config.random_spawns;
    // RandomSpawnsConfig: Java reads the ids only when the feature is on.
    let mobs_not_random: HashSet<i32> = if random.enable_random_monster_spawns {
        random.mobs_list_not_random.iter().copied().collect()
    } else {
        HashSet::new()
    };
    NpcSettings {
        show_npc_level: config.npc.show_npc_level,
        show_npc_aggression: config.npc.show_npc_aggression,
        enable_random_enchant_effect: config.npc.enable_random_enchant_effect,
        random_monster_spawns: random.enable_random_monster_spawns,
        mob_max_spawn_range: random.mob_max_spawn_range,
        mobs_not_random,
        // GeneralConfig: `MinNpcAnimation` and co., read by the random animation (`ai/idle.rs`).
        min_npc_animation: config.general.min_npc_animation,
        max_npc_animation: config.general.max_npc_animation,
        min_monster_animation: config.general.min_monster_animation,
        max_monster_animation: config.general.max_monster_animation,
    }
}

/// The `Npc.ini` values the NPC AI reads (Java `NpcConfig`).
pub fn ai_settings(config: &GameConfig) -> AiSettings {
    let npc = &config.npc;
    AiSettings {
        max_drift_range: npc.max_drift_range,
        aggro_distance_check_enabled: npc.aggro_distance_check_enabled,
        aggro_distance_check_range: npc.aggro_distance_check_range,
        aggro_distance_check_raids: npc.aggro_distance_check_raids,
        aggro_distance_check_raid_range: npc.aggro_distance_check_raid_range,
        aggro_distance_check_instances: npc.aggro_distance_check_instances,
        aggro_distance_check_restore_life: npc.aggro_distance_check_restore_life,
        alt_mob_agro_in_peacezone: npc.alt_mob_agro_in_peacezone,
        attackables_camp_player_corpses: npc.attackables_camp_player_corpses,
        raid_chaos_time: npc.raid_chaos_time,
        grand_chaos_time: npc.grand_chaos_time,
        minion_chaos_time: npc.minion_chaos_time,
        guard_attack_aggro_mob: npc.guard_attack_aggro_mob,
        // Java reads the key as an int into a double and casts it back where it is used.
        raid_minion_respawn_timer: npc.raid_minion_respawn_timer as i32,
        minions_respawn_time: npc.minions_respawn_time.clone(),
        force_delete_minions: npc.force_delete_minions,
    }
}

/// The switches of the custom scripts (M9-C07): `Custom/DelevelManager.ini`,
/// `Custom/NoblessMaster.ini`, `Custom/Transmog.ini`, `CustomNpcData`, and the `<schedule>`
/// patterns of the `config.xml` of each custom event.
pub fn custom_script_settings(
    config: &GameConfig,
    _config_dir: &Path,
    datapack_root: &Path,
) -> l2_world::custom::CustomScriptSettings {
    use l2_world::custom::{
        CustomScriptSettings, DelevelManagerSettings, NoblessMasterSettings, TransmogSettings,
    };
    let d = &config.delevel_manager;
    let n = &config.nobless_master;
    let t = &config.transmog;
    let banned_item_ids = t.transmog_banned_item_ids.clone();
    let mut event_schedules = std::collections::BTreeMap::new();
    for (name, folder) in [
        ("Elpies", "Elpies"),
        ("Rabbits", "Rabbits"),
        ("Race", "Race"),
        ("Deathmatch", "Deathmatch"),
        ("TvT", "TeamVsTeam"),
        ("CtF", "CaptureTheFlag"),
    ] {
        let path = datapack_root.join(format!("data/scripts/custom/events/{folder}/config.xml"));
        let patterns = std::fs::read_to_string(&path)
            .map(|text| event_schedule_patterns(&text))
            .unwrap_or_default();
        event_schedules.insert(name.to_owned(), patterns);
    }
    CustomScriptSettings {
        delevel: DelevelManagerSettings {
            enabled: d.delevel_manager_enabled,
            npc_id: d.delevel_manager_npcid,
            item_id: d.delevel_manager_itemid,
            item_count: d.delevel_manager_itemcount,
            minimum_delevel: d.delevel_manager_minimum_delevel,
        },
        nobless: NoblessMasterSettings {
            enabled: n.nobless_master_enabled,
            npc_id: n.nobless_master_npcid,
            level_requirement: n.nobless_master_level_requirement,
            item_id: n.nobless_master_item_id,
            item_count: n.nobless_master_item_count,
            reward_tiara: n.nobless_master_reward_tiara,
        },
        transmog: TransmogSettings {
            enabled: t.enable_transmog,
            share_account: t.transmog_share_account,
            apply_cost: t.transmog_apply_cost,
            remove_cost: t.transmog_remove_cost,
            banned_item_ids,
        },
        event_schedules,
        custom_npc_data: config.general.custom_npc_data,
        // The zones of the rift invasions (`data/RiftInvasion.xml`, D-64).
        rift_zones: l2_data::RiftZones::load(datapack_root).into_zones(),
    }
}

/// The settings of `config/L2R.ini` for the world (L2Rust's own features, not in Java).
pub fn l2r_settings(config: &GameConfig) -> L2rSettings {
    l2r_settings_of(&config.l2r)
}

/// [`l2r_settings`] for a file read again by `//reload l2r` (the game server task reads it).
pub fn l2r_settings_of(l2r: &L2rConfig) -> L2rSettings {
    let c = l2r;
    L2rSettings {
        companions: CompanionSettings {
            p_atk_multiplier: c.companion_p_atk_multiplier,
            m_atk_multiplier: c.companion_m_atk_multiplier,
            hp_multiplier: c.companion_hp_multiplier,
            auto_join_party: c.companion_auto_join_party,
            combat_mode: match c.companion_combat_mode {
                L2rCombatMode::Aggressive => CompanionCombatMode::Aggressive,
                L2rCombatMode::Passive => CompanionCombatMode::Passive,
            },
            fee_per_level: c.companion_fee_per_level,
            max_count: usize::try_from(c.companion_max_count).unwrap_or(1),
        },
        rift: rift_settings_of(&c.rift),
    }
}

/// The `[RiftInvasion]` section for the world (D-64): the minutes and seconds in milliseconds.
fn rift_settings_of(r: &l2_config::RiftInvasionConfig) -> RiftInvasionSettings {
    const MINUTE: i64 = 60_000;
    RiftInvasionSettings {
        enabled: r.enabled,
        interval_min_ms: r.interval_min_minutes * MINUTE,
        interval_max_ms: r.interval_max_minutes * MINUTE,
        announce_lead_ms: r.announce_lead_minutes * MINUTE,
        announce_reminder: r.announce_reminder,
        wave_count: usize::try_from(r.wave_count).unwrap_or(1),
        wave_interval_ms: r.wave_interval_seconds * 1_000,
        duration_ms: r.duration_minutes * MINUTE,
        mob_level_offset: r.mob_level_offset,
        scale_with_players: r.scale_with_players,
        scale_per_player_percent: r.scale_per_player_percent,
        scale_max_percent: r.scale_max_percent,
        participation_radius: r.participation_radius,
        min_contribution_percent: r.min_contribution_percent,
        shard_item_id: r.shard_item_id,
        shards_per_wave: r.shards_per_wave,
        shards_boss: r.shards_boss,
        top_contributor_bonus: r.top_contributor_bonus,
        shard_bound: r.shard_bound,
        weekly_cap: r.weekly_cap,
        weekly_reset_day: r.weekly_reset_day,
        weekly_reset_hour: r.weekly_reset_hour,
        mobs_normal_drops: r.mobs_normal_drops,
        mobs_exp_rate: r.mobs_exp_rate,
        mobs_sp_rate: r.mobs_sp_rate,
        merchant_npc_id: r.merchant_npc_id,
        merchant_loc: r.merchant_loc,
        zone_choice: match r.zone_choice {
            l2_config::RiftZoneChoice::Random => l2_world::RiftZoneChoice::Random,
            l2_config::RiftZoneChoice::ByPlayers => l2_world::RiftZoneChoice::ByPlayers,
        },
        zone_level_margin: r.zone_level_margin,
        adaptive_level: r.adaptive_level,
        adaptive_range: r.adaptive_range,
    }
}

/// The `pattern` attributes of the `<schedule>` elements of an event's `config.xml`, in file
/// order; the commented-out ones (the shipped default) do not count.
pub fn event_schedule_patterns(xml: &str) -> Vec<String> {
    let mut text = String::new();
    let mut rest = xml;
    while let Some(start) = rest.find("<!--") {
        text.push_str(&rest[..start]);
        match rest[start..].find("-->") {
            Some(end) => rest = &rest[start + end + 3..],
            None => {
                rest = "";
                break;
            }
        }
    }
    text.push_str(rest);
    let mut patterns = Vec::new();
    let mut tail = text.as_str();
    while let Some(at) = tail.find("<schedule") {
        tail = &tail[at + "<schedule".len()..];
        let element = tail.split('>').next().unwrap_or("");
        if let Some(p) = element.find("pattern=\"") {
            let value = &element[p + "pattern=\"".len()..];
            if let Some(end) = value.find('"') {
                patterns.push(value[..end].to_owned());
            }
        }
    }
    patterns
}

/// `ZoneManager.getZone(x, y, z, WaterZone.class) != null` over the datapack's zones.
pub fn water_check(datapack: &Arc<Datapack>) -> WaterCheck {
    let datapack = Arc::clone(datapack);
    Arc::new(move |x, y, z| datapack.zones.is_inside_zone_type(x, y, z, "WaterZone"))
}

/// Installs the NPC data into the world and spawns everything. Runs in the world's setup, before
/// the world thread starts ticking, so a long spawn does not delay any tick.
pub fn spawn_npcs(
    world: &mut World,
    datapack: &Arc<Datapack>,
    ids: Arc<IdFactory>,
    config: &GameConfig,
    config_dir: &Path,
) {
    spawn_npcs_with_scripts(world, datapack, ids, config, config_dir, None);
}

/// The lines of `data/FakePlayerChatData.xml` for the world (at the start and for
/// `//reload fakeplayerchat`).
pub fn fake_player_chat_entries(root: &Path) -> Vec<l2_world::FakePlayerChatEntry> {
    l2_data::load_fake_player_chat(root)
        .into_iter()
        .map(|c| l2_world::FakePlayerChatEntry {
            fpc_name: c.fpc_name,
            search_method: c.search_method,
            search_text: c.search_text,
            answers: c.answers,
        })
        .collect()
}

/// [`spawn_npcs`] with the scripts installed between the NPC templates and the spawns (Java
/// `GameServer`: the "Scripts" section, then `SpawnData`), so `ON_NPC_SPAWN` hears the first
/// spawns (S-API-01).
pub fn spawn_npcs_with_scripts(
    world: &mut World,
    datapack: &Arc<Datapack>,
    ids: Arc<IdFactory>,
    config: &GameConfig,
    config_dir: &Path,
    scripts: Option<l2_world::ScriptRegistry>,
) {
    world.set_npc_settings(npc_settings(config, config_dir));
    world.set_custom_script_settings(custom_script_settings(config, config_dir, &datapack.root));
    world.set_l2r_settings(l2r_settings(config));
    world.set_custom_settings(crate::custom_monsters::custom_settings(config, config_dir));
    world.set_ai_settings(ai_settings(config));
    world.set_water_check(water_check(datapack));
    world.set_id_factory(ids);
    if config.development.no_spawns {
        if let Some(scripts) = scripts {
            world.install_scripts(scripts);
        }
        return;
    }
    world.set_npc_data(Arc::clone(&datapack.npcs), Arc::clone(&datapack.spawns));
    // Java `FakePlayerChatManager`: the names and the chat lines of the fake players, when they
    // are on (`Custom/FakePlayers.ini`).
    if config.fake_players.fake_players_enabled {
        let chat = if config.fake_players.fake_player_chat {
            fake_player_chat_entries(&datapack.root)
        } else {
            Vec::new()
        };
        world.load_fake_players(chat);
    }
    if let Some(scripts) = scripts {
        world.install_scripts(scripts);
    }
    let started = Instant::now();
    let report = world.spawn_all_npcs(World::is_night_now());
    // Java GameTimeTaskManager: the day/night spawns switch while the server runs.
    world.start_game_clock();
    info!(
        "NPC spawns: {} NPCs in the world ({} minions) in {} ms.",
        report.npcs,
        report.minions,
        started.elapsed().as_millis()
    );
}

#[cfg(test)]
mod l2r_tests {
    use super::{l2r_settings, l2r_settings_of};
    use l2_config::{CompanionCombatMode as L2rCombatMode, GameConfig, L2rConfig};
    use l2_world::{CompanionCombatMode, CompanionSettings, L2rSettings};

    #[test]
    fn the_shipped_defaults_map_to_the_world_defaults() {
        assert_eq!(l2r_settings(&GameConfig::default()), L2rSettings::default());
    }

    #[test]
    fn every_companion_value_is_mapped() {
        let config = GameConfig {
            l2r: L2rConfig {
                companion_p_atk_multiplier: 2.0,
                companion_m_atk_multiplier: 3.5,
                companion_hp_multiplier: 0.5,
                companion_auto_join_party: false,
                companion_combat_mode: L2rCombatMode::Aggressive,
                companion_fee_per_level: 1_234,
                companion_max_count: 6,
                rift: l2_config::RiftInvasionConfig {
                    enabled: false,
                    interval_min_minutes: 2,
                    interval_max_minutes: 3,
                    wave_interval_seconds: 15,
                    shard_item_id: 4405,
                    merchant_npc_id: 50_030,
                    zone_choice: l2_config::RiftZoneChoice::Random,
                    adaptive_level: true,
                    adaptive_range: 2,
                    ..l2_config::RiftInvasionConfig::default()
                },
            },
            ..GameConfig::default()
        };
        assert_eq!(
            l2r_settings(&config).companions,
            CompanionSettings {
                p_atk_multiplier: 2.0,
                m_atk_multiplier: 3.5,
                hp_multiplier: 0.5,
                auto_join_party: false,
                combat_mode: CompanionCombatMode::Aggressive,
                fee_per_level: 1_234,
                max_count: 6,
            }
        );
        let rift = l2r_settings(&config).rift;
        assert!(!rift.enabled);
        assert_eq!((rift.interval_min_ms, rift.interval_max_ms), (120_000, 180_000));
        assert_eq!(rift.wave_interval_ms, 15_000);
        assert_eq!((rift.shard_item_id, rift.merchant_npc_id), (4405, 50_030));
        assert_eq!(rift.zone_choice, l2_world::RiftZoneChoice::Random);
        assert_eq!((rift.adaptive_level, rift.adaptive_range), (true, 2));
        // The reload path maps the file the same way.
        assert_eq!(l2r_settings_of(&config.l2r), l2r_settings(&config));
    }
}

#[cfg(test)]
mod custom_script_tests {
    use super::event_schedule_patterns;

    #[test]
    fn the_shipped_event_configs_schedule_nothing() {
        let xml = r#"<event name="Elpies"><!-- <schedule pattern="0 20 * * *" /> --></event>"#;
        assert!(event_schedule_patterns(xml).is_empty());
    }

    #[test]
    fn an_uncommented_schedule_is_read_in_file_order() {
        let xml = r#"<event name="Race"><!-- c --><schedule pattern="0 20 * * *" /><schedule pattern="30 6 * * 1"/></event>"#;
        assert_eq!(event_schedule_patterns(xml), ["0 20 * * *", "30 6 * * 1"]);
    }
}
