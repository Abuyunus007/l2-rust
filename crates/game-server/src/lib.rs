//! The game server: startup, background tasks and shutdown.
// Same layout as `login-server`:
// - `main.rs`    command line, logging, signals, exit code. Nothing else.
// - `lib.rs`     `Server::start` runs the startup steps in Java's order and returns a running
//                `Server`; `Server::run_until` waits for a signal or a shutdown request;
//                `Server::shutdown` stores the players and stops everything.
// - `state.rs`   the shared `Arc<GameState>` (config, pool, ids, datapack, world handle).
// - `runtime.rs` the world thread and its outbox seam.
// - `hexid.rs`, `admin.rs`, `client.rs`, `logging.rs`: one module per concern.

pub mod admin;
pub mod client;
pub mod custom_misc_data;
pub mod custom_monsters;
pub mod custom_play;
pub mod geo;
pub mod hexid;
pub mod item_auctions;
pub mod logging;
pub mod ls_link;
pub mod npcs;
pub mod persist;
pub mod runtime;
pub mod state;
pub mod world_data;

use std::net::SocketAddr;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use l2_config::{ConfigError, DatabaseConfig, GameConfig, IniFile, InterfaceConfig};
use l2_data::{DataError, Datapack, DatapackOptions};
use l2_db::{DbError, IdRepository, Pool};
use l2_net::CancellationToken;
use l2_util::WorldRng;
use l2_world::{IdError, IdFactory, RUST_FIRST_OBJECT_ID, RUST_LAST_OBJECT_ID, StaticData, WorldCommand};
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tracing::{info, warn};

use crate::persist::{DbStore, LsNotice, RetryPolicy};
use crate::runtime::WorldRuntime;
use crate::state::{ExitReason, GameState, ShutdownHandle};
use crate::world_data::DatapackData;

/// The clan values of `Player.ini`, `Feature.ini` and `Server.ini`.
pub fn clan_settings(config: &GameConfig, _config_dir: &Path) -> l2_world::ClanSettings {
    let (player, feature) = (&config.player, &config.feature);
    let template = config.manual.server.clan_name_template.clone();
    l2_world::ClanSettings {
        alt_clan_join_days: player.alt_clan_join_days,
        alt_clan_create_days: player.alt_clan_create_days,
        alt_clan_dissolve_days: player.alt_clan_dissolve_days,
        alt_clan_leader_instant_activation: player.alt_clan_leader_instant_activation,
        royal_guard_cost: feature.royal_guard_cost,
        knight_unit_cost: feature.knight_unit_cost,
        clan_level_costs: [
            feature.clan_level_6_cost,
            feature.clan_level_7_cost,
            feature.clan_level_8_cost,
            feature.clan_level_9_cost,
            feature.clan_level_10_cost,
            feature.clan_level_11_cost,
        ],
        clan_level_requirements: [
            feature.clan_level_6_requirement,
            feature.clan_level_7_requirement,
            feature.clan_level_8_requirement,
            feature.clan_level_9_requirement,
            feature.clan_level_10_requirement,
            feature.clan_level_11_requirement,
        ],
        clan_name_template: Arc::new(move |name: &str| template.is_match(name)),
        life_crystal_needed: player.life_crystal_needed,
        join_academy_min_rep_score: feature.join_academy_min_rep_score,
        join_academy_max_rep_score: feature.join_academy_max_rep_score,
        alt_ally_join_days_when_leaved: player.alt_ally_join_days_when_leaved,
        alt_ally_join_days_when_dismissed: player.alt_ally_join_days_when_dismissed,
        alt_accept_clan_days_when_dismissed: player.alt_accept_clan_days_when_dismissed,
        alt_create_ally_days_when_dissolved: player.alt_create_ally_days_when_dissolved,
        alt_max_num_of_clans_in_ally: player.alt_max_num_of_clans_in_ally,
        alt_clan_members_for_war: player.alt_clan_members_for_war,
        reputation_score_per_kill: feature.reputation_score_per_kill,
    }
}

/// The mail values of `General.ini` (`AllowMail`, `AllowAttachments`); the send interval is the
/// `SendMail` flood protector ([`world_data::flood_settings`]).
pub fn mail_settings(config: &GameConfig, _config_dir: &Path) -> l2_world::MailSettings {
    l2_world::MailSettings {
        allow_mail: config.general.allow_mail,
        allow_attachments: config.general.allow_attachments,
    }
}

/// The residence values of `Player.ini` and `Feature.ini`.
pub fn residence_settings(config: &GameConfig, _config_dir: &Path) -> l2_world::ResidenceSettings {
    let (player, feature) = (&config.player, &config.feature);
    l2_world::ResidenceSettings {
        remove_castle_circlets: player.remove_castle_circlets,
        take_castle_points: feature.take_castle_points,
        loose_castle_points: feature.loose_castle_points,
        castle_defended_points: feature.castle_defended_points,
        take_fort_points: feature.take_fort_points,
        loose_fort_points: feature.loose_fort_points,
        fs_max_supply_level: feature.fs_max_supply_level,
    }
}

/// `ConquerableHallSiege.ini` (Java `ConquerableHallSiegeConfig`) and `NoQuests`.
pub fn hall_siege_settings(config: &GameConfig) -> l2_world::HallSiegeSettings {
    let hall = &config.conquerable_hall_siege;
    l2_world::HallSiegeSettings {
        min_clan_level: hall.chs_clan_minlevel,
        max_attackers: hall.chs_max_attackers,
        max_flags_per_clan: hall.chs_max_flags_per_clan,
        enable_fame: hall.chs_enable_fame,
        fame_amount: hall.chs_fame_amount,
        fame_frequency: hall.chs_fame_frequency,
        no_quests: config.development.no_quests,
    }
}

/// The fortress values of `Feature.ini`, `Player.ini` and `OfflineTrade.ini`, and `FortSiege.ini`
/// (Java `FortSiegeManager.load`: a missing file is logged and the defaults are used).
pub fn fort_settings(config: &GameConfig, config_dir: &Path) -> l2_world::fort::FortSettings {
    let (player, feature) = (&config.player, &config.feature);
    let path = config_dir.join("FortSiege.ini");
    let properties: std::collections::BTreeMap<String, String> = match IniFile::load(&path) {
        Ok(ini) => ini
            .keys()
            .filter_map(|key| ini.get_value(key).map(|value| (key.to_owned(), value.to_owned())))
            .collect(),
        Err(e) => {
            warn!("Error while loading Fort Siege Manager settings! {e}");
            std::collections::BTreeMap::new()
        }
    };
    l2_world::fort::FortSettings {
        fs_tele_fee_ratio: feature.fs_tele_fee_ratio,
        fs_tele1_fee: feature.fs_tele1_fee,
        fs_tele2_fee: feature.fs_tele2_fee,
        fs_support_fee_ratio: feature.fs_support_fee_ratio,
        fs_support1_fee: feature.fs_support1_fee,
        fs_support2_fee: feature.fs_support2_fee,
        fs_mpreg_fee_ratio: feature.fs_mpreg_fee_ratio,
        fs_mpreg1_fee: feature.fs_mpreg1_fee,
        fs_mpreg2_fee: feature.fs_mpreg2_fee,
        fs_hpreg_fee_ratio: feature.fs_hpreg_fee_ratio,
        fs_hpreg1_fee: feature.fs_hpreg1_fee,
        fs_hpreg2_fee: feature.fs_hpreg2_fee,
        fs_expreg_fee_ratio: feature.fs_expreg_fee_ratio,
        fs_expreg1_fee: feature.fs_expreg1_fee,
        fs_expreg2_fee: feature.fs_expreg2_fee,
        fs_update_frq: feature.fs_update_frq,
        fs_blood_oath_count: feature.fs_blood_oath_count,
        fs_fee_for_castle: feature.fs_fee_for_castle,
        fs_max_own_time: feature.fs_max_own_time,
        fame_system_enabled: player.fame_system_enabled,
        fame_for_dead_players: player.fame_for_dead_players,
        offline_fame: config.offline_trade.offline_fame,
        fortress_zone_fame_task_frequency: player.fortress_zone_fame_task_frequency,
        fortress_zone_fame_aquire_points: player.fortress_zone_fame_aquire_points,
        siege: l2_world::fort::FortSiegeIni::from_properties(&properties),
        properties,
    }
}

/// The manor values of `General.ini` (`AllowManor`, `AltManor*`) and `Rates.ini`
/// (`RateDropManor`).
pub fn manor_settings(config: &GameConfig) -> l2_world::ManorSettings {
    let (general, rates) = (&config.general, &config.rates);
    l2_world::ManorSettings {
        allow_manor: general.allow_manor,
        refresh_time: general.alt_manor_refresh_time,
        refresh_min: general.alt_manor_refresh_min,
        approve_time: general.alt_manor_approve_time,
        approve_min: general.alt_manor_approve_min,
        maintenance_min: general.alt_manor_maintenance_min,
        save_all_actions: general.alt_manor_save_all_actions,
        save_period_rate: general.alt_manor_save_period_rate,
        rate_drop_manor: rates.rate_drop_manor,
    }
}

/// The boat values: `AllowBoat` and `BoatBroadcastRadius` of `General.ini` (M8-VEHICLE-01).
pub fn vehicle_settings(config: &GameConfig) -> l2_world::VehicleSettings {
    l2_world::VehicleSettings {
        allow_boat: config.general.allow_boat,
        boat_broadcast_radius: config.general.boat_broadcast_radius,
    }
}

/// The cursed weapon values: `AllowCursedWeapons` of `General.ini`.
pub fn cursed_settings(config: &GameConfig) -> l2_world::CursedSettings {
    l2_world::CursedSettings {
        allow_cursed_weapons: config.general.allow_cursed_weapons,
        clock_override_unix_ms: None,
    }
}

