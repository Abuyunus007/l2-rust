//! Castle sieges (M7-SIEGE-01): the siege schedule, the registration of attacker and defender
//! clans, the siege itself (start, the owner change in the middle, the end), the siege zones, the
//! siege state of players (relations, kills, fame, respawn points), the headquarters, the towers,
//! the artefact, the siege guards and the mercenary tickets, the castle door upgrades and the GM
//! `//siege` commands.
// Java: managers/{SiegeManager,SiegeGuardManager,MercTicketManager}.java
// Java: mechanics/siege/{Siege,SiegeClan,SiegeClanType,SiegeTeleportWhoType,SiegeScheduleDate,
//       Siegable,TowerSpawn}.java
// Java: mechanics/siege/Castle.java (engrave, spawnDoor, removeUpgrade, the door and trap
//       upgrades, getSiege)
// Java: entity/actor/instance/{SiegeFlag,ControlTower,FlameTower,Artefact}.java,
//       entity/actor/Tower.java, entity/actor/status/SiegeFlagStatus.java
// Java: entity/zone/type/SiegeZone.java
// Design: docs/design/m7-residences.md
//
// # Shape
// One [`Siege`] per castle in `World::sieges` (Java `Castle.getSiege()`), made when the
// residences are loaded ([`World::load_sieges`], Java `SiegeManager.getSieges()` at startup). A
// siege clan is a clan id with its kind and its headquarters (NPC object ids). The towers, flags
// and guards are NPCs of the world's tables; the siege only keeps their ids. Java's
// `ThreadPool.schedule` tasks are [`SiegeTimer`]s on the wheel.
//
// The rows Java reads and writes (`siege_clans`, `castle_siege_guards`, `castle_doorupgrade`,
// `castle_trapupgrade`) are kept as Java has them in the database ([`SiegeWorld::siege_clan_rows`],
// [`SiegeWorld::guard_rows`]); every statement Java runs changes the copy here first and then goes
// out as a [`ResidenceWrite`]. What Java reads back from the database (`loadSiegeClan`,
// `checkIsRegistered`, `loadSiegeGuard`) reads the copy.
//
// # Side by side (D-49, D-50, D-51)
// Sieghardt (the Java GS) runs the real sieges while both servers share the database:
// - Every write of the siege jobs and of the registrations goes through `World::siege_write`
//   (`SiegeDatabaseWrites`, default off). Off, the world still changes its own state, so a GM test
//   siege (`//siege`, `//startsiege`) works end to end.
// - The schedule runs here too (dates, the registration messages), but the start of a siege by
//   the schedule (`ScheduleStartSiegeTask` reaching `startSiege`) happens only with the switch on;
//   off, the castle and its registrations are read again once the Java siege is over
//   (`SiegeTimer::SideBySideReread`), which also starts the schedule again from the new date.
// - The registrations are read again before a player acts on them or looks at them (D-50):
//   `RequestJoinSiege`, `RequestConfirmSiegeWaitingList` and the two list requests go through
//   `World::request_siege_reread`; a failed read refuses the request. Rows this world added without
//   writing them (the switch is off) stay in the copy over a re-read, so a test registration
//   lives until the siege.
// - A castle lord's mercenary tickets are his own action (`Writes::Player`), like the tax rate.
//
// # Left to other cards
// - `SiegeGuardAI` (the defenders' AI) and the damage of doors and walls (doors are not creatures
//   of the stat engine yet): the defenders and doors become attackable during a siege here, but
//   the combat code still treats a door as no target and a defender has the plain attackable AI.
//   FOLLOW-UP(M7-SIEGE-02).
// - The fortress sieges (M7-FORT-01), the conquerable halls (M7-CHSIEGE-01) and Territory War
//   (M7-TW-01) set the player siege state and the siege zones through the API in `player.rs` and
//   `zone.rs`; their parts of the shared code are marked with their card.
// - Seven Signs (M8): `SevenSigns.getSealOwner(SEAL_STRIFE)` and the period are read through
//   [`World::seven_signs_strife_owner`] and [`World::seven_signs_is_seal_validation`], which
//   answer "no owner" and "seal validation" until M8 ports the real state.
// - Hero (M8): `Hero.setCastleTaken`. Scripts (D-05): the `OnCastleSiege*` events.

