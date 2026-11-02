//! The Java defaults of every key, for the config editors (G-02, G-05).
// Java: the `config.getX("Key", default)` calls of org/l2jmobius/{gameserver,loginserver}/config
//!
//! There is no table of defaults to keep in sync: the typed structs read their keys through
//! [`IniFile`], so [`IniFile::recording`] feeds an empty file through the very `from_ini` the
//! server uses and collects the default of every key it asks for. A key the structs do not read
//! (Rust-only files, placeholders of hand-ported fields) falls back to the `Default: X` line in
//! the file's own comment, see [`default_for`].

use std::collections::BTreeMap;
use std::path::Path;

use crate::game::*;
use crate::game_manual::GameManual;
use crate::game_manual_core::{CoreInis, CoreManual};
use crate::{DatabaseConfig, IniFile, InterfaceConfig, LoginConfig, NetworkConfig, ThreadConfig};

/// Which server a config folder belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Login,
    Game,
}

/// Defaults as text, keyed by the ini key. Empty for a file with no typed struct.
pub fn java_defaults(side: Side, rel_file: &str) -> BTreeMap<String, String> {
    match side {
        Side::Login => login_defaults(rel_file),
        Side::Game => game_defaults(rel_file),
    }
}

/// The default of `key`: the typed struct first, then the `Default:` line of the comment
/// (`comment_default`, from [`crate::ini_edit::IniEntry`]).
pub fn default_for(
    defaults: &BTreeMap<String, String>,
    key: &str,
    comment_default: Option<&str>,
) -> Option<String> {
    defaults
        .get(key)
        .cloned()
        .or_else(|| comment_default.map(str::to_owned))
}

fn login_defaults(rel_file: &str) -> BTreeMap<String, String> {
    let ini = IniFile::recording(rel_file);
    // Both depend on the CPU count in Java; any value gives the same keys.
    match rel_file {
        "Server.ini" => {
            let _ = LoginConfig::from_ini(&ini);
        }
        "Database.ini" => {
            let _ = DatabaseConfig::from_ini(&ini);
        }
        "Threads.ini" => {
            let _ = ThreadConfig::from_ini(&ini, 4);
        }
        "Network.ini" => {
            let _ = NetworkConfig::from_ini(&ini, 4);
        }
        "Interface.ini" => {
            let _ = InterfaceConfig::from_ini(&ini);
        }
        _ => {}
    }
    ini.recorded_defaults()
}

macro_rules! probe_generated {
    ($file:expr, $ini:expr, $($t:ident),* $(,)?) => {{
        $( if $file == $t::FILE { let _ = $t::from_ini($ini); } )*
    }};
}

