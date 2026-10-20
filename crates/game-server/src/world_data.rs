//! The datapack as the world sees it ([`l2_world::StaticData`], including the stat engine's
//! tables), and the settings the world reads from the configuration.
// Java: mechanics/stats/BaseStat.java (the statBonus.xml loader)
// Java: data/xml/{AdminData,SkillData}.java, data/holders/AccessLevel.java
// Java: config/{PlayerConfig,NpcConfig,ChampionMonstersConfig,OlympiadConfig,FeatureConfig}.java,
//       config/custom/ClassBalanceConfig.java (the values the stat getters and the attack read)
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
    AccessLevelStatic, AdminRights, BaseStat, BaseStatBonus, ChatBroadcast, CombatSettings, DeathSettings,
    DropSettings, ExpSettings, GroundItemSettings, ItemStatic, PlayerData, RegenSettings, StatData,
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
    /// The levels with `allowFixedRes="true"` (Java `AccessLevel.allowFixedRes()`).
    fixed_res: std::collections::BTreeSet<i32>,
    /// The levels with `takeAggro="false"`: monsters do not aggress their players (Java
    /// `AccessLevel.canTakeAggro()`). A level not listed takes aggro.
    no_take_aggro: std::collections::BTreeSet<i32>,
    /// The levels with `gainExp="false"` (Java `AccessLevel.canGainExp()`, default true).
    no_gain_exp: std::collections::BTreeSet<i32>,
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
                    if attribute(&e, "allowFixedRes").is_some_and(|v| v.eq_ignore_ascii_case("true")) {
                        table.fixed_res.insert(level);
                    }
                    // Java: `set.getBoolean("takeAggro", true)`, which is `Boolean.parseBoolean`
                    // for a present value: anything but "true" is false.
                    if attribute(&e, "takeAggro").is_some_and(|v| !v.eq_ignore_ascii_case("true")) {
                        table.no_take_aggro.insert(level);
                    }
                    if attribute(&e, "gainExp").is_some_and(|v| v.eq_ignore_ascii_case("false")) {
                        table.no_gain_exp.insert(level);
                    }
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

    /// Java `AdminData.getAccessLevel(level).allowFixedRes()`.
    pub fn allows_fixed_res(&self, level: i32) -> bool {
        let key = if level < 0 { -1 } else { level };
        self.fixed_res.contains(&key)
    }

    /// Java `AdminData.getAccessLevel(level).canTakeAggro()`.
    pub fn can_take_aggro(&self, level: i32) -> bool {
        let key = if level < 0 { -1 } else { level };
        !self.no_take_aggro.contains(&key)
    }

    /// Java `AdminData.getAccessLevel(level).canGainExp()`.
    pub fn can_gain_exp(&self, level: i32) -> bool {
        let key = if level < 0 { -1 } else { level };
        !self.no_gain_exp.contains(&key)
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

    fn item_allows_self_resurrection(&self, item_id: i32) -> bool {
        self.datapack
            .items
            .get(item_id)
            .is_some_and(|t| t.allow_self_resurrection)
    }

    fn access_level_allows_fixed_res(&self, level: i32) -> bool {
        self.access.allows_fixed_res(level)
    }

    fn can_take_aggro(&self, level: i32) -> bool {
        self.access.can_take_aggro(level)
    }

    fn access_level_can_gain_exp(&self, level: i32) -> bool {
        self.access.can_gain_exp(level)
    }

    fn augmentation_option(&self, id: i32) -> Option<&l2_data::AugmentationOption> {
        self.datapack.augmentation_options.get(id)
    }
}

/// The values exp, SP and levels read (Java `RatesConfig`, `PlayerConfig`). `MaxSp` is a `long`
/// the generated config keeps as a placeholder, so it comes from `Player.ini` here.
pub fn exp_settings(config: &GameConfig, config_dir: &Path) -> ExpSettings {
    use l2_config::IniFile;

    let r = &config.rates;
    let p = &config.player;
    let mut settings = ExpSettings {
        rate_xp: r.rate_xp,
        rate_sp: r.rate_sp,
        monster_exp_max_level_difference: r.monster_exp_max_level_difference,
        // Java: `if (RATE_KARMA_LOST == -1) RATE_KARMA_LOST = RATE_XP;`
        rate_karma_lost: if r.rate_karma_lost == -1.0 {
            r.rate_xp
        } else {
            r.rate_karma_lost
        },
        rate_karma_exp_lost: r.rate_karma_exp_lost,
        player_delevel: p.player_delevel,
        decrease_skill_level: p.decrease_skill_level,
        auto_learn_skills: p.auto_learn_skills,
        max_bonus_exp: p.max_bonus_exp,
        max_bonus_sp: p.max_bonus_sp,
        alt_party_range: p.alt_party_range,
        ..ExpSettings::default()
    };
    if let Ok(ini) = IniFile::load(config_dir.join(l2_config::game::PlayerConfig::FILE)) {
        // Java: `MAX_SP = getLong("MaxSp", 50000000000L) >= 0 ? that : Long.MAX_VALUE`.
        let max_sp = ini.get_long("MaxSp", 50_000_000_000);
        settings.max_sp = if max_sp >= 0 { max_sp } else { i64::MAX };
    }
    settings
}