mod admin;
mod battle;
mod conditions;
mod doors;
mod merc;
mod npcs;
mod player;
mod registration;
mod restart;
mod schedule;
mod zone;

use std::collections::{BTreeMap, HashMap};

use l2_util::ObjectId;
use tracing::info;

use crate::residence::{CastleId, ResidenceRef, ResidenceWrite};
use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::World;

pub(crate) use admin::register as register_admin;
pub use merc::{MERC_ITEM_IDS, MERC_NPC_IDS, MERCS_MAX_PER_CASTLE, ticket_castle_id};
pub use player::PlayerSiegeState;
pub use registration::SiegeInput;
pub use schedule::{next_siege_date, siege_hour_times};
pub use zone::SiegeZoneSiege;

/// Java `Siege.OWNER`: the type the owner would have (never a row).
pub const OWNER: i32 = -1;
/// Java `Siege.DEFENDER`: an approved defender (`siege_clans.type`).
pub const DEFENDER: i32 = 0;
/// Java `Siege.ATTACKER`.
pub const ATTACKER: i32 = 1;
/// Java `Siege.DEFENDER_NOT_APPROVED`: a defender waiting for the lord's approval.
pub const DEFENDER_NOT_APPROVED: i32 = 2;

/// Java `SiegeClanType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SiegeClanType {
    Owner,
    Defender,
    Attacker,
    DefenderPending,
}

/// Java `SiegeClan`: a clan in one of the siege's lists, with its headquarters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiegeClan {
    pub clan_id: i32,
    pub kind: SiegeClanType,
    /// `_flag`: the clan's headquarters (`SiegeFlag` NPCs).
    pub flags: Vec<ObjectId>,
}

impl SiegeClan {
    pub fn new(clan_id: i32, kind: SiegeClanType) -> Self {
        Self {
            clan_id,
            kind,
            flags: Vec::new(),
        }
    }
}

/// Java `SiegeTeleportWhoType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SiegeTeleportWhoType {
    Attacker,
    Owner,
    NotOwner,
    Spectator,
}

/// Java `TowerSpawn`: a control or flame tower of `Siege.ini`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TowerSpawn {
    pub npc_id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// The trap zones a flame tower switches (`zoneIds`).
    pub zone_list: Vec<i32>,
    /// `_upgradeLevel` (`castle_trapupgrade`).
    pub upgrade_level: i32,
}

/// The `Siege.ini` lines of one castle (Java `SiegeManager._controlTowers`, `_flameTowers` and
/// `MercTicketManager.MERCS_MAX_PER_CASTLE`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CastleSiegeConfig {
    pub control_towers: Vec<TowerSpawn>,
    pub flame_towers: Vec<TowerSpawn>,
    /// `<Name>MaxMercenaries`, else Java's table value.
    pub max_mercenaries: i32,
}