fn game_defaults(rel_file: &str) -> BTreeMap<String, String> {
    let ini = IniFile::recording(rel_file);
    probe_generated!(
        rel_file,
        &ini,
        AllowedPlayerRacesConfig,
        AutoPlayConfig,
        AutoPotionsConfig,
        BankingConfig,
        BossAnnouncementsConfig,
        CancelReturnConfig,
        CaptchaConfig,
        ChampionMonstersConfig,
        ChatModerationConfig,
        ClassBalanceConfig,
        CommunityBoardConfig,
        ConquerableHallSiegeConfig,
        CustomMailManagerConfig,
        DelevelManagerConfig,
        DevelopmentConfig,
        DualboxCheckConfig,
        FactionSystemConfig,
        FakePlayersConfig,
        FeatureConfig,
        FindPvpConfig,
        FloodProtectorConfig,
        FreeMountsConfig,
        GeneralConfig,
        GeoEngineConfig,
        GraciaSeedsConfig,
        GrandBossConfig,
        HellboundStatusConfig,
        IdManagerConfig,
        MerchantZeroSellPriceConfig,
        MultilingualSupportConfig,
        NoblessMasterConfig,
        NpcConfig,
        NpcStatMultipliersConfig,
        OfflinePlayConfig,
        OfflineTradeConfig,
        OlympiadConfig,
        OnlineInfoConfig,
        PasswordChangeConfig,
        PlayerConfig,
        PremiumSystemConfig,
        PrivateStoreRangeConfig,
        PvpAnnounceConfig,
        PvpConfig,
        PvpRewardItemConfig,
        PvpTitleColorConfig,
        RandomSpawnsConfig,
        RatesConfig,
        RebirthConfig,
        SchemeBufferConfig,
        ScreenWelcomeMessageConfig,
        SellBuffsConfig,
        ServerConfig,
        ServerTimeConfig,
        StartingLocationConfig,
        StartingTitleConfig,
        TransmogConfig,
        UndergroundColiseumConfig,
        WalkerBotProtectionConfig,
        WarehouseSortingConfig,
        WeddingConfig,
    );

    // The hand-ported fields read the same files through the "manual" structs. Each of the
    // files they use gets its own recorder; only the one asked for is returned.
    let rec = |name: &str| {
        if name == rel_file {
            ini.clone()
        } else {
            IniFile::recording(name)
        }
    };
    let (server, player, npc, faction) = (
        rec(ServerConfig::FILE),
        rec(PlayerConfig::FILE),
        rec(NpcConfig::FILE),
        rec(FactionSystemConfig::FILE),
    );
    let (development, feature, flood, general) = (
        rec(DevelopmentConfig::FILE),
        rec(FeatureConfig::FILE),
        rec(FloodProtectorConfig::FILE),
        rec(GeneralConfig::FILE),
    );
    let (geo, gracia, id_manager, olympiad, pvp, rates) = (
        rec(GeoEngineConfig::FILE),
        rec(GraciaSeedsConfig::FILE),
        rec(IdManagerConfig::FILE),
        rec(OlympiadConfig::FILE),
        rec(PvpConfig::FILE),
        rec(RatesConfig::FILE),
    );
    // `GameManual` can fail on a bad default only; ignore, the recorded keys are still there.
    let _ = GameManual::from_inis(&server, &player, &npc, &faction);
    // A folder that does not exist: the chat filter is then simply empty.
    let _ = CoreManual::from_inis(
        &CoreInis {
            server: &server,
            development: &development,
            feature: &feature,
            flood_protector: &flood,
            general: &general,
            geo_engine: &geo,
            gracia_seeds: &gracia,
            id_manager: &id_manager,
            npc: &npc,
            olympiad: &olympiad,
            player: &player,
            pvp: &pvp,
            rates: &rates,
        },
        Path::new("."),
    );

    // The recorders share one map with the clone `rec` handed out for `rel_file`.
    ini.recorded_defaults()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_defaults_come_from_the_structs() {
        let d = java_defaults(Side::Login, "Server.ini");
        assert_eq!(d["LoginserverPort"], "2106");
        assert_eq!(d["LoginPort"], "9013");
        assert_eq!(d["AcceptNewGameServer"], "true");
        assert_eq!(d["LoginserverHostname"], "0.0.0.0");
    }

    #[test]
    fn game_defaults_include_hand_ported_keys() {
        let d = java_defaults(Side::Game, "Rates.ini");
        assert_eq!(d["RateXp"], "1.0");
        let c = java_defaults(Side::Game, "Custom/Wedding.ini");
        assert!(!c.is_empty());
    }

    #[test]
    fn comment_default_is_the_fallback() {
        let d = BTreeMap::from([("A".to_owned(), "1".to_owned())]);
        assert_eq!(default_for(&d, "A", Some("9")).as_deref(), Some("1"));
        assert_eq!(default_for(&d, "B", Some("9")).as_deref(), Some("9"));
        assert_eq!(default_for(&d, "B", None), None);
    }

    #[test]
    fn unknown_file_has_no_defaults() {
        assert!(java_defaults(Side::Game, "Admin.ini").is_empty());
        assert!(java_defaults(Side::Login, "Nope.ini").is_empty());
    }

    #[test]
    fn recording_does_not_touch_real_files() {
        let ini = IniFile::recording("X.ini");
        assert_eq!(ini.get_int("A", 5), 5);
        assert!(ini.get_bool("B", true));
        assert_eq!(ini.get_string("C", "x"), "x");
        assert_eq!(ini.get_int_array("D", ",", "1,2"), [1, 2]);
        let d = ini.recorded_defaults();
        assert_eq!(d["A"], "5");
        assert_eq!(d["B"], "true");
        assert_eq!(d["C"], "x");
        assert_eq!(d["D"], "1,2");
        assert!(IniFile::empty("X").recorded_defaults().is_empty());
    }
}
