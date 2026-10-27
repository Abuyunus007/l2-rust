//! Hand-ported game config fields of the core (non-custom) classes (L-09): the `()` placeholders
//! of the generated structs that `game_manual.rs` (M2-CFG-01) did not cover.
// Java: org/l2jmobius/gameserver/config/{ServerConfig,DevelopmentConfig,FeatureConfig,
// FloodProtectorConfig,GeneralConfig,GeoEngineConfig,GraciaSeedsConfig,IdManagerConfig,NpcConfig,
// OlympiadConfig,PlayerConfig,PvpConfig,RatesConfig}.java
//
// One struct per Java class, reading the same keys with the same defaults, split rules and unit
// conversions as `load()`. They live in `GameManual::core`. The values the world needs reach it
// through the readers in `crates/game-server/src/world_data.rs`.
//
// Conventions of this file:
// * `ServerRest`, `NpcRest` and `PlayerRest` hold what `ServerManual`, `NpcManual` and
//   `PlayerManual` (game_manual.rs) do not.
// * A Java enum stays a name here (`String`): the enum belongs to the crate that uses it
//   (`l2-world`), which maps the name with its own `from_name`, as before.
// * Java throws (`NumberFormatException`, `ArrayIndexOutOfBoundsException`) while loading a list
//   with a bad piece and the server does not start. Here the piece is logged and skipped, like
//   the readers that existed before this file. Pieces are trimmed first (Java does not), so
//   `1, 2` works here and fails in Java.
// * `int` fields that Java multiplies (`getInt(..) * 1000`) wrap like Java's `int` arithmetic.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use tracing::{info, warn};

use crate::game_manual::{java_split, java_trim};
use crate::reader::parse_java_float;
use crate::{GameConfig, IniFile};

/// `ServerConfig.CHAT_FILTER_FILE` (below the config folder).
pub const CHAT_FILTER_FILE: &str = "chatfilter.txt";

/// Java `Integer.parseInt` of one piece of a config list. The piece is trimmed (see the top of
/// the file); a bad one is logged under `key` and skipped.
fn parse_piece(key: &str, piece: &str) -> Option<i32> {
    match java_trim(piece).parse::<i32>() {
        Ok(value) => Some(value),
        Err(_) => {
            warn!("{key}: \"{piece}\" is not a number; skipped.");
            None
        }
    }
}

/// `text.split(separator)` and `Integer.parseInt` of each piece, in order.
fn int_list(key: &str, text: &str, separator: char) -> Vec<i32> {
    java_split(text, separator)
        .into_iter()
        .filter_map(|piece| parse_piece(key, piece))
        .collect()
}

/// Same, as a sorted set (Java `HashSet<Integer>`; the order is not defined there).
fn int_set(key: &str, text: &str, separator: char) -> BTreeSet<i32> {
    int_list(key, text, separator).into_iter().collect()
}

/// Same, sorted like `Arrays.sort(int[])`.
fn sorted_int_list(key: &str, text: &str) -> Vec<i32> {
    let mut list = int_list(key, text, ',');
    list.sort_unstable();
    list
}

/// Java `StringUtil.isNumeric`: not empty, every char a digit. (ASCII digits only; Java's
/// `Character.isDigit` also accepts other scripts' digits.)
fn is_numeric(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|c| c.is_ascii_digit())
}

/// `split(sep)`, keep the numeric pieces, `Integer.parseInt` them (a number above `i32::MAX`
/// throws in Java and is skipped here).
fn numeric_list(text: &str, separator: char) -> Vec<i32> {
    java_split(text, separator)
        .into_iter()
        .filter(|piece| is_numeric(piece))
        .filter_map(|piece| piece.parse().ok())
        .collect()
}

/// Java `Float.parseFloat` of every comma separated piece. The default list is used when one
/// piece is bad (Java fails to load).
fn float_list(key: &str, text: &str, default: &[f32]) -> Vec<f32> {
    let parsed: Option<Vec<f32>> = java_split(text, ',')
        .into_iter()
        .map(parse_java_float::<f32>)
        .collect();
    parsed.unwrap_or_else(|| {
        warn!("{key}: cannot read '{text}' as a list of numbers; the default is used.");
        default.to_vec()
    })
}

/// `id,value;id,value` lists of `Player.ini` (`SkillDurationList`, `SkillReuseList`): `split(";")`,
/// each `split(",")` into exactly two `Integer.parseInt` values; a bad entry is logged and skipped.
fn skill_id_list(key: &str, text: &str) -> HashMap<i32, i32> {
    let mut map = HashMap::new();
    for skill in java_split(text, ';') {
        let pieces = java_split(skill, ',');
        if pieces.len() != 2 {
            warn!("[{key}]: invalid config property -> {key} \"{skill}\"");
            continue;
        }
        match (pieces[0].parse::<i32>(), pieces[1].parse::<i32>()) {
            (Ok(id), Ok(value)) => {
                map.insert(id, value);
            }
            _ => {
                if !skill.is_empty() {
                    warn!("[{key}]: invalid config property -> SkillList \"{}\"{}", pieces[0], pieces[1]);
                }
            }
        }
    }
    map
}

/// `id,value;id,value` with a `Float.parseFloat` value (`Rates.ini`
/// `DropAmountMultiplierByItemId` / `DropChanceMultiplierByItemId`).
// NOTE(java-quirk): Java skips the whole list when the first piece is empty (`"".split(";")` is
// `[""]`), so a list that starts with `;` is ignored too.
fn item_multipliers(key: &str, text: &str) -> BTreeMap<i32, f32> {
    let mut map = BTreeMap::new();
    let pieces = java_split(text, ';');
    if pieces.first().is_none_or(|first| first.is_empty()) {
        return map;
    }
    for item in pieces {
        let split = java_split(item, ',');
        if split.len() != 2 {
            warn!("Config.load(): invalid config property -> {key} \"{item}\"");
            continue;
        }
        match (split[0].parse::<i32>(), parse_java_float::<f32>(split[1])) {
            (Ok(id), Some(value)) => {
                map.insert(id, value);
            }
            _ => {
                if !item.is_empty() {
                    warn!("Config.load(): invalid config property -> {key} \"{item}\"");
                }
            }
        }
    }
    map
}

// ---- Server.ini ---------------------------------------------------------------------------

/// `ServerConfig` fields that `ServerManual` does not hold (`config/Server.ini`).
///
/// Not here, and why: `GAME_SERVER_SUBNETS` / `GAME_SERVER_HOSTS` (`ipconfig.xml`, ported in
/// `game-server/src/ls_link/ipconfig.rs`), `SERVER_ID` / `HEX_ID` (`hexid.txt`, `game-server/src/
/// hexid.rs`) and `RESERVE_HOST_ON_LOGIN` (a constant `false` that `load()` never assigns).
#[derive(Debug, Clone, PartialEq, Eq)]
// Java: ServerConfig.java:130-147, 212-241, 250-265
pub struct ServerRest {
    /// Java `DATAPACK_ROOT`: `DatapackRoot` (default `.`) with `\` replaced by `/`. Java then
    /// calls `getCanonicalFile()`; the game server resolves the text against its root folder.
    pub datapack_root: String,
    /// Java `SCRIPT_ROOT`: `ScriptRoot`, default `./data/scripts`, `\` replaced by `/`.
    pub script_root: String,
    /// Java `SERVER_LIST_TYPE`: `ServerListType`, default `Free`, as the flag word of the server
    /// list (see [`server_list_type_id`]).
    pub server_list_type: i32,
    /// Java `SERVER_RESTART_SCHEDULE`: `ServerRestartSchedule`, default `08:00`, `split(",")`.
    pub server_restart_schedule: Vec<String>,
    /// Java `SERVER_RESTART_DAYS`: `ServerRestartDays`, default empty; the numeric pieces.
    pub server_restart_days: Vec<i32>,
    /// Java `PRECAUTIONARY_RESTART_DELAY`: `PrecautionaryRestartDelay` (seconds, default 60) in
    /// milliseconds.
    pub precautionary_restart_delay: i32,
    /// Java `FILTER_LIST`: the lines of `chatfilter.txt` that are not empty and do not start with
    /// `#`, trimmed. Empty when the file cannot be read (Java logs a warning and leaves `null`).
    pub filter_list: Vec<String>,
}

/// Java `ServerConfig.getServerTypeId` on a `ServerListType` value (`split(",")` by the caller
/// or here): the flags of the known words, ignoring case and blanks. Unknown words count for
/// nothing.
// Java: ServerConfig.java:349-410
pub fn server_list_type_id(value: &str) -> i32 {
    java_split(value, ',')
        .into_iter()
        .map(|word| match java_trim(word).to_lowercase().as_str() {
            "normal" => 0x01,
            "relax" => 0x02,
            "test" => 0x04,
            "nolabel" => 0x08,
            "restricted" => 0x10,
            "event" => 0x20,
            "free" => 0x40,
            _ => 0,
        })
        .fold(0, |all, flag| all | flag)
}

