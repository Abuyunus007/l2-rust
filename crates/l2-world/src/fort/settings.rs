//! The configuration the fortresses read: the fortress values of `Feature.ini` and `Player.ini`,
//! and `FortSiege.ini` (commander and flag positions included).
// Java: config/FeatureConfig.java (FS_*)
// Java: config/PlayerConfig.java (FAME_SYSTEM_ENABLED, FAME_FOR_DEAD_PLAYERS,
//       FORTRESS_ZONE_FAME_TASK_FREQUENCY, FORTRESS_ZONE_FAME_AQUIRE_POINTS)
// Java: config/custom/OfflineTradeConfig.java (OFFLINE_FAME)
// Java: managers/FortSiegeManager.java (load)

use std::collections::BTreeMap;

use tracing::warn;

/// `FortSiege.ini` values (Java `FortSiegeManager`'s fields).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FortSiegeIni {
    /// `AttackerMaxClans`.
    pub attacker_max_clans: i32,
    /// `MaxFlags`.
    pub flag_max_count: i32,
    /// `SiegeClanMinLevel`.
    pub siege_clan_min_level: i32,
    /// `SiegeLength`, minutes.
    pub siege_length: i32,
    /// `CountDownLength`, minutes.
    pub count_down_length: i32,
    /// `SuspiciousMerchantRespawnDelay`, minutes.
    pub suspicious_merchant_respawn_delay: i32,
    /// `JustToTerritory`.
    pub just_to_territory: bool,
}

impl Default for FortSiegeIni {
    /// Java's field defaults (the same as the shipped `FortSiege.ini`).
    fn default() -> Self {
        Self {
            attacker_max_clans: 500,
            flag_max_count: 1,
            siege_clan_min_level: 4,
            siege_length: 60,
            count_down_length: 10,
            suspicious_merchant_respawn_delay: 180,
            just_to_territory: true,
        }
    }
}

/// Java `Integer.decode(text)`: an optional sign, then decimal, `0x` / `0X` / `#` hexadecimal or
/// a leading `0` for octal. `None` where Java throws `NumberFormatException`.
pub fn java_integer_decode(text: &str) -> Option<i32> {
    let (negative, rest) = match text.as_bytes().first()? {
        b'-' => (true, &text[1..]),
        b'+' => (false, &text[1..]),
        _ => (false, text),
    };
    let (radix, digits) = if let Some(hex) = rest.strip_prefix("0x").or_else(|| rest.strip_prefix("0X")) {
        (16, hex)
    } else if let Some(hex) = rest.strip_prefix('#') {
        (16, hex)
    } else if rest.len() > 1 && rest.starts_with('0') {
        (8, &rest[1..])
    } else {
        (10, rest)
    };
    // A second sign ("0x-1", "--1") is refused, as `Integer.valueOf` refuses it.
    if digits.is_empty() || digits.starts_with(['-', '+']) {
        return None;
    }
    let value = i64::from_str_radix(digits, radix).ok()?;
    let value = if negative { -value } else { value };
    i32::try_from(value).ok()
}

impl FortSiegeIni {
    /// `FortSiegeManager.load()` for the values of one `FortSiege.ini` (`Properties`: key to
    /// text). A missing key is Java's default. A text `Integer.decode` refuses makes Java's
    /// singleton fail to start; here the key keeps its default and a warning says so.
    pub fn from_properties(properties: &BTreeMap<String, String>) -> Self {
        let mut ini = Self::default();
        let int = |key: &str, default: i32| -> i32 {
            match properties.get(key) {
                None => default,
                Some(text) => java_integer_decode(text.trim()).unwrap_or_else(|| {
                    warn!("FortSiege.ini: {key} = {text} is no number; using {default}.");
                    default
                }),
            }
        };
        // `Boolean.parseBoolean(getProperty("JustToTerritory", "true"))`.
        ini.just_to_territory = properties
            .get("JustToTerritory")
            .is_none_or(|text| text.trim().eq_ignore_ascii_case("true"));
        ini.attacker_max_clans = int("AttackerMaxClans", ini.attacker_max_clans);
        ini.flag_max_count = int("MaxFlags", ini.flag_max_count);
        ini.siege_clan_min_level = int("SiegeClanMinLevel", ini.siege_clan_min_level);
        ini.siege_length = int("SiegeLength", ini.siege_length);
        ini.count_down_length = int("CountDownLength", ini.count_down_length);
        ini.suspicious_merchant_respawn_delay = int(
            "SuspiciousMerchantRespawnDelay",
            ini.suspicious_merchant_respawn_delay,
        );
        ini
    }
}

/// Java `FortSpawnHolder`: a commander's place, its NPC id and its number (`messageId`, 1 to 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FortSpawnHolder {
    pub fort_id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
    /// `getId()`: the NPC template id.
    pub npc_id: i32,
    /// `getMessageId()`: which commander of the fortress this is (the order of the keys).
    pub message_id: i32,
}

/// A combat flag's place in `FortSiege.ini` (`NameFlagN = x,y,z,flag_id`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlagSpawn {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub item_id: i32,
}

