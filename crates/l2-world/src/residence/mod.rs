//! Residences: castles, fortresses and clan halls (M7-RES-01). Their owners, the castle tax and
//! treasury, the residence functions and their fees, the residence doors and zones, the
//! residential skills of the owners' members, the castle circlets, and the re-read of a clan's
//! residences at `CharacterSelect` (D-50, D-51).
// Java: managers/{CastleManager,FortManager}.java
// Java: mechanics/siege/Castle.java (ownership, tax, treasury, functions, doors; not the siege)
// Java: mechanics/siege/Fort.java (ownership, functions, doors, the flag pole; not the siege,
//       the fort spawns or `FortUpdater`)
// Java: entity/residences/{AbstractResidence,ClanHall,AuctionableHall}.java (not the rent)
// Java: data/sql/ClanHallTable.java, managers/CHSiegeManager.java (the hall rows)
// Design: docs/design/m7-residences.md
//
// # Shape
// The residences live in [`ResidenceWorld`] (`World::residences`): the castles and fortresses in
// Java's list order (`ORDER BY id`, which is also the order Java's `getCastleIndex` walks), the
// clan halls (auctionable and conquerable, Java `ClanHallTable.getAllClanHalls()`) by id. A
// residence refers to its owner by clan id, to its doors by object id and to its zones by zone id
// (D-11). Ids of the residences themselves are [`CastleId`], [`FortId`] and [`ClanHallId`]; code
// that works on any of them takes a [`ResidenceRef`].
//
// They are loaded at startup from the rows the game server read ([`ResidenceLoad`], Java's
// `CastleManager.loadInstances`, `FortManager.loadInstances`, `ClanHallTable`, `CHSiegeManager`),
// after the clans, the doors and the zones. Every row Java writes goes out as
// `PersistCommand::Residence` with Java's statement ([`ResidenceWrite`]).
//
// # Side by side with the Java game server (D-39, D-49, D-50, D-51)
// Sieghardt (the Java GS) runs the real sieges and charges the real fees while both servers share
// the database. So the writes of the jobs that change residences on their own go out only with
// `SiegeDatabaseWrites = True` in `config/SideBySide.ini` ([`World::siege_write`]):
// - an owner change (`setOwner`, `removeOwner`, a hall set free), and what Java writes with it
//   (the `hasCastle` columns, the functions it drops, the circlets of offline members);
// - the castle treasury (`addToTreasuryNoTax`: the tax of every shop and multisell);
// - a function fee the clan warehouse pays (`FunctionTask` with `cwh`): without the switch the
//   task only moves the paid time on in this world; it neither charges nor drops the function.
// What a player does himself (buying a function at a chamberlain or hall manager, a castle lord
// setting the tax) is written like any other player action. The world changes its own state
// either way, so a GM test siege works end to end without the switch.
//
// The owners Sieghardt decided are read again at `CharacterSelect` for the clan of the character
// (`PlayerProfile::residence_refresh`, applied in `on_entered`, see `refresh.rs`), and any
// residence can be read again on demand through the outbox ([`World::request_residence_reread`],
// answered by `WorldCommand::ResidenceReread`). A failed read changes nothing.
//
// # Left to other cards (each has its marker where it belongs)
// - Sieges of every kind (`Castle.getSiege()`, `Fort.getSiege()`, the siege zones' activity,
//   `midVictory`, siege skills, siege guards, merc tickets, door and trap upgrades): M7-SIEGE-01,
//   M7-CHSIEGE-01 (the fortress siege itself is in `fort/`) (`// M7-SIEGE-01:` and friends).
// - (done in `fort/`, M7-FORT-01) `FortUpdater`, the fort spawns (`fort_spawnlist`), envoys and the castle contracts.
// - The clan hall rent (`AuctionableHall.FeeTask`) and the auctions: M7-CH-01 (`clanhall/`, done).
// - The schedule of the conquerable halls (`SiegableHall.updateNextSiege`): M7-CHSIEGE-01.
// - The manor reset of a castle that loses its owner (`CastleManorManager.resetManorData`) is
//   `manor/lists.rs` (M7-MANOR-01). The territory ward skills of a castle
//   (`TerritoryWarManager`): M7-TW-01.
// - The restart points of the residences (`MapRegionData.getTeleToLocation` with a castle, fort
//   or clan hall): the death and respawn wire-ins of M7-SIEGE-01.
// - Seven Signs' `CastleManager.validateTaxes` (M8).