/// The Olympiad values (`Olympiad.ini`, the hand-ported lists of `manual.core.olympiad`).
pub fn olympiad_settings(config: &GameConfig) -> l2_world::OlympiadSettings {
    let o = &config.olympiad;
    let m = &config.manual.core.olympiad;
    l2_world::OlympiadSettings {
        enabled: o.olympiad_enabled,
        start_time: o.olympiad_start_time,
        min: o.olympiad_min,
        max_buffs: o.olympiad_max_buffs,
        comp_period_ms: o.olympiad_cperiod,
        battle_ms: o.olympiad_battle,
        weekly_period_ms: o.olympiad_wperiod,
        validation_period_ms: o.olympiad_vperiod,
        start_points: o.olympiad_start_points,
        weekly_points: o.olympiad_weekly_points,
        classed: o.olympiad_classed,
        non_classed: o.olympiad_nonclassed,
        teams: o.olympiad_teams,
        reg_display: o.olympiad_reg_display,
        classed_reward: m.olympiad_classed_reward.clone(),
        non_classed_reward: m.olympiad_nonclassed_reward.clone(),
        team_reward: m.olympiad_team_reward.clone(),
        comp_reward_item: o.olympiad_comp_ritem,
        min_matches: o.olympiad_min_matches,
        gp_per_point: o.olympiad_gp_per_point,
        hero_points: o.olympiad_hero_points,
        hero_reputation_points: config.feature.hero_points,
        rank_points: [
            o.olympiad_rank1_points,
            o.olympiad_rank2_points,
            o.olympiad_rank3_points,
            o.olympiad_rank4_points,
            o.olympiad_rank5_points,
        ],
        max_points: o.olympiad_max_points,
        divider_classed: o.olympiad_divider_classed,
        divider_non_classed: o.olympiad_divider_non_classed,
        max_weekly_matches: o.olympiad_max_weekly_matches,
        max_weekly_matches_non_classed: o.olympiad_max_weekly_matches_non_classed,
        max_weekly_matches_classed: o.olympiad_max_weekly_matches_classed,
        max_weekly_matches_team: o.olympiad_max_weekly_matches_team,
        show_monthly_winners: o.olympiad_show_monthly_winners,
        announce_games: o.olympiad_announce_games,
        restricted_items: m.list_oly_restricted_items.clone(),
        disable_blessed_spiritshots: o.olympiad_disable_blessed_spiritshots,
        wait_time: o.olympiad_wait_time,
        use_custom_period_settings: o.olympiad_use_custom_period_settings,
        period: o.olympiad_period.clone(),
        period_multiplier: o.olympiad_period_multiplier,
        competition_days: m.olympiad_competition_days.clone(),
        clock_override_unix_ms: None,
        zone_offset_override_ms: None,
    }
}

/// The minigame values (M8-GAMES-01): the Block Checker keys of `General.ini`,
/// `UndergroundColiseum.ini`.
pub fn minigame_settings(config: &GameConfig) -> l2_world::MinigameSettings {
    l2_world::MinigameSettings {
        enable_block_checker_event: config.general.enable_block_checker_event,
        hbce_fair_play: config.general.hbce_fair_play,
        min_block_checker_team_members: config.effective_min_block_checker_team_members(),
        allow_party_in_same_event: config.general.allow_party_in_same_event,
        uc_start_time: config.underground_coliseum.uc_start_time.clone(),
        uc_time_period: config.underground_coliseum.uc_time_period,
        uc_allow_announce: config.underground_coliseum.uc_allow_announce,
        uc_party_size: config.underground_coliseum.uc_party_size,
        uc_ress_time: config.underground_coliseum.uc_ress_time,
        clock_override_unix_ms: None,
    }
}

/// `config/TerritoryWar.ini` (Java `TerritoryWarManager.load`'s `ConfigReader`: a missing file
/// or key means Java's default). The badge prices of the mercenary captain are read here too
/// (M7-TW-01).
pub fn territory_settings(config_dir: &Path) -> l2_world::TerritorySettings {
    const FILE: &str = "TerritoryWar.ini";
    let path = config_dir.join(FILE);
    let ini = match IniFile::load(&path) {
        Ok(ini) => ini,
        Err(e) => {
            warn!(
                "Cannot read {}: {e}; using the Territory War defaults.",
                path.display()
            );
            IniFile::empty(FILE)
        }
    };
    l2_world::TerritorySettings {
        defender_max_clans: ini.get_int("DefenderMaxClans", 500),
        defender_max_players: ini.get_int("DefenderMaxPlayers", 500),
        clan_min_level: ini.get_int("ClanMinLevel", 0),
        player_min_level: ini.get_int("PlayerMinLevel", 40),
        war_length_ms: ini.get_long("WarLength", 120).wrapping_mul(60_000),
        player_with_ward_can_be_killed_in_peace_zone: ini
            .get_bool("PlayerWithWardCanBeKilledInPeaceZone", false),
        spawn_wards_when_tw_is_not_in_progress: ini.get_bool("SpawnWardsWhenTWIsNotInProgress", false),
        return_wards_when_tw_starts: ini.get_bool("ReturnWardsWhenTWStarts", false),
        min_tw_badge_for_nobless: ini.get_int("MinTerritoryBadgeForNobless", 100),
        min_tw_badge_for_striders: ini.get_int("MinTerritoryBadgeForStriders", 50),
        min_tw_badge_for_big_strider: ini.get_int("MinTerritoryBadgeForBigStrider", 80),
    }
}

/// The values of `Feature.ini` the castle NPCs read: the fees of the castle functions, the door
/// and trap upgrade prices and the dawn tickets (M7-NPC-01). The badge prices of the mercenary
/// captain are `TerritoryWar.ini`'s ([`territory_settings`]).
pub fn residence_npc_settings(config: &GameConfig) -> l2_world::residence::npc::ResidenceNpcSettings {
    let f = &config.feature;
    l2_world::residence::npc::ResidenceNpcSettings {
        cs_tele_fee_ratio: f.cs_tele_fee_ratio,
        cs_tele1_fee: f.cs_tele1_fee,
        cs_tele2_fee: f.cs_tele2_fee,
        cs_support_fee_ratio: f.cs_support_fee_ratio,
        cs_support1_fee: f.cs_support1_fee,
        cs_support2_fee: f.cs_support2_fee,
        cs_mpreg_fee_ratio: f.cs_mpreg_fee_ratio,
        cs_mpreg1_fee: f.cs_mpreg1_fee,
        cs_mpreg2_fee: f.cs_mpreg2_fee,
        cs_hpreg_fee_ratio: f.cs_hpreg_fee_ratio,
        cs_hpreg1_fee: f.cs_hpreg1_fee,
        cs_hpreg2_fee: f.cs_hpreg2_fee,
        cs_expreg_fee_ratio: f.cs_expreg_fee_ratio,
        cs_expreg1_fee: f.cs_expreg1_fee,
        cs_expreg2_fee: f.cs_expreg2_fee,
        outer_door_upgrade_price2: f.outer_door_upgrade_price2,
        outer_door_upgrade_price3: f.outer_door_upgrade_price3,
        outer_door_upgrade_price5: f.outer_door_upgrade_price5,
        inner_door_upgrade_price2: f.inner_door_upgrade_price2,
        inner_door_upgrade_price3: f.inner_door_upgrade_price3,
        inner_door_upgrade_price5: f.inner_door_upgrade_price5,
        wall_upgrade_price2: f.wall_upgrade_price2,
        wall_upgrade_price3: f.wall_upgrade_price3,
        wall_upgrade_price5: f.wall_upgrade_price5,
        trap_upgrade_price1: f.trap_upgrade_price1,
        trap_upgrade_price2: f.trap_upgrade_price2,
        trap_upgrade_price3: f.trap_upgrade_price3,
        trap_upgrade_price4: f.trap_upgrade_price4,
        ssq_dawn_ticket_quantity: f.ssq_dawn_ticket_quantity,
        ssq_dawn_ticket_price: f.ssq_dawn_ticket_price,
        ssq_dawn_ticket_bundle: f.ssq_dawn_ticket_bundle,
        ssq_manors_agreement_id: f.ssq_manors_agreement_id,
    }
}

/// The clan hall fee tables of `Feature.ini` and `AltClanHallMpBuffFree` (M7-CH-01).
pub fn clan_hall_settings(config: &GameConfig) -> l2_world::ClanHallSettings {
    use l2_world::clanhall::ClanHallFunctionSettings as Fees;
    let f = &config.feature;
    l2_world::ClanHallSettings {
        tele: Fees::new(f.ch_tele_fee_ratio, [f.ch_tele1_fee, f.ch_tele2_fee]),
        support: Fees::new(
            f.ch_support_fee_ratio,
            [
                f.ch_support1_fee,
                f.ch_support2_fee,
                f.ch_support3_fee,
                f.ch_support4_fee,
                f.ch_support5_fee,
                f.ch_support6_fee,
                f.ch_support7_fee,
                f.ch_support8_fee,
            ],
        ),
        mp: Fees::new(
            f.ch_mpreg_fee_ratio,
            [
                f.ch_mpreg1_fee,
                f.ch_mpreg2_fee,
                f.ch_mpreg3_fee,
                f.ch_mpreg4_fee,
                f.ch_mpreg5_fee,
            ],
        ),
        hp: Fees::new(
            f.ch_hpreg_fee_ratio,
            [
                f.ch_hpreg1_fee,
                f.ch_hpreg2_fee,
                f.ch_hpreg3_fee,
                f.ch_hpreg4_fee,
                f.ch_hpreg5_fee,
                f.ch_hpreg6_fee,
                f.ch_hpreg7_fee,
                f.ch_hpreg8_fee,
                f.ch_hpreg9_fee,
                f.ch_hpreg10_fee,
                f.ch_hpreg11_fee,
                f.ch_hpreg12_fee,
                f.ch_hpreg13_fee,
            ],
        ),
        exp: Fees::new(
            f.ch_expreg_fee_ratio,
            [
                f.ch_expreg1_fee,
                f.ch_expreg2_fee,
                f.ch_expreg3_fee,
                f.ch_expreg4_fee,
                f.ch_expreg5_fee,
                f.ch_expreg6_fee,
                f.ch_expreg7_fee,
            ],
        ),
        item: Fees::new(
            f.ch_item_fee_ratio,
            [f.ch_item1_fee, f.ch_item2_fee, f.ch_item3_fee],
        ),
        curtain: Fees::new(f.ch_curtain_fee_ratio, [f.ch_curtain1_fee, f.ch_curtain2_fee]),
        front: Fees::new(f.ch_front_fee_ratio, [f.ch_front1_fee, f.ch_front2_fee]),
        buff_free: f.ch_buff_free,
    }
}