impl ServerRest {
    /// Reads the keys like `ServerConfig.load()`. `config_dir` is where `chatfilter.txt` is.
    pub fn from_ini(ini: &IniFile, config_dir: &Path) -> Self {
        let root = |key: &str, default: &str| ini.get_string(key, default).replace('\\', "/");
        Self {
            datapack_root: root("DatapackRoot", "."),
            script_root: root("ScriptRoot", "./data/scripts"),
            server_list_type: server_list_type_id(&ini.get_string("ServerListType", "Free")),
            server_restart_schedule: java_split(&ini.get_string("ServerRestartSchedule", "08:00"), ',')
                .into_iter()
                .map(str::to_owned)
                .collect(),
            server_restart_days: numeric_list(java_trim(&ini.get_string("ServerRestartDays", "")), ','),
            precautionary_restart_delay: ini.get_int("PrecautionaryRestartDelay", 60).wrapping_mul(1000),
            filter_list: load_chat_filter(&config_dir.join(CHAT_FILTER_FILE)),
        }
    }
}

impl Default for ServerRest {
    /// Every key missing: the Java defaults, and no chat filter words.
    fn default() -> Self {
        Self {
            datapack_root: ".".to_owned(),
            script_root: "./data/scripts".to_owned(),
            server_list_type: 0x40,
            server_restart_schedule: vec!["08:00".to_owned()],
            server_restart_days: Vec::new(),
            precautionary_restart_delay: 60_000,
            filter_list: Vec::new(),
        }
    }
}

/// Java `ServerConfig.loadChatFilter`.
fn load_chat_filter(path: &Path) -> Vec<String> {
    match std::fs::read_to_string(path) {
        Ok(text) => {
            let words: Vec<String> = text
                .lines()
                .map(|line| java_trim(line).to_owned())
                .filter(|line| !line.is_empty() && !line.starts_with('#'))
                .collect();
            info!("Loaded {} Filter Words.", words.len());
            words
        }
        Err(error) => {
            warn!("Error while loading chat filter words! ({}: {error})", path.display());
            Vec::new()
        }
    }
}

// ---- Development.ini, Feature.ini ---------------------------------------------------------

/// `DevelopmentConfig` field the generator could not port (`config/Development.ini`).
#[derive(Debug, Clone, PartialEq, Eq)]
// Java: DevelopmentConfig.java:63-65
pub struct DevelopmentManual {
    /// Java `EXCLUDED_DEBUG_PACKETS`: `ExcludedPacketList`, default empty, `trim().split(",")`,
    /// each piece trimmed (so the default is the set with the empty name).
    pub excluded_debug_packets: BTreeSet<String>,
}

impl DevelopmentManual {
    pub fn from_ini(ini: &IniFile) -> Self {
        let text = ini.get_string("ExcludedPacketList", "");
        Self {
            excluded_debug_packets: java_split(java_trim(&text), ',')
                .into_iter()
                .map(|packet| java_trim(packet).to_owned())
                .collect(),
        }
    }
}

impl Default for DevelopmentManual {
    /// Every key missing: the default text is empty, which `split` turns into the set `{""}`.
    fn default() -> Self {
        Self::from_ini(&IniFile::empty("Development.ini"))
    }
}

/// `FeatureConfig` field the generator could not port (`config/Feature.ini`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
// Java: FeatureConfig.java:264-271
pub struct FeatureManual {
    /// Java `SIEGE_HOUR_LIST`: `SiegeHourList`, default empty; the numeric pieces, in order.
    pub siege_hour_list: Vec<i32>,
}

impl FeatureManual {
    pub fn from_ini(ini: &IniFile) -> Self {
        Self {
            siege_hour_list: numeric_list(&ini.get_string("SiegeHourList", ""), ','),
        }
    }
}

// ---- FloodProtector.ini -------------------------------------------------------------------

/// Java `FloodProtectorSettings` as `FloodProtectorConfig.loadFloodProtectorConfig` fills it.
#[derive(Debug, Clone, PartialEq, Eq)]
// Java: util/FloodProtectorSettings.java
pub struct FloodProtectorSettings {
    /// `FloodProtector<Name>Interval` (game ticks of 100 ms in Java's flood protector).
    pub protection_interval: i32,
    /// `FloodProtector<Name>LogFlooding`, default false.
    pub log_flooding: bool,
    /// `FloodProtector<Name>PunishmentLimit`, default 0.
    pub punishment_limit: i32,
    /// `FloodProtector<Name>PunishmentType`, default `none`. The raw text.
    pub punishment_type: String,
    /// `FloodProtector<Name>PunishmentTime` (minutes, default 0) times 60000.
    // NOTE(java-quirk): `getInt(...) * 60000` is an int multiplication; a huge number of minutes
    // overflows before it widens.
    pub punishment_time: i32,
}

impl FloodProtectorSettings {
    /// Java `loadFloodProtectorConfig(reader, settings, name, defaultInterval)`.
    fn from_ini(ini: &IniFile, name: &str, default_interval: i32) -> Self {
        let key = |suffix: &str| format!("FloodProtector{name}{suffix}");
        Self {
            protection_interval: ini.get_int(&key("Interval"), default_interval),
            log_flooding: ini.get_bool(&key("LogFlooding"), false),
            punishment_limit: ini.get_int(&key("PunishmentLimit"), 0),
            punishment_type: ini.get_string(&key("PunishmentType"), "none"),
            punishment_time: ini.get_int(&key("PunishmentTime"), 0).wrapping_mul(60_000),
        }
    }
}

/// `FloodProtectorConfig`: the fifteen protectors of `config/FloodProtector.ini`.
#[derive(Debug, Clone, PartialEq, Eq)]
// Java: FloodProtectorConfig.java:55-102
pub struct FloodProtectorManual {
    pub use_item: FloodProtectorSettings,
    pub roll_dice: FloodProtectorSettings,
    pub item_pet_summon: FloodProtectorSettings,
    pub hero_voice: FloodProtectorSettings,
    pub global_chat: FloodProtectorSettings,
    pub subclass: FloodProtectorSettings,
    pub drop_item: FloodProtectorSettings,
    pub server_bypass: FloodProtectorSettings,
    pub multisell: FloodProtectorSettings,
    pub transaction: FloodProtectorSettings,
    pub manufacture: FloodProtectorSettings,
    pub sendmail: FloodProtectorSettings,
    pub character_select: FloodProtectorSettings,
    pub item_auction: FloodProtectorSettings,
    pub player_action: FloodProtectorSettings,
}

impl FloodProtectorManual {
    pub fn from_ini(ini: &IniFile) -> Self {
        let read = |name: &str, default_interval: i32| FloodProtectorSettings::from_ini(ini, name, default_interval);
        Self {
            use_item: read("UseItem", 4),
            roll_dice: read("RollDice", 42),
            item_pet_summon: read("ItemPetSummon", 16),
            hero_voice: read("HeroVoice", 100),
            global_chat: read("GlobalChat", 5),
            subclass: read("Subclass", 20),
            drop_item: read("DropItem", 10),
            server_bypass: read("ServerBypass", 5),
            multisell: read("MultiSell", 1),
            transaction: read("Transaction", 10),
            manufacture: read("Manufacture", 3),
            sendmail: read("SendMail", 100),
            character_select: read("CharacterSelect", 30),
            item_auction: read("ItemAuction", 9),
            player_action: read("PlayerAction", 3),
        }
    }

    /// The protector by the name its keys use (`UseItem`, `MultiSell`, ...), like Java's
    /// `FloodProtectorSettings` instances. `None` for a name Java has no protector for.
    pub fn by_config_name(&self, name: &str) -> Option<&FloodProtectorSettings> {
        Some(match name {
            "UseItem" => &self.use_item,
            "RollDice" => &self.roll_dice,
            "ItemPetSummon" => &self.item_pet_summon,
            "HeroVoice" => &self.hero_voice,
            "GlobalChat" => &self.global_chat,
            "Subclass" => &self.subclass,
            "DropItem" => &self.drop_item,
            "ServerBypass" => &self.server_bypass,
            "MultiSell" => &self.multisell,
            "Transaction" => &self.transaction,
            "Manufacture" => &self.manufacture,
            "SendMail" => &self.sendmail,
            "CharacterSelect" => &self.character_select,
            "ItemAuction" => &self.item_auction,
            "PlayerAction" => &self.player_action,
            _ => return None,
        })
    }
}

impl Default for FloodProtectorManual {
    fn default() -> Self {
        Self::from_ini(&IniFile::empty("FloodProtector.ini"))
    }
}

// ---- General.ini --------------------------------------------------------------------------

