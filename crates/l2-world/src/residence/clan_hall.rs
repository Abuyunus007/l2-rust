//! Clan halls: the hall table (auctionable and conquerable halls), the owner, the doors, the
//! zone and its decorations (`AgitDecoInfo`).
// Java: data/sql/ClanHallTable.java (setFree, setOwner, getClanHallByOwner, getAbstractHallByOwner,
//       getClanHall(x, y, z), getNearbyClanHall, getNearbyAbstractHall)
// Java: entity/residences/ClanHall.java (setOwner, free, openCloseDoor(s), banishForeigners)
// Java: entity/residences/AuctionableHall.java (setOwner, free, updateDb; not the rent)
// Java: mechanics/siege/clanhalls/SiegableHall.java (updateDb; not the siege)
// Java: entity/zone/type/ClanHallZone.java (onEnter: CLAN_HALL and AgitDecoInfo)
// Java: network/serverpackets/AgitDecoInfo.java (the decoration levels)
//
// The rent of an auctionable hall (`AuctionableHall.FeeTask`, the "please pay" messages and the
// loss of a hall a week overdue) and the auctions are `clanhall/` (M7-CH-01); the conquerable
// halls' siege and schedule are `clanhall/siege/` (M7-CHSIEGE-01).

use std::collections::BTreeMap;

use l2_protocol_game::server::AgitDecoInfo;
use l2_util::ObjectId;

use super::functions::{
    HALL_FUNC_DECO_CURTAINS, HALL_FUNC_DECO_FRONTPLATEFORM, HALL_FUNC_ITEM_CREATE, HALL_FUNC_RESTORE_EXP,
    HALL_FUNC_RESTORE_HP, HALL_FUNC_RESTORE_MP, HALL_FUNC_SUPPORT, HALL_FUNC_TELEPORT,
};
use super::{ClanHallId, ResidenceRef, ResidenceWrite, Writes};
use crate::world::{World, unix_ms_now};

/// What kind of hall, with the values only that kind has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClanHallKind {
    /// Java `AuctionableHall` (the `clanhall` table).
    Auctionable {
        /// `_lease`: the weekly rent.
        lease: i32,
        /// `_paidUntil` (epoch milliseconds).
        paid_until: i64,
        /// `_grade`.
        grade: i32,
        /// `_paid`.
        paid: bool,
    },
    /// Java `SiegableHall` (the `siegable_clanhall` table, conquered by siege).
    Siegable {
        /// `nextSiege` (epoch milliseconds); `clanhall/siege/` moves it (`updateNextSiege`).
        next_siege: i64,
        /// `_siegeLength` (milliseconds).
        siege_length: i64,
        /// `schedule_config` as read (`DD;MM;YY;HH;mm`).
        schedule_config: Option<String>,
    },
}

/// A clan hall (Java `ClanHall`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClanHall {
    pub id: ClanHallId,
    pub name: String,
    /// `_ownerId`: the owning clan, 0 for none.
    pub owner_id: i32,
    pub desc: String,
    pub location: String,
    /// `_isFree`: no function task runs while it is set.
    pub is_free: bool,
    /// `_functions`: the current function objects by type (see `functions.rs`).
    pub(crate) functions: BTreeMap<i32, u64>,
    /// `getDoors()`: the doors whose template names this hall (`clanhall_id`).
    pub doors: Vec<ObjectId>,
    /// The `ClanHallZone` (or `SiegableHallZone`) that registered with the hall, as `(zone id,
    /// zone type)`.
    pub zone: Option<(i32, String)>,
    pub kind: ClanHallKind,
}

impl ClanHall {
    /// Java `getGrade()` (0 for a conquerable hall).
    pub fn grade(&self) -> i32 {
        match self.kind {
            ClanHallKind::Auctionable { grade, .. } => grade,
            ClanHallKind::Siegable { .. } => 0,
        }
    }

    /// Java `getLease()` (0 for a conquerable hall).
    pub fn lease(&self) -> i32 {
        match self.kind {
            ClanHallKind::Auctionable { lease, .. } => lease,
            ClanHallKind::Siegable { .. } => 0,
        }
    }

    /// Java `getPaidUntil()` (0 for a conquerable hall).
    pub fn paid_until(&self) -> i64 {
        match self.kind {
            ClanHallKind::Auctionable { paid_until, .. } => paid_until,
            ClanHallKind::Siegable { .. } => 0,
        }
    }

