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
    /// The levels with `allowTransaction="false"` (Java `AccessLevel.allowTransaction()`, default
    /// true).
    no_transaction: std::collections::BTreeSet<i32>,
    /// The levels with `allowAltg="true"` (Java `AccessLevel.allowAltG()`, default false).
    alt_g: std::collections::BTreeSet<i32>,
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
                    if let Some(description) = attribute(&e, "description") {
                        self.rights.set_description(&command, &description);
                    }
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
                    if let Some(name) = attribute(&e, "name") {
                        table.rights.set_level_name(level, &name);
                    }
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
                    // Java: `set.getBoolean("allowTransaction", true)`: anything but "true" is false.
                    if attribute(&e, "allowTransaction").is_some_and(|v| !v.eq_ignore_ascii_case("true")) {
                        table.no_transaction.insert(level);
                    }
                    if attribute(&e, "allowAltg").is_some_and(|v| v.eq_ignore_ascii_case("true")) {
                        table.alt_g.insert(level);
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

    /// Java `AdminData.getAccessLevel(level).allowAltG()`.
    pub fn allows_alt_g(&self, level: i32) -> bool {
        let key = if level < 0 { -1 } else { level };
        self.alt_g.contains(&key)
    }

    /// Java `AdminData.getAccessLevel(level).allowTransaction()`.
    pub fn allows_transaction(&self, level: i32) -> bool {
        let key = if level < 0 { -1 } else { level };
        !self.no_transaction.contains(&key)
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

    fn transforms(&self) -> Option<&Arc<l2_data::Transforms>> {
        Some(&self.datapack.transforms)
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

    fn access_level_allows_transaction(&self, level: i32) -> bool {
        self.access.allows_transaction(level)
    }

    fn access_level_allows_alt_g(&self, level: i32) -> bool {
        self.access.allows_alt_g(level)
    }

    fn augmentation_option(&self, id: i32) -> Option<&l2_data::AugmentationOption> {
        self.datapack.augmentation_options.get(id)
    }

    fn enchant_skill_groups(&self) -> Option<Arc<l2_data::EnchantSkillGroupsData>> {
        Some(Arc::clone(&self.datapack.enchant_skill_groups))
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
        enable_free_strider: config.free_mounts.enable_free_strider,
        enable_free_wyvern: config.free_mounts.enable_free_wyvern,
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
        loot_raids_privilege_cc_size: p.loot_raids_privilege_cc_size,
        ..PartySettings::default()
    };
    if let Ok(ini) = IniFile::load(config_dir.join(l2_config::game::PlayerConfig::FILE)) {
        // Java: `Enum.valueOf(PartyExpType.class, getString("PartyXpCutoffMethod", "LEVEL").toUpperCase())`.
        // `LOOT_RAIDS_PRIVILEGE_INTERVAL = getInt("RaidLootRightsInterval", 900) * 1000`.
        settings.loot_raids_privilege_interval_ms =
            i64::from(ini.get_int("RaidLootRightsInterval", 900)).wrapping_mul(1000);
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

/// The names of Java's `ChatType`, in the order of its ids.
const CHAT_TYPE_NAMES: [&str; 24] = [
    "GENERAL",
    "SHOUT",
    "WHISPER",
    "PARTY",
    "CLAN",
    "GM",
    "PETITION_PLAYER",
    "PETITION_GM",
    "TRADE",
    "ALLIANCE",
    "ANNOUNCEMENT",
    "BOAT",
    "FRIEND",
    "MSNCHAT",
    "PARTYMATCH_ROOM",
    "PARTYROOM_COMMANDER",
    "PARTYROOM_ALL",
    "HERO_VOICE",
    "CRITICAL_ANNOUNCE",
    "SCREEN_ANNOUNCE",
    "BATTLEFIELD",
    "MPCC_ROOM",
    "NPC_GENERAL",
    "NPC_SHOUT",
];

/// The values punishments read (Java `GeneralConfig`): `DefaultPunish`, `DefaultPunishParam`,
/// `JailDisableChat` and `BanChatChannels`. The first and the last are post-processed in Java
/// (placeholders in the generated config), so they are read from `General.ini` here.
pub fn punishment_settings(config: &GameConfig, config_dir: &Path) -> l2_world::PunishmentSettings {
    use l2_config::IniFile;
    use l2_config::game::GeneralConfig;
    use l2_world::{IllegalActionPunishment, PunishmentSettings};

    let mut settings = PunishmentSettings {
        default_punish_param: config.effective_default_punish_param(),
        jail_disable_chat: config.general.jail_disable_chat,
        ..PunishmentSettings::default()
    };
    if let Ok(ini) = IniFile::load(config_dir.join(GeneralConfig::FILE)) {
        settings.default_punish =
            IllegalActionPunishment::find_by_name(&ini.get_string("DefaultPunish", "KICK"));
        // Java: `Enum.valueOf(ChatType.class, name)` throws for a bad name; the channel is skipped.
        let text = ini.get_string("BanChatChannels", "GENERAL;SHOUT;TRADE;HERO_VOICE;WHISPER");
        let mut channels = Vec::new();
        for name in text.trim().split(';') {
            match CHAT_TYPE_NAMES.iter().position(|n| *n == name) {
                Some(index) => {
                    if let Some(chat_type) = l2_protocol_game::server::ChatType::from_client_id(index as i32)
                        && !channels.contains(&chat_type)
                    {
                        channels.push(chat_type);
                    }
                }
                None => warn!("BanChatChannels: \"{name}\" is not a chat type; skipped."),
            }
        }
        settings.ban_chat_channels = channels;
    }
    settings
}

/// `BotReportPunishments.xml` (Java `BotReportTable.PunishmentsLoader`): `<punishment
/// neededReportCount skillId [skillLevel] [sysMessageId]>`. Java reports a bad attribute and then
/// still calls `addPunishment` with what it has (its `-1` defaults); the skill check of the world
/// drops that one.
pub fn bot_report_punishments_from_xml(xml: &str) -> BTreeMap<i32, l2_world::BotPunishment> {
    let mut punishments = BTreeMap::new();
    let mut reader = Reader::from_str(xml);
    loop {
        match reader.read_event() {
            Ok(Event::Empty(e) | Event::Start(e)) if e.name().as_ref() == "punishment" => {
                let number = |key: &str| attribute(&e, key).and_then(|v| v.trim().parse::<i32>().ok());
                let (Some(needed), Some(skill_id)) = (number("neededReportCount"), number("skillId")) else {
                    warn!("Problem with BotReportTable: a punishment without neededReportCount or skillId");
                    continue;
                };
                punishments.insert(
                    needed,
                    l2_world::BotPunishment {
                        skill_id,
                        skill_level: number("skillLevel").unwrap_or(1),
                        system_message: number("sysMessageId").unwrap_or(-1),
                    },
                );
            }
            Ok(Event::Eof) | Err(_) => break,
            Ok(_) => {}
        }
    }
    punishments
}

/// The bot report values (Java `GeneralConfig.BOTREPORT_*`). The reset hour and the delay are
/// post-processed in Java (placeholders in the generated config), so they are read from
/// `General.ini` here; the punishments come from `BotReportPunishments.xml` when the button is on.
pub fn bot_report_settings(config: &GameConfig, config_dir: &Path) -> l2_world::BotReportSettings {
    use l2_config::IniFile;
    use l2_config::game::GeneralConfig;

    let mut settings = l2_world::BotReportSettings {
        enable: config.general.botreport_enable,
        allow_reports_from_same_clan_members: config.general.botreport_allow_reports_from_same_clan_members,
        ..l2_world::BotReportSettings::default()
    };
    if let Ok(ini) = IniFile::load(config_dir.join(GeneralConfig::FILE)) {
        // Java: `config.getInt("BotReportDelay", 30) * 60000`.
        settings.report_delay_ms = i64::from(ini.get_int("BotReportDelay", 30)) * 60_000;
        // Java: `getString("BotReportPointsResetHour", "00:00").split(":")`, parsed with
        // `Integer.parseInt`; anything else fails the scheduling (every 24 hours).
        let text = ini.get_string("BotReportPointsResetHour", "00:00");
        let mut parts = text.split(':');
        settings.reset_hour_minute = match (parts.next(), parts.next()) {
            (Some(hour), Some(minute)) => hour
                .trim()
                .parse::<i64>()
                .ok()
                .zip(minute.trim().parse::<i64>().ok()),
            _ => None,
        };
    }
    if settings.enable {
        let path = config_dir.join("BotReportPunishments.xml");
        match std::fs::read(&path) {
            Ok(bytes) => {
                settings.punishments = bot_report_punishments_from_xml(&String::from_utf8_lossy(&bytes));
            }
            Err(e) => warn!(
                "BotReportTable: Could not load punishments from {}: {e}",
                path.display()
            ),
        }
    }
    settings
}

/// `FloodProtectorConfig.FLOOD_PROTECTOR_CONFIG_FILE`.
pub const FLOOD_PROTECTOR_FILE: &str = "FloodProtector.ini";

/// `config/FloodProtector.ini` (Java `FloodProtectorConfig.load`): the fifteen flood protectors.
/// A missing file or key is the Java default of the key.
pub fn flood_settings(config_dir: &Path) -> l2_world::FloodSettings {
    use l2_config::IniFile;

    match IniFile::load(config_dir.join(FLOOD_PROTECTOR_FILE)) {
        Ok(ini) => flood_settings_from_ini(&ini),
        Err(e) => {
            warn!("Cannot read {FLOOD_PROTECTOR_FILE}: {e}; using the flood protector defaults.");
            l2_world::FloodSettings::java_defaults()
        }
    }
}

/// `FloodProtectorConfig.loadFloodProtectorConfig` for every protector.
pub fn flood_settings_from_ini(ini: &l2_config::IniFile) -> l2_world::FloodSettings {
    use l2_world::{FloodConfig, FloodKind, FloodPunishment, FloodSettings};

    let mut settings = FloodSettings::java_defaults();
    for kind in FloodKind::ALL {
        let key = |suffix: &str| format!("FloodProtector{}{suffix}", kind.config_name());
        settings.set(
            kind,
            FloodConfig {
                protection_interval: ini.get_int(&key("Interval"), kind.java_default_interval()),
                log_flooding: ini.get_bool(&key("LogFlooding"), false),
                punishment_limit: ini.get_int(&key("PunishmentLimit"), 0),
                punishment_type: FloodPunishment::parse(&ini.get_string(&key("PunishmentType"), "none")),
                // NOTE(java-quirk): `getInt(...) * 60000` is an int multiplication; a huge number
                // of minutes overflows before it becomes the long.
                punishment_time: i64::from(ini.get_int(&key("PunishmentTime"), 0).wrapping_mul(60_000)),
            },
        );
    }
    settings
}

/// The enchant values (`Player.ini`, `General.ini`). `EnchantBlackList` and `OverEnchantPunishment`
/// are post-processed in Java (placeholders in the generated config), so they are read from
/// `Player.ini` here.
pub fn enchant_settings(config: &GameConfig, config_dir: &Path) -> l2_world::EnchantSettings {
    use l2_config::IniFile;
    use l2_config::game::PlayerConfig;
    use l2_world::{EnchantSettings, IllegalActionPunishment};

    let p = &config.player;
    let mut settings = EnchantSettings {
        over_enchant_protection: p.over_enchant_protection,
        disable_over_enchanting: p.disable_over_enchanting,
        enchant_chance_element_stone: p.enchant_chance_element_stone,
        enchant_chance_element_crystal: p.enchant_chance_element_crystal,
        enchant_chance_element_jewel: p.enchant_chance_element_jewel,
        enchant_chance_element_energy: p.enchant_chance_element_energy,
        log_item_enchants: config.general.log_item_enchants,
        log_skill_enchants: config.general.log_skill_enchants,
        es_sp_book_needed: p.es_sp_book_needed,
        normal_enchant_cost_multiplier: config.general.normal_enchant_cost_multiplier,
        safe_enchant_cost_multiplier: config.general.safe_enchant_cost_multiplier,
        ..EnchantSettings::default()
    };
    if let Ok(ini) = IniFile::load(config_dir.join(PlayerConfig::FILE)) {
        settings.over_enchant_punishment =
            IllegalActionPunishment::find_by_name(&ini.get_string("OverEnchantPunishment", "JAIL"));
        // Java: `Integer.parseInt` of each comma separated id (it throws for a bad one), sorted.
        let text = ini.get_string(
            "EnchantBlackList",
            "7816,7817,7818,7819,7820,7821,7822,7823,7824,7825,7826,7827,7828,7829,7830,7831,13293,13294,13296",
        );
        let ids: Result<Vec<i32>, _> = text.split(',').map(|id| id.trim().parse::<i32>()).collect();
        match ids {
            Ok(mut ids) => {
                ids.sort_unstable();
                settings.enchant_blacklist = ids;
            }
            Err(_) => warn!("EnchantBlackList = {text} is not a list of ids; using the default."),
        }
    }
    settings
}

/// The augmentation values (`Player.ini`). The four `RetailLikeAugmentation*GradeChance` lists and
/// `AugmentationBlackList` are post-processed in Java (placeholders in the generated config), so
/// they are read from `Player.ini` here.
pub fn augment_settings(config: &GameConfig, config_dir: &Path) -> l2_world::AugmentSettings {
    use l2_config::IniFile;
    use l2_config::game::PlayerConfig;
    use l2_world::AugmentSettings;

    let p = &config.player;
    let mut settings = AugmentSettings {
        ng_skill_chance: p.augmentation_ng_skill_chance,
        ng_glow_chance: p.augmentation_ng_glow_chance,
        mid_skill_chance: p.augmentation_mid_skill_chance,
        mid_glow_chance: p.augmentation_mid_glow_chance,
        high_skill_chance: p.augmentation_high_skill_chance,
        high_glow_chance: p.augmentation_high_glow_chance,
        top_skill_chance: p.augmentation_top_skill_chance,
        top_glow_chance: p.augmentation_top_glow_chance,
        basestat_chance: p.augmentation_basestat_chance,
        acc_skill_chance: p.augmentation_acc_skill_chance,
        retail_like: p.retail_like_augmentation,
        retail_like_accessory: p.retail_like_augmentation_accessory,
        alt_allow_augment_pvp_items: p.alt_allow_augment_pvp_items,
        ..AugmentSettings::default()
    };
    if let Ok(ini) = IniFile::load(config_dir.join(PlayerConfig::FILE)) {
        // Java: `Integer.parseInt` of the first four numbers of each list (it throws for a bad one
        // or a short list).
        let grades = [
            "RetailLikeAugmentationNoGradeChance",
            "RetailLikeAugmentationMidGradeChance",
            "RetailLikeAugmentationHighGradeChance",
            "RetailLikeAugmentationTopGradeChance",
        ];
        for (grade, key) in grades.iter().enumerate() {
            let text = ini.get_string(key, "55,35,7,3");
            let numbers: Vec<i32> = text.split(',').filter_map(|n| n.parse().ok()).collect();
            if text.split(',').count() >= 4 && numbers.len() == text.split(',').count() {
                settings.retail_like_chances[grade] = [numbers[0], numbers[1], numbers[2], numbers[3]];
            } else {
                warn!("{key} = {text} is not a list of four numbers; using the default.");
            }
        }
        // Java: `Integer.parseInt` of each comma separated id (it throws for a bad one), sorted.
        let text = ini.get_string("AugmentationBlackList", "");
        if !text.is_empty() {
            let ids: Result<Vec<i32>, _> = text.split(',').map(|id| id.trim().parse::<i32>()).collect();
            match ids {
                Ok(mut ids) => {
                    ids.sort_unstable();
                    settings.blacklist = ids;
                }
                Err(_) => warn!("AugmentationBlackList = {text} is not a list of ids; using the default."),
            }
        }
    }
    settings
}

/// The PvP flag times, the anti feed rules (`PVP.ini`) and the PvP title colours
/// (`Custom/PvpTitleColor.ini`). `AntiFeedInterval` (seconds, Java keeps ms) and the five
/// `ColorForAmountN` (`Integer.decode("0x" + text)`) are post-processed in Java (placeholders in
/// the generated config), so they are read from the files here.
pub fn pvp_settings(config: &GameConfig, config_dir: &Path) -> l2_world::PvpSettings {
    use l2_config::IniFile;
    use l2_world::{PvpSettings, PvpTitleColorSettings};

    let p = &config.pvp;
    let t = &config.pvp_title_color;
    let mut s = PvpSettings {
        pvp_normal_time_ms: p.pvp_normal_time,
        pvp_pvp_time_ms: p.pvp_pvp_time,
        flag_player_on_raid_attack: p.flag_player_on_raid_attack,
        flag_player_on_champion_attack: p.flag_player_on_champion_attack,
        flag_player_on_summon_pet_raid_champion_attack: p.flag_player_on_summon_pet_raid_champion_attack,
        karma_award_pk_kill: p.karma_award_pk_kill,
        antifeed_enable: p.antifeed_enable,
        antifeed_dualbox: p.antifeed_dualbox,
        antifeed_disconnected_as_dualbox: p.antifeed_disconnected_as_dualbox,
        faction_system_enabled: config.faction_system.faction_system_enabled,
        title_color: PvpTitleColorSettings {
            enabled: t.pvp_color_system_enabled,
            amounts: [
                t.pvp_amount1,
                t.pvp_amount2,
                t.pvp_amount3,
                t.pvp_amount4,
                t.pvp_amount5,
            ],
            titles: [
                t.title_for_pvp_amount1.clone(),
                t.title_for_pvp_amount2.clone(),
                t.title_for_pvp_amount3.clone(),
                t.title_for_pvp_amount4.clone(),
                t.title_for_pvp_amount5.clone(),
            ],
            ..PvpTitleColorSettings::default()
        },
        ..PvpSettings::default()
    };
    if let Ok(ini) = IniFile::load(config_dir.join(l2_config::game::PvpConfig::FILE)) {
        s.antifeed_interval_ms = i64::from(ini.get_int("AntiFeedInterval", 120)).saturating_mul(1000);
    }
    if let Ok(ini) = IniFile::load(config_dir.join(l2_config::game::PvpTitleColorConfig::FILE)) {
        for (i, color) in s.title_color.colors.iter_mut().enumerate() {
            let key = format!("ColorForAmount{}", i + 1);
            let text = ini.get_string(&key, "00FF00");
            match i32::from_str_radix(text.trim(), 16) {
                Ok(value) => *color = value,
                Err(_) => warn!("{key} = {text} is not a hex colour; using 00FF00."),
            }
        }
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
/// The summon values of `Player.ini` (`SummonStoreSkillCooltime`, `RestoreServitorOnReconnect`).
pub fn summon_settings(config: &GameConfig) -> l2_world::SummonSettings {
    l2_world::SummonSettings {
        summon_store_skill_cooltime: config.player.summon_store_skill_cooltime,
        restore_servitor_on_reconnect: config.player.restore_servitor_on_reconnect,
    }
}

/// The pet values (`Player.ini`: `RestorePetOnReconnect`, `PlayerSpawnProtection`; `Rates.ini`:
/// `PetXpRate`, `PetFoodRate`, `SinEaterXpRate`; `NPC.ini`: `MaximumSlotsForPet` and the two regen
/// multipliers, which Java divides by 100; `Server.ini`: `PetNameTemplate`).
pub fn pet_settings(config: &GameConfig, config_dir: &Path) -> l2_world::PetSettings {
    use l2_config::IniFile;

    let mut settings = l2_world::PetSettings {
        restore_pet_on_reconnect: config.player.restore_pet_on_reconnect,
        pet_xp_rate: config.rates.pet_xp_rate,
        pet_food_rate: config.rates.pet_food_rate,
        sineater_xp_rate: config.rates.sineater_xp_rate,
        inventory_maximum_pet: config.npc.inventory_maximum_pet,
        player_spawn_protection: config.player.player_spawn_protection,
        ..l2_world::PetSettings::default()
    };
    // Java: `PET_HP_REGEN_MULTIPLIER = config.getDouble("PetHpRegenMultiplier", 100) / 100;`
    if let Ok(ini) = IniFile::load(config_dir.join(l2_config::game::NpcConfig::FILE)) {
        settings.pet_hp_regen_multiplier = ini.get_double("PetHpRegenMultiplier", 100.0) / 100.0;
        settings.pet_mp_regen_multiplier = ini.get_double("PetMpRegenMultiplier", 100.0) / 100.0;
    }
    let template = config.manual.server.pet_name_template.clone();
    settings.pet_name_template = std::sync::Arc::new(move |name: &str| template.is_match(name));
    settings
}

/// The `PcCafe*` keys of `Custom/PremiumSystem.ini`, with the corrections `PremiumSystemConfig.load`
/// makes after reading them.
pub fn pc_cafe_settings(config: &GameConfig) -> l2_world::PcCafeSettings {
    let c = &config.premium_system;
    l2_world::PcCafeSettings {
        enabled: c.pc_cafe_enabled,
        only_premium: c.pc_cafe_only_premium,
        retail_like: c.pc_cafe_retail_like,
        reward_time_ms: c.pc_cafe_reward_time,
        // `if (PC_CAFE_MAX_POINTS < 0) PC_CAFE_MAX_POINTS = 0`.
        max_points: c.pc_cafe_max_points.max(0),
        enable_double_points: c.pc_cafe_enable_double_points,
        // `if (chance < 0 || chance > 100) chance = 1`.
        double_points_chance: if (0..=100).contains(&c.pc_cafe_double_points_chance) {
            c.pc_cafe_double_points_chance
        } else {
            1
        },
        retail_like_points: c.acquisition_pc_cafe_retail_like_points,
        // `if (PC_CAFE_POINT_RATE < 0) PC_CAFE_POINT_RATE = 1`.
        point_rate: if c.pc_cafe_point_rate < 0.0 {
            1.0
        } else {
            c.pc_cafe_point_rate
        },
        random_point: c.pc_cafe_random_point,
        reward_low_exp_kills: c.pc_cafe_reward_low_exp_kills,
        low_exp_kills_chance: c.pc_cafe_low_exp_kills_chance.clamp(0, 100),
    }
}

/// The multisell values of `General.ini` and `Player.ini` (`MultisellAmountLimit`,
/// `AltBlacksmithUseRecipes`, `MaxPersonalFamePoints`).
pub fn multisell_settings(config: &GameConfig) -> l2_world::MultisellSettings {
    l2_world::MultisellSettings {
        amount_limit: config.general.multisell_amount_limit,
        alt_blacksmith_use_recipes: config.player.alt_blacksmith_use_recipes,
        max_personal_fame_points: config.player.max_personal_fame_points,
    }
}

/// The configuration the server control commands, the announcements, the petition system and
/// `//debug` read (M7-ADMIN-10): `ServerGMOnly`, `ServerListType`, `ServerListAge`,
/// `ServerListBrackets`, `MaximumOnlineUsers`, `GMShowAnnouncerName`, `GMStartupAutoList`,
/// `PetitioningAllowed`, `MaxPetitionsPending`, `MaxPetitionsPerPlayer`, `DebugClientPackets`.
pub fn admin_settings(config: &GameConfig, config_dir: &Path) -> l2_world::AdminSettings {
    use l2_config::IniFile;

    use crate::ls_link::server_type_id;
    // `ServerListType` is one of the settings the config crate leaves to hand-written code.
    let server_list_type = IniFile::load(config_dir.join(crate::SERVER_CONFIG_FILE))
        .map(|ini| server_type_id(&ini.get_string("ServerListType", "Free")))
        .unwrap_or_else(|_| server_type_id("Free"));
    l2_world::AdminSettings {
        gm_startup_auto_list: config.general.gm_startup_auto_list,
        gm_announcer_name: config.general.gm_announcer_name,
        server_gmonly: config.general.server_gmonly,
        server_list_type,
        server_list_age: config.server.server_list_age,
        server_list_brackets: config.server.server_list_bracket,
        maximum_online_users: config.server.maximum_online_users,
        petitioning_allowed: config.player.petitioning_allowed,
        max_petitions_pending: config.player.max_petitions_pending,
        max_petitions_per_player: config.player.max_petitions_per_player,
        debug_client_packets: config.development.debug_client_packets,
        // `GameServer.getStartTime()`.
        server_start_ms: l2_util::time::now_millis(),
    }
}

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
        gm_startup_silence: config.general.gm_startup_silence,
        gm_startup_diet_mode: config.general.gm_startup_diet_mode,
        silence_mode_exclude: config.player.silence_mode_exclude,
        use_super_haste_as_gm_speed: config.general.use_super_haste_as_gm_speed,
        gm_give_special_skills: config.general.gm_give_special_skills,
        gm_give_special_aura_skills: config.general.gm_give_special_aura_skills,
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
        store: l2_world::StoreSettings {
            shop_min_range_from_player: config.private_store_range.shop_min_range_from_player,
            shop_min_range_from_npc: config.private_store_range.shop_min_range_from_npc,
            max_item_in_packet: config.effective_max_item_in_packet(),
        },
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
            alt_game_karma_player_can_trade: p.alt_game_karma_player_can_trade,
        },
        pvp: pvp_settings(config, config_dir),
        fishing: fishing_settings(config, config_dir),
        enchant: enchant_settings(config, config_dir),
        augment: augment_settings(config, config_dir),
        shop: l2_world::ShopSettings {
            allow_refund: config.general.allow_refund,
            only_gm_items_free: config.general.only_gm_items_free,
            rate_siege_guards_price: config.rates.rate_siege_guards_price,
            merchant_zero_sell_price: config.merchant_zero_sell_price.merchant_zero_sell_price,
            ..l2_world::ShopSettings::default()
        },
        warehouse: l2_world::WarehouseSettings {
            freight_slots: p.alt_freight_slots,
            freight_price: p.alt_freight_price,
            members_can_withdraw_from_clanwh: p.alt_members_can_withdraw_from_clanwh,
            sorting_private: config.warehouse_sorting.enable_warehousesorting_private,
            sorting_clan: config.warehouse_sorting.enable_warehousesorting_clan,
        },
        craft: l2_world::CraftSettings {
            is_crafting_enabled: p.is_crafting_enabled,
            crafting_in_combat: p.crafting_in_combat,
            craft_masterwork: p.craft_masterwork,
            craft_masterwork_chance_rate: p.craft_masterwork_chance_rate,
            alt_game_creation: p.alt_game_creation,
            alt_game_creation_speed: p.alt_game_creation_speed,
            alt_game_creation_xp_rate: p.alt_game_creation_xp_rate,
            alt_game_creation_sp_rate: p.alt_game_creation_sp_rate,
            alt_game_creation_rare_xpsp_rate: p.alt_game_creation_rare_xpsp_rate,
            store_recipe_shoplist: p.store_recipe_shoplist,
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

/// The fishing values of `General.ini` (`AllowFishing`, the `AltFishChampionship*` keys) and
/// `FishingChampionshipDatabaseWrites` of `SideBySide.ini` (D-49).
pub fn fishing_settings(config: &GameConfig, config_dir: &Path) -> l2_world::FishingSettings {
    let g = &config.general;
    l2_world::FishingSettings {
        allow_fishing: g.allow_fishing,
        championship_enabled: g.alt_fish_championship_enabled,
        championship_reward_item: g.alt_fish_championship_reward_item,
        championship_rewards: [
            g.alt_fish_championship_reward_1,
            g.alt_fish_championship_reward_2,
            g.alt_fish_championship_reward_3,
            g.alt_fish_championship_reward_4,
            g.alt_fish_championship_reward_5,
        ],
        championship_database_writes: fishing_championship_database_writes(config_dir),
        clock_override_night: None,
        clock_override_unix_ms: None,
    }
}

/// `FishingChampionshipDatabaseWrites` of `config/SideBySide.ini` (default False; a missing file
/// means the default). Off, the championship keeps its ranking in memory and leaves the
/// `fishing_championship` rows and the `fishChampionshipEnd` variable to the Java game server that
/// shares the database (D-39, D-49): its `shutdown()` deletes and rewrites every row. Logs the
/// active mode.
pub fn fishing_championship_database_writes(config_dir: &Path) -> bool {
    use l2_config::IniFile;

    let path = config_dir.join(SIDE_BY_SIDE_FILE);
    let on = if path.exists() {
        match IniFile::load(&path) {
            Ok(ini) => ini.get_bool("FishingChampionshipDatabaseWrites", false),
            Err(e) => {
                warn!(
                    "Cannot read {}: {e}; using FishingChampionshipDatabaseWrites = False.",
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
            "SideBySide: FishingChampionshipDatabaseWrites = True, the fishing championship writes its rows."
        );
    } else {
        info!(
            "SideBySide: FishingChampionshipDatabaseWrites = False, the fishing championship leaves its rows to the Java game server."
        );
    }
    on
}

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

/// Name of the siege config file in the config folder (Java `SiegeManager.SIEGE_CONFIG_FILE`).
pub const SIEGE_FILE: &str = "Siege.ini";

/// Java `SiegeManager.load()` (M7-SIEGE-01): `Siege.ini` with the tower lines and mercenary limits
/// of each castle (`castles`: id and name, in `CastleManager` order), plus the siege values of
/// `Feature.ini` (`SiegeHourList`, `AllowRideMountsDuringSiege`), `Player.ini` (the fame of the
/// castle zone) and `OfflineTrade.ini` (`OfflineFame`).
pub fn siege_settings(
    config: &GameConfig,
    config_dir: &Path,
    castles: &[(i32, String)],
) -> l2_world::SiegeSettings {
    use l2_config::IniFile;

    let path = config_dir.join(SIEGE_FILE);
    let ini = match IniFile::load(&path) {
        Ok(ini) => ini,
        Err(e) => {
            warn!("Cannot read {}: {e}; using the siege defaults.", path.display());
            IniFile::empty(SIEGE_FILE)
        }
    };
    let mut s = siege_settings_from_ini(&ini, castles);
    // `FeatureConfig.SIEGE_HOUR_LIST`: the numeric parts of `SiegeHourList` (`StringUtil.isNumeric`).
    if let Ok(feature) = IniFile::load(config_dir.join(l2_config::game::FeatureConfig::FILE)) {
        s.siege_hour_list = feature
            .get_string("SiegeHourList", "")
            .split(',')
            .filter(|h| !h.is_empty() && h.chars().all(|c| c.is_ascii_digit()))
            .filter_map(|h| h.parse().ok())
            .collect();
    }
    s.allow_mounts_during_siege = config.feature.allow_mounts_during_siege;
    s.fame_system_enabled = config.player.fame_system_enabled;
    s.castle_zone_fame_task_frequency = config.player.castle_zone_fame_task_frequency;
    s.castle_zone_fame_aquire_points = config.player.castle_zone_fame_aquire_points;
    s.fame_for_dead_players = config.player.fame_for_dead_players;
    s.offline_fame = config.offline_trade.offline_fame;
    s.teleport_while_siege_in_progress = config.player.teleport_while_siege_in_progress;
    s
}

/// The `Siege.ini` part of [`siege_settings`].
pub fn siege_settings_from_ini(
    ini: &l2_config::IniFile,
    castles: &[(i32, String)],
) -> l2_world::SiegeSettings {
    let mut s = l2_world::SiegeSettings {
        siege_cycle: ini.get_int("SiegeCycle", 2),
        attacker_max_clans: ini.get_int("AttackerMaxClans", 500),
        attacker_respawn_delay: ini.get_int("AttackerRespawn", 0),
        defender_max_clans: ini.get_int("DefenderMaxClans", 500),
        flag_max_count: ini.get_int("MaxFlags", 1),
        siege_clan_min_level: ini.get_int("SiegeClanMinLevel", 5),
        siege_length: ini.get_int("SiegeLength", 120),
        blood_alliance_reward: ini.get_int("BloodAllianceReward", 1),
        ..l2_world::SiegeSettings::default()
    };
    for (id, name) in castles {
        // `Integer.parseInt` of each token: a token that is not a number (spaces included) skips
        // the tower with Java's warning.
        let tower = |key: &str, flame: bool| -> Option<Option<l2_world::TowerSpawn>> {
            if !ini.contains_key(key) {
                return None;
            }
            let value = ini.get_string(key, "");
            let tokens: Vec<&str> = value.split(',').filter(|t| !t.is_empty()).collect();
            // A control tower reads four tokens, a flame tower every token.
            let wanted = if flame { tokens.len() } else { tokens.len().min(4) };
            let numbers: Option<Vec<i32>> = tokens[..wanted].iter().map(|t| t.parse::<i32>().ok()).collect();
            let parsed = numbers.filter(|n| n.len() >= 4).map(|n| l2_world::TowerSpawn {
                npc_id: n[3],
                x: n[0],
                y: n[1],
                z: n[2],
                zone_list: n[4..].to_vec(),
                upgrade_level: 0,
            });
            if parsed.is_none() {
                let kind = if flame { "flame" } else { "control" };
                warn!("SiegeManager: Error while loading {kind} tower(s) for {name} castle.");
            }
            Some(parsed)
        };
        let mut config = l2_world::CastleSiegeConfig::default();
        for i in 1..0xFF {
            match tower(&format!("{name}ControlTower{i}"), false) {
                None => break,
                Some(spawn) => config.control_towers.extend(spawn),
            }
        }
        for i in 1..0xFF {
            match tower(&format!("{name}FlameTower{i}"), true) {
                None => break,
                Some(spawn) => config.flame_towers.extend(spawn),
            }
        }
        let default_max = usize::try_from(id - 1)
            .ok()
            .and_then(|i| l2_world::siege::MERCS_MAX_PER_CASTLE.get(i).copied())
            .unwrap_or(0);
        config.max_mercenaries = ini.get_int(&format!("{name}MaxMercenaries"), default_max);
        s.castles.insert(*id, config);
    }
    s
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    /// The shipped `BotReportPunishments.xml`: exact counts and the negative range key.
    #[test]
    fn the_bot_report_punishments_are_read() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<list>
	<!-- a comment -->
	<punishment neededReportCount="25" skillId="6038" skillLevel="1" sysMessageId="2473" />
	<punishment neededReportCount="150" skillId="6057" sysMessageId="2480" />
	<punishment neededReportCount="-150" skillId="6040" skillLevel="2" />
	<punishment skillId="1" />
</list>"#;
        let punishments = bot_report_punishments_from_xml(xml);
        let punishment = |skill_id, skill_level, system_message| l2_world::BotPunishment {
            skill_id,
            skill_level,
            system_message,
        };
        assert_eq!(
            punishments,
            BTreeMap::from([
                (-150, punishment(6040, 2, -1)),
                // `skillLevel` defaults to 1.
                (25, punishment(6038, 1, 2473)),
                (150, punishment(6057, 1, 2480)),
            ])
        );
    }

    /// `BotReportDelay` is minutes, the reset hour is `HH:MM` (anything else: no hour, the reset
    /// then runs every 24 hours), the punishments are only read with the button on.
    #[test]
    fn the_bot_report_settings_come_from_general_ini() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("General.ini"),
            "EnableBotReportButton = True
BotReportPointsResetHour = 06:30
BotReportDelay = 45
AllowReportsFromSameClanMembers = True
",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("BotReportPunishments.xml"),
            r#"<list><punishment neededReportCount="25" skillId="6038" sysMessageId="2473" /></list>"#,
        )
        .unwrap();
        let config = GameConfig::load(dir.path()).unwrap();
        let settings = bot_report_settings(&config, dir.path());
        assert!(settings.enable);
        assert!(settings.allow_reports_from_same_clan_members);
        assert_eq!(settings.report_delay_ms, 45 * 60_000);
        assert_eq!(settings.reset_hour_minute, Some((6, 30)));
        assert_eq!(settings.punishments.len(), 1);

        std::fs::write(
            dir.path().join("General.ini"),
            "EnableBotReportButton = True
BotReportPointsResetHour = noon
",
        )
        .unwrap();
        let config = GameConfig::load(dir.path()).unwrap();
        let settings = bot_report_settings(&config, dir.path());
        assert_eq!(settings.reset_hour_minute, None);
        assert_eq!(settings.report_delay_ms, 30 * 60_000);

        // Off by default: no punishments are read.
        std::fs::write(dir.path().join("General.ini"), "").unwrap();
        let config = GameConfig::load(dir.path()).unwrap();
        let settings = bot_report_settings(&config, dir.path());
        assert!(!settings.enable);
        assert!(settings.punishments.is_empty());
        assert_eq!(settings.reset_hour_minute, Some((0, 0)));
    }

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

    #[test]
    fn the_fishing_championship_switch_defaults_to_off() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!fishing_championship_database_writes(dir.path()));
        let example = include_str!("../config-examples/SideBySide.ini");
        std::fs::write(dir.path().join(SIDE_BY_SIDE_FILE), example).unwrap();
        assert!(!fishing_championship_database_writes(dir.path()));
        assert!(example.contains("FishingChampionshipDatabaseWrites = False"));
        std::fs::write(
            dir.path().join(SIDE_BY_SIDE_FILE),
            "FishingChampionshipDatabaseWrites = True
",
        )
        .unwrap();
        assert!(fishing_championship_database_writes(dir.path()));
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
             PartyXpCutoffGapPercent = 100;50\nRaidLootRightsInterval = 60\nRaidLootRightsCCSize = 12\n",
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
        assert_eq!(settings.loot_raids_privilege_interval_ms, 60_000);
        assert_eq!(settings.loot_raids_privilege_cc_size, 12);
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

    /// M5-FU-04: `DefaultPunish`, `DefaultPunishParam`, `JailDisableChat` and `BanChatChannels`.
    #[test]
    fn punishment_settings_are_read_from_general_ini() {
        use l2_protocol_game::server::ChatType;
        use l2_world::IllegalActionPunishment;

        // Java defaults: a missing file keeps the shipped values (KICK, one hundred years).
        let dir = tempfile::tempdir().unwrap();
        let config = GameConfig::load(dir.path()).unwrap();
        let settings = punishment_settings(&config, dir.path());
        assert_eq!(settings.default_punish, IllegalActionPunishment::Kick);
        assert_eq!(settings.default_punish_param, 3_155_695_200);
        assert!(settings.jail_disable_chat);

        std::fs::write(
            dir.path().join("General.ini"),
            "DefaultPunish = kickban\nDefaultPunishParam = 90\nJailDisableChat = False\nBanChatChannels = SHOUT;PARTY;BOGUS;SHOUT\n",
        )
        .unwrap();
        let config = GameConfig::load(dir.path()).unwrap();
        let settings = punishment_settings(&config, dir.path());
        assert_eq!(settings.default_punish, IllegalActionPunishment::KickBan);
        assert_eq!(settings.default_punish_param, 90);
        assert!(!settings.jail_disable_chat);
        // A name Java would throw on is skipped; a channel listed twice counts once.
        assert_eq!(settings.ban_chat_channels, [ChatType::Shout, ChatType::Party]);
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
    fn allow_altg_is_true_only_where_the_file_says_so() {
        let levels = AccessLevels::from_xml(
            r#"<list>
	<access level="0" name="User" allowAltg="false" />
	<access level="20" name="Test GM" allowAltg="true" />
	<access level="70" name="Admin" />
</list>"#,
        );
        assert!(!levels.allows_alt_g(0));
        assert!(levels.allows_alt_g(20));
        // `set.getBoolean("allowAltg", false)`: absent is false.
        assert!(!levels.allows_alt_g(70));
        assert!(!levels.allows_alt_g(-1));
    }

    #[test]
    fn allow_transaction_is_false_only_where_the_file_says_so() {
        let levels = AccessLevels::from_xml(
            r#"<list>
	<access level="-1" name="Banned" allowTransaction="false" />
	<access level="0" name="User" allowTransaction="true" />
	<access level="20" name="Test GM" allowTransaction="false" />
	<access level="70" name="Admin" />
</list>"#,
        );
        // `set.getBoolean("allowTransaction", true)`: absent is true.
        assert!(levels.allows_transaction(0));
        assert!(levels.allows_transaction(70));
        assert!(!levels.allows_transaction(-1));
        assert!(!levels.allows_transaction(-5));
        assert!(!levels.allows_transaction(20));
        // A level the file does not list is Java's default AccessLevel, which may trade.
        assert!(levels.allows_transaction(55));
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

    /// The `PcCafe*` keys reach the world with the corrections `PremiumSystemConfig.load` makes: the
    /// defaults without a file, and out of range values fixed.
    #[test]
    fn the_pc_cafe_settings_follow_the_premium_system_file() {
        assert_eq!(
            pc_cafe_settings(&GameConfig::default()),
            l2_world::PcCafeSettings::default()
        );

        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("Custom")).unwrap();
        std::fs::write(
            dir.path().join("Custom/PremiumSystem.ini"),
            "PcCafeEnabled = True
PcCafeRetailLike = False
PcCafeOnlyPremium = True
PcCafeRewardTime = 60000
MaxPcCafePoints = -5
DoublingAcquisitionPoints = True
DoublingAcquisitionPointsChance = 101
AcquisitionPointsRetailLikePoints = 7
AcquisitionPointsRate = -2.0
AcquisitionPointsRandom = True
RewardLowExpKills = False
RewardLowExpKillsChance = 250
",
        )
        .unwrap();
        let config = GameConfig::load(dir.path()).unwrap();
        let settings = pc_cafe_settings(&config);
        assert!(settings.enabled && !settings.retail_like && settings.only_premium);
        assert_eq!(settings.reward_time_ms, 60_000);
        // `MaxPcCafePoints < 0` is 0, a chance outside 0 to 100 is 1, a negative rate is 1, the low
        // exp chance is held to 0 to 100.
        assert_eq!(settings.max_points, 0);
        assert!(settings.enable_double_points);
        assert_eq!(settings.double_points_chance, 1);
        assert_eq!(settings.retail_like_points, 7);
        assert_eq!(settings.point_rate, 1.0);
        assert!(settings.random_point && !settings.reward_low_exp_kills);
        assert_eq!(settings.low_exp_kills_chance, 100);
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

    /// `PVP.ini` and `Custom/PvpTitleColor.ini` (M6-PVP-01): the Java defaults without files, and
    /// the post-processed keys (`AntiFeedInterval` in seconds, `ColorForAmountN` as hex) from them.
    #[test]
    fn the_pvp_settings_are_read() {
        let dir = tempfile::tempdir().unwrap();
        let config = GameConfig::load(dir.path()).unwrap();
        let s = pvp_settings(&config, dir.path());
        assert_eq!(s, l2_world::PvpSettings::default());
        assert_eq!((s.pvp_normal_time_ms, s.pvp_pvp_time_ms), (120_000, 60_000));
        assert_eq!(s.antifeed_interval_ms, 120_000);

        std::fs::write(
            dir.path().join("PVP.ini"),
            "PvPVsNormalTime = 5000
PvPVsPvPTime = 2500
AntiFeedEnable = True
AntiFeedInterval = 30
             FlagPlayerOnRaidAttack = False
AwardPKKillPVPPoint = True
",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("Custom")).unwrap();
        std::fs::write(
            dir.path().join("Custom").join("PvpTitleColor.ini"),
            "EnablePvPColorSystem = True
PvpAmount1 = 7
ColorForAmount1 = 9C9C9C
ColorForAmount2 = nonsense
             PvPTitleForAmount1 = Sergeant
",
        )
        .unwrap();
        let config = GameConfig::load(dir.path()).unwrap();
        let s = pvp_settings(&config, dir.path());
        assert_eq!((s.pvp_normal_time_ms, s.pvp_pvp_time_ms), (5_000, 2_500));
        assert!(s.antifeed_enable && s.karma_award_pk_kill && !s.flag_player_on_raid_attack);
        assert_eq!(s.antifeed_interval_ms, 30_000);
        assert!(s.title_color.enabled);
        assert_eq!(s.title_color.amounts[0], 7);
        assert_eq!(s.title_color.titles[0], "Sergeant");
        // `Integer.decode("0x" + text)`.
        assert_eq!(s.title_color.colors[0], 0x009C_9C9C);
        // Java would throw for a bad colour; the default stays.
        assert_eq!(s.title_color.colors[1], 0x0000_FF00);
    }

    #[test]
    fn flood_protector_ini_gives_every_key_and_java_defaults_for_the_rest() {
        use l2_world::{FloodKind, FloodPunishment};

        // No file: the defaults of `FloodProtectorConfig`, not the numbers of the shipped file.
        let dir = tempfile::tempdir().unwrap();
        let defaults = flood_settings(dir.path());
        assert_eq!(defaults, l2_world::FloodSettings::java_defaults());
        assert_eq!(defaults.get(FloodKind::UseItem).protection_interval, 4);

        std::fs::write(
            dir.path().join(FLOOD_PROTECTOR_FILE),
            "FloodProtectorUseItemInterval = 0
FloodProtectorTransactionInterval = 7
FloodProtectorTransactionLogFlooding = True
FloodProtectorTransactionPunishmentLimit = 12
FloodProtectorTransactionPunishmentType = ban
FloodProtectorTransactionPunishmentTime = 5
FloodProtectorGlobalChatPunishmentType = kick
FloodProtectorPlayerActionInterval = 1
",
        )
        .unwrap();
        let s = flood_settings(dir.path());
        assert_eq!(s.get(FloodKind::UseItem).protection_interval, 0);
        let transaction = s.get(FloodKind::Transaction);
        assert_eq!(transaction.protection_interval, 7);
        assert!(transaction.log_flooding);
        assert_eq!(transaction.punishment_limit, 12);
        assert_eq!(transaction.punishment_type, FloodPunishment::Ban);
        // Minutes in the file, milliseconds in the settings.
        assert_eq!(transaction.punishment_time, 300_000);
        assert_eq!(
            s.get(FloodKind::GlobalChat).punishment_type,
            FloodPunishment::Kick
        );
        assert_eq!(s.get(FloodKind::GlobalChat).protection_interval, 5);
        assert_eq!(s.get(FloodKind::PlayerAction).protection_interval, 1);
        // A key that is missing keeps its own default.
        assert_eq!(s.get(FloodKind::HeroVoice).protection_interval, 100);
        assert_eq!(
            s.get(FloodKind::HeroVoice).punishment_type,
            FloodPunishment::Unknown("none".to_owned())
        );
    }

    /// The shipped `FloodProtector.ini` of the reference distribution: all fifteen protectors and
    /// every one of the five keys is read. Skipped without the reference folder.
    #[test]
    fn the_shipped_flood_protector_ini_is_read_completely() {
        use l2_world::FloodKind;

        let root = PathBuf::from("C:/l2-rust/l2jmobius/dist/game/config");
        if !root.join(FLOOD_PROTECTOR_FILE).exists() {
            eprintln!("skipped: the reference config folder is missing");
            return;
        }
        let s = flood_settings(&root);
        let intervals: Vec<i32> = FloodKind::ALL
            .iter()
            .map(|k| s.get(*k).protection_interval)
            .collect();
        // The numbers of the shipped file (some differ from the code defaults, e.g. UseItem 0).
        assert_eq!(
            intervals,
            [0, 42, 16, 100, 5, 20, 10, 3, 10, 10, 3, 100, 30, 9, 1]
        );
        for kind in FloodKind::ALL {
            assert!(!s.get(kind).log_flooding, "{kind:?}");
            assert_eq!(s.get(kind).punishment_limit, 0, "{kind:?}");
        }
    }
}

#[cfg(test)]
mod siege_tests {
    use super::*;

    /// `SiegeManager.load()` over the lines of the shipped `Siege.ini`: the values, the towers of
    /// a castle by its name (a flame tower with its trap zones), `MaxMercenaries`, and a broken
    /// line skipped with Java's warning.
    #[test]
    fn siege_ini_reads_like_java() {
        let text = [
            "SiegeCycle = 2",
            "SiegeLength = 120",
            "MaxFlags = 1",
            "AttackerRespawn = 0",
            "BloodAllianceReward = 1",
            "GludioFlameTower1=-18154,107591,-2560,13004,70017,70019",
            "GludioControlTower1=-18325,112811,-2377,13002",
            "GludioControlTower2=-18048,107098,-2378,13002",
            "GludioControlTower3=-18113,x,-2343,13002",
            "GludioControlTower5=1,2,3,13002",
            "GludioMaxMercenaries = 120",
        ]
        .join("\n");
        let ini = l2_config::IniFile::parse("Siege.ini", &text).unwrap();
        let castles = [(1, "Gludio".to_owned()), (2, "Dion".to_owned())];
        let s = siege_settings_from_ini(&ini, &castles);
        assert_eq!((s.siege_cycle, s.siege_length, s.flag_max_count), (2, 120, 1));
        assert_eq!(s.blood_alliance_reward, 1);
        let gludio = &s.castles[&1];
        // Tower 3 does not parse and is skipped; tower 4 is missing, so tower 5 is never read.
        assert_eq!(gludio.control_towers.len(), 2);
        assert_eq!(
            (gludio.control_towers[0].x, gludio.control_towers[0].npc_id),
            (-18325, 13002)
        );
        assert_eq!(gludio.flame_towers.len(), 1);
        assert_eq!(gludio.flame_towers[0].zone_list, [70017, 70019]);
        assert_eq!(gludio.max_mercenaries, 120);
        // Dion has no lines: Java's table value of `MERCS_MAX_PER_CASTLE`.
        assert!(s.castles[&2].control_towers.is_empty());
        assert_eq!(s.castles[&2].max_mercenaries, 150);
    }
}
