//! The `()` placeholders of the generated game config that were ported by hand, and where.
//!
//! A placeholder stays `()` (the generated file is rebuilt from the Java source every time), but
//! its doc comment says where the real value is instead of `TODO(M2-CFG-01)`, and the report
//! (`docs/analysis/config-manual-fields.md`) marks it. A row names the Java class, the Java
//! field and the Rust path below `GameConfig` (or the place that reads the value, when it is not
//! a config field).
//!
//! Test `ported_table_names_real_placeholders` (real Java source only) makes sure that no row
//! names a field that is not a placeholder, so the table cannot rot.

/// `(Java class, Java field, where the value lives)`.
pub const HAND_PORTED: &[(&str, &str, &str)] = &[
    // ---- ServerConfig (M2-CFG-01 `ServerManual`, L-09 `ServerRest`) ----
    (
        "ServerConfig",
        "DATAPACK_ROOT",
        "manual.core.server.datapack_root",
    ),
    ("ServerConfig", "SCRIPT_ROOT", "manual.core.server.script_root"),
    (
        "ServerConfig",
        "CHARNAME_TEMPLATE_PATTERN",
        "manual.server.charname_template",
    ),
    (
        "ServerConfig",
        "PET_NAME_TEMPLATE_PATTERN",
        "manual.server.pet_name_template",
    ),
    (
        "ServerConfig",
        "CLAN_NAME_TEMPLATE_PATTERN",
        "manual.server.clan_name_template",
    ),
    ("ServerConfig", "PROTOCOL_LIST", "manual.server.protocol_list"),
    (
        "ServerConfig",
        "SERVER_LIST_TYPE",
        "manual.core.server.server_list_type",
    ),
    (
        "ServerConfig",
        "SERVER_RESTART_SCHEDULE",
        "manual.core.server.server_restart_schedule",
    ),
    (
        "ServerConfig",
        "SERVER_RESTART_DAYS",
        "manual.core.server.server_restart_days",
    ),
    (
        "ServerConfig",
        "PRECAUTIONARY_RESTART_DELAY",
        "manual.core.server.precautionary_restart_delay",
    ),
    (
        "ServerConfig",
        "GAME_SERVER_SUBNETS",
        "game-server `ls_link::ipconfig::IpConfig` (ipconfig.xml)",
    ),
    (
        "ServerConfig",
        "GAME_SERVER_HOSTS",
        "game-server `ls_link::ipconfig::IpConfig` (ipconfig.xml)",
    ),
    ("ServerConfig", "FILTER_LIST", "manual.core.server.filter_list"),
    (
        "ServerConfig",
        "SERVER_ID",
        "game-server `hexid::HexId` (hexid.txt)",
    ),
    ("ServerConfig", "HEX_ID", "game-server `hexid::HexId` (hexid.txt)"),
    // ---- Development, Feature ----
    (
        "DevelopmentConfig",
        "EXCLUDED_DEBUG_PACKETS",
        "manual.core.development.excluded_debug_packets",
    ),
    (
        "FeatureConfig",
        "SIEGE_HOUR_LIST",
        "manual.core.feature.siege_hour_list",
    ),
    // ---- FloodProtectorConfig ----
    (
        "FloodProtectorConfig",
        "FLOOD_PROTECTOR_USE_ITEM",
        "manual.core.flood_protector.use_item",
    ),
    (
        "FloodProtectorConfig",
        "FLOOD_PROTECTOR_ROLL_DICE",
        "manual.core.flood_protector.roll_dice",
    ),
    (
        "FloodProtectorConfig",
        "FLOOD_PROTECTOR_ITEM_PET_SUMMON",
        "manual.core.flood_protector.item_pet_summon",
    ),
    (
        "FloodProtectorConfig",
        "FLOOD_PROTECTOR_HERO_VOICE",
        "manual.core.flood_protector.hero_voice",
    ),
    (
        "FloodProtectorConfig",
        "FLOOD_PROTECTOR_GLOBAL_CHAT",
        "manual.core.flood_protector.global_chat",
    ),
    (
        "FloodProtectorConfig",
        "FLOOD_PROTECTOR_SUBCLASS",
        "manual.core.flood_protector.subclass",
    ),
    (
        "FloodProtectorConfig",
        "FLOOD_PROTECTOR_DROP_ITEM",
        "manual.core.flood_protector.drop_item",
    ),
    (
        "FloodProtectorConfig",
        "FLOOD_PROTECTOR_SERVER_BYPASS",
        "manual.core.flood_protector.server_bypass",
    ),
    (
        "FloodProtectorConfig",
        "FLOOD_PROTECTOR_MULTISELL",
        "manual.core.flood_protector.multisell",
    ),
    (
        "FloodProtectorConfig",
        "FLOOD_PROTECTOR_TRANSACTION",
        "manual.core.flood_protector.transaction",
    ),
    (
        "FloodProtectorConfig",
        "FLOOD_PROTECTOR_MANUFACTURE",
        "manual.core.flood_protector.manufacture",
    ),
    (
        "FloodProtectorConfig",
        "FLOOD_PROTECTOR_SENDMAIL",
        "manual.core.flood_protector.sendmail",
    ),
    (
        "FloodProtectorConfig",
        "FLOOD_PROTECTOR_CHARACTER_SELECT",
        "manual.core.flood_protector.character_select",
    ),
    (
        "FloodProtectorConfig",
        "FLOOD_PROTECTOR_ITEM_AUCTION",
        "manual.core.flood_protector.item_auction",
    ),
    (
        "FloodProtectorConfig",
        "FLOOD_PROTECTOR_PLAYER_ACTION",
        "manual.core.flood_protector.player_action",
    ),
    // ---- GeneralConfig ----
    (
        "GeneralConfig",
        "LOG_ITEMS_IDS_LIST",
        "manual.core.general.log_items_ids_list",
    ),
    (
        "GeneralConfig",
        "HERB_AUTO_DESTROY_TIME",
        "manual.core.general.herb_auto_destroy_time",
    ),
    (
        "GeneralConfig",
        "LIST_PROTECTED_ITEMS",
        "manual.core.general.list_protected_items",
    ),
    (
        "GeneralConfig",
        "CHAR_DATA_STORE_INTERVAL",
        "manual.core.general.char_data_store_interval",
    ),
    (
        "GeneralConfig",
        "SAVE_DROPPED_ITEM_INTERVAL",
        "manual.core.general.save_dropped_item_interval",
    ),
    (
        "GeneralConfig",
        "DEFAULT_GLOBAL_CHAT",
        "manual.core.general.default_global_chat",
    ),
    (
        "GeneralConfig",
        "DEFAULT_TRADE_CHAT",
        "manual.core.general.default_trade_chat",
    ),
    (
        "GeneralConfig",
        "INSTANCE_FINISH_TIME",
        "manual.core.general.instance_finish_time",
    ),
    (
        "GeneralConfig",
        "EJECT_DEAD_PLAYER_TIME",
        "manual.core.general.eject_dead_player_time",
    ),
    (
        "GeneralConfig",
        "BAN_CHAT_CHANNELS",
        "manual.core.general.ban_chat_channels",
    ),
    (
        "GeneralConfig",
        "ALT_ITEM_AUCTION_TIME_EXTENDS_ON_BID",
        "manual.core.general.alt_item_auction_time_extends_on_bid",
    ),
    (
        "GeneralConfig",
        "DEFAULT_PUNISH",
        "manual.core.general.default_punish",
    ),
    (
        "GeneralConfig",
        "ALT_BIRTHDAY_MAIL_TEXT",
        "manual.core.general.alt_birthday_mail_text",
    ),
    (
        "GeneralConfig",
        "BOTREPORT_RESETPOINT_HOUR",
        "manual.core.general.botreport_resetpoint_hour",
    ),
    (
        "GeneralConfig",
        "BOTREPORT_REPORT_DELAY",
        "manual.core.general.botreport_report_delay",
    ),
    // ---- GeoEngine, GraciaSeeds, IdManager ----
    (
        "GeoEngineConfig",
        "GEODATA_PATH",
        "manual.core.geo_engine.geodata_path",
    ),
    (
        "GeoEngineConfig",
        "PATHNODE_PATH",
        "manual.core.geo_engine.pathnode_path",
    ),
    (
        "GeoEngineConfig",
        "GEOEDIT_PATH",
        "manual.core.geo_engine.geoedit_path",
    ),
    (
        "GraciaSeedsConfig",
        "SOD_STAGE_2_LENGTH",
        "manual.core.gracia_seeds.sod_stage_2_length",
    ),
    (
        "IdManagerConfig",
        "INITIAL_CAPACITY",
        "`GameConfig::effective_initial_capacity` (manual.core.id_manager)",
    ),
    // ---- NpcConfig (M2-CFG-01 `NpcManual`, L-09 `NpcRest`) ----
    ("NpcConfig", "NPC_DMG_PENALTY", "manual.core.npc.npc_dmg_penalty"),
    (
        "NpcConfig",
        "NPC_CRIT_DMG_PENALTY",
        "manual.core.npc.npc_crit_dmg_penalty",
    ),
    (
        "NpcConfig",
        "NPC_SKILL_DMG_PENALTY",
        "manual.core.npc.npc_skill_dmg_penalty",
    ),
    (
        "NpcConfig",
        "NPC_SKILL_CHANCE_PENALTY",
        "manual.core.npc.npc_skill_chance_penalty",
    ),
    (
        "NpcConfig",
        "LIST_PET_RENT_NPC",
        "manual.core.npc.list_pet_rent_npc",
    ),
    (
        "NpcConfig",
        "RAID_HP_REGEN_MULTIPLIER",
        "manual.npc.raid_hp_regen_multiplier",
    ),
    (
        "NpcConfig",
        "RAID_MP_REGEN_MULTIPLIER",
        "manual.npc.raid_mp_regen_multiplier",
    ),
    (
        "NpcConfig",
        "RAID_PDEFENCE_MULTIPLIER",
        "manual.core.npc.raid_pdefence_multiplier",
    ),
    (
        "NpcConfig",
        "RAID_MDEFENCE_MULTIPLIER",
        "manual.core.npc.raid_mdefence_multiplier",
    ),
    (
        "NpcConfig",
        "RAID_PATTACK_MULTIPLIER",
        "manual.core.npc.raid_pattack_multiplier",
    ),
    (
        "NpcConfig",
        "RAID_MATTACK_MULTIPLIER",
        "manual.core.npc.raid_mattack_multiplier",
    ),
    (
        "NpcConfig",
        "MINIONS_RESPAWN_TIME",
        "manual.npc.minions_respawn_time",
    ),
    (
        "NpcConfig",
        "PET_HP_REGEN_MULTIPLIER",
        "manual.core.npc.pet_hp_regen_multiplier",
    ),
    (
        "NpcConfig",
        "PET_MP_REGEN_MULTIPLIER",
        "manual.core.npc.pet_mp_regen_multiplier",
    ),
    // ---- OlympiadConfig ----
    (
        "OlympiadConfig",
        "OLYMPIAD_CLASSED_REWARD",
        "manual.core.olympiad.olympiad_classed_reward",
    ),
    (
        "OlympiadConfig",
        "OLYMPIAD_NONCLASSED_REWARD",
        "manual.core.olympiad.olympiad_nonclassed_reward",
    ),
    (
        "OlympiadConfig",
        "OLYMPIAD_TEAM_REWARD",
        "manual.core.olympiad.olympiad_team_reward",
    ),
    (
        "OlympiadConfig",
        "LIST_OLY_RESTRICTED_ITEMS",
        "manual.core.olympiad.list_oly_restricted_items",
    ),
    (
        "OlympiadConfig",
        "OLYMPIAD_COMPETITION_DAYS",
        "manual.core.olympiad.olympiad_competition_days",
    ),
    // ---- PlayerConfig (M2-CFG-01 `PlayerManual`, L-09 `PlayerRest`) ----
    (
        "PlayerConfig",
        "RESPAWN_RESTORE_CP",
        "manual.core.player.respawn_restore_cp",
    ),
    (
        "PlayerConfig",
        "RESPAWN_RESTORE_HP",
        "manual.core.player.respawn_restore_hp",
    ),
    (
        "PlayerConfig",
        "RESPAWN_RESTORE_MP",
        "manual.core.player.respawn_restore_mp",
    ),
    (
        "PlayerConfig",
        "HP_REGEN_MULTIPLIER",
        "manual.player.hp_regen_multiplier",
    ),
    (
        "PlayerConfig",
        "MP_REGEN_MULTIPLIER",
        "manual.player.mp_regen_multiplier",
    ),
    (
        "PlayerConfig",
        "CP_REGEN_MULTIPLIER",
        "manual.player.cp_regen_multiplier",
    ),
    (
        "PlayerConfig",
        "SKILL_DURATION_LIST",
        "manual.core.player.skill_duration_list",
    ),
    (
        "PlayerConfig",
        "SKILL_REUSE_LIST",
        "manual.core.player.skill_reuse_list",
    ),
    (
        "PlayerConfig",
        "ALT_GAME_CANCEL_BOW",
        "manual.core.player.alt_game_cancel_bow",
    ),
    (
        "PlayerConfig",
        "ALT_GAME_CANCEL_CAST",
        "manual.core.player.alt_game_cancel_cast",
    ),
    ("PlayerConfig", "MAX_SP", "manual.player.max_sp"),
    (
        "PlayerConfig",
        "MAX_ITEM_IN_PACKET",
        "`GameConfig::effective_max_item_in_packet`",
    ),
    (
        "PlayerConfig",
        "ENCHANT_BLACKLIST",
        "manual.core.player.enchant_blacklist",
    ),
    (
        "PlayerConfig",
        "OVER_ENCHANT_PUNISHMENT",
        "manual.core.player.over_enchant_punishment",
    ),
    (
        "PlayerConfig",
        "RETAIL_LIKE_AUGMENTATION_NG_CHANCE",
        "manual.core.player.retail_like_augmentation_ng_chance",
    ),
    (
        "PlayerConfig",
        "RETAIL_LIKE_AUGMENTATION_MID_CHANCE",
        "manual.core.player.retail_like_augmentation_mid_chance",
    ),
    (
        "PlayerConfig",
        "RETAIL_LIKE_AUGMENTATION_HIGH_CHANCE",
        "manual.core.player.retail_like_augmentation_high_chance",
    ),
    (
        "PlayerConfig",
        "RETAIL_LIKE_AUGMENTATION_TOP_CHANCE",
        "manual.core.player.retail_like_augmentation_top_chance",
    ),
    (
        "PlayerConfig",
        "AUGMENTATION_BLACKLIST",
        "manual.core.player.augmentation_blacklist",
    ),
    (
        "PlayerConfig",
        "LOOT_RAIDS_PRIVILEGE_INTERVAL",
        "manual.core.player.loot_raids_privilege_interval",
    ),
    (
        "PlayerConfig",
        "AUTO_LOOT_ITEM_IDS",
        "manual.core.player.auto_loot_item_ids",
    ),
    (
        "PlayerConfig",
        "PARTY_XP_CUTOFF_METHOD",
        "manual.core.player.party_xp_cutoff_method",
    ),
    (
        "PlayerConfig",
        "PARTY_XP_CUTOFF_GAPS",
        "manual.core.player.party_xp_cutoff_gaps",
    ),
    (
        "PlayerConfig",
        "PARTY_XP_CUTOFF_GAP_PERCENTS",
        "manual.core.player.party_xp_cutoff_gap_percents",
    ),
    ("PlayerConfig", "FORBIDDEN_NAMES", "manual.player.forbidden_names"),
    (
        "PlayerConfig",
        "PLAYER_MOVEMENT_BLOCK_TIME",
        "manual.core.player.player_movement_block_time",
    ),
    // ---- PvpConfig, RatesConfig ----
    (
        "PvpConfig",
        "KARMA_LIST_NONDROPPABLE_PET_ITEMS",
        "manual.core.pvp.karma_list_nondroppable_pet_items",
    ),
    (
        "PvpConfig",
        "KARMA_LIST_NONDROPPABLE_ITEMS",
        "manual.core.pvp.karma_list_nondroppable_items",
    ),
    (
        "PvpConfig",
        "ANTIFEED_INTERVAL",
        "manual.core.pvp.antifeed_interval",
    ),
    (
        "RatesConfig",
        "RATE_DROP_AMOUNT_BY_ID",
        "manual.core.rates.rate_drop_amount_by_id",
    ),
    (
        "RatesConfig",
        "RATE_DROP_CHANCE_BY_ID",
        "manual.core.rates.rate_drop_chance_by_id",
    ),
    (
        "RatesConfig",
        "BOSS_DROP_LIST",
        "manual.core.rates.boss_drop_list",
    ),
    // ---- FactionSystemConfig (custom, M2-CFG-01 `FactionSystemManual`) ----
    (
        "FactionSystemConfig",
        "FACTION_STARTING_LOCATION",
        "manual.faction_system.faction_starting_location",
    ),
    (
        "FactionSystemConfig",
        "FACTION_MANAGER_LOCATION",
        "manual.faction_system.faction_manager_location",
    ),
    (
        "FactionSystemConfig",
        "FACTION_GOOD_BASE_LOCATION",
        "manual.faction_system.faction_good_base_location",
    ),
    (
        "FactionSystemConfig",
        "FACTION_EVIL_BASE_LOCATION",
        "manual.faction_system.faction_evil_base_location",
    ),
];

