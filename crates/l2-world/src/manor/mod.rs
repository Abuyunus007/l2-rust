//! The castle manor (M7-MANOR-01): the seeds and crops of every castle, the manor periods, the
//! seed purchase and the crop sale, the `ManorManager` NPC, and the seeds and harvester a player
//! uses on monsters.
// Java: managers/CastleManorManager.java
// Java: mechanics/siege/manor/{Seed,SeedProduction,CropProcure,ManorMode}.java
// Java: network/clientpackets/{RequestManorList,RequestSetSeed,RequestSetCrop,RequestBuySeed,
//       RequestProcureCropList}.java
// Java: network/serverpackets/{BuyListSeed,ExShowManorDefaultInfo,ExShowSeedInfo,ExShowCropInfo,
//       ExShowSeedSetting,ExShowCropSetting,ExShowProcureCropDetail,ExShowSellCropList}.java
// Java: dist/game/data/scripts/ai/others/ManorManager/ManorManager.java (and its pages)
// Java: dist/game/data/scripts/handlers/items/{Seed,Harvester}.java
// Java: dist/game/data/scripts/handlers/skill/effects/{Sow,Harvesting}.java
// Java: entity/actor/Attackable.java (the seeded state: setSeeded, takeHarvest)
//
// # Shape
// - `period.rs`: the manor mode (modifiable, maintenance, approved) and the daily change on the
//   timer wheel; the change at 20:00 hands the crops to the clan warehouses, the unspent adena to
//   the treasuries and swaps the next period in (`changeMode`); `storeMe`.
// - `lists.rs`: the production (seeds the castle sells) and the procurement (crops it buys) of
//   the current and the next period, with their rows; the costs; `resetManorData`.
// - `handlers.rs`: the client packets (`RequestManorList`, `RequestSetSeed`, `RequestSetCrop`,
//   `RequestBuySeed`, `RequestProcureCropList`).
// - `packets.rs`: the packets built from the lists (the chamberlain pages of M7-NPC-01 use them).
// - `npc.rs`: the `ManorManager` script and the `manor_menu_select` bypass (`OnNpcManorBypass`) of
//   it and of the castle chamberlains.
// - `farming.rs`: the seeded state of a monster, the `Seed` and `Harvester` item handlers and the
//   `Sow` and `Harvesting` effects.
//
// # Side by side with the Java game server (D-39, D-49, D-50, D-51)
// Sieghardt (the Java GS) runs the real manor while both servers share the database: it keeps its
// own copy of both tables in memory, rewrites them whole (`storeMe`) at 20:00, every
// `AltManorSavePeriodRate` hours and at shutdown, and at the same 20:00 it hands the harvested
// crops to the clan warehouses and moves the adena of the treasuries. So here:
// - What the manor does on its own (the 20:00 change: warehouse items, treasuries; `storeMe`; the
//   reset of a castle that lost its owner) changes this world and writes its rows only with
//   `SiegeDatabaseWrites` (`World::manor_write` with [`Writes::Job`]). Without it the clan
//   warehouse is not touched at all: this server reads the clan warehouse again at every window
//   (M6-SYNC-01), so an item added in memory would only be gone at the next one.
// - What a player does (the castle lord sets the next period; with `AltManorSaveAllActions` the
//   rows of it and of every sale) is written as usual ([`Writes::Player`]), like the tax rate of a
//   chamberlain page. The treasury of a seed purchase is the castle's own row and follows the tax
//   income (Job).
// - The lists are read once at startup, like Java does; Java never reads them again either, so a
//   purchase here and one on Sieghardt do not see each other until a restart. The owner of the
//   castle (who may set the manor) is `Clan::castle_id`, which the session reads again at
//   `CharacterSelect` (D-51).
//
// # Quirks kept (marked where they are)
// - `CastleManorManager` sets the mode at start with `min >= maintenanceMin` for every hour after
//   20:00, so from 21:00 to 21:05 the manor is APPROVED instead of MODIFIABLE.
// - The slot check of the 20:00 change counts `getAllItemsByItemId(..) == null`, which is never
//   true, so it needs 0 slots; and clears the next period only when the clan warehouse is full
//   *and* the treasury is short (`&&`).
// - `Sow.calcSuccess` computes `Math.max(basicSuccess, 1)` and throws the result away.
// - The next-period lists share their `SeedProduction` objects with the current ones after the
//   change at 20:00 in Java, so a sale also lowers the "left" of the next period there (display
//   only, it is reset at the next change); here they are separate values.

mod farming;
mod handlers;
mod lists;
mod npc;
mod packets;
mod period;

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use l2_data::Seeds;
use l2_util::ObjectId;

use crate::rewards::drop::ItemHolder;
use crate::timer::TimerId;

