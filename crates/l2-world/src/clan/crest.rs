//! Clan, large clan and alliance crests: the crest cache, the uploads and the requests of the
//! client (M6-CLAN-02).
// Java: data/sql/CrestTable.java
// Java: entity/clan/Crest.java, entity/clan/enums/CrestType.java
// Java: entity/clan/Clan.java (changeClanCrest, changeAllyCrest, changeLargeCrest)
// Java: network/clientpackets/{RequestSetPledgeCrest,RequestExSetPledgeCrestLarge,
//       RequestSetAllyCrest,RequestPledgeCrest,RequestExPledgeCrestLarge,RequestAllyCrest}.java
//
// # Shape
// Java's `CrestTable` is a map `crest id -> Crest` backed by the `crests` table. Here the map is
// [`CrestTable`] (in `ClanWorld`), loaded at startup from the rows the game server read
// ([`World::load_crests`], D-15) and changed by the world; every change goes out as a
// `ClanWrite::Crest*` with Java's own statement.
//
// # Side by side with the Java game server (D-39, D-49)
// The start-up cleanup of `CrestTable.load` deletes every crest no clan uses (except the one with
// the highest id, which is never reused) and clears the crest columns of clans whose crest is
// gone. Those are global writes; they run only with `ClanMaintenanceDatabaseWrites = True`
// (`config/SideBySide.ini`). The world applies the same changes to its own state either way.
//
// # Crest ids (M7-FU-03)
// Java allocates a crest id as `max(crest_id) + 1` read at its start, so two servers that both
// register a crest in one session pick the same id and the second `INSERT` fails on the primary
// key. Java never takes an id below the maximum it read at its start, so Kain takes the **lowest
// free id above 0**: `createCrest` asks the persistence task for the used ids
// (`PersistCommand::CrestIdsRequest`, D-15) and the crest is set when the answer comes
// (`WorldCommand::CrestIdsLoaded`). The ids the world itself holds count as used too (the answer
// may not include an insert still in the queue). Only when there is no gap is the id `max + 1`
// (logged). An insert that fails (the other server took the id meanwhile) is retried once under a
// fresh read: the crest is renumbered (`WorldCommand::CrestInsertFailed`).
//
// Caveat: the client keeps the crests it has seen in its cache by id. A low id that an old,
// deleted crest used can show that old picture on a client that cached it, until the client's
// crest cache is cleared; Java avoids it by never reusing an id below its maximum.

use std::collections::{HashMap, HashSet};

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::pledge::{AllyCrest, ExPledgeEmblem, PledgeCrest};
use l2_protocol_game::server::{ActionFailed, SystemMessage};
use l2_util::ObjectId;
use tracing::{info, warn};

use super::model::ClanAccess;
use super::{ClanWrite, CrestWrite};
use crate::outbox::PersistCommand;
use crate::world::{World, unix_ms_now};

/// Java `CrestType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrestType {
    Pledge,
    PledgeLarge,
    Ally,
}

impl CrestType {
    /// Java `getId()`: the `crests.type` column.
    pub const fn id(self) -> i32 {
        match self {
            Self::Pledge => 1,
            Self::PledgeLarge => 2,
            Self::Ally => 3,
        }
    }

    /// Java `CrestType.getById(id)`.
    pub const fn from_id(id: i32) -> Option<Self> {
        match id {
            1 => Some(Self::Pledge),
            2 => Some(Self::PledgeLarge),
            3 => Some(Self::Ally),
            _ => None,
        }
    }
}

/// Java `Crest`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Crest {
    pub id: i32,
    pub data: Vec<u8>,
    pub kind: CrestType,
}

/// One `crests` row as the game server read it (`SELECT crest_id, data, type`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CrestRecord {
    pub crest_id: i32,
    pub data: Vec<u8>,
    /// The `type` column, mapped by [`CrestType::from_id`].
    pub crest_type: i32,
}

