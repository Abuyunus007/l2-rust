//! Hand-ported game config fields (M2-CFG-01): the `()` placeholders of the generated structs.
// Java: org/l2jmobius/gameserver/config/{ServerConfig,PlayerConfig,GeneralConfig}.java and
// config/custom/FactionSystemConfig.java
//
// The generator (M2-GEN-02) cannot port these, so each class gets a `*Manual` struct that reads
// the same keys with the same defaults and split rules as Java. `GameConfig` keeps them in
// `manual`. The `effective_*` accessors on `GameConfig` are the Java values after the
// post-processing statements of `load()` (the generated fields hold the raw getter result).

use std::collections::HashMap;

use regex::Regex;
use tracing::warn;

use crate::{ConfigError, GameConfig, IniFile};

/// Java `String.split(String)` for a one-char literal separator (every config list uses one).
///
/// No separator in the text gives the whole text (so `""` is `[""]`). Otherwise trailing empty
/// pieces are dropped, so `"1,2,"` is `["1", "2"]` and `","` is `[]`. An empty piece in the middle
/// is kept.
fn java_split(text: &str, separator: char) -> Vec<&str> {
    if !text.contains(separator) {
        return vec![text];
    }
    let mut pieces: Vec<&str> = text.split(separator).collect();
    while pieces.last().is_some_and(|piece| piece.is_empty()) {
        pieces.pop();
    }
    pieces
}

/// Java `String.trim()`: removes every leading and trailing char `<= ' '` (not Unicode spaces).
fn java_trim(text: &str) -> &str {
    text.trim_matches(|c: char| c <= ' ')
}

/// A Java `Location` (x, y, z and heading). The heading is 0 unless the key has a fourth value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Location {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
}

/// A Java name template: `Pattern.compile(setting)`, matched with `matcher(text).matches()`.
///
/// The setting is compiled once, when the config loads. Java's `matches()` needs the whole text to
/// match, so the regex is anchored as `\A(?:...)\z`. The `regex` crate has no look-around and no
/// backreferences: a setting that uses them is invalid here, so it falls back to `.*` (wider than
/// Java, which would enforce it; the alphanumeric check on character names still applies).
// Java: ServerConfig.java:150-185 (the three patterns and their `.*` fallback)
#[derive(Debug, Clone)]
pub struct NamePattern {
    /// The setting as written (or `.*` after a fallback). Equality compares this text.
    source: String,
    /// `None` is the `.*` fallback, which needs no regex.
    regex: Option<Regex>,
}

impl NamePattern {
    /// Java `Pattern.compile(source)`. An invalid pattern is logged under `key` and replaced by
    /// `.*`, like Java's `catch (PatternSyntaxException)`.
    pub fn compile(key: &str, source: &str) -> Self {
        match Self::try_compile(source) {
            Ok(pattern) => pattern,
            Err(error) => {
                warn!(
                    "{key} = {source} cannot be used ({error}): using .* instead, as Java does for an invalid pattern."
                );
                Self {
                    source: ANY_PATTERN.to_owned(),
                    regex: None,
                }
            }
        }
    }

    fn try_compile(source: &str) -> Result<Self, regex::Error> {
        // Compiled on its own first: inside `(?:...)` an unbalanced `)` could make a different,
        // valid pattern, and Java would reject the setting.
        Regex::new(source)?;
        let anchored = Regex::new(&format!(r"\A(?:{source})\z"))?;
        Ok(Self {
            source: source.to_owned(),
            regex: Some(anchored),
        })
    }

    /// Java `CHARNAME_TEMPLATE_PATTERN.matcher(text).matches()`: the whole text has to match.
    pub fn is_match(&self, text: &str) -> bool {
        match &self.regex {
            Some(regex) => regex.is_match(text),
            // Java's `.` does not match a line terminator.
            None => !text.contains(['\n', '\r', '\u{85}', '\u{2028}', '\u{2029}']),
        }
    }
}

/// The Java default of the three name templates.
const ANY_PATTERN: &str = ".*";

impl PartialEq for NamePattern {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source
    }
}

impl Eq for NamePattern {}

