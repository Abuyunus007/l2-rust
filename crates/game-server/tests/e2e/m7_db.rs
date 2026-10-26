//! The tables of the M7 scenarios as the bare world sees them: a model of what the persistence
//! task of the real game server leaves in the database. It starts from the same [`Seed`] the
//! throwaway database gets, applies the commands the world sent (`PersistCommand::Residence`,
//! `Clan`, `ItemRows`, ...) and answers the reads the world asks for
//! (`ResidenceRereadRequest`, `ClanWarehouseRequest`), so the bare world runs the D-50 steps like
//! the real servers do. `Rows` is what a scenario compares: the same struct comes out of the model
//! and out of `SELECT`s on the real database.
//!
//! The model is strict: a write it does not know panics, so a new statement of the world cannot
//! slip past the checks.

use std::collections::BTreeMap;

use l2_world::clan::{ClanDataRecord, ClanMemberRecord, ClanWrite};
use l2_world::clanhall::{AuctionBidRecord, AuctionRecord, AuctionWrite};
use l2_world::fort::{FortSiegeRows, FortSiegeWrite};
use l2_world::residence::{
    CastleRecord, ClanHallRecord, FortRecord, FunctionRecord, ResidenceReadKind, ResidenceRefresh,
    ResidenceRereadRows,
};
use l2_world::{
    ClanRecord, ItemData, ItemRowOp, PersistCommand, ResidenceRef, ResidenceWrite, SiegeClanRecord,
};

use super::m7_seed::{self, Seed};

/// One row of `castle`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastleRow {
    pub id: i32,
    pub tax_percent: i32,
    pub treasury: i64,
    pub siege_date: i64,
    pub reg_time_over: bool,
    pub reg_time_end: i64,
    pub show_npc_crest: bool,
    pub ticket_buy_count: i32,
}

/// The columns of `clan_data` the scenarios look at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClanDbRow {
    pub clan_id: i32,
    pub has_castle: i32,
    pub reputation: i32,
    pub blood_alliance_count: i32,
    pub blood_oath_count: i32,
    pub auction_bid_at: i32,
}

/// One row of `fort`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FortRow {
    pub id: i32,
    pub siege_date: i64,
    pub last_owned_time: i64,
    pub owner: i32,
    pub state: i32,
    pub castle_id: i32,
    pub supply_level: i32,
}

/// One row of `clanhall`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HallRow {
    pub id: i32,
    pub owner_id: i32,
    pub paid_until: i64,
    pub paid: bool,
}

/// One row of `castle_functions`, `fort_functions` or `clanhall_functions`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionRow {
    /// `castle`, `fort` or `clanhall`.
    pub table: &'static str,
    pub residence_id: i32,
    pub function_type: i32,
    pub lvl: i32,
    pub lease: i32,
    pub rate: i64,
    pub end_time: i64,
}

/// One row of `auction`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuctionRow {
    pub id: i32,
    pub seller_id: i32,
    pub starting_bid: i64,
    pub current_bid: i64,
    pub end_date: i64,
}

/// One row of `auction_bid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BidRow {
    pub auction_id: i32,
    pub bidder_id: i32,
    pub bidder_name: String,
    pub clan_name: String,
    pub max_bid: i64,
}

/// One row of `castle_siege_guards`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardRow {
    pub castle_id: i32,
    pub npc_id: i32,
    pub hired: bool,
}

/// The adena of one clan warehouse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClanAdena {
    pub clan_id: i32,
    pub adena: i64,
}

/// Everything the M7 scenarios compare, sorted so two instances are equal when the tables are.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Rows {
    pub castles: Vec<CastleRow>,
    pub clans: Vec<ClanDbRow>,
    /// `(castle_id, clan_id, type)`.
    pub siege_clans: Vec<(i32, i32, i32)>,
    pub guards: Vec<GuardRow>,
    pub forts: Vec<FortRow>,
    /// `(fort_id, clan_id)`.
    pub fort_clans: Vec<(i32, i32)>,
    pub halls: Vec<HallRow>,
    pub functions: Vec<FunctionRow>,
    pub auctions: Vec<AuctionRow>,
    pub bids: Vec<BidRow>,
    pub clan_adena: Vec<ClanAdena>,
}