/// Java `CrestTable`: the crests by id and the next id to give out.
#[derive(Debug)]
pub struct CrestTable {
    crests: HashMap<i32, Crest>,
    /// Java `_nextId`, starts at 1.
    next_id: i32,
    /// The id requests waiting for the persistence task (M7-FU-03).
    pending: Vec<PendingCrest>,
    next_ticket: u32,
    /// The ids a retry gave out; an insert of one of them that fails again is only logged.
    renumbered: HashSet<i32>,
}

/// What a crest id answer is for.
#[derive(Debug, Clone)]
enum PendingCrestKind {
    /// `createCrest` followed by the `change*Crest` of `target`.
    New {
        player: ObjectId,
        data: Vec<u8>,
        kind: CrestType,
        target: CrestTarget,
    },
    /// The insert of crest `old_id` failed: it gets a new id.
    Renumber { old_id: i32 },
}

#[derive(Debug, Clone)]
struct PendingCrest {
    ticket: u32,
    what: PendingCrestKind,
}

/// The clan column a new crest goes into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CrestTarget {
    Pledge {
        clan_id: i32,
    },
    PledgeLarge {
        clan_id: i32,
    },
    /// `changeAllyCrest(id, false)` of the alliance leader's clan.
    Ally {
        leader_clan_id: i32,
    },
}

impl Default for CrestTable {
    fn default() -> Self {
        Self {
            crests: HashMap::new(),
            next_id: 1,
            pending: Vec::new(),
            next_ticket: 0,
            renumbered: HashSet::new(),
        }
    }
}

impl CrestTable {
    /// Java `getCrest(crestId)`.
    pub fn get(&self, crest_id: i32) -> Option<&Crest> {
        self.crests.get(&crest_id)
    }

    /// The number of crests in the cache.
    pub fn len(&self) -> usize {
        self.crests.len()
    }

    pub fn is_empty(&self) -> bool {
        self.crests.is_empty()
    }

    /// The id the next crest gets.
    pub fn next_id(&self) -> i32 {
        self.next_id
    }
}

/// Which clan column a crest change is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClanCrestColumn {
    Pledge,
    PledgeLarge,
}

impl World {
    /// Java `CrestTable.getCrest(crestId)`.
    pub fn crest(&self, crest_id: i32) -> Option<&Crest> {
        self.clans.crests.get(crest_id)
    }

    /// The crests the world holds.
    pub fn crest_table(&self) -> &CrestTable {
        &self.clans.crests
    }