    /// Java `AuctionableHall.getPaid()` (false for a conquerable hall).
    pub fn paid(&self) -> bool {
        matches!(self.kind, ClanHallKind::Auctionable { paid: true, .. })
    }

    /// Java `isSiegableHall()`.
    pub fn is_siegable_hall(&self) -> bool {
        matches!(self.kind, ClanHallKind::Siegable { .. })
    }
}

impl World {
    // ------------------------------------------------------------------ ClanHallTable lookups

    /// Java `ClanHallTable.getClanHallById(id)` (both kinds).
    pub fn clan_hall(&self, id: ClanHallId) -> Option<&ClanHall> {
        self.residences.clan_halls.get(&id)
    }

    pub(crate) fn clan_hall_mut(&mut self, id: ClanHallId) -> Option<&mut ClanHall> {
        self.residences.clan_halls.get_mut(&id)
    }

    /// Java `ClanHallTable.getAllClanHalls()`, by id.
    pub fn clan_halls(&self) -> impl Iterator<Item = &ClanHall> {
        self.residences.clan_halls.values()
    }

    /// Java `ClanHallTable.isFree(id)`: an auctionable hall without an owner.
    pub fn clan_hall_is_free(&self, id: ClanHallId) -> bool {
        self.clan_hall(id)
            .is_some_and(|h| !h.is_siegable_hall() && h.owner_id <= 0)
    }

    /// Java `ClanHallTable.getClanHallByOwner(clan)`: the auctionable hall the clan owns.
    pub fn auctionable_hall_by_owner(&self, clan_id: i32) -> Option<ClanHallId> {
        self.residences
            .clan_halls
            .values()
            .find(|h| !h.is_siegable_hall() && h.owner_id > 0 && h.owner_id == clan_id)
            .map(|h| h.id)
    }

    /// Java `ClanHallTable.getAbstractHallByOwner(clan)`: the auctionable hall the clan owns, else
    /// its conquerable hall.
    pub fn clan_hall_by_owner(&self, clan_id: i32) -> Option<ClanHallId> {
        self.auctionable_hall_by_owner(clan_id).or_else(|| {
            self.residences
                .clan_halls
                .values()
                .find(|h| h.is_siegable_hall() && h.owner_id == clan_id && clan_id != 0)
                .map(|h| h.id)
        })
    }

    /// The zone of a hall, from the zone table.
    pub(crate) fn clan_hall_zone(&self, id: ClanHallId) -> Option<&l2_data::Zone> {
        let (zone_id, zone_type) = self.clan_hall(id)?.zone.as_ref()?;
        self.zoning.zones.as_ref()?.get_of_type(*zone_id, zone_type)
    }

    /// Java `ClanHall.checkIfInZone(x, y, z)`.
    pub fn clan_hall_contains(&self, id: ClanHallId, x: i32, y: i32, z: i32) -> bool {
        self.clan_hall_zone(id)
            .is_some_and(|zone| zone.is_inside_zone(x, y, z))
    }

    /// Java `ClanHallTable.getClanHall(x, y, z)`: the first hall (by id) whose zone holds the point.
    pub fn clan_hall_at(&self, x: i32, y: i32, z: i32) -> Option<ClanHallId> {
        self.residences
            .clan_halls
            .values()
            .find(|h| self.clan_hall_contains(h.id, x, y, z))
            .map(|h| h.id)
    }

    /// Java `ClanHallTable.getNearbyClanHall(x, y, maxDist)`: an owned auctionable hall whose
    /// zone is closer than `max_distance`, else a free one.
    pub fn nearby_auctionable_hall(&self, x: i32, y: i32, max_distance: i32) -> Option<ClanHallId> {
        let near = |h: &&ClanHall| {
            self.clan_hall_zone(h.id)
                .is_some_and(|zone| zone.distance_to_zone(x, y) < f64::from(max_distance))
        };
        let auctionable = || {
            self.residences
                .clan_halls
                .values()
                .filter(|h| !h.is_siegable_hall())
        };
        auctionable()
            .filter(|h| h.owner_id > 0)
            .find(near)
            .or_else(|| auctionable().filter(|h| h.owner_id <= 0).find(near))
            .map(|h| h.id)
    }

    /// Java `ClanHallTable.getNearbyAbstractHall(x, y, maxDist)`: any hall whose zone is closer
    /// than `max_distance`.
    pub fn nearby_clan_hall(&self, x: i32, y: i32, max_distance: i32) -> Option<ClanHallId> {
        self.residences
            .clan_halls
            .values()
            .find(|h| {
                self.clan_hall_zone(h.id)
                    .is_some_and(|zone| zone.distance_to_zone(x, y) < f64::from(max_distance))
            })
            .map(|h| h.id)
    }

