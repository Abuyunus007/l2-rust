//! Clan hall auctions: the auction list, bids, cancelling and the end of an auction.
// Java: entity/residences/ClanHallAuction.java
// Java: managers/ClanHallAuctionManager.java (load, getAuction, getAuctionIndex, initNPC)
// Java: data/sql/ClanHallTable.java (load: `initNPC` for a free hall without auction)
// Java: entity/clan/Clan.java (setAuctionBiddedAt)
//
// # How it runs here
// Java keeps every auction as an object with its own `AutoEndTask`; here the manager's list holds
// the auctions, a timer names an auction by its key (the hall's auction id is the same for every
// auction of a hall, a key tells two auctions of one hall apart).
//
// # Shared rows (D-50)
// Bidding, cancelling and ending change rows the Java game server may have changed (a bid of
// another clan, the end date, the auction itself). So each of these asks the persistence task to
// read the auction again first (`ResidenceReadKind::Auction`) and goes on with the rows it gets
// (`ResidenceReadThen::Auction*`). Adena taken from the clan warehouse is read again too
// (`PendingClanWindow::Auction*`, M6-SYNC-01). A read that fails refuses the action.
//
// # Side by side (D-51)
// The end of an auction and `initNPC` are jobs: their rows (and the adena they give back) go out
// only with `SiegeDatabaseWrites`. A clan's own bid, cancel or sale is written like any player
// action.
//
// # Where this differs from Java
// - NOTE: Java's `AutoEndTask` of an auction that was removed from the list (`cancelAuction`)
//   still runs at the old end date and refunds the seller with the highest bid of the cancelled
//   auction. The task is cancelled here with the auction.
// - NOTE: `getBidders()` is a `ConcurrentHashMap`; the bidder list shows the bidders by clan id.

use std::collections::BTreeMap;

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;
use l2_util::ObjectId;
use tracing::{error, info, warn};

use super::ClanHallTimer;
use crate::outbox::{ItemRowOp, PersistCommand};
use crate::residence::{ClanHallId, ResidenceReadKind, ResidenceReadThen, ResidenceWrite, Writes};
use crate::timer::{TimerKind, TimerWheel};
use crate::warehouse::PendingClanWindow;
use crate::world::{World, unix_ms_now};
use l2_data::item::ADENA_ID;

/// `7 * 24 * 3600000`.
const WEEK_MS: i64 = 604_800_000;

/// The auctions of the halls the game starts with, `ITEM_INIT_DATA`: hall id, name, starting bid,
/// end date (long past, `startAutoTask` moves it a week from now).
const ITEM_INIT_DATA: [(i32, &str, i64, i64); 38] = [
    (22, "Moonstone Hall", 20_000_000, 1_073_037_600_000),
    (23, "Onyx Hall", 20_000_000, 1_073_037_600_000),
    (24, "Topaz Hall", 20_000_000, 1_073_037_600_000),
    (25, "Ruby Hall", 20_000_000, 1_073_037_600_000),
    (26, "Crystal Hall", 20_000_000, 1_073_037_600_000),
    (27, "Onyx Hall", 20_000_000, 1_073_037_600_000),
    (28, "Sapphire Hall", 20_000_000, 1_073_037_600_000),
    (29, "Moonstone Hall", 20_000_000, 1_073_037_600_000),
    (30, "Emerald Hall", 20_000_000, 1_073_037_600_000),
    (31, "The Atramental Barracks", 8_000_000, 1_073_037_600_000),
    (32, "The Scarlet Barracks", 8_000_000, 1_073_037_600_000),
    (33, "The Viridian Barracks", 8_000_000, 1_073_037_600_000),
    (36, "The Golden Chamber", 50_000_000, 1_106_827_200_000),
    (37, "The Silver Chamber", 50_000_000, 1_106_827_200_000),
    (38, "The Mithril Chamber", 50_000_000, 1_106_827_200_000),
    (39, "Silver Manor", 50_000_000, 1_106_827_200_000),
    (40, "Gold Manor", 50_000_000, 1_106_827_200_000),
    (41, "The Bronze Chamber", 50_000_000, 1_106_827_200_000),
    (42, "The Golden Chamber", 50_000_000, 1_106_827_200_000),
    (43, "The Silver Chamber", 50_000_000, 1_106_827_200_000),
    (44, "The Mithril Chamber", 50_000_000, 1_106_827_200_000),
    (45, "The Bronze Chamber", 50_000_000, 1_106_827_200_000),
    (46, "Silver Manor", 50_000_000, 1_106_827_200_000),
    (47, "Moonstone Hall", 50_000_000, 1_106_827_200_000),
    (48, "Onyx Hall", 50_000_000, 1_106_827_200_000),
    (49, "Emerald Hall", 50_000_000, 1_106_827_200_000),
    (50, "Sapphire Hall", 50_000_000, 1_106_827_200_000),
    (51, "Mont Chamber", 50_000_000, 1_106_827_200_000),
    (52, "Astaire Chamber", 50_000_000, 1_106_827_200_000),
    (53, "Aria Chamber", 50_000_000, 1_106_827_200_000),
    (54, "Yiana Chamber", 50_000_000, 1_106_827_200_000),
    (55, "Roien Chamber", 50_000_000, 1_106_827_200_000),
    (56, "Luna Chamber", 50_000_000, 1_106_827_200_000),
    (57, "Traban Chamber", 50_000_000, 1_106_827_200_000),
    (58, "Eisen Hall", 20_000_000, 1_106_827_200_000),
    (59, "Heavy Metal Hall", 20_000_000, 1_106_827_200_000),
    (60, "Molten Ore Hall", 20_000_000, 1_106_827_200_000),
    (61, "Titan Hall", 20_000_000, 1_106_827_200_000),
];