    /// Java `CrestTable.load()`, after `ClanTable`: the cache from the `crests` rows, the
    /// deletion of the crests nobody uses, and the clans whose crest is gone lose it.
    pub fn load_crests(&mut self, mut records: Vec<CrestRecord>) {
        let write = self.clans.settings.maintenance_database_writes;
        let mut table = CrestTable::default();
        let mut in_use = HashSet::new();
        for clan in self.clans.clans.values() {
            for id in [clan.crest_id, clan.crest_large_id, clan.ally_crest_id] {
                if id != 0 {
                    in_use.insert(id);
                }
            }
        }
        // `ORDER BY crest_id DESC`
        records.sort_by_key(|r| std::cmp::Reverse(r.crest_id));
        let mut removals = Vec::new();
        for record in records {
            let id = record.crest_id;
            if table.next_id <= id {
                table.next_id = id.wrapping_add(1);
            }
            // "Delete all unused crests except the last one we do not want to reuse a crest id
            // because client will display wrong crest if it is reused."
            if !in_use.contains(&id) && id != table.next_id.wrapping_sub(1) {
                removals.push(id);
                continue;
            }
            match CrestType::from_id(record.crest_type) {
                Some(kind) => {
                    table.crests.insert(
                        id,
                        Crest {
                            id,
                            data: record.data,
                            kind,
                        },
                    );
                }
                None => warn!("Unknown crest type found in database. Type:{}", record.crest_type),
            }
        }
        info!("CrestTable: Loaded {} Crests.", table.crests.len());
        self.clans.crests = table;
        if write {
            for crest_id in removals {
                self.clan_write(ClanWrite::Crest(CrestWrite::Delete { crest_id }));
            }
        }

        let ids: Vec<i32> = self.clans.clans.keys().copied().collect();
        for clan_id in ids {
            let Some(clan) = self.clan(clan_id) else {
                continue;
            };
            let (name, crest, large, ally) = (
                clan.name.clone(),
                clan.crest_id,
                clan.crest_large_id,
                clan.ally_crest_id,
            );
            if crest != 0 && self.crest(crest).is_none() {
                info!("Removing non-existent crest for clan {name} [{clan_id}], crestId:{crest}");
                // `setCrestId(0)` first, so `changeClanCrest(0)` has no crest to remove.
                if let Some(c) = self.clan_mut(clan_id) {
                    c.crest_id = 0;
                }
                self.change_clan_crest_inner(clan_id, 0, write);
            }
            if large != 0 && self.crest(large).is_none() {
                info!("Removing non-existent large crest for clan {name} [{clan_id}], crestLargeId:{large}");
                if let Some(c) = self.clan_mut(clan_id) {
                    c.crest_large_id = 0;
                }
                self.change_large_crest_inner(clan_id, 0, write);
            }
            if ally != 0 && self.crest(ally).is_none() {
                info!("Removing non-existent ally crest for clan {name} [{clan_id}], allyCrestId:{ally}");
                if let Some(c) = self.clan_mut(clan_id) {
                    c.ally_crest_id = 0;
                }
                self.change_ally_crest_inner(clan_id, 0, true, write);
            }
        }
    }

    /// Java `CrestTable.createCrest(data, type)` plus the `change*Crest` that follows it: asks the
    /// persistence task for the used crest ids; the crest is made when the answer comes
    /// ([`World::on_crest_ids_loaded`]).
    pub(crate) fn request_new_crest(
        &mut self,
        player: ObjectId,
        data: Vec<u8>,
        kind: CrestType,
        target: CrestTarget,
    ) {
        self.push_crest_request(PendingCrestKind::New {
            player,
            data,
            kind,
            target,
        });
    }

    fn push_crest_request(&mut self, what: PendingCrestKind) {
        let table = &mut self.clans.crests;
        let ticket = table.next_ticket;
        table.next_ticket = ticket.wrapping_add(1);
        table.pending.push(PendingCrest { ticket, what });
        self.outbox.persist(PersistCommand::CrestIdsRequest { ticket });
    }

    /// The lowest id above 0 that neither the database (`used`) nor the cache has.
    fn free_crest_id(&self, used: &[i32]) -> i32 {
        let taken: HashSet<i32> = used
            .iter()
            .copied()
            .chain(self.clans.crests.crests.keys().copied())
            .collect();
        let max = taken.iter().copied().max().unwrap_or(0);
        match (1..=i32::MAX).find(|id| !taken.contains(id)) {
            Some(id) if id <= max => id,
            Some(id) => {
                info!("CrestTable: no free crest id below the maximum, taking {id}.");
                id
            }
            None => {
                // Every id is taken (not reachable with a real table).
                warn!("CrestTable: no free crest id left, reusing the next one.");
                self.clans.crests.next_id
            }
        }
    }

    /// Makes a crest of `data` under `id` and inserts it.
    fn add_crest(&mut self, id: i32, data: Vec<u8>, kind: CrestType) {
        let table = &mut self.clans.crests;
        if id >= table.next_id {
            table.next_id = id.wrapping_add(1);
        }
        table.crests.insert(
            id,
            Crest {
                id,
                data: data.clone(),
                kind,
            },
        );
        self.clan_write(ClanWrite::Crest(CrestWrite::Insert {
            crest_id: id,
            data,
            crest_type: kind.id(),
        }));
    }

