//! Reading residences again (D-50, D-51): the residences of a character's clan at
//! `CharacterSelect`, and any residence on demand before a card acts on it.
//
// Sieghardt (the Java GS) runs the real sieges, auctions and rents while this server runs, so an
// owner, a tax rate or a treasury in this world can be old. Two ways bring them up to date, both
// without waiting in a tick (D-15):
// - The session reads the residences of the character's clan at `CharacterSelect` (the clan's
//   `hasCastle` and that castle's row, the fortresses and halls whose owner is the clan) into
//   `PlayerProfile::residence_refresh`; `on_entered` applies it before the clan block of
//   `EnterWorld` hands out the residential skills.
// - A card that acts on a residence (a chamberlain page, the treasury, a registration) asks
//   [`World::request_residence_reread`]; the persistence task reads the rows and answers with
//   `WorldCommand::ResidenceReread`, and [`World::on_residence_reread`] applies them and goes on
//   with what the card named in [`ResidenceReadThen`]. A failed read changes nothing and refuses
//   the follow-up (D-50: never fall back to memory).
//
// Applying a read changes this world only (no row is written): owners move, the clans' castle,
// fortress and hall ids follow, online members of a clan that gained or lost a castle or
// fortress get or lose the residential skills (with a new `SkillList`) and every clan concerned
// gets `PledgeShowInfoUpdate`. The functions of a residence that was read are replaced by its rows.

use l2_util::ObjectId;
use tracing::{debug, warn};

use super::{
    CastleId, CastleRecord, ClanHallId, ClanHallKind, ClanHallRecord, FortId, FortRecord, ResidenceRef,
    SiegableHallRecord,
};
use crate::outbox::PersistCommand;
use crate::world::World;

/// The residences of one clan as the session read them at `CharacterSelect`
/// (`PlayerProfile::residence_refresh`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResidenceRefresh {
    pub clan_id: i32,
    /// The clan's `hasCastle` as read now (0 for none).
    pub castle_id: i32,
    /// That castle, read like `new Castle(id)` (its owner is the last clan whose `hasCastle`
    /// names it, maybe another clan).
    pub castle: Option<CastleRecord>,
    /// The `fort` rows whose owner is the clan, with their functions.
    pub forts: Vec<FortRecord>,
    /// The `clanhall` rows whose owner is the clan, with their functions.
    pub clan_halls: Vec<ClanHallRecord>,
    /// The `siegable_clanhall` rows whose owner is the clan, with their functions.
    pub siegable_halls: Vec<SiegableHallRecord>,
}

/// The rows of one residence read again on demand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResidenceRereadRows {
    Castle(CastleRecord),
    Fort(FortRecord),
    ClanHall(ClanHallRecord),
    SiegableHall(SiegableHallRecord),
    /// The `auction` row of a hall's auction with its bids; `None` when the database has no such
    /// auction any more (M7-CH-01).
    Auction {
        id: i32,
        record: Option<crate::clanhall::AuctionRecord>,
    },
    /// The database has no row for it (nothing changes).
    Missing,
    /// A castle (`None`: no row) and every `siege_clans` row (M7-SIEGE-01).
    SiegeClans {
        castle: Option<CastleRecord>,
        rows: Vec<crate::siege::SiegeClanRecord>,
    },
}

/// What the world does once a residence was read again. The cards that act on residences add
/// their variants here (a chamberlain page, a withdrawal from the treasury, ...) and handle them
/// in [`World::on_residence_reread`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResidenceReadThen {
    /// Only bring this world up to date.
    Nothing,
    /// `RequestJoinSiege` (M7-SIEGE-01).
    SiegeJoin {
        player: ObjectId,
        castle_id: i32,
        is_attacker: i32,
        is_joining: i32,
    },
    /// `RequestConfirmSiegeWaitingList` (M7-SIEGE-01).
    SiegeConfirmWaiting {
        player: ObjectId,
        castle_id: i32,
        clan_id: i32,
        approved: i32,
    },
    /// `RequestSiegeAttackerList` (M7-SIEGE-01).
    SiegeAttackerList { player: ObjectId, castle_id: i32 },
    /// `RequestSiegeDefenderList` (M7-SIEGE-01).
    SiegeDefenderList { player: ObjectId, castle_id: i32 },
    /// The Java GS ran the siege of a castle: start the schedule again (M7-SIEGE-01, D-51).
    SiegeRestartSchedule { castle_id: i32 },
    /// The end task of an auction (by its key) is due: decide on the fresh rows (M7-CH-01).
    AuctionEnd(u64),
    /// A clan bids: go on to read its clan warehouse (M7-CH-01).
    AuctionBid {
        player: ObjectId,
        auction_id: i32,
        bid: i64,
    },
    /// A clan cancels its bid (M7-CH-01).
    AuctionCancelBid { player: ObjectId, auction_id: i32 },
    /// The owner of the hall cancels its auction (M7-CH-01).
    AuctionCancel { player: ObjectId, auction_id: i32 },
}