/// A bidder of an auction (Java `ClanHallAuction.Bidder`), by clan id in `bidders`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bidder {
    /// `_name`: the clan leader's name (or the player's, from the `auction_bid` row).
    pub name: String,
    pub clan_name: String,
    pub bid: i64,
    /// `_timeBid` (epoch milliseconds).
    pub time_bid: i64,
}

/// A clan hall auction (Java `ClanHallAuction`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClanHallAuction {
    /// Tells two auctions with the same hall id apart (the timers name it).
    pub(crate) key: u64,
    /// `_id`: the hall's id.
    pub id: i32,
    pub end_date: i64,
    pub highest_bidder_id: i32,
    pub highest_bidder_name: String,
    pub highest_bidder_max_bid: i64,
    pub item_id: i32,
    pub item_name: String,
    pub item_object_id: i32,
    pub item_type: String,
    pub seller_id: i32,
    pub seller_clan_name: String,
    pub seller_name: String,
    pub current_bid: i64,
    pub starting_bid: i64,
    /// `_bidders`, by clan id.
    pub bidders: BTreeMap<i32, Bidder>,
}

/// One `auction_bid` row, as `loadBid` reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuctionBidRecord {
    pub bidder_id: i32,
    pub bidder_name: String,
    pub max_bid: i64,
    pub clan_name: String,
    pub time_bid: i64,
}

/// An `auction` row with its bids (`ClanHallAuction.load` and `loadBid`), the highest bid first.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AuctionRecord {
    pub id: i32,
    pub seller_id: i32,
    pub seller_name: String,
    pub seller_clan_name: String,
    pub item_type: String,
    pub item_id: i32,
    pub item_object_id: i32,
    pub item_name: String,
    pub starting_bid: i64,
    pub current_bid: i64,
    pub end_date: i64,
    pub bidders: Vec<AuctionBidRecord>,
}

/// A write of the auction tables, with Java's statement (`ResidenceWrite::ClanHallAuction`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuctionWrite {
    /// `saveAuctionDate`: `Update auction set endDate = ? where id = ?`.
    EndDate { id: i32, end_date: i64 },
    /// `updateInDB` for a clan that bid before: `UPDATE auction_bid SET bidderId=?, bidderName=?,
    /// maxBid=?, time_bid=? WHERE auctionId=? AND bidderId=?`.
    BidUpdate {
        auction_id: i32,
        bidder_id: i32,
        bidder_name: String,
        max_bid: i64,
        time_bid: i64,
    },
    /// `updateInDB` for a new bidder: `INSERT INTO auction_bid (id, auctionId, bidderId,
    /// bidderName, maxBid, clan_name, time_bid) VALUES (?, ?, ?, ?, ?, ?, ?)`.
    BidInsert {
        id: i32,
        auction_id: i32,
        bidder_id: i32,
        bidder_name: String,
        max_bid: i64,
        clan_name: String,
        time_bid: i64,
    },
    /// `removeBids`: `DELETE FROM auction_bid WHERE auctionId=?`.
    BidsDelete { auction_id: i32 },
    /// `deleteAuctionFromDB`: `DELETE FROM auction WHERE itemId=?`.
    AuctionDelete { item_id: i32 },
    /// `cancelBid`: `DELETE FROM auction_bid WHERE auctionId=? AND bidderId=?`.
    BidDelete { auction_id: i32, bidder_id: i32 },
    /// `confirmAuction`: `INSERT INTO auction (...)`.
    AuctionInsert(Box<AuctionRecord>),
    /// `initNPC`: `INSERT INTO auction VALUES (...)` of the hall's `ITEM_INIT_DATA` line.
    NpcAuctionInsert {
        hall_id: i32,
        name: String,
        starting_bid: i64,
        end_date: i64,
    },
    /// `Clan.setAuctionBiddedAt(id, true)`: `UPDATE clan_data SET auction_bid_at=? WHERE
    /// clan_id=?`.
    ClanBidAt { clan_id: i32, auction_id: i32 },
}

impl ClanHallAuction {
    /// `new ClanHallAuction(auctionId)` after `load()` and `loadBid()`.
    fn from_record(key: u64, record: &AuctionRecord) -> Self {
        let mut auction = Self {
            key,
            id: record.id,
            end_date: 0,
            highest_bidder_id: 0,
            highest_bidder_name: String::new(),
            highest_bidder_max_bid: 0,
            item_id: 0,
            item_name: String::new(),
            item_object_id: 0,
            item_type: String::new(),
            seller_id: 0,
            seller_clan_name: String::new(),
            seller_name: String::new(),
            current_bid: 0,
            starting_bid: 0,
            bidders: BTreeMap::new(),
        };
        auction.apply_record(record);
        auction
    }