/// The values parties read (Java `PlayerConfig`, `RatesConfig`, `ClassBalanceConfig`). The cut-off
/// method and its two lists are placeholders in the generated config, so they come from
/// `Player.ini`; the exp and SP amount multipliers from `Custom/ClassBalance.ini`.
pub fn party_settings(config: &GameConfig, config_dir: &Path) -> l2_world::PartySettings {
    use l2_config::IniFile;
    use l2_world::{PartyExpType, PartySettings};

    let r = &config.rates;
    let p = &config.player;
    let mut settings = PartySettings {
        alt_party_range: p.alt_party_range,
        alt_leave_party_leader: p.alt_leave_party_leader,
        rate_party_xp: r.rate_party_xp,
        rate_party_sp: r.rate_party_sp,
        xp_cutoff_percent: p.party_xp_cutoff_percent,
        xp_cutoff_level: p.party_xp_cutoff_level,
        ..PartySettings::default()
    };
    if let Ok(ini) = IniFile::load(config_dir.join(l2_config::game::PlayerConfig::FILE)) {
        // Java: `Enum.valueOf(PartyExpType.class, getString("PartyXpCutoffMethod", "LEVEL").toUpperCase())`.
        let method = ini.get_string("PartyXpCutoffMethod", "LEVEL");
        match PartyExpType::from_name(&method) {
            Some(method) => settings.xp_cutoff_method = method,
            None => warn!("PartyXpCutoffMethod = {method} is not a party exp type; using LEVEL."),
        }
        let gaps = ini.get_string("PartyXpCutoffGaps", "0,9;10,14;15,99");
        match parse_party_gaps(&gaps) {
            Some(gaps) => settings.xp_cutoff_gaps = gaps,
            None => warn!("PartyXpCutoffGaps = {gaps} is not a list of level pairs; using the default."),
        }
        let percents = ini.get_string("PartyXpCutoffGapPercent", "100;30;0");
        match parse_party_percents(&percents) {
            Some(percents) => settings.xp_cutoff_gap_percents = percents,
            None => {
                warn!("PartyXpCutoffGapPercent = {percents} is not a list of numbers; using the default.")
            }
        }
    }
    if let Ok(ini) = IniFile::load(config_dir.join(l2_config::game::ClassBalanceConfig::FILE)) {
        for (key, table) in [
            ("ExpAmountMultipliers", &mut settings.exp_amount_multipliers),
            ("SpAmountMultipliers", &mut settings.sp_amount_multipliers),
        ] {
            parse_class_multipliers(key, &ini.get_string(key, ""), table);
        }
    }
    settings
}

/// Java `PARTY_XP_CUTOFF_GAPS`: `"0,9;10,14;15,99"` is three level ranges. `None` where Java's
/// `Integer.parseInt` would throw.
fn parse_party_gaps(text: &str) -> Option<Vec<[i32; 2]>> {
    text.split(';')
        .map(|gap| {
            let mut parts = gap.split(',');
            let low = parts.next()?.parse().ok()?;
            let high = parts.next()?.parse().ok()?;
            Some([low, high])
        })
        .collect()
}

/// Java `PARTY_XP_CUTOFF_GAP_PERCENTS`: `"100;30;0"`.
fn parse_party_percents(text: &str) -> Option<Vec<i32>> {
    text.split(';').map(|percent| percent.parse().ok()).collect()
}

/// The values vitality reads (Java `PlayerConfig`, `RatesConfig`).
pub fn vitality_settings(config: &GameConfig) -> l2_world::VitalitySettings {
    let r = &config.rates;
    let p = &config.player;
    l2_world::VitalitySettings {
        enable_vitality: p.enable_vitality,
        recover_vitality_on_reconnect: p.recover_vitality_on_reconnect,
        raidboss_use_vitality: p.raidboss_use_vitality,
        rate_vitality_level: [
            r.rate_vitality_level_1,
            r.rate_vitality_level_2,
            r.rate_vitality_level_3,
            r.rate_vitality_level_4,
        ],
        rate_vitality_gain: r.rate_vitality_gain,
        rate_vitality_lost: r.rate_vitality_lost,
        rate_recovery_vitality_peace_zone: r.rate_recovery_vitality_peace_zone,
        rate_recovery_on_reconnect: r.rate_recovery_on_reconnect,
    }
}

/// The values Nevit's blessing reads (Java `PlayerConfig`; `NevitEnabled` is
/// `WorldSettings::nevit_enabled`).
pub fn nevit_settings(config: &GameConfig) -> l2_world::NevitSettings {
    let p = &config.player;
    l2_world::NevitSettings {
        max_points: p.nevit_max_points,
        bonus_effect_time: p.nevit_bonus_effect_time,
        advent_time: p.nevit_advent_time,
        ignore_advent_time: p.nevit_ignore_advent_time,
    }
}

/// The values dying and reviving read. `RespawnRestoreCP/HP/MP` are `()` placeholders in the
/// generated config (Java divides them by 100 after reading), so they come from `Player.ini`.
pub fn death_settings(config: &GameConfig, config_dir: &Path) -> DeathSettings {
    use l2_config::IniFile;

    let mut s = DeathSettings {
        disconnect_after_death: config.player.disconnect_after_death,
        default_corpse_time: config.npc.default_corpse_time,
        spoiled_corpse_extend_time: config.npc.spoiled_corpse_extend_time,
        corpse_consume_skill_allowed_time_before_decay: config
            .npc
            .corpse_consume_skill_allowed_time_before_decay,
        ..DeathSettings::default()
    };
    if let Ok(ini) = IniFile::load(config_dir.join(l2_config::game::PlayerConfig::FILE)) {
        s.respawn_restore_cp = ini.get_double("RespawnRestoreCP", 0.0) / 100.0;
        s.respawn_restore_hp = ini.get_double("RespawnRestoreHP", 65.0) / 100.0;
        s.respawn_restore_mp = ini.get_double("RespawnRestoreMP", 0.0) / 100.0;
    }
    s
}

/// Java `Integer.parseInt` of every piece of a comma separated list; a piece that is not a number
/// is skipped with a warning (Java throws while it loads the configuration).
fn parse_int_list(key: &str, text: &str) -> Vec<i32> {
    text.split(',')
        .filter_map(|piece| match piece.trim().parse::<i32>() {
            Ok(id) => Some(id),
            Err(_) => {
                warn!("{key}: \"{piece}\" is not an item id; skipped.");
                None
            }
        })
        .collect()
}

