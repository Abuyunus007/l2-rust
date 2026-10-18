//! Hand-ported game config fields (M2-CFG-01): the `()` placeholders of the generated structs.
// Java: org/l2jmobius/gameserver/config/{ServerConfig,PlayerConfig,GeneralConfig}.java and
// config/custom/FactionSystemConfig.java
//
// The generator (M2-GEN-02) cannot port these, so each class gets a `*Manual` struct that reads
// the same keys with the same defaults and split rules as Java. `GameConfig` keeps them in
// `manual`. The `effective_*` accessors on `GameConfig` are the Java values after the
// post-processing statements of `load()` (the generated fields hold the raw getter result).

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

/// `ServerConfig` fields that the generator could not port (`config/Server.ini`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerManual {
    /// Java `PROTOCOL_LIST`: `AllowedProtocolRevisions`, default `267;268;271;273`.
    ///
    /// Java `split(";")`, then `Integer.parseInt(piece.trim())`. A piece that does not parse is
    /// logged and skipped.
    pub protocol_list: Vec<i32>,
    /// Java `CHARNAME_TEMPLATE_PATTERN`: `CnameTemplate`, default `.*`.
    ///
    /// TODO(M2-CFG-01): stored as the raw pattern string. Java compiles it with
    /// `Pattern.compile` and falls back to `.*` when it is invalid. Regex matching needs an
    /// approved regex crate (none is approved yet), so the pattern is not compiled or checked.
    pub charname_template_pattern: String,
    /// Java `PET_NAME_TEMPLATE_PATTERN`: `PetNameTemplate`, default `.*`.
    ///
    /// TODO(M2-CFG-01): raw pattern string, same reason as `charname_template_pattern`.
    pub pet_name_template_pattern: String,
    /// Java `CLAN_NAME_TEMPLATE_PATTERN`: `ClanNameTemplate`, default `.*`.
    ///
    /// TODO(M2-CFG-01): raw pattern string, same reason as `charname_template_pattern`.
    pub clan_name_template_pattern: String,
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

        Self {
            protocol_list,
            // Java: ServerConfig.java:150-160 (CnameTemplate)
            charname_template_pattern: ini.get_string("CnameTemplate", ".*"),
            // Java: ServerConfig.java:162-172 (PetNameTemplate)
            pet_name_template_pattern: ini.get_string("PetNameTemplate", ".*"),
            // Java: ServerConfig.java:174-185 (ClanNameTemplate)
            clan_name_template_pattern: ini.get_string("ClanNameTemplate", ".*"),
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerManual {
    /// Java `FORBIDDEN_NAMES`: `ForbiddenNames`, default `""`.
    ///
    /// `split(",")` without trimming, so the default is `[""]` like Java.
    pub forbidden_names: Vec<String>,
    /// Java `MAX_SP`: `MaxSp`, default `50000000000`. A negative value means no limit
    /// (`Long.MAX_VALUE`).
    pub max_sp: i64,
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
        }
    }
}

impl Default for PlayerManual {
    /// Every key missing: the Java defaults.
    fn default() -> Self {
        Self::from_ini(&IniFile::empty("Player.ini"))
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
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GameManual {
    pub server: ServerManual,
    pub player: PlayerManual,
    pub faction_system: FactionSystemManual,
}

impl GameManual {
    /// Builds the manual fields from the three `.ini` files they come from.
    pub fn from_inis(
        server: &IniFile,
        player: &IniFile,
        faction_system: &IniFile,
    ) -> Result<Self, ConfigError> {
        Ok(Self {
            server: ServerManual::from_ini(server),
            player: PlayerManual::from_ini(player),
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
    }
}