/// `GeneralConfig` fields that the generator could not port (`config/General.ini`).
///
/// `DEFAULT_PUNISH_PARAM` and `MIN_BLOCK_CHECKER_TEAM_MEMBERS` are generated fields that `load()`
/// changes afterwards: see [`GameConfig::effective_default_punish_param`] and
/// [`GameConfig::effective_min_block_checker_team_members`].
#[derive(Debug, Clone, PartialEq, Eq)]
// Java: GeneralConfig.java:225-381
pub struct GeneralManual {
    /// Java `LOG_ITEMS_IDS_LIST`: `LogItemsIdsList`, default `0`.
    pub log_items_ids_list: BTreeSet<i32>,
    /// Java `HERB_AUTO_DESTROY_TIME`: `AutoDestroyHerbTime` (seconds, default 60) in ms.
    pub herb_auto_destroy_time: i32,
    /// Java `LIST_PROTECTED_ITEMS`: `ListOfProtectedItems`, default `0`.
    pub list_protected_items: BTreeSet<i32>,
    /// Java `CHAR_DATA_STORE_INTERVAL`: `CharacterDataStoreInterval` (minutes, default 15) in ms.
    pub char_data_store_interval: i32,
    /// Java `SAVE_DROPPED_ITEM_INTERVAL`: `SaveDroppedItemInterval` (minutes, default 60) in ms.
    pub save_dropped_item_interval: i32,
    /// Java `DEFAULT_GLOBAL_CHAT` (`ChatBroadcastType`): `GlobalChat`, default `ON`. The name.
    pub default_global_chat: String,
    /// Java `DEFAULT_TRADE_CHAT` (`ChatBroadcastType`): `TradeChat`, default `ON`. The name.
    pub default_trade_chat: String,
    /// Java `INSTANCE_FINISH_TIME`: `DefaultFinishTime` (seconds, default 300) in ms.
    pub instance_finish_time: i32,
    /// Java `EJECT_DEAD_PLAYER_TIME`: `EjectDeadPlayerTime` (seconds, default 60) in ms.
    pub eject_dead_player_time: i32,
    /// Java `BAN_CHAT_CHANNELS`: `BanChatChannels`, `;` separated `ChatType` names. Only names
    /// of Java's `ChatType` are kept (Java's `Enum.valueOf` throws for another one and the
    /// server does not start; here it is logged and skipped), without doubles, in file order.
    pub ban_chat_channels: Vec<String>,
    /// Java `ALT_ITEM_AUCTION_TIME_EXTENDS_ON_BID`: `AltItemAuctionTimeExtendsOnBid` (seconds,
    /// default 0) in ms. The multiplication is done in `int`, then widened.
    pub alt_item_auction_time_extends_on_bid: i64,
    /// Java `DEFAULT_PUNISH` (`IllegalActionPunishmentType`): `DefaultPunish`, default `KICK`.
    /// The name; `findByName` is the enum's job.
    pub default_punish: String,
    /// Java `ALT_BIRTHDAY_MAIL_TEXT`: `AltBirthdayMailText`.
    pub alt_birthday_mail_text: String,
    /// Java `BOTREPORT_RESETPOINT_HOUR`: `BotReportPointsResetHour`, default `00:00`, `split(":")`.
    pub botreport_resetpoint_hour: Vec<String>,
    /// Java `BOTREPORT_REPORT_DELAY`: `BotReportDelay` (minutes, default 30) in ms (`int`
    /// multiplication, then widened).
    pub botreport_report_delay: i64,
}

/// The names of Java's `ChatType`, in the order of the client ids (`ChatType.java`).
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

/// The client id of the `ChatType` called `name` (its index in `ChatType.java`).
pub fn chat_type_client_id(name: &str) -> Option<i32> {
    CHAT_TYPE_NAMES.iter().position(|n| *n == name).map(|i| i as i32)
}

/// The Java default of `AltBirthdayMailText` ends in `System.lineSeparator()` twice.
fn birthday_mail_default() -> String {
    let separator = if cfg!(windows) { "\r\n" } else { "\n" };
    format!(
        "Hello Adventurer!! Seeing as you're one year older now, I thought I would send you some birthday cheer :) Please find your birthday pack attached. May these gifts bring you joy and happiness on this very special day.{separator}{separator}Sincerely, Alegria"
    )
}

impl GeneralManual {
    pub fn from_ini(ini: &IniFile) -> Self {
        let mut ban_chat_channels: Vec<String> = Vec::new();
        let text = ini.get_string("BanChatChannels", "GENERAL;SHOUT;WORLD;TRADE;HERO_VOICE");
        for name in java_split(java_trim(&text), ';') {
            if chat_type_client_id(name).is_none() {
                warn!("BanChatChannels: \"{name}\" is not a chat type; skipped.");
            } else if !ban_chat_channels.iter().any(|known| known == name) {
                ban_chat_channels.push(name.to_owned());
            }
        }
        Self {
            log_items_ids_list: int_set("LogItemsIdsList", &ini.get_string("LogItemsIdsList", "0"), ','),
            herb_auto_destroy_time: ini.get_int("AutoDestroyHerbTime", 60).wrapping_mul(1000),
            list_protected_items: int_set(
                "ListOfProtectedItems",
                &ini.get_string("ListOfProtectedItems", "0"),
                ',',
            ),
            char_data_store_interval: ini
                .get_int("CharacterDataStoreInterval", 15)
                .wrapping_mul(60)
                .wrapping_mul(1000),
            save_dropped_item_interval: ini.get_int("SaveDroppedItemInterval", 60).wrapping_mul(60_000),
            default_global_chat: ini.get_string("GlobalChat", "ON"),
            default_trade_chat: ini.get_string("TradeChat", "ON"),
            instance_finish_time: ini.get_int("DefaultFinishTime", 300).wrapping_mul(1000),
            eject_dead_player_time: ini.get_int("EjectDeadPlayerTime", 60).wrapping_mul(1000),
            ban_chat_channels,
            alt_item_auction_time_extends_on_bid: i64::from(
                ini.get_int("AltItemAuctionTimeExtendsOnBid", 0).wrapping_mul(1000),
            ),
            default_punish: ini.get_string("DefaultPunish", "KICK"),
            alt_birthday_mail_text: ini.get_string("AltBirthdayMailText", &birthday_mail_default()),
            botreport_resetpoint_hour: java_split(&ini.get_string("BotReportPointsResetHour", "00:00"), ':')
                .into_iter()
                .map(str::to_owned)
                .collect(),
            botreport_report_delay: i64::from(ini.get_int("BotReportDelay", 30).wrapping_mul(60_000)),
        }
    }
}

impl Default for GeneralManual {
    fn default() -> Self {
        Self::from_ini(&IniFile::empty("General.ini"))
    }
}

// ---- GeoEngine.ini, GraciaSeeds.ini, IdManager.ini ------------------------------------------

/// `GeoEngineConfig` paths (`config/GeoEngine.ini`). Java builds each as `Paths.get(DATAPACK_ROOT.
/// getPath() + "/" + value)`; the text here is `<DatapackRoot as in Server.ini>/<value>`.
/// `game-server/src/geo.rs` resolves the geodata folder itself (it also has the per-region
/// override file); the pathnode and geo edit folders are used by nothing yet.
#[derive(Debug, Clone, PartialEq, Eq)]
// Java: GeoEngineConfig.java:54-56
pub struct GeoEngineManual {
    /// Java `GEODATA_PATH`: `GeoDataPath`, default `geodata`.
    pub geodata_path: String,
    /// Java `PATHNODE_PATH`: `PathnodePath`, default `pathnode`.
    pub pathnode_path: String,
    /// Java `GEOEDIT_PATH`: `GeoEditPath`, default `saves`.
    pub geoedit_path: String,
}

impl GeoEngineManual {
    pub fn from_ini(ini: &IniFile, datapack_root: &str) -> Self {
        let path = |key: &str, default: &str| format!("{datapack_root}/{}", ini.get_string(key, default));
        Self {
            geodata_path: path("GeoDataPath", "geodata"),
            pathnode_path: path("PathnodePath", "pathnode"),
            geoedit_path: path("GeoEditPath", "saves"),
        }
    }
}

impl Default for GeoEngineManual {
    fn default() -> Self {
        Self::from_ini(&IniFile::empty("GeoEngine.ini"), ".")
    }
}

/// `GraciaSeedsConfig` field the generator could not port (`config/GraciaSeeds.ini`).
#[derive(Debug, Clone, PartialEq, Eq)]
// Java: GraciaSeedsConfig.java:54
pub struct GraciaSeedsManual {
    /// Java `SOD_STAGE_2_LENGTH`: `Stage2Length` (minutes, default 720) in ms. Used by the Seed
    /// of Destruction manager (M8).
    pub sod_stage_2_length: i64,
}

impl GraciaSeedsManual {
    pub fn from_ini(ini: &IniFile) -> Self {
        Self {
            sod_stage_2_length: ini.get_long("Stage2Length", 720).wrapping_mul(60_000),
        }
    }
}

impl Default for GraciaSeedsManual {
    fn default() -> Self {
        Self::from_ini(&IniFile::empty("GraciaSeeds.ini"))
    }
}

/// `IdManagerConfig` field the generator could not port (`config/IdManager.ini`).
#[derive(Debug, Clone, PartialEq, Eq)]
// Java: IdManagerConfig.java:48
pub struct IdManagerManual {
    /// The `InitialCapacity` setting (default 100000) before Java caps it at `LAST_OBJECT_ID - 1`;
    /// the capped value is [`GameConfig::effective_initial_capacity`]. Java sizes its id bitset
    /// with it; the Rust id factory has no such table.
    pub initial_capacity_setting: i32,
}

impl IdManagerManual {
    pub fn from_ini(ini: &IniFile) -> Self {
        Self {
            initial_capacity_setting: ini.get_int("InitialCapacity", 100_000),
        }
    }
}

impl Default for IdManagerManual {
    fn default() -> Self {
        Self::from_ini(&IniFile::empty("IdManager.ini"))
    }
}

// ---- NPC.ini ------------------------------------------------------------------------------

