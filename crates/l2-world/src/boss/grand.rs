//! Java `managers/GrandBossManager.java`: the state of each grand boss (`grandboss_data`), the
//! info the boss AIs read (`_storedInfo`), the registered boss NPCs (`BOSSES`) and the 5 minute
//! store.
//
// The scripts of the grand bosses (D-05) are the callers of the setters; the status numbers are
// theirs (0 alive for all; 1 to 3 waiting, in fight and dead for Antharas, Valakas, Baium and
// Frintezza; 1 dead for the others, see `AdminGrandBoss`).

use std::collections::{BTreeMap, HashMap};

use l2_util::ObjectId;
use tracing::{info, warn};

use super::{BossWrite, GrandBossStoreRow};
use crate::world::World;

/// A `grandboss_data` row: the state of one grand boss and where it stands.
#[derive(Debug, Clone, PartialEq)]
pub struct GrandBossData {
    pub boss_id: i32,
    pub loc_x: i32,
    pub loc_y: i32,
    pub loc_z: i32,
    pub heading: i32,
    /// `respawn_time`, epoch milliseconds.
    pub respawn_time: i64,
    pub current_hp: f64,
    pub current_mp: f64,
    pub status: i32,
}

/// Java's `StatSet` of a grand boss (`_storedInfo`): the keys `loc_x`, `loc_y`, `loc_z`,
/// `heading`, `respawn_time`, `currentHP` and `currentMP`.
#[derive(Debug, Clone, PartialEq)]
pub struct GrandBossInfo {
    pub loc_x: i32,
    pub loc_y: i32,
    pub loc_z: i32,
    pub heading: i32,
    pub respawn_time: i64,
    pub current_hp: f64,
    pub current_mp: f64,
}

impl From<&GrandBossData> for GrandBossInfo {
    fn from(row: &GrandBossData) -> Self {
        Self {
            loc_x: row.loc_x,
            loc_y: row.loc_y,
            loc_z: row.loc_z,
            heading: row.heading,
            respawn_time: row.respawn_time,
            current_hp: row.current_hp,
            current_mp: row.current_mp,
        }
    }
}

/// Java `GrandBossManager._bossStatus`, `_storedInfo` and `BOSSES`.
#[derive(Default)]
pub(crate) struct GrandBosses {
    pub(crate) status: BTreeMap<i32, i32>,
    pub(crate) stored: BTreeMap<i32, GrandBossInfo>,
    /// `BOSSES`: the NPC a script registered for the boss id.
    pub(crate) bosses: HashMap<i32, ObjectId>,
}

impl World {
    /// Java `GrandBossManager.init`: the rows are in memory for the AIs.
    pub(crate) fn load_grand_bosses(&mut self, rows: Vec<GrandBossData>) {
        for row in &rows {
            self.boss.grand.status.insert(row.boss_id, row.status);
            self.boss.grand.stored.insert(row.boss_id, GrandBossInfo::from(row));
            // NOTE(java-quirk): Java throws on a boss without a template and drops the rest of the
            // rows; here the name is the id and the loading goes on.
            let name = self.boss_name(row.boss_id);
            info!("GrandBossManager: {name}({}) status is {}.", row.boss_id, row.status);
            if row.status > 0 {
                info!(
                    "GrandBossManager: Next spawn date of {name} is {}.",
                    super::java_date(row.respawn_time)
                );
            }
        }
        info!("GrandBossManager: Loaded {} Instances", self.boss.grand.stored.len());
    }

    pub(crate) fn boss_name(&self, npc_id: i32) -> String {
        self.npc_world
            .template(npc_id)
            .map_or_else(|| npc_id.to_string(), |t| t.name.clone())
    }

    /// Java `getStatus(bossId)`. Java unboxes a null and throws for a boss without a row; `None`.
    pub fn grand_boss_status(&self, boss_id: i32) -> Option<i32> {
        self.boss.grand.status.get(&boss_id).copied()
    }

    /// A grand boss spawns here only when its state row says it is alive (status 0, D-53). A boss
    /// without a row does not spawn.
    pub fn grand_boss_may_spawn(&self, boss_id: i32) -> bool {
        self.grand_boss_status(boss_id) == Some(0)
    }

    /// Java `setStatus(bossId, status)`: the status changes in memory and `UPDATE grandboss_data
    /// set status` follows (with `CycleDatabaseWrites`).
    pub fn set_grand_boss_status(&mut self, boss_id: i32, status: i32) {
        self.boss.grand.status.insert(boss_id, status);
        info!(
            "GrandBossManager: Updated {}({boss_id}) status to {status}",
            self.boss_name(boss_id)
        );
        self.grand_boss_update_db(boss_id, true);
    }

    /// Java `getStatSet(bossId)`.
    pub fn grand_boss_info(&self, boss_id: i32) -> Option<&GrandBossInfo> {
        self.boss.grand.stored.get(&boss_id)
    }