/// Where the value of the placeholder `class.field` lives, when it was ported by hand.
pub fn ported_at(class: &str, field: &str) -> Option<&'static str> {
    HAND_PORTED
        .iter()
        .find(|(c, f, _)| *c == class && *f == field)
        .map(|(_, _, at)| *at)
}

/// [`ported_at`] as text for a doc comment or the report: a path below `GameConfig` is quoted
/// as `GameConfig::path`, other places are used as written.
pub fn ported_where(class: &str, field: &str) -> Option<String> {
    ported_at(class, field).map(|at| {
        if at.contains(' ') || at.starts_with('`') {
            at.to_string()
        } else {
            format!("`GameConfig::{at}`")
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_row_is_listed_twice() {
        for (i, (class, field, _)) in HAND_PORTED.iter().enumerate() {
            assert!(
                HAND_PORTED[i + 1..]
                    .iter()
                    .all(|(c, f, _)| (c, f) != (class, field)),
                "{class}.{field} is listed twice"
            );
        }
    }

    #[test]
    fn lookup_finds_a_row() {
        assert_eq!(
            ported_at("PvpConfig", "ANTIFEED_INTERVAL"),
            Some("manual.core.pvp.antifeed_interval")
        );
        assert_eq!(ported_at("PvpConfig", "NOPE"), None);
    }
}