mod castle;
mod clan_hall;
mod fort;
mod functions;
mod load;
pub mod npc;
mod refresh;
mod skills;

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use l2_data::SiegeSchedule;
use l2_util::ObjectId;

pub use castle::Castle;
pub use clan_hall::{ClanHall, ClanHallKind};
pub use fort::Fort;
pub use functions::{
    CASTLE_FUNC_RESTORE_EXP, CASTLE_FUNC_RESTORE_HP, CASTLE_FUNC_RESTORE_MP, CASTLE_FUNC_SUPPORT,
    CASTLE_FUNC_TELEPORT, HALL_FUNC_DECO_CURTAINS, HALL_FUNC_DECO_FRONTPLATEFORM, HALL_FUNC_ITEM_CREATE,
    HALL_FUNC_RESTORE_EXP, HALL_FUNC_RESTORE_HP, HALL_FUNC_RESTORE_MP, HALL_FUNC_SUPPORT, HALL_FUNC_TELEPORT,
    ResidenceFunction,
};
pub use load::{CastleRecord, ClanHallRecord, FortRecord, FunctionRecord, ResidenceLoad, SiegableHallRecord};
pub use refresh::{ResidenceReadKind, ResidenceReadThen, ResidenceRefresh, ResidenceRereadRows};

use crate::outbox::PersistCommand;
use crate::world::World;

/// A castle id (Java `Castle.getResidenceId()`, 1 to 9 in H5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CastleId(pub i32);

/// A fortress id (Java `Fort.getResidenceId()`, 101 to 121 in H5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FortId(pub i32);

/// A clan hall id (Java `ClanHall.getId()`): the auctionable halls and the conquerable ones
/// (21, 34, 35, 62, 63, 64) share one id space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClanHallId(pub i32);

impl fmt::Display for CastleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "castle {}", self.0)
    }
}

impl fmt::Display for FortId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "fortress {}", self.0)
    }
}

impl fmt::Display for ClanHallId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "clan hall {}", self.0)
    }
}

/// Any residence (Java `AbstractResidence` for castles and forts, `ClanHall` for halls).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResidenceRef {
    Castle(CastleId),
    Fort(FortId),
    ClanHall(ClanHallId),
}

impl ResidenceRef {
    /// Java `getResidenceId()` / `ClanHall.getId()`.
    pub fn residence_id(self) -> i32 {
        match self {
            Self::Castle(id) => id.0,
            Self::Fort(id) => id.0,
            Self::ClanHall(id) => id.0,
        }
    }
}

impl fmt::Display for ResidenceRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Castle(id) => id.fmt(f),
            Self::Fort(id) => id.fmt(f),
            Self::ClanHall(id) => id.fmt(f),
        }
    }
}

/// Who decides whether a residence row is written (D-49, D-51).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Writes {
    /// What a player does himself (a function bought, a tax rate set, a deposit to the
    /// treasury): always written.
    Player,
    /// A siege-like job (an owner change, the tax income, a fee the clan warehouse pays): only
    /// with `SiegeDatabaseWrites`.
    Job,
    /// Not written: the job of another switch that is off (a clan dissolution without
    /// `ClanMaintenanceDatabaseWrites`).
    Never,
}