    /// The columns `load()` reads and the bids `loadBid()` reads.
    fn apply_record(&mut self, record: &AuctionRecord) {
        self.current_bid = record.current_bid;
        self.end_date = record.end_date;
        self.item_id = record.item_id;
        self.item_name.clone_from(&record.item_name);
        self.item_object_id = record.item_object_id;
        self.item_type.clone_from(&record.item_type);
        self.seller_id = record.seller_id;
        self.seller_clan_name.clone_from(&record.seller_clan_name);
        self.seller_name.clone_from(&record.seller_name);
        self.starting_bid = record.starting_bid;
        self.load_bids(&record.bidders);
    }

    /// Java `loadBid()`: the first row (the highest bid) is the highest bidder.
    fn load_bids(&mut self, rows: &[AuctionBidRecord]) {
        self.highest_bidder_id = 0;
        self.highest_bidder_name.clear();
        self.highest_bidder_max_bid = 0;
        self.bidders.clear();
        for (index, row) in rows.iter().enumerate() {
            if index == 0 {
                self.highest_bidder_id = row.bidder_id;
                self.highest_bidder_name.clone_from(&row.bidder_name);
                self.highest_bidder_max_bid = row.max_bid;
            }
            self.bidders.insert(
                row.bidder_id,
                Bidder {
                    name: row.bidder_name.clone(),
                    clan_name: row.clan_name.clone(),
                    bid: row.max_bid,
                    time_bid: row.time_bid,
                },
            );
        }
    }

    /// The rows of this auction as `confirmAuction` writes them and a re-read returns them.
    pub(crate) fn to_record(&self) -> AuctionRecord {
        let mut bidders: Vec<AuctionBidRecord> = self
            .bidders
            .iter()
            .map(|(id, b)| AuctionBidRecord {
                bidder_id: *id,
                bidder_name: b.name.clone(),
                max_bid: b.bid,
                clan_name: b.clan_name.clone(),
                time_bid: b.time_bid,
            })
            .collect();
        bidders.sort_by_key(|b| std::cmp::Reverse(b.max_bid));
        AuctionRecord {
            id: self.id,
            seller_id: self.seller_id,
            seller_name: self.seller_name.clone(),
            seller_clan_name: self.seller_clan_name.clone(),
            item_type: self.item_type.clone(),
            item_id: self.item_id,
            item_object_id: self.item_object_id,
            item_name: self.item_name.clone(),
            starting_bid: self.starting_bid,
            current_bid: self.current_bid,
            end_date: self.end_date,
            bidders,
        }
    }

    /// Java `new ClanHallAuction(itemId, clan, delay, bid, name)`: an auction a clan is about to
    /// put up, not in the list yet.
    pub(crate) fn pending(
        id: i32,
        seller_id: i32,
        seller_name: String,
        seller_clan_name: String,
        delay_ms: i64,
        bid: i64,
        name: String,
    ) -> Self {
        Self {
            key: 0,
            id,
            end_date: unix_ms_now().wrapping_add(delay_ms),
            highest_bidder_id: 0,
            highest_bidder_name: String::new(),
            highest_bidder_max_bid: 0,
            item_id: id,
            item_name: name,
            item_object_id: 0,
            item_type: "ClanHall".to_owned(),
            seller_id,
            seller_clan_name,
            seller_name,
            current_bid: 0,
            starting_bid: bid,
            bidders: BTreeMap::new(),
        }
    }
}

/// Java `Clan.getLeaderName()`.
pub(crate) fn leader_name_of(clan: &crate::clan::Clan) -> String {
    clan.leader_member().map(|m| m.name.clone()).unwrap_or_default()
}

impl World {
    // ------------------------------------------------------------------ the list

    /// Java `ClanHallAuctionManager.getAuctions()`.
    pub fn clan_hall_auctions(&self) -> &[ClanHallAuction] {
        &self.clanhall.auctions
    }

    /// Java `ClanHallAuctionManager.getAuction(auctionId)`.
    pub fn clan_hall_auction(&self, auction_id: i32) -> Option<&ClanHallAuction> {
        self.clanhall.auctions.iter().find(|a| a.id == auction_id)
    }

    fn auction_by_key(&self, key: u64) -> Option<&ClanHallAuction> {
        self.clanhall.auctions.iter().find(|a| a.key == key)
    }

    fn auction_by_key_mut(&mut self, key: u64) -> Option<&mut ClanHallAuction> {
        self.clanhall.auctions.iter_mut().find(|a| a.key == key)
    }

    /// Java `ClanHallAuctionManager.load()` and the `initNPC` calls of `ClanHallTable.load`: the
    /// auctions the database has, then an auction for every free hall that has none.
    pub fn load_clan_hall_auctions(&mut self, records: Vec<AuctionRecord>) {
        let count = records.len();
        for record in &records {
            self.add_auction_from_record(record);
        }
        info!("ClanHallAuctionManager: Loaded {count} auction(s)");
        let free: Vec<i32> = self
            .clan_halls()
            .filter(|h| !h.is_siegable_hall() && h.owner_id <= 0 && h.lease() > 0)
            .map(|h| h.id.0)
            .collect();
        for id in free {
            if self.clan_hall_auction(id).is_none() {
                self.clan_hall_init_npc_auction(id);
            }
        }
    }