impl Rows {
    pub fn castle(&self, id: i32) -> &CastleRow {
        self.castles.iter().find(|c| c.id == id).expect("a castle row")
    }

    pub fn clan(&self, id: i32) -> &ClanDbRow {
        self.clans.iter().find(|c| c.clan_id == id).expect("a clan row")
    }

    pub fn fort(&self, id: i32) -> &FortRow {
        self.forts.iter().find(|f| f.id == id).expect("a fort row")
    }

    pub fn hall(&self, id: i32) -> &HallRow {
        self.halls.iter().find(|h| h.id == id).expect("a hall row")
    }

    pub fn adena_of(&self, clan_id: i32) -> i64 {
        self.clan_adena
            .iter()
            .find(|c| c.clan_id == clan_id)
            .map_or(0, |c| c.adena)
    }
}

/// The model.
#[derive(Debug, Clone, Default)]
pub struct Db {
    castles: BTreeMap<i32, CastleRow>,
    clans: BTreeMap<i32, ClanDbRow>,
    siege_clans: Vec<(i32, i32, i32)>,
    guards: Vec<(i32, i32, bool)>,
    forts: BTreeMap<i32, FortRow>,
    fort_clans: Vec<(i32, i32)>,
    halls: BTreeMap<i32, HallRow>,
    hall_static: BTreeMap<i32, ClanHallRecord>,
    functions: BTreeMap<(&'static str, i32, i32), FunctionRow>,
    auctions: BTreeMap<i32, AuctionRecord>,
    items: BTreeMap<i32, (i32, ItemData)>,
}

/// The table name of a residence's function table.
fn function_table(residence: ResidenceRef) -> &'static str {
    match residence {
        ResidenceRef::Castle(_) => "castle",
        ResidenceRef::Fort(_) => "fort",
        ResidenceRef::ClanHall(_) => "clanhall",
    }
}

impl Db {
    /// The database before the game server starts, then the clans the scenario adds later
    /// (`Seed::late_sql`).
    pub fn seeded(seed: &Seed) -> Self {
        let mut db = Self::default();
        let castle = seed.castle_record(0);
        db.castles.insert(
            castle.id,
            CastleRow {
                id: castle.id,
                tax_percent: castle.tax_percent,
                treasury: castle.treasury,
                siege_date: castle.siege_date,
                reg_time_over: castle.time_registration_over,
                reg_time_end: castle.time_registration_over_date,
                show_npc_crest: castle.show_npc_crest,
                ticket_buy_count: castle.ticket_buy_count,
            },
        );
        db.forts.insert(
            m7_seed::SHANTY,
            FortRow {
                id: m7_seed::SHANTY,
                siege_date: 0,
                last_owned_time: 0,
                owner: 0,
                state: 0,
                castle_id: 0,
                supply_level: 0,
            },
        );
        let hall = seed.hall_record(0);
        db.halls.insert(
            hall.id,
            HallRow {
                id: hall.id,
                owner_id: 0,
                paid_until: 0,
                paid: false,
            },
        );
        db.hall_static.insert(hall.id, hall);
        db.auctions.insert(m7_seed::HALL, seed.auction_record());
        for (clan, _, owns_castle, _) in Seed::clans() {
            db.clans.insert(
                clan,
                ClanDbRow {
                    clan_id: clan,
                    has_castle: i32::from(owns_castle),
                    reputation: m7_seed::CLAN_REPUTATION,
                    blood_alliance_count: 0,
                    blood_oath_count: 0,
                    auction_bid_at: 0,
                },
            );
        }
        let adena = Seed::warehouse_adena();
        db.items.insert(adena.object_id.0, (m7_seed::WOLVES, adena));
        db
    }

    /// The tables after `commands`.
    pub fn replay(seed: &Seed, commands: &[PersistCommand]) -> Self {
        let mut db = Self::seeded(seed);
        for command in commands {
            db.apply(command);
        }
        db
    }

