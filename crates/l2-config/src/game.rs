//! The game server's configuration: every `.ini` file of `config/` and `config/Custom/`.
// Java: org/l2jmobius/gameserver/config/ConfigLoader.java (+ the 60 classes it loads)
//
// One struct per Java config class lives in `generated/game/` (produced by `codegen config`):
// every `NAME = config.getX("Key", default);` line of the Java `load()` is a typed field. Fields
// the generator cannot port are `()` placeholders, listed in `docs/analysis/config-manual-fields.md`
// and ported by hand later (M2-CFG-01).

use std::path::Path;

use crate::{ConfigError, IniFile};

pub use crate::generated::game::{
    AllowedPlayerRacesConfig, AutoPlayConfig, AutoPotionsConfig, BankingConfig, BossAnnouncementsConfig,
    CLASSES, CancelReturnConfig, CaptchaConfig, ChampionMonstersConfig, ChatModerationConfig,
    ClassBalanceConfig, CommunityBoardConfig, ConquerableHallSiegeConfig, CustomMailManagerConfig,
    DelevelManagerConfig, DevelopmentConfig, DualboxCheckConfig, FactionSystemConfig, FakePlayersConfig,
    FeatureConfig, FindPvpConfig, FloodProtectorConfig, FreeMountsConfig, GeneralConfig, GeoEngineConfig,
    GraciaSeedsConfig, GrandBossConfig, HellboundStatusConfig, IdManagerConfig, MerchantZeroSellPriceConfig,
    MultilingualSupportConfig, NoblessMasterConfig, NpcConfig, NpcStatMultipliersConfig, OfflinePlayConfig,
    OfflineTradeConfig, OlympiadConfig, OnlineInfoConfig, PasswordChangeConfig, PlayerConfig,
    PremiumSystemConfig, PrivateStoreRangeConfig, PvpAnnounceConfig, PvpConfig, PvpRewardItemConfig,
    PvpTitleColorConfig, RandomSpawnsConfig, RatesConfig, RebirthConfig, SchemeBufferConfig,
    ScreenWelcomeMessageConfig, SellBuffsConfig, ServerConfig, ServerTimeConfig, StartingLocationConfig,
    StartingTitleConfig, TransmogConfig, UndergroundColiseumConfig, WalkerBotProtectionConfig,
    WarehouseSortingConfig, WeddingConfig,
};

/// All game server config classes, loaded once at startup. Wrap it in an `Arc` to share it.
///
/// Field names are the Java class names without `Config`, in snake case (`GeneralConfig` ->
/// `general`); the classes under `config/Custom/` are in the same list, in Java's load order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GameConfig {
    pub server: ServerConfig,
    pub conquerable_hall_siege: ConquerableHallSiegeConfig,
    pub development: DevelopmentConfig,
    pub feature: FeatureConfig,
    pub flood_protector: FloodProtectorConfig,
    pub general: GeneralConfig,
    pub geo_engine: GeoEngineConfig,
    pub gracia_seeds: GraciaSeedsConfig,
    pub grand_boss: GrandBossConfig,
    pub id_manager: IdManagerConfig,
    pub npc: NpcConfig,
    pub olympiad: OlympiadConfig,
    pub player: PlayerConfig,
    pub pvp: PvpConfig,
    pub rates: RatesConfig,
    pub underground_coliseum: UndergroundColiseumConfig,
    pub allowed_player_races: AllowedPlayerRacesConfig,
    pub auto_play: AutoPlayConfig,
    pub auto_potions: AutoPotionsConfig,
    pub banking: BankingConfig,
    pub boss_announcements: BossAnnouncementsConfig,
    pub cancel_return: CancelReturnConfig,
    pub captcha: CaptchaConfig,
    pub champion_monsters: ChampionMonstersConfig,
    pub chat_moderation: ChatModerationConfig,
    pub class_balance: ClassBalanceConfig,
    pub community_board: CommunityBoardConfig,
    pub custom_mail_manager: CustomMailManagerConfig,
    pub delevel_manager: DelevelManagerConfig,
    pub dualbox_check: DualboxCheckConfig,
    pub faction_system: FactionSystemConfig,
    pub fake_players: FakePlayersConfig,
    pub find_pvp: FindPvpConfig,
    pub free_mounts: FreeMountsConfig,
    pub hellbound_status: HellboundStatusConfig,
    pub merchant_zero_sell_price: MerchantZeroSellPriceConfig,
    pub multilingual_support: MultilingualSupportConfig,
    pub nobless_master: NoblessMasterConfig,
    pub npc_stat_multipliers: NpcStatMultipliersConfig,
    pub offline_play: OfflinePlayConfig,
    pub offline_trade: OfflineTradeConfig,
    pub online_info: OnlineInfoConfig,
    pub password_change: PasswordChangeConfig,
    pub premium_system: PremiumSystemConfig,
    pub private_store_range: PrivateStoreRangeConfig,
    pub pvp_announce: PvpAnnounceConfig,
    pub pvp_reward_item: PvpRewardItemConfig,
    pub pvp_title_color: PvpTitleColorConfig,
    pub random_spawns: RandomSpawnsConfig,
    pub rebirth: RebirthConfig,
    pub scheme_buffer: SchemeBufferConfig,
    pub screen_welcome_message: ScreenWelcomeMessageConfig,
    pub sell_buffs: SellBuffsConfig,
    pub server_time: ServerTimeConfig,
    pub starting_location: StartingLocationConfig,
    pub starting_title: StartingTitleConfig,
    pub transmog: TransmogConfig,
    pub walker_bot_protection: WalkerBotProtectionConfig,
    pub warehouse_sorting: WarehouseSortingConfig,
    pub wedding: WeddingConfig,
}