/// `NpcConfig` fields that `NpcManual` does not hold (`config/NPC.ini`).
#[derive(Debug, Clone, PartialEq)]
// Java: NpcConfig.java:107-202
pub struct NpcRest {
    /// Java `NPC_DMG_PENALTY`: `DmgPenaltyForLvLDifferences`, default `0.7, 0.6, 0.6, 0.55`.
    pub npc_dmg_penalty: Vec<f32>,
    /// Java `NPC_CRIT_DMG_PENALTY`: `CritDmgPenaltyForLvLDifferences`, default
    /// `0.75, 0.65, 0.6, 0.58`.
    pub npc_crit_dmg_penalty: Vec<f32>,
    /// Java `NPC_SKILL_DMG_PENALTY`: `SkillDmgPenaltyForLvLDifferences`, default
    /// `0.8, 0.7, 0.65, 0.62`.
    pub npc_skill_dmg_penalty: Vec<f32>,
    /// Java `NPC_SKILL_CHANCE_PENALTY`: `SkillChancePenaltyForLvLDifferences`, default
    /// `2.5, 3.0, 3.25, 3.5`.
    pub npc_skill_chance_penalty: Vec<f32>,
    /// Java `LIST_PET_RENT_NPC`: `ListPetRentNpc`, default `30827`.
    pub list_pet_rent_npc: Vec<i32>,
    /// Java `RAID_PDEFENCE_MULTIPLIER`: `RaidPDefenceMultiplier` / 100 (default 100).
    pub raid_pdefence_multiplier: f64,
    /// Java `RAID_MDEFENCE_MULTIPLIER`: `RaidMDefenceMultiplier` / 100 (default 100).
    pub raid_mdefence_multiplier: f64,
    /// Java `RAID_PATTACK_MULTIPLIER`: `RaidPAttackMultiplier` / 100 (default 100).
    pub raid_pattack_multiplier: f64,
    /// Java `RAID_MATTACK_MULTIPLIER`: `RaidMAttackMultiplier` / 100 (default 100).
    pub raid_mattack_multiplier: f64,
    /// Java `PET_HP_REGEN_MULTIPLIER`: `PetHpRegenMultiplier` / 100 (default 100).
    pub pet_hp_regen_multiplier: f64,
    /// Java `PET_MP_REGEN_MULTIPLIER`: `PetMpRegenMultiplier` / 100 (default 100).
    pub pet_mp_regen_multiplier: f64,
}

impl NpcRest {
    pub fn from_ini(ini: &IniFile) -> Self {
        let list = |key: &str, default: &str| {
            let default_list = float_list(key, default, &[]);
            float_list(key, &ini.get_string(key, default), &default_list)
        };
        let percent = |key: &str| ini.get_double(key, 100.0) / 100.0;
        Self {
            npc_dmg_penalty: list("DmgPenaltyForLvLDifferences", "0.7, 0.6, 0.6, 0.55"),
            npc_crit_dmg_penalty: list("CritDmgPenaltyForLvLDifferences", "0.75, 0.65, 0.6, 0.58"),
            npc_skill_dmg_penalty: list("SkillDmgPenaltyForLvLDifferences", "0.8, 0.7, 0.65, 0.62"),
            npc_skill_chance_penalty: list("SkillChancePenaltyForLvLDifferences", "2.5, 3.0, 3.25, 3.5"),
            list_pet_rent_npc: int_list("ListPetRentNpc", &ini.get_string("ListPetRentNpc", "30827"), ','),
            raid_pdefence_multiplier: percent("RaidPDefenceMultiplier"),
            raid_mdefence_multiplier: percent("RaidMDefenceMultiplier"),
            raid_pattack_multiplier: percent("RaidPAttackMultiplier"),
            raid_mattack_multiplier: percent("RaidMAttackMultiplier"),
            pet_hp_regen_multiplier: percent("PetHpRegenMultiplier"),
            pet_mp_regen_multiplier: percent("PetMpRegenMultiplier"),
        }
    }
}

impl Default for NpcRest {
    fn default() -> Self {
        Self::from_ini(&IniFile::empty("NPC.ini"))
    }
}

// ---- Olympiad.ini -------------------------------------------------------------------------

/// `OlympiadConfig` fields that the generator could not port (`config/Olympiad.ini`). The
/// olympiad itself is M8; these are read for it.
#[derive(Debug, Clone, PartialEq, Eq)]
// Java: OlympiadConfig.java:105-148, 160-203
pub struct OlympiadManual {
    /// Java `OLYMPIAD_CLASSED_REWARD`: `OlympiadClassedReward`, default `13722,50`, as
    /// `[itemId, count]` pairs.
    pub olympiad_classed_reward: Vec<[i32; 2]>,
    /// Java `OLYMPIAD_NONCLASSED_REWARD`: `OlympiadNonClassedReward`, default `13722,40`.
    pub olympiad_nonclassed_reward: Vec<[i32; 2]>,
    /// Java `OLYMPIAD_TEAM_REWARD`: `OlympiadTeamReward`, default `13722,85`.
    pub olympiad_team_reward: Vec<[i32; 2]>,
    /// Java `LIST_OLY_RESTRICTED_ITEMS`: `OlympiadRestrictedItems`, default empty (then the set
    /// is empty).
    pub list_oly_restricted_items: BTreeSet<i32>,
    /// Java `OLYMPIAD_COMPETITION_DAYS`: `OlympiadCompetitionDays`, default `1,2,3,4,5,6,7`.
    pub olympiad_competition_days: Vec<i32>,
}

/// Java `OlympiadConfig.parseItemsList`: `split(";")`, each entry `itemId,itemNumber`.
// NOTE(java-quirk): Java sizes the result by the number of entries and leaves a skipped entry
// as `null` in it (a later `null` dereference). Here a skipped entry is simply not in the list.
fn parse_items_list(text: &str) -> Vec<[i32; 2]> {
    let mut list = Vec::new();
    for value in java_split(text, ';') {
        let pieces = java_split(value, ',');
        if pieces.len() != 2 {
            warn!(
                "parseItemsList[Config.load()]: invalid entry -> \"{}\", should be itemId,itemNumber. Skipping to the next entry in the list.",
                pieces.first().copied().unwrap_or("")
            );
            continue;
        }
        match (pieces[0].parse::<i32>(), pieces[1].parse::<i32>()) {
            (Ok(item_id), Ok(count)) => list.push([item_id, count]),
            _ => warn!(
                "parseItemsList[Config.load()]: invalid itemId or item number in \"{value}\", values must be integers. Skipping to the next entry in the list."
            ),
        }
    }
    list
}

impl OlympiadManual {
    pub fn from_ini(ini: &IniFile) -> Self {
        let restricted = ini.get_string("OlympiadRestrictedItems", "");
        let restricted = java_trim(&restricted);
        Self {
            olympiad_classed_reward: parse_items_list(&ini.get_string("OlympiadClassedReward", "13722,50")),
            olympiad_nonclassed_reward: parse_items_list(
                &ini.get_string("OlympiadNonClassedReward", "13722,40"),
            ),
            olympiad_team_reward: parse_items_list(&ini.get_string("OlympiadTeamReward", "13722,85")),
            list_oly_restricted_items: if restricted.is_empty() {
                BTreeSet::new()
            } else {
                int_set("OlympiadRestrictedItems", restricted, ',')
            },
            olympiad_competition_days: int_list(
                "OlympiadCompetitionDays",
                &ini.get_string("OlympiadCompetitionDays", "1,2,3,4,5,6,7"),
                ',',
            ),
        }
    }
}

impl Default for OlympiadManual {
    fn default() -> Self {
        Self::from_ini(&IniFile::empty("Olympiad.ini"))
    }
}

// ---- Player.ini ---------------------------------------------------------------------------

/// The Java default of `AugmentationBlackList`.
const AUGMENTATION_BLACKLIST_DEFAULT: &str = "6656,6657,6658,6659,6660,6661,6662,8191,10170,10314,13740,13741,13742,13743,13744,13745,13746,13747,13748,14592,14593,14594,14595,14596,14597,14598,14599,14600,14664,14665,14666,14667,14668,14669,14670,14671,14672,14801,14802,14803,14804,14805,14806,14807,14808,14809,15282,15283,15284,15285,15286,15287,15288,15289,15290,15291,15292,15293,15294,15295,15296,15297,15298,15299,16025,16026,21712,22173,22174,22175";

/// The Java default of `EnchantBlackList`.
const ENCHANT_BLACKLIST_DEFAULT: &str =
    "7816,7817,7818,7819,7820,7821,7822,7823,7824,7825,7826,7827,7828,7829,7830,7831,13293,13294,13296";

