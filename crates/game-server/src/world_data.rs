//! The datapack as the world sees it ([`l2_world::StaticData`]), the base stat formulas of
//! `UserInfo`, and the settings the world reads from the configuration.
// Java: mechanics/stats/BaseStat.java + functions/formulas/Func*.java (the base formulas)
// Java: entity/actor/Creature.java (getMaxLoad, getLevelMod), stat/CreatureStat.java
// Java: data/xml/{AdminData,SkillData}.java, data/holders/AccessLevel.java
//
// What Java computes with its stat system (`Calculator` chains of base formulas, equipment
// functions, passive skills, effects) is only available here as the base formulas: the numbers
// are those of an unequipped character without skills or buffs (so a weapon's P. Atk. or armor's
// P. Def. are missing from the `UserInfo` window). The stat system itself is M4.

use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use std::sync::Arc;

use l2_config::GameConfig;
use l2_data::Datapack;
use l2_data::ItemKind;
use l2_world::{
    AccessLevelStatic, ChatBroadcast, DerivedStats, ItemStatic, PlayerData, StaticData, WorldSettings,
};
use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};
use tracing::{info, warn};

/// `BaseStat.MAX_STAT_VALUE`.
const MAX_STAT_VALUE: usize = 100;
/// `CommonSkill.CREATE_DWARVEN` and friends are not needed here; weights use `ALT_WEIGHT_LIMIT`.
const WEIGHT_BASE: f64 = 69_000.0;

/// All six columns of `data/stats/statBonus.xml` (Java `BaseStat`).
#[derive(Debug, Clone, PartialEq)]
pub struct BaseStatBonus {
    str: Vec<f64>,
    int: Vec<f64>,
    dex: Vec<f64>,
    wit: Vec<f64>,
    con: Vec<f64>,
    men: Vec<f64>,
}

impl Default for BaseStatBonus {
    /// Java's arrays start at 0.0 and the file fills them in.
    fn default() -> Self {
        let zeros = vec![0.0; MAX_STAT_VALUE];
        Self {
            str: zeros.clone(),
            int: zeros.clone(),
            dex: zeros.clone(),
            wit: zeros.clone(),
            con: zeros.clone(),
            men: zeros,
        }
    }
}

impl BaseStatBonus {
    /// Reads `<root>/data/stats/statBonus.xml`. A missing file leaves every bonus at 0 (Java
    /// throws and the server does not start).
    pub fn load(root: &Path) -> Self {
        let path = root.join("data/stats/statBonus.xml");
        match std::fs::read(&path) {
            Ok(bytes) => Self::from_xml(&String::from_utf8_lossy(&bytes)),
            Err(e) => {
                warn!("[BaseStats] Could not read {}: {e}", path.display());
                Self::default()
            }
        }
    }

    /// Parses the file text. A broken value is skipped like Java's "Invalid stats value".
    pub fn from_xml(xml: &str) -> Self {
        let mut table = Self::default();
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
                    let target = match section.as_deref().map(str::to_ascii_uppercase).as_deref() {
                        Some("STR") => Some(&mut table.str),
                        Some("INT") => Some(&mut table.int),
                        Some("DEX") => Some(&mut table.dex),
                        Some("WIT") => Some(&mut table.wit),
                        Some("CON") => Some(&mut table.con),
                        Some("MEN") => Some(&mut table.men),
                        _ => None,
                    };
                    if let Some(slot) = target.and_then(|t| t.get_mut(value)) {
                        *slot = bonus;
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

    fn at(table: &[f64], value: i32) -> f64 {
        usize::try_from(value)
            .ok()
            .and_then(|v| table.get(v))
            .copied()
            .unwrap_or(0.0)
    }
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

/// The ids of the passive skills (`operateType` `P`) in `data/stats/skills/*.xml`.
///
/// The skills are not loaded as such yet (M4). `SkillList` only needs this one flag, so the files
/// are scanned for it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PassiveSkills {
    ids: HashSet<i32>,
}

impl PassiveSkills {
    /// Scans `<root>/data/stats/skills`. A missing folder gives no passives (every skill is then
    /// listed as active).
    pub fn load(root: &Path) -> Self {
        let dir = root.join("data/stats/skills");
        let mut table = Self::default();
        let Ok(entries) = std::fs::read_dir(&dir) else {
            warn!("Directory not found: {}", dir.display());
            return table;
        };
        let mut files: Vec<_> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("xml")))
            .collect();
        files.sort();
        for file in files {
            match std::fs::read(&file) {
                Ok(bytes) => table.add_xml(&String::from_utf8_lossy(&bytes)),
                Err(e) => warn!("Cannot read {}: {e}", file.display()),
            }
        }
        info!("SkillData: found {} passive skills.", table.ids.len());
        table
    }