/// The values ground items read (Java `GeneralConfig` and `PlayerConfig`). The herb destroy time,
/// the protected item list, the save interval and the auto loot item list are post-processed in
/// Java (placeholders in the generated config), so they are read from the files here.
pub fn ground_item_settings(config: &GameConfig, config_dir: &Path) -> GroundItemSettings {
    use l2_config::IniFile;
    use l2_config::game::GeneralConfig;

    let g = &config.general;
    let p = &config.player;
    let mut s = GroundItemSettings {
        autodestroy_item_after: g.autodestroy_item_after,
        destroy_dropped_player_item: g.destroy_dropped_player_item,
        destroy_equipable_player_item: g.destroy_equipable_player_item,
        save_dropped_item: g.save_dropped_item,
        multiple_item_drop: g.multiple_item_drop,
        auto_loot: p.auto_loot,
        auto_loot_raids: p.auto_loot_raids,
        auto_loot_herbs: p.auto_loot_herbs,
        max_adena: config.effective_max_adena(),
        ..GroundItemSettings::default()
    };
    if let Ok(ini) = IniFile::load(config_dir.join(GeneralConfig::FILE)) {
        // `HERB_AUTO_DESTROY_TIME = getInt("AutoDestroyHerbTime", 60) * 1000`.
        s.herb_auto_destroy_time = ini.get_int("AutoDestroyHerbTime", 60).wrapping_mul(1000);
        s.list_protected_items = parse_int_list(
            "ListOfProtectedItems",
            &ini.get_string("ListOfProtectedItems", "0"),
        );
        // `SAVE_DROPPED_ITEM_INTERVAL = getInt("SaveDroppedItemInterval", 60) * 60000`.
        s.save_dropped_item_interval_ms = i64::from(ini.get_int("SaveDroppedItemInterval", 60)) * 60_000;
    }
    if let Ok(ini) = IniFile::load(config_dir.join(l2_config::game::PlayerConfig::FILE)) {
        // 0 and unparsable entries are left out of `AUTO_LOOT_ITEM_IDS`.
        s.auto_loot_item_ids = parse_int_list("AutoLootItemIds", &ini.get_string("AutoLootItemIds", "0"))
            .into_iter()
            .filter(|id| *id != 0)
            .collect();
    }
    s
}

/// `Map<Integer, Float>` of `Rates.ini`: `id,value;id,value` (Java's `DropAmountMultiplierByItemId`
/// and `DropChanceMultiplierByItemId`). A malformed pair is skipped with a warning.
fn parse_item_multipliers(key: &str, text: &str) -> std::collections::BTreeMap<i32, f32> {
    let mut map = std::collections::BTreeMap::new();
    if text.is_empty() {
        return map;
    }
    for item in text.split(';') {
        let parts: Vec<&str> = item.split(',').collect();
        let parsed = match parts.as_slice() {
            [id, value] => id.parse::<i32>().ok().zip(value.parse::<f32>().ok()),
            _ => None,
        };
        match parsed {
            Some((id, value)) => {
                map.insert(id, value);
            }
            None => {
                if !item.is_empty() {
                    warn!("Config.load(): invalid config property -> {key} \"{item}\"");
                }
            }
        }
    }
    map
}

/// `BossDropList` of `Rates.ini`: `itemId,min,max,chance;...` (Java `Integer.parseInt` for the
/// first three, `Double.parseDouble` for the chance). A malformed entry is skipped with a warning.
fn parse_boss_drop_list(text: &str) -> Vec<l2_data::npc::DropEntry> {
    let mut list = Vec::new();
    for s in text.trim().split(';') {
        if s.is_empty() {
            continue;
        }
        let parts: Vec<&str> = s.split(',').collect();
        let entry = match parts.as_slice() {
            [id, min, max, chance, ..] => (|| {
                Some(l2_data::npc::DropEntry {
                    drop_type: l2_data::npc::DropType::Drop,
                    item_id: id.parse().ok()?,
                    min: i64::from(min.parse::<i32>().ok()?),
                    max: i64::from(max.parse::<i32>().ok()?),
                    chance: chance.parse().ok()?,
                })
            })(),
            _ => None,
        };
        match entry {
            Some(entry) => list.push(entry),
            None => warn!("BossDropList: \"{s}\" is not itemId,min,max,chance; skipped."),
        }
    }
    list
}