    /// `WorldCommand::CrestIdsLoaded`: the used crest ids are in; the crest gets the lowest free
    /// one. A failed read creates nothing (the player gets `ActionFailed`).
    pub(crate) fn on_crest_ids_loaded(&mut self, ticket: u32, result: Result<Vec<i32>, ()>) {
        let table = &mut self.clans.crests;
        let Some(pos) = table.pending.iter().position(|p| p.ticket == ticket) else {
            return;
        };
        let pending = table.pending.remove(pos);
        let used = match result {
            Ok(used) => used,
            Err(()) => {
                warn!("CrestTable: could not read the used crest ids; the crest is not created.");
                if let PendingCrestKind::New { player, .. } = pending.what {
                    self.send(player, &ActionFailed);
                }
                return;
            }
        };
        let id = self.free_crest_id(&used);
        match pending.what {
            PendingCrestKind::New {
                player,
                data,
                kind,
                target,
            } => self.finish_new_crest(player, id, data, kind, target),
            PendingCrestKind::Renumber { old_id } => self.renumber_crest(old_id, id),
        }
    }

    fn finish_new_crest(
        &mut self,
        player: ObjectId,
        id: i32,
        data: Vec<u8>,
        kind: CrestType,
        target: CrestTarget,
    ) {
        // The clan may be gone while the database answered.
        let clan_id = match target {
            CrestTarget::Pledge { clan_id } | CrestTarget::PledgeLarge { clan_id } => clan_id,
            CrestTarget::Ally { leader_clan_id } => leader_clan_id,
        };
        if self.clan(clan_id).is_none() {
            return;
        }
        self.add_crest(id, data, kind);
        let message = match target {
            CrestTarget::Pledge { clan_id } => {
                self.change_clan_crest(clan_id, id);
                SystemMessageId::TheCrestWasSuccessfullyRegistered
            }
            CrestTarget::PledgeLarge { clan_id } => {
                self.change_large_crest(clan_id, id);
                SystemMessageId::TheClanCrestWasSuccessfullyRegisteredRememberOnlyAClanThatOwnsAClanHallOrCastleCanDisplayACrest
            }
            CrestTarget::Ally { leader_clan_id } => {
                self.change_ally_crest(leader_clan_id, id, false);
                SystemMessageId::TheCrestWasSuccessfullyRegistered
            }
        };
        self.send(player, &SystemMessage::new(message));
    }

    /// `WorldCommand::CrestInsertFailed`: the `INSERT` of the crest failed (the other server took
    /// the id). It is retried once under a fresh read of the used ids.
    pub(crate) fn on_crest_insert_failed(&mut self, crest_id: i32) {
        let table = &self.clans.crests;
        if !table.crests.contains_key(&crest_id) {
            return;
        }
        if table.renumbered.contains(&crest_id) {
            warn!("CrestTable: the insert of crest {crest_id} failed again; it stays in memory only.");
            return;
        }
        warn!("CrestTable: the insert of crest {crest_id} failed; reading the used ids again.");
        self.push_crest_request(PendingCrestKind::Renumber { old_id: crest_id });
    }

    /// The crest `old_id` (whose insert failed) moves to `new_id`; the clans that use it follow.
    fn renumber_crest(&mut self, old_id: i32, new_id: i32) {
        let Some(crest) = self.clans.crests.crests.remove(&old_id) else {
            return;
        };
        self.clans.crests.renumbered.insert(new_id);
        self.add_crest(new_id, crest.data, crest.kind);
        let ids: Vec<i32> = self.clans.clans.keys().copied().collect();
        for clan_id in ids {
            let Some(clan) = self.clan_mut(clan_id) else {
                continue;
            };
            let mut writes = Vec::new();
            if clan.crest_id == old_id {
                clan.crest_id = new_id;
                writes.push(CrestWrite::ClanCrest {
                    clan_id,
                    crest_id: new_id,
                });
            }
            if clan.crest_large_id == old_id {
                clan.crest_large_id = new_id;
                writes.push(CrestWrite::ClanLargeCrest {
                    clan_id,
                    crest_id: new_id,
                });
            }
            if clan.ally_crest_id == old_id {
                clan.ally_crest_id = new_id;
                writes.push(CrestWrite::AllyCrestOfClan {
                    clan_id,
                    crest_id: new_id,
                });
            }
            if writes.is_empty() {
                continue;
            }
            for write in writes {
                self.clan_write(ClanWrite::Crest(write));
            }
            for member in self.clan_online_members(clan_id, 0) {
                self.broadcast_user_info(member);
            }
        }
    }

