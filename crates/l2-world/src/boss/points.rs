//! Java `managers/RaidBossPointsManager.java`: the raid points of every player (the sum of the
//! points of each boss), the ranking, `RequestGetBossRecord`'s numbers and the weekly reset.
//
// # Java's `HashMap` order
// `getList(player)` is a `HashMap<Integer, Integer>`, and `ExGetBossRecord` writes it in the
// map's order; the ranking sorts a `HashMap` of the players by points, so players with the same
// points keep the map's order. [`java_hash_order`] reproduces the order of a `HashMap` with
// integer keys (table size from the entry count, bucket from the spread hash, insertion order
// inside a bucket). Java's `_list` itself is a `ConcurrentHashMap`; its order is not modelled, the
// players count in the order they got their first points.

use std::collections::HashMap;

use l2_net::Encode;
use l2_protocol_game::server::{BossRecordEntry, ExGetBossRecord};
use l2_util::ObjectId;
use tracing::info;

use super::BossTimer;
use crate::world::{World, unix_ms_now};

/// A `character_raid_points` row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RaidPointsRecord {
    pub char_id: i32,
    pub boss_id: i32,
    pub points: i32,
}

/// Java `_list`: character id to `{boss id: points}`.
#[derive(Default)]
pub(crate) struct RaidPoints {
    /// The characters in the order they first got points.
    order: Vec<i32>,
    /// The points of each boss in the order the bosses were first added (Java's inner `HashMap`
    /// order is [`java_hash_order`] of this).
    by_char: HashMap<i32, Vec<(i32, i32)>>,
}

impl RaidPoints {
    fn add(&mut self, char_id: i32, boss_id: i32, points: i32) -> i32 {
        if !self.by_char.contains_key(&char_id) {
            self.order.push(char_id);
        }
        let entries = self.by_char.entry(char_id).or_default();
        // `merge(bossId, points, Integer::sum)`.
        match entries.iter_mut().find(|(b, _)| *b == boss_id) {
            Some((_, total)) => {
                *total = total.wrapping_add(points);
                *total
            }
            None => {
                entries.push((boss_id, points));
                points
            }
        }
    }

    fn total(&self, char_id: i32) -> i32 {
        // `for (int points : tmpPoint.values()) totalPoints += points`.
        self.by_char
            .get(&char_id)
            .map_or(0, |e| e.iter().fold(0i32, |sum, (_, p)| sum.wrapping_add(*p)))
    }

    fn clear(&mut self) {
        self.order.clear();
        self.by_char.clear();
    }
}

/// The indices of `keys` in the iteration order of a Java `HashMap<Integer, ?>` that got the keys
/// in this order and never lost one.
pub(crate) fn java_hash_order(keys: &[i32]) -> Vec<usize> {
    let mut table: usize = 16;
    // The table doubles when a put makes the size exceed 0.75 of it.
    while keys.len() > table / 4 * 3 {
        table *= 2;
    }
    let bucket = |key: i32| {
        let h = key as u32;
        ((h ^ (h >> 16)) as usize) & (table - 1)
    };
    let mut order: Vec<usize> = (0..keys.len()).collect();
    // A stable sort keeps the insertion order inside a bucket.
    order.sort_by_key(|&i| bucket(keys[i]));
    order
}

/// The numbers `RequestGetBossRecord` answers with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BossRecord {
    /// `calculateRanking`: 0 without points.
    pub ranking: i32,
    /// `getPointsByOwnerId`.
    pub total_points: i32,
    /// `getList(player)` in the map's order; `None` for a player without a map.
    pub list: Option<Vec<(i32, i32)>>,
}

/// Java `TimeUtil.getNextTime(hour, minute)`: the next hh:mm:00.000 in the zone, at or after
/// `now_ms` (`offset_at(utc_ms)` is the zone's offset at that moment, D-48).
pub(crate) fn next_time_ms(now_ms: i64, hour: i64, minute: i64, offset_at: impl Fn(i64) -> i64) -> i64 {
    const DAY_MS: i64 = 86_400_000;
    let of_day = (hour * 60 + minute) * 60_000;
    let local_now = now_ms + offset_at(now_ms);
    let day_start = local_now - local_now.rem_euclid(DAY_MS);
    let to_utc = |local: i64| {
        let guess = local - offset_at(now_ms);
        local - offset_at(guess)
    };
    let today = to_utc(day_start + of_day);
    // "If the target time has already passed today, schedule for the next day."
    if today < now_ms {
        to_utc(day_start + DAY_MS + of_day)
    } else {
        today
    }
}

impl World {
    /// Java `RaidBossPointsManager.init()`.
    pub(crate) fn load_raid_points(&mut self, rows: Vec<RaidPointsRecord>) {
        for row in rows {
            // `values.put(bossId, points)`: a later row of the same pair replaces the points.
            let entries = self.boss.points.by_char.entry(row.char_id).or_default();
            if entries.is_empty() {
                self.boss.points.order.push(row.char_id);
            }
            match entries.iter_mut().find(|(b, _)| *b == row.boss_id) {
                Some((_, p)) => *p = row.points,
                None => entries.push((row.boss_id, row.points)),
            }
        }
        info!(
            "RaidBossPointsManager: Loaded {} Characters Raid Points.",
            self.boss.points.order.len()
        );
    }