/// The drop rates and level gap rules (Java `RatesConfig`), `AltPartyRange` and the karma drop
/// lists (`PvpConfig`). The per-item multiplier maps and the boss list are post-processed in Java
/// (placeholders in the generated config), so they are read from `Rates.ini` here.
pub fn drop_settings(config: &GameConfig, config_dir: &Path) -> DropSettings {
    use l2_config::IniFile;

    let r = &config.rates;
    let mut s = DropSettings {
        rate_death_drop_amount_multiplier: r.rate_death_drop_amount_multiplier,
        rate_spoil_drop_amount_multiplier: r.rate_spoil_drop_amount_multiplier,
        rate_herb_drop_amount_multiplier: r.rate_herb_drop_amount_multiplier,
        rate_raid_drop_amount_multiplier: r.rate_raid_drop_amount_multiplier,
        rate_death_drop_chance_multiplier: r.rate_death_drop_chance_multiplier,
        rate_spoil_drop_chance_multiplier: r.rate_spoil_drop_chance_multiplier,
        rate_herb_drop_chance_multiplier: r.rate_herb_drop_chance_multiplier,
        rate_raid_drop_chance_multiplier: r.rate_raid_drop_chance_multiplier,
        drop_max_occurrences_normal: r.drop_max_occurrences_normal,
        drop_max_occurrences_raidboss: r.drop_max_occurrences_raidboss,
        drop_adena_min_level_difference: r.drop_adena_min_level_difference,
        drop_adena_max_level_difference: r.drop_adena_max_level_difference,
        drop_adena_min_level_gap_chance: r.drop_adena_min_level_gap_chance,
        drop_item_min_level_difference: r.drop_item_min_level_difference,
        drop_item_max_level_difference: r.drop_item_max_level_difference,
        drop_item_min_level_gap_chance: r.drop_item_min_level_gap_chance,
        boss_drop_enabled: r.boss_drop_enabled,
        boss_drop_min_level: r.boss_drop_min_level,
        boss_drop_max_level: r.boss_drop_max_level,
        alt_party_range: config.player.alt_party_range,
        player_drop_limit: r.player_drop_limit,
        player_rate_drop: r.player_rate_drop,
        player_rate_drop_item: r.player_rate_drop_item,
        player_rate_drop_equip: r.player_rate_drop_equip,
        player_rate_drop_equip_weapon: r.player_rate_drop_equip_weapon,
        karma_drop_limit: r.karma_drop_limit,
        karma_rate_drop: r.karma_rate_drop,
        karma_rate_drop_item: r.karma_rate_drop_item,
        karma_rate_drop_equip: r.karma_rate_drop_equip,
        karma_rate_drop_equip_weapon: r.karma_rate_drop_equip_weapon,
        karma_drop_gm: config.pvp.karma_drop_gm,
        karma_pk_limit: config.pvp.karma_pk_limit,
        ..DropSettings::default()
    };
    // `Arrays.sort(KARMA_LIST_NONDROPPABLE_ITEMS)`.
    let mut nondroppable = parse_int_list("ListOfNonDroppableItems", &config.pvp.karma_nondroppable_items);
    nondroppable.sort_unstable();
    s.karma_list_nondroppable_items = nondroppable;
    if let Ok(ini) = IniFile::load(config_dir.join(l2_config::game::RatesConfig::FILE)) {
        s.rate_drop_amount_by_id = parse_item_multipliers(
            "DropAmountMultiplierByItemId",
            &ini.get_string("DropAmountMultiplierByItemId", ""),
        );
        s.rate_drop_chance_by_id = parse_item_multipliers(
            "DropChanceMultiplierByItemId",
            &ini.get_string("DropChanceMultiplierByItemId", ""),
        );
        s.boss_drop_list = parse_boss_drop_list(&ini.get_string("BossDropList", ""));
    }
    s
}

/// The regeneration multipliers the world reads (Java `PlayerConfig` and `NpcConfig`, after the
/// `/ 100` of their post-processing; see `l2_config::GameManual`).
pub fn regen_settings(config: &GameConfig) -> RegenSettings {
    RegenSettings {
        hp_regen_multiplier: config.manual.player.hp_regen_multiplier,
        mp_regen_multiplier: config.manual.player.mp_regen_multiplier,
        cp_regen_multiplier: config.manual.player.cp_regen_multiplier,
        raid_hp_regen_multiplier: config.manual.npc.raid_hp_regen_multiplier,
        raid_mp_regen_multiplier: config.manual.npc.raid_mp_regen_multiplier,
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
        gm_startup_invulnerable: config.general.gm_startup_invulnerable,
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
        combat: combat_settings(config, config_dir),
        death: death_settings(config, config_dir),
        ground: ground_item_settings(config, config_dir),
        drops: drop_settings(config, config_dir),
        exp: exp_settings(config, config_dir),
        party: party_settings(config, config_dir),
        vitality: vitality_settings(config),
        nevit: nevit_settings(config),
        effects: l2_world::EffectSettings {
            buffs_max_amount: i32::from(p.buffs_max_amount),
            triggered_buffs_max_amount: i32::from(p.triggered_buffs_max_amount),
            dances_max_amount: i32::from(p.dances_max_amount),
            effect_tick_ratio: p.effect_tick_ratio,
            show_effect_messages_on_login: p.show_effect_messages_on_login,
            debuff_duration_uses_resists: config.general.debuff_duration_uses_resists,
            fake_death_up_protection: p.player_fakedeath_up_protection,
            fake_death_untarget: p.fake_death_untarget,
            fake_death_damage_stand: p.fake_death_damage_stand,
            store_skill_cooltime: p.store_skill_cooltime,
            alt_store_toggles: p.alt_store_toggles,
            alt_store_dances: p.alt_store_dances,
            alt_validate_trigger_skills: p.alt_validate_trigger_skills,
            cancel_return_on: config.cancel_return.cancel_return_on,
            cancel_return_mob: config.cancel_return.cancel_return_mob,
            cancel_return_player: config.cancel_return.cancel_return_player,
            cancel_return_player_olys: config.cancel_return.cancel_return_player_olys,
            // `TimeToReturn` is post-processed (seconds -> ms): read from the file below.
            ..l2_world::EffectSettings::default()
        },
        learn: l2_world::LearnSettings {
            alt_game_skill_learn: p.alt_game_skill_learn,
            divine_sp_book_needed: p.divine_sp_book_needed,
            auto_learn_skills_without_items: p.auto_learn_skills_without_items,
            auto_learn_fs_skills: p.auto_learn_fs_skills,
            auto_learn_divine_inspiration: p.auto_learn_divine_inspiration,
        },
        talk: TalkSettings {
            karma_player_can_shop: p.alt_game_karma_player_can_shop,
            karma_player_can_use_gk: p.alt_game_karma_player_can_use_gk,
            karma_player_can_use_warehouse: p.alt_game_karma_player_can_use_warehouse,
            max_free_teleport_level: p.max_free_teleport_level,
            teleport_while_siege_in_progress: p.teleport_while_siege_in_progress,
            gm_debug_html_paths: config.general.gm_debug_html_paths,
            ..TalkSettings::default()
        },
        inventory: l2_world::InventorySettings {
            allow_discard_item: config.general.allow_discarditem,
            destroy_all_items: config.general.destroy_all_items,
            gm_trade_restricted_items: config.general.gm_trade_restricted_items,
            gm_item_restriction: config.general.gm_item_restriction,
            auto_loot_slot_limit: p.auto_loot_slot_limit,
            rate_extractable: config.rates.rate_extractable,
            alt_allow_augment_trade: p.alt_allow_augment_trade,
            alt_allow_augment_destroy: p.alt_allow_augment_destroy,
        },
        cast: l2_world::CastSettings {
            alt_game_karma_player_can_teleport: p.alt_game_karma_player_can_teleport,
            dance_consume_additional_mp: p.dance_consume_additional_mp,
            raid_disable_curse: config.npc.raid_disable_curse,
            disable_raidboss_heal_from_players: config.grand_boss.disable_raidboss_heal_from_players,
            disable_raidboss_buff_from_players: config.grand_boss.disable_raidboss_buff_from_players,
            ..l2_world::CastSettings::default()
        },
        ..WorldSettings::default()
    };
    // `SkillReuseMultipliers` is a post-processed list in the generated config, read from
    // `ClassBalance.ini` like the other class multipliers.
    if let Ok(ini) = IniFile::load(config_dir.join(l2_config::game::ClassBalanceConfig::FILE)) {
        let key = "SkillReuseMultipliers";
        parse_class_multipliers(
            key,
            &ini.get_string(key, ""),
            &mut settings.cast.skill_reuse_multipliers,
        );
    }
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
    // `TimeToReturn` (seconds, Java keeps ms) is a hand-port placeholder in the generated config.
    if let Ok(ini) = IniFile::load(config_dir.join(l2_config::game::CancelReturnConfig::FILE)) {
        settings.effects.cancel_return_time_ms = ini.get_int("TimeToReturn", 10).saturating_mul(1000);
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
    settings.daily_reset_database_writes = daily_reset_database_writes(config_dir);
    settings
}

/// Name of the Rust-only side-by-side settings file in the config folder (D-36).
pub const SIDE_BY_SIDE_FILE: &str = "SideBySide.ini";

/// `DailyResetDatabaseWrites` of `config/SideBySide.ini` (default False; a missing file means the
/// default). Off, the daily reset leaves the global `character_reco_bonus` `UPDATE`s to the Java
/// game server that shares the database (D-39); on, this server runs them (it runs alone).
/// Logs the active mode.
pub fn daily_reset_database_writes(config_dir: &Path) -> bool {
    use l2_config::IniFile;

    let path = config_dir.join(SIDE_BY_SIDE_FILE);
    let on = if path.exists() {
        match IniFile::load(&path) {
            Ok(ini) => ini.get_bool("DailyResetDatabaseWrites", false),
            Err(e) => {
                warn!(
                    "Cannot read {}: {e}; using DailyResetDatabaseWrites = False.",
                    path.display()
                );
                false
            }
        }
    } else {
        false
    };
    if on {
        info!(
            "SideBySide: DailyResetDatabaseWrites = True, the daily reset updates character_reco_bonus for every character."
        );
    } else {
        info!(
            "SideBySide: DailyResetDatabaseWrites = False, the daily reset leaves the character_reco_bonus UPDATEs to the Java game server."
        );
    }
    on
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
        load_class_balance(&ini, &mut s.class_balance);
    }
    s
}

