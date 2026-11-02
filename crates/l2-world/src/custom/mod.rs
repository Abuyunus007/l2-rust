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

pub mod auto_play;
mod auto_play_ui;
pub mod auto_potion;
pub mod boss_announcements;
pub mod champion;
pub mod community_board;
pub mod events;
pub mod faction;
pub mod fake_players;
pub mod offline_play;
pub mod offline_trade;
pub mod premium;
pub mod pvp_rewards;
pub mod rebirth;
pub mod scheme_buffer;
pub mod script_features;
pub mod sell_buffs;
pub mod transmog;

pub use boss_announcements::BossAnnouncementSettings;
pub use champion::ChampionSettings;

pub use auto_play::AutoPlayRules;
pub use auto_potion::AutoPotionSettings;
pub use offline_play::OfflinePlaySettings;
pub use offline_trade::OfflineCommandSettings;
pub use premium::{PremiumRates, PremiumSettings, PremiumWrite, ShownRates};
pub use script_features::{
    CustomScriptSettings, CustomScriptWorld, DelevelManagerSettings, NoblessMasterSettings, TransmogSettings,
};

use std::collections::BTreeMap;

use l2_util::ObjectId;

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

/// The timers of the features of this card (`TimerKind::CustomPlay`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomPlayTimer {
    /// Java `PremiumManager.PremiumExpireTask`: the premium status of the player ends.
    PremiumExpire(ObjectId),
    /// Java `AutoPotionTaskManager` (1 s): one pass over the players with auto potions.
    AutoPotion,
    /// Java `AutoPlayTaskManager.AutoPlay` (700 ms): one pass over the auto players.
    AutoPlay,
    /// Java `AutoUseTaskManager.AutoUse` (300 ms): one pass over the auto players.
    AutoUse,
}

/// The order in which Java walks a `HashSet<Integer>` or the key set of a `ConcurrentHashMap`
/// with these keys, inserted in the order of `values`: by bucket (`hash ^ (hash >>> 16)` modulo
/// the table size), insertion order inside a bucket. The table starts at 16 and doubles when
/// three quarters are used. A repeated value is kept once.
// NOTE: the table never shrinks in Java, so a set that was larger once can order differently;
// the size here comes from the present number of values.
pub(crate) fn java_hash_set_order(values: &[i32]) -> Vec<i32> {
    let mut unique: Vec<i32> = Vec::with_capacity(values.len());
    for value in values {
        if !unique.contains(value) {
            unique.push(*value);
        }
    }
    let mut table: usize = 16;
    while unique.len() >= table / 4 * 3 {
        table *= 2;
    }
    let mut buckets: BTreeMap<usize, Vec<i32>> = BTreeMap::new();
    for value in &unique {
        let hash = *value as u32;
        let spread = ((hash ^ (hash >> 16)) & 0x7fff_ffff) as usize;
        buckets.entry(spread & (table - 1)).or_default().push(*value);
    }
    buckets.into_values().flatten().collect()
}

impl crate::world::World {
    /// The timers of [`CustomPlayTimer`].
    pub(crate) fn on_custom_play_timer(&mut self, timer: CustomPlayTimer) {
        match timer {
            CustomPlayTimer::PremiumExpire(id) => self.on_premium_expire_timer(id),
            CustomPlayTimer::AutoPotion => self.on_auto_potion_timer(),
            CustomPlayTimer::AutoPlay => self.on_auto_play_timer(),
            CustomPlayTimer::AutoUse => self.on_auto_use_timer(),
        }
    }

    /// `ON_PLAYER_LOGIN` of the custom features (called from `EnterWorld`).
    pub(crate) fn custom_play_on_enter(&mut self, id: ObjectId) {
        self.premium_on_enter(id);
        self.auto_play_on_enter(id);
        self.offline_play_on_enter(id);
    }

    /// `ON_PLAYER_LOGOUT` listeners that change what the final store writes (called before the
    /// snapshot is taken).
    pub(crate) fn custom_play_before_store(&mut self, id: ObjectId) {
        self.auto_play_before_store(id);
    }

    /// `deleteMe` of the custom features (called when a player leaves). `true` when the player
    /// was an offline player.
    pub(crate) fn custom_play_on_leave(&mut self, id: ObjectId) {
        self.premium_on_leave(id);
        self.auto_play_on_leave(id);
        self.auto_potion_on_leave(id);
        self.offline_play_on_leave(id);
    }

    /// The voiced commands of the custom features. `None`: no handler of the word (the feature is
    /// off, or it is not one of its words).
    pub(crate) fn custom_voiced_handler(
        &mut self,
        player: ObjectId,
        word: &str,
        params: Option<&str>,
    ) -> Option<bool> {
        if let Some(result) = self.premium_voiced_handler(player, word) {
            return Some(result);
        }
        if let Some(result) = self.auto_potion_voiced_handler(player, word) {
            return Some(result);
        }
        if let Some(result) = self.auto_play_voiced_handler(player, word, params) {
            return Some(result);
        }
        if let Some(result) = self.offline_play_voiced_handler(player, word) {
            return Some(result);
        }
        self.offline_trade_voiced_handler(player, word)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `new HashSet<>(List.of(1540, 1539, 1061, 1060))` iterates 1539, 1540, 1060, 1061: the
    /// buckets are 3, 4, 4 (1540 inserted first) and 5.
    #[test]
    fn hash_set_order_is_by_bucket() {
        assert_eq!(
            java_hash_set_order(&[1540, 1539, 1061, 1060]),
            vec![1539, 1540, 1060, 1061]
        );
        assert_eq!(java_hash_set_order(&[5592, 5591]), vec![5591, 5592]);
        assert_eq!(java_hash_set_order(&[3, 3, 1]), vec![1, 3]);
        assert!(java_hash_set_order(&[]).is_empty());
    }

    #[test]
    fn a_big_set_uses_a_bigger_table() {
        // 16 and 32 are the same bucket modulo 16 but not modulo 32 (or 64 for 12 values).
        let values: Vec<i32> = (0..12).map(|i| i * 16 + 1).collect();
        let order = java_hash_set_order(&values);
        assert_eq!(order.len(), 12);
        assert_eq!(order[0], 1);
        assert_eq!(order[1], 33);
    }
}
