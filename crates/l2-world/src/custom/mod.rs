//! Custom features of L2JMobius (`config/Custom/*.ini`), one module per feature (D-57).
//!
//! Every feature is off by default, as in Java; the settings below default to Java's *code*
//! defaults. The game server fills them from the `.ini` files (`game-server/src/custom_monsters.rs`).
//!
//! Already in their own subsystems (switched by the same keys, not repeated here):
//! - `MerchantZeroSellPrice.ini`: `shop/` (`ShopSettings::merchant_zero_sell_price`).
//! - `PrivateStoreRange.ini`: `trade/store.rs` (`StoreSettings::shop_min_range_from_*`).
//! - `WarehouseSorting.ini`: `warehouse/` (`WarehouseSettings::sorting_private` / `sorting_clan`).
//! - `RandomSpawns.ini`: `npc.rs` (`NpcSettings::random_monster_spawns`) and `script/spawn.rs`.
//! - `NpcStatMultipliers.ini`: applied while the NPC templates load (`l2_data::npc`).
//!
//! M9-C06: the screen welcome message, `.online`, `.changepassword`, the multilingual support,
//! the custom mail manager, the dual-box check, the captcha, the chat moderation commands, the
//! L2Walker protection, the Hellbound command switch and the banking commands (`misc.rs` has the
//! switches and the hooks of these).

pub mod banking;
pub mod boss_announcements;
pub mod captcha;
pub mod captcha_image;
pub mod champion;
pub mod chat_admin;
pub mod community_board;
pub mod custom_mail;
pub mod dualbox;
pub mod faction;
pub mod fake_players;
pub mod misc;
pub mod multilingual;
pub mod online_info;
pub mod password_change;
pub mod scheme_buffer;
pub mod screen_welcome;
pub mod sell_buffs;
pub mod walker_bot;

pub use boss_announcements::BossAnnouncementSettings;
pub use champion::ChampionSettings;
pub use custom_mail::CustomMailEntry;
pub use misc::{
    BankingSettings, CaptchaSettings, CustomMiscSettings, CustomMiscState, CustomMiscTimer, DualboxSettings,
    MultilangSettings, ScreenWelcomeSettings,
};

use crate::world::World;

/// The settings of the custom features this module owns.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CustomSettings {
    /// `ChampionMonsters.ini` (Java `ChampionMonstersConfig`).
    pub champion: ChampionSettings,
    /// `BossAnnouncements.ini` (Java `BossAnnouncementsConfig`).
    pub boss_announcements: BossAnnouncementSettings,
}

/// What the custom features keep in the world.
#[derive(Debug, Default)]
pub(crate) struct CustomWorld {
    pub(crate) settings: CustomSettings,
}

impl World {
    /// Installs the settings of the custom features.
    pub fn set_custom_settings(&mut self, settings: CustomSettings) {
        self.custom.settings = settings;
    }

    /// The settings of the custom features.
    pub fn custom_settings(&self) -> &CustomSettings {
        &self.custom.settings
    }
}

// Java: gameserver/config/custom/*.java, dist/game/data/scripts/custom/**
//
// # The buffer cards (M9-C03)
// - `community_board.rs`: the custom home board (`CommunityBoard.ini`).
// - `scheme_buffer.rs`: the scheme buffer NPC (`SchemeBuffer.ini`).
// - `sell_buffs.rs`: the buff shop (`SellBuffs.ini`).

/// The state of the M9-C03 features, owned by the world (D-10).
#[derive(Debug, Default)]
pub struct CustomBuffers {
    pub(crate) board: community_board::CustomBoardSettings,
    pub(crate) scheme: scheme_buffer::SchemeBufferState,
    pub(crate) sell: sell_buffs::SellBuffsState,
}
