//! Item auctions: the auctioneer NPCs that sell the items of `data/ItemAuctions.xml` to the
//! highest bidder (L-05).
// Java: managers/ItemAuctionManager.java
// Java: entity/item/auction/{ItemAuctionInstance,ItemAuction,ItemAuctionBid,AuctionItem,
//       AuctionDateGenerator,ItemAuctionState,ItemAuctionExtendState}.java
// Java: network/clientpackets/{RequestInfoItemAuction,RequestBidItemAuction}.java
// Java: network/serverpackets/ExItemAuctionInfoPacket.java
// Java: dist/game/data/scripts/handlers/bypass/npc/ItemAuctionLink.java
//
// # Where it lives
// One [`ItemAuctionWorld`] in the world (D-10): the instances of the XML (one per auctioneer NPC
// id), each with its auctions, the current and the next one and its state timer on the wheel
// (`TimerKind::ItemAuction`). The XML, the `item_auction` rows with their bids and the highest
// auction id are read before the world starts ([`ItemAuctionLoad`], D-15) like Java's manager
// constructor does.
//
// # Files
// - `date.rs`: `AuctionDateGenerator`, the start of the next auction.
// - `state.rs`: the load, `checkAndSetCurrentAndNextAuction`, the state timer, the end of an
//   auction (the winner's item).
// - `bid.rs`: `registerBid`, `cancelBid`.
// - `request.rs`: the two client packets, the `ItemAuction` bypass, `ExItemAuctionInfo`.
//
// # Rows (D-39)
// `item_auction` (the state of an auction) and `item_auction_bid` (one row per bidder) are shared
// with the Java GS. Statements are Java's, as [`ItemAuctionWrite`] through
// `PersistCommand::ItemAuction`. The winner's item is a new `items` row (`loc = WAREHOUSE`).
//
// # Side by side (D-53)
// `item_auction` rows and the end of an auction are global: with `CycleDatabaseWrites = False`
// ([`ItemAuctionSettings::cycle_database_writes`]) this server
// - writes no `item_auction` row and deletes none (new auctions, the start and the end, expired
//   or invalid rows),
// - hands out no winner's item and clears no canceled bid at the end (the Java GS does, from the
//   shared `item_auction_bid` rows),
// - reads the auctions of an instance again before it shows them, takes a bid or returns a
//   canceled one (`PersistCommand::ItemAuctionRequest` / `WorldCommand::ItemAuctionReread`, D-50:
//   a failed read refuses the action),
// - still keeps the states in memory and moves them with its own timers, so a player sees the
//   countdown; what the Java GS wrote (a state further on) wins at every re-read.
// The bids and canceled bids of a player are per-player rows and are written in both modes.
//
// # Left out on purpose
// - Java sends the "bidder exists" extension message from a thread pool task; it is sent at once
//   after the "bid submitted" message here.
// - `ItemProcessType` logging (`LogItems`).
// - NOTE(java-quirk): `ItemAuction.endingTime` is never stored: an extended auction ends at its
//   original time again after a restart.
// - A bid the Java GS never saw (a Kain bid while the switch is off) is unknown to Java's memory,
//   so Java may end the auction with another winner. Nothing on this side can prevent that.

mod bid;
mod date;
mod request;
mod state;

pub use date::DateGenerator;
pub use request::ItemAuctionInput;
pub(crate) use request::item_auction_link_handler;

use std::collections::{BTreeMap, HashMap};

use l2_util::ObjectId;

use crate::timer::TimerId;

/// `START_TIME_SPACE`: a new auction starts at least a minute from now.
pub const START_TIME_SPACE: i64 = 60_000;
/// `FINISH_TIME_SPACE`: ten minutes between the end of one auction and the start of the next.
pub const FINISH_TIME_SPACE: i64 = 600_000;
/// `ENDING_TIME_EXTEND_5`.
pub const ENDING_TIME_EXTEND_5: i64 = 300_000;
/// `ENDING_TIME_EXTEND_3`.
pub const ENDING_TIME_EXTEND_3: i64 = 180_000;
/// The largest bid `registerBid` takes.
pub const MAX_BID: i64 = 100_000_000_000;