/// The configuration values the residence code reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResidenceSettings {
    /// `SiegeDatabaseWrites` of `config/SideBySide.ini` (D-51): the residence and siege jobs
    /// write their global rows (owner changes, the treasury, fees the clan warehouse pays).
    pub siege_database_writes: bool,
    /// `RemoveCastleCirclets` (`Player.ini`).
    pub remove_castle_circlets: bool,
    /// `TakeCastlePoints` (`Feature.ini`).
    pub take_castle_points: i32,
    /// `LooseCastlePoints` (`Feature.ini`).
    pub loose_castle_points: i32,
    /// `CastleDefendedPoints` (`Feature.ini`).
    pub castle_defended_points: i32,
    /// `TakeFortPoints` (`Feature.ini`).
    pub take_fort_points: i32,
    /// `LooseFortPoints` (`Feature.ini`).
    pub loose_fort_points: i32,
    /// `FortressMaxSupplyLevel` (`Feature.ini`).
    pub fs_max_supply_level: i32,
}

impl Default for ResidenceSettings {
    /// Java's defaults; side by side (no global writes).
    fn default() -> Self {
        Self {
            siege_database_writes: false,
            remove_castle_circlets: true,
            take_castle_points: 1500,
            loose_castle_points: 3000,
            castle_defended_points: 750,
            take_fort_points: 200,
            loose_fort_points: 0,
            fs_max_supply_level: 6,
        }
    }
}

/// The residence state of the world.
#[derive(Debug, Default)]
pub struct ResidenceWorld {
    pub(crate) settings: ResidenceSettings,
    /// Java `CastleManager._castles`, in `ORDER BY id` order (Java's castle index is the position).
    pub(crate) castles: Vec<Castle>,
    /// Java `FortManager._forts`, in `ORDER BY id` order.
    pub(crate) forts: Vec<Fort>,
    /// Java `ClanHallTable.getAllClanHalls()`: the auctionable and the conquerable halls.
    pub(crate) clan_halls: BTreeMap<ClanHallId, ClanHall>,
    /// `config/SiegeSchedule.xml` (Java `SiegeScheduleData`), for the siege card.
    pub(crate) siege_schedule: Arc<SiegeSchedule>,
    /// The function objects (see `functions.rs`), by their own key.
    pub(crate) function_objects: BTreeMap<u64, functions::FunctionTask>,
    pub(crate) next_function_task: u64,
}

/// The residence timers (one `TimerKind::Residence`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidenceTimer {
    /// Java `CastleFunction.FunctionTask` (and the fort and clan hall ones): the fee of one
    /// function object is due. The key names the object in `ResidenceWorld::function_tasks`.
    FunctionFee(u64),
}