/// Java `ClassBalanceConfig.load`: every `*Multipliers` key of `Custom/ClassBalance.ini` that the
/// stat settings hold (`SkillReuseMultipliers` is in `CastSettings`, the Exp/Sp amounts belong to
/// the party). Each table starts at 1.0 and the entries of the key replace it.
fn load_class_balance(ini: &l2_config::IniFile, cb: &mut l2_world::stats::ClassBalance) {
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
        (
            "PvpPhysicalSkillDefenceMultipliers",
            &mut cb.pvp_physical_skill_defence,
        ),
        (
            "PvpPhysicalAttackCriticalChanceMultipliers",
            &mut cb.pvp_physical_attack_critical_chance,
        ),
        (
            "PvePhysicalAttackCriticalChanceMultipliers",
            &mut cb.pve_physical_attack_critical_chance,
        ),
        (
            "PvpPhysicalAttackCriticalDamageMultipliers",
            &mut cb.pvp_physical_attack_critical_damage,
        ),
        (
            "PvePhysicalAttackCriticalDamageMultipliers",
            &mut cb.pve_physical_attack_critical_damage,
        ),
        (
            "PvpPhysicalAttackDamageMultipliers",
            &mut cb.pvp_physical_attack_damage,
        ),
        (
            "PvePhysicalAttackDamageMultipliers",
            &mut cb.pve_physical_attack_damage,
        ),
        (
            "PveMagicalSkillDamageMultipliers",
            &mut cb.pve_magical_skill_damage,
        ),
        (
            "PvpMagicalSkillDamageMultipliers",
            &mut cb.pvp_magical_skill_damage,
        ),
        (
            "PvpMagicalSkillDefenceMultipliers",
            &mut cb.pvp_magical_skill_defence,
        ),
        (
            "PveMagicalSkillCriticalDamageMultipliers",
            &mut cb.pve_magical_skill_critical_damage,
        ),
        (
            "PvpMagicalSkillCriticalDamageMultipliers",
            &mut cb.pvp_magical_skill_critical_damage,
        ),
        (
            "PvePhysicalSkillDamageMultipliers",
            &mut cb.pve_physical_skill_damage,
        ),
        (
            "PvpPhysicalSkillDamageMultipliers",
            &mut cb.pvp_physical_skill_damage,
        ),
        (
            "PvePhysicalSkillCriticalChanceMultipliers",
            &mut cb.pve_physical_skill_critical_chance,
        ),
        (
            "PvpPhysicalSkillCriticalChanceMultipliers",
            &mut cb.pvp_physical_skill_critical_chance,
        ),
        (
            "PvePhysicalSkillCriticalDamageMultipliers",
            &mut cb.pve_physical_skill_critical_damage,
        ),
        (
            "PvpPhysicalSkillCriticalDamageMultipliers",
            &mut cb.pvp_physical_skill_critical_damage,
        ),
        ("PveBlowSkillDamageMultipliers", &mut cb.pve_blow_skill_damage),
        ("PvpBlowSkillDamageMultipliers", &mut cb.pvp_blow_skill_damage),
        ("PveBlowSkillDefenceMultipliers", &mut cb.pve_blow_skill_defence),
        ("PvpBlowSkillDefenceMultipliers", &mut cb.pvp_blow_skill_defence),
        ("PveEnergySkillDamageMultipliers", &mut cb.pve_energy_skill_damage),
        ("PvpEnergySkillDamageMultipliers", &mut cb.pvp_energy_skill_damage),
        (
            "PveEnergySkillDefenceMultipliers",
            &mut cb.pve_energy_skill_defence,
        ),
        (
            "PvpEnergySkillDefenceMultipliers",
            &mut cb.pvp_energy_skill_defence,
        ),
        ("PlayerHealingSkillMultipliers", &mut cb.player_healing_skill),
        ("SkillMasteryChanceMultipliers", &mut cb.skill_mastery_chance),
    ] {
        parse_class_multipliers(key, &ini.get_string(key, ""), table);
    }
}

