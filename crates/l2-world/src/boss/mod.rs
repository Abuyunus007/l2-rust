//! Grand bosses, raid bosses and raid points (M8-BOSS-01): the state of the grand bosses
//! (`grandboss_data`) and of their zones (`grandboss_list`), the raid boss spawn list and its
//! respawn times (`raidboss_spawnlist`), the raid points of the players (`character_raid_points`),
//! `RequestGetBossRecord` and the admin `//grandboss` page.
// Java: managers/GrandBossManager.java
// Java: managers/RaidBossSpawnManager.java
// Java: managers/RaidBossPointsManager.java
// Java: entity/actor/instance/{RaidBoss,GrandBoss}.java (doDie)
// Java: entity/zone/type/BossZone.java
// Java: network/clientpackets/RequestGetBossRecord.java
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminGrandBoss.java
//
// # Shape
// What the three Java singletons keep lives in [`BossWorld`] (`World::boss`). A raid boss is an
// ordinary NPC of the world (object id); its spawn is a runtime spawn (`add_runtime_spawn`) that
// never respawns by the regular path (Java: `Npc.onDecay` skips `decreaseCount` for a raid boss
// the manager defines), the manager's own timer ([`BossTimer::RaidSpawn`], Java `SpawnSchedule`)
// brings it back. Everything refers to NPCs, players and zones by id (D-11).
//
// # The database (D-15, D-39, D-49, D-53)
// The world never waits for the database. Starting data comes in with [`BossLoad`] and
// [`RaidBossData`] (read before the world runs); every write is a [`BossWrite`] sent through the
// outbox. The Java game server shares the rows, so:
// - the grand boss states (`grandboss_data`, `grandboss_list`) and the raid boss rows
//   (`raidboss_spawnlist`: respawn time, HP, MP, insert, delete) are written only with
//   `BossSettings::cycle_database_writes` (`CycleDatabaseWrites` of `SideBySide.ini`, D-53,
//   default off). The world changes its own state either way and reads the rows at start;
//   `//grandboss <id>` reads the boss row again before it shows the state (D-50);
// - the raid points of a player are written as Java writes them (`REPLACE` of the new total, per
//   player). The rows of a character are read again when it enters the world (D-50), because the
//   Java game server may have given it points since this server started. The weekly reset
//   (`DELETE`, clan reputation) is a global job: with the switch off it only forgets the points
//   in memory and gives no reputation (the Java server does both, D-49).
//
// # Left out, with the reason
// - The boss AIs and the instance scripts (D-05) call [`World::set_grand_boss_status`],
//   [`World::add_grand_boss`], [`World::allow_boss_zone_entry`] and friends; nobody calls them yet.
// - `Hero.setRBkilled` is `olympiad/hero.rs` (M8-HERO-01); a kill by a noble writes the diary.
// - `ConditionPlayerCanEscape` (`GrandBossManager.getZone(player) != null`): the condition is not
//   modelled by the stat engine; [`World::grand_boss_zone_of`] is the lookup it will use.
// - `BossZone` for summons and NPCs (their zones are inert) and `returnHome` of a raid boss that
//   leaves its zone.

mod admin;
mod grand;
mod points;
mod raid;
mod zone;

use std::collections::{BTreeMap, HashMap};

use tracing::info;

use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::{World, unix_ms_now};

pub(crate) use admin::register as admin_register;
pub use grand::{GrandBossData, GrandBossInfo};
pub use points::RaidPointsRecord;
pub use raid::{RaidBossData, RaidBossStatus};

/// Java `new Date(epochMs).toString()` in the machine's zone (D-48), for the log lines.
pub(crate) fn java_date(epoch_ms: i64) -> String {
    crate::clanhall::siege::schedule::java_date_string(epoch_ms, l2_util::time::local_offset_millis(epoch_ms))
}

/// The configuration the managers read.
#[derive(Debug, Clone, PartialEq)]
pub struct BossSettings {
    /// `CycleDatabaseWrites` of `config/SideBySide.ini` (D-53): the global boss rows are written.
    pub cycle_database_writes: bool,
    /// `NpcConfig.RAID_MIN_RESPAWN_MULTIPLIER` (`RaidMinRespawnMultiplier`).
    pub raid_min_respawn_multiplier: f32,
    /// `NpcConfig.RAID_MAX_RESPAWN_MULTIPLIER` (`RaidMaxRespawnMultiplier`).
    pub raid_max_respawn_multiplier: f32,
    /// `FeatureConfig.RAID_RANKING_1ST` ... `RAID_RANKING_10TH`.
    pub raid_ranking: [i32; 10],
    /// `FeatureConfig.RAID_RANKING_UP_TO_50TH`.
    pub raid_ranking_up_to_50th: i32,
    /// `FeatureConfig.RAID_RANKING_UP_TO_100TH`.
    pub raid_ranking_up_to_100th: i32,
}