/// A residence row write, with Java's statement (`PersistCommand::Residence`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResidenceWrite {
    /// `Castle.addToTreasuryNoTax`: `UPDATE castle SET treasury = ? WHERE id = ?`.
    CastleTreasury { castle_id: i32, treasury: i64 },
    /// `Castle.setTaxPercent`: `UPDATE castle SET taxPercent = ? WHERE id = ?`.
    CastleTaxPercent { castle_id: i32, tax_percent: i32 },
    /// `Castle.updateShowNpcCrest`: `UPDATE castle SET showNpcCrest = ? WHERE id = ?`.
    CastleShowNpcCrest { castle_id: i32, show: bool },
    /// `Castle.setTicketBuyCount`: `UPDATE castle SET ticketBuyCount = ? WHERE id = ?`.
    CastleTicketBuyCount { castle_id: i32, count: i32 },
    /// `Castle.updateOwnerInDB`: `UPDATE clan_data SET hasCastle = 0 WHERE hasCastle = ?`, then
    /// `UPDATE clan_data SET hasCastle = ? WHERE clan_id = ?` (0 for no owner).
    CastleOwner { castle_id: i32, clan_id: i32 },
    /// `Fort.updateOwnerInDB`: `UPDATE fort SET owner=?,lastOwnedTime=?,state=?,castleId=? WHERE
    /// id = ?` (state and castleId 0).
    FortOwner {
        fort_id: i32,
        owner: i32,
        last_owned_time: i64,
    },
    /// `Fort.setFortState`: `UPDATE fort SET state=?,castleId=? WHERE id = ?`.
    FortState {
        fort_id: i32,
        state: i32,
        castle_id: i32,
    },
    /// `Fort.saveFortVariables`: `UPDATE fort SET supplyLvL=? WHERE id = ?`.
    FortSupplyLevel { fort_id: i32, supply_level: i32 },
    /// `AuctionableHall.updateDb`: `UPDATE clanhall SET ownerId=?, paidUntil=?, paid=? WHERE id=?`.
    ClanHall {
        hall_id: i32,
        owner_id: i32,
        paid_until: i64,
        paid: bool,
    },
    /// `SiegableHall.updateDb`: `UPDATE siegable_clanhall SET ownerId=?, nextSiege=? WHERE
    /// clanHallId=?`.
    SiegableHall {
        hall_id: i32,
        owner_id: i32,
        next_siege: i64,
    },
    /// `CastleFunction.dbSave` (and the fort and hall ones): `REPLACE INTO <table>_functions`.
    FunctionSave {
        residence: ResidenceRef,
        function_type: i32,
        lvl: i32,
        lease: i32,
        rate: i64,
        end_time: i64,
    },
    /// `Castle.removeFunction` (and the fort and hall ones): `DELETE FROM <table>_functions WHERE
    /// <id>=? AND type=?`.
    FunctionDelete {
        residence: ResidenceRef,
        function_type: i32,
    },
    /// `CastleManager.removeCirclet` of an offline member: `DELETE FROM items WHERE owner_id = ?
    /// and item_id = ?`.
    DeleteCirclets { owner_id: i32, item_id: i32 },
    /// `Siege.saveSiegeDate`: `UPDATE castle SET siegeDate = ?, regTimeEnd = ?, regTimeOver = ?
    /// WHERE id = ?` (M7-SIEGE-01).
    CastleSiegeDate {
        castle_id: i32,
        siege_date: i64,
        reg_time_end: i64,
        reg_time_over: bool,
    },
    /// `Siege.saveSiegeClan` (a new registration): `INSERT INTO siege_clans
    /// (clan_id,castle_id,type,castle_owner) values (?,?,?,0)`.
    SiegeClanInsert { clan_id: i32, castle_id: i32, kind: i32 },
    /// `Siege.saveSiegeClan` (an approval): `UPDATE siege_clans SET type = ? WHERE castle_id = ? AND
    /// clan_id = ?`.
    SiegeClanUpdate { castle_id: i32, clan_id: i32, kind: i32 },
    /// `Siege.removeSiegeClan(clanId)`: `DELETE FROM siege_clans WHERE castle_id=? and clan_id=?`.
    SiegeClanDelete { castle_id: i32, clan_id: i32 },
    /// `Siege.clearSiegeClan`: `DELETE FROM siege_clans WHERE castle_id=?`.
    SiegeClansDeleteCastle { castle_id: i32 },
    /// `Siege.clearSiegeClan` of an owned castle: `DELETE FROM siege_clans WHERE clan_id=?`.
    SiegeClansDeleteClan { clan_id: i32 },
    /// `Siege.clearSiegeWaitingClan`: `DELETE FROM siege_clans WHERE castle_id=? and type = 2`.
    SiegeClansDeleteWaiting { castle_id: i32 },
    /// `SiegeGuardManager.saveSiegeGuard`: `Insert Into castle_siege_guards (castleId, npcId, x,
    /// y, z, heading, respawnDelay, isHired) Values (?, ?, ?, ?, ?, ?, ?, ?)`.
    SiegeGuardInsert(crate::siege::SiegeGuardRecord),
    /// `SiegeGuardManager.removeMerc`: `Delete From castle_siege_guards Where npcId = ? And x = ?
    /// AND y = ? AND z = ? AND isHired = 1`.
    SiegeGuardDeleteHired { npc_id: i32, x: i32, y: i32, z: i32 },
    /// `SiegeGuardManager.removeMercs`: `Delete From castle_siege_guards Where castleId = ? And
    /// isHired = 1`.
    SiegeGuardsDeleteHired { castle_id: i32 },
    /// `Castle.setDoorUpgrade(.., save)`: `REPLACE INTO castle_doorupgrade (doorId, ratio,
    /// castleId) values (?,?,?)`.
    CastleDoorUpgrade {
        door_id: i32,
        ratio: i32,
        castle_id: i32,
    },
    /// `Castle.removeDoorUpgrade`: `DELETE FROM castle_doorupgrade WHERE castleId=?`.
    CastleDoorUpgradesDelete { castle_id: i32 },
    /// `Castle.setTrapUpgrade(.., save)`: `REPLACE INTO castle_trapupgrade (castleId, towerIndex,
    /// level) values (?,?,?)`.
    CastleTrapUpgrade {
        castle_id: i32,
        tower_index: i32,
        level: i32,
    },
    /// `Castle.removeTrapUpgrade`: `DELETE FROM castle_trapupgrade WHERE castleId=?`.
    CastleTrapUpgradesDelete { castle_id: i32 },
    /// `Clan.updateBloodAllianceCountInDB`: `UPDATE clan_data SET blood_alliance_count=? WHERE
    /// clan_id=?`.
    ClanBloodAllianceCount { clan_id: i32, count: i32 },
    /// The fortress siege tables and the blood oath count (M7-FORT-01, `fort/`).
    FortSiege(crate::fort::FortSiegeWrite),
    /// A write of the clan hall auction tables (`auction`, `auction_bid`, `clan_data.
    /// auction_bid_at`), see `clanhall/` (M7-CH-01).
    ClanHallAuction(crate::clanhall::AuctionWrite),
}