    fn apply(&mut self, command: &PersistCommand) {
        match command {
            PersistCommand::Residence(write) => self.apply_residence(write),
            PersistCommand::Clan(write) => self.apply_clan(write),
            PersistCommand::ItemRows { ops, .. } => {
                for op in ops {
                    match op {
                        ItemRowOp::Write { owner_id, item }
                        | ItemRowOp::Update { owner_id, item }
                        | ItemRowOp::Insert { owner_id, item } => {
                            self.items.insert(item.object_id.0, (*owner_id, item.clone()));
                        }
                        ItemRowOp::Delete { object_id } => {
                            self.items.remove(&object_id.0);
                        }
                    }
                }
            }
            PersistCommand::ItemChanges {
                owner,
                upserts,
                removed,
            } => {
                for item in upserts {
                    self.items.insert(item.object_id.0, (owner.0, item.clone()));
                }
                for id in removed {
                    self.items.remove(&id.0);
                }
            }
            PersistCommand::StorePlayer(data) => {
                // A full store replaces the character's inventory and worn rows.
                let owner = data.object_id.0;
                self.items.retain(|_, (o, item)| {
                    !(*o == owner && matches!(item.loc.as_str(), "INVENTORY" | "PAPERDOLL"))
                });
                for item in &data.items {
                    self.items.insert(item.object_id.0, (owner, item.clone()));
                }
            }
            _ => {}
        }
    }

    fn apply_clan(&mut self, write: &ClanWrite) {
        // The other clan rows (levels, crests, members of the scenarios of M6) are not looked at
        // here.
        if let ClanWrite::Update(row) = write
            && let Some(clan) = self.clans.get_mut(&row.clan_id)
        {
            clan.reputation = row.reputation_score;
        }
    }

    fn apply_residence(&mut self, write: &ResidenceWrite) {
        match write {
            ResidenceWrite::CastleTreasury { castle_id, treasury } => {
                self.castle_mut(*castle_id).treasury = *treasury;
            }
            ResidenceWrite::CastleTaxPercent {
                castle_id,
                tax_percent,
            } => {
                self.castle_mut(*castle_id).tax_percent = *tax_percent;
            }
            ResidenceWrite::CastleShowNpcCrest { castle_id, show } => {
                self.castle_mut(*castle_id).show_npc_crest = *show;
            }
            ResidenceWrite::CastleTicketBuyCount { castle_id, count } => {
                self.castle_mut(*castle_id).ticket_buy_count = *count;
            }
            ResidenceWrite::CastleOwner { castle_id, clan_id } => {
                for clan in self.clans.values_mut() {
                    if clan.has_castle == *castle_id {
                        clan.has_castle = 0;
                    }
                }
                if let Some(clan) = self.clans.get_mut(clan_id) {
                    clan.has_castle = *castle_id;
                }
            }
            ResidenceWrite::FortOwner {
                fort_id,
                owner,
                last_owned_time,
            } => {
                let fort = self.fort_mut(*fort_id);
                fort.owner = *owner;
                fort.last_owned_time = *last_owned_time;
                fort.state = 0;
                fort.castle_id = 0;
            }
            ResidenceWrite::FortState {
                fort_id,
                state,
                castle_id,
            } => {
                let fort = self.fort_mut(*fort_id);
                fort.state = *state;
                fort.castle_id = *castle_id;
            }
            ResidenceWrite::FortSupplyLevel {
                fort_id,
                supply_level,
            } => {
                self.fort_mut(*fort_id).supply_level = *supply_level;
            }
            ResidenceWrite::ClanHall {
                hall_id,
                owner_id,
                paid_until,
                paid,
            } => {
                let hall = self.halls.get_mut(hall_id).expect("a hall row");
                hall.owner_id = *owner_id;
                hall.paid_until = *paid_until;
                hall.paid = *paid;
            }
            ResidenceWrite::FunctionSave {
                residence,
                function_type,
                lvl,
                lease,
                rate,
                end_time,
            } => {
                let table = function_table(*residence);
                let key = (table, residence.residence_id(), *function_type);
                self.functions.insert(
                    key,
                    FunctionRow {
                        table,
                        residence_id: residence.residence_id(),
                        function_type: *function_type,
                        lvl: *lvl,
                        lease: *lease,
                        rate: *rate,
                        end_time: *end_time,
                    },
                );
            }
            ResidenceWrite::FunctionDelete {
                residence,
                function_type,
            } => {
                self.functions.remove(&(
                    function_table(*residence),
                    residence.residence_id(),
                    *function_type,
                ));
            }
            ResidenceWrite::CastleSiegeDate {
                castle_id,
                siege_date,
                reg_time_end,
                reg_time_over,
            } => {
                let castle = self.castle_mut(*castle_id);
                castle.siege_date = *siege_date;
                castle.reg_time_end = *reg_time_end;
                castle.reg_time_over = *reg_time_over;
            }
            ResidenceWrite::SiegeClanInsert {
                clan_id,
                castle_id,
                kind,
            } => {
                self.siege_clans.push((*castle_id, *clan_id, *kind));
            }
            ResidenceWrite::SiegeClanUpdate {
                castle_id,
                clan_id,
                kind,
            } => {
                for row in &mut self.siege_clans {
                    if row.0 == *castle_id && row.1 == *clan_id {
                        row.2 = *kind;
                    }
                }
            }
            ResidenceWrite::SiegeClanDelete { castle_id, clan_id } => {
                self.siege_clans
                    .retain(|r| !(r.0 == *castle_id && r.1 == *clan_id));
            }
            ResidenceWrite::SiegeClansDeleteCastle { castle_id } => {
                self.siege_clans.retain(|r| r.0 != *castle_id);
            }
            ResidenceWrite::SiegeClansDeleteClan { clan_id } => {
                self.siege_clans.retain(|r| r.1 != *clan_id);
            }
            ResidenceWrite::SiegeClansDeleteWaiting { castle_id } => {
                self.siege_clans.retain(|r| !(r.0 == *castle_id && r.2 == 2));
            }
            ResidenceWrite::SiegeGuardInsert(record) => {
                self.guards
                    .push((record.castle_id, record.npc_id, record.is_hired));
            }
            ResidenceWrite::SiegeGuardDeleteHired { npc_id, .. } => {
                if let Some(index) = self.guards.iter().position(|g| g.1 == *npc_id && g.2) {
                    self.guards.remove(index);
                }
            }
            ResidenceWrite::SiegeGuardsDeleteHired { castle_id } => {
                self.guards.retain(|g| !(g.0 == *castle_id && g.2));
            }
            ResidenceWrite::ClanBloodAllianceCount { clan_id, count } => {
                if let Some(clan) = self.clans.get_mut(clan_id) {
                    clan.blood_alliance_count = *count;
                }
            }
            // The upgrades and the circlets are not tables the scenarios compare.
            ResidenceWrite::CastleDoorUpgrade { .. }
            | ResidenceWrite::CastleDoorUpgradesDelete { .. }
            | ResidenceWrite::CastleTrapUpgrade { .. }
            | ResidenceWrite::CastleTrapUpgradesDelete { .. }
            | ResidenceWrite::DeleteCirclets { .. } => {}
            ResidenceWrite::FortSiege(write) => self.apply_fort_siege(write),
            ResidenceWrite::ClanHallAuction(write) => self.apply_auction(write),
            other => panic!("the model of the tables does not know the write {other:?}"),
        }
    }

