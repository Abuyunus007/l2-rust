//! Clan halls (M7-CH-01): the weekly rent of an auctionable hall, the auctions and their board,
//! the hall manager (the functions a clan buys, the hall's buffs, teleport and item creation), the
//! hall doorman and the effect of the functions (regeneration, the exp a resurrection gives back).
// Java: entity/residences/{ClanHall,AuctionableHall,ClanHallAuction}.java
// Java: managers/ClanHallAuctionManager.java
// Java: data/sql/ClanHallTable.java (initNPC at the load, `loaded()`)
// Java: entity/actor/instance/{Auctioneer,ClanHallManager,ClanHallDoorman,Doorman}.java
// Java: mechanics/stats/Formulas.java (calcHpRegen, calcMpRegen: the hall functions)
// Design: docs/design/m7-residences.md
//
// # What is where
// The halls themselves (owner, functions, doors, zone, `setOwner`, `free`) are `residence/`
// (M7-RES-01). This module adds what that card left for this one:
// - `rent.rs`: `AuctionableHall.FeeTask` and `initialyzeTask` on the timer wheel.
// - `auction.rs`: `ClanHallAuction`, the auction list (`ClanHallAuctionManager`), bids, cancelling,
//   the end of an auction and `initNPC`.
// - `auctioneer.rs`: the pages of the `Auctioneer` NPC (the auction board).
// - `manager.rs`: the `ClanHallManager` NPC.
// - `doorman.rs`: the `ClanHallDoorman` NPC.
// - `functions.rs`: the regeneration factors of the hall functions and the exp of a resurrection.
// - this file: the settings, the timers and the list of the halls (`RequestAllAgitInfo`).
//
// # Side by side with the Java game server (D-39, D-49, D-50, D-51)
// - Global writes of a scheduled job go out only with `SiegeDatabaseWrites` ([`World::siege_write`]):
//   the rent (the adena taken from the clan warehouse, the `clanhall` row, losing the hall), the end
//   of an auction (the refunds, the `auction` and `auction_bid` rows, the new owner) and the creation
//   of the NPC auctions (`initNPC`). Without the switch those jobs only move this world on: the rent
//   charges nothing and writes nothing (the Java game server collects the real rent), an auction
//   that ends changes the owner in this world only.
// - What a clan does itself is written like any player action: bidding, cancelling a bid or an
//   auction, putting the hall up for sale, buying a function.
// - The rows an action reads are read again first (D-50): the auction (its bids change on the Java
//   game server) before a bid, a cancel or the end, and the clan warehouse before adena is taken
//   from it or given back to it. The follow-up is refused when a read fails.

mod auction;
mod auctioneer;
mod doorman;
mod format;
mod functions;
mod manager;
mod rent;
pub mod siege;

use std::collections::BTreeMap;

use l2_util::ObjectId;

pub use auction::{AuctionBidRecord, AuctionRecord, AuctionWrite, Bidder, ClanHallAuction};
pub use functions::ClanHallFunctionSettings;
pub use siege::{
    FlagWarData, FlagWarRecord, HallGuardRecord, HallScript, HallSiegeHallLoad, HallSiegeLoad, HallSiegeRows,
    HallSiegeSettings, HallSiegeTimer, HallSiegeWorld, HallSiegeWrite, HallThen, SiegeStatus,
};

use crate::residence::ClanHallId;
use crate::timer::TimerId;
use crate::world::World;

/// `Feature.ini` fee tables and `AltClanHallMpBuffFree` (Java `FeatureConfig.CH_*`).
///
/// `Default` is Java's default of each key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClanHallSettings {
    /// `ClanHallTeleportFunctionFeeRatio` and `...FeeLvl1`, `...FeeLvl2`.
    pub tele: ClanHallFunctionSettings<2>,
    /// `ClanHallSupportFunctionFeeRatio` and `ClanHallSupportFeeLvl1` to `8`.
    pub support: ClanHallFunctionSettings<8>,
    /// `ClanHallMpRegenerationFunctionFeeRatio` and `ClanHallMpRegenerationFeeLvl1` to `5`.
    pub mp: ClanHallFunctionSettings<5>,
    /// `ClanHallHpRegenerationFunctionFeeRatio` and `ClanHallHpRegenerationFeeLvl1` to `13`.
    pub hp: ClanHallFunctionSettings<13>,
    /// `ClanHallExpRegenerationFunctionFeeRatio` and `ClanHallExpRegenerationFeeLvl1` to `7`.
    pub exp: ClanHallFunctionSettings<7>,
    /// `ClanHallItemCreationFunctionFeeRatio` and `...FeeLvl1` to `3`.
    pub item: ClanHallFunctionSettings<3>,
    /// `ClanHallCurtainFunctionFeeRatio` and `...FeeLvl1`, `...FeeLvl2`.
    pub curtain: ClanHallFunctionSettings<2>,
    /// `ClanHallFrontPlatformFunctionFeeRatio` and `...FeeLvl1`, `...FeeLvl2`.
    pub front: ClanHallFunctionSettings<2>,
    /// `AltClanHallMpBuffFree`: the hall manager's buffs cost no mana.
    pub buff_free: bool,
}