pub use handlers::ManorInput;
pub use period::{manor_initial_mode, manor_next_mode_change_ms};

/// Java `ManorMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ManorMode {
    /// `AllowManor = False`.
    Disabled,
    /// The lords set the next period (20:06 to 4:30).
    Modifiable,
    /// 20:00 to 20:06: the change of the period.
    Maintenance,
    /// 4:30 to 20:00: the approved period is on, nothing can be set.
    #[default]
    Approved,
}

impl ManorMode {
    /// Java `ManorMode.toString()` (`getCurrentModeName`).
    pub fn name(self) -> &'static str {
        match self {
            Self::Disabled => "DISABLED",
            Self::Modifiable => "MODIFIABLE",
            Self::Maintenance => "MAINTENANCE",
            Self::Approved => "APPROVED",
        }
    }
}

/// Java `SeedProduction`: a seed a castle sells in a period.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeedProduction {
    /// The seed item id.
    pub id: i32,
    /// `getAmount()`: how many are left to buy.
    pub amount: i64,
    pub price: i64,
    /// `getStartAmount()`.
    pub start_amount: i64,
}

impl SeedProduction {
    /// Java `decreaseAmount(value)`: `false` (and nothing changes) when there are fewer left.
    pub fn decrease_amount(&mut self, value: i64) -> bool {
        let next = self.amount.wrapping_sub(value);
        if next < 0 {
            return false;
        }
        self.amount = next;
        true
    }
}

/// Java `CropProcure`: a crop a castle buys in a period.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CropProcure {
    /// The crop item id.
    pub id: i32,
    /// `getAmount()`: how many the castle still wants.
    pub amount: i64,
    /// `getReward()`: the reward type (1 or 2) of the seed.
    pub reward: i32,
    pub start_amount: i64,
    pub price: i64,
}

impl CropProcure {
    /// Java `decreaseAmount(value)`.
    pub fn decrease_amount(&mut self, value: i64) -> bool {
        let next = self.amount.wrapping_sub(value);
        if next < 0 {
            return false;
        }
        self.amount = next;
        true
    }
}

/// The configuration values the manor reads (`General.ini` `AllowManor` and `AltManor*`,
/// `Rates.ini` `RateDropManor`). `Default` is the Java default of each key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManorSettings {
    /// `GeneralConfig.ALLOW_MANOR`.
    pub allow_manor: bool,
    /// `ALT_MANOR_REFRESH_TIME` (hour).
    pub refresh_time: i32,
    /// `ALT_MANOR_REFRESH_MIN`.
    pub refresh_min: i32,
    /// `ALT_MANOR_APPROVE_TIME` (hour).
    pub approve_time: i32,
    /// `ALT_MANOR_APPROVE_MIN`.
    pub approve_min: i32,
    /// `ALT_MANOR_MAINTENANCE_MIN`.
    pub maintenance_min: i32,
    /// `ALT_MANOR_SAVE_ALL_ACTIONS`.
    pub save_all_actions: bool,
    /// `ALT_MANOR_SAVE_PERIOD_RATE` (hours between two saves).
    pub save_period_rate: i32,
    /// `RatesConfig.RATE_DROP_MANOR`.
    pub rate_drop_manor: i32,
}

impl Default for ManorSettings {
    fn default() -> Self {
        Self {
            allow_manor: true,
            refresh_time: 20,
            refresh_min: 0,
            approve_time: 4,
            approve_min: 30,
            maintenance_min: 6,
            save_all_actions: false,
            save_period_rate: 2,
            rate_drop_manor: 1,
        }
    }
}

/// One `castle_manor_production` row (the world's copy of `l2_db::ProductionRow`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManorProductionRow {
    pub castle_id: i32,
    pub seed_id: i32,
    pub amount: i64,
    pub start_amount: i64,
    pub price: i64,
    pub next_period: bool,
}

/// One `castle_manor_procure` row (the world's copy of `l2_db::ProcureRow`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManorProcureRow {
    pub castle_id: i32,
    pub crop_id: i32,
    pub amount: i64,
    pub start_amount: i64,
    pub price: i64,
    pub reward_type: i32,
    pub next_period: bool,
}

/// What the game server read for the manor at startup (Java `CastleManorManager.loadDb`): every
/// row of both tables (the world sorts them by castle, the way Java reads per castle).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ManorLoad {
    pub production: Vec<ManorProductionRow>,
    pub procure: Vec<ManorProcureRow>,
}