    fn apply_fort_siege(&mut self, write: &FortSiegeWrite) {
        match write {
            FortSiegeWrite::SiegeDate { fort_id, siege_date } => {
                self.fort_mut(*fort_id).siege_date = *siege_date;
            }
            FortSiegeWrite::AddAttacker { fort_id, clan_id } => {
                self.fort_clans.push((*fort_id, *clan_id));
            }
            FortSiegeWrite::RemoveAttackers { fort_id, clan_id } => {
                self.fort_clans
                    .retain(|r| !(r.0 == *fort_id && (*clan_id == 0 || r.1 == *clan_id)));
            }
            FortSiegeWrite::ClearSiegeClans {
                fort_id,
                owner_clan_id,
            } => {
                self.fort_clans
                    .retain(|r| !(r.0 == *fort_id || owner_clan_id.is_some_and(|c| r.1 == c)));
            }
            FortSiegeWrite::BloodOathCount { clan_id, count } => {
                if let Some(clan) = self.clans.get_mut(clan_id) {
                    clan.blood_oath_count = *count;
                }
            }
        }
    }

    fn apply_auction(&mut self, write: &AuctionWrite) {
        match write {
            AuctionWrite::EndDate { id, end_date } => {
                if let Some(auction) = self.auctions.get_mut(id) {
                    auction.end_date = *end_date;
                }
            }
            AuctionWrite::BidUpdate {
                auction_id,
                bidder_id,
                bidder_name,
                max_bid,
                time_bid,
            } => {
                if let Some(bid) = self
                    .auctions
                    .get_mut(auction_id)
                    .and_then(|a| a.bidders.iter_mut().find(|b| b.bidder_id == *bidder_id))
                {
                    bid.bidder_name.clone_from(bidder_name);
                    bid.max_bid = *max_bid;
                    bid.time_bid = *time_bid;
                }
            }
            AuctionWrite::BidInsert {
                auction_id,
                bidder_id,
                bidder_name,
                max_bid,
                clan_name,
                time_bid,
                ..
            } => {
                if let Some(auction) = self.auctions.get_mut(auction_id) {
                    auction.bidders.push(AuctionBidRecord {
                        bidder_id: *bidder_id,
                        bidder_name: bidder_name.clone(),
                        max_bid: *max_bid,
                        clan_name: clan_name.clone(),
                        time_bid: *time_bid,
                    });
                }
            }
            AuctionWrite::BidsDelete { auction_id } => {
                if let Some(auction) = self.auctions.get_mut(auction_id) {
                    auction.bidders.clear();
                }
            }
            AuctionWrite::AuctionDelete { item_id } => {
                self.auctions.retain(|_, a| a.item_id != *item_id);
            }
            AuctionWrite::BidDelete {
                auction_id,
                bidder_id,
            } => {
                if let Some(auction) = self.auctions.get_mut(auction_id) {
                    auction.bidders.retain(|b| b.bidder_id != *bidder_id);
                }
            }
            AuctionWrite::AuctionInsert(record) => {
                self.auctions.insert(record.id, (**record).clone());
            }
            AuctionWrite::NpcAuctionInsert {
                hall_id,
                name,
                starting_bid,
                end_date,
            } => {
                self.auctions.insert(
                    *hall_id,
                    AuctionRecord {
                        id: *hall_id,
                        seller_id: 0,
                        seller_name: "NPC".to_owned(),
                        seller_clan_name: "NPC Clan".to_owned(),
                        item_type: "ClanHall".to_owned(),
                        item_id: *hall_id,
                        item_object_id: 0,
                        item_name: name.clone(),
                        starting_bid: *starting_bid,
                        current_bid: 0,
                        end_date: *end_date,
                        bidders: Vec::new(),
                    },
                );
            }
            AuctionWrite::ClanBidAt { clan_id, auction_id } => {
                if let Some(clan) = self.clans.get_mut(clan_id) {
                    clan.auction_bid_at = *auction_id;
                }
            }
        }
    }

