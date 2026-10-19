//! The datapack as the world sees it ([`l2_world::StaticData`], including the stat engine's
//! tables), and the settings the world reads from the configuration.
// Java: mechanics/stats/BaseStat.java (the statBonus.xml loader)
// Java: data/xml/{AdminData,SkillData}.java, data/holders/AccessLevel.java
// Java: config/{PlayerConfig,NpcConfig,ChampionMonstersConfig,OlympiadConfig,FeatureConfig}.java,
//       config/custom/ClassBalanceConfig.java (the values the stat getters read)
//
// The stat engine itself is `l2_world::stats` (M4-STAT-01); this file only hands it the tables
// (`StatData`) and the settings (`StatSettings`).

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use l2_config::GameConfig;
use l2_data::Datapack;
use l2_data::ItemKind;
use l2_world::{
    AccessLevelStatic, AdminRights, BaseStat, BaseStatBonus, ChatBroadcast, ItemStatic, PlayerData, StatData,
    StatSettings, StaticData, TalkSettings, TeleportSettings, WorldSettings, ZoneSettings,
};
use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};
use tracing::{info, warn};

/// Reads `<root>/data/stats/statBonus.xml` (Java `BaseStat`'s static block). A missing file
/// leaves every bonus at 0 (Java throws and the server does not start).
pub fn load_stat_bonus(root: &Path) -> BaseStatBonus {
    let path = root.join("data/stats/statBonus.xml");
    match std::fs::read(&path) {
        Ok(bytes) => stat_bonus_from_xml(&String::from_utf8_lossy(&bytes)),
        Err(e) => {
            warn!("[BaseStats] Could not read {}: {e}", path.display());
            BaseStatBonus::loaded()
        }
    }
}

/// Parses the text of `statBonus.xml`. A broken value is skipped like Java's "Invalid stats
/// value", an unknown section like its "Invalid stats name".
pub fn stat_bonus_from_xml(xml: &str) -> BaseStatBonus {
    let mut table = BaseStatBonus::loaded();
    let mut reader = Reader::from_str(xml);
    let mut section: Option<String> = None;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = e.name().as_ref().to_owned();
                if name != "list" && name != "stat" {
                    section = Some(name);
                }
            }
            Ok(Event::End(e)) => {
                if section.as_deref().is_some_and(|s| s == e.name().as_ref()) {
                    section = None;
                }
            }
            Ok(Event::Empty(e)) if e.name().as_ref() == "stat" => {
                let (Some(value), Some(bonus)) = (attribute(&e, "value"), attribute(&e, "bonus")) else {
                    continue;
                };
                let (Ok(value), Ok(bonus)) = (value.parse::<usize>(), bonus.parse::<f64>()) else {
                    warn!("[BaseStats] Invalid stats value, skipping");
                    continue;
                };
                match section.as_deref().and_then(BaseStat::from_section) {
                    Some(stat) => {
                        if !table.set(stat, value, bonus) {
                            warn!("[BaseStats] Invalid stats value: {value}, skipping");
                        }
                    }
                    None => warn!(
                        "[BaseStats] Invalid stats name: {}, skipping",
                        section.as_deref().unwrap_or("")
                    ),
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => {
                warn!("[BaseStats] Could not parse statBonus.xml: {e}");
                break;
            }
        }
    }
    table
}

fn attribute(element: &BytesStart<'_>, key: &str) -> Option<String> {
    element
        .attributes()
        .flatten()
        .find(|attr| attr.key.as_ref() == key)
        .and_then(|attr| {
            attr.normalized_value(XmlVersion::Implicit1_0)
                .ok()
                .map(std::borrow::Cow::into_owned)
        })
}

/// `config/AccessLevels.xml` and `config/AdminCommands.xml` (Java `AdminData`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AccessLevels {
    levels: BTreeMap<i32, AccessLevelStatic>,
    /// Who may run which admin command.
    rights: AdminRights,
}

impl AccessLevels {
    pub const FILE: &'static str = "AccessLevels.xml";
    pub const COMMANDS_FILE: &'static str = "AdminCommands.xml";

    pub fn load(config_dir: &Path) -> Self {
        let path = config_dir.join(Self::FILE);
        let mut table = match std::fs::read(&path) {
            Ok(bytes) => Self::from_xml(&String::from_utf8_lossy(&bytes)),
            Err(e) => {
                warn!(
                    "Cannot read {}: {e}; every access level is a plain user.",
                    path.display()
                );
                Self::default()
            }
        };
        info!("AdminData: Loaded {} access levels.", table.rights.level_count());
        let path = config_dir.join(Self::COMMANDS_FILE);
        match std::fs::read(&path) {
            Ok(bytes) => table.add_commands_xml(&String::from_utf8_lossy(&bytes)),
            Err(e) => warn!(
                "Cannot read {}: {e}; no GM may use admin commands.",
                path.display()
            ),
        }
        info!(
            "AdminData: Loaded {} access commands.",
            table.rights.command_count()
        );
        table
    }