/// The configuration values the siege code reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiegeSettings {
    /// `SiegeCycle` (weeks).
    pub siege_cycle: i32,
    /// `AttackerMaxClans`.
    pub attacker_max_clans: i32,
    /// `AttackerRespawn` (ms).
    pub attacker_respawn_delay: i32,
    /// `DefenderMaxClans`.
    pub defender_max_clans: i32,
    /// `MaxFlags`.
    pub flag_max_count: i32,
    /// `SiegeClanMinLevel`.
    pub siege_clan_min_level: i32,
    /// `SiegeLength` (minutes).
    pub siege_length: i32,
    /// `BloodAllianceReward`.
    pub blood_alliance_reward: i32,
    /// The tower lines and mercenary limits by castle id.
    pub castles: BTreeMap<i32, CastleSiegeConfig>,
    /// `FeatureConfig.SIEGE_HOUR_LIST` (`SiegeHourList`).
    pub siege_hour_list: Vec<i32>,
    /// `FeatureConfig.ALLOW_MOUNTS_DURING_SIEGE` (`AllowRideMountsDuringSiege`).
    pub allow_mounts_during_siege: bool,
    /// `PlayerConfig.FAME_SYSTEM_ENABLED` (`EnableFameSystem`).
    pub fame_system_enabled: bool,
    /// `PlayerConfig.CASTLE_ZONE_FAME_TASK_FREQUENCY` (`CastleZoneFameTaskFrequency`, seconds).
    pub castle_zone_fame_task_frequency: i32,
    /// `PlayerConfig.CASTLE_ZONE_FAME_AQUIRE_POINTS` (`CastleZoneFameAquirePoints`).
    pub castle_zone_fame_aquire_points: i32,
    /// `PlayerConfig.FAME_FOR_DEAD_PLAYERS` (`FameForDeadPlayers`).
    pub fame_for_dead_players: bool,
    /// `OfflineTradeConfig.OFFLINE_FAME` (`OfflineFame`).
    pub offline_fame: bool,
    /// `PlayerConfig.TELEPORT_WHILE_SIEGE_IN_PROGRESS` (`TeleportWhileSiegeInProgress`).
    pub teleport_while_siege_in_progress: bool,
    /// For tests: Java's `System.currentTimeMillis()` at world tick 0 (the siege clock then runs
    /// with the ticks). `None`: the wall clock.
    pub clock_override_unix_ms: Option<i64>,
    /// For tests: the zone offset Java's `Calendar` would use, instead of the machine's (D-48).
    pub zone_offset_override_ms: Option<i64>,
}

impl Default for SiegeSettings {
    /// Java's defaults (`SiegeManager.load` and the shipped `Siege.ini` without castle lines).
    fn default() -> Self {
        Self {
            siege_cycle: 2,
            attacker_max_clans: 500,
            attacker_respawn_delay: 0,
            defender_max_clans: 500,
            flag_max_count: 1,
            siege_clan_min_level: 5,
            siege_length: 120,
            blood_alliance_reward: 1,
            castles: BTreeMap::new(),
            siege_hour_list: Vec::new(),
            allow_mounts_during_siege: false,
            fame_system_enabled: true,
            castle_zone_fame_task_frequency: 300,
            castle_zone_fame_aquire_points: 125,
            fame_for_dead_players: true,
            offline_fame: true,
            teleport_while_siege_in_progress: true,
            clock_override_unix_ms: None,
            zone_offset_override_ms: None,
        }
    }
}

/// One `siege_clans` row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SiegeClanRecord {
    pub castle_id: i32,
    pub clan_id: i32,
    /// `type` ([`DEFENDER`], [`ATTACKER`], [`DEFENDER_NOT_APPROVED`]; NULL reads as 0).
    pub kind: i32,
}

/// One `castle_siege_guards` row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SiegeGuardRecord {
    pub castle_id: i32,
    pub npc_id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
    /// Seconds.
    pub respawn_delay: i32,
    pub is_hired: bool,
}

/// One `castle_doorupgrade` row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DoorUpgradeRecord {
    pub door_id: i32,
    pub ratio: i32,
    pub castle_id: i32,
}

/// One `castle_trapupgrade` row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrapUpgradeRecord {
    pub castle_id: i32,
    pub tower_index: i32,
    pub level: i32,
}

/// What the game server read for the sieges at startup (D-15: before the world runs; only reads).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SiegeLoad {
    /// `SELECT clan_id,type FROM siege_clans` of every castle.
    pub siege_clans: Vec<SiegeClanRecord>,
    /// `SELECT * FROM castle_siege_guards` (the datapack's guards and the hired mercenaries).
    pub siege_guards: Vec<SiegeGuardRecord>,
    /// `SELECT * FROM castle_doorupgrade`.
    pub door_upgrades: Vec<DoorUpgradeRecord>,
    /// `SELECT * FROM castle_trapupgrade`.
    pub trap_upgrades: Vec<TrapUpgradeRecord>,
}