    fn castle_mut(&mut self, id: i32) -> &mut CastleRow {
        self.castles.get_mut(&id).expect("a castle row")
    }

    fn fort_mut(&mut self, id: i32) -> &mut FortRow {
        self.forts.get_mut(&id).expect("a fort row")
    }

    // ---- reads ---------------------------------------------------------------------------------

    /// The functions of a residence as `function_records` hands them to the world.
    fn function_records(&self, table: &'static str, residence_id: i32) -> Vec<FunctionRecord> {
        self.functions
            .values()
            .filter(|f| f.table == table && f.residence_id == residence_id)
            .map(|f| FunctionRecord {
                function_type: f.function_type,
                lvl: f.lvl,
                lease: f.lease,
                rate: f.rate,
                end_time: f.end_time,
            })
            .collect()
    }

    /// `read_castle_record` of the game server.
    pub fn castle_record(&self, id: i32) -> CastleRecord {
        let mut record = CastleRecord {
            id,
            time_registration_over: true,
            ..CastleRecord::default()
        };
        if let Some(row) = self.castles.get(&id) {
            record.found = true;
            record.name = "Gludio".to_owned();
            record.tax_percent = row.tax_percent;
            record.treasury = row.treasury;
            record.siege_date = row.siege_date;
            record.time_registration_over = row.reg_time_over;
            record.time_registration_over_date = row.reg_time_end;
            record.show_npc_crest = row.show_npc_crest;
            record.ticket_buy_count = row.ticket_buy_count;
        }
        record.owner_id = self
            .clans
            .values()
            .filter(|c| c.has_castle == id)
            .map(|c| c.clan_id)
            .next_back()
            .unwrap_or(0);
        if record.owner_id != 0 {
            record.functions = self.function_records("castle", id);
            record.owner_blood_alliance_count =
                self.clans.get(&record.owner_id).map(|c| c.blood_alliance_count);
        }
        record
    }