/// `PlayerConfig` fields that `PlayerManual` does not hold (`config/Player.ini`).
#[derive(Debug, Clone, PartialEq)]
// Java: PlayerConfig.java:258-567
pub struct PlayerRest {
    /// Java `RESPAWN_RESTORE_CP`: `RespawnRestoreCP` / 100 (default 0).
    pub respawn_restore_cp: f64,
    /// Java `RESPAWN_RESTORE_HP`: `RespawnRestoreHP` / 100 (default 65).
    pub respawn_restore_hp: f64,
    /// Java `RESPAWN_RESTORE_MP`: `RespawnRestoreMP` / 100 (default 0).
    pub respawn_restore_mp: f64,
    /// Java `SKILL_DURATION_LIST`: `SkillDurationList`, `skillId,duration;...`. Read only when
    /// `EnableModifySkillDuration` is true, empty otherwise.
    pub skill_duration_list: HashMap<i32, i32>,
    /// Java `SKILL_REUSE_LIST`: `SkillReuseList`. Read only when `EnableModifySkillReuse` is true.
    pub skill_reuse_list: HashMap<i32, i32>,
    /// Java `ALT_GAME_CANCEL_BOW`: `AltGameCancelByHit` (default `Cast`) is `bow` or `all`.
    pub alt_game_cancel_bow: bool,
    /// Java `ALT_GAME_CANCEL_CAST`: `AltGameCancelByHit` is `cast` or `all`.
    pub alt_game_cancel_cast: bool,
    /// Java `ENCHANT_BLACKLIST`: `EnchantBlackList`, sorted.
    pub enchant_blacklist: Vec<i32>,
    /// Java `OVER_ENCHANT_PUNISHMENT` (`IllegalActionPunishmentType`): `OverEnchantPunishment`,
    /// default `JAIL`. The name.
    pub over_enchant_punishment: String,
    /// Java `RETAIL_LIKE_AUGMENTATION_NG_CHANCE`: `RetailLikeAugmentationNoGradeChance`, default
    /// `55,35,7,3` (the first four numbers).
    pub retail_like_augmentation_ng_chance: [i32; 4],
    /// Java `RETAIL_LIKE_AUGMENTATION_MID_CHANCE`: `RetailLikeAugmentationMidGradeChance`.
    pub retail_like_augmentation_mid_chance: [i32; 4],
    /// Java `RETAIL_LIKE_AUGMENTATION_HIGH_CHANCE`: `RetailLikeAugmentationHighGradeChance`.
    pub retail_like_augmentation_high_chance: [i32; 4],
    /// Java `RETAIL_LIKE_AUGMENTATION_TOP_CHANCE`: `RetailLikeAugmentationTopGradeChance`.
    pub retail_like_augmentation_top_chance: [i32; 4],
    /// Java `AUGMENTATION_BLACKLIST`: `AugmentationBlackList`, sorted.
    pub augmentation_blacklist: Vec<i32>,
    /// Java `LOOT_RAIDS_PRIVILEGE_INTERVAL`: `RaidLootRightsInterval` (seconds, default 900) in ms.
    pub loot_raids_privilege_interval: i32,
    /// Java `AUTO_LOOT_ITEM_IDS`: `AutoLootItemIds`, default `0`; the ids except 0.
    pub auto_loot_item_ids: BTreeSet<i32>,
    /// Java `PARTY_XP_CUTOFF_METHOD` (`PartyExpType`): `PartyXpCutoffMethod`, default `LEVEL`.
    /// The name, as written.
    pub party_xp_cutoff_method: String,
    /// Java `PARTY_XP_CUTOFF_GAPS`: `PartyXpCutoffGaps`, default `0,9;10,14;15,99`, as
    /// `[low, high]` level pairs.
    pub party_xp_cutoff_gaps: Vec<[i32; 2]>,
    /// Java `PARTY_XP_CUTOFF_GAP_PERCENTS`: `PartyXpCutoffGapPercent`, default `100;30;0`.
    pub party_xp_cutoff_gap_percents: Vec<i32>,
    /// Java `PLAYER_MOVEMENT_BLOCK_TIME`: `NpcTalkBlockingTime` (seconds, default 0) in ms.
    pub player_movement_block_time: i32,
}

/// Java `RetailLikeAugmentation*GradeChance`: `split(",")`, `Integer.parseInt` of the first four
/// pieces. A list that is too short or has a bad number fails Java's load; the default is used
/// here.
fn retail_like_chances(key: &str, text: &str) -> [i32; 4] {
    const DEFAULT: [i32; 4] = [55, 35, 7, 3];
    let pieces = java_split(text, ',');
    let numbers: Option<Vec<i32>> = pieces.iter().take(4).map(|p| java_trim(p).parse().ok()).collect();
    match numbers.as_deref() {
        Some([a, b, c, d]) => [*a, *b, *c, *d],
        _ => {
            warn!("{key} = {text} is not a list of four numbers; using the default.");
            DEFAULT
        }
    }
}

impl PlayerRest {
    pub fn from_ini(ini: &IniFile) -> Self {
        let percent = |key: &str, default: f64| ini.get_double(key, default) / 100.0;

        let skill_duration_list = if ini.get_bool("EnableModifySkillDuration", false) {
            skill_id_list("SkillDurationList", &ini.get_string("SkillDurationList", ""))
        } else {
            HashMap::new()
        };
        let skill_reuse_list = if ini.get_bool("EnableModifySkillReuse", false) {
            skill_id_list("SkillReuseList", &ini.get_string("SkillReuseList", ""))
        } else {
            HashMap::new()
        };

        let cancel = ini.get_string("AltGameCancelByHit", "Cast");

        let grade = |key: &str| retail_like_chances(key, &ini.get_string(key, "55,35,7,3"));

        let gaps_text = ini.get_string("PartyXpCutoffGaps", "0,9;10,14;15,99");
        let party_xp_cutoff_gaps = parse_party_gaps(&gaps_text).unwrap_or_else(|| {
            warn!("PartyXpCutoffGaps = {gaps_text} is not a list of level pairs; using the default.");
            vec![[0, 9], [10, 14], [15, 99]]
        });
        let percents_text = ini.get_string("PartyXpCutoffGapPercent", "100;30;0");
        let party_xp_cutoff_gap_percents = parse_party_percents(&percents_text).unwrap_or_else(|| {
            warn!("PartyXpCutoffGapPercent = {percents_text} is not a list of numbers; using the default.");
            vec![100, 30, 0]
        });

        Self {
            respawn_restore_cp: percent("RespawnRestoreCP", 0.0),
            respawn_restore_hp: percent("RespawnRestoreHP", 65.0),
            respawn_restore_mp: percent("RespawnRestoreMP", 0.0),
            skill_duration_list,
            skill_reuse_list,
            alt_game_cancel_bow: cancel.eq_ignore_ascii_case("bow") || cancel.eq_ignore_ascii_case("all"),
            alt_game_cancel_cast: cancel.eq_ignore_ascii_case("cast") || cancel.eq_ignore_ascii_case("all"),
            enchant_blacklist: sorted_int_list(
                "EnchantBlackList",
                &ini.get_string("EnchantBlackList", ENCHANT_BLACKLIST_DEFAULT),
            ),
            over_enchant_punishment: ini.get_string("OverEnchantPunishment", "JAIL"),
            retail_like_augmentation_ng_chance: grade("RetailLikeAugmentationNoGradeChance"),
            retail_like_augmentation_mid_chance: grade("RetailLikeAugmentationMidGradeChance"),
            retail_like_augmentation_high_chance: grade("RetailLikeAugmentationHighGradeChance"),
            retail_like_augmentation_top_chance: grade("RetailLikeAugmentationTopGradeChance"),
            augmentation_blacklist: sorted_int_list(
                "AugmentationBlackList",
                &ini.get_string("AugmentationBlackList", AUGMENTATION_BLACKLIST_DEFAULT),
            ),
            loot_raids_privilege_interval: ini.get_int("RaidLootRightsInterval", 900).wrapping_mul(1000),
            auto_loot_item_ids: int_set("AutoLootItemIds", &ini.get_string("AutoLootItemIds", "0"), ',')
                .into_iter()
                .filter(|id| *id != 0)
                .collect(),
            party_xp_cutoff_method: ini.get_string("PartyXpCutoffMethod", "LEVEL"),
            party_xp_cutoff_gaps,
            party_xp_cutoff_gap_percents,
            player_movement_block_time: ini.get_int("NpcTalkBlockingTime", 0).wrapping_mul(1000),
        }
    }
}

impl Default for PlayerRest {
    fn default() -> Self {
        Self::from_ini(&IniFile::empty("Player.ini"))
    }
}

/// Java `PARTY_XP_CUTOFF_GAPS`: `"0,9;10,14;15,99"` is three level ranges. `None` where Java's
/// `Integer.parseInt` or the array index would throw.
fn parse_party_gaps(text: &str) -> Option<Vec<[i32; 2]>> {
    java_split(text, ';')
        .into_iter()
        .map(|gap| {
            let pieces = java_split(gap, ',');
            let low = java_trim(pieces.first()?).parse().ok()?;
            let high = java_trim(pieces.get(1)?).parse().ok()?;
            Some([low, high])
        })
        .collect()
}

/// Java `PARTY_XP_CUTOFF_GAP_PERCENTS`: `"100;30;0"`.
fn parse_party_percents(text: &str) -> Option<Vec<i32>> {
    java_split(text, ';')
        .into_iter()
        .map(|percent| java_trim(percent).parse().ok())
        .collect()
}

// ---- PVP.ini ------------------------------------------------------------------------------

/// `PvpConfig` fields that the generator could not port (`config/PVP.ini`).
#[derive(Debug, Clone, PartialEq, Eq)]
// Java: PvpConfig.java:59-79
pub struct PvpManual {
    /// Java `KARMA_LIST_NONDROPPABLE_PET_ITEMS`: `ListOfPetItems`, sorted.
    pub karma_list_nondroppable_pet_items: Vec<i32>,
    /// Java `KARMA_LIST_NONDROPPABLE_ITEMS`: `ListOfNonDroppableItems`, sorted.
    pub karma_list_nondroppable_items: Vec<i32>,
    /// Java `ANTIFEED_INTERVAL`: `AntiFeedInterval` (seconds, default 120) in ms.
    pub antifeed_interval: i32,
}