/// The values the attack code reads (M4-COMBAT-01). `AltGameCancelByHit` and the NPC damage
/// penalty lists are post-processed in Java (placeholders in the generated config), so they are
/// read from `Player.ini` and `NPC.ini` here.
pub fn combat_settings(config: &GameConfig, config_dir: &Path) -> CombatSettings {
    use l2_config::IniFile;

    let p = &config.player;
    let mut s = CombatSettings {
        alt_game_shield_blocks: p.alt_game_shield_blocks,
        alt_perfect_shld_block: p.alt_perfect_shld_block,
        randomize_auto_attack_damage: p.randomize_auto_attack_damage,
        calculate_distance_bow_damage: p.calculate_distance_bow_damage,
        karma_player_can_be_killed_in_peacezone: p.alt_game_karma_player_can_be_killed_in_peacezone,
        min_npc_level_dmg_penalty: config.npc.min_npc_level_dmg_penalty,
        alt_attackable_npcs: config.npc.alt_attackable_npcs,
        alt_game_magicfailures: p.alt_game_magicfailures,
        calculate_magic_success_by_skill_magic_level: p.calculate_magic_success_by_skill_magic_level,
        randomize_physical_skill_damage: p.randomize_physical_skill_damage,
        randomize_magical_skill_damage: p.randomize_magical_skill_damage,
        min_npc_level_magic_penalty: config.npc.min_npc_level_magic_penalty,
        ..CombatSettings::default()
    };
    // Java: `ALT_GAME_CANCEL_BOW = AltGameCancelByHit is "bow" or "all"`, `ALT_GAME_CANCEL_CAST`
    // the same with "cast" (both ignoring case, default "Cast").
    if let Ok(ini) = IniFile::load(config_dir.join(l2_config::game::PlayerConfig::FILE)) {
        let cancel = ini.get_string("AltGameCancelByHit", "Cast");
        s.alt_game_cancel_bow = cancel.eq_ignore_ascii_case("bow") || cancel.eq_ignore_ascii_case("all");
        s.alt_game_cancel_cast = cancel.eq_ignore_ascii_case("cast") || cancel.eq_ignore_ascii_case("all");
    }
    if let Ok(ini) = IniFile::load(config_dir.join(l2_config::game::NpcConfig::FILE)) {
        s.npc_dmg_penalty = float_list(
            "DmgPenaltyForLvLDifferences",
            &ini.get_string("DmgPenaltyForLvLDifferences", "0.7, 0.6, 0.6, 0.55"),
            &s.npc_dmg_penalty,
        );
        s.npc_crit_dmg_penalty = float_list(
            "CritDmgPenaltyForLvLDifferences",
            &ini.get_string("CritDmgPenaltyForLvLDifferences", "0.75, 0.65, 0.6, 0.58"),
            &s.npc_crit_dmg_penalty,
        );
        s.npc_skill_dmg_penalty = float_list(
            "SkillDmgPenaltyForLvLDifferences",
            &ini.get_string("SkillDmgPenaltyForLvLDifferences", "0.8, 0.7, 0.65, 0.62"),
            &s.npc_skill_dmg_penalty,
        );
        s.npc_skill_chance_penalty = float_list(
            "SkillChancePenaltyForLvLDifferences",
            &ini.get_string("SkillChancePenaltyForLvLDifferences", "2.5, 3.0, 3.25, 3.5"),
            &s.npc_skill_chance_penalty,
        );
    }
    s
}