impl GameConfig {
    /// Loads every config file from `config_dir` (Java: `./config`), like `ConfigLoader.init()`.
    ///
    /// A missing file gives the Java defaults for that file (with warnings), like Java. The only
    /// error is a malformed `\uXXXX` escape in one of the files.
    pub fn load(config_dir: &Path) -> Result<Self, ConfigError> {
        Ok(Self {
            server: ServerConfig::from_ini(&IniFile::load(config_dir.join(ServerConfig::FILE))?),
            conquerable_hall_siege: ConquerableHallSiegeConfig::from_ini(&IniFile::load(
                config_dir.join(ConquerableHallSiegeConfig::FILE),
            )?),
            development: DevelopmentConfig::from_ini(&IniFile::load(
                config_dir.join(DevelopmentConfig::FILE),
            )?),
            feature: FeatureConfig::from_ini(&IniFile::load(config_dir.join(FeatureConfig::FILE))?),
            flood_protector: FloodProtectorConfig::from_ini(&IniFile::load(
                config_dir.join(FloodProtectorConfig::FILE),
            )?),
            general: GeneralConfig::from_ini(&IniFile::load(config_dir.join(GeneralConfig::FILE))?),
            geo_engine: GeoEngineConfig::from_ini(&IniFile::load(config_dir.join(GeoEngineConfig::FILE))?),
            gracia_seeds: GraciaSeedsConfig::from_ini(&IniFile::load(
                config_dir.join(GraciaSeedsConfig::FILE),
            )?),
            grand_boss: GrandBossConfig::from_ini(&IniFile::load(config_dir.join(GrandBossConfig::FILE))?),
            id_manager: IdManagerConfig::from_ini(&IniFile::load(config_dir.join(IdManagerConfig::FILE))?),
            npc: NpcConfig::from_ini(&IniFile::load(config_dir.join(NpcConfig::FILE))?),
            olympiad: OlympiadConfig::from_ini(&IniFile::load(config_dir.join(OlympiadConfig::FILE))?),
            player: PlayerConfig::from_ini(&IniFile::load(config_dir.join(PlayerConfig::FILE))?),
            pvp: PvpConfig::from_ini(&IniFile::load(config_dir.join(PvpConfig::FILE))?),
            rates: RatesConfig::from_ini(&IniFile::load(config_dir.join(RatesConfig::FILE))?),
            underground_coliseum: UndergroundColiseumConfig::from_ini(&IniFile::load(
                config_dir.join(UndergroundColiseumConfig::FILE),
            )?),
            allowed_player_races: AllowedPlayerRacesConfig::from_ini(&IniFile::load(
                config_dir.join(AllowedPlayerRacesConfig::FILE),
            )?),
            auto_play: AutoPlayConfig::from_ini(&IniFile::load(config_dir.join(AutoPlayConfig::FILE))?),
            auto_potions: AutoPotionsConfig::from_ini(&IniFile::load(
                config_dir.join(AutoPotionsConfig::FILE),
            )?),
            banking: BankingConfig::from_ini(&IniFile::load(config_dir.join(BankingConfig::FILE))?),
            boss_announcements: BossAnnouncementsConfig::from_ini(&IniFile::load(
                config_dir.join(BossAnnouncementsConfig::FILE),
            )?),
            cancel_return: CancelReturnConfig::from_ini(&IniFile::load(
                config_dir.join(CancelReturnConfig::FILE),
            )?),
            captcha: CaptchaConfig::from_ini(&IniFile::load(config_dir.join(CaptchaConfig::FILE))?),
            champion_monsters: ChampionMonstersConfig::from_ini(&IniFile::load(
                config_dir.join(ChampionMonstersConfig::FILE),
            )?),
            chat_moderation: ChatModerationConfig::from_ini(&IniFile::load(
                config_dir.join(ChatModerationConfig::FILE),
            )?),
            class_balance: ClassBalanceConfig::from_ini(&IniFile::load(
                config_dir.join(ClassBalanceConfig::FILE),
            )?),
            community_board: CommunityBoardConfig::from_ini(&IniFile::load(
                config_dir.join(CommunityBoardConfig::FILE),
            )?),
            custom_mail_manager: CustomMailManagerConfig::from_ini(&IniFile::load(
                config_dir.join(CustomMailManagerConfig::FILE),
            )?),
            delevel_manager: DelevelManagerConfig::from_ini(&IniFile::load(
                config_dir.join(DelevelManagerConfig::FILE),
            )?),
            dualbox_check: DualboxCheckConfig::from_ini(&IniFile::load(
                config_dir.join(DualboxCheckConfig::FILE),
            )?),
            faction_system: FactionSystemConfig::from_ini(&IniFile::load(
                config_dir.join(FactionSystemConfig::FILE),
            )?),
            fake_players: FakePlayersConfig::from_ini(&IniFile::load(
                config_dir.join(FakePlayersConfig::FILE),
            )?),
            find_pvp: FindPvpConfig::from_ini(&IniFile::load(config_dir.join(FindPvpConfig::FILE))?),
            free_mounts: FreeMountsConfig::from_ini(&IniFile::load(config_dir.join(FreeMountsConfig::FILE))?),
            hellbound_status: HellboundStatusConfig::from_ini(&IniFile::load(
                config_dir.join(HellboundStatusConfig::FILE),
            )?),
            merchant_zero_sell_price: MerchantZeroSellPriceConfig::from_ini(&IniFile::load(
                config_dir.join(MerchantZeroSellPriceConfig::FILE),
            )?),
            multilingual_support: MultilingualSupportConfig::from_ini(&IniFile::load(
                config_dir.join(MultilingualSupportConfig::FILE),
            )?),
            nobless_master: NoblessMasterConfig::from_ini(&IniFile::load(
                config_dir.join(NoblessMasterConfig::FILE),
            )?),
            npc_stat_multipliers: NpcStatMultipliersConfig::from_ini(&IniFile::load(
                config_dir.join(NpcStatMultipliersConfig::FILE),
            )?),
            offline_play: OfflinePlayConfig::from_ini(&IniFile::load(
                config_dir.join(OfflinePlayConfig::FILE),
            )?),
            offline_trade: OfflineTradeConfig::from_ini(&IniFile::load(
                config_dir.join(OfflineTradeConfig::FILE),
            )?),
            online_info: OnlineInfoConfig::from_ini(&IniFile::load(config_dir.join(OnlineInfoConfig::FILE))?),
            password_change: PasswordChangeConfig::from_ini(&IniFile::load(
                config_dir.join(PasswordChangeConfig::FILE),
            )?),
            premium_system: PremiumSystemConfig::from_ini(&IniFile::load(
                config_dir.join(PremiumSystemConfig::FILE),
            )?),
            private_store_range: PrivateStoreRangeConfig::from_ini(&IniFile::load(
                config_dir.join(PrivateStoreRangeConfig::FILE),
            )?),
            pvp_announce: PvpAnnounceConfig::from_ini(&IniFile::load(
                config_dir.join(PvpAnnounceConfig::FILE),
            )?),
            pvp_reward_item: PvpRewardItemConfig::from_ini(&IniFile::load(
                config_dir.join(PvpRewardItemConfig::FILE),
            )?),
            pvp_title_color: PvpTitleColorConfig::from_ini(&IniFile::load(
                config_dir.join(PvpTitleColorConfig::FILE),
            )?),
            random_spawns: RandomSpawnsConfig::from_ini(&IniFile::load(
                config_dir.join(RandomSpawnsConfig::FILE),
            )?),
            rebirth: RebirthConfig::from_ini(&IniFile::load(config_dir.join(RebirthConfig::FILE))?),
            scheme_buffer: SchemeBufferConfig::from_ini(&IniFile::load(
                config_dir.join(SchemeBufferConfig::FILE),
            )?),
            screen_welcome_message: ScreenWelcomeMessageConfig::from_ini(&IniFile::load(
                config_dir.join(ScreenWelcomeMessageConfig::FILE),
            )?),
            sell_buffs: SellBuffsConfig::from_ini(&IniFile::load(config_dir.join(SellBuffsConfig::FILE))?),
            server_time: ServerTimeConfig::from_ini(&IniFile::load(config_dir.join(ServerTimeConfig::FILE))?),
            starting_location: StartingLocationConfig::from_ini(&IniFile::load(
                config_dir.join(StartingLocationConfig::FILE),
            )?),
            starting_title: StartingTitleConfig::from_ini(&IniFile::load(
                config_dir.join(StartingTitleConfig::FILE),
            )?),
            transmog: TransmogConfig::from_ini(&IniFile::load(config_dir.join(TransmogConfig::FILE))?),
            walker_bot_protection: WalkerBotProtectionConfig::from_ini(&IniFile::load(
                config_dir.join(WalkerBotProtectionConfig::FILE),
            )?),
            warehouse_sorting: WarehouseSortingConfig::from_ini(&IniFile::load(
                config_dir.join(WarehouseSortingConfig::FILE),
            )?),
            wedding: WeddingConfig::from_ini(&IniFile::load(config_dir.join(WeddingConfig::FILE))?),
        })
    }
}
