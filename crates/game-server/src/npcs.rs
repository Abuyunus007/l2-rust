//! Hands the world the NPC templates and spawn list and starts the spawns, where Java does
//! (`GameServer`: `SpawnData.getInstance()` then `DayNightSpawnManager...notifyChangeMode()`).
// Java: GameServer.java, data/xml/SpawnData.java, config/NpcConfig.java,
//       config/custom/RandomSpawnsConfig.java

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use l2_config::{GameConfig, IniFile};
use l2_data::Datapack;
use l2_world::npc::{NpcSettings, WaterCheck};
use l2_world::{AiSettings, IdFactory, World};
use tracing::{info, warn};

/// The NPC-related configuration values.
pub fn npc_settings(config: &GameConfig, config_dir: &Path) -> NpcSettings {
    let random = &config.random_spawns;
    // RandomSpawnsConfig: the ids are only read when the feature is on; the generated config
    // does not carry the list, so it is read here.
    let mut mobs_not_random = HashSet::new();
    if random.enable_random_monster_spawns {
        let file = "Custom/RandomSpawns.ini";
        let ini = IniFile::load(config_dir.join(file)).unwrap_or_else(|_| IniFile::empty(file));
        for id in ini
            .get_string("MobsSpawnNotRandom", "18812,18813,18814,22138")
            .split(',')
        {
            // Java: Integer.parseInt (no trimming) throws on a bad id and aborts the load; a bad
            // id is skipped with a warning here.
            match id.parse::<i32>() {
                Ok(id) => {
                    mobs_not_random.insert(id);
                }
                Err(_) => warn!("RandomSpawns.ini: MobsSpawnNotRandom has an invalid id: {id:?}"),
            }
        }
    }
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
        minions_respawn_time: config.manual.npc.minions_respawn_time.clone(),
        force_delete_minions: npc.force_delete_minions,
    }
}

/// The switches of the custom scripts (M9-C07): `Custom/DelevelManager.ini`,
/// `Custom/NoblessMaster.ini`, `Custom/Transmog.ini`, `CustomNpcData`, and the `<schedule>`
/// patterns of the `config.xml` of each custom event.
pub fn custom_script_settings(
    config: &GameConfig,
    config_dir: &Path,
    datapack_root: &Path,
) -> l2_world::custom::CustomScriptSettings {
    use l2_world::custom::{
        CustomScriptSettings, DelevelManagerSettings, NoblessMasterSettings, TransmogSettings,
    };
    let d = &config.delevel_manager;
    let n = &config.nobless_master;
    let t = &config.transmog;
    let transmog_file = l2_config::TransmogConfig::FILE;
    let ini = IniFile::load(config_dir.join(transmog_file)).unwrap_or_else(|_| IniFile::empty(transmog_file));
    // Java: `Integer.parseInt(s.trim())` of each part of the comma list.
    let banned_item_ids = ini
        .get_string("TransmogBannedItemIds", "")
        .split(',')
        .filter_map(|part| part.trim().parse::<i32>().ok())
        .collect();
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
    world.set_ai_settings(ai_settings(config));
    world.set_water_check(water_check(datapack));
    world.set_id_factory(ids);
    if config.development.no_spawns {
        // Java: nothing is loaded and nothing is logged.
        if let Some(scripts) = scripts {
            world.install_scripts(scripts);
        }
        return;
    }
    world.set_npc_data(Arc::clone(&datapack.npcs), Arc::clone(&datapack.spawns));
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
