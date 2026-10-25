//! Loading the residences at startup: the rows the game server read, in Java's order.
// Java: managers/CHSiegeManager.java (loadClanHalls), data/sql/ClanHallTable.java (load)
// Java: entity/residences/{ClanHall,AuctionableHall}.java (constructors)
// Java: managers/CastleManager.java (loadInstances, activateInstances)
// Java: mechanics/siege/Castle.java (constructor, load, loadDoor, initResidenceZone)
// Java: managers/FortManager.java (loadInstances, activateInstances)
// Java: mechanics/siege/Fort.java (constructor, load, loadFlagPoles, loadDoor, initResidenceZone)
// Java: entity/actor/instance/Door.java (constructor: the clan hall doors; getCastle, getFort)
// Java: entity/zone/type/ClanHallZone.java (setParameter: the zone registers with its hall)

use std::sync::Arc;

use l2_data::residence::{
    castle_zone_of, clan_hall_zone_of, fort_zone_of, residence_teleport_zone_of, siege_zone_of,
};
use l2_util::ObjectId;
use tracing::{info, warn};

use super::skills::residential_skills_of;
use super::{Castle, CastleId, ClanHall, ClanHallId, ClanHallKind, Fort, FortId, ResidenceRef};
use crate::world::World;

/// One row of a function table (`castle_functions`, `fort_functions`, `clanhall_functions`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FunctionRecord {
    pub function_type: i32,
    pub lvl: i32,
    pub lease: i32,
    pub rate: i64,
    pub end_time: i64,
}

/// A castle as `new Castle(id)` reads it: its `castle` row, its owner (the last clan whose
/// `hasCastle` names it) and, for an owned castle, its functions.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CastleRecord {
    pub id: i32,
    /// `false` when the `castle` row was not found (the fields keep Java's defaults).
    pub found: bool,
    pub name: String,
    pub tax_percent: i32,
    pub treasury: i64,
    pub siege_date: i64,
    /// `Boolean.parseBoolean(regTimeOver)`.
    pub time_registration_over: bool,
    pub time_registration_over_date: i64,
    /// `Boolean.parseBoolean(showNpcCrest)`.
    pub show_npc_crest: bool,
    pub ticket_buy_count: i32,
    /// `SELECT clan_id FROM clan_data WHERE hasCastle = ?` (the last row), 0 for none.
    pub owner_id: i32,
    /// `castle_functions` (read only for an owned castle).
    pub functions: Vec<FunctionRecord>,
    /// `clan_data.blood_alliance_count` of the owner, read again on demand (D-50: the Java game
    /// server adds to it); `None` at startup and for a castle without an owner.
    pub owner_blood_alliance_count: Option<i32>,
}

/// A fortress as `new Fort(id)` reads it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FortRecord {
    pub id: i32,
    pub name: String,
    pub siege_date: i64,
    pub last_owned_time: i64,
    /// `owner`, 0 for none.
    pub owner_id: i32,
    pub fort_type: i32,
    pub state: i32,
    pub castle_id: i32,
    pub supply_level: i32,
    /// `fort_functions` (read only for an owned fortress).
    pub functions: Vec<FunctionRecord>,
}

/// An auctionable hall (a `clanhall` row).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ClanHallRecord {
    pub id: i32,
    pub name: String,
    pub owner_id: i32,
    pub lease: i32,
    pub desc: String,
    pub location: String,
    pub paid_until: i64,
    pub grade: i32,
    pub paid: bool,
    /// `clanhall_functions` (read only for an owned hall).
    pub functions: Vec<FunctionRecord>,
}

/// A conquerable hall (a `siegable_clanhall` row; Java's `getInt` / `getLong` of NULL is 0).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SiegableHallRecord {
    pub id: i32,
    pub name: String,
    pub owner_id: i32,
    pub desc: String,
    pub location: String,
    pub next_siege: i64,
    pub siege_length: i64,
    pub schedule_config: Option<String>,
    /// `clanhall_functions` (read only for an owned hall).
    pub functions: Vec<FunctionRecord>,
}

/// Everything the game server read for the residences at startup (D-15: read before the world
/// runs, only reads, D-39).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResidenceLoad {
    /// `SELECT id FROM castle ORDER BY id`, each read like `new Castle(id)`.
    pub castles: Vec<CastleRecord>,
    /// `SELECT id FROM fort ORDER BY id`, each read like `new Fort(id)`.
    pub forts: Vec<FortRecord>,
    /// `SELECT * FROM clanhall ORDER BY id`.
    pub clan_halls: Vec<ClanHallRecord>,
    /// `SELECT * FROM siegable_clanhall`.
    pub siegable_halls: Vec<SiegableHallRecord>,
}