/// The `Rates.ini` and `General.ini` values the script helpers read.
pub fn script_settings(config: &GameConfig, _config_dir: &Path) -> l2_world::ScriptSettings {
    let rates = &config.rates;
    l2_world::ScriptSettings {
        rate_quest_reward: rates.rate_quest_reward,
        rate_quest_reward_xp: rates.rate_quest_reward_xp,
        rate_quest_reward_sp: rates.rate_quest_reward_sp,
        rate_quest_reward_adena: rates.rate_quest_reward_adena,
        rate_quest_reward_use_multipliers: rates.rate_quest_reward_use_multipliers,
        rate_quest_reward_potion: rates.rate_quest_reward_potion,
        rate_quest_reward_scroll: rates.rate_quest_reward_scroll,
        rate_quest_reward_recipe: rates.rate_quest_reward_recipe,
        rate_quest_reward_material: rates.rate_quest_reward_material,
        quest_item_drop_amount_multiplier: rates.quest_item_drop_amount_multiplier,
        autodelete_invalid_quest_data: config.general.autodelete_invalid_quest_data,
        bloodalliance_points: config.feature.bloodalliance_points,
        bloodoath_points: config.feature.bloodoath_points,
        knightsepaulette_points: config.feature.knightsepaulette_points,
        ballista_points: config.feature.ballista_points,
        fee_delete_transfer_skills: config.player.fee_delete_transfer_skills,
        alt_birthday_gift: config.general.alt_birthday_gift,
        alt_birthday_mail_subject: config.general.alt_birthday_mail_subject.clone(),
        alt_birthday_mail_text: config.manual.core.general.alt_birthday_mail_text.clone(),
        allow_wyvern_always: config.feature.allow_wyvern_always,
        allow_wyvern_during_siege: config.feature.allow_wyvern_during_siege,
        disable_tutorial: config.player.disable_tutorial,
        rate_hb_trust_increase: rates.rate_hb_trust_increase,
        rate_hb_trust_decrease: rates.rate_hb_trust_decrease,
        rate_death_drop_amount_multiplier: rates.rate_death_drop_amount_multiplier,
        hellbound_without_quest: config.general.hellbound_without_quest,
        gracia_seeds: gracia_seeds_settings(config),
    }
}

/// The `GraciaSeeds.ini` values of the seed managers and the Gracia instances (S-AI-07).
pub fn gracia_seeds_settings(config: &GameConfig) -> l2_world::script::GraciaSeedsSettings {
    let g = &config.gracia_seeds;
    l2_world::script::GraciaSeedsSettings {
        sod_tiat_kill_count: g.sod_tiat_kill_count,
        sod_stage_2_length: config.manual.core.gracia_seeds.sod_stage_2_length,
        min_tiat_players: g.min_tiat_players,
        max_tiat_players: g.max_tiat_players,
        min_tiat_level: g.min_tiat_level,
        soi_ekimus_kill_count: g.soi_ekimus_kill_count,
        erosion_attack_min_players: g.erosion_attack_min_players,
        erosion_attack_max_players: g.erosion_attack_max_players,
        erosion_defence_min_players: g.erosion_defence_min_players,
        erosion_defence_max_players: g.erosion_defence_max_players,
        heart_attack_min_players: g.heart_attack_min_players,
        heart_attack_max_players: g.heart_attack_max_players,
        heart_defence_min_players: g.heart_defence_min_players,
        heart_defence_max_players: g.heart_defence_max_players,
    }
}

/// The Seven Signs values of `Feature.ini` and `Npc.ini`.
pub fn seven_signs_settings(
    config: &GameConfig,
    _config_dir: &Path,
) -> l2_world::sevensigns::SevenSignsSettings {
    let f = &config.feature;
    l2_world::sevensigns::SevenSignsSettings {
        lazy_update: f.alt_sevensigns_lazy_update,
        max_player_contrib: f.alt_maximum_player_contrib,
        strict: f.alt_strict_sevensigns,
        open_catacombs: f.alt_sevensigns_open_catacumbs,
        open_necropolis: f.alt_sevensigns_open_necropolis,
        castle_for_dawn: f.alt_game_castle_dawn,
        castle_for_dusk: f.alt_game_castle_dusk,
        join_dawn_fee: f.ssq_join_dawn_adena_fee,
        manors_agreement_id: f.ssq_manors_agreement_id,
        announce_mammon_spawn: config.npc.announce_mammon_spawn,
        festival: l2_world::sevensigns::FestivalSettings {
            min_player: f.alt_festival_min_player,
            manager_start: f.alt_festival_manager_start,
            length: f.alt_festival_length,
            cycle_length: f.alt_festival_cycle_length,
            first_spawn: f.alt_festival_first_spawn,
            first_swarm: f.alt_festival_first_swarm,
            second_spawn: f.alt_festival_second_spawn,
            second_swarm: f.alt_festival_second_swarm,
            chest_spawn: f.alt_festival_chest_spawn,
            win_points: f.festival_win_points,
        },
    }
}

/// The Dimensional Rift values of `General.ini` (M8-FEST-01).
pub fn rift_settings(config: &GameConfig) -> l2_world::sevensigns::RiftSettings {
    let g = &config.general;
    l2_world::sevensigns::RiftSettings {
        min_party_size: g.rift_min_party_size,
        max_jumps: g.rift_max_jumps,
        spawn_delay: g.rift_spawn_delay,
        auto_jumps_time_min: g.rift_auto_jumps_time_min,
        auto_jumps_time_max: g.rift_auto_jumps_time_max,
        boss_room_time_multiply: g.rift_boss_room_time_mutiply,
        enter_cost: [
            g.rift_enter_cost_recruit,
            g.rift_enter_cost_soldier,
            g.rift_enter_cost_officer,
            g.rift_enter_cost_captain,
            g.rift_enter_cost_commander,
            g.rift_enter_cost_hero,
        ],
    }
}

/// `OfflineTrade.ini` (Java `OfflineTradeConfig`). `OfflineNameColor` is `Integer.decode("0x" + value)`; a value that does not
/// read as hex falls back to the default with a warning (Java would not start).
pub fn offline_settings(config: &GameConfig, config_dir: &Path) -> l2_world::OfflineSettings {
    let t = &config.offline_trade;
    let defaults = l2_world::OfflineSettings::default();
    let name_color = IniFile::load(config_dir.join(l2_config::game::OfflineTradeConfig::FILE))
        .map(|ini| ini.get_string("OfflineNameColor", "808080"))
        .unwrap_or_else(|_| "808080".to_owned());
    let name_color =
        i32::from_str_radix(name_color.trim().trim_start_matches("0x"), 16).unwrap_or_else(|_| {
            warn!("OfflineNameColor {name_color:?} is not a hexadecimal colour; using 808080.");
            defaults.offline_name_color
        });
    l2_world::OfflineSettings {
        offline_trade_enable: t.offline_trade_enable,
        offline_craft_enable: t.offline_craft_enable,
        offline_mode_in_peace_zone: t.offline_mode_in_peace_zone,
        offline_mode_no_damage: t.offline_mode_no_damage,
        offline_set_name_color: t.offline_set_name_color,
        offline_name_color: name_color,
        restore_offliners: t.restore_offliners,
        offline_max_days: t.offline_max_days,
        offline_disconnect_finished: t.offline_disconnect_finished,
        store_offline_trade_in_realtime: t.store_offline_trade_in_realtime,
    }
}

/// File names inside the config directory.
pub const DATABASE_CONFIG_FILE: &str = "Database.ini";
pub const INTERFACE_CONFIG_FILE: &str = "Interface.ini";
pub const SERVER_CONFIG_FILE: &str = "Server.ini";

/// Where the server reads and writes. Relative paths are relative to `root`, which plays the
/// role of Java's working directory (the `dist/game` folder).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    pub root: PathBuf,
    /// `*.ini` files and `hexid.txt` (Java `./config`).
    pub config_dir: PathBuf,
    /// Log files (Java `./log`).
    pub log_dir: PathBuf,
}

impl Paths {
    pub fn resolve(root: &Path, config_dir: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            config_dir: root.join(config_dir),
            log_dir: root.join(logging::LOG_DIR_NAME),
        }
    }
}