    /// The admin command rights.
    pub fn rights(&self) -> &AdminRights {
        &self.rights
    }

    /// Java `AdminData.parseDocument`, `<admin>`: `command`, `accessLevel` (default 100) and
    /// `confirmDlg` (default false).
    pub fn add_commands_xml(&mut self, xml: &str) {
        let mut reader = Reader::from_str(xml);
        loop {
            match reader.read_event() {
                Ok(Event::Empty(e) | Event::Start(e)) if e.name().as_ref() == "admin" => {
                    let Some(command) = attribute(&e, "command") else {
                        continue;
                    };
                    let level = attribute(&e, "accessLevel")
                        .and_then(|v| v.trim().parse::<i32>().ok())
                        .unwrap_or(100);
                    let confirm = attribute(&e, "confirmDlg").is_some_and(|v| v.eq_ignore_ascii_case("true"));
                    self.rights.add_command(&command, level, confirm);
                }
                Ok(Event::Eof) | Err(_) => break,
                Ok(_) => {}
            }
        }
    }

    pub fn from_xml(xml: &str) -> Self {
        let mut table = Self::default();
        let mut reader = Reader::from_str(xml);
        loop {
            match reader.read_event() {
                Ok(Event::Empty(e) | Event::Start(e)) if e.name().as_ref() == "access" => {
                    let Some(level) = attribute(&e, "level").and_then(|v| v.trim().parse::<i32>().ok())
                    else {
                        continue;
                    };
                    // Java: `Integer.decode("0x" + set.getString("nameColor", "FFFFFF"))`
                    let color = |key: &str| {
                        attribute(&e, key)
                            .and_then(|v| i32::from_str_radix(v.trim(), 16).ok())
                            .unwrap_or(0x00FF_FFFF)
                    };
                    let child = attribute(&e, "childAccess")
                        .and_then(|v| v.trim().parse::<i32>().ok())
                        .unwrap_or(0);
                    table.rights.add_level(level, child);
                    table.levels.insert(
                        level,
                        AccessLevelStatic {
                            name_color: color("nameColor"),
                            title_color: color("titleColor"),
                            is_gm: attribute(&e, "isGM").is_some_and(|v| v.eq_ignore_ascii_case("true")),
                        },
                    );
                }
                Ok(Event::Eof) | Err(_) => break,
                Ok(_) => {}
            }
        }
        table
    }

    /// Java `AdminData.getAccessLevel`: below 0 is the banned level; a level the file does not
    /// list is a default `AccessLevel` (white name and title, no GM rights).
    pub fn get(&self, level: i32) -> AccessLevelStatic {
        let key = if level < 0 { -1 } else { level };
        self.levels
            .get(&key)
            .copied()
            .unwrap_or(AccessLevelStatic::UNREGISTERED)
    }
}

/// [`StaticData`] over the loaded datapack.
pub struct DatapackData {
    datapack: Arc<Datapack>,
    access: AccessLevels,
    /// The tables of the stat engine (shared with the datapack).
    stats: StatData,
}

impl DatapackData {
    pub fn new(datapack: Arc<Datapack>, bonus: BaseStatBonus, access: AccessLevels) -> Self {
        let stats = StatData {
            items: Arc::clone(&datapack.items),
            player_templates: Arc::clone(&datapack.player_templates),
            armor_sets: Arc::clone(&datapack.armor_sets),
            enchant_hp_bonus: Arc::clone(&datapack.enchant_hp_bonus),
            hennas: Arc::clone(&datapack.hennas),
            base_stat_bonus: Arc::new(bonus),
            skills: Arc::clone(&datapack.skills),
        };
        Self {
            datapack,
            access,
            stats,
        }
    }

    /// Loads the extra tables from the datapack folder and the config folder.
    pub fn load(datapack: Arc<Datapack>, config_dir: &Path) -> Self {
        let bonus = load_stat_bonus(&datapack.root);
        let access = AccessLevels::load(config_dir);
        Self::new(datapack, bonus, access)
    }
}