/// A manor row write with Java's statement (`PersistCommand::Manor`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManorWrite {
    /// `storeMe`: both tables are emptied and written again.
    StoreAll {
        production: Vec<ManorProductionRow>,
        procure: Vec<ManorProcureRow>,
    },
    /// `setNextSeedProduction` with `AltManorSaveAllActions`: the castle's next-period rows are
    /// replaced.
    ReplaceNextProduction {
        castle_id: i32,
        rows: Vec<ManorProductionRow>,
    },
    /// `setNextCropProcure` with `AltManorSaveAllActions`.
    ReplaceNextProcure {
        castle_id: i32,
        rows: Vec<ManorProcureRow>,
    },
    /// `updateCurrentProduction`: `(seed id, amount left)` of the current period.
    UpdateCurrentProduction {
        castle_id: i32,
        amounts: Vec<(i32, i64)>,
    },
    /// `updateCurrentProcure`: `(crop id, amount left)` of the current period.
    UpdateCurrentProcure {
        castle_id: i32,
        amounts: Vec<(i32, i64)>,
    },
    /// `resetManorData` with `AltManorSaveAllActions`: every row of the castle.
    DeleteCastle { castle_id: i32 },
}

/// The manor timers (one `TimerKind::Manor`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManorTimer {
    /// Java `ThreadPool.schedule(this::changeMode, delay)`.
    ModeChange,
    /// Java `ThreadPool.scheduleAtFixedRate(this::storeMe, rate hours, rate hours)`.
    AutoSave,
}

/// The seeded state of one monster (Java `Attackable._seeded`, `_seed`, `_seederObjId` and
/// `_harvestItem`). A monster that is not in the table is not seeded.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct MonsterSeed {
    /// `_seeded`: the sowing worked.
    pub seeded: bool,
    /// `_seed`: the seed item id used (set before the skill is cast).
    pub seed: Option<i32>,
    /// `_seederObjId`.
    pub seeder: Option<ObjectId>,
    /// `_harvestItem`: the crop the harvester takes.
    pub harvest: Option<ItemHolder>,
}

/// The manor state of the world.
#[derive(Debug, Default)]
pub(crate) struct ManorWorld {
    pub(crate) settings: ManorSettings,
    /// `data/Seeds.xml`.
    pub(crate) seeds: Arc<Seeds>,
    /// `_mode`.
    pub(crate) mode: ManorMode,
    /// `_nextModeChange` (epoch milliseconds); `None` is Java's `null` (the manor is disabled).
    pub(crate) next_mode_change: Option<i64>,
    /// `_production`, `_productionNext`, `_procure` and `_procureNext`, by castle id.
    pub(crate) production: BTreeMap<i32, Vec<SeedProduction>>,
    pub(crate) production_next: BTreeMap<i32, Vec<SeedProduction>>,
    pub(crate) procure: BTreeMap<i32, Vec<CropProcure>>,
    pub(crate) procure_next: BTreeMap<i32, Vec<CropProcure>>,
    pub(crate) mode_timer: Option<TimerId>,
    pub(crate) save_timer: Option<TimerId>,
    /// The seeded monsters.
    pub(crate) monsters: HashMap<ObjectId, MonsterSeed>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Java `SeedProduction.decreaseAmount`: `false` and no change when there are fewer left.
    #[test]
    fn decreasing_an_amount_stops_at_zero() {
        let mut seed = SeedProduction {
            id: 5016,
            amount: 10,
            price: 7,
            start_amount: 10,
        };
        assert!(seed.decrease_amount(4));
        assert_eq!(seed.amount, 6);
        assert!(!seed.decrease_amount(7));
        assert_eq!(seed.amount, 6);
        assert!(seed.decrease_amount(6));
        assert_eq!(seed.amount, 0);

        let mut crop = CropProcure {
            id: 5073,
            amount: 3,
            reward: 1,
            start_amount: 3,
            price: 9,
        };
        assert!(!crop.decrease_amount(4));
        assert!(crop.decrease_amount(3));
        assert_eq!(crop.amount, 0);
    }

    #[test]
    fn the_modes_have_javas_names_and_the_manor_starts_approved() {
        assert_eq!(ManorMode::default(), ManorMode::Approved);
        assert_eq!(ManorMode::Modifiable.name(), "MODIFIABLE");
        assert_eq!(ManorMode::Maintenance.name(), "MAINTENANCE");
        assert_eq!(ManorMode::Approved.name(), "APPROVED");
        assert_eq!(ManorMode::Disabled.name(), "DISABLED");
    }

    #[test]
    fn the_defaults_are_the_java_defaults() {
        let s = ManorSettings::default();
        assert!(s.allow_manor && !s.save_all_actions);
        assert_eq!((s.refresh_time, s.refresh_min), (20, 0));
        assert_eq!((s.approve_time, s.approve_min), (4, 30));
        assert_eq!(
            (s.maintenance_min, s.save_period_rate, s.rate_drop_manor),
            (6, 2, 1)
        );
    }
}