    /// Java `CrestTable.removeCrest(crestId)`.
    pub(crate) fn remove_crest(&mut self, crest_id: i32) {
        self.clans.crests.crests.remove(&crest_id);
        // "avoid removing last crest id we do not want to lose index... Because client will
        // display wrong crest if it is reused."
        if crest_id == self.clans.crests.next_id.wrapping_sub(1) {
            return;
        }
        self.clan_write(ClanWrite::Crest(CrestWrite::Delete { crest_id }));
    }

    // ------------------------------------------------------------------ Clan.change*Crest

    /// Java `Clan.changeClanCrest(crestId)`: 0 removes the crest.
    pub(crate) fn change_clan_crest(&mut self, clan_id: i32, crest_id: i32) {
        self.change_clan_crest_inner(clan_id, crest_id, true);
    }

    fn change_clan_crest_inner(&mut self, clan_id: i32, crest_id: i32, write: bool) {
        self.change_own_crest(clan_id, crest_id, write, ClanCrestColumn::Pledge);
    }

    /// Java `Clan.changeLargeCrest(crestId)`.
    pub(crate) fn change_large_crest(&mut self, clan_id: i32, crest_id: i32) {
        self.change_large_crest_inner(clan_id, crest_id, true);
    }

    fn change_large_crest_inner(&mut self, clan_id: i32, crest_id: i32, write: bool) {
        self.change_own_crest(clan_id, crest_id, write, ClanCrestColumn::PledgeLarge);
    }

    /// `changeClanCrest` and `changeLargeCrest`: they differ in the column only.
    fn change_own_crest(&mut self, clan_id: i32, crest_id: i32, write: bool, column: ClanCrestColumn) {
        let Some(clan) = self.clan(clan_id) else {
            return;
        };
        let old = match column {
            ClanCrestColumn::Pledge => clan.crest_id,
            ClanCrestColumn::PledgeLarge => clan.crest_large_id,
        };
        if old != 0 {
            self.remove_crest(old);
        }
        if let Some(clan) = self.clan_mut(clan_id) {
            match column {
                ClanCrestColumn::Pledge => clan.crest_id = crest_id,
                ClanCrestColumn::PledgeLarge => clan.crest_large_id = crest_id,
            }
        }
        if write {
            self.clan_write(ClanWrite::Crest(match column {
                ClanCrestColumn::Pledge => CrestWrite::ClanCrest { clan_id, crest_id },
                ClanCrestColumn::PledgeLarge => CrestWrite::ClanLargeCrest { clan_id, crest_id },
            }));
        }
        for member in self.clan_online_members(clan_id, 0) {
            self.broadcast_user_info(member);
        }
    }

    /// Java `Clan.changeAllyCrest(crestId, onlyThisClan)`.
    pub(crate) fn change_ally_crest(&mut self, clan_id: i32, crest_id: i32, only_this_clan: bool) {
        self.change_ally_crest_inner(clan_id, crest_id, only_this_clan, true);
    }