/// The siege of one castle (Java `Siege`).
#[derive(Debug, Clone, PartialEq)]
pub struct Siege {
    pub castle_id: CastleId,
    /// `_attackerClans`.
    pub(crate) attacker_clans: Vec<SiegeClan>,
    /// `_defenderClans`.
    pub(crate) defender_clans: Vec<SiegeClan>,
    /// `_defenderWaitingClans`.
    pub(crate) defender_waiting_clans: Vec<SiegeClan>,
    /// `_isNormalSide`: always true in Java (set at the start, never cleared).
    pub(crate) is_normal_side: bool,
    /// `_isInProgress`.
    pub in_progress: bool,
    /// `_isRegistrationOver`.
    pub registration_over: bool,
    /// `_siegeEndDate` (epoch milliseconds).
    pub siege_end_date: i64,
    /// `_firstOwnerClanId`.
    pub first_owner_clan_id: i32,
    /// `_controlTowerCount`.
    pub control_tower_count: i32,
    /// `_controlTowers` (NPC ids).
    pub(crate) control_towers: Vec<ObjectId>,
    /// `_flameTowers` (NPC ids).
    pub(crate) flame_towers: Vec<ObjectId>,
    /// `SiegeGuardManager._siegeGuardSpawn`: the spawn indices of the guards spawned now.
    pub(crate) guard_spawns: Vec<usize>,
    /// `_scheduledStartSiegeTask`.
    pub(crate) start_timer: Option<TimerId>,
}

impl Siege {
    fn new(castle_id: CastleId) -> Self {
        Self {
            castle_id,
            attacker_clans: Vec::new(),
            defender_clans: Vec::new(),
            defender_waiting_clans: Vec::new(),
            is_normal_side: true,
            in_progress: false,
            registration_over: false,
            siege_end_date: 0,
            first_owner_clan_id: -1,
            control_tower_count: 0,
            control_towers: Vec::new(),
            flame_towers: Vec::new(),
            guard_spawns: Vec::new(),
            start_timer: None,
        }
    }

    /// Java `getAttackerClans()`.
    pub fn attacker_clans(&self) -> &[SiegeClan] {
        if self.is_normal_side {
            &self.attacker_clans
        } else {
            &self.defender_clans
        }
    }

    /// Java `getDefenderClans()`.
    pub fn defender_clans(&self) -> &[SiegeClan] {
        if self.is_normal_side {
            &self.defender_clans
        } else {
            &self.attacker_clans
        }
    }

    pub(crate) fn attacker_clans_mut(&mut self) -> &mut Vec<SiegeClan> {
        if self.is_normal_side {
            &mut self.attacker_clans
        } else {
            &mut self.defender_clans
        }
    }

    pub(crate) fn defender_clans_mut(&mut self) -> &mut Vec<SiegeClan> {
        if self.is_normal_side {
            &mut self.defender_clans
        } else {
            &mut self.attacker_clans
        }
    }

    /// Java `getDefenderWaitingClans()`.
    pub fn defender_waiting_clans(&self) -> &[SiegeClan] {
        &self.defender_waiting_clans
    }

    /// Java `getAttackerClan(clanId)`.
    pub fn attacker_clan(&self, clan_id: i32) -> Option<&SiegeClan> {
        self.attacker_clans().iter().find(|sc| sc.clan_id == clan_id)
    }

    /// Java `getDefenderClan(clanId)`.
    pub fn defender_clan(&self, clan_id: i32) -> Option<&SiegeClan> {
        self.defender_clans().iter().find(|sc| sc.clan_id == clan_id)
    }

    /// Java `getDefenderWaitingClan(clanId)`.
    pub fn defender_waiting_clan(&self, clan_id: i32) -> Option<&SiegeClan> {
        self.defender_waiting_clans
            .iter()
            .find(|sc| sc.clan_id == clan_id)
    }

    /// Java `checkIsAttacker(clan)` (`None` is Java's null clan).
    pub fn is_attacker(&self, clan_id: Option<i32>) -> bool {
        clan_id.is_some_and(|id| self.attacker_clan(id).is_some())
    }

    /// Java `checkIsDefender(clan)`.
    pub fn is_defender(&self, clan_id: Option<i32>) -> bool {
        clan_id.is_some_and(|id| self.defender_clan(id).is_some())
    }

    /// Java `checkIsDefenderWaiting(clan)`.
    pub fn is_defender_waiting(&self, clan_id: Option<i32>) -> bool {
        clan_id.is_some_and(|id| self.defender_waiting_clan(id).is_some())
    }