/// `ServerConfig` fields that the generator could not port (`config/Server.ini`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerManual {
    /// Java `PROTOCOL_LIST`: `AllowedProtocolRevisions`, default `267;268;271;273`.
    ///
    /// Java `split(";")`, then `Integer.parseInt(piece.trim())`. A piece that does not parse is
    /// logged and skipped.
    pub protocol_list: Vec<i32>,
    /// Java `CHARNAME_TEMPLATE_PATTERN`: `CnameTemplate`, default `.*`. The raw setting.
    pub charname_template_pattern: String,
    /// The compiled `charname_template_pattern`, used by the character creation name check.
    pub charname_template: NamePattern,
    /// Java `PET_NAME_TEMPLATE_PATTERN`: `PetNameTemplate`, default `.*`. The raw setting.
    pub pet_name_template_pattern: String,
    /// The compiled `pet_name_template_pattern`. Not used yet (no pets).
    pub pet_name_template: NamePattern,
    /// Java `CLAN_NAME_TEMPLATE_PATTERN`: `ClanNameTemplate`, default `.*`. The raw setting.
    pub clan_name_template_pattern: String,
    /// The compiled `clan_name_template_pattern`. Not used yet (no clans).
    pub clan_name_template: NamePattern,
}

impl ServerManual {
    /// Reads the keys like `ServerConfig.load()`.
    pub fn from_ini(ini: &IniFile) -> Self {
        // Java: ServerConfig.java:198-209
        let protocol_list = java_split(
            &ini.get_string("AllowedProtocolRevisions", "267;268;271;273"),
            ';',
        )
        .into_iter()
        .filter_map(|protocol| match java_trim(protocol).parse::<i32>() {
            Ok(version) => Some(version),
            Err(_) => {
                warn!("Wrong config protocol version: {protocol}. Skipped.");
                None
            }
        })
        .collect();

        // Java: ServerConfig.java:150-160 (CnameTemplate)
        let charname_template_pattern = ini.get_string("CnameTemplate", ANY_PATTERN);
        // Java: ServerConfig.java:162-172 (PetNameTemplate)
        let pet_name_template_pattern = ini.get_string("PetNameTemplate", ANY_PATTERN);
        // Java: ServerConfig.java:174-185 (ClanNameTemplate)
        let clan_name_template_pattern = ini.get_string("ClanNameTemplate", ANY_PATTERN);

        Self {
            protocol_list,
            charname_template: NamePattern::compile("CnameTemplate", &charname_template_pattern),
            charname_template_pattern,
            pet_name_template: NamePattern::compile("PetNameTemplate", &pet_name_template_pattern),
            pet_name_template_pattern,
            clan_name_template: NamePattern::compile("ClanNameTemplate", &clan_name_template_pattern),
            clan_name_template_pattern,
        }
    }
}

impl Default for ServerManual {
    /// Every key missing: the Java defaults.
    fn default() -> Self {
        Self::from_ini(&IniFile::empty("Server.ini"))
    }
}

/// `PlayerConfig` fields that the generator could not port (`config/Player.ini`).
// Java: PlayerConfig.java:261-263 (the regeneration multipliers, `getDouble(key, 100) / 100`)
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerManual {
    /// Java `FORBIDDEN_NAMES`: `ForbiddenNames`, default `""`.
    ///
    /// `split(",")` without trimming, so the default is `[""]` like Java.
    pub forbidden_names: Vec<String>,
    /// Java `MAX_SP`: `MaxSp`, default `50000000000`. A negative value means no limit
    /// (`Long.MAX_VALUE`).
    pub max_sp: i64,
    /// Java `HP_REGEN_MULTIPLIER`: `HpRegenMultiplier` / 100, default 100 (so 1.0).
    pub hp_regen_multiplier: f64,
    /// Java `MP_REGEN_MULTIPLIER`: `MpRegenMultiplier` / 100, default 100 (so 1.0).
    pub mp_regen_multiplier: f64,
    /// Java `CP_REGEN_MULTIPLIER`: `CpRegenMultiplier` / 100, default 100 (so 1.0).
    pub cp_regen_multiplier: f64,
}