impl StaticData for DatapackData {
    fn item(&self, item_id: i32) -> Option<ItemStatic> {
        let t = self.datapack.items.get(item_id)?;
        Some(ItemStatic {
            display_id: t.display_id,
            type2: i16::try_from(t.type2).unwrap_or(5),
            body_part: t.body_part.mask(),
            quest_item: t.quest_item,
            weight: t.weight,
            // Shortcuts.restoreMe only reads the group of etc items; the others keep -1.
            shared_reuse_group: if matches!(t.kind, ItemKind::Etc(_)) {
                t.shared_reuse_group
            } else {
                -1
            },
            weapon: t.is_weapon(),
        })
    }

    // Java `SkillList`: `skill.isPassive()` (operate type `P`). The list only has ids here, and a
    // skill's operate type is the same at every level, so level 1 (or the highest) is asked.
    fn skill_is_passive(&self, skill_id: i32) -> bool {
        let skills = &self.datapack.skills;
        skills
            .get(skill_id, 1)
            .or_else(|| skills.get(skill_id, skills.max_level(skill_id)))
            .is_some_and(l2_data::SkillTemplate::is_passive)
    }

    fn item_name(&self, item_id: i32) -> Option<String> {
        self.datapack.items.get(item_id).map(|t| t.name.clone())
    }

    fn item_stackable(&self, item_id: i32) -> bool {
        self.datapack.items.get(item_id).is_some_and(|t| t.stackable)
    }

    fn stat_data(&self) -> Option<&StatData> {
        Some(&self.stats)
    }

    fn exp_percent(&self, exp: i64, level: i32) -> f64 {
        self.datapack.experience.exp_percent(exp, level).unwrap_or(0.0)
    }

    fn access_level(&self, level: i32) -> AccessLevelStatic {
        self.access.get(level)
    }

    fn admin_rights(&self) -> Option<&AdminRights> {
        Some(self.access.rights())
    }

    fn safe_fall_height(&self, player: &PlayerData) -> i32 {
        self.datapack
            .player_templates
            .get_by_id(player.class_id)
            .map_or(333, |t| t.safe_fall)
    }
}

/// The values the world reads from the configuration (Java `PlayerConfig`, `GeneralConfig`).
///
/// `GlobalChat` and `TradeChat` are enum placeholders in the generated config, so they are read
/// from `General.ini` here.
pub fn world_settings(config: &GameConfig, config_dir: &Path) -> WorldSettings {
    use l2_config::IniFile;
    use l2_config::game::GeneralConfig;

    let p = &config.player;
    let mut settings = WorldSettings {
        nevit_enabled: p.nevit_enabled,
        store_ui_settings: p.store_ui_settings,
        inventory_maximum_no_dwarf: p.inventory_maximum_no_dwarf,
        inventory_maximum_dwarf: p.inventory_maximum_dwarf,
        inventory_maximum_gm: p.inventory_maximum_gm,
        inventory_maximum_quest_items: p.inventory_maximum_quest_items,
        warehouse_slots_no_dwarf: p.warehouse_slots_no_dwarf,
        warehouse_slots_dwarf: p.warehouse_slots_dwarf,
        warehouse_slots_clan: p.warehouse_slots_clan,
        max_pvtstoresell_slots_dwarf: p.max_pvtstoresell_slots_dwarf,
        max_pvtstoresell_slots_other: p.max_pvtstoresell_slots_other,
        max_pvtstorebuy_slots_dwarf: p.max_pvtstorebuy_slots_dwarf,
        max_pvtstorebuy_slots_other: p.max_pvtstorebuy_slots_other,
        dwarf_recipe_limit: p.dwarf_recipe_limit,
        common_recipe_limit: p.common_recipe_limit,
        minimum_chat_level: config.general.minimum_chat_level,
        grids_always_on: config.general.grids_always_on,
        grid_neighbor_turnon_secs: config.general.grid_neighbor_turnon_time,
        grid_neighbor_turnoff_secs: config.general.grid_neighbor_turnoff_time,
        gm_hero_aura: config.general.gm_hero_aura,
        log_chat: config.general.log_chat,
        gm_audit: config.general.gmaudit,
        gm_startup_builder_hide: config.general.gm_startup_builder_hide,
        zones: ZoneSettings {
            peace_zone_mode: config.general.peace_zone_mode,
            jail_is_pvp: config.general.jail_is_pvp,
            jail_disable_transaction: config.general.jail_disable_transaction,
        },
        teleport: TeleportSettings {
            offset_on_teleport_enabled: p.offset_on_teleport_enabled,
            max_offset_on_teleport: p.max_offset_on_teleport,
            random_respawn_in_town_enabled: p.random_respawn_in_town_enabled,
            teleport_watchdog_timeout: p.teleport_watchdog_timeout,
        },
        stats: stat_settings(config, config_dir),
        talk: TalkSettings {
            karma_player_can_shop: p.alt_game_karma_player_can_shop,
            karma_player_can_use_gk: p.alt_game_karma_player_can_use_gk,
            karma_player_can_use_warehouse: p.alt_game_karma_player_can_use_warehouse,
            max_free_teleport_level: p.max_free_teleport_level,
            teleport_while_siege_in_progress: p.teleport_while_siege_in_progress,
            max_npc_animation: config.general.max_npc_animation,
            gm_debug_html_paths: config.general.gm_debug_html_paths,
            ..TalkSettings::default()
        },
        ..WorldSettings::default()
    };
    // `ListPetRentNpc` is a post-processed list in the generated config, so it is read from
    // `NPC.ini` directly (Java: `Integer.parseInt` of each comma separated id).
    if let Ok(ini) = IniFile::load(config_dir.join(l2_config::game::NpcConfig::FILE)) {
        let text = ini.get_string("ListPetRentNpc", "30827");
        let ids: Result<Vec<i32>, _> = text.split(',').map(|id| id.trim().parse::<i32>()).collect();
        match ids {
            Ok(ids) => settings.talk.list_pet_rent_npc = ids.into_iter().collect(),
            Err(_) => warn!("ListPetRentNpc = {text} is not a list of ids; using 30827."),
        }
    }
    // `NpcTalkBlockingTime` is post-processed (seconds -> ms) in the generated config, so it is
    // read from `Player.ini` directly.
    if let Ok(ini) = IniFile::load(config_dir.join(l2_config::game::PlayerConfig::FILE)) {
        settings.player_movement_block_time_ms = ini.get_int("NpcTalkBlockingTime", 0).saturating_mul(1000);
    }
    settings.keyboard_movement = p.enable_keyboard_movement;
    // Java: `Enum.valueOf(ChatBroadcastType.class, config.getString("GlobalChat", "ON"))` throws
    // for a bad name; the default is kept here.
    if let Ok(ini) = IniFile::load(config_dir.join(GeneralConfig::FILE)) {
        for (key, target) in [
            ("GlobalChat", &mut settings.global_chat),
            ("TradeChat", &mut settings.trade_chat),
        ] {
            let text = ini.get_string(key, "ON");
            match ChatBroadcast::from_name(text.trim()) {
                Some(mode) => *target = mode,
                None => warn!("{key} = {text} is not ON, OFF, GM or GLOBAL; using ON."),
            }
        }
    }
    settings
}