impl Castle {
    /// `new Castle(id)` up to `load()`: the row's values and the owner.
    pub(crate) fn from_record(record: &CastleRecord) -> Self {
        let mut castle = Castle::new(CastleId(record.id));
        if record.found {
            castle.name = record.name.clone();
            castle.siege_date = record.siege_date;
            castle.time_registration_over_date = record.time_registration_over_date;
            castle.time_registration_over = record.time_registration_over;
            castle.tax_percent = record.tax_percent;
            castle.treasury = record.treasury;
            castle.show_npc_crest = record.show_npc_crest;
            castle.ticket_buy_count = record.ticket_buy_count;
        }
        castle.tax_rate = f64::from(castle.tax_percent) / 100.0;
        castle.owner_id = record.owner_id;
        castle
    }
}

impl Fort {
    /// `new Fort(id)`'s `load()` without the owner (the world sets it when the clan exists).
    pub(crate) fn from_record(record: &FortRecord) -> Self {
        let mut fort = Fort::new(FortId(record.id));
        fort.name = record.name.clone();
        fort.siege_date = record.siege_date;
        fort.last_owned_time = record.last_owned_time;
        fort.fort_type = record.fort_type;
        fort.state = record.state;
        fort.castle_id = record.castle_id;
        fort.supply_level = record.supply_level;
        fort
    }
}

impl World {
    /// Java's residence managers at startup, in `GameServer`'s order: `CHSiegeManager` and
    /// `ClanHallTable` (after `ClanTable`), the clan hall doors (`DoorData`), the hall zones
    /// (`ZoneManager`), `CastleManager.loadInstances` and `activateInstances`, then
    /// `FortManager.loadInstances` and `activateInstances`. Call after the clans, the doors and
    /// static objects, the zones and the skill trees are installed.
    pub fn load_residences(&mut self, load: ResidenceLoad) {
        // Java: CHSiegeManager.loadClanHalls.
        let siegable = load.siegable_halls.len();
        for record in load.siegable_halls {
            self.install_siegable_hall(record);
        }
        info!("CHSiegeManager: Loaded {siegable} conquerable clan halls.");
        // Java: ClanHallTable.load.
        let (mut owned, mut free) = (0, 0);
        for record in load.clan_halls {
            if self.install_auctionable_hall(record) {
                owned += 1;
            } else {
                free += 1;
            }
        }
        info!("ClanHallTable: Loaded {owned} clan halls");
        info!("ClanHallTable: Loaded {free} free clan halls");
        self.bind_clan_hall_doors_and_zones();
        // `_loaded = true`: the rent and the auctions wait for it.
        self.clanhall.loaded = true;

        // Java: CastleManager.loadInstances.
        for record in &load.castles {
            self.install_castle(record);
        }
        info!("CastleManager: Loaded {} castles", self.residences.castles.len());
        // Java: CastleManager.activateInstances (`loadDoor`).
        self.bind_siege_zone_doors(true);

        // Java: FortManager.loadInstances. A fortress without a flag pole throws in Java and the
        // forts after it are not loaded either.
        for record in &load.forts {
            if !self.install_fort(record) {
                break;
            }
        }
        info!("FortManager: Loaded {} fortress", self.residences.forts.len());
        // Java: FortManager.activateInstances.
        self.bind_siege_zone_doors(false);
    }

    /// Java `new SiegableHall(set)` (through `ClanHall(set)`), then `ClanHallTable.addClanHall`.
    fn install_siegable_hall(&mut self, record: SiegableHallRecord) {
        let id = ClanHallId(record.id);
        let hall = ClanHall {
            id,
            name: record.name,
            owner_id: record.owner_id,
            desc: record.desc,
            location: record.location,
            is_free: true,
            functions: Default::default(),
            doors: Vec::new(),
            zone: None,
            kind: ClanHallKind::Siegable {
                next_siege: record.next_siege,
                siege_length: record.siege_length,
                schedule_config: record.schedule_config.clone(),
            },
        };
        self.residences.clan_halls.insert(id, hall);
        if record.owner_id > 0 {
            if let Some(clan) = self.clan_mut(record.owner_id) {
                clan.hideout_id = id.0;
            } else {
                // `free()` from the `ClanHall` constructor.
                // NOTE(java-quirk): `SiegableHall.updateDb` runs before `_nextSiege` exists, throws
                // and is logged, so Java writes no row here; neither does this.
                if let Some(hall) = self.clan_hall_mut(id) {
                    hall.owner_id = 0;
                }
                warn!("Exception: SiegableHall.updateDb(): the owner of {id} does not exist.");
            }
        }
        // `updateNextSiege()` when the next siege is due within an hour (a write, D-51).
        if record.next_siege - self.siege_now_ms() - 3_600_000 < 0 {
            let name = self.clan_hall(id).map(|h| h.name.clone()).unwrap_or_default();
            let config = crate::clanhall::siege::schedule::parse_schedule_config(
                &name,
                record.schedule_config.as_deref(),
            );
            self.hall_update_next_siege_with(id, config);
        }
        if self.clan_hall(id).is_some_and(|h| h.owner_id != 0) {
            if let Some(hall) = self.clan_hall_mut(id) {
                hall.is_free = false;
            }
            self.load_residence_functions(ResidenceRef::ClanHall(id), &record.functions);
        }
    }