impl World {
    /// Installs the residence configuration. Call before the residences are loaded.
    pub fn set_residence_settings(&mut self, settings: ResidenceSettings) {
        self.residences.settings = settings;
    }

    /// The residence configuration.
    pub fn residence_settings(&self) -> &ResidenceSettings {
        &self.residences.settings
    }

    /// `SiegeDatabaseWrites` of `SideBySide.ini` (D-51): whether the siege-like jobs write their
    /// global rows. The siege, fortress, clan hall, manor and territory cards ask this.
    pub fn siege_database_writes(&self) -> bool {
        self.residences.settings.siege_database_writes
    }

    /// Installs `config/SiegeSchedule.xml` (Java `SiegeScheduleData`).
    pub fn set_siege_schedule(&mut self, schedule: Arc<SiegeSchedule>) {
        self.residences.siege_schedule = schedule;
    }

    /// Java `SiegeScheduleData.getInstance()`.
    pub fn siege_schedule(&self) -> &SiegeSchedule {
        &self.residences.siege_schedule
    }

    /// Hands a residence row write to the persistence task: what a player does himself and
    /// what Java writes outside of the siege-like jobs.
    pub(crate) fn residence_write(&mut self, write: ResidenceWrite) {
        self.outbox.persist(PersistCommand::Residence(write));
    }

    /// Hands a residence row write of a siege-like job to the persistence task, only with
    /// `SiegeDatabaseWrites` (D-51). Returns whether it went out. The siege cards use this for
    /// every global write of theirs.
    pub fn siege_write(&mut self, write: ResidenceWrite) -> bool {
        if self.residences.settings.siege_database_writes {
            self.residence_write(write);
            true
        } else {
            tracing::debug!("SideBySide: not written (SiegeDatabaseWrites = False): {write:?}");
            false
        }
    }