    /// The headquarters of an attacker clan (Java `getFlag(clan)`).
    pub fn flags_of(&self, clan_id: i32) -> &[ObjectId] {
        self.attacker_clan(clan_id).map_or(&[], |sc| sc.flags.as_slice())
    }
}

/// What a siege NPC is to the siege code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SiegeNpc {
    /// `ControlTower`: the guard spawns registered to it (`registerGuard`).
    ControlTower { castle: CastleId, guards: Vec<usize> },
    /// `FlameTower`: `_upgradeLevel` and `_zoneList`.
    FlameTower {
        castle: CastleId,
        upgrade_level: i32,
        zone_list: Vec<i32>,
    },
    /// `SiegeFlag`: the clan, the siege it belongs to (`None` in Territory War), `_isAdvanced`,
    /// `_canTalk` and the invulnerability of an outpost.
    Flag {
        clan_id: i32,
        siege: Option<ResidenceRef>,
        advanced: bool,
        can_talk: bool,
        invul: bool,
    },
    /// A guard of a castle siege (`Defender` from `castle_siege_guards`).
    Guard { castle: CastleId },
}

/// The siege timers (one `TimerKind::Siege`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SiegeTimer {
    /// Java `ScheduleStartSiegeTask`.
    Start(CastleId),
    /// Java `ScheduleEndSiegeTask`.
    End(CastleId),
    /// Side by side (D-51): read the castle and its registrations again once the Java GS has run
    /// its siege, then start the schedule again.
    SideBySideReread(CastleId),
    /// Java `SiegeFlag.ScheduleTalkTask`: the headquarters may warn its clan again.
    FlagTalk(ObjectId),
    /// Java `FameTask` of one player (`scheduleAtFixedRate`).
    Fame(ObjectId),
    /// Java `Npc.scheduleDespawn` of a mercenary placed by a ticket (`spawnMercenary`, 3 s).
    MercDespawn(ObjectId),
    /// Java `RequestRestartPoint.DeathTask`: an attacker's restart after `AttackerRespawn`.
    RestartPoint { player: ObjectId, point_type: i32 },
}

/// The siege state of the world.
#[derive(Debug, Default)]
pub struct SiegeWorld {
    pub(crate) settings: SiegeSettings,
    /// Java `CastleManager` castles' `getSiege()`, by castle id.
    pub(crate) sieges: BTreeMap<CastleId, Siege>,
    /// Java `CastleManager._castleSiegeDate`.
    pub(crate) castle_siege_dates: BTreeMap<i32, i64>,
    /// The `siege_clans` table as Java has it in the database, in insertion order. `unsaved`:
    /// written by this world while `SiegeDatabaseWrites` was off.
    pub(crate) siege_clan_rows: Vec<(SiegeClanRecord, bool)>,
    /// The `castle_siege_guards` table, in row order.
    pub(crate) guard_rows: Vec<SiegeGuardRecord>,
    /// The player siege state (`_siegeState`, `_siegeSide`, `_isInSiege`, the fame task).
    pub(crate) players: HashMap<ObjectId, PlayerSiegeState>,
    /// The siege zones' settings (`SiegeZone.Settings`) that were ever set, by zone id.
    pub(crate) zones: HashMap<i32, zone::SiegeZoneState>,
    /// The siege NPCs (towers, flags, guards).
    pub(crate) npcs: HashMap<ObjectId, SiegeNpc>,
    /// `MercTicketManager.DROPPED_TICKETS`, in the order they were dropped.
    pub(crate) dropped_tickets: Vec<ObjectId>,
    /// The castle trap zones a flame tower switched (`ZoneType.setEnabled`).
    pub(crate) trap_zones: HashMap<i32, bool>,
    /// `Npc.setTitle` of the siege NPCs (the headquarters' clan name).
    pub(crate) npc_titles: HashMap<ObjectId, String>,
    /// `Clan._siegeKills` and `_siegeDeaths` by clan id.
    pub(crate) clan_kills: HashMap<i32, (i32, i32)>,
}