/// A startup failure. The process exits with code 1.
#[derive(Debug, thiserror::Error)]
pub enum StartupError {
    #[error("cannot load the configuration: {0}")]
    Config(#[from] ConfigError),
    #[error("database error: {0}")]
    Database(#[from] DbError),
    #[error("object ids: {0}")]
    Ids(#[from] IdError),
    #[error("cannot load the datapack: {0}")]
    Datapack(#[from] DataError),
    #[error("cannot start the world thread: {0}")]
    WorldThread(std::io::Error),
    /// A listener could not be opened.
    #[error("{what} listener could not be started on {address}: {source}")]
    Listen {
        what: &'static str,
        address: String,
        source: std::io::Error,
    },
    #[error("internal error: {0}")]
    Internal(String),
}

/// A running game server (initialized, background tasks started).
pub struct Server {
    state: Arc<GameState>,
    shutdown_requests: mpsc::UnboundedReceiver<ExitReason>,
    /// The world thread and the consumer of its outbox (the persistence task).
    world: WorldRuntime,
    /// What the login server link sends next: `PlayerInGame`, and `PlayerLogout` once the
    /// character is stored. Taken once by the link (M2-GS-02).
    ls_notices: Option<mpsc::UnboundedReceiver<LsNotice>>,
    /// Background and listener tasks. All are stopped on shutdown.
    tasks: JoinSet<()>,
    /// Cancelled on shutdown; listeners and their connections watch it (child tokens).
    cancel: CancellationToken,
    /// Where the client listener is bound (useful with port 0 in tests).
    client_addr: Option<SocketAddr>,
    /// Where the admin endpoint is bound, when enabled.
    admin_addr: Option<SocketAddr>,
    /// The link to the login server (M2-GS-02).
    ls_link: Option<ls_link::LsLink>,
}

impl Server {
    /// Runs the startup steps. On success the server is initialized and listening.
    pub async fn start(paths: &Paths) -> Result<Self, StartupError> {
        let config = GameConfig::load(&paths.config_dir)?;
        let database = DatabaseConfig::from_ini(&IniFile::load(paths.config_dir.join(DATABASE_CONFIG_FILE))?);
        let interface =
            InterfaceConfig::from_ini(&IniFile::load(paths.config_dir.join(INTERFACE_CONFIG_FILE))?);
        info!("Configuration loaded from {}.", paths.config_dir.display());
        if interface.enable_gui {
            // The Java default is true, so this is normal; it only tells that nothing opens.
            info!("EnableGUI is set, but the Rust game server is headless; it is ignored.");
        }
        if database.backup_database {
            warn!("BackupDatabase is set, but database backups on shutdown are not supported yet.");
        }

        let pool = l2_db::connect(&database).await?;
        Self::start_with(config, pool, paths).await
    }

    /// Everything after the database connection.
    async fn start_with(config: GameConfig, pool: Pool, paths: &Paths) -> Result<Self, StartupError> {
        let used = IdRepository::new(pool.clone())
            .used_object_ids(RUST_FIRST_OBJECT_ID, RUST_LAST_OBJECT_ID)
            .await?;
        let ids = Arc::new(IdFactory::new(used.ids)?);
        info!(
            "IdManager: Free ObjectID's remaining: {} ({} ids of other ranges are left to the Java game server).",
            ids.available_count(),
            used.outside_range
        );

        let datapack_root = resolve_datapack_root(&paths.root, &config.manual.core.server.datapack_root);
        let datapack = {
            let root = datapack_root.clone();
            let options = options_from_config(&config, &paths.config_dir);
            tokio::task::spawn_blocking(move || Datapack::load_with(&root, &options))
                .await
                .map_err(|e| StartupError::Internal(format!("datapack loader: {e}")))??
        };
        let datapack = Arc::new(datapack);
        // The stat engine tables (statBonus.xml and the datapack parts, skills included) and the
        // access levels (see `world_data`), and the config values the world reads.
        let static_data: Arc<dyn StaticData> = {
            let (datapack, config_dir) = (Arc::clone(&datapack), paths.config_dir.clone());
            Arc::new(
                tokio::task::spawn_blocking(move || DatapackData::load(datapack, &config_dir))
                    .await
                    .map_err(|e| StartupError::Internal(format!("data loader: {e}")))?,
            )
        };
        let world_settings = world_data::world_settings(&config, &paths.config_dir);
        let regen_settings = world_data::regen_settings(&config);
        let summon_settings = world_data::summon_settings(&config);
        let pc_cafe_settings = world_data::pc_cafe_settings(&config);
        let custom_play = custom_play::custom_play_settings(&config, &paths.config_dir);
        let pet_settings = world_data::pet_settings(&config, &paths.config_dir);
        let instance_settings = world_data::instance_settings(&config, &paths.config_dir);
        let pet_names = persist::load_pet_names(&pool).await;
        // `itemsonground` only with SaveDroppedItem / ClearDroppedItemTable on, like Java.
        let ground_items = persist::restore_ground_items(&pool, &config.general).await;
        // names the punishment commands look up).
        let punishments = persist::load_punishments(&pool).await;
        let char_names = persist::load_char_names(&pool).await;
        let punishment_settings = world_data::punishment_settings(&config, &paths.config_dir);
        // server control commands, the petitions and `//debug` read (M7-ADMIN-10).
        let announcements = persist::load_announcements(&pool).await;
        let admin_settings = world_data::admin_settings(&config, &paths.config_dir);
        // `fishing_championship` and `fishChampionshipEnd`, read only.
        let fishing_championship = if config.general.alt_fish_championship_enabled {
            Some(persist::load_fishing_championship(&pool).await)
        } else {
            None
        };
        // here; the scripts write them (S-API-01).
        let global_variables = persist::load_global_variables(&pool).await;
        let buffer_schemes = persist::load_buffer_schemes(&pool).await;
        let script_settings = script_settings(&config, &paths.config_dir);
        // the world writes it (S-HAND-03).
        let lottery_settings = world_data::lottery_settings(&config, &paths.config_dir);
        let lottery_row = persist::load_lottery(&pool).await;
        // `mods_wedding`, read only here.
        let wedding_settings = world_data::wedding_settings(&config);
        let couples = if wedding_settings.allow_wedding {
            persist::load_couples(&pool).await
        } else {
            Vec::new()
        };
        // (M8-BOSS-01): the boss tables, read only.
        let boss_settings = world_data::boss_settings(&config, &paths.config_dir);
        let boss_load = persist::load_bosses(&pool).await;
        let raid_boss_rows = persist::load_raid_boss_rows(&pool).await;
        let (clans, clan_wars) = persist::load_clans(&pool).await;
        let mail = persist::load_mail(&pool).await;
        let mail_settings = mail_settings(&config, &paths.config_dir);
        // the reports of the table, read only.
        let bot_report_settings = world_data::bot_report_settings(&config, &paths.config_dir);
        let bot_reports = if bot_report_settings.enable {
            persist::load_bot_reports(&pool).await
        } else {
            Vec::new()
        };
        // RESTORE_OFFLINERS): the shops of the
        // tables, with their characters, read only.
        let offline_settings = offline_settings(&config, &paths.config_dir);
        let offline_traders = if offline_settings.restore_wanted() {
            persist::load_offline_traders(&pool, &offline_settings, l2_util::time::now_millis()).await
        } else {
            Vec::new()
        };
        // RestoreAutoPlayOffliners), read only.
        let restore_offline_play = custom_play.offline_play.restore_wanted();
        let (offline_players, offline_play_groups) = if restore_offline_play {
            persist::load_offline_players(&pool).await
        } else {
            (Vec::new(), Vec::new())
        };
        let flood_settings = world_data::flood_settings_from_config(&config);
        let community_board_settings = world_data::community_board_settings(&config);
        let custom_board_settings = world_data::custom_board_settings(&config, &paths.config_dir);
        let scheme_buffer_settings = world_data::scheme_buffer_settings(&config);
        let sell_buffs_settings = world_data::sell_buffs_settings(&config);
        // SendMessageLocalisationData, NpcNameLocalisationData (M9-L02). Nothing is read with the
        // switch off.
        let lang_data = if config.multilingual_support.multilang_enable {
            l2_data::load_lang_data(&datapack.root, &config.custom_misc().multilang_allowed)
        } else {
            l2_data::LangData::default()
        };
        let user_command_info_settings = l2_world::UserCommandInfoSettings {
            display_server_time: config.server_time.display_server_time,
        };
        let script_handler_settings = l2_world::ScriptHandlerSettings {
            unstuck_interval: config.player.unstuck_interval,
            allow_wear: config.general.allow_wear,
            allow_rentpet: config.general.allow_rentpet,
            enable_find_pvp: config.find_pvp.enable_find_pvp,
            alt_game_viewnpc: config.npc.alt_game_viewnpc,
        };
        let crests = persist::load_crests(&pool).await;
        let clan_settings = clan_settings(&config, &paths.config_dir);
        let residences = persist::load_residences(&pool).await;
        let item_auction_settings = item_auctions::item_auction_settings(&config, &paths.config_dir);
        let item_auction_load =
            item_auctions::load_item_auctions(&pool, &item_auction_settings, &datapack_root, |item_id| {
                datapack.items.get(item_id).is_some()
            })
            .await;
        let clan_hall_auctions = persist::load_clan_hall_auctions(&pool).await;
        let clan_hall_settings = clan_hall_settings(&config);
        let residence_npc_settings = residence_npc_settings(&config);
        let residence_settings = residence_settings(&config, &paths.config_dir);
        let sieges = persist::load_sieges(&pool).await;
        let castle_names: Vec<(i32, String)> = residences
            .castles
            .iter()
            .map(|c| (c.id, c.name.clone()))
            .collect();
        let siege_settings = world_data::siege_settings(&config, &paths.config_dir, &castle_names);
        // fortsiege_clans; M7-FORT-01) and FortSiegeManager (FortSiege.ini).
        let fort_sieges = persist::load_fort_sieges(&pool).await;
        let fort_settings = fort_settings(&config, &paths.config_dir);
        // territory_registrations, the nextTWStartDate global variable; M7-TW-01).
        let territory_load = persist::load_territories(&pool).await;
        let territory_settings = territory_settings(&paths.config_dir);
        // `rainbowsprings_attacker_list`) and ConquerableHallSiege.ini (M7-CHSIEGE-01).
        let hall_sieges = persist::load_hall_sieges(&pool).await;
        let hall_siege_settings = hall_siege_settings(&config);
        let manor_settings = manor_settings(&config);
        let manor_load = if manor_settings.allow_manor {
            persist::load_manor(&pool).await
        } else {
            l2_world::ManorLoad::default()
        };
        // row, read only here) and AutoSpawnHandler (`random_spawn`, read only; M8-SS-01).
        let seven_signs_settings = seven_signs_settings(&config, &paths.config_dir);
        let seven_signs_load = persist::load_seven_signs(&pool).await;
        let auto_spawns = persist::load_auto_spawns(&pool).await;
        // data/DimensionalRift.xml; M8-FEST-01).
        let rift_settings = rift_settings(&config);
        let rift_rooms = persist::load_dimensional_rift_rooms(&pool).await;
        let rift_spawns = l2_data::RiftSpawns::load(&datapack_root);
        // `controlPlayers`).
        let cursed_settings = cursed_settings(&config);
        let cursed_load = if cursed_settings.allow_cursed_weapons {
            persist::load_cursed(&pool, &datapack.cursed_weapons).await
        } else {
            l2_world::CursedLoad::default()
        };
        // read here (M8-OLY-01).
        let olympiad_settings = olympiad_settings(&config);
        let olympiad_load = if olympiad_settings.enabled {
            persist::load_olympiad(&pool, olympiad_settings.min_matches).await
        } else {
            l2_world::OlympiadLoad::default()
        };
        // diary and the match history of the heroes, read here (M8-HERO-01).
        let hero_load = if olympiad_settings.enabled {
            let before = l2_world::hero_fights_cutoff_ms(
                l2_util::time::now_millis(),
                &l2_util::time::local_offset_millis,
            );
            persist::load_heroes(&pool, before).await
        } else {
            l2_world::HeroLoad::default()
        };
        // restoreBestTeams. Read always: they are only read here.
        let minigame_settings = minigame_settings(&config);
        let minigame_load = persist::load_minigames(&pool).await;
        let vehicle_load = persist::load_airships(&pool).await;
        // MerchantPriceConfigTable.
        let (buy_lists, price_configs) = {
            let datapack = Arc::clone(&datapack);
            let options = l2_data::BuyListOptions {
                custom_buylist_load: config.general.custom_buylist_load,
                correct_prices: config.general.correct_prices,
            };
            tokio::task::spawn_blocking(move || {
                (
                    Arc::new(l2_data::BuyLists::load(&datapack.root, &datapack.items, &options)),
                    Arc::new(l2_data::MerchantPriceConfigs::load(&datapack.root)),
                )
            })
            .await
            .map_err(|e| StartupError::Internal(format!("buy list loader: {e}")))?
        };
        let buy_list_stock = persist::load_buy_list_stock(&pool).await;

        // world thread gets the engine. Not fatal: Java runs without geodata too.
        let geo_settings = geo::GeoSettings::load(&config, &paths.config_dir, &datapack_root);
        let geo_engine = {
            let settings = geo_settings.clone();
            tokio::task::spawn_blocking(move || geo::load_engine(&settings))
                .await
                .map_err(|e| StartupError::Internal(format!("geodata loader: {e}")))?
        };

        let hexid = hexid::load(&paths.config_dir);

        // task; it stores in order and reports logouts (after the store) on `ls_notices`.
        let autosave = persist::autosave_interval_from_config(&config);
        let (notices_tx, ls_notices) = mpsc::unbounded_channel();
        let left_acks = persist::LeftAcks::default();
        let world = {
            let acks = left_acks.clone();
            let store = DbStore::new(pool.clone());
            let (data, settings) = (Arc::clone(&static_data), world_settings.clone());
            let geo_engine = Arc::clone(&geo_engine);
            // `//path_find` and `//zone_build_save` (admin/geodata.rs, admin/zone_tools.rs).
            let (path_options, zone_build_dir) =
                (geo_settings.path.clone(), paths.root.join("data").join("zones"));
            let (zones, map_regions) = (Arc::clone(&datapack.zones), Arc::clone(&datapack.map_regions));
            let (html, teleporters) = (Arc::clone(&datapack.html), Arc::clone(&datapack.teleporters));
            let hit_condition_bonus = Arc::clone(&datapack.hit_condition_bonus);
            let exp_tables = (
                Arc::clone(&datapack.experience),
                Arc::clone(&datapack.experience_loss),
                Arc::clone(&datapack.karma_loss),
                Arc::clone(&datapack.skill_trees),
            );
            let skill_learn = Arc::clone(&datapack.skill_learn);
            let (scheme_buffer_skills, sell_buff_data) = (
                Arc::clone(&datapack.scheme_buffer_skills),
                Arc::clone(&datapack.sell_buff_data),
            );
            let pet_skills = Arc::clone(&datapack.pet_skills);
            let pets = Arc::clone(&datapack.pets);
            let cubics = Arc::clone(&datapack.cubics);
            let recipes = Arc::clone(&datapack.recipes);
            let fishing = (
                Arc::clone(&datapack.fish),
                Arc::clone(&datapack.fishing_rods),
                Arc::clone(&datapack.fishing_monsters),
            );
            let class_list = Arc::clone(&datapack.class_list);
            let multisell = (
                Arc::clone(&datapack.multisell),
                world_data::multisell_settings(&config),
            );
            let enchant_data = (
                Arc::clone(&datapack.enchant_items),
                Arc::clone(&datapack.enchant_groups),
                Arc::clone(&datapack.elemental_attributes),
            );
            let npc_start = (
                Arc::clone(&datapack),
                Arc::clone(&ids),
                config.clone(),
                paths.config_dir.clone(),
            );
            WorldRuntime::start_configured(
                WorldRng::from_entropy(),
                autosave,
                move |world| {
                    world.set_static_data(Box::new(data));
                    world.set_settings(settings);
                    world.start_custom_misc();
                    world.set_regen_settings(regen_settings);
                    world.set_summon_settings(summon_settings);
                    world.set_pc_cafe_settings(pc_cafe_settings);
                    custom_play.install(world);
                    world.set_geo(geo_engine);
                    world.set_admin_path_options(path_options);
                    world.set_zone_build_dir(zone_build_dir);
                    // Zones (flags, compass icon) and the restart points of the map regions.
                    world.set_zone_data(zones, map_regions);
                    // The pages of NPC dialogs and the gatekeeper lists (always preloaded).
                    world.set_html_cache(html);
                    world.set_teleporter_data(teleporters);
                    world.set_hit_condition_bonus(hit_condition_bonus);
                    let (experience, experience_loss, karma_loss, skill_trees) = exp_tables;
                    world.set_exp_data(experience, experience_loss, karma_loss, skill_trees);
                    world.set_skill_learn_data(skill_learn);
                    world.set_pet_skill_data(pet_skills);
                    world.set_pet_data(pets);
                    world.set_pet_settings(pet_settings);
                    world.load_pet_names(pet_names);
                    world.set_cubic_data(cubics);
                    world.set_recipe_data(recipes);
                    let (fish, fishing_rods, fishing_monsters) = fishing;
                    world.set_fishing_data(fish, fishing_rods, fishing_monsters);
                    world.set_class_list(class_list);
                    let (multisell_data, multisell_settings) = multisell;
                    world.set_multisell_data(multisell_data, multisell_settings);
                    let (enchant_items, enchant_groups, elemental_attributes) = enchant_data;
                    world.set_enchant_data(enchant_items, enchant_groups, elemental_attributes);
                    let (datapack, ids, config, config_dir) = npc_start;
                    world.set_augmentation_data(datapack.augmentation_data.clone());
                    // spawns.
                    if let Err(e) = world.spawn_door_data(&datapack, &ids) {
                        tracing::error!("Doors, fences and static objects could not all be spawned: {e}");
                    }
                    world.set_instance_settings(instance_settings);
                    world.set_instance_data(Arc::clone(&datapack.instances));
                    // `isTargeted` and starts its route).
                    world.set_walking_routes(Arc::clone(&datapack.routes));
                    // skipped with `NoQuests`) comes before `SpawnData` (S-API-01).
                    // `initZones`, after the zones), both before the "Scripts" section: the grand
                    // boss scripts read the states when they load (M8-BOSS-01, S-BOSS-01).
                    world.set_boss_settings(boss_settings);
                    world.load_bosses(boss_load);
                    world.set_script_settings(script_settings);
                    world.set_global_variables(global_variables);
                    // The scripts that Java creates only with a config key (`Script::enabled`) read
                    // these two sets of settings when they are installed; they are set again in
                    // their own sections below (S-API-04).
                    world.set_olympiad_settings(olympiad_settings.clone());
                    world.set_minigame_settings(minigame_settings.clone());
                    world.set_category_data(Arc::clone(&datapack.categories));
                    // its monsters from it; the chance multiplier is applied at load, S-API-05).
                    world.set_level_up_crystal_data(Arc::new(l2_data::LevelUpCrystalData::load(
                        &datapack.root,
                        config.player.soul_crystal_chance_multiplier,
                    )));
                    let scripts = (!config.development.no_quests)
                        .then(|| l2_scripts::all_with(l2_data::ClassMasterConfig::load(&datapack.root)));
                    npcs::spawn_npcs_with_scripts(world, &datapack, ids, &config, &config_dir, scripts);
                    if !config.development.no_spawns {
                        world.spawn_raid_bosses(raid_boss_rows);
                    }
                    world.load_ground_items(ground_items);
                    world.start_daily_reset_task();
                    world.set_punishment_settings(punishment_settings);
                    world.load_char_names(char_names);
                    world.load_punishments(punishments);
                    // values of `//server_*` and the petition limits (M7-ADMIN-10).
                    world.set_admin_settings(admin_settings);
                    world.load_announcements(announcements);
                    world.set_clan_settings(clan_settings);
                    world.load_clans(clans, clan_wars);
                    // FortManager, SiegeScheduleData (M7-RES-01).
                    world.set_residence_settings(residence_settings);
                    world.set_fort_settings(fort_settings);
                    world.set_siege_schedule(Arc::clone(&datapack.siege_schedule));
                    world.load_residences(residences);
                    // (M7-SIEGE-01).
                    world.set_siege_settings(siege_settings);
                    world.load_sieges(sieges);
                    // (M7-FORT-01).
                    world.load_fort_data(fort_sieges);
                    // and MercenaryCaptain constructors (the next war date, the shouts;
                    // M7-TW-01).
                    world.set_territory_settings(territory_settings);
                    world.load_territories(territory_load);
                    // residences (M7-CHSIEGE-01).
                    world.set_hall_siege_settings(hall_siege_settings);
                    world.load_hall_sieges(hall_sieges);
                    // castles; M7-MANOR-01).
                    world.set_manor_settings(manor_settings);
                    world.set_manor_seeds(Arc::clone(&datapack.seeds));
                    world.load_manor(manor_load);
                    // the sieges, spawnSevenSignsNPC with AutoSpawnHandler (M8-SS-01).
                    world.set_seven_signs_settings(seven_signs_settings);
                    world.load_seven_signs(seven_signs_load);
                    world.load_auto_spawns(auto_spawns);
                    world.spawn_seven_signs_npc();
                    world.set_rift_settings(rift_settings);
                    world.load_dimensional_rift(rift_rooms, &rift_spawns);
                    // M8-CURSED-01).
                    world.set_cursed_settings(cursed_settings);
                    world.load_cursed_weapons(&datapack.cursed_weapons, cursed_load);
                    world.set_olympiad_settings(olympiad_settings);
                    world.load_olympiad(olympiad_load);
                    world.load_heroes(hero_load);
                    // spawns and the doors) and the Block Checker queue (M8-GAMES-01).
                    world.set_minigame_settings(minigame_settings);
                    world.set_minigame_data(Arc::clone(&datapack.minigames));
                    world.start_minigames(minigame_load);
                    world.set_vehicle_settings(vehicle_settings(&config));
                    world.load_airships(vehicle_load);
                    world.spawn_boats();
                    world.start_air_ships();
                    world.set_clan_hall_settings(clan_hall_settings);
                    // dawn tickets; M7-NPC-01).
                    world.set_residence_npc_settings(residence_npc_settings);
                    world.load_clan_hall_auctions(clan_hall_auctions);
                    // item templates and the id factory).
                    world.set_item_auction_settings(item_auction_settings);
                    world.load_item_auctions(item_auction_load);
                    world.set_flood_settings(flood_settings);
                    world.set_community_board_settings(community_board_settings);
                    world.set_custom_board_settings(custom_board_settings);
                    world.set_scheme_buffer(scheme_buffer_settings, scheme_buffer_skills);
                    world.load_buffer_schemes(buffer_schemes);
                    world.set_sell_buffs(sell_buffs_settings, sell_buff_data);
                    world.set_lang_texts(&lang_data);
                    world.set_user_command_info_settings(user_command_info_settings);
                    world.set_script_handler_settings(script_handler_settings);
                    world.set_mail_settings(mail_settings);
                    world.load_mail(mail);
                    let bot_report_on = bot_report_settings.enable;
                    world.set_bot_report(bot_report_settings);
                    if bot_report_on {
                        world.load_bot_reports(bot_reports);
                        world.start_bot_report_reset_task();
                    }
                    world.set_buy_lists(buy_lists, price_configs);
                    world.load_buy_list_stock(buy_list_stock);
                    world.load_crests(crests);
                    world.set_lottery_settings(lottery_settings);
                    world.load_lottery(lottery_row);
                    let wedding_on = wedding_settings.allow_wedding;
                    world.set_wedding_settings(wedding_settings);
                    if wedding_on {
                        world.load_couples(couples);
                    }
                    if let Some(data) = fishing_championship {
                        world.load_fishing_championship(data);
                    }
                    // spawns and the clans, so the traders find their surroundings).
                    let restore_traders = offline_settings.restore_wanted();
                    world.set_offline_settings(offline_settings);
                    if restore_traders {
                        world.restore_offline_traders(offline_traders);
                    }
                    if restore_offline_play {
                        world.restore_offline_players(offline_players, offline_play_groups);
                    }
                },
                move |outbox| persist::spawn(store, outbox, notices_tx, RetryPolicy::default(), acks),
            )
            .map_err(StartupError::WorldThread)?
        };

        // The persistence task answers the reads the world asked for (the pet rows of a collar)
        // with a command on the world's channel.
        left_acks.set_world(world.handle());

        // The requests of the world that are not its to do (M7-ADMIN-10): the end of a GM
        // shutdown countdown (Java `System.exit`) and `//reload html`, whose files are read here,
        // never inside a tick.
        let (shutdown, shutdown_requests) = ShutdownHandle::channel();
        {
            let (stop, acks) = (shutdown.clone(), left_acks.clone());
            let root = datapack_root.clone();
            let (lazy, check_encoding) = (!config.general.htm_cache, config.general.check_html_encoding);
            // `//reload fakeplayerchat` (`Custom/FakePlayers.ini`).
            let read_fake_player_chat =
                config.fake_players.fake_players_enabled && config.fake_players.fake_player_chat;
            // `//respawnall` reads the spawn files again with the same options as the start.
            let (npcs, spawn_options, spawn_root) = {
                let options = options_from_config(&config, &paths.config_dir);
                (
                    Arc::clone(&datapack.npcs),
                    l2_data::SpawnLoadOptions {
                        fake_players_enabled: options.fake_players_enabled,
                        no_spawns: options.no_spawns,
                    },
                    // `SpawnData.addNewSpawn` writes below the server's own folder.
                    paths.root.join("data/spawns/Others"),
                )
            };
            left_acks.set_control(Arc::new(move |request| match request {
                l2_world::ControlRequest::Shutdown { restart } => stop.request(restart),
                l2_world::ControlRequest::ReloadSpawns => {
                    let (acks, root, npcs) = (acks.clone(), root.clone(), Arc::clone(&npcs));
                    let options = spawn_options.clone();
                    tokio::task::spawn_blocking(move || {
                        let spawns = Arc::new(l2_data::SpawnData::load(&root, &npcs, &options));
                        if !acks.send_to_world(WorldCommand::SpawnsReloaded { spawns }) {
                            warn!("The reloaded spawns could not be handed to the world.");
                        }
                    });
                }
                l2_world::ControlRequest::ReloadFakePlayerChat { requester } => {
                    let (acks, root) = (acks.clone(), root.clone());
                    tokio::task::spawn_blocking(move || {
                        // The file is read only while the chat is on (Java `FakePlayerChatManager.load`).
                        let chat = if read_fake_player_chat {
                            npcs::fake_player_chat_entries(&root)
                        } else {
                            Vec::new()
                        };
                        if !acks.send_to_world(WorldCommand::FakePlayerChatReloaded { requester, chat }) {
                            warn!("The reloaded fake player chat could not be handed to the world.");
                        }
                    });
                }
                l2_world::ControlRequest::StoreSpawn { spawn } => {
                    let dir = spawn_root.clone();
                    tokio::task::spawn_blocking(move || {
                        match l2_data::spawn_store::add_new_spawn(&dir, &spawn) {
                            Ok(file) => info!("SpawnData: Stored spawn in {}", file.display()),
                            Err(error) => warn!(
                                "SpawnData: Spawn of NPC {} could not be added to the spawn XML files: {error}",
                                spawn.npc_id
                            ),
                        }
                    });
                }
                l2_world::ControlRequest::LoadHtmlFile { requester, path } => {
                    let (acks, root) = (acks.clone(), root.clone());
                    tokio::task::spawn_blocking(move || {
                        let content = l2_data::HtmCache::read_file(&root, &path);
                        let answer = WorldCommand::HtmlFileLoaded {
                            requester,
                            path,
                            content,
                        };
                        if !acks.send_to_world(answer) {
                            warn!("The html file could not be handed to the world.");
                        }
                    });
                }
                l2_world::ControlRequest::ReloadHtml { requester, path } => {
                    let (acks, root) = (acks.clone(), root.clone());
                    tokio::task::spawn_blocking(move || {
                        let result = reload_html(&root, path.as_deref(), lazy, check_encoding);
                        let answer = WorldCommand::HtmlReloaded {
                            requester,
                            path,
                            result,
                        };
                        if !acks.send_to_world(answer) {
                            warn!("The html reload could not be handed to the world.");
                        }
                    });
                }
            }));
        }

        // D-14: the path workers answer with `WorldCommand::PathFound` on the world's channel.
        let geo = geo::Geo::start(
            geo_engine,
            &geo_settings,
            Arc::new(geo::WorldPathSink::new(world.handle())),
        );

        // The world asks the pool for paths through this (nothing to install when it is off: a
        // move then goes straight to its destination, like Java with `PathFinding = 0`).
        if geo.paths.is_enabled() {
            let requests = geo::WorldPathRequests::new(Arc::clone(&geo.paths));
            if !world
                .handle()
                .send(WorldCommand::SetPathRequests(Box::new(requests)))
            {
                warn!("The world stopped before it got the path service.");
            }
        }

        let max_players = config.server.maximum_online_users;
        let state = Arc::new(GameState {
            config: Arc::new(config),
            paths: paths.clone(),
            pool,
            ids,
            datapack,
            static_data,
            world_settings,
            geo,
            world: world.handle(),
            left_acks,
            hexid,
            shutdown,
        });

        let mut server = Self {
            state,
            shutdown_requests,
            world,
            ls_notices: Some(ls_notices),
            tasks: JoinSet::new(),
            cancel: CancellationToken::new(),
            client_addr: None,
            admin_addr: None,
            ls_link: None,
        };
        // The login server link first: the client sessions authenticate through it. (Java starts
        // `LoginServerThread` after the listener; nothing can log in before the link is up
        // either way.)
        let link = ls_link::start(&mut server);
        // Java `LoginServerThread._serverName`: the name the login server gave this server, which
        // `//server_login` shows (M7-ADMIN-10).
        {
            let (mut status, world) = (link.subscribe(), server.state().world.clone());
            server.spawn(async move {
                let mut last = String::new();
                loop {
                    let name = status.borrow_and_update().server_name.clone();
                    if !name.is_empty() && name != last {
                        world.send(WorldCommand::LoginServerRegistered { name: name.clone() });
                        last = name;
                    }
                    if status.changed().await.is_err() {
                        break;
                    }
                }
            });
        }
        if let Some(mut notices) = server.take_ls_notices() {
            // PlayerInGame / PlayerLogout in the order the world and the persistence task
            // produced them; a logout only after the character is stored (D-15).
            server.spawn(async move {
                while let Some(notice) = notices.recv().await {
                    match notice {
                        LsNotice::PlayerInGame { account } => link.player_in_game(&account).await,
                        LsNotice::PlayerLogout { account } => link.player_logout(&account).await,
                        LsNotice::AccessLevel { account, level } => {
                            link.send_access_level(&account, level).await
                        }
                        LsNotice::ChangePassword {
                            account,
                            character,
                            old_pass,
                            new_pass,
                        } => {
                            link.send_change_password(&account, &character, &old_pass, &new_pass)
                                .await
                        }
                        // The world only sends the six valid statuses.
                        LsNotice::SetServerStatus(status) => {
                            let _ = link.set_server_status(status).await;
                        }
                        LsNotice::SetMaxPlayer(max_players) => link.set_max_player(max_players).await,
                        LsNotice::ServerStatus { id, value } => link.send_server_status(id, value).await,
                    }
                }
            });
        }
        client::start_listener(&mut server).await?;
        admin::start(&mut server, &paths.config_dir).await?;

        info!("GameServer: Maximum number of connected players is {max_players}.");
        info!("Game server is ready.");
        Ok(server)
    }

    /// The login server notices in the order the world produced them. A `PlayerLogout` comes
    /// only after the character's final store (requirement of M2-PERSIST-01). `None` after the
    /// first call: the login server link takes it.
    pub fn take_ls_notices(&mut self) -> Option<mpsc::UnboundedReceiver<LsNotice>> {
        self.ls_notices.take()
    }

    /// The bound address of the client listener.
    pub fn client_addr(&self) -> Option<SocketAddr> {
        self.client_addr
    }

    /// The bound address of the admin endpoint (`None` when `Admin.ini` does not enable it).
    pub fn admin_addr(&self) -> Option<SocketAddr> {
        self.admin_addr
    }

    /// The link to the login server (`auth_player`, `player_logout`, ...).
    pub fn ls_link(&self) -> Option<&ls_link::LsLink> {
        self.ls_link.as_ref()
    }

    /// Runs `task` until shutdown. Listener and link cards use this.
    pub fn spawn<F>(&mut self, task: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.tasks.spawn(task);
    }

    /// A token that is cancelled when the server shuts down (for `l2_net::listen` and
    /// connections).
    pub fn cancel_token(&self) -> CancellationToken {
        self.cancel.child_token()
    }

    pub fn state(&self) -> &Arc<GameState> {
        &self.state
    }

    /// Waits until `stop` completes (a signal: exit code 0) or something requests a shutdown or
    /// restart through [`GameState::request_shutdown`], then shuts down.
    pub async fn run_until(mut self, stop: impl Future<Output = ()>) -> ExitReason {
        let reason = tokio::select! {
            () = stop => {
                info!("Shutdown signal received.");
                ExitReason::Shutdown
            }
            request = self.shutdown_requests.recv() => {
                // `None` cannot happen while the state holds a sender; treat it as a shutdown.
                request.unwrap_or(ExitReason::Shutdown)
            }
        };
        self.finish(reason).await
    }

    /// Stops right away. `restart` gives exit code 2 instead of 0.
    pub async fn shutdown(self, restart: bool) -> ExitReason {
        let reason = if restart {
            ExitReason::Restart
        } else {
            ExitReason::Shutdown
        };
        self.finish(reason).await
    }

    async fn finish(mut self, reason: ExitReason) -> ExitReason {
        info!(
            "Game server is shutting down{}.",
            if reason == ExitReason::Restart {
                " for a restart"
            } else {
                ""
            }
        );
        // 0. The path workers first: they send to the world's channel, and no search can start
        //    once the world no longer reads.
        let state = Arc::clone(&self.state);
        if tokio::task::spawn_blocking(move || state.geo.paths.shutdown())
            .await
            .is_err()
        {
            warn!("The path worker shutdown task failed.");
        }
        // 1. Store and disconnect every player, and wait until the outbox is drained. This
        //    comes first so the players get their goodbye packets while the connections are
        //    still open, and so the login server link (a task below) is still up for the
        //    `PlayerLogout` messages M2-GS-02 sends once a character is saved.
        self.world.shutdown().await;
        if let Some(link) = &self.ls_link {
            link.shutdown().await;
        }
        // 2. Stop listeners and connections, then abort the remaining tasks and wait for them,
        //    so nothing touches the pool later.
        self.cancel.cancel();
        self.tasks.shutdown().await;
        // 3. Java: DatabaseFactory.close(), the very last thing.
        self.state.pool.close().await;
        info!("Game server stopped, exit code {}.", reason.code());
        reason
    }
}

/// `//reload html [path]` (Java `HtmCache.reload`): the html cache read again. With a `path`
/// (relative to `data/html/`) the file or folder must exist, like Java's check. The whole cache
/// is read either way, see `l2_world::admin::reload`.
fn reload_html(
    root: &Path,
    path: Option<&str>,
    lazy: bool,
    check_encoding: bool,
) -> l2_world::HtmlReloadResult {
    if let Some(path) = path
        && !root.join("data/html").join(path).exists()
    {
        return l2_world::HtmlReloadResult::MissingFile;
    }
    l2_world::HtmlReloadResult::Reloaded(Arc::new(l2_data::HtmCache::load_with(root, lazy, check_encoding)))
}

/// A relative path is relative to the server's root folder (Java: the working directory).
/// Symlinks are not resolved.
fn resolve_datapack_root(root: &Path, value: &str) -> PathBuf {
    let path = PathBuf::from(value.replace('\\', "/"));
    let joined = root.join(path);
    let absolute = std::path::absolute(&joined).unwrap_or(joined);
    let mut out = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Each value is the key's getter result after Java's `load()`, with the same defaults, so an
/// empty config gives [`DatapackOptions::default`]. Keys: `General.ini` `CustomItemsLoad`,
/// `CustomNpcData` and `HtmCache`; `NPC.ini` `MaxAggroRange` and `DefaultCorpseTime`;
/// `Custom/FakePlayers.ini` `EnableFakePlayers`; `Development.ini` `NoSpawns`; `General.ini`
/// `CheckHtmlEncoding`.
// the NoSpawns read in DevelopmentConfig; HtmCache.java:140 (CHECK_HTML_ENCODING).
fn options_from_config(config: &GameConfig, config_dir: &Path) -> DatapackOptions {
    DatapackOptions {
        skills: world_data::skill_load_options(config, config_dir),
        custom_items_load: config.general.custom_items_load,
        custom_npc_data: config.general.custom_npc_data,
        fake_players_enabled: config.fake_players.fake_players_enabled,
        no_spawns: config.development.no_spawns,
        max_aggro_range: config.npc.max_aggro_range,
        default_corpse_time: config.npc.default_corpse_time,
        npc_stat_multipliers: custom_monsters::npc_stat_multipliers(config),
        htm_cache: config.general.htm_cache,
        check_html_encoding: config.general.check_html_encoding,
        custom_multisell_load: config.general.custom_multisell_load,
        correct_prices: config.general.correct_prices,
    }
}

/// Building blocks for unit tests that need a `Server` without a database.
#[cfg(test)]
pub(crate) mod test_support {
    use sqlx::mysql::MySqlPoolOptions;

    use super::*;
    use crate::state::WorldHandle;

    /// A pool that never connects, so shutdown paths run without a database.
    pub(crate) fn lazy_pool() -> Pool {
        MySqlPoolOptions::new()
            .connect_lazy("mysql://nobody@127.0.0.1:1/none")
            .unwrap()
    }

    /// A server with a running world, no listeners and a pool that is never used.
    pub(crate) fn idle_server() -> Server {
        idle_server_with(|_| {})
    }

    /// Like [`idle_server`], and `setup` configures the world before its thread starts (G-03:
    /// the admin tests give it GM rights).
    pub(crate) fn idle_server_with(setup: impl FnOnce(&mut l2_world::World)) -> Server {
        let world = WorldRuntime::start_configured(
            WorldRng::from_seed(1),
            l2_world::DEFAULT_AUTOSAVE_INTERVAL,
            setup,
            crate::runtime::spawn_stub_consumer,
        )
        .unwrap();
        let (shutdown, shutdown_requests) = ShutdownHandle::channel();
        let handle: WorldHandle = world.handle();
        let state = Arc::new(GameState {
            config: Arc::new(GameConfig::default()),
            paths: Paths::resolve(Path::new("."), Path::new("config")),
            pool: lazy_pool(),
            ids: Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()),
            datapack: Arc::new(empty_datapack()),
            static_data: Arc::new(l2_world::NoData),
            world_settings: l2_world::WorldSettings::default(),
            geo: geo::Geo::empty(),
            world: handle,
            left_acks: persist::LeftAcks::default(),
            hexid: None,
            shutdown,
        });
        Server {
            state,
            shutdown_requests,
            world,
            ls_notices: None,
            tasks: JoinSet::new(),
            cancel: CancellationToken::new(),
            client_addr: None,
            admin_addr: None,
            ls_link: None,
        }
    }

    fn empty_datapack() -> Datapack {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("data")).unwrap();
        Datapack::load(dir.path()).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::test_support::idle_server;

    /// The keys come from `OfflineTrade.ini` with Java's defaults (`OfflineNameColor` is hex).
    #[test]
    fn the_offline_settings_follow_the_ini() {
        let dir = tempfile::tempdir().unwrap();
        // Without OfflineTrade.ini: the Java defaults.
        let config = GameConfig::default();
        let settings = offline_settings(&config, dir.path());
        assert_eq!(settings, l2_world::OfflineSettings::default());
        // Restoring needs a trade key and RestoreOffliners.
        assert!(!settings.restore_wanted());
        let wanted = l2_world::OfflineSettings {
            offline_trade_enable: true,
            restore_offliners: true,
            ..settings
        };
        assert!(wanted.restore_wanted());

        // The name colour is hexadecimal.
        std::fs::create_dir_all(dir.path().join("Custom")).unwrap();
        std::fs::write(
            dir.path().join("Custom/OfflineTrade.ini"),
            "OfflineNameColor = FF8000\nOfflineMaxDays = 3\nStoreOfflineTradeInRealtime = False\n",
        )
        .unwrap();
        let config = GameConfig::load(dir.path()).unwrap();
        let settings = offline_settings(&config, dir.path());
        assert_eq!(settings.offline_name_color, 0x00FF_8000);
        assert_eq!(settings.offline_max_days, 3);
        assert!(!settings.store_offline_trade_in_realtime);
    }

    #[test]
    fn the_residence_values_come_from_player_ini_and_feature_ini() {
        let dir = tempfile::tempdir().unwrap();
        let config = GameConfig::default();
        let settings = residence_settings(&config, dir.path());
        assert_eq!(settings.take_castle_points, config.feature.take_castle_points);
        assert_eq!(
            settings.remove_castle_circlets,
            config.player.remove_castle_circlets
        );
    }

    /// `//reload html`: a path must exist below `data/html`, the cache is read again either way.
    #[test]
    fn reload_html_checks_the_path_and_reads_the_files_again() {
        use l2_world::HtmlReloadResult;
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("data/html/admin")).unwrap();
        std::fs::write(dir.path().join("data/html/admin/a.htm"), "<html>a</html>").unwrap();

        assert!(matches!(
            reload_html(dir.path(), Some("admin/nope.htm"), false, true),
            HtmlReloadResult::MissingFile
        ));
        for path in [Some("admin/a.htm"), Some("admin"), None] {
            match reload_html(dir.path(), path, false, true) {
                HtmlReloadResult::Reloaded(cache) => {
                    assert!(cache.contains("data/html/admin/a.htm"), "{path:?}");
                }
                other => panic!("{path:?}: {other:?}"),
            }
        }
    }

    #[test]
    fn paths_follow_the_java_dist_layout() {
        let paths = Paths::resolve(Path::new("dist/game"), Path::new("./config"));
        assert_eq!(paths.config_dir, Path::new("dist/game").join("./config"));
        assert_eq!(paths.log_dir, Path::new("dist/game").join("log"));
    }

    #[test]
    fn absolute_config_directories_are_kept() {
        let absolute = std::env::temp_dir();
        let paths = Paths::resolve(Path::new("dist/game"), &absolute);
        assert_eq!(paths.config_dir, absolute);
    }

    #[test]
    fn datapack_root_is_relative_to_the_root_and_normalized() {
        let root = std::env::temp_dir().join("gs_root");
        assert_eq!(resolve_datapack_root(&root, "."), root);
        assert_eq!(resolve_datapack_root(&root, "./a/../b"), root.join("b"));
        // Backslashes are separators, like Java's `replace('\\', '/')`.
        assert_eq!(resolve_datapack_root(&root, "x\\y"), root.join("x").join("y"));
        // An absolute value ignores the root.
        let elsewhere = std::env::temp_dir().join("elsewhere");
        let value = elsewhere.to_string_lossy().replace('\\', "/");
        assert_eq!(resolve_datapack_root(&root, &value), elsewhere);
    }

    #[tokio::test]
    async fn shutdown_request_ends_run_until_with_its_exit_code() {
        let server = idle_server();
        server.state().request_shutdown(true);
        let reason = tokio::time::timeout(
            Duration::from_secs(10),
            server.run_until(std::future::pending::<()>()),
        )
        .await
        .unwrap();
        assert_eq!(reason, ExitReason::Restart);
        assert_eq!(reason.code(), 2);
    }

    #[tokio::test]
    async fn stop_signal_ends_run_until_with_exit_code_0() {
        let server = idle_server();
        let reason = tokio::time::timeout(Duration::from_secs(10), server.run_until(std::future::ready(())))
            .await
            .unwrap();
        assert_eq!(reason, ExitReason::Shutdown);
        assert_eq!(reason.code(), 0);
    }

    #[tokio::test]
    async fn direct_shutdown_stops_the_world_thread() {
        let server = idle_server();
        let state = Arc::clone(server.state());
        assert_eq!(server.shutdown(false).await, ExitReason::Shutdown);
        // The world is gone, so its channel refuses commands.
        assert!(!state.world.send(l2_world::WorldCommand::Disconnected {
            session: l2_world::SessionId(1)
        }));
    }

    #[test]
    fn fatal_has_exit_code_1() {
        assert_eq!(ExitReason::Fatal.code(), 1);
    }

    /// No config files at all: every key has Java's default, so the options are the defaults.
    #[test]
    fn options_from_an_empty_config_are_java_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let config = GameConfig::load(dir.path()).unwrap();
        assert_eq!(
            options_from_config(&config, dir.path()),
            DatapackOptions::default()
        );
        assert_eq!(
            options_from_config(&GameConfig::default(), dir.path()),
            DatapackOptions::default()
        );
    }

    /// The shipped `dist/game/config` turns the custom folders on and HtmCache off. Skips when
    /// the Java checkout is not there (CI).
    #[test]
    fn options_from_the_shipped_config_enable_custom_data() {
        let dir = l2_testkit::game_dir().join("config");
        if !dir.is_dir() {
            eprintln!("skipped: no dist config at {}", dir.display());
            return;
        }
        let config = GameConfig::load(&dir).unwrap();
        let options = options_from_config(&config, &dir);
        assert!(options.custom_items_load);
        assert!(options.custom_npc_data);
        assert!(!options.htm_cache);
        assert!(options.check_html_encoding);
        assert!(!options.no_spawns);
        assert_eq!(options.max_aggro_range, 450);
        // Player.ini: EnableModifySkillDuration = True with the dance and song list.
        let skills = &options.skills;
        assert_eq!(
            (
                skills.min_abnormal_state_success_rate,
                skills.max_abnormal_state_success_rate
            ),
            (10, 90)
        );
        if skills.enable_modify_skill_duration {
            assert_eq!(skills.skill_duration_list.get(&264), Some(&3660));
        }
    }

    /// Each key reaches its option: EnableFakePlayers, NoSpawns, MaxAggroRange and
    /// DefaultCorpseTime from their own files, CustomItemsLoad from General.ini.
    #[test]
    fn each_config_key_reaches_its_option() {
        let dir = tempfile::tempdir().unwrap();
        let custom = dir.path().join("Custom");
        std::fs::create_dir_all(&custom).unwrap();
        std::fs::write(
            dir.path().join("General.ini"),
            "CustomItemsLoad = True\nHtmCache = False\nCheckHtmlEncoding = False\nCustomSkillsLoad = True\n\
             CustomMultisellLoad = True\nCorrectPrices = False\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("NPC.ini"),
            "MaxAggroRange = 900\nDefaultCorpseTime = 3\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("Development.ini"), "NoSpawns = True\n").unwrap();
        std::fs::write(custom.join("FakePlayers.ini"), "EnableFakePlayers = True\n").unwrap();
        std::fs::write(
            dir.path().join("Player.ini"),
            "MinAbnormalStateSuccessRate = 5\nMaxAbnormalStateSuccessRate = 95\n\
             EnableModifySkillDuration = True\nSkillDurationList = 264,3660;265,x;bad;\n\
             EnableModifySkillReuse = True\nSkillReuseList = 1,2\n",
        )
        .unwrap();

        let options = options_from_config(&GameConfig::load(dir.path()).unwrap(), dir.path());
        assert_eq!(
            options,
            DatapackOptions {
                custom_items_load: true,
                custom_npc_data: false,
                fake_players_enabled: true,
                no_spawns: true,
                max_aggro_range: 900,
                default_corpse_time: 3,
                npc_stat_multipliers: None,
                htm_cache: false,
                check_html_encoding: false,
                custom_multisell_load: true,
                correct_prices: false,
                skills: l2_data::SkillLoadOptions {
                    custom_skills_load: true,
                    min_abnormal_state_success_rate: 5,
                    max_abnormal_state_success_rate: 95,
                    enable_modify_skill_duration: true,
                    skill_duration_list: [(264, 3660)].into_iter().collect(),
                    enable_modify_skill_reuse: true,
                    skill_reuse_list: [(1, 2)].into_iter().collect(),
                },
            }
        );
    }
}