    /// Java `setStatSet(bossId, info)`: the info is replaced and the row updated.
    pub fn set_grand_boss_info(&mut self, boss_id: i32, info: GrandBossInfo) {
        self.boss.grand.stored.insert(boss_id, info);
        self.grand_boss_update_db(boss_id, false);
    }

    /// Java `addBoss(boss)`: the NPC of the boss id (a script spawned it).
    pub fn add_grand_boss(&mut self, boss_id: i32, npc: ObjectId) {
        self.boss.grand.bosses.insert(boss_id, npc);
    }

    /// Java `getBoss(bossId)`.
    pub fn grand_boss_npc(&self, boss_id: i32) -> Option<ObjectId> {
        self.boss.grand.bosses.get(&boss_id).copied()
    }

    /// The row Java writes for a boss (`UPDATE_GRAND_BOSS_DATA`): the boss NPC's place and
    /// vitals, `respawn_time` of the info, the status. A dead boss is written with full HP and MP.
    /// `None` when no NPC was registered for the boss or it has no info (Java then writes the
    /// status only).
    fn grand_boss_full_row(&self, boss_id: i32) -> Option<GrandBossData> {
        let npc = *self.boss.grand.bosses.get(&boss_id)?;
        let info = self.boss.grand.stored.get(&boss_id)?;
        let status = self.boss.grand.status.get(&boss_id).copied()?;
        // NOTE: Java keeps the NPC object after it decayed and writes its last place; the world
        // forgets a decayed NPC, so the place and vitals of the info stand in for it.
        let live = self.core_of(npc).map(|core| (core.x, core.y, core.z, core.heading));
        let (x, y, z, heading) = live.unwrap_or((info.loc_x, info.loc_y, info.loc_z, info.heading));
        let (mut hp, mut mp) = match (self.core_of(npc), self.hp_and_max(npc), self.mp_and_max(npc)) {
            (Some(_), Some((hp, _)), Some((mp, _))) => (hp, mp),
            _ => (info.current_hp, info.current_mp),
        };
        if self.is_dead(npc)
            && let (Some((_, max_hp)), Some((_, max_mp))) = (self.hp_and_max(npc), self.mp_and_max(npc))
        {
            hp = max_hp;
            mp = max_mp;
        }
        Some(GrandBossData {
            boss_id,
            loc_x: x,
            loc_y: y,
            loc_z: z,
            heading,
            respawn_time: info.respawn_time,
            current_hp: hp,
            current_mp: mp,
            status,
        })
    }

    /// Java `GrandBossManager.updateDb(bossId, statusOnly)`.
    fn grand_boss_update_db(&mut self, boss_id: i32, status_only: bool) {
        if !self.boss_writes_enabled() {
            return;
        }
        let Some(status) = self.boss.grand.status.get(&boss_id).copied() else {
            // Java: `_bossStatus.get(bossId)` unboxes a null.
            warn!("GrandBossManager: Couldn't update grandbosses to database: no status for {boss_id}");
            return;
        };
        let full = if status_only { None } else { self.grand_boss_full_row(boss_id) };
        match full {
            Some(row) => self.boss_write(BossWrite::GrandBoss(row)),
            None => self.boss_write(BossWrite::GrandBossStatus { boss_id, status }),
        }
    }

    /// Java `storeMe()`: the zone lists are replaced and every boss row updated (with
    /// `CycleDatabaseWrites`).
    pub fn store_grand_bosses(&mut self) {
        if !self.boss_writes_enabled() {
            return;
        }
        let list = self.boss_zone_list();
        let ids: Vec<i32> = self.boss.grand.stored.keys().copied().collect();
        let mut rows = Vec::with_capacity(ids.len());
        for boss_id in ids {
            let Some(status) = self.boss.grand.status.get(&boss_id).copied() else {
                continue;
            };
            rows.push(match self.grand_boss_full_row(boss_id) {
                Some(row) => GrandBossStoreRow::Full(row),
                None => GrandBossStoreRow::StatusOnly { boss_id, status },
            });
        }
        self.boss_write(BossWrite::GrandBossStoreAll { list, rows });
    }

    pub(crate) fn schedule_grand_boss_store(&mut self) {
        self.schedule_boss_periodic("grand_store", 5 * 60 * 1000, super::BossTimer::GrandBossStore);
    }

    /// Java `cleanUp()`: stores, then forgets everything.
    pub(crate) fn grand_boss_clean_up(&mut self) {
        self.store_grand_bosses();
        self.boss.grand.bosses.clear();
        self.boss.grand.stored.clear();
        self.boss.grand.status.clear();
        self.boss.zones.clear();
    }

    /// The answer of a re-read (D-50): the row replaces the status and the info in memory.
    pub(crate) fn apply_grand_boss_row(&mut self, boss_id: i32, row: Option<GrandBossData>) {
        match row {
            Some(row) => {
                self.boss.grand.status.insert(boss_id, row.status);
                self.boss.grand.stored.insert(boss_id, GrandBossInfo::from(&row));
            }
            None => warn!("GrandBossManager: no grandboss_data row for {boss_id}."),
        }
    }
}