    /// Adds the passive skills of one file's text.
    pub fn add_xml(&mut self, xml: &str) {
        let mut reader = Reader::from_str(xml);
        let mut current: Option<i32> = None;
        let mut in_operate_type = false;
        loop {
            match reader.read_event() {
                Ok(Event::Start(e)) => match e.name().as_ref() {
                    "skill" => current = attribute(&e, "id").and_then(|v| v.trim().parse().ok()),
                    "operateType" => in_operate_type = true,
                    _ => {}
                },
                Ok(Event::Empty(e)) if e.name().as_ref() == "skill" => current = None,
                Ok(Event::Text(t)) if in_operate_type => {
                    if t.xml10_content().trim() == "P"
                        && let Some(id) = current
                    {
                        self.ids.insert(id);
                    }
                }
                Ok(Event::End(e)) => match e.name().as_ref() {
                    "skill" => current = None,
                    "operateType" => in_operate_type = false,
                    _ => {}
                },
                Ok(Event::Eof) | Err(_) => break,
                Ok(_) => {}
            }
        }
    }

    pub fn contains(&self, skill_id: i32) -> bool {
        self.ids.contains(&skill_id)
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }
}

/// `config/AccessLevels.xml` (Java `AdminData.loadAccessLevels`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AccessLevels {
    levels: BTreeMap<i32, AccessLevelStatic>,
}

impl AccessLevels {
    pub const FILE: &'static str = "AccessLevels.xml";

    pub fn load(config_dir: &Path) -> Self {
        let path = config_dir.join(Self::FILE);
        match std::fs::read(&path) {
            Ok(bytes) => Self::from_xml(&String::from_utf8_lossy(&bytes)),
            Err(e) => {
                warn!("Cannot read {}: {e}; every access level is a plain user.", path.display());
                Self::default()
            }
        }
    }