impl World {
    // ------------------------------------------------------------------ CharacterSelect

    /// Applies the residences the session read for the player's clan (see the module comment).
    /// Called from `on_entered` after the clan restore.
    pub(crate) fn residence_refresh_player(&mut self, id: ObjectId) {
        let Some(refresh) = self
            .players
            .get_mut(&id)
            .and_then(|p| p.data.profile.residence_refresh.take())
        else {
            return;
        };
        if refresh.clan_id <= 0
            || refresh.clan_id != self.clan_id_of(id)
            || self.clan(refresh.clan_id).is_none()
        {
            return;
        }
        self.apply_residence_refresh(*refresh);
    }

    /// The residences of a clan as the database has them now.
    pub fn apply_residence_refresh(&mut self, refresh: ResidenceRefresh) {
        let clan_id = refresh.clan_id;
        // The castle.
        if let Some(castle) = self.castle_by_owner(clan_id)
            && castle.0 != refresh.castle_id
        {
            self.set_castle_owner_in_memory(castle, 0);
            self.clear_residence_functions(ResidenceRef::Castle(castle));
        }
        if let Some(record) = refresh.castle.filter(|r| r.id == refresh.castle_id) {
            self.apply_castle_record(&record);
        }
        if let Some(clan) = self.clan_mut(clan_id) {
            clan.castle_id = refresh.castle_id;
        }
        // The fortresses.
        let db_forts: Vec<i32> = refresh.forts.iter().map(|r| r.id).collect();
        let stale: Vec<FortId> = self
            .residences
            .forts
            .iter()
            .filter(|f| f.owner_id == clan_id && !db_forts.contains(&f.id.0))
            .map(|f| f.id)
            .collect();
        for fort in stale {
            self.set_fort_owner_in_memory(fort, 0);
            self.clear_residence_functions(ResidenceRef::Fort(fort));
        }
        for record in &refresh.forts {
            self.apply_fort_record(record);
        }
        // The halls.
        let db_halls: Vec<i32> = refresh
            .clan_halls
            .iter()
            .map(|r| r.id)
            .chain(refresh.siegable_halls.iter().map(|r| r.id))
            .collect();
        let stale: Vec<ClanHallId> = self
            .residences
            .clan_halls
            .values()
            .filter(|h| h.owner_id == clan_id && !db_halls.contains(&h.id.0))
            .map(|h| h.id)
            .collect();
        for hall in stale {
            self.set_clan_hall_owner_in_memory(hall, 0);
            self.clear_residence_functions(ResidenceRef::ClanHall(hall));
        }
        for record in &refresh.clan_halls {
            self.apply_clan_hall_record(record);
        }
        for record in &refresh.siegable_halls {
            self.apply_siegable_hall_record(record);
        }
    }

    // ------------------------------------------------------------------ on demand

    /// Asks the persistence task to read a residence again; the answer comes back as
    /// `WorldCommand::ResidenceReread` and goes on with `then` (D-50, D-15).
    pub fn request_residence_reread(&mut self, residence: ResidenceRef, then: ResidenceReadThen) {
        let kind = match residence {
            ResidenceRef::ClanHall(id) if self.clan_hall(id).is_some_and(|h| h.is_siegable_hall()) => {
                ResidenceReadKind::SiegableHall
            }
            ResidenceRef::ClanHall(_) => ResidenceReadKind::ClanHall,
            ResidenceRef::Castle(_) => ResidenceReadKind::Castle,
            ResidenceRef::Fort(_) => ResidenceReadKind::Fort,
        };
        self.outbox.persist(PersistCommand::ResidenceRereadRequest {
            residence_id: residence.residence_id(),
            kind,
            then,
        });
    }