/// Java `StringTokenizer(text, ",")` then `Integer.parseInt` on each token, `count` of them. A
/// missing token or a token `parseInt` refuses is Java's exception: `None`.
fn parse_ints(text: &str, count: usize) -> Option<Vec<i32>> {
    let mut tokens = text.split(',').filter(|t| !t.is_empty());
    (0..count)
        .map(|_| tokens.next().and_then(|t| t.parse::<i32>().ok()))
        .collect()
}

/// `fort.getName().replace(" ", "")`.
fn key_name(fort_name: &str) -> String {
    fort_name.replace(' ', "")
}

/// The commanders of a fortress: keys `<Name>Commander1` to `<Name>Commander4`, in order, up to the
/// first key that is missing or empty. A line Java cannot parse is logged and skipped (the later
/// keys are still read).
pub fn commander_spawn_list(
    properties: &BTreeMap<String, String>,
    fort_id: i32,
    fort_name: &str,
) -> Vec<FortSpawnHolder> {
    let mut list = Vec::new();
    for i in 1..5 {
        let key = format!("{}Commander{i}", key_name(fort_name));
        let params = properties.get(&key).map_or("", |v| v.trim());
        if params.is_empty() {
            break;
        }
        match parse_ints(params, 5) {
            Some(v) => list.push(FortSpawnHolder {
                fort_id,
                x: v[0],
                y: v[1],
                z: v[2],
                heading: v[3],
                npc_id: v[4],
                message_id: i,
            }),
            None => warn!("Error while loading commander(s) for {fort_name} fort."),
        }
    }
    list
}

/// The combat flags of a fortress: keys `<Name>Flag1` to `<Name>Flag3`.
pub fn flag_list(properties: &BTreeMap<String, String>, fort_name: &str) -> Vec<FlagSpawn> {
    let mut list = Vec::new();
    for i in 1..4 {
        let key = format!("{}Flag{i}", key_name(fort_name));
        let params = properties.get(&key).map_or("", |v| v.trim());
        if params.is_empty() {
            break;
        }
        match parse_ints(params, 4) {
            Some(v) => list.push(FlagSpawn {
                x: v[0],
                y: v[1],
                z: v[2],
                item_id: v[3],
            }),
            None => warn!("Error while loading flag(s) for {fort_name} fort."),
        }
    }
    list
}

/// Everything the fortresses read from the configuration. `Default` is the Java default of each
/// key; the game server fills it from `Feature.ini`, `Player.ini`, `OfflineTrade.ini` and
/// `FortSiege.ini` ([`World::set_fort_settings`](crate::World::set_fort_settings)).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FortSettings {
    /// `FortressTeleportFunctionFeeRatio` (ms).
    pub fs_tele_fee_ratio: i64,
    /// `FortressTeleportFunctionFeeLvl1`.
    pub fs_tele1_fee: i32,
    /// `FortressTeleportFunctionFeeLvl2`.
    pub fs_tele2_fee: i32,
    /// `FortressSupportFunctionFeeRatio` (ms).
    pub fs_support_fee_ratio: i64,
    /// `FortressSupportFeeLvl1`.
    pub fs_support1_fee: i32,
    /// `FortressSupportFeeLvl2`.
    pub fs_support2_fee: i32,
    /// `FortressMpRegenerationFunctionFeeRatio` (ms).
    pub fs_mpreg_fee_ratio: i64,
    /// `FortressMpRegenerationFeeLvl1`.
    pub fs_mpreg1_fee: i32,
    /// `FortressMpRegenerationFeeLvl2`.
    pub fs_mpreg2_fee: i32,
    /// `FortressHpRegenerationFunctionFeeRatio` (ms).
    pub fs_hpreg_fee_ratio: i64,
    /// `FortressHpRegenerationFeeLvl1`.
    pub fs_hpreg1_fee: i32,
    /// `FortressHpRegenerationFeeLvl2`.
    pub fs_hpreg2_fee: i32,
    /// `FortressExpRegenerationFunctionFeeRatio` (ms).
    pub fs_expreg_fee_ratio: i64,
    /// `FortressExpRegenerationFeeLvl1`.
    pub fs_expreg1_fee: i32,
    /// `FortressExpRegenerationFeeLvl2`.
    pub fs_expreg2_fee: i32,
    /// `FortressPeriodicUpdateFrequency` (minutes).
    pub fs_update_frq: i32,
    /// `FortressBloodOathCount`.
    pub fs_blood_oath_count: i32,
    /// `FortressFeeForCastle`.
    pub fs_fee_for_castle: i32,
    /// `FortressMaximumOwnTime` (hours; 0 or less: no limit).
    pub fs_max_own_time: i32,
    /// `EnableFameSystem` (`Player.ini`).
    pub fame_system_enabled: bool,
    /// `FameForDeadPlayers`.
    pub fame_for_dead_players: bool,
    /// `OfflineFame` (`OfflineTrade.ini`).
    pub offline_fame: bool,
    /// `FortressZoneFameTaskFrequency` (seconds).
    pub fortress_zone_fame_task_frequency: i32,
    /// `FortressZoneFameAquirePoints`.
    pub fortress_zone_fame_aquire_points: i32,
    /// The values of `FortSiege.ini`.
    pub siege: FortSiegeIni,
    /// The raw keys of `FortSiege.ini` (the commander and flag positions are read from them when
    /// the fortresses are known).
    pub properties: BTreeMap<String, String>,
}