impl PlayerManual {
    /// Reads the keys like `PlayerConfig.load()`.
    pub fn from_ini(ini: &IniFile) -> Self {
        // Java: PlayerConfig.java:564
        let forbidden_names = java_split(&ini.get_string("ForbiddenNames", ""), ',')
            .into_iter()
            .map(str::to_owned)
            .collect();

        // Java: PlayerConfig.java:374
        let max_sp = ini.get_long("MaxSp", 50_000_000_000);
        Self {
            forbidden_names,
            max_sp: if max_sp >= 0 { max_sp } else { i64::MAX },
            hp_regen_multiplier: ini.get_double("HpRegenMultiplier", 100.0) / 100.0,
            mp_regen_multiplier: ini.get_double("MpRegenMultiplier", 100.0) / 100.0,
            cp_regen_multiplier: ini.get_double("CpRegenMultiplier", 100.0) / 100.0,
        }
    }
}

impl Default for PlayerManual {
    /// Every key missing: the Java defaults.
    fn default() -> Self {
        Self::from_ini(&IniFile::empty("Player.ini"))
    }
}

/// `NpcConfig` fields that the generator could not port (`config/NPC.ini`).
// Java: NpcConfig.java:163-164 (`getDouble(key, 100) / 100`)
#[derive(Debug, Clone, PartialEq)]
pub struct NpcManual {
    /// Java `RAID_HP_REGEN_MULTIPLIER`: `RaidHpRegenMultiplier` / 100, default 100 (so 1.0).
    pub raid_hp_regen_multiplier: f64,
    /// Java `RAID_MP_REGEN_MULTIPLIER`: `RaidMpRegenMultiplier` / 100, default 100 (so 1.0).
    pub raid_mp_regen_multiplier: f64,
    /// Java `MINIONS_RESPAWN_TIME`: `CustomMinionsRespawnTime`, `minionId,seconds;...`, default
    /// empty. Minion NPC id -> respawn time in seconds (0: never respawns).
    pub minions_respawn_time: HashMap<i32, i32>,
}

impl NpcManual {
    /// Reads the keys like `NpcConfig.load()`.
    pub fn from_ini(ini: &IniFile) -> Self {
        Self {
            raid_hp_regen_multiplier: ini.get_double("RaidHpRegenMultiplier", 100.0) / 100.0,
            raid_mp_regen_multiplier: ini.get_double("RaidMpRegenMultiplier", 100.0) / 100.0,
            minions_respawn_time: parse_minions_respawn_time(&ini.get_string("CustomMinionsRespawnTime", "")),
        }
    }
}

/// Java `NpcConfig`'s `CustomMinionsRespawnTime` loop: `split(";")`, each piece `split(",")`
/// into two `Integer.parseInt` values (not trimmed).
// NOTE(java-quirk): a piece with more than two values warns and uses the first two. A piece with
// one value warns and then throws `ArrayIndexOutOfBoundsException` in Java (the server does not
// start); here it is skipped. The empty default makes Java warn once and then swallow the
// `NumberFormatException`; here an empty piece is skipped without a warning.
fn parse_minions_respawn_time(text: &str) -> HashMap<i32, i32> {
    let mut table = HashMap::new();
    for prop in java_split(text, ';') {
        if prop.is_empty() {
            continue;
        }
        let pieces = java_split(prop, ',');
        if pieces.len() != 2 {
            warn!(
                "[CustomMinionsRespawnTime]: invalid config property -> CustomMinionsRespawnTime \"{prop}\""
            );
        }
        let (Some(id), Some(seconds)) = (pieces.first(), pieces.get(1)) else {
            continue;
        };
        match (id.parse::<i32>(), seconds.parse::<i32>()) {
            (Ok(id), Ok(seconds)) => {
                table.insert(id, seconds);
            }
            _ => warn!(
                "[CustomMinionsRespawnTime]: invalid config property -> CustomMinionsRespawnTime \"{prop}\""
            ),
        }
    }
    table
}

impl Default for NpcManual {
    /// Every key missing: the Java defaults.
    fn default() -> Self {
        Self::from_ini(&IniFile::empty("NPC.ini"))
    }
}