    fn change_ally_crest_inner(&mut self, clan_id: i32, crest_id: i32, only_this_clan: bool, write: bool) {
        let Some(clan) = self.clan(clan_id) else {
            return;
        };
        let (own_crest, ally_id) = (clan.ally_crest_id, clan.ally_id);
        if only_this_clan {
            if write {
                self.clan_write(ClanWrite::Crest(CrestWrite::AllyCrestOfClan {
                    clan_id,
                    crest_id,
                }));
            }
            if let Some(clan) = self.clan_mut(clan_id) {
                clan.ally_crest_id = crest_id;
            }
            for member in self.clan_online_members(clan_id, 0) {
                self.broadcast_user_info(member);
            }
        } else {
            if own_crest != 0 {
                self.remove_crest(own_crest);
            }
            if write {
                self.clan_write(ClanWrite::Crest(CrestWrite::AllyCrestOfAlly {
                    ally_id,
                    crest_id,
                }));
            }
            // NOTE(java-quirk): `dissolveAlly` sets the clan's ally id to 0 before it calls this,
            // so the statement above is `WHERE ally_id = 0` and the loop below walks the clans of
            // alliance 0, which are none: the clan keeps its old `_allyCrestId`.
            for ally in self.clan_allies(ally_id) {
                if let Some(c) = self.clan_mut(ally) {
                    c.ally_crest_id = crest_id;
                }
                for member in self.clan_online_members(ally, 0) {
                    self.broadcast_user_info(member);
                }
            }
        }
    }

    // ------------------------------------------------------------------ the client's requests

    /// Java `RequestPledgeCrest.runImpl`: `new PledgeCrest(crestId)`.
    pub(crate) fn on_request_pledge_crest(&mut self, player: ObjectId, crest_id: i32) {
        let data = self.crest(crest_id).map(|c| c.data.clone()).unwrap_or_default();
        self.send(player, &PledgeCrest { crest_id, data });
    }

    /// Java `RequestExPledgeCrestLarge.runImpl`: `new ExPledgeEmblem(crestId)`.
    pub(crate) fn on_request_ex_pledge_crest_large(&mut self, player: ObjectId, crest_id: i32) {
        let data = self.crest(crest_id).map(|c| c.data.clone()).unwrap_or_default();
        self.send(player, &ExPledgeEmblem { crest_id, data });
    }

    /// Java `RequestAllyCrest.runImpl`: `new AllyCrest(crestId)`.
    pub(crate) fn on_request_ally_crest(&mut self, player: ObjectId, crest_id: i32) {
        let data = self.crest(crest_id).map(|c| c.data.clone()).unwrap_or_default();
        self.send(player, &AllyCrest { crest_id, data });
    }