impl Default for ClanHallSettings {
    fn default() -> Self {
        const DAY: i64 = 86_400_000;
        const WEEK: i64 = 604_800_000;
        Self {
            tele: ClanHallFunctionSettings::new(WEEK, [7000, 14000]),
            support: ClanHallFunctionSettings::new(
                DAY,
                [2500, 5000, 7000, 11000, 21000, 36000, 37000, 52000],
            ),
            mp: ClanHallFunctionSettings::new(DAY, [2000, 3750, 6500, 13750, 20000]),
            hp: ClanHallFunctionSettings::new(
                DAY,
                [
                    700, 800, 1000, 1166, 1500, 1750, 2000, 2250, 2500, 3250, 3750, 4250, 5166,
                ],
            ),
            exp: ClanHallFunctionSettings::new(DAY, [3000, 6000, 9000, 15000, 21000, 23330, 30000]),
            item: ClanHallFunctionSettings::new(DAY, [30000, 70000, 140000]),
            curtain: ClanHallFunctionSettings::new(WEEK, [2000, 2500]),
            front: ClanHallFunctionSettings::new(3 * DAY, [1300, 4000]),
            buff_free: false,
        }
    }
}

/// The clan hall timers (one `TimerKind::ClanHall`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClanHallTimer {
    /// Java `AuctionableHall.FeeTask`: the rent of a hall is due.
    Rent(ClanHallId),
    /// Java `ClanHallAuction.AutoEndTask`: an auction (by its key) is over.
    AutoEnd(u64),
    /// The conquerable hall sieges (M7-CHSIEGE-01), see `siege/`.
    HallSiege(siege::HallSiegeTimer),
}

/// The clan hall state of the world (Java `ClanHallAuctionManager`, the pending auctions of the
/// `Auctioneer` NPCs, the cached hall of the hall NPCs).
#[derive(Debug, Default)]
pub struct ClanHallWorld {
    pub(crate) settings: ClanHallSettings,
    /// `ClanHallAuctionManager._auctions`, in list order.
    pub(crate) auctions: Vec<ClanHallAuction>,
    pub(crate) next_auction_key: u64,
    /// The pending `AutoEndTask` of each auction, by key.
    pub(crate) auction_timers: BTreeMap<u64, TimerId>,
    /// The pending `FeeTask` of each hall.
    pub(crate) rent_timers: BTreeMap<ClanHallId, TimerId>,
    /// `ClanHallTable._loaded`: the halls are read.
    pub(crate) loaded: bool,
    /// `Auctioneer._pendingAuctions`: the auctions a clan is about to confirm, per NPC and
    /// auction id.
    pub(crate) pending_auctions: BTreeMap<(ObjectId, i32), ClanHallAuction>,
    /// `ClanHallManager._clanHallId` and `ClanHallDoorman._clanHall`: the hall the NPC belongs to,
    /// found at its first use.
    pub(crate) npc_halls: BTreeMap<ObjectId, Option<ClanHallId>>,
    /// The conquerable hall sieges (`CHSiegeManager`, the engines and their scripts).
    pub(crate) siege: siege::HallSiegeWorld,
}

impl World {
    /// Installs the clan hall configuration. Call before the halls are loaded.
    pub fn set_clan_hall_settings(&mut self, settings: ClanHallSettings) {
        self.clanhall.settings = settings;
    }

    /// The clan hall configuration.
    pub fn clan_hall_settings(&self) -> &ClanHallSettings {
        &self.clanhall.settings
    }

    /// The clan hall timers.
    pub(crate) fn on_clan_hall_timer(&mut self, timer: ClanHallTimer) {
        match timer {
            ClanHallTimer::Rent(id) => self.on_rent_due(id),
            ClanHallTimer::AutoEnd(key) => self.on_auction_auto_end(key),
            ClanHallTimer::HallSiege(timer) => self.on_hall_siege_timer(timer),
        }
    }
}

impl World {
    /// Java `showChatWindow(player)` of the NPC classes of this module (`Auctioneer`,
    /// `ClanHallManager`, `ClanHallDoorman`); `false` for any other NPC.
    pub(crate) fn clan_hall_npc_show_chat_window(&mut self, npc: ObjectId, player: ObjectId) -> bool {
        let npc_type = self
            .npcs
            .get(&npc)
            .and_then(|n| self.npc_world.template(n.npc_id))
            .map(|t| t.npc_type.clone());
        match npc_type.as_deref() {
            Some("Auctioneer") => self.auctioneer_show_chat_window(npc, player),
            Some("ClanHallManager") => self.clan_hall_manager_show_chat_window(npc, player),
            Some("ClanHallDoorman") => self.clan_hall_doorman_show_chat_window(npc, player),
            _ => return false,
        }
        true
    }
}

impl World {
    /// Java `RequestAllAgitInfo.runImpl`: `ExShowAgitInfo`, the auctionable halls by id with their
    /// owner clan and its leader.
    pub(crate) fn on_request_all_agit_info(&mut self, id: ObjectId) {
        if !self.players.contains_key(&id) {
            return;
        }
        let halls = self
            .clan_halls()
            .filter(|h| !h.is_siegable_hall())
            .map(|h| {
                // `getClan(ownerId).getName()` throws for a clan that is gone; the line is empty.
                let (owner_clan_name, leader_name) = if h.owner_id <= 0 {
                    (String::new(), String::new())
                } else {
                    self.clan(h.owner_id).map_or_else(
                        || (String::new(), String::new()),
                        |clan| (clan.name.clone(), auction::leader_name_of(clan)),
                    )
                };
                l2_protocol_game::server::AgitInfo {
                    hall_id: h.id.0,
                    owner_clan_name,
                    leader_name,
                    grade_below_one: h.grade() < 1,
                }
            })
            .collect();
        self.send(id, &l2_protocol_game::server::ExShowAgitInfo { halls });
    }
}