/// The skill loader's options (Java `GeneralConfig.CUSTOM_SKILLS_LOAD`, `PlayerConfig`'s abnormal
/// success rates and the duration / reuse override lists). The two lists are post-processed in
/// Java (the generated config has placeholders), so they are read from `Player.ini` here.
pub fn skill_load_options(config: &GameConfig, config_dir: &Path) -> l2_data::SkillLoadOptions {
    use l2_config::IniFile;

    let p = &config.player;
    let mut options = l2_data::SkillLoadOptions {
        custom_skills_load: config.general.custom_skills_load,
        min_abnormal_state_success_rate: p.min_abnormal_state_success_rate,
        max_abnormal_state_success_rate: p.max_abnormal_state_success_rate,
        enable_modify_skill_duration: p.enable_modify_skill_duration,
        enable_modify_skill_reuse: p.enable_modify_skill_reuse,
        ..l2_data::SkillLoadOptions::default()
    };
    if let Ok(ini) = IniFile::load(config_dir.join(l2_config::game::PlayerConfig::FILE)) {
        // Java reads each list only when its switch is on.
        if options.enable_modify_skill_duration {
            options.skill_duration_list =
                skill_id_list("SkillDurationList", &ini.get_string("SkillDurationList", ""));
        }
        if options.enable_modify_skill_reuse {
            options.skill_reuse_list = skill_id_list("SkillReuseList", &ini.get_string("SkillReuseList", ""));
        }
    }
    options
}

/// Java's `id,value;id,value` lists: `split(";")` (trailing empty parts dropped), then
/// `split(",")` into exactly two numbers; a bad entry is logged and skipped.
fn skill_id_list(key: &str, text: &str) -> std::collections::HashMap<i32, i32> {
    let mut parts: Vec<&str> = text.split(';').collect();
    while parts.last().is_some_and(|p| p.is_empty()) {
        parts.pop();
    }
    let mut map = std::collections::HashMap::new();
    for skill in parts {
        let split: Vec<&str> = skill.split(',').collect();
        if split.len() != 2 {
            warn!("[{key}]: invalid config property -> {key} {skill}");
            continue;
        }
        match (split[0].parse::<i32>(), split[1].parse::<i32>()) {
            (Ok(id), Ok(value)) => {
                map.insert(id, value);
            }
            _ => {
                if !skill.is_empty() {
                    warn!(
                        "[{key}]: invalid config property -> SkillList \"{}\"{}",
                        split[0], split[1]
                    );
                }
            }
        }
    }
    map
}