    /// `new ClanHallAuction(id)` added to the list: the auction and its end task.
    fn add_auction_from_record(&mut self, record: &AuctionRecord) -> u64 {
        self.clanhall.next_auction_key += 1;
        let key = self.clanhall.next_auction_key;
        self.clanhall
            .auctions
            .push(ClanHallAuction::from_record(key, record));
        self.start_auction_auto_task(key);
        key
    }

    /// Java `ClanHallAuctionManager.initNPC(id)`: the hall's NPC auction (a job: the row goes out
    /// only with `SiegeDatabaseWrites`).
    pub(crate) fn clan_hall_init_npc_auction(&mut self, hall_id: i32) {
        let Some((_, name, starting_bid, end_date)) =
            ITEM_INIT_DATA.iter().find(|entry| entry.0 == hall_id).copied()
        else {
            warn!("ClanHallAuctionManager: Clan Hall auction not found for Id :{hall_id}");
            return;
        };
        self.auction_write(
            Writes::Job,
            AuctionWrite::NpcAuctionInsert {
                hall_id,
                name: name.to_owned(),
                starting_bid,
                end_date,
            },
        );
        // `new ClanHallAuction(id)` reads the row it just wrote.
        let record = AuctionRecord {
            id: hall_id,
            seller_id: 0,
            seller_name: "NPC".to_owned(),
            seller_clan_name: "NPC Clan".to_owned(),
            item_type: "ClanHall".to_owned(),
            item_id: hall_id,
            item_object_id: 0,
            item_name: name.to_owned(),
            starting_bid,
            current_bid: 0,
            end_date,
            bidders: Vec::new(),
        };
        self.add_auction_from_record(&record);
        info!("ClanHallAuctionManager: Created auction for ClanHall: {hall_id}");
    }

    /// Removes an auction from the list and stops its end task (Java `getAuctions().remove(..)`).
    fn remove_auction(&mut self, key: u64) -> Option<ClanHallAuction> {
        let index = self.clanhall.auctions.iter().position(|a| a.key == key)?;
        if let Some(timer) = self.clanhall.auction_timers.remove(&key) {
            self.timers.cancel(timer);
        }
        Some(self.clanhall.auctions.remove(index))
    }

    // ------------------------------------------------------------------ writes

    /// An auction row write as `writes` says.
    pub(crate) fn auction_write(&mut self, writes: Writes, write: AuctionWrite) {
        self.write_residence(writes, ResidenceWrite::ClanHallAuction(write));
    }

    /// Java `Clan.setAuctionBiddedAt(id, storeInDb)`; `writes` `None` is `storeInDb` false.
    pub(crate) fn clan_set_auction_bidded_at(
        &mut self,
        clan_id: i32,
        auction_id: i32,
        writes: Option<Writes>,
    ) {
        if let Some(clan) = self.clan_mut(clan_id) {
            clan.auction_bid_at = auction_id;
        }
        if let Some(writes) = writes {
            self.auction_write(writes, AuctionWrite::ClanBidAt { clan_id, auction_id });
        }
    }

    // ------------------------------------------------------------------ the end task

    /// Java `startAutoTask()`: an auction whose end is past ends a week from now; the end task is
    /// due at the end date.
    fn start_auction_auto_task(&mut self, key: u64) {
        let now = unix_ms_now();
        let Some(auction) = self.auction_by_key(key) else {
            return;
        };
        let (id, end_date) = (auction.id, auction.end_date);
        let delay = if end_date <= now {
            let new_end = now.wrapping_add(WEEK_MS);
            if let Some(auction) = self.auction_by_key_mut(key) {
                auction.end_date = new_end;
            }
            self.auction_write(
                Writes::Job,
                AuctionWrite::EndDate {
                    id,
                    end_date: new_end,
                },
            );
            0
        } else {
            end_date - now
        };
        self.schedule_auction_end(key, delay);
    }

    /// `ThreadPool.schedule(new AutoEndTask(), delay)`.
    fn schedule_auction_end(&mut self, key: u64, delay_ms: i64) {
        let due = self.tick + TimerWheel::ms_to_ticks(u64::try_from(delay_ms).unwrap_or(0));
        let timer = self
            .timers
            .schedule_at(due, TimerKind::ClanHall(ClanHallTimer::AutoEnd(key)));
        if let Some(old) = self.clanhall.auction_timers.insert(key, timer) {
            self.timers.cancel(old);
        }
    }

    /// Java `AutoEndTask.run()` -> `endAuction()`: waits for the halls (3 s), then reads the
    /// auction again (D-50) before it decides.
    pub(crate) fn on_auction_auto_end(&mut self, key: u64) {
        self.clanhall.auction_timers.remove(&key);
        let Some(id) = self.auction_by_key(key).map(|a| a.id) else {
            return;
        };
        if !self.clanhall.loaded {
            // Task waiting ClanHallManager is loaded every 3s.
            self.schedule_auction_end(key, 3000);
            return;
        }
        self.request_auction_reread(id, ResidenceReadThen::AuctionEnd(key));
    }