/// `FactionSystemConfig` location fields (`config/Custom/FactionSystem.ini`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactionSystemManual {
    /// Java `FACTION_STARTING_LOCATION`: `StartingLocation`, default `85332,16199,-1252`.
    pub faction_starting_location: Location,
    /// Java `FACTION_MANAGER_LOCATION`: `ManagerSpawnLocation`, default `85712,15974,-1260,26808`.
    pub faction_manager_location: Location,
    /// Java `FACTION_GOOD_BASE_LOCATION`: `GoodBaseLocation`, default `45306,48878,-3058`.
    pub faction_good_base_location: Location,
    /// Java `FACTION_EVIL_BASE_LOCATION`: `EvilBaseLocation`, default `-44037,-113283,-237`.
    pub faction_evil_base_location: Location,
}

impl FactionSystemManual {
    /// Reads the keys like `FactionSystemConfig.load()`.
    ///
    /// Java has no fallback here: `Integer.parseInt` or the array index throws and the server
    /// does not start. That is returned as [`ConfigError::InvalidLocation`].
    pub fn from_ini(ini: &IniFile) -> Result<Self, ConfigError> {
        Ok(Self {
            // Java: FactionSystemConfig.java:57-58 (three values, heading 0)
            faction_starting_location: parse_location(ini, "StartingLocation", "85332,16199,-1252", false)?,
            // Java: FactionSystemConfig.java:59-61 (four values, the fourth is the heading)
            faction_manager_location: parse_location(
                ini,
                "ManagerSpawnLocation",
                "85712,15974,-1260,26808",
                true,
            )?,
            // Java: FactionSystemConfig.java:62-63
            faction_good_base_location: parse_location(ini, "GoodBaseLocation", "45306,48878,-3058", false)?,
            // Java: FactionSystemConfig.java:64-65
            faction_evil_base_location: parse_location(
                ini,
                "EvilBaseLocation",
                "-44037,-113283,-237",
                false,
            )?,
        })
    }
}

impl Default for FactionSystemManual {
    /// Every key missing: the Java defaults (written out, so no parse can fail here).
    fn default() -> Self {
        Self {
            faction_starting_location: Location {
                x: 85332,
                y: 16199,
                z: -1252,
                heading: 0,
            },
            faction_manager_location: Location {
                x: 85712,
                y: 15974,
                z: -1260,
                heading: 26808,
            },
            faction_good_base_location: Location {
                x: 45306,
                y: 48878,
                z: -3058,
                heading: 0,
            },
            faction_evil_base_location: Location {
                x: -44037,
                y: -113283,
                z: -237,
                heading: 0,
            },
        }
    }
}

/// `text.split(",")` then `Integer.parseInt` of the first three pieces (and the fourth when
/// `with_heading`). Java does not trim the pieces. A missing or bad piece is an error.
fn parse_location(
    ini: &IniFile,
    key: &str,
    default: &str,
    with_heading: bool,
) -> Result<Location, ConfigError> {
    let text = ini.get_string(key, default);
    let pieces = java_split(&text, ',');
    let number = |index: usize| pieces.get(index).and_then(|piece| piece.parse::<i32>().ok());
    let invalid = || ConfigError::InvalidLocation {
        file: ini.file_name().to_owned(),
        key: key.to_owned(),
        value: text.clone(),
    };

    let (Some(x), Some(y), Some(z)) = (number(0), number(1), number(2)) else {
        return Err(invalid());
    };
    let heading = if with_heading {
        number(3).ok_or_else(invalid)?
    } else {
        0
    };
    Ok(Location { x, y, z, heading })
}

/// The hand-ported fields of every game config class that needs them.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GameManual {
    pub server: ServerManual,
    pub player: PlayerManual,
    pub npc: NpcManual,
    pub faction_system: FactionSystemManual,
}

impl GameManual {
    /// Builds the manual fields from the three `.ini` files they come from.
    pub fn from_inis(
        server: &IniFile,
        player: &IniFile,
        npc: &IniFile,
        faction_system: &IniFile,
    ) -> Result<Self, ConfigError> {
        Ok(Self {
            server: ServerManual::from_ini(server),
            player: PlayerManual::from_ini(player),
            npc: NpcManual::from_ini(npc),
            faction_system: FactionSystemManual::from_ini(faction_system)?,
        })
    }
}