    /// Hands a residence row write to the persistence task as `writes` says. Returns whether it
    /// went out.
    pub fn write_residence(&mut self, writes: Writes, write: ResidenceWrite) -> bool {
        match writes {
            Writes::Player => {
                self.residence_write(write);
                true
            }
            Writes::Job => self.siege_write(write),
            Writes::Never => false,
        }
    }

    /// The residence timers.
    pub(crate) fn on_residence_timer(&mut self, timer: ResidenceTimer) {
        match timer {
            ResidenceTimer::FunctionFee(key) => self.on_function_fee(key),
        }
    }

    /// The name of a residence (Java `getName()`), empty for an unknown one.
    pub fn residence_name(&self, residence: ResidenceRef) -> String {
        match residence {
            ResidenceRef::Castle(id) => self.castle(id).map(|c| c.name.clone()),
            ResidenceRef::Fort(id) => self.fort(id).map(|f| f.name.clone()),
            ResidenceRef::ClanHall(id) => self.clan_hall(id).map(|h| h.name.clone()),
        }
        .unwrap_or_default()
    }

    /// The owning clan id of a residence (0 for none or an unknown residence).
    pub fn residence_owner_id(&self, residence: ResidenceRef) -> i32 {
        match residence {
            ResidenceRef::Castle(id) => self.castle(id).map_or(0, |c| c.owner_id),
            ResidenceRef::Fort(id) => self.fort(id).map_or(0, |f| f.owner_id),
            ResidenceRef::ClanHall(id) => self.clan_hall(id).map_or(0, |h| h.owner_id),
        }
    }

    /// The doors of a residence (object ids; Java `getDoors()`).
    pub fn residence_doors(&self, residence: ResidenceRef) -> Vec<ObjectId> {
        match residence {
            ResidenceRef::Castle(id) => self.castle(id).map(|c| c.doors.clone()),
            ResidenceRef::Fort(id) => self.fort(id).map(|f| f.doors.clone()),
            ResidenceRef::ClanHall(id) => self.clan_hall(id).map(|h| h.doors.clone()),
        }
        .unwrap_or_default()
    }

    /// Java `getDoor(doorId)`: the residence's door with this template id (`None` for 0 or less).
    pub fn residence_door(&self, residence: ResidenceRef, door_id: i32) -> Option<ObjectId> {
        if door_id <= 0 {
            return None;
        }
        self.residence_doors(residence)
            .into_iter()
            .find(|d| self.door(*d).is_some_and(|door| door.door_id == door_id))
    }

    /// The residence zone of a castle (`CastleZone`), fortress (`FortZone`) or clan hall, as
    /// `(zone id, zone type)`.
    fn residence_zone_key(&self, residence: ResidenceRef) -> Option<(i32, String)> {
        match residence {
            ResidenceRef::Castle(id) => self.castle(id)?.zone.map(|z| (z, "CastleZone".to_owned())),
            ResidenceRef::Fort(id) => self.fort(id)?.zone.map(|z| (z, "FortZone".to_owned())),
            ResidenceRef::ClanHall(id) => self.clan_hall(id)?.zone.clone(),
        }
    }

    /// The players a zone holds (Java `getPlayersInside()`), by object id.
    pub(crate) fn players_inside_zone(&self, zone_id: i32, zone_type: &str) -> Vec<ObjectId> {
        let mut inside: Vec<ObjectId> = self
            .players
            .iter()
            .filter(|(_, p)| {
                p.zones
                    .members()
                    .iter()
                    .any(|(id, ty)| *id == zone_id && ty == zone_type)
            })
            .map(|(id, _)| *id)
            .collect();
        inside.sort_unstable();
        inside
    }

    /// `ZoneRespawn`'s `Rnd.get(size)` with `RandomRespawnInTownEnabled`, else the first point.
    fn residence_spawn_pick(&mut self, len: usize) -> usize {
        if self.settings.teleport.random_respawn_in_town_enabled {
            usize::try_from(self.rng().below(i32::try_from(len).unwrap_or(i32::MAX))).unwrap_or(0)
        } else {
            0
        }
    }