    // ------------------------------------------------------------------ owner

    /// Java `ClanHallTable.setOwner(chId, clan)`: an owned hall is freed first, then the clan
    /// gets it (`ClanHall.setOwner`) and its rent starts (`initialyzeTask(true)`).
    pub fn clan_hall_table_set_owner(&mut self, id: ClanHallId, clan_id: i32) {
        if self.clan_hall(id).is_some_and(|h| h.owner_id > 0) {
            self.clan_hall_free(id);
        }
        if let Some(clan) = self.clan_mut(clan_id) {
            clan.hideout_id = id.0;
        }
        self.clan_hall_set_owner(id, clan_id);
    }

    /// Java `ClanHall.setOwner(clan)` (and `AuctionableHall.setOwner`): only a free hall changes
    /// hands; the clan gets the hall id and `PledgeShowInfoUpdate`, the row is written (D-51: only
    /// with `SiegeDatabaseWrites`).
    pub fn clan_hall_set_owner(&mut self, id: ClanHallId, clan_id: i32) {
        let Some(hall) = self.clan_hall(id) else {
            return;
        };
        // Verify that this ClanHall is Free and Clan isn't null.
        if hall.owner_id <= 0 && self.clan(clan_id).is_some() {
            if let Some(hall) = self.clan_hall_mut(id) {
                hall.owner_id = clan_id;
                hall.is_free = false;
            }
            if let Some(clan) = self.clan_mut(clan_id) {
                clan.hideout_id = id.0;
            }
            // Announce to Online member new ClanHall.
            self.broadcast_clan_info_update(clan_id);
            let written = self.clan_hall_update_db(id, Writes::Job);
            self.set_residence_owner_source(ResidenceRef::ClanHall(id), written);
        }
        // `AuctionableHall.setOwner`: `_paidUntil = now` even when the hall was not free.
        if let Some(ClanHallKind::Auctionable { paid_until, .. }) =
            self.clan_hall_mut(id).map(|h| &mut h.kind)
        {
            *paid_until = unix_ms_now();
            self.clan_hall_initialize_rent_task(id, true);
        }
    }

    /// Java `ClanHallTable.setFree(chId)`: the clan loses the hall id, the hall is free.
    pub fn clan_hall_table_set_free(&mut self, id: ClanHallId) {
        let Some(owner) = self.clan_hall(id).map(|h| h.owner_id) else {
            return;
        };
        if let Some(clan) = self.clan_mut(owner) {
            clan.hideout_id = 0;
        }
        self.clan_hall_free(id);
    }

    /// Java `ClanHall.free()` (and `AuctionableHall.free()`): no owner, the functions go, the row
    /// is written (D-51: only with `SiegeDatabaseWrites`).
    pub fn clan_hall_free(&mut self, id: ClanHallId) {
        self.clan_hall_free_with(id, Writes::Job);
    }

    /// [`Self::clan_hall_free`] with the rows written as `writes` says.
    pub(crate) fn clan_hall_free_with(&mut self, id: ClanHallId, writes: Writes) {
        if let Some(hall) = self.clan_hall_mut(id) {
            hall.owner_id = 0;
            hall.is_free = true;
        } else {
            return;
        }
        // A free hall pays no rent (the task would find `isFree()` and end).
        self.clan_hall_cancel_rent_task(id);
        self.residence_remove_all_functions_with(ResidenceRef::ClanHall(id), writes);
        // NOTE(java-quirk): `ClanHall.free()` writes the row before `AuctionableHall.free()` resets
        // `paidUntil` and `paid`, so the row keeps the old values with owner 0.
        self.clan_hall_update_db(id, writes);
        self.set_residence_owner_source(ResidenceRef::ClanHall(id), true);
        if let Some(ClanHallKind::Auctionable { paid_until, paid, .. }) =
            self.clan_hall_mut(id).map(|h| &mut h.kind)
        {
            *paid_until = 0;
            *paid = false;
        }
    }