impl GameConfig {
    /// `PLAYER_MAXIMUM_LEVEL`: `MaximumPlayerLevel` (default 85) plus one, like Java's `++`
    /// on a `byte` (so 127 wraps to -128).
    // Java: PlayerConfig.java:375-376
    pub fn effective_player_maximum_level(&self) -> i8 {
        self.player.player_maximum_level.wrapping_add(1)
    }

    /// `MAX_ADENA`: a negative `MaxAdena` means no limit (`Long.MAX_VALUE`).
    // Java: PlayerConfig.java:497-501
    pub fn effective_max_adena(&self) -> i64 {
        if self.player.max_adena < 0 {
            i64::MAX
        } else {
            self.player.max_adena
        }
    }

    /// `MAX_ITEM_IN_PACKET`: the largest of the three inventory limits.
    // Java: PlayerConfig.java:388
    pub fn effective_max_item_in_packet(&self) -> i32 {
        let player = &self.player;
        player
            .inventory_maximum_no_dwarf
            .max(player.inventory_maximum_dwarf.max(player.inventory_maximum_gm))
    }

    /// `KICK_MISSING_HWID`: true also when `MaxPlayersPerHWID` is above zero.
    // Java: ServerConfig.java:189-192
    pub fn effective_kick_missing_hwid(&self) -> bool {
        self.server.kick_missing_hwid || self.server.max_players_per_hwid > 0
    }