    /// Asks the persistence task for the `auction` row and bids of a hall's auction; the answer
    /// goes on with `then` (D-50).
    pub(crate) fn request_auction_reread(&mut self, auction_id: i32, then: ResidenceReadThen) {
        self.outbox.persist(PersistCommand::ResidenceRereadRequest {
            residence_id: auction_id,
            kind: ResidenceReadKind::Auction,
            then,
        });
    }

    /// A failed read: an auction whose end could not be decided is tried again in a minute.
    pub(crate) fn auction_read_failed(&mut self, then: &ResidenceReadThen) {
        if let ResidenceReadThen::AuctionEnd(key) = then
            && self.auction_by_key(*key).is_some()
        {
            self.schedule_auction_end(*key, 60_000);
        }
    }

    /// The rows of an auction read again replace this world's copy (a missing row: the auction is
    /// gone), then the follow-up runs.
    pub(crate) fn apply_auction_rows(&mut self, auction_id: i32, record: Option<&AuctionRecord>) {
        let existing = self.clan_hall_auction(auction_id).map(|a| a.key);
        match (existing, record) {
            (Some(key), Some(record)) => {
                if let Some(auction) = self.auction_by_key_mut(key) {
                    auction.apply_record(record);
                }
            }
            (Some(key), None) => {
                self.remove_auction(key);
            }
            (None, Some(record)) => {
                self.add_auction_from_record(record);
            }
            (None, None) => {}
        }
    }

    /// The follow-up of a read that came back with rows.
    pub(crate) fn auction_read_then(&mut self, then: ResidenceReadThen) {
        match then {
            ResidenceReadThen::AuctionEnd(key) => self.auction_end_after_read(key),
            ResidenceReadThen::AuctionBid {
                player,
                auction_id,
                bid,
            } => self.auction_bid_after_read(player, auction_id, bid),
            ResidenceReadThen::AuctionCancelBid { player, auction_id } => {
                self.auction_cancel_bid_after_read(player, auction_id);
            }
            ResidenceReadThen::AuctionCancel { player, auction_id } => {
                self.auction_cancel_after_read(player, auction_id);
            }
            // Handled by `fort/` and `residence/npc/` before this is called.
            ResidenceReadThen::Nothing | ResidenceReadThen::Fort(_) | ResidenceReadThen::Npc(_) => {}
            // The Territory War's (M7-TW-01).
            ResidenceReadThen::Territory(_) => {}
            // The siege follow-ups are `on_residence_reread`'s own (M7-SIEGE-01).
            ResidenceReadThen::SiegeJoin { .. }
            | ResidenceReadThen::SiegeConfirmWaiting { .. }
            | ResidenceReadThen::SiegeAttackerList { .. }
            | ResidenceReadThen::SiegeDefenderList { .. }
            | ResidenceReadThen::SiegeRestartSchedule { .. } => {}
        }
    }

    /// The rows are in: Java `endAuction()` on the fresh auction.
    fn auction_end_after_read(&mut self, key: u64) {
        let Some(end_date) = self.auction_by_key(key).map(|a| a.end_date) else {
            // The Java game server ended it (or cancelled it) already.
            return;
        };
        let now = unix_ms_now();
        if end_date > now {
            // The Java game server moved the end on.
            self.schedule_auction_end(key, end_date - now);
            return;
        }
        self.end_auction(key);
    }

    // ------------------------------------------------------------------ the end

    /// Java `endAuction()` after the `loaded()` check.
    fn end_auction(&mut self, key: u64) {
        let Some(auction) = self.auction_by_key(key) else {
            return;
        };
        let (highest_id, seller_id) = (auction.highest_bidder_id, auction.seller_id);
        if highest_id == 0 && seller_id == 0 {
            self.start_auction_auto_task(key);
            return;
        }
        if highest_id == 0 && seller_id > 0 {
            // If seller haven't sell ClanHall, auction removed, THIS MUST BE CONFIRMED
            // NOTE(java-quirk): only the list entry goes; the `auction` row stays until the
            // next restart reads it again.
            self.remove_auction(key);
            return;
        }
        let Some(mut auction) = self.auction_by_key(key).cloned() else {
            return;
        };
        if seller_id > 0 {
            self.return_item(
                &auction.seller_clan_name,
                auction.highest_bidder_max_bid,
                true,
                Writes::Job,
            );
            // `getAuctionableHallById(_itemId).getLease()` throws for an unknown hall: the end
            // stops after the first refund.
            let Some(lease) = self
                .clan_hall(ClanHallId(auction.item_id))
                .filter(|h| !h.is_siegable_hall())
                .map(|h| h.lease())
            else {
                error!(
                    "ClanHallAuction: no auctionable hall {} to end the auction.",
                    auction.item_id
                );
                return;
            };
            self.return_item(&auction.seller_clan_name, i64::from(lease), false, Writes::Job);
        }
        self.delete_auction_from_db(key, auction.item_id, Writes::Job);
        // `_bidders.get(_highestBidderId).getClanName()` throws when the highest bidder is not
        // among the bidders.
        let Some(winner_name) = auction.bidders.get(&highest_id).map(|b| b.clan_name.clone()) else {
            error!("ClanHallAuction: the highest bidder {highest_id} is not a bidder.");
            return;
        };
        let Some(clan_id) = self.clan_by_name(&winner_name).map(|c| c.id) else {
            error!("ClanHallAuction: the winning clan {winner_name} does not exist.");
            return;
        };
        auction.bidders.remove(&highest_id);
        self.clan_set_auction_bidded_at(clan_id, 0, Some(Writes::Job));
        self.remove_bids(&mut auction, Writes::Job);
        self.clan_hall_table_set_owner(ClanHallId(auction.item_id), clan_id);
    }

