//! The vitality system: the points a player has (0 to 20000, shown in `UserInfo`), the level they
//! make (0 to 4), the exp and SP multiplier of that level, the points a kill uses up and the
//! points a rest in a peace zone or a time offline gives back.
// Java: entity/actor/stat/PlayerStat.java (updateVitalityLevel, setVitalityPoints,
//       updateVitalityPoints, getVitalityMultiplier, getVitalityLevel)
// Java: entity/actor/Attackable.java (getVitalityPoints, useVitalityRate)
// Java: entity/actor/instance/{RaidBoss,GrandBoss}.java (getVitalityPoints, useVitalityRate)
// Java: entity/actor/tasks/player/VitalityTask.java, entity/actor/Player.java (startVitalityTask)
// Java: network/clientpackets/EnterWorld.java (RecoverVitalityOnReconnect)
//
// # Where the points live
// Java keeps `_vitalityPoints` as a `float` in `PlayerStat` and writes `(int) points` to
// `characters.vitality_points` with every store. Here the float and the level are in
// [`PlayerVitality`] (`ExpWorld::vitality`), created from the profile when the player first needs
// them (Java `Player.restore` -> `setVitalityPoints(value, true)`), and the whole-number part is
// mirrored into `PlayerProfile::vitality_points` after every change. That mirror is what `UserInfo`
// shows and what the persistence task writes, so the Java GS reads exactly the number it would
// have stored itself (D-39).
//
// # What is left out
// - Parties (`Party.distributeXpAndSp`): there are none. The kill share is the single-player
//   branch of `Attackable.calculateRewards`.
// - Champions (`_champion`) are not rolled, so `useVitalityRate()` is only false for bosses.
// - Items and skills that change vitality (`VitalityPointUp`, `Stat::VitalityConsumeRate` buffs
//   apart from the stat itself): M5.

use std::collections::HashMap;

use l2_data::Stat;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ExVitalityPointInfo, SystemMessage};
use l2_util::ObjectId;

use crate::npc::NpcClass;
use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::World;
use crate::zones::ZoneId;

/// Java `PlayerStat.VITALITY_LEVELS`: the points up to which a level lasts (level 0 up to 240,
/// level 1 up to 2000, ... level 4 above 17000).
pub const VITALITY_LEVELS: [i32; 5] = [240, 2000, 13000, 17000, 20000];
/// Java `PlayerStat.MAX_VITALITY_POINTS`.
pub const MAX_VITALITY_POINTS: i32 = VITALITY_LEVELS[4];
/// Java `PlayerStat.MIN_VITALITY_POINTS`.
pub const MIN_VITALITY_POINTS: i32 = 1;

/// Java `Player.startVitalityTask`: `scheduleAtFixedRate(new VitalityTask(this), 1000, 60000)`.
const VITALITY_TASK_FIRST_MS: u64 = 1_000;
const VITALITY_TASK_PERIOD_MS: u64 = 60_000;

/// The `.ini` values vitality reads. `Default` is the Java default of each key.
#[derive(Debug, Clone, PartialEq)]
pub struct VitalitySettings {
    /// `PlayerConfig.ENABLE_VITALITY` (`EnableVitality`).
    pub enable_vitality: bool,
    /// `PlayerConfig.RECOVER_VITALITY_ON_RECONNECT` (`RecoverVitalityOnReconnect`).
    pub recover_vitality_on_reconnect: bool,
    /// `PlayerConfig.RAIDBOSS_USE_VITALITY` (`RaidbossUseVitality`).
    pub raidboss_use_vitality: bool,
    /// `RatesConfig.RATE_VITALITY_LEVEL_1` to `_4` (`RateVitalityLevel1` to `4`).
    pub rate_vitality_level: [f32; 4],
    /// `RatesConfig.RATE_VITALITY_GAIN` (`RateVitalityGain`).
    pub rate_vitality_gain: f32,
    /// `RatesConfig.RATE_VITALITY_LOST` (`RateVitalityLost`).
    pub rate_vitality_lost: f32,
    /// `RatesConfig.RATE_RECOVERY_VITALITY_PEACE_ZONE` (`RateRecoveryPeaceZone`).
    pub rate_recovery_vitality_peace_zone: f32,
    /// `RatesConfig.RATE_RECOVERY_ON_RECONNECT` (`RateRecoveryOnReconnect`).
    pub rate_recovery_on_reconnect: f32,
}

