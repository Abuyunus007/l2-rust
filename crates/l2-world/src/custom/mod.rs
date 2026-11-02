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

pub mod boss_announcements;
pub mod champion;

pub use boss_announcements::BossAnnouncementSettings;
pub use champion::ChampionSettings;

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