    /// Java `deleteAuctionFromDB()`: out of the list, the row deleted.
    fn delete_auction_from_db(&mut self, key: u64, item_id: i32, writes: Writes) {
        self.remove_auction(key);
        self.auction_write(writes, AuctionWrite::AuctionDelete { item_id });
    }

    /// Java `ClanHallAuctionManager.getAuction(auctionId).deleteAuctionFromDB()` (the `//clanhallset`
    /// word of AdminSiege): the auction with this id leaves the list and its row is deleted (D-51:
    /// the row only with `SiegeDatabaseWrites`). Nothing happens without such an auction.
    pub(crate) fn clan_hall_delete_auction(&mut self, auction_id: i32) {
        let Some((key, item_id)) = self
            .clanhall
            .auctions
            .iter()
            .find(|a| a.id == auction_id)
            .map(|a| (a.key, a.item_id))
        else {
            return;
        };
        self.delete_auction_from_db(key, item_id, Writes::Job);
    }

    /// Java `removeBids()`: the bids are deleted; a clan without a hall gets 90% back, a clan that
    /// owns one hears that it won.
    fn remove_bids(&mut self, auction: &mut ClanHallAuction, writes: Writes) {
        self.auction_write(
            writes,
            AuctionWrite::BidsDelete {
                auction_id: auction.id,
            },
        );
        let bidders: Vec<Bidder> = auction.bidders.values().cloned().collect();
        for bidder in bidders {
            // `getClanByName(...)` is null for a clan that is gone: Java throws and stops here.
            let Some((clan_id, hideout)) = self.clan_by_name(&bidder.clan_name).map(|c| (c.id, c.hideout_id))
            else {
                error!(
                    "ClanHallAuction: the bidding clan {} does not exist.",
                    bidder.clan_name
                );
                return;
            };
            if hideout == 0 {
                self.return_item(&bidder.clan_name, bidder.bid, true, writes); // 10 % tax
            } else if let Some(player) = self.player_by_name(&bidder.name) {
                self.send_message(player, "Congratulation you have won ClanHall!");
            }
            self.clan_set_auction_bidded_at(clan_id, 0, Some(writes));
        }
        auction.bidders.clear();
    }

    /// Java `returnItem(clanName, quantity, penalty)`: adena back into the clan warehouse (with
    /// the 10% tax if `penalty`), never above `MaxAdena`.
    fn return_item(&mut self, clan_name: &str, quantity: i64, penalty: bool, writes: Writes) {
        let mut amount = quantity;
        if penalty {
            amount = (amount as f64 * 0.9) as i64; // take 10% tax fee if needed
        }
        let Some(clan_id) = self.clan_by_name(clan_name).map(|c| c.id) else {
            warn!("Clan {clan_name} doesn't exist!");
            return;
        };
        // avoid overflow on return
        let limit = self.settings.ground.max_adena - self.clan_warehouse_adena(clan_id);
        amount = amount.min(limit);
        self.clan_warehouse_add_adena(clan_id, amount, writes);
    }

    /// Java `ClanWarehouse.addItem(REFUND, ADENA_ID, amount, null, null)`: the adena stack grows
    /// or a new one is made. The row goes out as `writes` says.
    fn clan_warehouse_add_adena(&mut self, clan_id: i32, amount: i64, writes: Writes) {
        let max_adena = self.settings.ground.max_adena;
        let existing = self
            .clan_mut(clan_id)
            .and_then(|clan| clan.warehouse.iter_mut().find(|i| i.item_id == ADENA_ID))
            .map(|item| {
                if amount != 0 {
                    item.count = if item.count > max_adena.wrapping_sub(amount) {
                        max_adena
                    } else {
                        item.count.wrapping_add(amount)
                    };
                }
                item.clone()
            });
        let op = if let Some(item) = existing {
            ItemRowOp::Update {
                owner_id: clan_id,
                item,
            }
        } else {
            let Some(templates) = self.item_templates() else {
                warn!("No item templates: the adena cannot be returned to clan {clan_id}.");
                return;
            };
            let Some(template) = templates.get(ADENA_ID) else {
                warn!("Invalid ItemId ({ADENA_ID}) requested by REFUND");
                return;
            };
            let Some(object_id) = self.allocate_item_id() else {
                return;
            };
            let mut item = self.new_item_data(template, object_id, amount);
            item.loc = crate::warehouse::ContainerRef::Clan(clan_id)
                .base_location()
                .to_owned();
            item.loc_data = 0;
            item.enchant_level = template.default_enchant_level;
            if let Some(clan) = self.clan_mut(clan_id) {
                clan.warehouse.push(item.clone());
            }
            ItemRowOp::Insert {
                owner_id: clan_id,
                item,
            }
        };
        let emit = match writes {
            Writes::Player => true,
            Writes::Job => self.siege_database_writes(),
            Writes::Never => false,
        };
        if emit {
            self.outbox.persist(PersistCommand::ItemRows {
                tracked: None,
                ops: vec![op],
            });
        }
    }