impl Default for VitalitySettings {
    fn default() -> Self {
        Self {
            enable_vitality: true,
            recover_vitality_on_reconnect: true,
            raidboss_use_vitality: true,
            rate_vitality_level: [1.5, 2.0, 2.5, 3.0],
            rate_vitality_gain: 1.0,
            rate_vitality_lost: 1.0,
            rate_recovery_vitality_peace_zone: 1.0,
            rate_recovery_on_reconnect: 4.0,
        }
    }
}

/// Java `PlayerStat.updateVitalityLevel`: the level of a number of points.
pub fn vitality_level_of(points: f32) -> u8 {
    // `_vitalityPoints <= VITALITY_LEVELS[n]` compares a float with an int (promoted to float).
    if points <= VITALITY_LEVELS[0] as f32 {
        0
    } else if points <= VITALITY_LEVELS[1] as f32 {
        1
    } else if points <= VITALITY_LEVELS[2] as f32 {
        2
    } else if points <= VITALITY_LEVELS[3] as f32 {
        3
    } else {
        4
    }
}

/// One player's vitality (Java `PlayerStat._vitalityPoints`, `_vitalityLevel` and the
/// `_vitalityTask` of `Player`).
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerVitality {
    /// Java `_vitalityPoints` (a `float`, so a kill can use up a fraction of a point).
    pub points: f32,
    /// Java `_vitalityLevel`.
    pub level: u8,
    /// The pending run of the 60 s recovery task.
    task: Option<TimerId>,
}

impl PlayerVitality {
    /// Java `Player.restore` -> `setVitalityPoints(value, true)`: the stored number clamped to the
    /// range, the level that goes with it, no message.
    pub fn restored(stored: i32) -> Self {
        let points = stored.clamp(MIN_VITALITY_POINTS, MAX_VITALITY_POINTS) as f32;
        Self {
            points,
            level: vitality_level_of(points),
            task: None,
        }
    }
}

/// The vitality of every player in the world (a field of [`ExpWorld`](super::exp::ExpWorld)).
#[derive(Debug, Default)]
pub struct VitalityWorld {
    players: HashMap<ObjectId, PlayerVitality>,
}

impl VitalityWorld {
    /// The vitality of a player, if the world created it.
    pub fn get(&self, id: ObjectId) -> Option<&PlayerVitality> {
        self.players.get(&id)
    }
}

impl World {
    /// The state of a player, created from the stored number the first time it is needed.
    fn vitality_mut(&mut self, id: ObjectId) -> Option<&mut PlayerVitality> {
        if !self.rewards.vitality.players.contains_key(&id) {
            let stored = self.players.get(&id)?.data.profile.vitality_points;
            self.rewards
                .vitality
                .players
                .insert(id, PlayerVitality::restored(stored));
        }
        self.rewards.vitality.players.get_mut(&id)
    }

    /// The points and level of a player without creating the state.
    fn vitality_of(&self, id: ObjectId) -> Option<(f32, u8)> {
        if let Some(v) = self.rewards.vitality.players.get(&id) {
            return Some((v.points, v.level));
        }
        let restored = PlayerVitality::restored(self.players.get(&id)?.data.profile.vitality_points);
        Some((restored.points, restored.level))
    }

    /// Java `PlayerStat.getVitalityPoints()`: `(int) _vitalityPoints`.
    pub fn vitality_points(&self, id: ObjectId) -> i32 {
        self.vitality_of(id).map_or(0, |(points, _)| points as i32)
    }