impl Default for BossSettings {
    fn default() -> Self {
        Self {
            cycle_database_writes: false,
            raid_min_respawn_multiplier: 1.0,
            raid_max_respawn_multiplier: 1.0,
            raid_ranking: [1250, 900, 700, 600, 450, 350, 300, 200, 150, 100],
            raid_ranking_up_to_50th: 25,
            raid_ranking_up_to_100th: 12,
        }
    }
}

/// The rows read at start, in the database's order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BossLoad {
    /// `grandboss_data`, ordered by boss id.
    pub grand_bosses: Vec<GrandBossData>,
    /// `grandboss_list` as `(player_id, zone)`.
    pub grand_boss_list: Vec<(i32, i32)>,
    /// `character_raid_points`.
    pub raid_points: Vec<RaidPointsRecord>,
}

/// The timers of `TimerKind::Boss`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BossTimer {
    /// Java `GrandBossManager`: `scheduleAtFixedRate(this::storeMe, 5 min, 5 min)`.
    GrandBossStore,
    /// Java `RaidBossSpawnManager.SpawnSchedule` of the raid boss with this NPC id.
    RaidSpawn(i32),
    /// Java `RaidBossPointsManager`: `resetRaidPoints`, every day at 00:10.
    PointsReset,
}

/// A row write for the persistence task, with Java's statement. Only sent when the rules in the
/// module comment allow it.
#[derive(Debug, Clone, PartialEq)]
pub enum BossWrite {
    /// `UPDATE grandboss_data set status = ? where boss_id = ?`.
    GrandBossStatus { boss_id: i32, status: i32 },
    /// `UPDATE grandboss_data set loc_x = ?, ... where boss_id = ?`.
    GrandBoss(GrandBossData),
    /// `GrandBossManager.storeMe`: `grandboss_list` is replaced, every boss row is updated.
    GrandBossStoreAll {
        list: Vec<(i32, i32)>,
        rows: Vec<GrandBossStoreRow>,
    },
    /// `INSERT INTO raidboss_spawnlist`.
    RaidBossInsert(RaidBossData),
    /// `DELETE FROM raidboss_spawnlist WHERE boss_id=?`.
    RaidBossDelete { boss_id: i32 },
    /// `UPDATE raidboss_spawnlist SET respawn_time = ?, currentHP = ?, currentMP = ? WHERE boss_id = ?`
    /// for each row.
    RaidBossUpdate(Vec<RaidBossUpdateRow>),
    /// `REPLACE INTO character_raid_points`.
    RaidPoints { char_id: i32, boss_id: i32, points: i32 },
    /// `DELETE from character_raid_points WHERE charId > 0`.
    RaidPointsDeleteAll,
}

/// One boss of `GrandBossManager.storeMe`.
#[derive(Debug, Clone, PartialEq)]
pub enum GrandBossStoreRow {
    /// `UPDATE_GRAND_BOSS_DATA2`.
    StatusOnly { boss_id: i32, status: i32 },
    /// `UPDATE_GRAND_BOSS_DATA`.
    Full(GrandBossData),
}

/// One statement of `RaidBossSpawnManager.updateDb`.
#[derive(Debug, Clone, PartialEq)]
pub struct RaidBossUpdateRow {
    pub boss_id: i32,
    pub respawn_time: i64,
    pub current_hp: f64,
    pub current_mp: f64,
}

/// What a read of the shared rows asks for (D-50).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BossReadKind {
    /// The `grandboss_data` row of this boss.
    GrandBoss(i32),
    /// The `character_raid_points` rows of this character.
    RaidPoints(i32),
}

/// What happens when the answer of a read is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BossReadThen {
    /// Nothing more (the rows are in memory).
    Nothing,
    /// `AdminGrandBoss.manageHtml`: show the state page to this GM.
    ShowGrandBoss { gm: l2_util::ObjectId, boss_id: i32 },
}

/// The answer of a read (`WorldCommand::BossReread`).
#[derive(Debug, Clone, PartialEq)]
pub enum BossRereadRows {
    /// The row of the boss, `None` when the table has none.
    GrandBoss {
        boss_id: i32,
        row: Option<GrandBossData>,
    },
    /// The rows of the character.
    RaidPoints {
        char_id: i32,
        rows: Vec<RaidPointsRecord>,
    },
}