    /// `DEFAULT_PUNISH_PARAM`: a zero `DefaultPunishParam` means one hundred years in seconds.
    // Java: GeneralConfig.java:346-350
    pub fn effective_default_punish_param(&self) -> i64 {
        if self.general.default_punish_param == 0 {
            // One hundred years in seconds.
            3_155_695_200
        } else {
            self.general.default_punish_param
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    fn ini(text: &str) -> IniFile {
        IniFile::parse("test.ini", text).unwrap()
    }

    #[test]
    fn java_split_matches_string_split() {
        assert_eq!(java_split("", ','), vec![""]);
        assert_eq!(java_split(",", ','), Vec::<&str>::new());
        assert_eq!(java_split("1,2,", ','), vec!["1", "2"]);
        assert_eq!(java_split("1,,2", ','), vec!["1", "", "2"]);
        assert_eq!(java_split(",1", ','), vec!["", "1"]);
    }

    #[test]
    fn java_trim_only_drops_chars_up_to_space() {
        assert_eq!(java_trim(" 267\t\n"), "267");
        assert_eq!(java_trim("\u{a0}267"), "\u{a0}267");
    }

    #[test]
    fn protocol_list_defaults_and_skips_bad_pieces() {
        assert_eq!(ServerManual::default().protocol_list, vec![267, 268, 271, 273]);
        let server = ServerManual::from_ini(&ini("AllowedProtocolRevisions = 267; x ;273;"));
        assert_eq!(server.protocol_list, vec![267, 273]);
    }

    #[test]
    fn name_templates_default_to_any_pattern() {
        let server = ServerManual::default();
        assert_eq!(server.charname_template_pattern, ".*");
        assert_eq!(server.pet_name_template_pattern, ".*");
        assert_eq!(server.clan_name_template_pattern, ".*");
        assert!(server.charname_template.is_match("Any Name, 123"));
        assert!(!server.charname_template.is_match("two\nlines"));
    }

    #[test]
    fn name_template_matches_the_whole_name() {
        let pattern = NamePattern::compile("CnameTemplate", "[A-Za-z0-9]{1,16}");
        assert!(pattern.is_match("Hero1"));
        assert!(pattern.is_match("abcdefghijklmnop"));
        assert!(!pattern.is_match(""));
        assert!(!pattern.is_match("abcdefghijklmnopq"));
        // A partial match is not a match in Java's `matches()`.
        assert!(!pattern.is_match("Hero!"));
        assert!(!pattern.is_match("!Hero"));
        assert!(!pattern.is_match("a b"));
        // Alternation is grouped, so the anchors apply to every branch.
        let alternation = NamePattern::compile("CnameTemplate", "cat|dog");
        assert!(alternation.is_match("dog"));
        assert!(!alternation.is_match("cats"));
        assert!(!alternation.is_match("hotdog"));
    }

    #[test]
    fn name_template_keeps_java_case_and_dot_rules() {
        let pattern = NamePattern::compile("CnameTemplate", "(?i)hero.*");
        assert!(pattern.is_match("HERO of Dawn"));
        // Java's `.` does not cross a line terminator.
        assert!(!pattern.is_match("hero\nx"));
    }

    #[test]
    fn invalid_template_falls_back_to_any_pattern() {
        for bad in ["[A-Z", "a)|(b", "abc\\", "(?=a)a", "(a)\\1"] {
            let pattern = NamePattern::compile("CnameTemplate", bad);
            assert_eq!(pattern, NamePattern::compile("CnameTemplate", ".*"), "{bad}");
            assert!(pattern.is_match("anything"), "{bad}");
            assert!(!pattern.is_match("two\nlines"), "{bad}");
        }
    }

    #[test]
    fn server_ini_keys_compile_the_three_templates() {
        let text = "CnameTemplate = [A-Z][a-z]*\nPetNameTemplate = [a-z]+\nClanNameTemplate = x\n";
        let server = ServerManual::from_ini(&ini(text));
        assert!(server.charname_template.is_match("Hero"));
        assert!(!server.charname_template.is_match("hero"));
        assert!(server.pet_name_template.is_match("pet"));
        assert!(!server.pet_name_template.is_match("Pet"));
        assert!(server.clan_name_template.is_match("x"));
        assert!(!server.clan_name_template.is_match("xx"));
        assert_eq!(server.charname_template_pattern, "[A-Z][a-z]*");
    }

    #[test]
    fn forbidden_names_default_is_one_empty_name() {
        assert_eq!(PlayerManual::default().forbidden_names, vec![""]);
        let player = PlayerManual::from_ini(&ini("ForbiddenNames = admin,gm"));
        assert_eq!(player.forbidden_names, vec!["admin", "gm"]);
    }

    #[test]
    fn max_sp_negative_means_no_limit() {
        assert_eq!(PlayerManual::default().max_sp, 50_000_000_000);
        assert_eq!(PlayerManual::from_ini(&ini("MaxSp = -1")).max_sp, i64::MAX);
    }

    #[test]
    fn faction_locations_default_and_parse() {
        assert_eq!(
            FactionSystemManual::from_ini(&IniFile::empty("f.ini")).unwrap(),
            FactionSystemManual::default()
        );
        let faction = FactionSystemManual::from_ini(&ini(
            "ManagerSpawnLocation = 1,2,3,4\nEvilBaseLocation = -1,-2,-3,9",
        ))
        .unwrap();
        assert_eq!(
            faction.faction_manager_location,
            Location {
                x: 1,
                y: 2,
                z: 3,
                heading: 4
            }
        );
        assert_eq!(
            faction.faction_evil_base_location,
            Location {
                x: -1,
                y: -2,
                z: -3,
                heading: 0
            }
        );
    }

    #[test]
    fn faction_locations_reject_what_java_throws_on() {
        let short = FactionSystemManual::from_ini(&ini("ManagerSpawnLocation = 1,2,3"));
        assert!(matches!(short, Err(ConfigError::InvalidLocation { .. })));
        let text = FactionSystemManual::from_ini(&ini("StartingLocation = 1,x,3"));
        assert!(matches!(text, Err(ConfigError::InvalidLocation { .. })));
    }

    #[test]
    fn effective_values_match_java_post_processing() {
        let mut config = GameConfig::default();
        // Defaults: MaximumPlayerLevel 85 -> 86, the inventory maximum is 250, no HWID kick.
        assert_eq!(config.effective_player_maximum_level(), 86);
        assert_eq!(config.effective_max_item_in_packet(), 250);
        assert!(!config.effective_kick_missing_hwid());
        assert_eq!(config.effective_default_punish_param(), 3_155_695_200);
        assert_eq!(config.effective_max_adena(), 99_900_000_000);

        config.server.max_players_per_hwid = 2;
        assert!(config.effective_kick_missing_hwid());
        config.player.max_adena = -5;
        assert_eq!(config.effective_max_adena(), i64::MAX);
        config.player.player_maximum_level = 127;
        assert_eq!(config.effective_player_maximum_level(), -128);
    }

    /// The real `dist/game/config` values parse (skips when the Java checkout is missing).
    #[test]
    fn dist_values_parse() {
        let root = std::env::var("L2J_ROOT").unwrap_or_else(|_| r"C:\l2-rust\l2jmobius".to_string());
        let dir = std::path::Path::new(&root)
            .join("dist")
            .join("game")
            .join("config");
        if !dir.is_dir() {
            eprintln!("skipped: no Java dist config at {}", dir.display());
            return;
        }
        let config = crate::GameConfig::load(&dir).unwrap();

        // Server.ini: AllowedProtocolRevisions = 267;268;271;273, *Template = .*
        assert_eq!(config.manual.server.protocol_list, vec![267, 268, 271, 273]);
        assert_eq!(config.manual.server.charname_template_pattern, ".*");
        // Player.ini: ForbiddenNames, MaxSp = 2000000000, MaxAdena = 3000000000000000
        assert_eq!(
            config.manual.player.forbidden_names,
            vec!["annou", "ammou", "amnou", "anmou", "anou", "amou"]
        );
        assert_eq!(config.manual.player.max_sp, 2_000_000_000);
        assert_eq!(config.effective_max_adena(), 3_000_000_000_000_000);
        assert_eq!(config.effective_player_maximum_level(), 86);
        assert_eq!(config.effective_max_item_in_packet(), 250);
        assert!(!config.effective_kick_missing_hwid());
        // Custom/FactionSystem.ini: the same as the Java defaults.
        assert_eq!(config.manual.faction_system, FactionSystemManual::default());
        // General.ini: DefaultPunishParam = 0 -> the one hundred years default.
        assert_eq!(config.effective_default_punish_param(), 3_155_695_200);
        // Player.ini and NPC.ini: the regeneration multipliers, 100 -> 1.0.
        assert_eq!(config.manual.player.hp_regen_multiplier, 1.0);
        assert_eq!(config.manual.npc.raid_mp_regen_multiplier, 1.0);
    }

    #[test]
    fn regen_multipliers_are_the_java_values_over_one_hundred() {
        assert_eq!(PlayerManual::default().mp_regen_multiplier, 1.0);
        assert_eq!(NpcManual::default().raid_hp_regen_multiplier, 1.0);
        let player = PlayerManual::from_ini(&ini("HpRegenMultiplier = 150
MpRegenMultiplier = 25.5
"));
        assert_eq!(player.hp_regen_multiplier, 1.5);
        assert_eq!(player.mp_regen_multiplier, 0.255);
        assert_eq!(player.cp_regen_multiplier, 1.0);
        let npc = NpcManual::from_ini(&ini("RaidHpRegenMultiplier = 200
"));
        assert_eq!(npc.raid_hp_regen_multiplier, 2.0);
        assert_eq!(npc.raid_mp_regen_multiplier, 1.0);
    }

    #[test]
    fn custom_minions_respawn_time_is_an_id_to_seconds_table() {
        // Java default: the key is empty.
        assert!(NpcManual::default().minions_respawn_time.is_empty());
        let npc = NpcManual::from_ini(&ini("CustomMinionsRespawnTime = 22450,30;22371,120;22543,0
"));
        assert_eq!(
            npc.minions_respawn_time,
            HashMap::from([(22450, 30), (22371, 120), (22543, 0)])
        );
        // A bad piece is dropped (a one-value piece crashes Java, a word is a NumberFormatException)
        // and the others stay; a third value is ignored, the last of equal ids wins.
        let npc = NpcManual::from_ini(&ini("CustomMinionsRespawnTime = 1,2;x,3;4;5, 6;7,8,9;1,10;
"));
        assert_eq!(npc.minions_respawn_time, HashMap::from([(1, 10), (7, 8)]));
    }

    /// The shipped `NPC.ini` table (the file the Java server runs with).
    #[test]
    fn shipped_minions_respawn_table() {
        let ini = ini(
            "CustomMinionsRespawnTime = 22450,30;22371,120;22543,0;25545,0;22424,30;25608,0
",
        );
        let table = NpcManual::from_ini(&ini).minions_respawn_time;
        assert_eq!(table.len(), 6);
        assert_eq!(table.get(&25545), Some(&0));
    }
}