impl PvpManual {
    pub fn from_ini(ini: &IniFile) -> Self {
        Self {
            karma_list_nondroppable_pet_items: sorted_int_list(
                "ListOfPetItems",
                &ini.get_string(
                    "ListOfPetItems",
                    "2375,3500,3501,3502,4422,4423,4424,4425,6648,6649,6650,9882",
                ),
            ),
            karma_list_nondroppable_items: sorted_int_list(
                "ListOfNonDroppableItems",
                &ini.get_string(
                    "ListOfNonDroppableItems",
                    "57,1147,425,1146,461,10,2368,7,6,2370,2369,6842,6611,6612,6613,6614,6615,6616,6617,6618,6619,6620,6621,7694,8181,5575,7694,9388,9389,9390",
                ),
            ),
            antifeed_interval: ini.get_int("AntiFeedInterval", 120).wrapping_mul(1000),
        }
    }
}

impl Default for PvpManual {
    fn default() -> Self {
        Self::from_ini(&IniFile::empty("PVP.ini"))
    }
}

// ---- Rates.ini ----------------------------------------------------------------------------

/// A `DropHolder` of `BossDropList` (`DropType.DROP` is the only type used).
#[derive(Debug, Clone, PartialEq)]
// Java: holders/DropHolder.java
pub struct BossDrop {
    pub item_id: i32,
    pub min: i32,
    pub max: i32,
    pub chance: f64,
}

/// `RatesConfig` fields that the generator could not port (`config/Rates.ini`).
///
/// `RATE_KARMA_LOST` is a generated field that `load()` changes afterwards: see
/// [`GameConfig::effective_rate_karma_lost`].
#[derive(Debug, Clone, PartialEq, Default)]
// Java: RatesConfig.java:172-246
pub struct RatesManual {
    /// Java `RATE_DROP_AMOUNT_BY_ID`: `DropAmountMultiplierByItemId`, `itemId,multiplier;...`.
    pub rate_drop_amount_by_id: BTreeMap<i32, f32>,
    /// Java `RATE_DROP_CHANCE_BY_ID`: `DropChanceMultiplierByItemId`.
    pub rate_drop_chance_by_id: BTreeMap<i32, f32>,
    /// Java `BOSS_DROP_LIST`: `BossDropList`, `itemId,min,max,chance;...`, default empty.
    pub boss_drop_list: Vec<BossDrop>,
}

impl RatesManual {
    pub fn from_ini(ini: &IniFile) -> Self {
        Self {
            rate_drop_amount_by_id: item_multipliers(
                "DropAmountMultiplierByItemId",
                &ini.get_string("DropAmountMultiplierByItemId", ""),
            ),
            rate_drop_chance_by_id: item_multipliers(
                "DropChanceMultiplierByItemId",
                &ini.get_string("DropChanceMultiplierByItemId", ""),
            ),
            boss_drop_list: parse_boss_drop_list(&ini.get_string("BossDropList", "")),
        }
    }
}

/// `BossDropList`: `trim().split(";")`, each entry `itemId,min,max,chance` (`Integer.parseInt`
/// for the first three, `Double.parseDouble` for the chance). Java throws on an entry with fewer
/// than four values or a bad number; here the entry is logged and skipped. Entries with more
/// values use the first four, like Java.
fn parse_boss_drop_list(text: &str) -> Vec<BossDrop> {
    let mut list = Vec::new();
    for entry in java_split(java_trim(text), ';') {
        if entry.is_empty() {
            continue;
        }
        let parts = java_split(entry, ',');
        let parsed = (|| {
            Some(BossDrop {
                item_id: parts.first()?.parse().ok()?,
                min: parts.get(1)?.parse().ok()?,
                max: parts.get(2)?.parse().ok()?,
                chance: parse_java_float::<f64>(parts.get(3)?)?,
            })
        })();
        match parsed {
            Some(drop) => list.push(drop),
            None => warn!("BossDropList: \"{entry}\" is not itemId,min,max,chance; skipped."),
        }
    }
    list
}

// ---- All of them --------------------------------------------------------------------------

/// The `.ini` files behind [`CoreManual`], read once by `GameConfig::load`.
pub struct CoreInis<'a> {
    pub server: &'a IniFile,
    pub development: &'a IniFile,
    pub feature: &'a IniFile,
    pub flood_protector: &'a IniFile,
    pub general: &'a IniFile,
    pub geo_engine: &'a IniFile,
    pub gracia_seeds: &'a IniFile,
    pub id_manager: &'a IniFile,
    pub npc: &'a IniFile,
    pub olympiad: &'a IniFile,
    pub player: &'a IniFile,
    pub pvp: &'a IniFile,
    pub rates: &'a IniFile,
}

/// The hand-ported fields of the core (non-custom) config classes. `Default` is every key
/// missing (the Java defaults).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CoreManual {
    pub server: ServerRest,
    pub development: DevelopmentManual,
    pub feature: FeatureManual,
    pub flood_protector: FloodProtectorManual,
    pub general: GeneralManual,
    pub geo_engine: GeoEngineManual,
    pub gracia_seeds: GraciaSeedsManual,
    pub id_manager: IdManagerManual,
    pub npc: NpcRest,
    pub olympiad: OlympiadManual,
    pub player: PlayerRest,
    pub pvp: PvpManual,
    pub rates: RatesManual,
}

impl CoreManual {
    /// Reads every group. `config_dir` is where `chatfilter.txt` is.
    pub fn from_inis(inis: &CoreInis<'_>, config_dir: &Path) -> Self {
        let server = ServerRest::from_ini(inis.server, config_dir);
        Self {
            geo_engine: GeoEngineManual::from_ini(inis.geo_engine, &server.datapack_root),
            server,
            development: DevelopmentManual::from_ini(inis.development),
            feature: FeatureManual::from_ini(inis.feature),
            flood_protector: FloodProtectorManual::from_ini(inis.flood_protector),
            general: GeneralManual::from_ini(inis.general),
            gracia_seeds: GraciaSeedsManual::from_ini(inis.gracia_seeds),
            id_manager: IdManagerManual::from_ini(inis.id_manager),
            npc: NpcRest::from_ini(inis.npc),
            olympiad: OlympiadManual::from_ini(inis.olympiad),
            player: PlayerRest::from_ini(inis.player),
            pvp: PvpManual::from_ini(inis.pvp),
            rates: RatesManual::from_ini(inis.rates),
        }
    }
}

impl GameConfig {
    /// `MIN_BLOCK_CHECKER_TEAM_MEMBERS`: `MinBlockCheckerTeamMembers` clamped to 1..=6.
    // Java: GeneralConfig.java:367-374
    pub fn effective_min_block_checker_team_members(&self) -> i32 {
        self.general.min_block_checker_team_members.clamp(1, 6)
    }

    /// `INITIAL_CAPACITY`: `InitialCapacity` at most `LastObjectId - 1`.
    // Java: IdManagerConfig.java:48
    pub fn effective_initial_capacity(&self) -> i32 {
        self.manual
            .core
            .id_manager
            .initial_capacity_setting
            .min(self.id_manager.last_object_id.wrapping_sub(1))
    }

    /// `RATE_KARMA_LOST`: `RateKarmaLost` (default -1), and the exp rate when it is -1.
    // Java: RatesConfig.java:143-147
    pub fn effective_rate_karma_lost(&self) -> f32 {
        if self.rates.rate_karma_lost == -1.0 {
            self.rates.rate_xp
        } else {
            self.rates.rate_karma_lost
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ini(text: &str) -> IniFile {
        IniFile::parse("test.ini", text).unwrap()
    }

    fn empty() -> IniFile {
        IniFile::empty("test.ini")
    }

    #[test]
    fn server_defaults_and_values() {
        let dir = tempfile::tempdir().unwrap();
        let defaults = ServerRest::from_ini(&empty(), dir.path());
        assert_eq!(defaults, ServerRest::default());
        assert_eq!(defaults.server_list_type, 0x40);

        std::fs::write(
            dir.path().join(CHAT_FILTER_FILE),
            "# comment\n  badword  \n\nother\n#another\n",
        )
        .unwrap();
        let server = ServerRest::from_ini(
            &ini("DatapackRoot = ..\\\\data\nScriptRoot = scripts\nServerListType = Normal, Relax\n\
                  ServerRestartSchedule = 04:00,16:30\nServerRestartDays = 1,x,3,,7\n\
                  PrecautionaryRestartDelay = 5\n"),
            dir.path(),
        );
        assert_eq!(server.datapack_root, "../data");
        assert_eq!(server.script_root, "scripts");
        assert_eq!(server.server_list_type, 0x03);
        assert_eq!(server.server_restart_schedule, ["04:00", "16:30"]);
        assert_eq!(server.server_restart_days, [1, 3, 7]);
        assert_eq!(server.precautionary_restart_delay, 5000);
        assert_eq!(server.filter_list, ["badword", "other"]);
    }

    #[test]
    fn server_list_type_flags_follow_java() {
        assert_eq!(server_list_type_id("Free"), 0x40);
        assert_eq!(server_list_type_id("Normal,Relax"), 0x03);
        assert_eq!(server_list_type_id(" test , NOLABEL ,restricted,event"), 0x04 | 0x08 | 0x10 | 0x20);
        assert_eq!(server_list_type_id("bogus"), 0);
        assert_eq!(server_list_type_id(""), 0);
    }

    #[test]
    fn a_missing_chat_filter_file_is_an_empty_list() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load_chat_filter(&dir.path().join(CHAT_FILTER_FILE)).is_empty());
    }