impl World {
    /// Installs the `Siege.ini` values (Java `SiegeManager.load`). Call before [`Self::load_sieges`].
    pub fn set_siege_settings(&mut self, settings: SiegeSettings) {
        self.sieges.settings = settings;
    }

    /// The siege configuration.
    pub fn siege_settings(&self) -> &SiegeSettings {
        &self.sieges.settings
    }

    /// Java's siege start at server start, after the residences: `SiegeManager.load` (the trap
    /// upgrades of the owned castles), `SiegeManager.getSieges()` (a `Siege` per castle, which
    /// starts its schedule when `SiegeSchedule.xml` enables it), `Castle.loadDoorUpgrade` of the
    /// owned castles, then `MercTicketManager` (the tickets of the hired mercenaries). Call after
    /// [`Self::load_residences`].
    pub fn load_sieges(&mut self, load: SiegeLoad) {
        let now = self.siege_now_ms();
        self.load_sieges_at(load, now);
    }

    /// [`Self::load_sieges`] at a given clock.
    pub fn load_sieges_at(&mut self, load: SiegeLoad, now_ms: i64) {
        self.sieges.siege_clan_rows = load.siege_clans.into_iter().map(|r| (r, false)).collect();
        self.sieges.guard_rows = load.siege_guards;
        let castles: Vec<(CastleId, i32)> = self.castles().iter().map(|c| (c.id, c.owner_id)).collect();
        // SiegeManager.load: the trap upgrades of an owned castle.
        for (id, owner) in &castles {
            if *owner != 0 {
                for row in load.trap_upgrades.iter().filter(|r| r.castle_id == id.0) {
                    self.set_trap_upgrade_level_in_memory(*id, row.tower_index, row.level);
                }
            }
        }
        // SiegeManager.getSieges(): `new Siege(castle)` for every castle.
        for (id, _) in &castles {
            self.sieges.sieges.insert(*id, Siege::new(*id));
            let enabled = self
                .siege_schedule()
                .for_castle(id.0)
                .is_some_and(|s| s.siege_enabled);
            if enabled {
                self.siege_start_auto_task_at(*id, now_ms);
            }
        }
        // CastleManager.activateInstances: `loadDoorUpgrade()` of an owned castle.
        for (id, owner) in &castles {
            if *owner != 0 {
                for row in load.door_upgrades.iter().filter(|r| r.castle_id == id.0) {
                    self.castle_set_door_upgrade(*id, row.door_id, row.ratio, false);
                }
            }
        }
        // MercTicketManager.getInstance().
        self.merc_tickets_load();
    }

    /// Java `Castle.getSiege()`.
    pub fn castle_siege(&self, id: CastleId) -> Option<&Siege> {
        self.sieges.sieges.get(&id)
    }

    pub(crate) fn castle_siege_mut(&mut self, id: CastleId) -> Option<&mut Siege> {
        self.sieges.sieges.get_mut(&id)
    }

    /// Java `SiegeManager.getSieges()`, in castle id order.
    pub fn castle_sieges(&self) -> impl Iterator<Item = &Siege> {
        self.sieges.sieges.values()
    }

    /// Java `castle.getSiege().isInProgress()`.
    pub fn is_castle_siege_in_progress(&self, id: CastleId) -> bool {
        self.castle_siege(id).is_some_and(|s| s.in_progress)
    }

    /// Java `Siege.checkIfInZone(x, y, z)`: a siege in progress and the point in the castle's
    /// siege zone.
    pub fn siege_contains(&self, id: CastleId, x: i32, y: i32, z: i32) -> bool {
        self.is_castle_siege_in_progress(id) && self.castle_contains(id, x, y, z)
    }

    /// Java `SiegeManager.getSiege(x, y, z)`: the castle whose siege runs at the point.
    pub fn siege_at(&self, x: i32, y: i32, z: i32) -> Option<CastleId> {
        self.castles()
            .iter()
            .map(|c| c.id)
            .find(|id| self.siege_contains(*id, x, y, z))
    }

    /// Java `SiegeManager.getSiege(object)`.
    pub fn siege_of(&self, object: ObjectId) -> Option<CastleId> {
        let (x, y, z) = self.position_of(object)?;
        self.siege_at(x, y, z)
    }