const MILLIS_IN_DAY: i64 = 86_400_000;

/// Java `ItemAuctionState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ItemAuctionState {
    Created,
    Started,
    Finished,
}

impl ItemAuctionState {
    /// `getStateId()`.
    pub fn state_id(self) -> i8 {
        match self {
            Self::Created => 0,
            Self::Started => 1,
            Self::Finished => 2,
        }
    }

    /// `stateForStateId(id)`.
    pub fn from_id(id: i8) -> Option<Self> {
        match id {
            0 => Some(Self::Created),
            1 => Some(Self::Started),
            2 => Some(Self::Finished),
            _ => None,
        }
    }
}

/// Java `ItemAuctionExtendState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtendState {
    Initial,
    ExtendBy5Min,
    ExtendBy3Min,
    ExtendByConfigPhaseA,
    ExtendByConfigPhaseB,
}

/// An `<item>` of an `<instance>` (Java `AuctionItem`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuctionItemDef {
    pub auction_item_id: i32,
    /// `auctionLength`, minutes.
    pub auction_length: i32,
    pub auction_init_bid: i64,
    pub item_id: i32,
    pub item_count: i64,
    /// `<extra augmentation_id>`, 0 for none.
    pub augmentation_id: i32,
}

/// An `<instance>` of `ItemAuctions.xml`: the auctioneer NPC id, when it starts auctions and what
/// it sells.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceDef {
    pub id: i32,
    pub generator: DateGenerator,
    pub items: Vec<AuctionItemDef>,
}

/// An `item_auction` row with its `item_auction_bid` rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemAuctionRecord {
    pub auction_id: i32,
    pub instance_id: i32,
    pub auction_item_id: i32,
    pub starting_time: i64,
    pub ending_time: i64,
    /// `auctionStateId`.
    pub state_id: i8,
    /// `(playerObjId, playerBid)`.
    pub bids: Vec<(i32, i64)>,
}

/// What the game server read before the world started (Java `ItemAuctionManager` and the
/// `ItemAuctionInstance` constructors).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ItemAuctionLoad {
    /// `_auctionIds`: the highest `auctionId` plus one, 1 for an empty table.
    pub next_auction_id: i32,
    pub instances: Vec<InstanceDef>,
    /// The rows of the instances in `instances`.
    pub auctions: Vec<ItemAuctionRecord>,
}

/// The values the item auctions read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemAuctionSettings {
    /// `GeneralConfig.ALT_ITEM_AUCTION_ENABLED` (`AltItemAuctionEnabled`).
    pub enabled: bool,
    /// `ALT_ITEM_AUCTION_EXPIRED_AFTER` (`AltItemAuctionExpiredAfter`), days.
    pub expired_after_days: i32,
    /// `ALT_ITEM_AUCTION_TIME_EXTENDS_ON_BID` (`AltItemAuctionTimeExtendsOnBid` * 1000),
    /// milliseconds.
    pub time_extends_on_bid_ms: i64,
    /// `CycleDatabaseWrites` of `config/SideBySide.ini` (D-53), see the module notes.
    pub cycle_database_writes: bool,
}

impl Default for ItemAuctionSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            expired_after_days: 14,
            time_extends_on_bid_ms: 0,
            cycle_database_writes: false,
        }
    }
}

impl ItemAuctionSettings {
    /// `TimeUnit.MILLISECONDS.convert(ALT_ITEM_AUCTION_EXPIRED_AFTER, TimeUnit.DAYS)`.
    pub(crate) fn expired_after_ms(&self) -> i64 {
        i64::from(self.expired_after_days).wrapping_mul(MILLIS_IN_DAY)
    }
}