    /// Java `RequestSetPledgeCrest.runImpl`. `data` is `None` above the packet's limit.
    pub(crate) fn on_request_set_pledge_crest(
        &mut self,
        player: ObjectId,
        length: i32,
        data: Option<Vec<u8>>,
    ) {
        if !self.players.contains_key(&player) {
            return;
        }
        if length < 0 {
            self.send(
                player,
                &SystemMessage::new(
                    SystemMessageId::TheSizeOfTheUploadedCrestOrInsigniaDoesNotMeetTheStandardRequirements,
                ),
            );
            return;
        }
        if length > 256 {
            self.send(
                player,
                &SystemMessage::new(
                    SystemMessageId::TheSizeOfTheImageFileIsInappropriatePleaseAdjustTo16x12Pixels,
                ),
            );
            return;
        }
        let clan_id = self.clan_id_of(player);
        let Some(clan) = self.clan(clan_id) else {
            return;
        };
        let (dissolving, level, crest_id) = (clan.dissolving_expiry_time, clan.level, clan.crest_id);
        if dissolving > unix_ms_now() {
            self.send(
                player,
                &SystemMessage::new(
                    SystemMessageId::AsYouAreCurrentlyScheduleForClanDissolutionYouCannotRegisterOrDeleteAClanCrest,
                ),
            );
            return;
        }
        if !self.has_clan_access(player, ClanAccess::ChangeCrest) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouAreNotAuthorizedToDoThat),
            );
            return;
        }
        if length == 0 {
            if crest_id != 0 {
                self.change_clan_crest(clan_id, 0);
                self.send(
                    player,
                    &SystemMessage::new(SystemMessageId::TheClanSCrestHasBeenDeleted),
                );
            }
        } else {
            if level < 3 {
                self.send(
                    player,
                    &SystemMessage::new(
                        SystemMessageId::AClanCrestCanOnlyBeRegisteredWhenTheClanSSkillLevelIs3OrAbove,
                    ),
                );
                return;
            }
            self.request_new_crest(
                player,
                data.unwrap_or_default(),
                CrestType::Pledge,
                CrestTarget::Pledge { clan_id },
            );
        }
    }

    /// Java `RequestExSetPledgeCrestLarge.runImpl`.
    pub(crate) fn on_request_ex_set_pledge_crest_large(
        &mut self,
        player: ObjectId,
        length: i32,
        data: Option<Vec<u8>>,
    ) {
        let Some(data) = data else {
            return;
        };
        if !self.players.contains_key(&player) {
            return;
        }
        let clan_id = self.clan_id_of(player);
        let Some(clan) = self.clan(clan_id) else {
            return;
        };
        let (dissolving, level, large_id) = (clan.dissolving_expiry_time, clan.level, clan.crest_large_id);
        if !(0..=2176).contains(&length) {
            self.send(
                player,
                &SystemMessage::new(
                    SystemMessageId::TheSizeOfTheUploadedCrestOrInsigniaDoesNotMeetTheStandardRequirements,
                ),
            );
            return;
        }
        if dissolving > unix_ms_now() {
            self.send(
                player,
                &SystemMessage::new(
                    SystemMessageId::AsYouAreCurrentlyScheduleForClanDissolutionYouCannotRegisterOrDeleteAClanCrest,
                ),
            );
            return;
        }
        if !self.has_clan_access(player, ClanAccess::ChangeCrest) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouAreNotAuthorizedToDoThat),
            );
            return;
        }
        if length == 0 {
            if large_id != 0 {
                self.change_large_crest(clan_id, 0);
                self.send(
                    player,
                    &SystemMessage::new(SystemMessageId::TheClanSCrestHasBeenDeleted),
                );
            }
        } else {
            if level < 3 {
                self.send(
                    player,
                    &SystemMessage::new(
                        SystemMessageId::AClanCrestCanOnlyBeRegisteredWhenTheClanSSkillLevelIs3OrAbove,
                    ),
                );
                return;
            }
            self.request_new_crest(
                player,
                data,
                CrestType::PledgeLarge,
                CrestTarget::PledgeLarge { clan_id },
            );
        }
    }

    /// Java `RequestSetAllyCrest.runImpl`.
    pub(crate) fn on_request_set_ally_crest(&mut self, player: ObjectId, length: i32, data: Option<Vec<u8>>) {
        if !self.players.contains_key(&player) {
            return;
        }
        if length < 0 {
            self.send_message(player, "File transfer error.");
            return;
        }
        if length > 192 {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::PleaseAdjustTheImageSizeTo8x12),
            );
            return;
        }
        let ally_id = self.ally_id_of(player);
        if ally_id == 0 {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::ThisFeatureIsOnlyAvailableToAllianceLeaders),
            );
            return;
        }
        // `ClanTable.getClan(player.getAllyId())`: the clan with the id of the alliance.
        let Some(leader_clan) = self.clan(ally_id) else {
            // Java dereferences null here; the packet is dropped.
            return;
        };
        let (leader_id, leader_crest) = (leader_clan.id, leader_clan.ally_crest_id);
        if self.clan_id_of(player) != leader_id || !self.is_clan_leader(player) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::ThisFeatureIsOnlyAvailableToAllianceLeaders),
            );
            return;
        }
        if length == 0 {
            if leader_crest != 0 {
                self.change_ally_crest(leader_id, 0, false);
            }
        } else {
            self.request_new_crest(
                player,
                data.unwrap_or_default(),
                CrestType::Ally,
                CrestTarget::Ally {
                    leader_clan_id: leader_id,
                },
            );
        }
    }
}
