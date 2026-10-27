//! Java `managers/RaidBossSpawnManager.java` and the `doDie` of `RaidBoss` / `GrandBoss`: the raid
//! boss spawn list (`raidboss_spawnlist`), the respawn times, the status of each boss and the
//! raid points a kill gives.
//
// A raid boss is spawned from its row at start (`spawn_raid_bosses`), or scheduled when its
// `respawn_time` is in the future. Its death sets the respawn time and a [`BossTimer::RaidSpawn`]
// timer (Java `SpawnSchedule`); the timer spawns it again. The rows are written only with
// `CycleDatabaseWrites` (D-53, see the module comment of `boss`).

use std::collections::{BTreeMap, HashMap};

use l2_data::spawn::NpcSpawn;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;
use l2_util::ObjectId;
use tracing::{info, warn};

use super::{BossTimer, BossWrite, RaidBossUpdateRow};
use crate::npc::{NpcClass, SpawnReport};
use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::{World, unix_ms_now};

/// Java `RaidBossStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RaidBossStatus {
    Alive,
    Dead,
    Undefined,
}

impl RaidBossStatus {
    /// Java `name()`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Alive => "ALIVE",
            Self::Dead => "DEAD",
            Self::Undefined => "UNDEFINED",
        }
    }
}

/// A `raidboss_spawnlist` row.
#[derive(Debug, Clone, PartialEq)]
pub struct RaidBossData {
    pub boss_id: i32,
    pub amount: i32,
    pub loc_x: i32,
    pub loc_y: i32,
    pub loc_z: i32,
    pub heading: i32,
    /// `respawn_delay` and `respawn_random`, in seconds (`Spawn.setRespawnDelay`).
    pub respawn_delay: i32,
    pub respawn_random: i32,
    /// `respawn_time`, epoch milliseconds; 0 for a boss that is alive.
    pub respawn_time: i64,
    pub current_hp: f64,
    pub current_mp: f64,
}

/// Java `_spawns`: the `Spawn` of a raid boss.
#[derive(Debug, Clone, Copy)]
struct RaidSpawn {
    /// The runtime spawn in the world's spawn index space.
    index: usize,
    /// `Spawn.getRespawnMinDelay()` and `getRespawnMaxDelay()`, in milliseconds.
    min_ms: i32,
    max_ms: i32,
}

/// Java `_bosses`: the boss NPC and its `RaidBossStatus`.
#[derive(Debug, Clone, Copy)]
struct RaidEntry {
    npc: ObjectId,
    status: RaidBossStatus,
}

/// Java `_storedInfo`: `currentHP`, `currentMP` and `respawnTime`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RaidInfo {
    pub(crate) current_hp: f64,
    pub(crate) current_mp: f64,
    pub(crate) respawn_time: i64,
}

#[derive(Default)]
pub(crate) struct RaidBosses {
    spawns: BTreeMap<i32, RaidSpawn>,
    bosses: BTreeMap<i32, RaidEntry>,
    pub(crate) stored: BTreeMap<i32, RaidInfo>,
    schedules: HashMap<i32, TimerId>,
}

/// Java `Spawn.setRespawnDelay(delay, randomInterval)`: `(min, max)` in milliseconds.
fn respawn_delays(delay: i32, random: i32) -> (i32, i32) {
    if delay == 0 {
        return (0, 0);
    }
    let min = delay.wrapping_sub(random);
    let max = delay.wrapping_add(random);
    (min.max(1).wrapping_mul(1000), max.max(1).wrapping_mul(1000))
}

impl World {
    /// Java `RaidBossSpawnManager.load()`: the spawn list is read and every boss that is due
    /// stands in the world; the others wait for their respawn time. Call after the spawns of the
    /// datapack (`GameServer`: `RaidBossSpawnManager.getInstance()` follows `SpawnData`).
    pub fn spawn_raid_bosses(&mut self, rows: Vec<RaidBossData>) {
        // `if (!_spawns.isEmpty()) deleteSpawn(spawn, false)` for each, then the maps are cleared.
        let defined: Vec<i32> = self.boss.raid.spawns.keys().copied().collect();
        for boss_id in defined {
            self.raid_delete_spawn(boss_id, false);
        }
        self.boss.raid.bosses.clear();
        self.boss.raid.spawns.clear();
        self.boss.raid.stored.clear();
        for (_, timer) in self.boss.raid.schedules.drain() {
            self.timers.cancel(timer);
        }

        for row in rows {
            if self.npc_world.template(row.boss_id).is_none() {
                // NOTE(java-quirk): `new Spawn(id)` throws for an unknown template and the catch
                // is outside the loop, so Java drops this row and every one after it.
                warn!(
                    "RaidBossSpawnManager: Error while initializing RaidBossSpawnManager: no template {}",
                    row.boss_id
                );
                break;
            }
            let (min_ms, max_ms) = respawn_delays(row.respawn_delay, row.respawn_random);
            self.raid_add_new_spawn(
                &row,
                min_ms,
                max_ms,
                row.respawn_time,
                row.current_hp,
                row.current_mp,
                false,
            );
        }
        info!(
            "RaidBossSpawnManager: Loaded {} Instances",
            self.boss.raid.bosses.len()
        );
        info!(
            "RaidBossSpawnManager: Scheduled {} Instances",
            self.boss.raid.schedules.len()
        );
    }

