//! The rows the M7 scenarios start from. One description of them feeds both halves of every
//! scenario: the records the bare world loads (the way `persist::load_residences` and friends hand
//! them to the world at the start, or the way the session hands a clan to the world at
//! `CharacterSelect`) and the `INSERT` statements the real game server finds in its throwaway
//! database.
//!
//! The residences (castle, fortress, hall, auction) are in the database before the game server
//! starts. The clans come later, after the characters exist (their ids are made by the game
//! server): a clan the world does not know is read at `CharacterSelect` with the residences it
//! owns (D-50, D-51), which is the path this test wants to see.

use l2_world::clanhall::{AuctionBidRecord, AuctionRecord};
use l2_world::fort::{FortSiegeFortLoad, FortSiegeGuardRecord, FortSiegeLoad, FortSpawnRecord};
use l2_world::residence::{CastleRecord, ClanHallRecord, FortRecord, ResidenceLoad};
use l2_world::{ItemData, ObjectId, SiegeLoad};

use super::m7_data;
use super::{ADENA_ID, NAME_GM, NAME_LORD, NAME_RAIDER, Who};

/// The clan that owns Gludio from the start (the lord) and the clan that attacks (the raider).
pub const LIONS: i32 = 500;
pub const WOLVES: i32 = 600;
pub const LIONS_NAME: &str = "Lions";
pub const WOLVES_NAME: &str = "Wolves";
/// Clan level 5: the lowest that may register for a siege (`SiegeClanMinLevel`).
pub const CLAN_LEVEL: i32 = 5;
/// The reputation both clans start with (a castle changes it by thousands).
pub const CLAN_REPUTATION: i32 = 20_000;
/// The adena in the clan warehouse of the Wolves: auction bids and the fortress fee come out of it.
pub const WOLVES_WAREHOUSE_ADENA: i64 = 5_000_000;
/// The object id of that adena stack (a Java-range id, below the range of the Rust game server).
pub const WOLVES_WAREHOUSE_ITEM: i32 = 0x1000_0100;
/// The lord's mercenary ticket.
pub const LORD_TICKET_ITEM: i32 = 0x1000_0200;
/// The fortress and the auctionable hall of the scenarios.
pub const SHANTY: i32 = 101;
pub const SHANTY_NAME: &str = "Shanty";
pub const HALL: i32 = 22;
pub const HALL_NAME: &str = "Moonstone Hall";
/// The hall's rent and the starting bid of its auction.
pub const HALL_LEASE: i32 = 100_000;
pub const HALL_STARTING_BID: i64 = 200_000;
/// Fortress rows: the spawns that always stand, the commanders' places are in `FortSiege.ini`.
pub const FORT_FLAG_POLE: &str = "Shanty_flagpole";

/// What the scenarios start from; `now` is the clock at the moment the rows are made.
#[derive(Debug, Clone, Copy)]
pub struct Seed {
    pub now: i64,
    /// How long the hall auction runs from `now`.
    pub auction_ms: i64,
}

impl Seed {
    pub fn new(now: i64, auction_ms: i64) -> Self {
        Self { now, auction_ms }
    }

    /// The next siege of Gludio: ten days ahead, so nothing in the schedule fires while the
    /// scenario runs.
    pub fn siege_date(&self) -> i64 {
        self.now + 10 * m7_data::DAY_MS
    }

    /// The end of the registration period: nine days ahead (the registration is open).
    pub fn registration_end(&self) -> i64 {
        self.now + 9 * m7_data::DAY_MS
    }

    pub fn auction_end(&self) -> i64 {
        self.now + self.auction_ms
    }

    // ---- what the world reads at the start -------------------------------------------------------

    /// `persist::load_residences`: the castle, fortress and hall rows. No owners: the clans are
    /// not in the database yet.
    pub fn residences(&self) -> ResidenceLoad {
        ResidenceLoad {
            castles: vec![self.castle_record(0)],
            forts: vec![FortRecord {
                id: SHANTY,
                name: SHANTY_NAME.to_owned(),
                ..FortRecord::default()
            }],
            clan_halls: vec![self.hall_record(0)],
            siegable_halls: Vec::new(),
        }
    }

    /// The castle row of Gludio; `owner` is what `SELECT clan_id FROM clan_data WHERE hasCastle`
    /// says.
    pub fn castle_record(&self, owner: i32) -> CastleRecord {
        CastleRecord {
            id: m7_data::GLUDIO,
            found: true,
            name: "Gludio".to_owned(),
            tax_percent: m7_data::CASTLE_TAX,
            treasury: 1_000,
            siege_date: self.siege_date(),
            time_registration_over: false,
            time_registration_over_date: self.registration_end(),
            show_npc_crest: false,
            ticket_buy_count: 0,
            owner_id: owner,
            functions: Vec::new(),
            owner_blood_alliance_count: None,
        }
    }