    #[test]
    fn development_and_feature_lists() {
        assert_eq!(
            DevelopmentManual::from_ini(&empty()).excluded_debug_packets,
            BTreeSet::from([String::new()])
        );
        let dev = DevelopmentManual::from_ini(&ini("ExcludedPacketList = A , B,C\n"));
        assert_eq!(
            dev.excluded_debug_packets,
            BTreeSet::from(["A".to_owned(), "B".to_owned(), "C".to_owned()])
        );
        assert!(FeatureManual::from_ini(&empty()).siege_hour_list.is_empty());
        assert_eq!(
            FeatureManual::from_ini(&ini("SiegeHourList = 16,20,x,22\n")).siege_hour_list,
            [16, 20, 22]
        );
    }

    #[test]
    fn flood_protector_keys_and_defaults() {
        let defaults = FloodProtectorManual::from_ini(&empty());
        assert_eq!(defaults.use_item.protection_interval, 4);
        assert_eq!(defaults.roll_dice.protection_interval, 42);
        assert_eq!(defaults.hero_voice.protection_interval, 100);
        assert_eq!(defaults.player_action.protection_interval, 3);
        assert_eq!(defaults.use_item.punishment_type, "none");
        assert!(!defaults.use_item.log_flooding);
        assert_eq!(defaults, FloodProtectorManual::default());

        let flood = FloodProtectorManual::from_ini(&ini(
            "FloodProtectorMultiSellInterval = 7\nFloodProtectorMultiSellLogFlooding = true\n\
             FloodProtectorMultiSellPunishmentLimit = 9\nFloodProtectorMultiSellPunishmentType = jail\n\
             FloodProtectorMultiSellPunishmentTime = 2\n",
        ));
        let multisell = flood.by_config_name("MultiSell").unwrap();
        assert_eq!(multisell.protection_interval, 7);
        assert!(multisell.log_flooding);
        assert_eq!(multisell.punishment_limit, 9);
        assert_eq!(multisell.punishment_type, "jail");
        assert_eq!(multisell.punishment_time, 120_000);
        assert!(flood.by_config_name("Nope").is_none());
    }

    #[test]
    fn general_defaults_and_units() {
        let d = GeneralManual::from_ini(&empty());
        assert_eq!(d.log_items_ids_list, BTreeSet::from([0]));
        assert_eq!(d.herb_auto_destroy_time, 60_000);
        assert_eq!(d.list_protected_items, BTreeSet::from([0]));
        assert_eq!(d.char_data_store_interval, 900_000);
        assert_eq!(d.save_dropped_item_interval, 3_600_000);
        assert_eq!((d.default_global_chat.as_str(), d.default_trade_chat.as_str()), ("ON", "ON"));
        assert_eq!(d.instance_finish_time, 300_000);
        assert_eq!(d.eject_dead_player_time, 60_000);
        assert_eq!(d.alt_item_auction_time_extends_on_bid, 0);
        assert_eq!(d.default_punish, "KICK");
        assert!(d.alt_birthday_mail_text.starts_with("Hello Adventurer!!"));
        assert!(d.alt_birthday_mail_text.ends_with("Sincerely, Alegria"));
        assert_eq!(d.botreport_resetpoint_hour, ["00", "00"]);
        assert_eq!(d.botreport_report_delay, 1_800_000);
        // WORLD is not a ChatType of this client version: skipped like a Java exception.
        assert_eq!(d.ban_chat_channels, ["GENERAL", "SHOUT", "TRADE", "HERO_VOICE"]);

        let g = GeneralManual::from_ini(&ini(
            "LogItemsIdsList = 57,57,100\nAutoDestroyHerbTime = 5\nCharacterDataStoreInterval = 2\n\
             SaveDroppedItemInterval = 1\nGlobalChat = GM\nTradeChat = GLOBAL\nDefaultFinishTime = 10\n\
             EjectDeadPlayerTime = 3\nBanChatChannels = WHISPER;PARTY;WHISPER;NOPE\n\
             AltItemAuctionTimeExtendsOnBid = 30\nDefaultPunish = JAIL\nAltBirthdayMailText = Hi\n\
             BotReportPointsResetHour = 06:30\nBotReportDelay = 2\n",
        ));
        assert_eq!(g.log_items_ids_list, BTreeSet::from([57, 100]));
        assert_eq!(g.herb_auto_destroy_time, 5000);
        assert_eq!(g.char_data_store_interval, 120_000);
        assert_eq!(g.save_dropped_item_interval, 60_000);
        assert_eq!((g.default_global_chat.as_str(), g.default_trade_chat.as_str()), ("GM", "GLOBAL"));
        assert_eq!(g.instance_finish_time, 10_000);
        assert_eq!(g.eject_dead_player_time, 3000);
        assert_eq!(g.ban_chat_channels, ["WHISPER", "PARTY"]);
        assert_eq!(g.alt_item_auction_time_extends_on_bid, 30_000);
        assert_eq!(g.default_punish, "JAIL");
        assert_eq!(g.alt_birthday_mail_text, "Hi");
        assert_eq!(g.botreport_resetpoint_hour, ["06", "30"]);
        assert_eq!(g.botreport_report_delay, 120_000);
    }

    #[test]
    fn int_units_wrap_like_java() {
        // Java multiplies in `int`: 2_200_000 * 1000 overflows.
        let g = GeneralManual::from_ini(&ini("AutoDestroyHerbTime = 2200000\nBotReportDelay = 100000\n"));
        assert_eq!(g.herb_auto_destroy_time, 2_200_000_i32.wrapping_mul(1000));
        assert_eq!(g.botreport_report_delay, i64::from(100_000_i32.wrapping_mul(60_000)));
    }

    #[test]
    fn chat_type_ids_are_the_client_ids() {
        assert_eq!(chat_type_client_id("GENERAL"), Some(0));
        assert_eq!(chat_type_client_id("TRADE"), Some(8));
        assert_eq!(chat_type_client_id("HERO_VOICE"), Some(17));
        assert_eq!(chat_type_client_id("NPC_SHOUT"), Some(23));
        assert_eq!(chat_type_client_id("WORLD"), None);
    }

    #[test]
    fn geo_gracia_and_id_manager() {
        let geo = GeoEngineManual::from_ini(&empty(), "/srv/root");
        assert_eq!(geo.geodata_path, "/srv/root/geodata");
        assert_eq!(geo.pathnode_path, "/srv/root/pathnode");
        assert_eq!(geo.geoedit_path, "/srv/root/saves");
        let geo = GeoEngineManual::from_ini(&ini("GeoDataPath = geo2\n"), ".");
        assert_eq!(geo.geodata_path, "./geo2");

        assert_eq!(GraciaSeedsManual::default().sod_stage_2_length, 720 * 60_000);
        assert_eq!(
            GraciaSeedsManual::from_ini(&ini("Stage2Length = 10\n")).sod_stage_2_length,
            600_000
        );

        assert_eq!(IdManagerManual::default().initial_capacity_setting, 100_000);
    }

    #[test]
    fn npc_lists_and_multipliers() {
        let d = NpcRest::from_ini(&empty());
        assert_eq!(d.npc_dmg_penalty, [0.7, 0.6, 0.6, 0.55]);
        assert_eq!(d.npc_crit_dmg_penalty, [0.75, 0.65, 0.6, 0.58]);
        assert_eq!(d.npc_skill_dmg_penalty, [0.8, 0.7, 0.65, 0.62]);
        assert_eq!(d.npc_skill_chance_penalty, [2.5, 3.0, 3.25, 3.5]);
        assert_eq!(d.list_pet_rent_npc, [30827]);
        assert_eq!(d.raid_pdefence_multiplier, 1.0);
        assert_eq!(d.pet_hp_regen_multiplier, 1.0);

        let n = NpcRest::from_ini(&ini(
            "DmgPenaltyForLvLDifferences = 0.5, 0.4\nSkillDmgPenaltyForLvLDifferences = oops\n\
             ListPetRentNpc = 1,2,x,3\nRaidPDefenceMultiplier = 150\nRaidMDefenceMultiplier = 50\n\
             RaidPAttackMultiplier = 200\nRaidMAttackMultiplier = 25\nPetHpRegenMultiplier = 300\n\
             PetMpRegenMultiplier = 10\n",
        ));
        assert_eq!(n.npc_dmg_penalty, [0.5, 0.4]);
        // A bad number fails Java's load: the default list stays.
        assert_eq!(n.npc_skill_dmg_penalty, d.npc_skill_dmg_penalty);
        assert_eq!(n.list_pet_rent_npc, [1, 2, 3]);
        assert_eq!(n.raid_pdefence_multiplier, 1.5);
        assert_eq!(n.raid_mdefence_multiplier, 0.5);
        assert_eq!(n.raid_pattack_multiplier, 2.0);
        assert_eq!(n.raid_mattack_multiplier, 0.25);
        assert_eq!(n.pet_hp_regen_multiplier, 3.0);
        assert_eq!(n.pet_mp_regen_multiplier, 0.1);
    }