/// A row write with Java's statement (see the module notes for which are global).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemAuctionWrite {
    /// `ItemAuction.storeMe`: `INSERT INTO item_auction (auctionId,instanceId,auctionItemId,
    /// startingTime,endingTime,auctionStateId) VALUES (?,?,?,?,?,?) ON DUPLICATE KEY UPDATE
    /// auctionStateId=?`. Global.
    Store {
        auction_id: i32,
        instance_id: i32,
        auction_item_id: i32,
        starting_time: i64,
        ending_time: i64,
        state_id: i8,
    },
    /// `ItemAuction.updatePlayerBidInternal`: `INSERT INTO item_auction_bid (auctionId,
    /// playerObjId, playerBid) VALUES (?, ?, ?) ON DUPLICATE KEY UPDATE playerBid = ?`.
    Bid {
        auction_id: i32,
        player_obj_id: i32,
        bid: i64,
    },
    /// `ItemAuction.updatePlayerBidInternal` with `delete`: `DELETE FROM item_auction_bid WHERE
    /// auctionId = ? AND playerObjId = ?`.
    DeleteBid { auction_id: i32, player_obj_id: i32 },
    /// `ItemAuctionManager.deleteAuction`: `DELETE FROM item_auction WHERE auctionId=?`, then
    /// `DELETE FROM item_auction_bid WHERE auctionId=?`. Global.
    DeleteAuction { auction_id: i32 },
}

/// What to do when the re-read of an instance's rows is answered (D-50).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemAuctionThen {
    /// `RequestInfoItemAuction`.
    Info { player: ObjectId },
    /// The `ItemAuction show` bypass.
    Show { player: ObjectId },
    /// `RequestBidItemAuction`.
    Bid { player: ObjectId, bid: i64 },
    /// The `ItemAuction cancel` bypass.
    Cancel { player: ObjectId },
}

/// The item auction timers (one `TimerKind::ItemAuction`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemAuctionTimer {
    /// Java `ItemAuctionInstance.ScheduleAuctionTask`: the auction (of the instance) starts, is
    /// extended or ends.
    State { instance: i32, auction: i32 },
}

/// Java `ItemAuctionBid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Bid {
    pub player: i32,
    pub last_bid: i64,
}

impl Bid {
    /// `isCanceled()`.
    pub fn is_canceled(&self) -> bool {
        self.last_bid <= 0
    }
}

/// Java `ItemAuction`.
#[derive(Debug, Clone)]
pub(crate) struct ItemAuction {
    pub id: i32,
    pub instance_id: i32,
    pub starting_time: i64,
    pub ending_time: i64,
    pub item: AuctionItemDef,
    pub bids: Vec<Bid>,
    pub state: ItemAuctionState,
    pub scheduled_extend: ExtendState,
    pub extend: ExtendState,
    /// `_highestBid`: the player whose bid it is.
    pub highest: Option<i32>,
    /// `_lastBidPlayerObjId`.
    pub last_bid_player: i32,
    /// The object id the item of the packet shows (Java creates a real item for `ItemInfo`).
    pub item_object_id: i32,
    /// Created while `CycleDatabaseWrites` is off: no `item_auction` row, so a re-read must not
    /// drop it.
    pub local_only: bool,
}

impl ItemAuction {
    #[allow(clippy::too_many_arguments)] // Java's constructor takes these.
    pub fn new(
        id: i32,
        instance_id: i32,
        starting_time: i64,
        ending_time: i64,
        item: AuctionItemDef,
        bids: Vec<Bid>,
        state: ItemAuctionState,
        item_object_id: i32,
    ) -> Self {
        let mut auction = Self {
            id,
            instance_id,
            starting_time,
            ending_time,
            item,
            bids,
            state,
            scheduled_extend: ExtendState::Initial,
            extend: ExtendState::Initial,
            highest: None,
            last_bid_player: 0,
            item_object_id,
            local_only: false,
        };
        auction.find_highest();
        auction
    }

    /// The loop of the constructor: the first bid with the largest `lastBid`.
    pub fn find_highest(&mut self) {
        let mut highest: Option<usize> = None;
        for (index, bid) in self.bids.iter().enumerate() {
            if highest.is_none_or(|h| self.bids[h].last_bid < bid.last_bid) {
                highest = Some(index);
            }
        }
        self.highest = highest.map(|h| self.bids[h].player);
    }

    /// `getBidFor(playerObjId)`: Java searches from the end.
    pub fn bid_for(&self, player: i32) -> Option<&Bid> {
        self.bids.iter().rev().find(|b| b.player == player)
    }

    pub fn bid_for_mut(&mut self, player: i32) -> Option<&mut Bid> {
        self.bids.iter_mut().rev().find(|b| b.player == player)
    }