    pub fn hall_record(&self, owner: i32) -> ClanHallRecord {
        ClanHallRecord {
            id: HALL,
            name: HALL_NAME.to_owned(),
            owner_id: owner,
            lease: HALL_LEASE,
            desc: "A hall in Gludio".to_owned(),
            location: "Gludio".to_owned(),
            paid_until: 0,
            grade: 1,
            paid: false,
            functions: Vec::new(),
        }
    }

    /// `persist::load_sieges`: no registrations, no guards and no hired mercenaries yet (the lord
    /// hires one in the scenario).
    pub fn sieges(&self) -> SiegeLoad {
        SiegeLoad::default()
    }

    /// `persist::load_clan_hall_auctions`: the hall's auction, started by the NPC.
    pub fn auctions(&self) -> Vec<AuctionRecord> {
        vec![self.auction_record()]
    }

    pub fn auction_record(&self) -> AuctionRecord {
        AuctionRecord {
            id: HALL,
            seller_id: 0,
            seller_name: "NPC".to_owned(),
            seller_clan_name: "NPC Clan".to_owned(),
            item_type: "ClanHall".to_owned(),
            item_id: HALL,
            item_object_id: 0,
            item_name: HALL_NAME.to_owned(),
            starting_bid: HALL_STARTING_BID,
            current_bid: 0,
            end_date: self.auction_end(),
            bidders: Vec::<AuctionBidRecord>::new(),
        }
    }

    /// `persist::load_fort_sieges`: the always-standing NPCs of the fortress and its guards.
    pub fn fort_sieges(&self) -> FortSiegeLoad {
        FortSiegeLoad {
            forts: vec![FortSiegeFortLoad {
                fort_id: SHANTY,
                spawns: self.fort_spawn_rows(),
                siege_guards: vec![self.fort_guard_row()],
                siege_clans: Vec::new(),
            }],
        }
    }

    /// `fort_spawnlist`: the foreman always stands there (type 0), the suspicious merchant is
    /// there before a siege (type 2).
    pub fn fort_spawn_rows(&self) -> Vec<FortSpawnRecord> {
        let row = |id: i32, npc_id: i32, pos: (i32, i32, i32), spawn_type: i32| FortSpawnRecord {
            id,
            npc_id,
            x: pos.0,
            y: pos.1,
            z: pos.2,
            heading: 0,
            spawn_type,
            castle_id: 0,
        };
        vec![
            row(1, m7_data::FORT_FOREMAN_ID, m7_data::FORT_FOREMAN_POS, 0),
            row(2, m7_data::FORT_MERCHANT_ID, m7_data::FORT_MERCHANT_POS, 2),
        ]
    }

    pub fn fort_guard_row(&self) -> FortSiegeGuardRecord {
        FortSiegeGuardRecord {
            npc_id: m7_data::FORT_GUARD_ID,
            x: m7_data::FORT_GUARD_POS.0,
            y: m7_data::FORT_GUARD_POS.1,
            z: m7_data::FORT_GUARD_POS.2,
            heading: 0,
            respawn_delay: 600,
        }
    }

    // ---- the clans (later) -----------------------------------------------------------------------

    /// The clans and who belongs to them: `(clan id, name, owns Gludio, leader)`.
    pub fn clans() -> [(i32, &'static str, bool, Who); 2] {
        [
            (LIONS, LIONS_NAME, true, Who::Lord),
            (WOLVES, WOLVES_NAME, false, Who::Raider),
        ]
    }

    pub fn clan_of(who: Who) -> Option<i32> {
        match who {
            Who::Lord => Some(LIONS),
            Who::Raider => Some(WOLVES),
            Who::Gm => None,
        }
    }

    pub fn name_of(who: Who) -> &'static str {
        match who {
            Who::Lord => NAME_LORD,
            Who::Raider => NAME_RAIDER,
            Who::Gm => NAME_GM,
        }
    }

    /// The adena stack in the clan warehouse of the Wolves.
    pub fn warehouse_adena() -> ItemData {
        ItemData {
            object_id: ObjectId(WOLVES_WAREHOUSE_ITEM),
            item_id: ADENA_ID,
            count: WOLVES_WAREHOUSE_ADENA,
            loc: "CLANWH".to_owned(),
            loc_data: 0,
            mana_left: -1,
            time: -1,
            ..Default::default()
        }
    }