    fn fort_record(&self, id: i32) -> Option<FortRecord> {
        let row = self.forts.get(&id)?;
        Some(FortRecord {
            id,
            name: m7_seed::SHANTY_NAME.to_owned(),
            siege_date: row.siege_date,
            last_owned_time: row.last_owned_time,
            owner_id: row.owner,
            fort_type: 0,
            state: row.state,
            castle_id: row.castle_id,
            supply_level: row.supply_level,
            functions: if row.owner > 0 {
                self.function_records("fort", id)
            } else {
                Vec::new()
            },
        })
    }

    fn hall_record(&self, id: i32) -> Option<ClanHallRecord> {
        let row = self.halls.get(&id)?;
        let mut record = self.hall_static.get(&id)?.clone();
        record.owner_id = row.owner_id;
        record.paid_until = row.paid_until;
        record.paid = row.paid;
        record.functions = if row.owner_id != 0 {
            self.function_records("clanhall", id)
        } else {
            Vec::new()
        };
        Some(record)
    }

    /// The answer of the persistence task to a `ResidenceRereadRequest`.
    pub fn reread(&self, kind: ResidenceReadKind, id: i32) -> ResidenceRereadRows {
        match kind {
            ResidenceReadKind::Castle => ResidenceRereadRows::Castle(self.castle_record(id)),
            ResidenceReadKind::Fort => self
                .fort_record(id)
                .map_or(ResidenceRereadRows::Missing, ResidenceRereadRows::Fort),
            ResidenceReadKind::FortSiege => match self.fort_record(id) {
                Some(fort) => {
                    let owner = fort.owner_id;
                    ResidenceRereadRows::FortSiege(Box::new(FortSiegeRows {
                        fort,
                        clans: self
                            .fort_clans
                            .iter()
                            .filter(|r| r.0 == id)
                            .map(|r| r.1)
                            .collect(),
                        owner_blood_oath_count: (owner > 0)
                            .then(|| self.clans.get(&owner).map(|c| c.blood_oath_count))
                            .flatten(),
                    }))
                }
                None => ResidenceRereadRows::Missing,
            },
            ResidenceReadKind::ClanHall => self
                .hall_record(id)
                .map_or(ResidenceRereadRows::Missing, ResidenceRereadRows::ClanHall),
            ResidenceReadKind::Auction => ResidenceRereadRows::Auction {
                id,
                record: self.auctions.get(&id).map(|a| {
                    let mut record = a.clone();
                    // The highest bid first (`ORDER BY maxBid DESC`).
                    record.bidders.sort_by_key(|b| std::cmp::Reverse(b.max_bid));
                    record
                }),
            },
            // Every auction row (M7-FU-03: the auctioneer's list), the highest bid first.
            ResidenceReadKind::Auctions => ResidenceRereadRows::Auctions(
                self.auctions
                    .values()
                    .map(|a| {
                        let mut record = a.clone();
                        record.bidders.sort_by_key(|b| std::cmp::Reverse(b.max_bid));
                        record
                    })
                    .collect(),
            ),
            // No conquerable hall in the seed.
            ResidenceReadKind::SiegableHall | ResidenceReadKind::HallSiege => ResidenceRereadRows::Missing,
            // No territory in the seed (M7-TW-01).
            ResidenceReadKind::Territory => ResidenceRereadRows::Territory(Box::default()),
            ResidenceReadKind::SiegeClans => {
                let castle = self.castle_record(id);
                ResidenceRereadRows::SiegeClans {
                    castle: castle.found.then_some(castle),
                    rows: self
                        .siege_clans
                        .iter()
                        .map(|r| SiegeClanRecord {
                            castle_id: r.0,
                            clan_id: r.1,
                            kind: r.2,
                        })
                        .collect(),
                }
            }
        }
    }