    /// Java `PlayerStat.getVitalityLevel()`: level 4 while Nevit's blessing is active.
    pub fn vitality_level(&self, id: ObjectId) -> u8 {
        if self.is_advent_blessing_active(id) {
            return 4;
        }
        self.vitality_of(id).map_or(0, |(_, level)| level)
    }

    /// Java `PlayerStat.getVitalityMultiplier()`: the rate of the vitality level, 1 for level 0 or
    /// with `EnableVitality = False`.
    pub fn vitality_multiplier(&self, id: ObjectId) -> f64 {
        if !self.settings.vitality.enable_vitality {
            return 1.0;
        }
        match self.vitality_level(id) {
            level @ 1..=4 => f64::from(self.settings.vitality.rate_vitality_level[usize::from(level) - 1]),
            _ => 1.0,
        }
    }

    /// Writes the whole-number part of the points where `UserInfo` and the store read it.
    fn mirror_vitality(&mut self, id: ObjectId) {
        let Some(points) = self.rewards.vitality.players.get(&id).map(|v| v.points as i32) else {
            return;
        };
        if let Some(player) = self.players.get_mut(&id) {
            player.data.profile.vitality_points = points;
        }
    }

    /// Java `PlayerStat.updateVitalityLevel(quiet)`: the new level and, unless `quiet`, the
    /// messages about it.
    fn update_vitality_level(&mut self, id: ObjectId, quiet: bool) {
        let Some(vitality) = self.rewards.vitality.players.get_mut(&id) else {
            return;
        };
        let level = vitality_level_of(vitality.points);
        let old = vitality.level;
        vitality.level = level;
        if quiet || level == old {
            return;
        }
        if level < old {
            self.send(id, &SystemMessage::new(SystemMessageId::YourVitalityHasDecreased));
        } else {
            self.send(id, &SystemMessage::new(SystemMessageId::YourVitalityHasIncreased));
        }
        if level == 0 {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YourVitalityIsFullyExhausted),
            );
        } else if level == 4 {
            self.send(id, &SystemMessage::new(SystemMessageId::YourVitalityIsAtMaximum));
        }
    }

    /// Java `PlayerStat.setVitalityPoints(value, quiet)`: sets the points (clamped), and tells the
    /// client with `ExVitalityPointInfo`.
    pub fn set_vitality_points(&mut self, id: ObjectId, value: i32, quiet: bool) {
        let points = value.clamp(MIN_VITALITY_POINTS, MAX_VITALITY_POINTS);
        let Some(vitality) = self.vitality_mut(id) else {
            return;
        };
        // `points == _vitalityPoints` compares an int with a float.
        if points as f32 == vitality.points {
            return;
        }
        vitality.points = points as f32;
        self.update_vitality_level(id, quiet);
        self.mirror_vitality(id);
        let points = self.vitality_points(id);
        self.send(id, &ExVitalityPointInfo { points });
    }

    /// Java `PlayerStat.updateVitalityPoints(value, useRates, quiet)`: adds `value` (a kill uses
    /// points up, so it is negative there), with the gain and loss rates when `use_rates`.
    pub fn update_vitality_points(&mut self, id: ObjectId, value: f32, use_rates: bool, quiet: bool) {
        if value == 0.0 || !self.settings.vitality.enable_vitality {
            return;
        }
        let mut points = value;
        if use_rates {
            if self.is_lucky(id) {
                return;
            }
            if points < 0.0 {
                // Vitality consumed.
                let mut stat = self.stats(id).map_or(1.0, |view| {
                    view.calc_stat(Stat::VitalityConsumeRate, 1.0, None, None)
                }) as i32;
                if self.is_advent_blessing_active(id) {
                    // Increase Vitality During Blessing.
                    stat = -10;
                }
                if stat == 0 {
                    return;
                }
                if stat < 0 {
                    points = -points;
                }
            }
            if points > 0.0 {
                // Vitality increased.
                points *= self.settings.vitality.rate_vitality_gain;
            } else {
                // Vitality decreased.
                points *= self.settings.vitality.rate_vitality_lost;
            }
        }

        let Some(vitality) = self.vitality_mut(id) else {
            return;
        };
        let current = vitality.points;
        if points > 0.0 {
            points = (current + points).min(MAX_VITALITY_POINTS as f32);
        } else {
            points = (current + points).max(MIN_VITALITY_POINTS as f32);
        }
        if (points - current).abs() <= 1e-6 {
            return;
        }
        vitality.points = points;
        self.update_vitality_level(id, quiet);
        self.mirror_vitality(id);
    }

    // ------------------------------------------------------------------ monsters

    /// Java `Attackable.getVitalityPoints(level, damage)` (with the overrides of `RaidBoss` and
    /// `GrandBoss`): the points a kill by `damage` uses up (negative), for a killer of `level`.
    pub(crate) fn kill_vitality_points(&self, npc: ObjectId, level: i32, damage: i64) -> f32 {
        let base = self.attackable_vitality_points(npc, level, damage);
        match self.npcs.get(&npc).map(|n| n.class) {
            // NOTE(java-quirk): a raid or grand boss negates the (negative) cost and divides it by
            // 100, so killing one gives a little vitality instead of using it up.
            Some(NpcClass::RaidBoss | NpcClass::GrandBoss) => -base / 100.0,
            _ => base,
        }
    }

    /// Java `Attackable.getVitalityPoints(level, damage)`: "vitality points decrease (if
    /// positive) or increase (if negative) based on damage. Maximum for damage = maxHp".
    // `level` only goes to `getExpReward(level)`, which ignores it without `DynamicExpRates.xml`.
    fn attackable_vitality_points(&self, npc: ObjectId, _level: i32, damage: i64) -> f32 {
        // Sanity check.
        if damage <= 0 {
            return 0.0;
        }
        let exp_reward = self.npc_exp_reward(npc);
        let Some(view) = self.stats(npc) else {
            return 0.0;
        };
        let npc_level = view.level();
        let base_hp_max = self
            .npcs
            .get(&npc)
            .and_then(|n| self.npc_world.template(n.npc_id))
            .map_or(0.0, |t| t.base_hp_max);
        // Java: a `float` expression, `(baseHpMax * 9 * level * level) / (100 * expReward)`.
        let divider: f32 = if npc_level > 0 && exp_reward > 0 {
            (base_hp_max * 9.0 * npc_level as f32 * npc_level as f32)
                / (100i64.wrapping_mul(exp_reward) as f32)
        } else {
            0.0
        };
        if divider == 0.0 {
            return 0.0;
        }
        // Negative value: the vitality will be consumed.
        let used = damage.min(i64::from(view.max_hp()));
        (-used) as f32 / divider
    }

    /// Java `Attackable.useVitalityRate()` with the overrides: "True if vitality rate for exp and
    /// sp should be applied". A raid boss follows `RaidbossUseVitality`, a grand boss never.
    pub(crate) fn use_vitality_rate(&self, npc: ObjectId) -> bool {
        match self.npcs.get(&npc).map(|n| n.class) {
            Some(NpcClass::RaidBoss) => self.settings.vitality.raidboss_use_vitality,
            Some(NpcClass::GrandBoss) => false,
            // `!_champion || CHAMPION_ENABLE_VITALITY`: champions are not rolled.
            _ => true,
        }
    }

    // ------------------------------------------------------------------ the 60 s task

    /// Java `Player.startVitalityTask`: the task runs after 1 s and then every minute.
    pub(crate) fn start_vitality_task(&mut self, id: ObjectId) {
        if !self.settings.vitality.enable_vitality || !self.players.contains_key(&id) {
            return;
        }
        if self.vitality_mut(id).is_none_or(|v| v.task.is_some()) {
            return;
        }
        let due = self.tick + TimerWheel::ms_to_ticks(VITALITY_TASK_FIRST_MS);
        let timer = self.timers.schedule_at(due, TimerKind::Vitality(id));
        if let Some(vitality) = self.rewards.vitality.players.get_mut(&id) {
            vitality.task = Some(timer);
        }
    }

    /// Java `VitalityTask.run`: in a peace zone a player below the maximum recovers
    /// `RateRecoveryPeaceZone` points.
    pub(crate) fn on_vitality_timer(&mut self, id: ObjectId) {
        // A player who left has no state any more (`forget_vitality` cancelled the timer).
        let Some(vitality) = self.rewards.vitality.players.get_mut(&id) else {
            return;
        };
        let due = self.tick + TimerWheel::ms_to_ticks(VITALITY_TASK_PERIOD_MS);
        vitality.task = Some(self.timers.schedule_at(due, TimerKind::Vitality(id)));

        if !self.is_inside_zone(id, ZoneId::Peace) {
            return;
        }
        if self.vitality_points(id) >= MAX_VITALITY_POINTS {
            return;
        }
        let rate = self.settings.vitality.rate_recovery_vitality_peace_zone;
        self.update_vitality_points(id, rate, false, false);
        let points = self.vitality_points(id);
        self.send(id, &ExVitalityPointInfo { points });
    }

    /// Java `Player.stopVitalityTask` and the end of the player object: the timer goes, the
    /// state goes.
    pub(crate) fn forget_vitality(&mut self, id: ObjectId) {
        if let Some(vitality) = self.rewards.vitality.players.remove(&id)
            && let Some(timer) = vitality.task
        {
            self.timers.cancel(timer);
        }
    }

    /// Java `EnterWorld`: "RecoverVitalityOnReconnect". The time since the last logout gives
    /// `RateRecoveryOnReconnect` points a minute, quietly.
    pub(crate) fn recover_vitality_on_reconnect(&mut self, id: ObjectId, now_ms: i64) {
        let settings = &self.settings.vitality;
        if !settings.enable_vitality || !settings.recover_vitality_on_reconnect {
            return;
        }
        let Some(last_access) = self.players.get(&id).map(|p| p.data.profile.last_access) else {
            return;
        };
        // Java: `(RATE * (System.currentTimeMillis() - getLastAccess())) / 60000` in `float`.
        let points =
            (settings.rate_recovery_on_reconnect * now_ms.wrapping_sub(last_access) as f32) / 60_000.0;
        if points > 0.0 {
            self.update_vitality_points(id, points, false, true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_change_at_the_java_thresholds() {
        // `<=` on each threshold: the point after it is the next level.
        let cases = [
            (1.0, 0),
            (240.0, 0),
            (240.5, 1),
            (2000.0, 1),
            (2001.0, 2),
            (13000.0, 2),
            (13001.0, 3),
            (17000.0, 3),
            (17001.0, 4),
            (20000.0, 4),
        ];
        for (points, level) in cases {
            assert_eq!(vitality_level_of(points), level, "{points} points");
        }
    }

    #[test]
    fn a_stored_number_is_clamped_like_set_vitality_points() {
        assert_eq!(PlayerVitality::restored(0).points, 1.0);
        assert_eq!(PlayerVitality::restored(-5).level, 0);
        assert_eq!(PlayerVitality::restored(555).level, 1);
        assert_eq!(PlayerVitality::restored(25_000).points, 20_000.0);
        assert_eq!(PlayerVitality::restored(25_000).level, 4);
    }

    #[test]
    fn the_default_settings_are_the_java_defaults() {
        let s = VitalitySettings::default();
        assert!(s.enable_vitality && s.recover_vitality_on_reconnect && s.raidboss_use_vitality);
        assert_eq!(s.rate_vitality_level, [1.5, 2.0, 2.5, 3.0]);
        assert_eq!(s.rate_recovery_on_reconnect, 4.0);
    }
}