    /// The mercenary ticket the lord starts with (an inventory item, in the throwaway database a
    /// row of `items` with a Java-range object id).
    pub fn lord_ticket() -> ItemData {
        ItemData {
            object_id: ObjectId(LORD_TICKET_ITEM),
            item_id: m7_data::MERC_TICKET,
            count: 1,
            loc: "INVENTORY".to_owned(),
            loc_data: 0,
            mana_left: -1,
            time: -1,
            ..Default::default()
        }
    }

    // ---- the statements ----------------------------------------------------------------------------

    /// What is in the database before the game server starts.
    pub fn startup_sql(&self) -> Vec<String> {
        let mut sql = vec![
            format!(
                "INSERT INTO castle (id, name, taxPercent, treasury, siegeDate, regTimeOver, regTimeEnd, showNpcCrest, ticketBuyCount) \
                 VALUES ({}, 'Gludio', {}, 1000, {}, 'false', {}, 'false', 0)",
                m7_data::GLUDIO,
                m7_data::CASTLE_TAX,
                self.siege_date(),
                self.registration_end()
            ),
            format!(
                "INSERT INTO fort (id, name, siegeDate, lastOwnedTime, owner, fortType, state, castleId, supplyLvL) \
                 VALUES ({SHANTY}, '{SHANTY_NAME}', 0, 0, 0, 0, 0, 0, 0)"
            ),
            format!(
                "INSERT INTO clanhall (id, name, ownerId, lease, `desc`, location, paidUntil, Grade, paid) \
                 VALUES ({HALL}, '{HALL_NAME}', 0, {HALL_LEASE}, 'A hall in Gludio', 'Gludio', 0, 1, 0)"
            ),
            format!(
                "INSERT INTO auction (id, sellerId, sellerName, sellerClanName, itemType, itemId, itemObjectId, itemName, \
                 itemQuantity, startingBid, currentBid, endDate) \
                 VALUES ({HALL}, 0, 'NPC', 'NPC Clan', 'ClanHall', {HALL}, 0, '{HALL_NAME}', 1, {HALL_STARTING_BID}, 0, {})",
                self.auction_end()
            ),
        ];
        for row in self.fort_spawn_rows() {
            sql.push(format!(
                "INSERT INTO fort_spawnlist (fortId, npcId, x, y, z, heading, spawnType, castleId) \
                 VALUES ({SHANTY}, {}, {}, {}, {}, {}, {}, {})",
                row.npc_id, row.x, row.y, row.z, row.heading, row.spawn_type, row.castle_id
            ));
        }
        let guard = self.fort_guard_row();
        sql.push(format!(
            "INSERT INTO fort_siege_guards (fortId, npcId, x, y, z, heading, respawnDelay, isHired) \
             VALUES ({SHANTY}, {}, {}, {}, {}, {}, {}, 0)",
            guard.npc_id, guard.x, guard.y, guard.z, guard.heading, guard.respawn_delay
        ));
        sql
    }

    /// What the scenario adds after the characters exist: the clans, the memberships, the
    /// warehouse of the Wolves, the GM's access level. `ids` are the characters' object ids.
    pub fn late_sql(&self, ids: [i32; 3]) -> Vec<String> {
        let mut sql = Vec::new();
        for (clan, name, owns_castle, leader) in Self::clans() {
            let leader_id = ids[leader.index()];
            sql.push(format!(
                "INSERT INTO clan_data (clan_id, clan_name, clan_level, reputation_score, hasCastle, leader_id, crest_id, crest_large_id, ally_crest_id) \
                 VALUES ({clan}, '{name}', {CLAN_LEVEL}, {CLAN_REPUTATION}, {}, {leader_id}, 0, 0, 0)",
                u8::from(owns_castle)
            ));
            sql.push(format!(
                "UPDATE characters SET clanid = {clan}, power_grade = 1, clan_privs = {} WHERE charId = {leader_id}",
                0x00FF_FFFF
            ));
        }
        sql.push(format!(
            "INSERT INTO items (owner_id, object_id, item_id, `count`, enchant_level, loc, loc_data, mana_left, `time`) \
             VALUES ({WOLVES}, {WOLVES_WAREHOUSE_ITEM}, {ADENA_ID}, {WOLVES_WAREHOUSE_ADENA}, 0, 'CLANWH', 0, -1, 0)"
        ));
        sql.push(format!(
            "INSERT INTO items (owner_id, object_id, item_id, `count`, enchant_level, loc, loc_data, mana_left, `time`) \
             VALUES ({}, {LORD_TICKET_ITEM}, {}, 1, 0, 'INVENTORY', 0, -1, 0)",
            ids[Who::Lord.index()],
            m7_data::MERC_TICKET
        ));
        sql.push(format!(
            "UPDATE characters SET accesslevel = {} WHERE charId = {}",
            m7_data::GM_LEVEL,
            ids[Who::Gm.index()]
        ));
        sql
    }
}