    pub fn from_xml(xml: &str) -> Self {
        let mut table = Self::default();
        let mut reader = Reader::from_str(xml);
        loop {
            match reader.read_event() {
                Ok(Event::Empty(e) | Event::Start(e)) if e.name().as_ref() == "access" => {
                    let Some(level) = attribute(&e, "level").and_then(|v| v.trim().parse::<i32>().ok()) else {
                        continue;
                    };
                    // Java: `Integer.decode("0x" + set.getString("nameColor", "FFFFFF"))`
                    let color = |key: &str| {
                        attribute(&e, key)
                            .and_then(|v| i32::from_str_radix(v.trim(), 16).ok())
                            .unwrap_or(0x00FF_FFFF)
                    };
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

/// The limits of `PlayerConfig` that cap the stats `UserInfo` shows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StatLimits {
    pub max_patk: i32,
    pub max_matk: i32,
    pub max_pcrit_rate: i32,
    pub max_evasion: i32,
    pub max_patk_speed: i32,
    pub max_matk_speed: i32,
    pub max_run_speed: i32,
    pub alt_weight_limit: f64,
}

impl StatLimits {
    pub fn from_config(config: &GameConfig) -> Self {
        let p = &config.player;
        Self {
            max_patk: p.max_patk,
            max_matk: p.max_matk,
            max_pcrit_rate: p.max_pcrit_rate,
            max_evasion: p.max_evasion,
            max_patk_speed: p.max_patk_speed,
            max_matk_speed: p.max_matk_speed,
            max_run_speed: p.max_run_speed,
            alt_weight_limit: p.alt_weight_limit,
        }
    }
}

/// [`StaticData`] over the loaded datapack.
pub struct DatapackData {
    datapack: Arc<Datapack>,
    bonus: BaseStatBonus,
    passive: PassiveSkills,
    access: AccessLevels,
    limits: StatLimits,
}

impl DatapackData {
    pub fn new(
        datapack: Arc<Datapack>,
        bonus: BaseStatBonus,
        passive: PassiveSkills,
        access: AccessLevels,
        limits: StatLimits,
    ) -> Self {
        Self {
            datapack,
            bonus,
            passive,
            access,
            limits,
        }
    }

    /// Loads the extra tables from the datapack folder and the config folder.
    pub fn load(datapack: Arc<Datapack>, config: &GameConfig, config_dir: &Path) -> Self {
        let bonus = BaseStatBonus::load(&datapack.root);
        let passive = PassiveSkills::load(&datapack.root);
        let access = AccessLevels::load(config_dir);
        Self::new(datapack, bonus, passive, access, StatLimits::from_config(config))
    }
}

/// Java `Creature.getLevelMod()`.
fn level_mod(level: i32) -> f64 {
    f64::from(level + 89) / 100.0
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

    fn skill_is_passive(&self, skill_id: i32) -> bool {
        self.passive.contains(skill_id)
    }

    fn exp_percent(&self, exp: i64, level: i32) -> f64 {
        self.datapack.experience.exp_percent(exp, level).unwrap_or(0.0)
    }

    fn derive_stats(&self, player: &PlayerData) -> DerivedStats {
        let Some(t) = self.datapack.player_templates.get_by_id(player.class_id) else {
            warn!("No class template for class {}: using default stats.", player.class_id);
            return DerivedStats::default();
        };
        let b = &self.bonus;
        let l = &self.limits;
        let level = player.level;
        let lvl = level_mod(level);
        let (str, dex, con, int, wit, men) = (
            t.base_str, t.base_dex, t.base_con, t.base_int, t.base_wit, t.base_men,
        );
        let str_b = BaseStatBonus::at(&b.str, str);
        let int_b = BaseStatBonus::at(&b.int, int);
        let dex_b = BaseStatBonus::at(&b.dex, dex);
        let wit_b = BaseStatBonus::at(&b.wit, wit);
        let con_b = BaseStatBonus::at(&b.con, con);
        let men_b = BaseStatBonus::at(&b.men, men);
        let gm = self.access.get(player.access_level).is_gm;

        // FuncPAtkMod, FuncMAtkMod, FuncPDefMod / FuncMDefMod (nothing equipped)
        let p_atk = (f64::from(t.p_atk) * str_b * lvl).min(f64::from(l.max_patk));
        let m_atk = (f64::from(t.m_atk) * (int_b * lvl).powi(2)).min(f64::from(l.max_matk));
        let p_def = f64::from(t.p_def) * lvl;
        let m_def = f64::from(t.m_def) * men_b * lvl;

        // FuncAtkAccuracy (no weapon bonus)
        let mut accuracy = f64::from(dex).sqrt() * 6.0 + f64::from(level);
        if level > 77 {
            accuracy += f64::from(level - 76);
        }
        if level > 69 {
            accuracy += f64::from(level - 69);
        }
        // FuncAtkEvasion (player branch)
        let mut evasion = f64::from(dex).sqrt() * 6.0 + f64::from(level);
        let mut diff = f64::from(level - 69);
        if level >= 78 {
            diff *= 1.2;
        }
        if level >= 70 {
            evasion += diff;
        }
        let evasion = (evasion.round() as i32).min(l.max_evasion);
        // FuncAtkCritical, then CreatureStat.getCriticalHit
        let critical = (f64::from(t.crit_rate) * dex_b * 10.0).min(f64::from(l.max_pcrit_rate));
        // FuncPAtkSpeed / FuncMAtkSpeed
        let mut p_atk_spd = (f64::from(t.p_atk_spd) * dex_b).round();
        if !gm && p_atk_spd > f64::from(l.max_patk_speed) {
            p_atk_spd = f64::from(l.max_patk_speed);
        }
        let m_atk_spd = (f64::from(t.m_atk_spd) * wit_b).min(f64::from(l.max_matk_speed));

        // FuncMoveSpeed: every speed is multiplied by the DEX bonus, and `UserInfo` divides the
        // movement multiplier out again, so the numbers shown are the template speeds.
        let move_multiplier = dex_b;
        let speed = |base: f64| (base * dex_b / move_multiplier).round() as i32;
        let female = player.sex != 0;
        let collision = if female { t.collision_female } else { t.collision_male };

        // Creature.getMaxLoad: floor(CON bonus * 69000 * AltWeightLimit)
        let max_load = (con_b * WEIGHT_BASE * l.alt_weight_limit).floor() as i32;

        let mut stats = DerivedStats {
            str,
            dex,
            con,
            int,
            wit,
            men,
            // `(int)` casts truncate
            p_atk: p_atk as i32,
            p_atk_spd: p_atk_spd as i32,
            p_def: p_def as i32,
            evasion,
            accuracy: accuracy.round() as i32,
            critical: (critical + 0.5) as i32,
            m_atk: m_atk as i32,
            m_atk_spd: m_atk_spd as i32,
            m_def: m_def as i32,
            run_speed: speed(t.run_speed),
            walk_speed: speed(t.walk_speed),
            swim_run_speed: speed(t.fast_swim_speed),
            swim_walk_speed: speed(t.slow_swim_speed),
            move_multiplier,
            // CreatureStat.getAttackSpeedMultiplier: (float) ((1.1 * getPAtkSpd()) / basePAtkSpd)
            attack_speed_multiplier: ((1.1 * p_atk_spd) / f64::from(t.p_atk_spd)) as f32,
            collision_radius: collision.radius,
            collision_height: collision.height,
            max_load,
        };
        if !gm && stats.run_speed > l.max_run_speed {
            stats.run_speed = l.max_run_speed;
        }
        stats
    }

    fn access_level(&self, level: i32) -> AccessLevelStatic {
        self.access.get(level)
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
        gm_hero_aura: config.general.gm_hero_aura,
        log_chat: config.general.log_chat,
        ..WorldSettings::default()
    };
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