    #[test]
    fn olympiad_rewards_and_lists() {
        let d = OlympiadManual::from_ini(&empty());
        assert_eq!(d.olympiad_classed_reward, [[13722, 50]]);
        assert_eq!(d.olympiad_nonclassed_reward, [[13722, 40]]);
        assert_eq!(d.olympiad_team_reward, [[13722, 85]]);
        assert!(d.list_oly_restricted_items.is_empty());
        assert_eq!(d.olympiad_competition_days, [1, 2, 3, 4, 5, 6, 7]);

        let o = OlympiadManual::from_ini(&ini(
            "OlympiadClassedReward = 57,10;bad;1,x;6,7,8;9,11\nOlympiadRestrictedItems = 5, 6 ,7\n\
             OlympiadCompetitionDays = 2,4\n",
        ));
        // Entries that are not `id,count` are skipped (Java leaves nulls).
        assert_eq!(o.olympiad_classed_reward, [[57, 10], [9, 11]]);
        assert_eq!(o.list_oly_restricted_items, BTreeSet::from([5, 6, 7]));
        assert_eq!(o.olympiad_competition_days, [2, 4]);
    }

    #[test]
    fn player_defaults() {
        let d = PlayerRest::from_ini(&empty());
        assert_eq!(
            (d.respawn_restore_cp, d.respawn_restore_hp, d.respawn_restore_mp),
            (0.0, 0.65, 0.0)
        );
        assert!(d.skill_duration_list.is_empty() && d.skill_reuse_list.is_empty());
        assert!(!d.alt_game_cancel_bow && d.alt_game_cancel_cast);
        assert_eq!(d.enchant_blacklist.first(), Some(&7816));
        assert_eq!(d.enchant_blacklist.len(), 19);
        assert_eq!(d.over_enchant_punishment, "JAIL");
        assert_eq!(d.retail_like_augmentation_ng_chance, [55, 35, 7, 3]);
        assert_eq!(d.augmentation_blacklist.len(), 70);
        assert_eq!(d.augmentation_blacklist.first(), Some(&6656));
        assert_eq!(d.loot_raids_privilege_interval, 900_000);
        assert!(d.auto_loot_item_ids.is_empty());
        assert_eq!(d.party_xp_cutoff_method, "LEVEL");
        assert_eq!(d.party_xp_cutoff_gaps, [[0, 9], [10, 14], [15, 99]]);
        assert_eq!(d.party_xp_cutoff_gap_percents, [100, 30, 0]);
        assert_eq!(d.player_movement_block_time, 0);
        assert_eq!(d, PlayerRest::default());
    }

    #[test]
    fn player_values() {
        let p = PlayerRest::from_ini(&ini(
            "RespawnRestoreCP = 10\nRespawnRestoreHP = 100\nRespawnRestoreMP = 50\n\
             EnableModifySkillDuration = True\nSkillDurationList = 1,100;2,x;3;4,5,6;7,8\n\
             EnableModifySkillReuse = False\nSkillReuseList = 1,100\nAltGameCancelByHit = All\n\
             EnchantBlackList = 30,10,20\nOverEnchantPunishment = KICK\n\
             RetailLikeAugmentationMidGradeChance = 1,2,3,4,5\n\
             RetailLikeAugmentationTopGradeChance = 1,2,3\nAugmentationBlackList = 9,1\n\
             RaidLootRightsInterval = 60\nAutoLootItemIds = 57,0,5,x\nPartyXpCutoffMethod = Auto\n\
             PartyXpCutoffGaps = 1,2\nPartyXpCutoffGapPercent = 50;25\nNpcTalkBlockingTime = 4\n",
        ));
        assert_eq!(p.respawn_restore_cp, 0.1);
        assert_eq!(p.respawn_restore_hp, 1.0);
        assert_eq!(p.respawn_restore_mp, 0.5);
        assert_eq!(p.skill_duration_list, HashMap::from([(1, 100), (7, 8)]));
        // The reuse list is read only when its switch is on.
        assert!(p.skill_reuse_list.is_empty());
        assert!(p.alt_game_cancel_bow && p.alt_game_cancel_cast);
        assert_eq!(p.enchant_blacklist, [10, 20, 30]);
        assert_eq!(p.over_enchant_punishment, "KICK");
        // Only the first four numbers count; a short list falls back to the default.
        assert_eq!(p.retail_like_augmentation_mid_chance, [1, 2, 3, 4]);
        assert_eq!(p.retail_like_augmentation_top_chance, [55, 35, 7, 3]);
        assert_eq!(p.augmentation_blacklist, [1, 9]);
        assert_eq!(p.loot_raids_privilege_interval, 60_000);
        assert_eq!(p.auto_loot_item_ids, BTreeSet::from([5, 57]));
        assert_eq!(p.party_xp_cutoff_method, "Auto");
        assert_eq!(p.party_xp_cutoff_gaps, [[1, 2]]);
        assert_eq!(p.party_xp_cutoff_gap_percents, [50, 25]);
        assert_eq!(p.player_movement_block_time, 4000);
        assert!(PlayerRest::from_ini(&ini("AltGameCancelByHit = bow\n")).alt_game_cancel_bow);
        assert!(!PlayerRest::from_ini(&ini("AltGameCancelByHit = bow\n")).alt_game_cancel_cast);
    }

    #[test]
    fn party_lists_reject_what_java_rejects() {
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

    #[test]
    fn pvp_lists_and_antifeed() {
        let d = PvpManual::from_ini(&empty());
        assert_eq!(d.karma_list_nondroppable_pet_items.first(), Some(&2375));
        assert_eq!(d.karma_list_nondroppable_pet_items.len(), 12);
        assert!(d.karma_list_nondroppable_items.windows(2).all(|w| w[0] <= w[1]));
        assert_eq!(d.karma_list_nondroppable_items.len(), 30);
        assert_eq!(d.antifeed_interval, 120_000);
        let p = PvpManual::from_ini(&ini("ListOfPetItems = 3,1\nAntiFeedInterval = 30\n"));
        assert_eq!(p.karma_list_nondroppable_pet_items, [1, 3]);
        assert_eq!(p.antifeed_interval, 30_000);
    }

    #[test]
    fn rates_maps_and_boss_drops() {
        let d = RatesManual::from_ini(&empty());
        assert!(d.rate_drop_amount_by_id.is_empty());
        assert!(d.rate_drop_chance_by_id.is_empty());
        assert!(d.boss_drop_list.is_empty());
        let r = RatesManual::from_ini(&ini(
            "DropAmountMultiplierByItemId = 57,2.5;6,x;7;8,0.5\nDropChanceMultiplierByItemId = 57,3\n\
             BossDropList = 57,1,10,25.5;9,1;5,2,3,0.5\n",
        ));
        assert_eq!(r.rate_drop_amount_by_id, BTreeMap::from([(8, 0.5), (57, 2.5)]));
        assert_eq!(r.rate_drop_chance_by_id, BTreeMap::from([(57, 3.0)]));
        assert_eq!(
            r.boss_drop_list,
            [
                BossDrop {
                    item_id: 57,
                    min: 1,
                    max: 10,
                    chance: 25.5
                },
                BossDrop {
                    item_id: 5,
                    min: 2,
                    max: 3,
                    chance: 0.5
                },
            ]
        );
    }

    #[test]
    fn the_effective_values_follow_the_java_post_processing() {
        let mut config = GameConfig::default();
        config.general.min_block_checker_team_members = 0;
        assert_eq!(config.effective_min_block_checker_team_members(), 1);
        config.general.min_block_checker_team_members = 9;
        assert_eq!(config.effective_min_block_checker_team_members(), 6);
        config.general.min_block_checker_team_members = 3;
        assert_eq!(config.effective_min_block_checker_team_members(), 3);

        config.manual.core.id_manager.initial_capacity_setting = 100_000;
        config.id_manager.last_object_id = 50_000;
        assert_eq!(config.effective_initial_capacity(), 49_999);
        config.id_manager.last_object_id = 2_147_483_647;
        assert_eq!(config.effective_initial_capacity(), 100_000);

        config.rates.rate_xp = 7.0;
        config.rates.rate_karma_lost = -1.0;
        assert_eq!(config.effective_rate_karma_lost(), 7.0);
        config.rates.rate_karma_lost = 2.0;
        assert_eq!(config.effective_rate_karma_lost(), 2.0);
    }

    /// `GameConfig::load` fills `manual.core` from the files (the one place that wires them).
    #[test]
    fn game_config_load_reads_the_core_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("General.ini"), "DefaultPunish = JAIL\n").unwrap();
        std::fs::write(dir.path().join("Player.ini"), "NpcTalkBlockingTime = 2\n").unwrap();
        std::fs::write(dir.path().join("Rates.ini"), "BossDropList = 1,2,3,4\n").unwrap();
        std::fs::write(dir.path().join("Server.ini"), "ServerListType = Normal\n").unwrap();
        let config = GameConfig::load(dir.path()).unwrap();
        assert_eq!(config.manual.core.general.default_punish, "JAIL");
        assert_eq!(config.manual.core.player.player_movement_block_time, 2000);
        assert_eq!(config.manual.core.rates.boss_drop_list.len(), 1);
        assert_eq!(config.manual.core.server.server_list_type, 0x01);
        // Missing files are the Java defaults.
        assert_eq!(config.manual.core.pvp, PvpManual::default());
        assert_eq!(config.manual.core.olympiad, OlympiadManual::default());
        assert_eq!(config.manual.core.geo_engine, GeoEngineManual::default());
    }
}