    /// Java `RaidBossPointsManager` constructor: `scheduleAtFixedRate(resetRaidPoints, until the
    /// next 00:10, 24 h)`.
    pub(crate) fn start_raid_points_reset_task(&mut self) {
        self.start_raid_points_reset_task_at(unix_ms_now(), l2_util::time::local_offset_millis);
    }

    /// [`Self::start_raid_points_reset_task`] for a given clock.
    pub fn start_raid_points_reset_task_at(&mut self, now_ms: i64, offset_at: impl Fn(i64) -> i64) {
        let next = next_time_ms(now_ms, 0, 10, offset_at);
        let delay = u64::try_from(next.saturating_sub(now_ms)).unwrap_or(0);
        self.schedule_boss_periodic("points_reset", delay, BossTimer::PointsReset);
    }

    /// The timer of the fixed rate: the next run in 24 hours (a fixed period, like Java), then
    /// the run itself.
    pub(crate) fn on_raid_points_reset_timer(&mut self) {
        self.schedule_boss_periodic("points_reset", 86_400_000, BossTimer::PointsReset);
        let local_day = l2_util::time::local_now_millis().div_euclid(86_400_000);
        self.reset_raid_points_on_day(local_day);
    }

    /// Java `resetRaidPoints()` on the local day `local_day` (days since 1970-01-01, which was a
    /// Thursday): only a Monday does anything. Clans get reputation by the ranking of their
    /// members, then every point goes. Both are global jobs the Java game server runs as well
    /// (D-49): without `CycleDatabaseWrites` this server only forgets the points in memory.
    pub fn reset_raid_points_on_day(&mut self, local_day: i64) {
        // Thursday is 0 days after the epoch; Monday is `(day + 3) % 7 == 0`.
        if (local_day + 3).rem_euclid(7) != 0 {
            return;
        }
        if self.boss_writes_enabled() {
            self.reward_clan_reputation_by_ranking();
        }
        self.raid_points_clean_up();
        info!("RaidBossPointsManager: Reset task launched.");
    }

    /// The reward half of `resetRaidPoints`.
    fn reward_clan_reputation_by_ranking(&mut self) {
        let rank_list = self.raid_rank_list();
        let settings = self.boss.settings.clone();
        let clan_ids: Vec<i32> = self.clans().map(|c| c.id).collect();
        for clan_id in clan_ids {
            for (char_id, rank) in &rank_list {
                let is_member = self.clan(clan_id).is_some_and(|c| c.is_member(*char_id));
                if *rank > 100 || !is_member {
                    continue;
                }
                let reputation = match *rank {
                    1..=10 => settings.raid_ranking[(*rank - 1) as usize],
                    11..=50 => settings.raid_ranking_up_to_50th,
                    _ => settings.raid_ranking_up_to_100th,
                };
                self.add_reputation_score(clan_id, reputation);
            }
        }
    }

    /// Java `cleanUp()`: the rows are deleted (with `CycleDatabaseWrites`) and the points go.
    pub fn raid_points_clean_up(&mut self) {
        if self.boss_writes_enabled() {
            self.boss_write(super::BossWrite::RaidPointsDeleteAll);
        }
        self.boss.points.clear();
    }

    /// Java `addPoints(player, bossId, points)`: the new total of the boss is written for the
    /// player (`REPLACE`), as Java does for every player.
    pub fn add_raid_points(&mut self, player: ObjectId, boss_id: i32, points: i32) {
        let total = self.boss.points.add(player.0, boss_id, points);
        self.boss_write(super::BossWrite::RaidPoints {
            char_id: player.0,
            boss_id,
            points: total,
        });
    }

    /// Java `getPointsByOwnerId(ownerId)`.
    pub fn raid_points_of(&self, owner: i32) -> i32 {
        self.boss.points.total(owner)
    }

    /// Java `getList(player)`: the points of each boss in the order of Java's `HashMap`.
    pub fn raid_points_list(&self, owner: i32) -> Option<Vec<(i32, i32)>> {
        let entries = self.boss.points.by_char.get(&owner)?;
        let keys: Vec<i32> = entries.iter().map(|(b, _)| *b).collect();
        Some(java_hash_order(&keys).into_iter().map(|i| entries[i]).collect())
    }

    /// Java `getRankList()`: `(character id, rank)` for every character with points other than 0,
    /// best first (equal points keep the order of Java's `HashMap`).
    pub fn raid_rank_list(&self) -> Vec<(i32, i32)> {
        let with_points: Vec<(i32, i32)> = self
            .boss
            .points
            .order
            .iter()
            .filter_map(|c| {
                let total = self.boss.points.total(*c);
                (total != 0).then_some((*c, total))
            })
            .collect();
        let keys: Vec<i32> = with_points.iter().map(|(c, _)| *c).collect();
        let mut list: Vec<(i32, i32)> = java_hash_order(&keys)
            .into_iter()
            .map(|i| with_points[i])
            .collect();
        // `comparingInt(getValue).reversed()`, a stable sort.
        list.sort_by_key(|(_, points)| std::cmp::Reverse(*points));
        list.into_iter()
            .enumerate()
            .map(|(i, (char_id, _))| (char_id, i as i32 + 1))
            .collect()
    }