    /// Java `Spawn.doSpawn()`: `_doRespawn ? doSpawn(false) : null`.
    fn raid_do_spawn(&mut self, index: usize) -> Option<ObjectId> {
        if !self.is_respawn_enabled(index) {
            return None;
        }
        let mut report = SpawnReport::default();
        self.initialize_npc(index, None, &mut report)
    }

    /// Java `addNewSpawn(spawnDat, respawnTime, currentHP, currentMP, storeInDb)`. `spawn` carries
    /// the place and the amount; the delays are the `Spawn`'s own, in milliseconds. Returns
    /// `false` when the boss is defined already (Java returns silently).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn raid_add_new_spawn(
        &mut self,
        spawn: &RaidBossData,
        min_ms: i32,
        max_ms: i32,
        respawn_time: i64,
        current_hp: f64,
        current_mp: f64,
        store_in_db: bool,
    ) -> bool {
        let boss_id = spawn.boss_id;
        if self.boss.raid.spawns.contains_key(&boss_id) {
            return false;
        }
        let now = unix_ms_now();
        // `SpawnTable.getInstance().addSpawn(spawnDat)`.
        let index = self.add_runtime_spawn(NpcSpawn {
            npc_id: boss_id,
            amount: spawn.amount,
            location: l2_data::Location {
                x: spawn.loc_x,
                y: spawn.loc_y,
                z: spawn.loc_z,
            },
            heading: spawn.heading,
            respawn_min_ms: min_ms,
            respawn_max_ms: max_ms,
            // `new Spawn(...)` starts with `_doRespawn = true`; `init()` is never called.
            respawn_enabled: true,
            chase_range: 0,
            name: None,
            territory: None,
            period_of_day: None,
            ai_data: None,
            group: None,
            source: "raidboss_spawnlist".to_owned(),
        });
        let entry = RaidSpawn {
            index,
            min_ms,
            max_ms,
        };
        if respawn_time == 0 || now > respawn_time {
            if let Some(npc) = self.raid_do_spawn(index) {
                self.set_current_hp(npc, current_hp, true);
                self.set_current_mp(npc, current_mp, true);
                self.boss.raid.bosses.insert(
                    boss_id,
                    RaidEntry {
                        npc,
                        status: RaidBossStatus::Alive,
                    },
                );
                self.boss.raid.stored.insert(
                    boss_id,
                    RaidInfo {
                        current_hp,
                        current_mp,
                        respawn_time: 0,
                    },
                );
            }
        } else {
            let delay = u64::try_from(respawn_time - now).unwrap_or(0);
            self.raid_schedule_spawn(boss_id, delay);
        }
        self.boss.raid.spawns.insert(boss_id, entry);

        if store_in_db && self.boss_writes_enabled() {
            self.boss_write(BossWrite::RaidBossInsert(RaidBossData {
                respawn_time,
                current_hp,
                current_mp,
                ..spawn.clone()
            }));
        }
        true
    }

    fn raid_schedule_spawn(&mut self, boss_id: i32, delay_ms: u64) {
        let due = self.tick + TimerWheel::ms_to_ticks(delay_ms);
        let timer = self
            .timers
            .schedule_at(due, TimerKind::Boss(BossTimer::RaidSpawn(boss_id)));
        if let Some(old) = self.boss.raid.schedules.insert(boss_id, timer) {
            self.timers.cancel(old);
        }
    }

    /// Java `SpawnSchedule.run()`: the boss is spawned again.
    pub(crate) fn on_raid_spawn_timer(&mut self, boss_id: i32) {
        let Some(spawn) = self.boss.raid.spawns.get(&boss_id).copied() else {
            // Java: `_spawns.get(bossId).doSpawn()` throws a NullPointerException.
            self.boss.raid.schedules.remove(&boss_id);
            return;
        };
        if let Some(npc) = self.raid_do_spawn(spawn.index) {
            let (hp, mp) = self
                .hp_and_max(npc)
                .zip(self.mp_and_max(npc))
                .map_or((0.0, 0.0), |(h, m)| (h.0, m.0));
            self.boss.raid.stored.insert(
                boss_id,
                RaidInfo {
                    current_hp: hp,
                    current_mp: mp,
                    respawn_time: 0,
                },
            );
            info!(
                "RaidBossSpawnManager: Spawning Raid Boss {}",
                self.boss_name(boss_id)
            );
            self.boss.raid.bosses.insert(
                boss_id,
                RaidEntry {
                    npc,
                    status: RaidBossStatus::Alive,
                },
            );
        }
        self.boss.raid.schedules.remove(&boss_id);
    }

    /// Java `updateStatus(boss, isBossDead)`.
    pub(crate) fn raid_update_status(&mut self, npc: ObjectId, dead: bool) {
        let Some(boss_id) = self.npcs.get(&npc).map(|n| n.npc_id) else {
            return;
        };
        if !self.boss.raid.stored.contains_key(&boss_id) {
            return;
        }
        if dead {
            self.raid_set_status(boss_id, npc, RaidBossStatus::Dead);
            let Some(spawn) = self.boss.raid.spawns.get(&boss_id).copied() else {
                return;
            };
            // `(int) (getRespawnMinDelay() * RAID_MIN_RESPAWN_MULTIPLIER)`: int times float.
            let min = (spawn.min_ms as f32 * self.boss.settings.raid_min_respawn_multiplier) as i32;
            let max = (spawn.max_ms as f32 * self.boss.settings.raid_max_respawn_multiplier) as i32;
            let delay = self.rng().get(min, max);
            let respawn_time = unix_ms_now() + i64::from(delay);
            let (max_hp, max_mp) = self
                .hp_and_max(npc)
                .zip(self.mp_and_max(npc))
                .map_or((0.0, 0.0), |(h, m)| (h.1, m.1));
            if let Some(info) = self.boss.raid.stored.get_mut(&boss_id) {
                info.current_hp = max_hp;
                info.current_mp = max_mp;
                info.respawn_time = respawn_time;
            }
            if !self.boss.raid.schedules.contains_key(&boss_id) && (min > 0 || max > 0) {
                info!(
                    "RaidBossSpawnManager: Updated {} respawn time to {}",
                    self.boss_name(boss_id),
                    super::java_date(respawn_time)
                );
                self.raid_schedule_spawn(boss_id, u64::try_from(delay).unwrap_or(0));
                self.raid_update_db();
            }
        } else {
            self.raid_set_status(boss_id, npc, RaidBossStatus::Alive);
            let hp_mp = self.hp_and_max(npc).zip(self.mp_and_max(npc));
            if let (Some(info), Some((hp, mp))) = (self.boss.raid.stored.get_mut(&boss_id), hp_mp) {
                info.current_hp = hp.0;
                info.current_mp = mp.0;
                info.respawn_time = 0;
            }
        }
    }

    /// `boss.setRaidStatus(status)` of the NPC the manager holds for `boss_id`.
    fn raid_set_status(&mut self, boss_id: i32, npc: ObjectId, status: RaidBossStatus) {
        if let Some(entry) = self.boss.raid.bosses.get_mut(&boss_id)
            && entry.npc == npc
        {
            entry.status = status;
        }
    }

    /// Java `updateDb()`: the respawn time, HP and MP of every boss the manager holds.
    fn raid_update_db(&mut self) {
        let ids: Vec<i32> = self.boss.raid.stored.keys().copied().collect();
        let mut rows = Vec::new();
        for boss_id in ids {
            let Some(entry) = self.boss.raid.bosses.get(&boss_id).copied() else {
                continue;
            };
            if entry.status == RaidBossStatus::Alive {
                self.raid_update_status(entry.npc, false);
            }
            let Some(info) = self.boss.raid.stored.get(&boss_id).copied() else {
                continue;
            };
            let (hp, mp) = if self.is_dead(entry.npc) {
                self.hp_and_max(entry.npc)
                    .zip(self.mp_and_max(entry.npc))
                    .map_or((info.current_hp, info.current_mp), |(h, m)| (h.1, m.1))
            } else {
                (info.current_hp, info.current_mp)
            };
            rows.push(RaidBossUpdateRow {
                boss_id,
                respawn_time: info.respawn_time,
                current_hp: hp,
                current_mp: mp,
            });
        }
        if self.boss_writes_enabled() && !rows.is_empty() {
            self.boss_write(BossWrite::RaidBossUpdate(rows));
        }
    }

    /// Java `deleteSpawn(spawnDat, updateDb)`.
    pub fn raid_delete_spawn(&mut self, boss_id: i32, update_db: bool) {
        let Some(spawn) = self.boss.raid.spawns.remove(&boss_id) else {
            return;
        };
        // `SpawnTable.getInstance().removeSpawn(spawnDat)`: the runtime spawn stays in the list
        // (indices are stable) but never spawns again.
        self.stop_respawn(spawn.index);
        self.boss.raid.bosses.remove(&boss_id);
        if let Some(timer) = self.boss.raid.schedules.remove(&boss_id) {
            self.timers.cancel(timer);
        }
        self.boss.raid.stored.remove(&boss_id);
        if update_db && self.boss_writes_enabled() {
            self.boss_write(BossWrite::RaidBossDelete { boss_id });
        }
    }

    /// Java `AdminSpawn.spawnMonster` for a template of type `RaidBoss`: the spawn goes to the
    /// manager with the delays 43200 and 129600 (`permanent` writes the row, with
    /// `CycleDatabaseWrites`); a spawn that is not permanent or has no respawn delay stops
    /// respawning right after.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn admin_spawn_raid_boss(
        &mut self,
        boss_id: i32,
        (x, y, z): (i32, i32, i32),
        heading: i32,
        amount: i32,
        (base_hp, base_mp): (f64, f64),
        permanent: bool,
        respawn_time: i32,
    ) {
        let data = RaidBossData {
            boss_id,
            amount,
            loc_x: x,
            loc_y: y,
            loc_z: z,
            heading,
            respawn_delay: respawn_time,
            respawn_random: 0,
            respawn_time: 0,
            current_hp: base_hp,
            current_mp: base_mp,
        };
        // NOTE(java-quirk): `setRespawnMinDelay(43200)` and `setRespawnMaxDelay(129600)` are
        // meant as seconds (12 and 36 hours) but the fields hold milliseconds: 43 to 130 seconds.
        self.raid_add_new_spawn(&data, 43_200, 129_600, 0, base_hp, base_mp, permanent);
        if !permanent || respawn_time <= 0 {
            self.raid_stop_respawn(boss_id);
        }
    }

    /// Java `AdminTeleport` `admin_recall_npc` for a `RaidBoss` with a spawn: the spawn is deleted
    /// (with its row) and a new one is made at the GM's place with the boss's current HP and MP.
    // NOTE(java-quirk): `deleteSpawn` does not delete the NPC, so the old boss stays in the world
    // next to the new one.
    pub(crate) fn admin_recall_raid_boss(&mut self, npc: ObjectId, gm: ObjectId) {
        let Some(boss_id) = self.npcs.get(&npc).map(|n| n.npc_id) else {
            return;
        };
        let (hp, mp) = self
            .hp_and_max(npc)
            .zip(self.mp_and_max(npc))
            .map_or((0.0, 0.0), |(h, m)| (h.0, m.0));
        self.raid_delete_spawn(boss_id, true);
        let (x, y, z) = self.position_of(gm).unwrap_or((0, 0, 0));
        let heading = self.players.get(&gm).map_or(0, |p| p.core.heading);
        let data = RaidBossData {
            boss_id,
            amount: 1,
            loc_x: x,
            loc_y: y,
            loc_z: z,
            heading,
            respawn_delay: 0,
            respawn_random: 0,
            respawn_time: 0,
            current_hp: hp,
            current_mp: mp,
        };
        if !self.raid_add_new_spawn(&data, 43_200, 129_600, 0, hp, mp, true) {
            self.send(gm, &SystemMessage::new(SystemMessageId::YourTargetCannotBeFound));
        }
    }

    /// Java `isDefined(bossId)`.
    pub fn raid_boss_defined(&self, boss_id: i32) -> bool {
        self.boss.raid.spawns.contains_key(&boss_id)
    }

    /// Java `Spawn.stopRespawn()` of a raid boss spawn (`//spawn` with a respawn of 0 or a spawn
    /// that is not permanent).
    pub(crate) fn raid_stop_respawn(&mut self, boss_id: i32) {
        if let Some(spawn) = self.boss.raid.spawns.get(&boss_id).copied() {
            self.stop_respawn(spawn.index);
        }
    }

    /// Java `getRaidBossStatusId(bossId)`.
    pub fn raid_boss_status(&self, boss_id: i32) -> RaidBossStatus {
        if let Some(entry) = self.boss.raid.bosses.get(&boss_id) {
            entry.status
        } else if self.boss.raid.schedules.contains_key(&boss_id) {
            RaidBossStatus::Dead
        } else {
            RaidBossStatus::Undefined
        }
    }

    /// Java `getAllRaidBossStatus()`: `name: STATUS` for each boss the manager holds.
    pub fn raid_boss_status_lines(&self) -> Vec<String> {
        self.boss
            .raid
            .bosses
            .iter()
            .map(|(id, entry)| format!("{}: {}", self.boss_name(*id), entry.status.name()))
            .collect()
    }

    /// Java `getRaidBossStatus(bossId)`.
    pub fn raid_boss_status_text(&self, boss_id: i32) -> String {
        let mut msg = String::from("RaidBoss Status...");
        msg.push_str(if cfg!(windows) { "\r\n" } else { "\n" });
        if let Some(entry) = self.boss.raid.bosses.get(&boss_id) {
            msg.push_str(&format!("{}: {}", self.boss_name(boss_id), entry.status.name()));
        }
        msg
    }

    /// Java `notifySpawnNightBoss(raidboss)`: the day and night manager spawned the boss.
    pub fn raid_notify_spawn_night_boss(&mut self, npc: ObjectId) {
        let Some(boss_id) = self.npcs.get(&npc).map(|n| n.npc_id) else {
            return;
        };
        let (hp, mp) = self
            .hp_and_max(npc)
            .zip(self.mp_and_max(npc))
            .map_or((0.0, 0.0), |(h, m)| (h.0, m.0));
        self.boss.raid.stored.insert(
            boss_id,
            RaidInfo {
                current_hp: hp,
                current_mp: mp,
                respawn_time: 0,
            },
        );
        info!(
            "RaidBossSpawnManager: Spawning Night Raid Boss {}",
            self.boss_name(boss_id)
        );
        self.boss.raid.bosses.insert(
            boss_id,
            RaidEntry {
                npc,
                status: RaidBossStatus::Alive,
            },
        );
    }

    /// The respawn time the manager holds for a raid boss (`getStoredInfo().get(id)`), epoch
    /// milliseconds; 0 while the boss is alive.
    pub fn raid_boss_respawn_time(&self, boss_id: i32) -> Option<i64> {
        self.boss.raid.stored.get(&boss_id).map(|i| i.respawn_time)
    }

    /// The NPC the manager holds for a raid boss (`getBosses().get(id)`).
    pub fn raid_boss_npc(&self, boss_id: i32) -> Option<ObjectId> {
        self.boss.raid.bosses.get(&boss_id).map(|e| e.npc)
    }

    /// Java `cleanUp()`: stores the rows, then forgets the bosses and cancels the respawns.
    pub fn raid_clean_up(&mut self) {
        self.raid_update_db();
        self.boss.raid.bosses.clear();
        for (_, timer) in self.boss.raid.schedules.drain() {
            self.timers.cancel(timer);
        }
        self.boss.raid.stored.clear();
        self.boss.raid.spawns.clear();
    }

    /// Java `RaidBoss.doDie` / `GrandBoss.doDie` after `Monster.doDie`: the raid points of a kill
    /// and, for a raid boss, the new status.
    pub(crate) fn boss_npc_died(&mut self, npc: ObjectId, killer: Option<ObjectId>) {
        let Some((class, boss_id, level)) = self.npcs.get(&npc).map(|n| (n.class, n.npc_id, n.core.level))
        else {
            return;
        };
        if !matches!(class, NpcClass::RaidBoss | NpcClass::GrandBoss) {
            return;
        }
        // `killer != null && killer.isPlayable()`: `asPlayer()` is the owner of a summon.
        let player = killer.and_then(|k| {
            if self.is_player(k) {
                Some(k)
            } else {
                self.summon_owner(k)
            }
        });
        if let Some(player) = player {
            let message = SystemMessage::new(SystemMessageId::CongratulationsYourRaidWasSuccessful);
            let members = match self.party_of(player) {
                Some(party) => {
                    self.broadcast_party(party, &message);
                    self.party_members(party)
                }
                None => {
                    self.send(player, &message);
                    vec![player]
                }
            };
            for member in members {
                let points = level / 2 + self.rng().get(-5, 5);
                self.add_raid_points(member, boss_id, points);
                // FOLLOW-UP(M8 hero): `if (member.isNoble()) Hero.getInstance().setRBkilled(member
                // object id, getId())` once the hero card exists.
            }
        }
        if class == NpcClass::RaidBoss {
            self.raid_update_status(npc, true);
        }
    }
}