/// Everything the three managers keep.
#[derive(Default)]
pub(crate) struct BossWorld {
    pub(crate) settings: BossSettings,
    pub(crate) grand: grand::GrandBosses,
    pub(crate) raid: raid::RaidBosses,
    pub(crate) points: points::RaidPoints,
    pub(crate) zones: BTreeMap<i32, zone::BossZoneState>,
    /// The pending `GrandBossStore` and `PointsReset` timers.
    pub(crate) periodic: HashMap<&'static str, TimerId>,
    /// `GameServer.getStartTime()`, for the re-entry rule of a boss zone.
    pub(crate) started_ms: i64,
}

impl World {
    /// Installs the settings of the managers. Call before [`World::load_bosses`].
    pub fn set_boss_settings(&mut self, settings: BossSettings) {
        self.boss.settings = settings;
    }

    /// Java `GameServer`: `RaidBossPointsManager.getInstance()` (the points and their reset task),
    /// `GrandBossManager.getInstance()` (the states and the 5 minute store) and its
    /// `initZones()`. The zone table must be installed ([`World::set_zone_data`]). The raid boss
    /// spawn list is [`World::spawn_raid_bosses`], after the spawns.
    pub fn load_bosses(&mut self, load: BossLoad) {
        self.boss.started_ms = unix_ms_now();
        self.load_raid_points(load.raid_points);
        self.load_grand_bosses(load.grand_bosses);
        self.load_boss_zones(&load.grand_boss_list);
        self.start_boss_timers();
    }

    fn start_boss_timers(&mut self) {
        // `scheduleAtFixedRate(this::storeMe, 5 * 60 * 1000, 5 * 60 * 1000)`.
        self.schedule_grand_boss_store();
        self.start_raid_points_reset_task();
    }

    pub(crate) fn schedule_boss_periodic(&mut self, name: &'static str, delay_ms: u64, timer: BossTimer) {
        let due = self.tick + TimerWheel::ms_to_ticks(delay_ms);
        let id = self.timers.schedule_at(due, TimerKind::Boss(timer));
        if let Some(old) = self.boss.periodic.insert(name, id) {
            self.timers.cancel(old);
        }
    }

    /// A `TimerKind::Boss` timer fired.
    pub(crate) fn on_boss_timer(&mut self, timer: BossTimer) {
        match timer {
            BossTimer::GrandBossStore => {
                self.schedule_grand_boss_store();
                self.store_grand_bosses();
            }
            BossTimer::RaidSpawn(boss_id) => self.on_raid_spawn_timer(boss_id),
            BossTimer::PointsReset => self.on_raid_points_reset_timer(),
        }
    }

    /// Java `Shutdown`: `RaidBossSpawnManager.cleanUp()`, then `GrandBossManager.cleanUp()`.
    pub(crate) fn boss_shutdown(&mut self) {
        self.raid_clean_up();
        self.grand_boss_clean_up();
        info!("Boss managers: all boss info saved.");
    }

    /// Whether the D-53 switch lets this server write the global boss rows.
    pub(crate) fn boss_writes_enabled(&self) -> bool {
        self.boss.settings.cycle_database_writes
    }

    /// Sends `write` to the persistence task.
    pub(crate) fn boss_write(&mut self, write: BossWrite) {
        self.outbox.persist(crate::outbox::PersistCommand::Boss(write));
    }

    /// The answer of a read of the shared rows (`WorldCommand::BossReread`). A failed read changes
    /// nothing and the action that waited for it is refused (D-50).
    pub(crate) fn on_boss_reread(&mut self, result: Result<BossRereadRows, ()>, then: BossReadThen) {
        let ok = match result {
            Ok(rows) => {
                self.apply_boss_rows(rows);
                true
            }
            Err(()) => false,
        };
        if let BossReadThen::ShowGrandBoss { gm, boss_id } = then {
            if ok {
                self.show_grand_boss_page(gm, boss_id);
            } else {
                self.send_sys_message(gm, "The boss state could not be read from the database.");
            }
        }
    }

    fn apply_boss_rows(&mut self, rows: BossRereadRows) {
        match rows {
            BossRereadRows::GrandBoss { boss_id, row } => self.apply_grand_boss_row(boss_id, row),
            BossRereadRows::RaidPoints { char_id, rows } => self.apply_raid_points_rows(char_id, &rows),
        }
    }
}