    /// Java `calculateRanking(playerObjId)`.
    pub fn raid_ranking_of(&self, owner: i32) -> i32 {
        self.raid_rank_list()
            .into_iter()
            .find(|(c, _)| *c == owner)
            .map_or(0, |(_, rank)| rank)
    }

    /// What `RequestGetBossRecord.runImpl` sends in `ExGetBossRecord`.
    pub fn boss_record(&self, player: ObjectId) -> BossRecord {
        BossRecord {
            ranking: self.raid_ranking_of(player.0),
            total_points: self.raid_points_of(player.0),
            list: self.raid_points_list(player.0),
        }
    }

    /// Java `RequestGetBossRecord.runImpl`: `ExGetBossRecord` with the ranking, the points and the
    /// points of each boss.
    pub(crate) fn on_request_get_boss_record(&mut self, player: ObjectId, boss_id: i32) {
        if !self.players.contains_key(&player) {
            return;
        }
        if boss_id != 0 {
            // `PacketLogger.info("C5: RequestGetBossRecord: d: ..")`, "should be always 0".
            info!("C5: RequestGetBossRecord: d: {boss_id} ActiveChar: {}", player.0);
        }
        let record = self.boss_record(player);
        let packet = ExGetBossRecord {
            ranking: record.ranking,
            total_points: record.total_points,
            records: record
                .list
                .iter()
                .flatten()
                .map(|(boss_id, points)| BossRecordEntry {
                    boss_id: *boss_id,
                    points: *points,
                })
                .collect(),
        };
        let mut bytes = packet.to_bytes();
        if record.list.is_none() {
            // NOTE(java-quirk): `ExGetBossRecord` writes four zeros for a null list (the count
            // and three more), where an empty list is the count and nothing else.
            bytes.extend_from_slice(&[0; 12]);
        }
        self.send_bytes(player, bytes);
    }

    /// D-50: the rows of a character are read again when it enters the world (the Java game
    /// server may have given it points since this server started).
    pub(crate) fn request_raid_points_reread(&mut self, char_id: i32) {
        // Without the managers (nothing loaded) there is nothing to refresh.
        if self.boss.started_ms == 0 {
            return;
        }
        self.outbox
            .persist(crate::outbox::PersistCommand::BossRereadRequest {
                kind: super::BossReadKind::RaidPoints(char_id),
                then: super::BossReadThen::Nothing,
            });
    }

    /// The answer of that read: the rows replace what this server holds of the character.
    pub(crate) fn apply_raid_points_rows(&mut self, char_id: i32, rows: &[RaidPointsRecord]) {
        let entries: Vec<(i32, i32)> = rows
            .iter()
            .filter(|r| r.char_id == char_id)
            .map(|r| (r.boss_id, r.points))
            .collect();
        if entries.is_empty() {
            self.boss.points.by_char.remove(&char_id);
            self.boss.points.order.retain(|c| *c != char_id);
        } else {
            if !self.boss.points.by_char.contains_key(&char_id) {
                self.boss.points.order.push(char_id);
            }
            self.boss.points.by_char.insert(char_id, entries);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_keys_come_out_by_bucket() {
        // 16 buckets: 29001 & 15 = 9, 25001 & 15 = 9, 29020 & 15 = 12, 29028 & 15 = 4.
        let keys = [29001, 29020, 29028, 25001];
        let order: Vec<i32> = java_hash_order(&keys).into_iter().map(|i| keys[i]).collect();
        assert_eq!(order, [29028, 29001, 25001, 29020]);
    }

    #[test]
    fn the_table_grows_after_twelve_entries() {
        // 13 keys need 32 buckets: key 16 sits after key 3 only in a 32 table when 16 > 3,
        // and key 17 (bucket 1 of 16, bucket 17 of 32) comes after 2.
        let keys: Vec<i32> = (0..13).map(|i| i * 17).collect();
        let order: Vec<i32> = java_hash_order(&keys).into_iter().map(|i| keys[i]).collect();
        let mut by_bucket = keys.clone();
        by_bucket.sort_by_key(|k| (*k as usize) & 31);
        assert_eq!(order, by_bucket);
    }

    #[test]
    fn the_next_time_is_today_or_tomorrow() {
        let utc = |_: i64| 0;
        // 1970-01-01 00:05 -> 00:10 the same day; 00:15 -> the next day.
        assert_eq!(next_time_ms(5 * 60_000, 0, 10, utc), 10 * 60_000);
        assert_eq!(next_time_ms(15 * 60_000, 0, 10, utc), 86_400_000 + 10 * 60_000);
        // A zone two hours ahead: local 00:10 is 22:10 UTC the day before.
        let ahead = |_: i64| 2 * 3_600_000;
        assert_eq!(next_time_ms(0, 0, 10, ahead), 22 * 3_600_000 + 10 * 60_000);
    }
}