/// The caps and multipliers the stat getters read. The raid multipliers and the class balance
/// arrays are post-processed in Java (the generated config has placeholders for them), so they
/// are read from `NPC.ini` and `Custom/ClassBalance.ini` here.
pub fn stat_settings(config: &GameConfig, config_dir: &Path) -> StatSettings {
    use l2_config::IniFile;

    let p = &config.player;
    let mut s = StatSettings {
        max_patk: p.max_patk,
        max_matk: p.max_matk,
        max_pcrit_rate: p.max_pcrit_rate,
        max_mcrit_rate: p.max_mcrit_rate,
        max_evasion: p.max_evasion,
        max_patk_speed: p.max_patk_speed,
        max_matk_speed: p.max_matk_speed,
        max_run_speed: p.max_run_speed,
        run_spd_boost: p.run_spd_boost,
        alt_weight_limit: p.alt_weight_limit,
        champion_enable: config.champion_monsters.champion_enable,
        champion_atk: config.champion_monsters.champion_atk,
        champion_spd_atk: config.champion_monsters.champion_spd_atk,
        olympiad_enchant_limit: config.olympiad.olympiad_enchant_limit,
        dawn_gates_pdef_mult: config.feature.alt_siege_dawn_gates_pdef_mult,
        dusk_gates_pdef_mult: config.feature.alt_siege_dusk_gates_pdef_mult,
        dawn_gates_mdef_mult: config.feature.alt_siege_dawn_gates_mdef_mult,
        dusk_gates_mdef_mult: config.feature.alt_siege_dusk_gates_mdef_mult,
        gm_skill_restriction: config.general.gm_skill_restriction,
        expertise_penalty: p.expertise_penalty,
        ..StatSettings::default()
    };
    // Java: `RAID_PDEFENCE_MULTIPLIER = config.getDouble("RaidPDefenceMultiplier", 100) / 100;` ...
    if let Ok(ini) = IniFile::load(config_dir.join(l2_config::game::NpcConfig::FILE)) {
        s.raid_pdefence_multiplier = ini.get_double("RaidPDefenceMultiplier", 100.0) / 100.0;
        s.raid_mdefence_multiplier = ini.get_double("RaidMDefenceMultiplier", 100.0) / 100.0;
        s.raid_pattack_multiplier = ini.get_double("RaidPAttackMultiplier", 100.0) / 100.0;
        s.raid_mattack_multiplier = ini.get_double("RaidMAttackMultiplier", 100.0) / 100.0;
    }
    if let Ok(ini) = IniFile::load(config_dir.join(l2_config::game::ClassBalanceConfig::FILE)) {
        let cb = &mut s.class_balance;
        for (key, table) in [
            (
                "PvpPhysicalAttackDefenceMultipliers",
                &mut cb.pvp_physical_attack_defence,
            ),
            (
                "PvePhysicalAttackDefenceMultipliers",
                &mut cb.pve_physical_attack_defence,
            ),
            (
                "PveMagicalSkillDefenceMultipliers",
                &mut cb.pve_magical_skill_defence,
            ),
            (
                "PvePhysicalSkillDefenceMultipliers",
                &mut cb.pve_physical_skill_defence,
            ),
            (
                "PvpMagicalSkillCriticalChanceMultipliers",
                &mut cb.pvp_magical_skill_critical_chance,
            ),
            (
                "PveMagicalSkillCriticalChanceMultipliers",
                &mut cb.pve_magical_skill_critical_chance,
            ),
        ] {
            parse_class_multipliers(key, &ini.get_string(key, ""), table);
        }
    }
    s
}