    /// The answer of [`Self::request_residence_reread`]: the rows replace this world's copy, then
    /// the follow-up runs. A failed read changes nothing and the follow-up is refused.
    pub(crate) fn on_residence_reread(
        &mut self,
        result: Result<ResidenceRereadRows, ()>,
        then: ResidenceReadThen,
    ) {
        let Ok(rows) = result else {
            warn!("A residence could not be read again; {then:?} is refused.");
            self.auction_read_failed(&then);
            return;
        };
        match &rows {
            ResidenceRereadRows::Castle(record) => self.apply_castle_record(record),
            ResidenceRereadRows::Fort(record) => self.apply_fort_record(record),
            ResidenceRereadRows::ClanHall(record) => self.apply_clan_hall_record(record),
            ResidenceRereadRows::SiegableHall(record) => self.apply_siegable_hall_record(record),
            ResidenceRereadRows::Auction { id, record } => self.apply_auction_rows(*id, record.as_ref()),
            ResidenceRereadRows::Missing => debug!("A residence read again has no row."),
            ResidenceRereadRows::SiegeClans { castle, rows } => {
                self.apply_siege_reread(castle.as_ref(), rows)
            }
        }
        match then {
            ResidenceReadThen::Nothing => {}
            ResidenceReadThen::SiegeJoin {
                player,
                castle_id,
                is_attacker,
                is_joining,
            } => self.siege_join_after_reread(player, CastleId(castle_id), is_attacker, is_joining),
            ResidenceReadThen::SiegeConfirmWaiting {
                player,
                castle_id,
                clan_id,
                approved,
            } => self.siege_confirm_after_reread(player, CastleId(castle_id), clan_id, approved),
            ResidenceReadThen::SiegeAttackerList { player, castle_id } => {
                self.send_siege_attacker_list(player, CastleId(castle_id));
            }
            ResidenceReadThen::SiegeDefenderList { player, castle_id } => {
                self.send_siege_defender_list(player, CastleId(castle_id));
            }
            ResidenceReadThen::SiegeRestartSchedule { castle_id } => {
                self.siege_restart_schedule(CastleId(castle_id));
            }
            then => self.auction_read_then(then),
        }
    }

    // ------------------------------------------------------------------ applying rows

    /// A castle row as the database has it now: its values, its owner and (for an owner) its
    /// functions.
    pub(crate) fn apply_castle_record(&mut self, record: &CastleRecord) {
        let id = CastleId(record.id);
        let Some(castle) = self.castle_mut(id) else {
            return;
        };
        if record.found {
            castle.name = record.name.clone();
            castle.tax_percent = record.tax_percent;
            castle.tax_rate = f64::from(record.tax_percent) / 100.0;
            castle.treasury = record.treasury;
            castle.siege_date = record.siege_date;
            castle.time_registration_over = record.time_registration_over;
            castle.time_registration_over_date = record.time_registration_over_date;
            castle.show_npc_crest = record.show_npc_crest;
            castle.ticket_buy_count = record.ticket_buy_count;
        }
        let owner = if self.clan(record.owner_id).is_some() {
            record.owner_id
        } else {
            0
        };
        self.set_castle_owner_in_memory(id, owner);
        self.clear_residence_functions(ResidenceRef::Castle(id));
        if owner != 0 {
            self.load_residence_functions(ResidenceRef::Castle(id), &record.functions);
        }
    }

    /// A fortress row as the database has it now.
    pub(crate) fn apply_fort_record(&mut self, record: &FortRecord) {
        let id = FortId(record.id);
        let Some(fort) = self.fort_mut(id) else {
            return;
        };
        fort.siege_date = record.siege_date;
        fort.last_owned_time = record.last_owned_time;
        fort.fort_type = record.fort_type;
        fort.state = record.state;
        fort.castle_id = record.castle_id;
        fort.supply_level = record.supply_level;
        let owner = if self.clan(record.owner_id).is_some() {
            record.owner_id
        } else {
            0
        };
        self.set_fort_owner_in_memory(id, owner);
        self.clear_residence_functions(ResidenceRef::Fort(id));
        if owner != 0 {
            self.load_residence_functions(ResidenceRef::Fort(id), &record.functions);
        }
    }

    /// An auctionable hall's row as the database has it now.
    pub(crate) fn apply_clan_hall_record(&mut self, record: &ClanHallRecord) {
        let id = ClanHallId(record.id);
        let Some(hall) = self.clan_hall_mut(id) else {
            return;
        };
        if let ClanHallKind::Auctionable {
            lease,
            paid_until,
            grade,
            paid,
        } = &mut hall.kind
        {
            *lease = record.lease;
            *paid_until = record.paid_until;
            *grade = record.grade;
            *paid = record.paid;
        }
        self.apply_hall_owner(id, record.owner_id, &record.functions);
        // `new AuctionableHall(set)` starts the rent of an owned hall.
        self.clan_hall_rent_after_reread(id);
    }

    /// A conquerable hall's row as the database has it now.
    pub(crate) fn apply_siegable_hall_record(&mut self, record: &SiegableHallRecord) {
        let id = ClanHallId(record.id);
        let Some(hall) = self.clan_hall_mut(id) else {
            return;
        };
        if let ClanHallKind::Siegable {
            next_siege,
            siege_length,
            schedule_config,
        } = &mut hall.kind
        {
            *next_siege = record.next_siege;
            *siege_length = record.siege_length;
            schedule_config.clone_from(&record.schedule_config);
        }
        self.apply_hall_owner(id, record.owner_id, &record.functions);
    }