    /// `persist::load_clan` of a clan, as the session reads it at `CharacterSelect`. `ids` are the
    /// object ids of the characters.
    pub fn clan_record(&self, clan: i32, ids: [i32; 3]) -> ClanRecord {
        let row = self.clans.get(&clan).expect("a clan row");
        let (_, name, _, leader) = Seed::clans().into_iter().find(|c| c.0 == clan).expect("a clan");
        let leader_id = ids[leader.index()];
        ClanRecord {
            clan_id: clan,
            data: Some(ClanDataRecord {
                name: Some(name.to_owned()),
                level: m7_seed::CLAN_LEVEL,
                reputation_score: row.reputation,
                castle_id: row.has_castle,
                blood_alliance_count: row.blood_alliance_count,
                blood_oath_count: row.blood_oath_count,
                leader_id,
                auction_bid_at: row.auction_bid_at,
                ..ClanDataRecord::default()
            }),
            members: vec![ClanMemberRecord {
                object_id: leader_id,
                name: Seed::name_of(leader).to_owned(),
                level: super::m6_data::LEADER_LEVEL,
                power_grade: 1,
                ..ClanMemberRecord::default()
            }],
            warehouse: self.clan_warehouse(clan),
            ..ClanRecord::default()
        }
    }

    /// `persist::load_residence_refresh` of a clan.
    pub fn refresh(&self, clan: i32) -> ResidenceRefresh {
        let row = self.clans.get(&clan).expect("a clan row");
        ResidenceRefresh {
            clan_id: clan,
            castle_id: row.has_castle,
            castle: (row.has_castle > 0).then(|| self.castle_record(row.has_castle)),
            forts: self
                .forts
                .values()
                .filter(|f| f.owner == clan)
                .filter_map(|f| self.fort_record(f.id))
                .collect(),
            clan_halls: self
                .halls
                .values()
                .filter(|h| h.owner_id == clan)
                .filter_map(|h| self.hall_record(h.id))
                .collect(),
            siegable_halls: Vec::new(),
        }
    }

    /// The items of the clan warehouse of `clan_id` (`CLANWH`).
    pub fn clan_warehouse(&self, clan_id: i32) -> Vec<ItemData> {
        self.items
            .values()
            .filter(|(owner, item)| *owner == clan_id && item.loc == "CLANWH")
            .map(|(_, item)| item.clone())
            .collect()
    }

    /// The items a player owns, by object id.
    pub fn items_of(&self, owner: i32) -> Vec<ItemData> {
        self.items
            .values()
            .filter(|(o, _)| *o == owner)
            .map(|(_, item)| item.clone())
            .collect()
    }

    /// The tables, sorted.
    pub fn rows(&self) -> Rows {
        let mut clan_adena: Vec<ClanAdena> = self
            .clans
            .keys()
            .map(|clan_id| ClanAdena {
                clan_id: *clan_id,
                adena: self
                    .clan_warehouse(*clan_id)
                    .iter()
                    .filter(|i| i.item_id == super::ADENA_ID)
                    .map(|i| i.count)
                    .sum(),
            })
            .collect();
        clan_adena.sort_by_key(|c| c.clan_id);
        let mut guards: Vec<GuardRow> = self
            .guards
            .iter()
            .map(|g| GuardRow {
                castle_id: g.0,
                npc_id: g.1,
                hired: g.2,
            })
            .collect();
        guards.sort_by_key(|g| (g.castle_id, g.npc_id, g.hired));
        let mut siege_clans = self.siege_clans.clone();
        siege_clans.sort_unstable();
        let mut fort_clans = self.fort_clans.clone();
        fort_clans.sort_unstable();
        let mut bids: Vec<BidRow> = self
            .auctions
            .values()
            .flat_map(|a| {
                a.bidders.iter().map(|b| BidRow {
                    auction_id: a.id,
                    bidder_id: b.bidder_id,
                    bidder_name: b.bidder_name.clone(),
                    clan_name: b.clan_name.clone(),
                    max_bid: b.max_bid,
                })
            })
            .collect();
        bids.sort_by_key(|b| (b.auction_id, b.bidder_id));
        Rows {
            castles: self.castles.values().cloned().collect(),
            clans: self.clans.values().cloned().collect(),
            siege_clans,
            guards,
            forts: self.forts.values().cloned().collect(),
            fort_clans,
            halls: self.halls.values().cloned().collect(),
            functions: self.functions.values().cloned().collect(),
            auctions: self
                .auctions
                .values()
                .map(|a| AuctionRow {
                    id: a.id,
                    seller_id: a.seller_id,
                    starting_bid: a.starting_bid,
                    current_bid: a.current_bid,
                    end_date: a.end_date,
                })
                .collect(),
            bids,
            clan_adena,
        }
    }
}