/// Java `ClassBalanceConfig.load` for one key: `id*multiplier;id*multiplier...`.
// NOTE: Java also accepts a `PlayerClass` enum constant name instead of the numeric id; the Rust
// side has no table of those names yet, so such an entry is skipped with a warning (Java would
// apply it, or throw for a name it does not know).
fn parse_class_multipliers(key: &str, text: &str, table: &mut [f32]) {
    for info in text.trim().split(';') {
        let class_info: Vec<&str> = info.trim().split('*').collect();
        if class_info.len() != 2 {
            continue;
        }
        let id = class_info[0].trim();
        let (Ok(id), Ok(value)) = (id.parse::<usize>(), class_info[1].trim().parse::<f32>()) else {
            warn!("{key}: cannot read the class multiplier '{info}'; skipped.");
            continue;
        };
        match table.get_mut(id) {
            Some(slot) => *slot = value,
            None => warn!("{key}: class id {id} is outside the table; skipped."),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn test_player() -> PlayerData {
        PlayerData {
            object_id: l2_world::ObjectId(1),
            account: "acc".into(),
            name: "Test".into(),
            title: String::new(),
            class_id: 0,
            base_class: 0,
            race: 0,
            sex: 0,
            face: 0,
            hair_style: 0,
            hair_color: 0,
            level: 1,
            exp: 0,
            sp: 0,
            karma: 0,
            pvp_kills: 0,
            pk_kills: 0,
            access_level: 0,
            x: 0,
            y: 0,
            z: 0,
            heading: 0,
            cur_hp: 1.0,
            max_hp: 1.0,
            cur_mp: 1.0,
            max_mp: 1.0,
            cur_cp: 1.0,
            max_cp: 1.0,
            online_time: 0,
            items: Vec::new(),
            skills: Vec::new(),
            shortcuts: Vec::new(),
            profile: l2_world::PlayerProfile::default(),
        }
    }

    struct NoSink;

    impl l2_world::PacketSink for NoSink {
        fn send(&self, _bytes: Vec<u8>) {}
        fn left_world(&self, _reason: l2_world::LeaveReason) {}
    }

    struct NoOutbox;

    impl l2_world::Outbox for NoOutbox {
        fn persist(&mut self, _command: l2_world::PersistCommand) {}
        fn ls_event(&mut self, _event: l2_world::LsEvent) {}
    }

    /// The `UserInfo` the world builds for `player` over `data`.
    fn user_info(data: DatapackData, player: PlayerData) -> l2_protocol_game::server::UserInfo {
        let mut world = l2_world::World::new(l2_world::WorldRng::from_seed(1), Box::new(NoOutbox));
        world.set_static_data(Box::new(data));
        let id = player.object_id;
        world.handle(l2_world::WorldCommand::EnterWorld {
            session: l2_world::SessionId(1),
            player: Box::new(player),
            sink: Box::new(NoSink),
            ip: None,
        });
        world.user_info(id).unwrap()
    }

    #[test]
    fn stat_bonus_reads_every_section_by_name() {
        let table = stat_bonus_from_xml(
            r#"<list>
	<STR><stat value="40" bonus="1.2" /></STR>
	<INT><stat value="2" bonus="0.5" /></INT>
	<CON><stat value="24" bonus="0.9" /><stat value="x" bonus="1" /></CON>
	<MEN><stat value="3" bonus="2" /></MEN>
	<DEX><stat value="24" bonus="1.04" /><stat value="100" bonus="9" /></DEX>
	<WIT><stat value="23" bonus="1.26" /></WIT>
	<LUC><stat value="1" bonus="3" /></LUC>
</list>"#,
        );
        assert_eq!(table.bonus(BaseStat::Str, 40), 1.2);
        assert_eq!(table.bonus(BaseStat::Int, 2), 0.5);
        assert_eq!(table.bonus(BaseStat::Con, 24), 0.9);
        assert_eq!(table.bonus(BaseStat::Men, 3), 2.0);
        assert_eq!(table.bonus(BaseStat::Dex, 24), 1.04);
        assert_eq!(table.bonus(BaseStat::Wit, 23), 1.26);
        // Unlisted, out of range and negative values are 0 (Java's arrays start at 0).
        assert_eq!(table.bonus(BaseStat::Str, 41), 0.0);
        assert_eq!(table.bonus(BaseStat::Dex, 100), 0.0);
        assert_eq!(table.bonus(BaseStat::Dex, -1), 0.0);
    }

    #[test]
    fn class_balance_entries_are_id_star_multiplier() {
        let mut table = vec![1.0_f32; 137];
        parse_class_multipliers("K", " 0*1.5; 88*0.9;bad;Duelist*2;500*3;7*x ", &mut table);
        assert_eq!((table[0], table[88], table[1]), (1.5, 0.9, 1.0));
        parse_class_multipliers("K", "", &mut table);
        assert_eq!(table[0], 1.5);
    }

    #[test]
    fn access_levels_use_the_hex_colors_and_the_java_default() {
        let levels = AccessLevels::from_xml(
            r#"<list>
	<access level="-1" name="Banned" nameColor="FFFFFF" titleColor="ECF9A2" isGM="false" />
	<access level="0" name="User" nameColor="FFFFFF" titleColor="ECF9A2" isGM="false" />
	<access level="100" name="Master" nameColor="00CCFF" titleColor="0FF000" isGM="true" />
</list>"#,
        );
        assert_eq!(levels.get(0), AccessLevelStatic::USER);
        let master = levels.get(100);
        assert_eq!(
            (master.name_color, master.title_color, master.is_gm),
            (0x00CCFF, 0x0FF000, true)
        );
        // Below zero is the banned level; a level the file lacks is Java's default AccessLevel.
        assert_eq!(levels.get(-5), levels.get(-1));
        assert_eq!(levels.get(55), AccessLevelStatic::UNREGISTERED);
        assert!(!levels.get(55).is_gm);
    }

    #[test]
    fn admin_rights_come_from_the_two_config_files() {
        let mut table = AccessLevels::from_xml(
            r#"<list>
	<access level="0" name="User" isGM="false" />
	<access level="30" name="General GM" childAccess="20" isGM="false" />
	<access level="70" name="Admin" childAccess="30" isGM="true" />
	<access level="100" name="Master" childAccess="70" isGM="true" />
</list>"#,
        );
        table.add_commands_xml(
            r#"<list>
	<admin command="admin" description="Open main Admin menu." accessLevel="30" />
	<admin command="kill" description="Kill." />
	<admin command="recall_char_menu" accessLevel="100" confirmDlg="true" />
</list>"#,
        );
        let rights = table.rights();
        assert_eq!((rights.level_count(), rights.command_count()), (4, 3));
        assert!(rights.has_access("admin_admin", 100));
        assert!(rights.has_access("admin_admin", 30));
        assert!(!rights.has_access("admin_admin", 0));
        // `accessLevel` defaults to 100.
        assert!(rights.has_access("admin_kill", 100));
        assert!(!rights.has_access("admin_kill", 70));
        assert!(rights.requires_confirm("admin_recall_char_menu", 100));
        assert!(!rights.requires_confirm("admin_kill", 100));
    }

    #[test]
    fn the_shipped_admin_files_give_a_master_every_listed_command() {
        let root = std::env::var_os("L2JMOBIUS_GAME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\l2-rust\l2jmobius\dist\game"));
        let config = root.join("config");
        if !config.join(AccessLevels::COMMANDS_FILE).is_file() {
            eprintln!("skipped: {} has no admin config", config.display());
            return;
        }
        let table = AccessLevels::load(&config);
        let rights = table.rights();
        assert_eq!(rights.level_count(), 10);
        assert!(rights.command_count() > 400);
        // Master (100) reaches Admin (70) down to General GM (30); the plain user nothing.
        for command in [
            "admin_admin",
            "admin_spawn",
            "admin_move_to",
            "admin_kill",
            "admin_heal",
        ] {
            assert!(rights.has_access(command, 100), "{command}");
            assert!(!rights.has_access(command, 0), "{command}");
        }
        assert!(rights.has_access("admin_goto", 70));
        assert!(!rights.has_access("admin_kill", 70));
        // Only Admin (70) and Master (100) are GMs.
        assert!(table.get(70).is_gm && table.get(100).is_gm);
        assert!(!table.get(60).is_gm);
    }

    #[test]
    fn stats_follow_the_base_formulas() {
        let (_dir, datapack) = crate::client::testdata::datapack();
        let mut bonus = BaseStatBonus::loaded();
        for (stat, value, b) in [
            (BaseStat::Str, 40, 1.2),
            (BaseStat::Int, 21, 1.0),
            (BaseStat::Dex, 30, 1.5),
            (BaseStat::Wit, 11, 0.5),
            (BaseStat::Con, 43, 1.5),
            (BaseStat::Men, 25, 1.28),
        ] {
            assert!(bonus.set(stat, value, b));
        }
        let data = || DatapackData::new(Arc::clone(&datapack), bonus.clone(), AccessLevels::default());
        let mut player = test_player();
        // Human fighter of the test datapack: STR 40 CON 43 DEX 30 INT 21 WIT 11 MEN 25, P. Atk. 4.
        player.class_id = 0;
        player.level = 11;
        let s = user_info(data(), player.clone());
        assert_eq!(
            (s.str, s.dex, s.con, s.int, s.wit, s.men),
            (40, 30, 43, 21, 11, 25)
        );
        // FuncPAtkMod: (4 * 1.2 * levelMod 1.0) truncated
        assert_eq!(s.p_atk, 4);
        // weight limit: floor(1.5 * 69000)
        assert_eq!(s.max_load, 103_500);
        // sqrt(30) * 6 + level 11 = 43.86
        assert_eq!(s.evasion, 44);
        assert_eq!(s.accuracy, 44);
        assert_eq!(s.collision_radius, 9.0);
        player.sex = 1;
        assert_eq!(user_info(data(), player.clone()).collision_radius, 8.0);
        // Above level 69/77 the accuracy and evasion get their extra points.
        player.level = 80;
        let s = user_info(data(), player);
        // accuracy: sqrt(30)*6 + 80 + (80-76) + (80-69) = 127.86; evasion: + (80-69)*1.2 = 126.06
        assert_eq!((s.accuracy, s.evasion), (128, 126));
    }

    /// The numbers of the recorded level 1 Elven Mystic (STR 21 DEX 24 CON 25 INT 37 WIT 23 MEN 40)
    /// against the real datapack. Skipped without the reference folder.
    #[test]
    fn the_real_datapack_gives_the_stats_of_the_recorded_character() {
        let root = std::env::var_os("L2JMOBIUS_GAME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\l2-rust\l2jmobius\dist\game"));
        if !root.join("data/stats/statBonus.xml").is_file() {
            eprintln!("skipped: {} has no datapack", root.display());
            return;
        }
        let datapack = Arc::new(Datapack::load(&root).unwrap());
        let load = || {
            DatapackData::new(
                Arc::clone(&datapack),
                load_stat_bonus(&root),
                AccessLevels::from_xml(
                    &std::fs::read_to_string(root.join("config/AccessLevels.xml")).unwrap(),
                ),
            )
        };
        let data = load();

        // The passive flags against the SKILL_LIST Java sent (194, 163, 118, 214 are passive).
        for (id, passive) in [
            (194, true),
            (163, true),
            (118, true),
            (214, true),
            (1216, false),
            (1177, false),
            (1322, false),
        ] {
            assert_eq!(data.skill_is_passive(id), passive, "skill {id}");
        }

        let mut player = test_player();
        player.class_id = 25;
        player.base_class = 25;
        player.level = 1;
        player.cur_hp = 1_000.0;
        player.cur_mp = 1_000.0;
        player.cur_cp = 1_000.0;
        let s = user_info(load(), player);
        assert_eq!(
            (s.str, s.dex, s.con, s.int, s.wit, s.men),
            (21, 24, 25, 37, 23, 40)
        );
        // Values of the recording that no equipment or skill touches.
        assert_eq!(
            (s.run_speed, s.walk_speed, s.swim_run_speed, s.swim_walk_speed),
            (122, 85, 50, 50)
        );
        // `getMoveSpeed() * (1. / baseSpeed)`: (122 * DEX 24 bonus 1.04) * (1 / 122).
        assert_eq!(s.move_multiplier, (122.0 * 1.04) * (1.0 / 122.0));
        assert_eq!(s.max_load, 64_170);
        assert_eq!(s.evasion, 30);
        assert_eq!((s.collision_radius, s.collision_height), (7.5, 24.0));
        assert_eq!(s.critical, 42);
        // Hand-checked from the Java formulas (to be confirmed by the M4-JV-01 vectors), naked:
        // max HP (int) (104 * CON 25 bonus 0.93) = 96, MP (int) (40 * MEN 1.49) = 59,
        // CP (int) (52 * 0.93) = 48 (the stored 1000 is clamped to these);
        // P. Atk. 3 * STR 0.61 * 0.9 = 1.647, M. Atk. 6 * 1.12^2 * 0.9^2 = 6.096,
        // P. Def. 54 * 0.9 = 48.6, M. Def. 41 * 1.49 * 0.9 = 54.98,
        // accuracy round(sqrt(24) * 6 + 1) = 30, Atk. Spd. round(300 * 1.04) = 312,
        // Casting Spd. (int) (333 * WIT 1.16) = 386.
        assert_eq!((s.max_hp, s.max_mp, s.max_cp), (96, 59, 48));
        assert_eq!((s.current_hp, s.current_mp, s.current_cp), (96, 59, 48));
        assert_eq!((s.p_atk, s.m_atk, s.p_def, s.m_def), (1, 6, 48, 54));
        assert_eq!((s.accuracy, s.p_atk_spd, s.m_atk_spd), (30, 312, 386));
        // The exp bar of level 1 with no exp.
        assert_eq!(data.exp_percent(0, 1), 0.0);
        // Items: the Apprentice's Wand (6) is a weapon, the Tutorial Guide (5588) is no quest item.
        let wand = data.item(6).unwrap();
        assert!(wand.weapon && wand.type2 == 0);
        assert_eq!(wand.body_part, 0x0080);
    }
}