    fn apply_hall_owner(&mut self, id: ClanHallId, owner_id: i32, functions: &[super::FunctionRecord]) {
        let owner = if self.clan(owner_id).is_some() {
            owner_id
        } else {
            0
        };
        self.set_clan_hall_owner_in_memory(id, owner);
        self.clear_residence_functions(ResidenceRef::ClanHall(id));
        if owner != 0 {
            self.load_residence_functions(ResidenceRef::ClanHall(id), functions);
        }
    }

    // ------------------------------------------------------------------ owners in memory

    /// A castle changes hands in this world only (a re-read): the clans' castle ids, the
    /// residential skills of their online members and `PledgeShowInfoUpdate`.
    pub(crate) fn set_castle_owner_in_memory(&mut self, id: CastleId, owner: i32) {
        let Some(old) = self.castle(id).map(|c| c.owner_id) else {
            return;
        };
        if old == owner {
            return;
        }
        if let Some(castle) = self.castle_mut(id) {
            castle.owner_id = owner;
        }
        let residence = ResidenceRef::Castle(id);
        if old != 0 && self.clan(old).is_some() {
            if let Some(clan) = self.clan_mut(old)
                && clan.castle_id == id.0
            {
                clan.castle_id = 0;
            }
            for member in self.clan_online_members(old, 0) {
                self.remove_residential_skills(residence, member);
                self.send_skill_list(member);
            }
            self.broadcast_clan_info_update(old);
        }
        if owner != 0 {
            if let Some(clan) = self.clan_mut(owner) {
                clan.castle_id = id.0;
            }
            for member in self.clan_online_members(owner, 0) {
                self.give_residential_skills(residence, member);
                self.send_skill_list(member);
            }
            self.broadcast_clan_info_update(owner);
        }
    }

    /// A fortress changes hands in this world only (a re-read); the flag follows.
    pub(crate) fn set_fort_owner_in_memory(&mut self, id: FortId, owner: i32) {
        let Some(old) = self.fort(id).map(|f| f.owner_id) else {
            return;
        };
        if old == owner {
            return;
        }
        if let Some(fort) = self.fort_mut(id) {
            fort.owner_id = owner;
        }
        self.fort_set_visible_flag(id, owner != 0);
        let residence = ResidenceRef::Fort(id);
        if old != 0 && self.clan(old).is_some() {
            if let Some(clan) = self.clan_mut(old)
                && clan.fort_id == id.0
            {
                clan.fort_id = 0;
            }
            for member in self.clan_online_members(old, 0) {
                self.remove_residential_skills(residence, member);
                self.send_skill_list(member);
            }
            self.broadcast_clan_info_update(old);
        }
        if owner != 0 {
            if let Some(clan) = self.clan_mut(owner) {
                clan.fort_id = id.0;
            }
            for member in self.clan_online_members(owner, 0) {
                self.give_residential_skills(residence, member);
                self.send_skill_list(member);
            }
            self.broadcast_clan_info_update(owner);
        }
    }

    /// A clan hall changes hands in this world only (a re-read).
    pub(crate) fn set_clan_hall_owner_in_memory(&mut self, id: ClanHallId, owner: i32) {
        let Some(old) = self.clan_hall(id).map(|h| h.owner_id) else {
            return;
        };
        if let Some(hall) = self.clan_hall_mut(id) {
            hall.owner_id = owner;
            hall.is_free = owner == 0;
        }
        if old == owner {
            return;
        }
        if old != 0 && self.clan(old).is_some() {
            if let Some(clan) = self.clan_mut(old)
                && clan.hideout_id == id.0
            {
                clan.hideout_id = 0;
            }
            self.broadcast_clan_info_update(old);
        }
        if owner != 0 {
            if let Some(clan) = self.clan_mut(owner) {
                clan.hideout_id = id.0;
            }
            self.broadcast_clan_info_update(owner);
        }
    }
}

/// Which table a re-read reads (`PersistCommand::ResidenceRereadRequest`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidenceReadKind {
    /// `castle` (with the owner from `clan_data` and the functions).
    Castle,
    /// `fort`.
    Fort,
    /// `clanhall`.
    ClanHall,
    /// `siegable_clanhall`.
    SiegableHall,
    /// `castle` (as [`Self::Castle`]) and every `siege_clans` row (M7-SIEGE-01).
    SiegeClans,
    /// `auction` and `auction_bid` of a hall's auction (M7-CH-01).
    Auction,
}