    /// Java `updateDb()` of the hall's class (D-51: only with `SiegeDatabaseWrites`).
    pub(crate) fn clan_hall_update_db(&mut self, id: ClanHallId, writes: Writes) -> bool {
        let Some(hall) = self.clan_hall(id) else {
            return false;
        };
        let write = match &hall.kind {
            ClanHallKind::Auctionable { paid_until, paid, .. } => ResidenceWrite::ClanHall {
                hall_id: id.0,
                owner_id: hall.owner_id,
                paid_until: *paid_until,
                paid: *paid,
            },
            ClanHallKind::Siegable { next_siege, .. } => ResidenceWrite::SiegableHall {
                hall_id: id.0,
                owner_id: hall.owner_id,
                next_siege: *next_siege,
            },
        };
        self.write_residence(writes, write)
    }

    // ------------------------------------------------------------------ doors and zone

    /// Java `ClanHall.openCloseDoors(player, open)`: a member of the owning clan opens or closes
    /// every door of the hall.
    pub fn clan_hall_open_close_doors(&mut self, player: ObjectId, id: ClanHallId, open: bool) {
        let Some(owner) = self.clan_hall(id).map(|h| h.owner_id) else {
            return;
        };
        if self.clan_id_of(player) != owner {
            return;
        }
        for door in self.residence_doors(ResidenceRef::ClanHall(id)) {
            if open {
                self.open_door(door);
            } else {
                self.close_door(door);
            }
        }
    }

    /// Java `ClanHallZone.onEnter` after the `CLAN_HALL` flag: the decorations of an auctionable
    /// hall.
    pub(crate) fn on_clan_hall_zone_enter(&mut self, player: ObjectId, hall_id: i32) {
        if let Some(packet) = self.agit_deco_info(ClanHallId(hall_id)) {
            self.send(player, &packet);
        }
    }

    /// Java `new AgitDecoInfo(clanHall)`: the decoration bytes of an auctionable hall from its
    /// functions and grade (`None` for a conquerable hall, which Java does not send).
    pub fn agit_deco_info(&self, id: ClanHallId) -> Option<AgitDecoInfo> {
        let hall = self.clan_hall(id)?;
        if hall.is_siegable_hall() {
            return None;
        }
        let grade = hall.grade();
        let residence = ResidenceRef::ClanHall(id);
        // 0 without the function (or at level 0), 1 when `low(level)`, else 2.
        let deco = |function_type: i32, low: &dyn Fn(i32) -> bool| -> u8 {
            match self.residence_function(residence, function_type).map(|f| f.lvl) {
                None | Some(0) => 0,
                Some(lvl) if low(lvl) => 1,
                Some(_) => 2,
            }
        };
        let item_create = |lvl: i32| (grade == 0 && lvl < 2) || lvl < 3;
        Some(AgitDecoInfo {
            hall_id: id.0,
            // Fireplace
            fireplace: deco(HALL_FUNC_RESTORE_HP, &|lvl| {
                (grade == 0 && lvl < 220)
                    || (grade == 1 && lvl < 160)
                    || (grade == 2 && lvl < 260)
                    || (grade == 3 && lvl < 300)
            }),
            // Carpet - Statue
            carpet_statue: deco(HALL_FUNC_RESTORE_MP, &|lvl| {
                ((grade == 0 || grade == 1) && lvl < 25)
                    || (grade == 2 && lvl < 30)
                    || (grade == 3 && lvl < 40)
            }),
            // Chandelier
            chandelier: deco(HALL_FUNC_RESTORE_EXP, &|lvl| {
                (grade == 0 && lvl < 25)
                    || (grade == 1 && lvl < 30)
                    || (grade == 2 && lvl < 40)
                    || (grade == 3 && lvl < 50)
            }),
            // Mirror
            mirror: deco(HALL_FUNC_TELEPORT, &|lvl| lvl < 2),
            // Crystal
            crystal: 0,
            // Curtain
            curtain: deco(HALL_FUNC_DECO_CURTAINS, &|lvl| lvl <= 1),
            // Magic Curtain
            magic_curtain: deco(HALL_FUNC_ITEM_CREATE, &item_create),
            // Support? - Flag
            flag: deco(HALL_FUNC_SUPPORT, &|lvl| {
                (grade == 0 && lvl < 2)
                    || (grade == 1 && lvl < 4)
                    || (grade == 2 && lvl < 5)
                    || (grade == 3 && lvl < 8)
            }),
            // Front platform
            front_platform: deco(HALL_FUNC_DECO_FRONTPLATEFORM, &|lvl| lvl <= 1),
            // Item create?
            item_create: deco(HALL_FUNC_ITEM_CREATE, &item_create),
        })
    }
}