    /// `getHighestBid().getLastBid()`.
    pub fn highest_bid_value(&self) -> Option<i64> {
        self.highest.and_then(|p| self.bid_for(p)).map(|b| b.last_bid)
    }
}

/// Java `ItemAuctionInstance`.
#[derive(Debug)]
pub(crate) struct Instance {
    pub generator: DateGenerator,
    pub items: Vec<AuctionItemDef>,
    /// `_auctions`, by auction id.
    pub auctions: BTreeMap<i32, ItemAuction>,
    pub current: Option<i32>,
    pub next: Option<i32>,
    /// `_stateTask`.
    pub state_task: Option<TimerId>,
}

/// The item auction state of the world (Java `ItemAuctionManager`).
#[derive(Debug)]
pub struct ItemAuctionWorld {
    pub(crate) settings: ItemAuctionSettings,
    /// `_managerInstances`, by auctioneer NPC id.
    pub(crate) instances: BTreeMap<i32, Instance>,
    /// `_auctionIds`.
    pub(crate) next_auction_id: i32,
    /// `Player._lastItemAuctionInfoRequest`, by player.
    pub(crate) last_request: HashMap<ObjectId, i64>,
}

impl Default for ItemAuctionWorld {
    fn default() -> Self {
        Self {
            settings: ItemAuctionSettings::default(),
            instances: BTreeMap::new(),
            next_auction_id: 1,
            last_request: HashMap::new(),
        }
    }
}

/// `DateTimeFormatter.ofPattern("HH:mm:ss dd.MM.yyyy")` in the machine's zone (D-48).
pub(crate) fn format_local_time(unix_ms: i64) -> String {
    let local = unix_ms.wrapping_add(l2_util::time::local_offset_millis(unix_ms));
    let days = local.div_euclid(MILLIS_IN_DAY);
    let millis = local.rem_euclid(MILLIS_IN_DAY);
    // Days since 1970-01-01 to a civil date (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!(
        "{:02}:{:02}:{:02} {day:02}.{month:02}.{year:04}",
        millis / 3_600_000,
        millis / 60_000 % 60,
        millis / 1000 % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_state_ids_are_javas() {
        for state in [
            ItemAuctionState::Created,
            ItemAuctionState::Started,
            ItemAuctionState::Finished,
        ] {
            assert_eq!(ItemAuctionState::from_id(state.state_id()), Some(state));
        }
        assert_eq!(ItemAuctionState::from_id(3), None);
        assert_eq!(ItemAuctionState::from_id(-1), None);
    }

    fn item() -> AuctionItemDef {
        AuctionItemDef {
            auction_item_id: 1,
            auction_length: 300,
            auction_init_bid: 100,
            item_id: 9901,
            item_count: 1,
            augmentation_id: 0,
        }
    }

    #[test]
    fn the_first_of_equal_highest_bids_stays_the_highest() {
        let bids = vec![
            Bid {
                player: 1,
                last_bid: 500,
            },
            Bid {
                player: 2,
                last_bid: 500,
            },
            Bid {
                player: 3,
                last_bid: 400,
            },
        ];
        let auction = ItemAuction::new(1, 1, 0, 10, item(), bids, ItemAuctionState::Started, 0);
        assert_eq!(auction.highest, Some(1));
        assert_eq!(auction.highest_bid_value(), Some(500));
        assert!(auction.bid_for(3).is_some());
        assert!(auction.bid_for(4).is_none());
    }

    #[test]
    fn a_canceled_bid_is_zero_or_less() {
        assert!(
            Bid {
                player: 1,
                last_bid: -1
            }
            .is_canceled()
        );
        assert!(
            Bid {
                player: 1,
                last_bid: 0
            }
            .is_canceled()
        );
        assert!(
            !Bid {
                player: 1,
                last_bid: 1
            }
            .is_canceled()
        );
    }

    #[test]
    fn a_time_prints_like_java() {
        // 2026-10-27 12:00:00 UTC in the zone of the machine: only the shape and the month that
        // every zone shares can be checked.
        let text = format_local_time(1_793_102_400_000);
        assert_eq!(text.len(), 19);
        assert!(text.ends_with(".10.2026"));
    }
}