    // ------------------------------------------------------------------ bidding

    /// Java `takeItem(bidder, quantity)`: the adena comes out of the clan warehouse.
    fn auction_take_item(&mut self, player: ObjectId, clan_id: i32, quantity: i64) -> bool {
        if self.clan_warehouse_adena(clan_id) >= quantity {
            // `destroyItemByItemId(BUY, ADENA_ID, quantity, bidder, bidder)`.
            self.clan_warehouse_destroy_adena(clan_id, quantity);
            return true;
        }
        self.send(
            player,
            &SystemMessage::new(SystemMessageId::ThereIsNotEnoughAdenaInTheClanHallWarehouse),
        );
        false
    }

    /// The auction is read again: next the clan warehouse of the bidder is read again (D-50).
    pub(crate) fn auction_bid_after_read(&mut self, player: ObjectId, auction_id: i32, bid: i64) {
        let clan_id = self.clan_id_of(player);
        if self.clan_hall_auction(auction_id).is_none() || clan_id == 0 {
            self.send_message(player, "Invalid bid!");
            return;
        }
        self.request_clan_window(player, clan_id, PendingClanWindow::AuctionBid { auction_id, bid });
    }

    /// Java `ClanHallAuction.setBid(bidder, bid)`, on fresh rows.
    pub(crate) fn auction_set_bid(&mut self, player: ObjectId, auction_id: i32, bid: i64) {
        let Some(clan) = self.clan_of(player) else {
            self.send_message(player, "Invalid bid!");
            return;
        };
        let (clan_id, leader_name) = (clan.id, leader_name_of(clan));
        let Some(auction) = self.clan_hall_auction(auction_id) else {
            self.send_message(player, "Invalid bid!");
            return;
        };
        let (highest_id, highest_name, highest_max, starting) = (
            auction.highest_bidder_id,
            auction.highest_bidder_name.clone(),
            auction.highest_bidder_max_bid,
            auction.starting_bid,
        );
        let required_adena = if highest_name == leader_name {
            bid - highest_max
        } else {
            bid
        };
        if ((highest_id > 0 && bid > highest_max) || (highest_id == 0 && bid >= starting))
            && self.auction_take_item(player, clan_id, required_adena)
        {
            self.auction_update_in_db(player, auction_id, bid);
            self.clan_set_auction_bidded_at(clan_id, auction_id, Some(Writes::Player));
            return;
        }
        if bid < starting || bid <= highest_max {
            self.send(
                player,
                &SystemMessage::new(
                    SystemMessageId::YourBidPriceMustBeHigherThanTheMinimumPriceCurrentlyBeingBid,
                ),
            );
        }
    }

    /// Java `updateInDB(bidder, bid)`.
    fn auction_update_in_db(&mut self, player: ObjectId, auction_id: i32, bid: i64) {
        let Some(clan) = self.clan_of(player) else {
            return;
        };
        let (clan_id, clan_name, leader_name) = (clan.id, clan.name.clone(), leader_name_of(clan));
        let player_name = self
            .players
            .get(&player)
            .map(|p| p.data.name.clone())
            .unwrap_or_default();
        let now = unix_ms_now();
        let Some(auction) = self.clan_hall_auction(auction_id) else {
            return;
        };
        let known_bidder = auction.bidders.contains_key(&clan_id);
        let outbid = auction.highest_bidder_name.clone();
        if known_bidder {
            self.auction_write(
                Writes::Player,
                AuctionWrite::BidUpdate {
                    auction_id,
                    bidder_id: clan_id,
                    bidder_name: leader_name.clone(),
                    max_bid: bid,
                    time_bid: now,
                },
            );
        } else {
            // `IdManager.getNextId()`.
            match self.allocate_item_id() {
                Some(id) => self.auction_write(
                    Writes::Player,
                    AuctionWrite::BidInsert {
                        id: id.0,
                        auction_id,
                        bidder_id: clan_id,
                        bidder_name: player_name,
                        max_bid: bid,
                        clan_name: clan_name.clone(),
                        time_bid: now,
                    },
                ),
                None => error!("ClanHallAuction: no id for the bid row of clan {clan_id}."),
            }
            if let Some(previous) = self.player_by_name(&outbid) {
                self.send_message(previous, "You have been out bidded");
            }
        }
        if let Some(auction) = self.clanhall.auctions.iter_mut().find(|a| a.id == auction_id) {
            auction.highest_bidder_id = clan_id;
            auction.highest_bidder_max_bid = bid;
            auction.highest_bidder_name.clone_from(&leader_name);
            match auction.bidders.get_mut(&clan_id) {
                None => {
                    auction.bidders.insert(
                        clan_id,
                        Bidder {
                            name: leader_name,
                            clan_name,
                            bid,
                            time_bid: now,
                        },
                    );
                }
                Some(bidder) => {
                    bidder.bid = bid;
                    bidder.time_bid = now;
                }
            }
        }
        self.send(
            player,
            &SystemMessage::new(SystemMessageId::YourBidHasBeenSuccessfullyPlaced),
        );
    }