    /// Java `System.currentTimeMillis()` for the sieges (see [`SiegeSettings::clock_override_unix_ms`]).
    pub(crate) fn siege_now_ms(&self) -> i64 {
        match self.sieges.settings.clock_override_unix_ms {
            Some(base) => base.saturating_add(self.world_ms()),
            None => crate::world::unix_ms_now(),
        }
    }

    /// The zone offset Java's `Calendar` uses at `utc` (D-48).
    pub(crate) fn siege_offset_at(&self, utc: i64) -> i64 {
        self.sieges
            .settings
            .zone_offset_override_ms
            .unwrap_or_else(|| l2_util::time::local_offset_millis(utc))
    }

    /// Schedules a siege timer `delay_ms` from now.
    pub(crate) fn schedule_siege_timer(&mut self, timer: SiegeTimer, delay_ms: i64) -> TimerId {
        let delay = u64::try_from(delay_ms.max(0)).unwrap_or(0);
        let due = self.tick + TimerWheel::ms_to_ticks(delay);
        self.timers.schedule_at(due, TimerKind::Siege(timer))
    }

    /// The siege timers.
    pub(crate) fn on_siege_timer(&mut self, timer: SiegeTimer) {
        match timer {
            SiegeTimer::Start(castle) => self.on_siege_start_task(castle),
            SiegeTimer::End(castle) => self.on_siege_end_task(castle),
            SiegeTimer::SideBySideReread(castle) => self.on_side_by_side_reread_timer(castle),
            SiegeTimer::FlagTalk(flag) => self.on_flag_talk_timer(flag),
            SiegeTimer::Fame(player) => self.on_fame_timer(player),
            SiegeTimer::MercDespawn(npc) => {
                self.delete_npc(npc);
            }
            SiegeTimer::RestartPoint { player, point_type } => {
                if self.players.contains_key(&player) {
                    self.port_player(player, point_type);
                }
            }
        }
    }

    /// Java `World.broadcastToAllOnlinePlayers(packet)`.
    pub(crate) fn broadcast_to_all_online_players(&self, packet: &impl l2_net::Encode) {
        let bytes = packet.to_bytes();
        let mut ids: Vec<ObjectId> = self.sinks.keys().copied().collect();
        ids.sort_unstable();
        for id in ids {
            self.send_bytes(id, bytes.clone());
        }
    }

    /// Java `SevenSigns.getInstance().getSealOwner(SEAL_STRIFE)`: `CABAL_NULL` (0) until M8.
    // M8: the Seven Signs state.
    pub fn seven_signs_strife_owner(&self) -> i32 {
        0
    }

    /// Java `SevenSigns.getInstance().getCurrentPeriod() == PERIOD_SEAL_VALIDATION`: true until
    /// M8.
    // M8: the Seven Signs state.
    pub fn seven_signs_is_seal_validation(&self) -> bool {
        true
    }

    /// A siege NPC is gone from the world (its decay or delete): the siege forgets it.
    pub(crate) fn siege_forget_npc(&mut self, id: ObjectId) {
        self.sieges.npc_titles.remove(&id);
        if self.sieges.npcs.remove(&id).is_some() {
            for siege in self.sieges.sieges.values_mut() {
                for sc in siege
                    .attacker_clans
                    .iter_mut()
                    .chain(siege.defender_clans.iter_mut())
                {
                    sc.flags.retain(|f| *f != id);
                }
                siege.control_towers.retain(|t| *t != id);
                siege.flame_towers.retain(|t| *t != id);
            }
        }
    }

    /// Hands a siege write to the persistence task with `SiegeDatabaseWrites` (D-51); returns
    /// whether it went out.
    pub(crate) fn siege_job_write(&mut self, write: ResidenceWrite) -> bool {
        self.siege_write(write)
    }

    /// Logs the line of `startAutoTask` ("Siege of Gludio: ...").
    pub(crate) fn log_siege_date(&self, id: CastleId) {
        let Some(castle) = self.castle(id) else {
            return;
        };
        info!("Siege of {}: {} (epoch ms)", castle.name, castle.siege_date);
    }
}
