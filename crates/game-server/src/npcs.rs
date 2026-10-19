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
    }
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
    world.set_npc_settings(npc_settings(config, config_dir));
    world.set_ai_settings(ai_settings(config));
    world.set_water_check(water_check(datapack));
    world.set_id_factory(ids);
    if config.development.no_spawns {
        // Java: nothing is loaded and nothing is logged.
        return;
    }
    world.set_npc_data(Arc::clone(&datapack.npcs), Arc::clone(&datapack.spawns));
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