    // ------------------------------------------------------------------ cancelling

    /// The auction is read again: next the clan warehouse of the bidder is read again (D-50).
    pub(crate) fn auction_cancel_bid_after_read(&mut self, player: ObjectId, auction_id: i32) {
        let clan_id = self.clan_id_of(player);
        if clan_id == 0 || self.clan_hall_auction(auction_id).is_none() {
            return;
        }
        self.request_clan_window(
            player,
            clan_id,
            PendingClanWindow::AuctionCancelBid { auction_id },
        );
    }

    /// `doCancelBid` on fresh rows: Java `cancelBid(clanId)` and the message.
    pub(crate) fn auction_cancel_bid(&mut self, player: ObjectId, auction_id: i32) {
        let clan_id = self.clan_id_of(player);
        if self.clan_hall_auction(auction_id).is_none() {
            return;
        }
        if self.auction_cancel_bid_of(auction_id, clan_id, Writes::Player) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouHaveCanceledYourBid),
            );
        }
    }

    /// Java `cancelBid(bidder)`: the bid row goes, the clan gets 90% of the bid back, the bids are
    /// read again (here: the bidder leaves the map and the highest bid is found again).
    fn auction_cancel_bid_of(&mut self, auction_id: i32, bidder: i32, writes: Writes) -> bool {
        self.auction_write(
            writes,
            AuctionWrite::BidDelete {
                auction_id,
                bidder_id: bidder,
            },
        );
        // `_bidders.get(bidder).getClanName()` throws for a clan that did not bid.
        let Some(entry) = self
            .clan_hall_auction(auction_id)
            .and_then(|a| a.bidders.get(&bidder))
            .cloned()
        else {
            return false;
        };
        self.return_item(&entry.clan_name, entry.bid, true, writes);
        if let Some(clan_id) = self.clan_by_name(&entry.clan_name).map(|c| c.id) {
            self.clan_set_auction_bidded_at(clan_id, 0, Some(writes));
        }
        if let Some(auction) = self.clanhall.auctions.iter_mut().find(|a| a.id == auction_id) {
            // `_bidders.clear(); loadBid();` with the row gone.
            let mut rows = auction.to_record().bidders;
            rows.retain(|b| b.bidder_id != bidder);
            auction.load_bids(&rows);
        }
        true
    }

    /// Java `ClanTable.destroyClan`: `auction.cancelBid(clan.getId())` for the clan's bid. The
    /// clan warehouse is destroyed right after, so nothing is given back; the bid row goes as the
    /// clan maintenance writes say (D-49).
    pub(crate) fn auction_cancel_bid_of_destroyed_clan(&mut self, clan_id: i32, write_db: bool) {
        let auction_id = self.clan(clan_id).map_or(0, |c| c.auction_bid_at);
        if self.clan_hall_auction(auction_id).is_none() {
            return;
        }
        let writes = if write_db { Writes::Player } else { Writes::Never };
        self.auction_write(
            writes,
            AuctionWrite::BidDelete {
                auction_id,
                bidder_id: clan_id,
            },
        );
        if let Some(auction) = self.clanhall.auctions.iter_mut().find(|a| a.id == auction_id) {
            let mut rows = auction.to_record().bidders;
            rows.retain(|b| b.bidder_id != clan_id);
            auction.load_bids(&rows);
        }
    }

    /// The auction is read again: the clan that sells it cancels it (Java `cancelAuction()`).
    pub(crate) fn auction_cancel_after_read(&mut self, player: ObjectId, auction_id: i32) {
        let Some(key) = self.clan_hall_auction(auction_id).map(|a| a.key) else {
            return;
        };
        let Some(mut auction) = self.auction_by_key(key).cloned() else {
            return;
        };
        self.delete_auction_from_db(key, auction.item_id, Writes::Player);
        self.remove_bids(&mut auction, Writes::Player);
        self.send_message(player, "Your auction has been canceled");
    }

    // ------------------------------------------------------------------ selling

    /// Java `confirmAuction()`: the auction joins the list and its row is written.
    pub(crate) fn auction_confirm(&mut self, pending: ClanHallAuction) {
        let record = pending.to_record();
        self.clanhall.next_auction_key += 1;
        let key = self.clanhall.next_auction_key;
        let mut auction = pending;
        auction.key = key;
        self.clanhall.auctions.push(auction);
        // NOTE(java-quirk): `confirmAuction` does not start the end task; the auction ends
        // after the next restart, which reads it and runs `startAutoTask`.
        self.auction_write(Writes::Player, AuctionWrite::AuctionInsert(Box::new(record)));
    }
}