    /// Java `new AuctionableHall(set)` and `ClanHallTable.load`'s lists. Returns whether the hall
    /// has an owner (`_clanHall`, else `_freeClanHall`).
    fn install_auctionable_hall(&mut self, record: ClanHallRecord) -> bool {
        let id = ClanHallId(record.id);
        let hall = ClanHall {
            id,
            name: record.name,
            owner_id: record.owner_id,
            desc: record.desc,
            location: record.location,
            is_free: true,
            functions: Default::default(),
            doors: Vec::new(),
            zone: None,
            kind: ClanHallKind::Auctionable {
                lease: record.lease,
                paid_until: record.paid_until,
                grade: record.grade,
                paid: record.paid,
            },
        };
        self.residences.clan_halls.insert(id, hall);
        if record.owner_id > 0 {
            if let Some(clan) = self.clan_mut(record.owner_id) {
                clan.hideout_id = id.0;
            } else {
                // `free()` from the `ClanHall` constructor, before `AuctionableHall` set its own
                // fields.
                // NOTE(java-quirk): the row gets owner 0, `paidUntil` 0 and `paid` 0 (the fields
                // are not set yet), while the hall keeps the row's `paidUntil` and `paid`.
                if let Some(hall) = self.clan_hall_mut(id) {
                    hall.owner_id = 0;
                }
                self.siege_write(super::ResidenceWrite::ClanHall {
                    hall_id: id.0,
                    owner_id: 0,
                    paid_until: 0,
                    paid: false,
                });
            }
        }
        let owned = self.clan_hall(id).is_some_and(|h| h.owner_id > 0);
        if self.clan_hall(id).is_some_and(|h| h.owner_id != 0) {
            if let Some(hall) = self.clan_hall_mut(id) {
                hall.is_free = false;
            }
            self.clan_hall_initialize_rent_task(id, false);
            self.load_residence_functions(ResidenceRef::ClanHall(id), &record.functions);
        }
        // `ClanHallAuctionManager.initNPC(id)` for a free hall without an auction runs when the
        // auctions are installed (`World::load_clan_hall_auctions`).
        owned
    }

    /// The `Door` constructor's `hall.getDoors().add(this)` (the template's `clanhall_id`) and the
    /// `ClanHallZone.setParameter` registration of each hall's zone.
    fn bind_clan_hall_doors_and_zones(&mut self) {
        let mut doors: Vec<(i32, ObjectId, i32)> = self
            .doors
            .values()
            .filter(|d| d.template.clanhall_id > 0)
            .map(|d| (d.door_id, d.object_id, d.template.clanhall_id))
            .collect();
        doors.sort_unstable();
        for (_, object_id, hall_id) in doors {
            if let Some(hall) = self.clan_hall_mut(ClanHallId(hall_id)) {
                hall.doors.push(object_id);
            }
        }
        let Some(zones) = self.zoning.zones.clone() else {
            return;
        };
        let ids: Vec<ClanHallId> = self.residences.clan_halls.keys().copied().collect();
        for id in ids {
            let zone = clan_hall_zone_of(&zones, id.0).map(|z| (z.id, z.zone_type.clone()));
            if let Some(hall) = self.clan_hall_mut(id) {
                hall.zone = zone;
            }
        }
        for zone in zones.created().filter(|z| z.is_a("ClanHallZone")) {
            if let Some(hall_id) = zone.int_param("clanHallId")
                && self.clan_hall(ClanHallId(hall_id)).is_none()
            {
                warn!("ClanHallZone: Clan hall with id {hall_id} does not exist!");
            }
        }
    }