/// Java's `split(",")` + `Float.parseFloat` of the NPC damage penalty lists. Java fails to load the
/// config on a broken number; the default list is kept then, with a warning.
fn float_list(key: &str, text: &str, default: &[f32]) -> Vec<f32> {
    let parsed: Result<Vec<f32>, _> = text.split(',').map(|v| v.trim().parse::<f32>()).collect();
    match parsed {
        Ok(values) => values,
        Err(e) => {
            warn!("{key}: cannot read '{text}' ({e}); the default is used.");
            default.to_vec()
        }
    }
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

    #[test]
    fn the_side_by_side_switch_defaults_to_off_and_reads_the_file() {
        let dir = tempfile::tempdir().unwrap();
        // A missing file means the default.
        assert!(!daily_reset_database_writes(dir.path()));
        let file = dir.path().join(SIDE_BY_SIDE_FILE);
        std::fs::write(
            &file,
            "# nothing set
",
        )
        .unwrap();
        assert!(!daily_reset_database_writes(dir.path()));
        std::fs::write(
            &file,
            "DailyResetDatabaseWrites = True
",
        )
        .unwrap();
        assert!(daily_reset_database_writes(dir.path()));
        std::fs::write(
            &file,
            "DailyResetDatabaseWrites = False
",
        )
        .unwrap();
        assert!(!daily_reset_database_writes(dir.path()));
    }

    #[test]
    fn the_example_file_is_the_default() {
        let dir = tempfile::tempdir().unwrap();
        let example = include_str!("../config-examples/SideBySide.ini");
        std::fs::write(dir.path().join(SIDE_BY_SIDE_FILE), example).unwrap();
        assert!(!daily_reset_database_writes(dir.path()));
        // The example text is also the only place the key is spelled out for owners.
        assert!(example.contains("DailyResetDatabaseWrites = False"));
    }

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

    /// Java `PlayerConfig`: `PartyXpCutoffGaps` is `low,high` pairs split by `;`, the percents are
    /// numbers split by `;`; a bad number is a Java exception (here the default stays).
    #[test]
    fn the_party_cutoff_lists_parse_like_java() {
        assert_eq!(
            parse_party_gaps("0,9;10,14;15,99"),
            Some(vec![[0, 9], [10, 14], [15, 99]])
        );
        assert_eq!(parse_party_gaps("1,2"), Some(vec![[1, 2]]));
        assert_eq!(parse_party_gaps("1;2,3"), None);
        assert_eq!(parse_party_gaps("a,2"), None);
        assert_eq!(parse_party_percents("100;30;0"), Some(vec![100, 30, 0]));
        assert_eq!(parse_party_percents("100;x"), None);
    }

    /// The party settings come from `Player.ini`, `Rates.ini` and `Custom/ClassBalance.ini`; the
    /// defaults of a missing file are Java's.
    #[test]
    fn party_settings_read_the_config_files() {
        use l2_world::PartyExpType;

        let dir = tempfile::tempdir().unwrap();
        let config = GameConfig::load(dir.path()).unwrap();
        let defaults = party_settings(&config, dir.path());
        assert_eq!(defaults, l2_world::PartySettings::default());

        std::fs::write(
            dir.path().join(l2_config::game::PlayerConfig::FILE),
            "AltPartyRange = 900\nAltLeavePartyLeader = True\nPartyXpCutoffMethod = highfive\n\
             PartyXpCutoffPercent = 4.5\nPartyXpCutoffLevel = 7\nPartyXpCutoffGaps = 0,4;5,9\n\
             PartyXpCutoffGapPercent = 100;50\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join(l2_config::game::RatesConfig::FILE),
            "RatePartyXp = 1.5\nRatePartySp = 2\n",
        )
        .unwrap();
        let custom = dir.path().join("Custom");
        std::fs::create_dir_all(&custom).unwrap();
        std::fs::write(
            dir.path().join(l2_config::game::ClassBalanceConfig::FILE),
            "ExpAmountMultipliers = 0*0.5;18*2\nSpAmountMultipliers = 1*3\n",
        )
        .unwrap();
        let config = GameConfig::load(dir.path()).unwrap();
        let settings = party_settings(&config, dir.path());
        assert_eq!(settings.alt_party_range, 900);
        assert!(settings.alt_leave_party_leader);
        assert_eq!(settings.rate_party_xp, 1.5);
        assert_eq!(settings.rate_party_sp, 2.0);
        assert_eq!(settings.xp_cutoff_method, PartyExpType::HighFive);
        assert_eq!(settings.xp_cutoff_percent, 4.5);
        assert_eq!(settings.xp_cutoff_level, 7);
        assert_eq!(settings.xp_cutoff_gaps, vec![[0, 4], [5, 9]]);
        assert_eq!(settings.xp_cutoff_gap_percents, vec![100, 50]);
        assert_eq!(settings.exp_amount_multipliers[0], 0.5);
        assert_eq!(settings.exp_amount_multipliers[18], 2.0);
        assert_eq!(settings.exp_amount_multipliers[1], 1.0);
        assert_eq!(settings.sp_amount_multipliers[1], 3.0);
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

    /// M5-FIX-03: every skill table of `ClassBalanceConfig` is read from its own key, the others
    /// stay at Java's `Arrays.fill(.., 1f)`.
    #[test]
    fn class_balance_skill_tables_are_read_from_their_keys() {
        let ini = l2_config::IniFile::parse("ClassBalance.ini", "PveMagicalSkillDamageMultipliers = 3*2.5\nPvpMagicalSkillDamageMultipliers = 3*0.5\nPvpMagicalSkillDefenceMultipliers = 3*1.5\nPveMagicalSkillCriticalDamageMultipliers = 3*1.25\nPvpMagicalSkillCriticalDamageMultipliers = 3*1.75\nPvePhysicalSkillDamageMultipliers = 3*2\nPvpPhysicalSkillDamageMultipliers = 3*3\nPvePhysicalSkillCriticalChanceMultipliers = 3*4\nPvpPhysicalSkillCriticalChanceMultipliers = 3*5\nPvePhysicalSkillCriticalDamageMultipliers = 3*6\nPvpPhysicalSkillCriticalDamageMultipliers = 3*7\nPveBlowSkillDamageMultipliers = 3*8\nPvpBlowSkillDamageMultipliers = 3*9\nPveBlowSkillDefenceMultipliers = 3*10\nPvpBlowSkillDefenceMultipliers = 3*11\nPveEnergySkillDamageMultipliers = 3*12\nPvpEnergySkillDamageMultipliers = 3*13\nPveEnergySkillDefenceMultipliers = 3*14\nPvpEnergySkillDefenceMultipliers = 3*15\nPlayerHealingSkillMultipliers = 3*16\nSkillMasteryChanceMultipliers = 3*17\nPvpPhysicalAttackDamageMultipliers = 3*18\n").unwrap();
        let mut cb = l2_world::stats::ClassBalance::default();
        load_class_balance(&ini, &mut cb);
        let at3 = [
            (&cb.pve_magical_skill_damage, 2.5_f32),
            (&cb.pvp_magical_skill_damage, 0.5_f32),
            (&cb.pvp_magical_skill_defence, 1.5_f32),
            (&cb.pve_magical_skill_critical_damage, 1.25_f32),
            (&cb.pvp_magical_skill_critical_damage, 1.75_f32),
            (&cb.pve_physical_skill_damage, 2_f32),
            (&cb.pvp_physical_skill_damage, 3_f32),
            (&cb.pve_physical_skill_critical_chance, 4_f32),
            (&cb.pvp_physical_skill_critical_chance, 5_f32),
            (&cb.pve_physical_skill_critical_damage, 6_f32),
            (&cb.pvp_physical_skill_critical_damage, 7_f32),
            (&cb.pve_blow_skill_damage, 8_f32),
            (&cb.pvp_blow_skill_damage, 9_f32),
            (&cb.pve_blow_skill_defence, 10_f32),
            (&cb.pvp_blow_skill_defence, 11_f32),
            (&cb.pve_energy_skill_damage, 12_f32),
            (&cb.pvp_energy_skill_damage, 13_f32),
            (&cb.pve_energy_skill_defence, 14_f32),
            (&cb.pvp_energy_skill_defence, 15_f32),
            (&cb.player_healing_skill, 16_f32),
            (&cb.skill_mastery_chance, 17_f32),
            (&cb.pvp_physical_attack_damage, 18_f32),
        ];
        for (index, (table, expected)) in at3.into_iter().enumerate() {
            assert_eq!(table[3], expected, "table {index}");
            assert_eq!(table[2], 1.0, "table {index}");
            assert_eq!(table.len(), 137);
        }
        // A table whose key is not in the file stays at 1.0.
        assert!(cb.pve_physical_attack_damage.iter().all(|v| *v == 1.0));
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
    fn take_aggro_is_false_only_where_the_file_says_so() {
        let levels = AccessLevels::from_xml(
            r#"<list>
	<access level="-1" name="Banned" takeAggro="false" />
	<access level="0" name="User" takeAggro="true" />
	<access level="20" name="Test GM" takeAggro="false" />
	<access level="30" name="General GM" takeAggro="False" />
	<access level="70" name="Admin" />
	<access level="100" name="Master" takeAggro="yes" />
</list>"#,
        );
        // `takeAggro` true, or absent (Java's default true), takes aggro.
        assert!(levels.can_take_aggro(0));
        assert!(levels.can_take_aggro(70));
        // Java `Boolean.parseBoolean`: "False" is false, "yes" is not "true".
        assert!(!levels.can_take_aggro(-1));
        assert!(!levels.can_take_aggro(-5));
        assert!(!levels.can_take_aggro(20));
        assert!(!levels.can_take_aggro(30));
        assert!(!levels.can_take_aggro(100));
        // A level the file does not list is Java's default AccessLevel, which takes aggro.
        assert!(levels.can_take_aggro(55));
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

    /// The spoil and magic success keys (M5-SPOIL-01) reach the world settings: with no config
    /// files the Java defaults, with `NPC.ini` / `Player.ini` their values.
    #[test]
    fn the_spoil_and_magic_success_settings_are_read() {
        let dir = tempfile::tempdir().unwrap();
        let config = GameConfig::load(dir.path()).unwrap();
        let combat = combat_settings(&config, dir.path());
        assert!(combat.calculate_magic_success_by_skill_magic_level);
        assert_eq!(combat.min_npc_level_magic_penalty, 78);
        assert_eq!(combat.npc_skill_chance_penalty, [2.5, 3.0, 3.25, 3.5]);
        let death = death_settings(&config, dir.path());
        assert_eq!(death.spoiled_corpse_extend_time, 10);
        assert_eq!(death.corpse_consume_skill_allowed_time_before_decay, 2000);

        std::fs::write(
            dir.path().join("NPC.ini"),
            "SkillChancePenaltyForLvLDifferences = 1.5, 2.5\nMinNPCLevelForMagicPenalty = 70\n\
             SpoiledCorpseExtendTime = 33\nCorpseConsumeSkillAllowedTimeBeforeDecay = 1234\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("Player.ini"),
            "CalculateMagicSuccessBySkillMagicLevel = False\n",
        )
        .unwrap();
        let config = GameConfig::load(dir.path()).unwrap();
        let combat = combat_settings(&config, dir.path());
        assert!(!combat.calculate_magic_success_by_skill_magic_level);
        assert_eq!(combat.min_npc_level_magic_penalty, 70);
        assert_eq!(combat.npc_skill_chance_penalty, [1.5, 2.5]);
        let death = death_settings(&config, dir.path());
        assert_eq!(death.spoiled_corpse_extend_time, 33);
        assert_eq!(death.corpse_consume_skill_allowed_time_before_decay, 1234);
    }

    /// The vitality and Nevit settings carry the Java defaults of the generated config.
    #[test]
    fn the_vitality_and_nevit_settings_are_the_java_defaults() {
        let config = GameConfig::default();
        assert_eq!(vitality_settings(&config), l2_world::VitalitySettings::default());
        assert_eq!(nevit_settings(&config), l2_world::NevitSettings::default());
    }

    /// The shipped `Rates.ini` / `Player.ini` reach the world: `RateVitalityLost = 0.6`,
    /// `RaidbossUseVitality = False`.
    #[test]
    fn the_shipped_vitality_values_are_read() {
        let root = std::env::var_os("L2JMOBIUS_GAME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\l2-rust\l2jmobius\dist\game"));
        let config_dir = root.join("config");
        if !config_dir.join("Rates.ini").is_file() {
            eprintln!("skipped: {} has no Rates.ini", config_dir.display());
            return;
        }
        let config = GameConfig::load(&config_dir).unwrap();
        let v = vitality_settings(&config);
        assert!(v.enable_vitality && v.recover_vitality_on_reconnect);
        assert!(!v.raidboss_use_vitality);
        assert_eq!(v.rate_vitality_level, [1.5, 2.0, 2.5, 3.0]);
        assert_eq!((v.rate_vitality_gain, v.rate_vitality_lost), (1.0, 0.6));
        assert_eq!(v.rate_recovery_on_reconnect, 4.0);
    }
}