    /// Java `banishForeigners()` of a castle, fortress or clan hall (`ResidenceZone
    /// .banishForeigners(ownerId)`): every player inside the residence zone who is not of the
    /// owning clan goes to one of the zone's banish points (`teleToLocation(loc, true)`).
    pub fn residence_banish_foreigners(&mut self, residence: ResidenceRef) {
        let Some((zone_id, zone_type)) = self.residence_zone_key(residence) else {
            if let ResidenceRef::ClanHall(id) = residence {
                tracing::warn!(
                    "ClanHall: Zone is null for clan hall: {} {}",
                    id.0,
                    self.residence_name(residence)
                );
            }
            return;
        };
        let Some(zone) = self
            .zoning
            .zones
            .as_ref()
            .and_then(|zones| zones.get_of_type(zone_id, &zone_type))
            .cloned()
        else {
            return;
        };
        let owner = self.residence_owner_id(residence);
        let offset = self.settings.teleport.max_offset_on_teleport;
        for player in self.players_inside_zone(zone_id, &zone_type) {
            if owner != 0 && self.clan_id_of(player) == owner {
                continue;
            }
            let loc = {
                let mut pick = |len: usize| self.residence_spawn_pick(len);
                zone.banish_spawn_loc(&mut pick)
            };
            let Some(loc) = loc else {
                continue;
            };
            self.tele_to_location(
                player,
                crate::teleport::TeleportTarget::at(loc.x, loc.y, loc.z).with_random_offset(offset),
            );
        }
    }

    /// Java `Castle.oustAllPlayers()` (`ResidenceTeleportZone.oustAllPlayers`): every player in the
    /// castle's teleport zone goes to one of its points (`teleToLocation(getSpawnLoc(), 200)`).
    pub fn castle_oust_all_players(&mut self, id: CastleId) {
        let Some(zone_id) = self.castle(id).and_then(|c| c.teleport_zone) else {
            return;
        };
        let Some(zone) = self
            .zoning
            .zones
            .as_ref()
            .and_then(|zones| zones.get_of_type(zone_id, "ResidenceTeleportZone"))
            .cloned()
        else {
            return;
        };
        for player in self.players_inside_zone(zone_id, "ResidenceTeleportZone") {
            let loc = {
                let mut pick = |len: usize| self.residence_spawn_pick(len);
                zone.spawn_loc(&mut pick)
            };
            let Some(loc) = loc else {
                continue;
            };
            self.tele_to_location(
                player,
                crate::teleport::TeleportTarget::at(loc.x, loc.y, loc.z).with_random_offset(200),
            );
        }
    }

    /// Java `openCloseDoor(player, doorId, open)` of a castle, fortress or clan hall: only a
    /// member of the owning clan opens or closes the residence's own doors.
    pub fn residence_open_close_door(
        &mut self,
        player: ObjectId,
        residence: ResidenceRef,
        door_id: i32,
        open: bool,
    ) {
        let owner = self.residence_owner_id(residence);
        let clan_id = self.clan_id_of(player);
        let allowed = match residence {
            // `player.getClanId() != _ownerId`.
            ResidenceRef::Castle(_) => clan_id == owner,
            // `player.getClan() != _fortOwner`.
            // NOTE(java-quirk): a player without a clan passes at a fortress without an owner
            // (`null == null`).
            ResidenceRef::Fort(_) => self.clan_of(player).map_or(0, |c| c.id) == owner,
            // `player.getClanId() == _ownerId`.
            ResidenceRef::ClanHall(_) => clan_id == owner,
        };
        if !allowed {
            return;
        }
        if let Some(door) = self.residence_door(residence, door_id) {
            if open {
                self.open_door(door);
            } else {
                self.close_door(door);
            }
        }
    }
}