    /// Java `new Castle(id)`.
    fn install_castle(&mut self, record: &CastleRecord) {
        let trees = Arc::clone(&self.rewards.skill_trees);
        let mut castle = Castle::from_record(record);
        castle.residential_skills = residential_skills_of(&trees, record.id);
        if let Some(zones) = self.zoning.zones.as_ref() {
            castle.zone = castle_zone_of(zones, record.id).map(|z| z.id);
            castle.siege_zone = siege_zone_of(zones, record.id).map(|z| z.id);
            castle.teleport_zone = residence_teleport_zone_of(zones, record.id).map(|z| z.id);
        }
        let owned = castle.owner_id != 0;
        let id = castle.id;
        self.residences.castles.push(castle);
        if owned {
            self.load_residence_functions(ResidenceRef::Castle(id), &record.functions);
            // `loadDoorUpgrade()` (`castle_doorupgrade`): `World::load_sieges` (M7-SIEGE-01).
        }
    }

    /// Java `new Fort(id)` (the parts that are not the siege or the spawns). `false` where Java
    /// throws ("Can't find flagpole for Fort ..."), which ends `FortManager.loadInstances`.
    fn install_fort(&mut self, record: &FortRecord) -> bool {
        let trees = Arc::clone(&self.rewards.skill_trees);
        let mut fort = Fort::from_record(record);
        fort.residential_skills = residential_skills_of(&trees, record.id);
        let id = fort.id;
        // `load()`: the owner is set when the clan exists; `clan.setFortId` of a missing clan
        // throws and the fortress stays without an owner.
        let owner = if record.owner_id > 0 {
            if self.clan(record.owner_id).is_some() {
                record.owner_id
            } else {
                warn!(
                    "Exception: loadFortData(): the owner {} of {id} does not exist.",
                    record.owner_id
                );
                0
            }
        } else {
            0
        };
        // `loadFlagPoles()`: the first static object of type 3 whose name starts with the name.
        let mut poles: Vec<(ObjectId, &str)> = self
            .static_objects
            .values()
            .filter(|o| o.kind == 3)
            .map(|o| (o.object_id, o.name.as_str()))
            .collect();
        poles.sort_unstable_by_key(|(object_id, _)| *object_id);
        let Some(pole) = poles
            .into_iter()
            .find(|(_, name)| name.starts_with(fort.name.as_str()))
            .map(|(object_id, _)| object_id)
        else {
            warn!(
                "Exception: loadFortData(): Can't find flagpole for Fort {}({})",
                fort.name, id.0
            );
            return false;
        };
        fort.flag_pole = Some(pole);
        if let Some(zones) = self.zoning.zones.as_ref() {
            fort.zone = fort_zone_of(zones, record.id).map(|z| z.id);
            fort.siege_zone = siege_zone_of(zones, record.id).map(|z| z.id);
        }
        self.residences.forts.push(fort);
        if owner != 0 {
            if let Some(clan) = self.clan_mut(owner) {
                clan.fort_id = id.0;
            }
            if let Some(fort) = self.fort_mut(id) {
                fort.owner_id = owner;
            }
            self.fort_set_visible_flag(id, true);
            self.load_residence_functions(ResidenceRef::Fort(id), &record.functions);
            // The `FortUpdater` tasks of an owned fortress (`Fort.load()`; M7-FORT-01). They use
            // the fortress settings, installed with `set_fort_settings` before the residences.
            self.fort_start_updaters_on_load(id);
        }
        // `initNpcs()`, `initSiegeNpcs()`, `initNpcCommanders()`, `spawnNpcCommanders()`,
        // `initSpecialEnvoys()` and the envoys of an owned fortress need the rows of
        // `fort_spawnlist`: they run in `World::load_fort_data` (M7-FORT-01).
        true
    }

    /// Java `Castle.loadDoor()` / `Fort.loadDoor()`: every door whose `getCastle()` (the first
    /// castle whose siege zone holds it) / `getFort()` is the residence.
    fn bind_siege_zone_doors(&mut self, castles: bool) {
        let mut doors: Vec<(i32, ObjectId, i32, i32, i32)> = self
            .doors
            .values()
            .map(|d| (d.door_id, d.object_id, d.x, d.y, d.z))
            .collect();
        doors.sort_unstable();
        for (_, object_id, x, y, z) in doors {
            if castles {
                if let Some(castle) = self.castle_at(x, y, z).and_then(|id| self.castle_mut(id)) {
                    castle.doors.push(object_id);
                }
            } else if let Some(fort) = self.fort_at(x, y, z).and_then(|id| self.fort_mut(id)) {
                fort.doors.push(object_id);
            }
        }
    }
}