impl Default for FortSettings {
    fn default() -> Self {
        Self {
            fs_tele_fee_ratio: 604_800_000,
            fs_tele1_fee: 1000,
            fs_tele2_fee: 10_000,
            fs_support_fee_ratio: 86_400_000,
            fs_support1_fee: 7000,
            fs_support2_fee: 17_000,
            fs_mpreg_fee_ratio: 86_400_000,
            fs_mpreg1_fee: 6500,
            fs_mpreg2_fee: 9300,
            fs_hpreg_fee_ratio: 86_400_000,
            fs_hpreg1_fee: 2000,
            fs_hpreg2_fee: 3500,
            fs_expreg_fee_ratio: 86_400_000,
            fs_expreg1_fee: 9000,
            fs_expreg2_fee: 10_000,
            fs_update_frq: 360,
            fs_blood_oath_count: 1,
            fs_fee_for_castle: 25_000,
            fs_max_own_time: 168,
            fame_system_enabled: true,
            fame_for_dead_players: true,
            offline_fame: true,
            fortress_zone_fame_task_frequency: 300,
            fortress_zone_fame_aquire_points: 31,
            siege: FortSiegeIni::default(),
            properties: BTreeMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn props(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn integer_decode_follows_java() {
        assert_eq!(java_integer_decode("60"), Some(60));
        assert_eq!(java_integer_decode("-5"), Some(-5));
        assert_eq!(java_integer_decode("+7"), Some(7));
        assert_eq!(java_integer_decode("0x10"), Some(16));
        assert_eq!(java_integer_decode("#ff"), Some(255));
        // A leading zero is octal.
        assert_eq!(java_integer_decode("010"), Some(8));
        assert_eq!(java_integer_decode("0"), Some(0));
        assert_eq!(java_integer_decode("09"), None);
        assert_eq!(java_integer_decode(""), None);
        assert_eq!(java_integer_decode("0x"), None);
        assert_eq!(java_integer_decode("--1"), None);
        assert_eq!(java_integer_decode("2147483648"), None);
        assert_eq!(java_integer_decode("-2147483648"), Some(i32::MIN));
        assert_eq!(java_integer_decode("12a"), None);
    }

    #[test]
    fn the_shipped_defaults() {
        let ini = FortSiegeIni::from_properties(&BTreeMap::new());
        assert_eq!(ini, FortSiegeIni::default());
        assert_eq!(
            (
                ini.attacker_max_clans,
                ini.flag_max_count,
                ini.siege_clan_min_level
            ),
            (500, 1, 4)
        );
        assert_eq!(
            (
                ini.siege_length,
                ini.count_down_length,
                ini.suspicious_merchant_respawn_delay
            ),
            (60, 10, 180)
        );
        assert!(ini.just_to_territory);
    }

    #[test]
    fn values_are_read_and_a_bad_one_keeps_its_default() {
        let ini = FortSiegeIni::from_properties(&props(&[
            ("SiegeLength", "90"),
            ("CountDownLength", "x"),
            ("JustToTerritory", "False"),
            ("MaxFlags", "0x2"),
        ]));
        assert_eq!(ini.siege_length, 90);
        assert_eq!(ini.count_down_length, 10);
        assert_eq!(ini.flag_max_count, 2);
        assert!(!ini.just_to_territory);
    }

    #[test]
    fn commanders_and_flags_come_from_the_name_without_spaces() {
        let p = props(&[
            ("WhiteSandsCommander1", "117216,205648,-3048,0,35921"),
            ("WhiteSandsCommander2", "118880,203568,-3304,5396,35915"),
            ("WhiteSandsCommander4", "1,2,3,4,5"),
            ("WhiteSandsFlag1", "118640,205151,-3176,9819"),
            ("WhiteSandsFlag2", "118690,205062,-3176,9819"),
        ]);
        let commanders = commander_spawn_list(&p, 108, "White Sands");
        // The third key is missing, so the list stops there (the fourth is never read).
        assert_eq!(commanders.len(), 2);
        assert_eq!(
            commanders[1],
            FortSpawnHolder {
                fort_id: 108,
                x: 118_880,
                y: 203_568,
                z: -3304,
                heading: 5396,
                npc_id: 35_915,
                message_id: 2
            }
        );
        let flags = flag_list(&p, "White Sands");
        assert_eq!(flags.len(), 2);
        assert_eq!(flags[0].item_id, 9819);
    }

    #[test]
    fn a_broken_commander_line_is_skipped_and_the_next_key_is_read() {
        let p = props(&[
            ("ShantyCommander1", "1,2,3,4"),
            ("ShantyCommander2", "1,2,3,4,x"),
            ("ShantyCommander3", "5,6,7,8,9"),
        ]);
        let list = commander_spawn_list(&p, 101, "Shanty");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].message_id, 3);
    }
}
