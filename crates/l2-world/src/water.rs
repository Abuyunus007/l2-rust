//! Drowning and falling damage of a player.
// Java: entity/zone/type/WaterZone.java (onEnter/onExit), entity/actor/tasks/player/WaterTask.java,
//       entity/actor/Player.java (startWaterTask, stopWaterTask, checkWaterState, isFalling),
//       mechanics/stats/Formulas.java (calcFallDam)
//
// The tasks live in the zone context (`zoning`), keyed by player id: they are world timers, no
// player field changes.

use l2_data::stats::Stat;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SetupGauge, SystemMessage, SystemMessageParam};
use l2_util::ObjectId;

use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::World;
use crate::zones::ZoneId;

/// Java `startWaterTask`: "Base time in water is 60 seconds".
const BASE_TIME_IN_WATER_MS: i32 = 60_000;
/// The period of `WaterTask` after the first run.
const WATER_PERIOD_MS: u64 = 1000;
/// `isFalling`: the damage lands 1.5 s after the fall was seen.
const FALLING_DAMAGE_DELAY_MS: u64 = 1500;

/// What a timer of this module does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaterTimer {
    /// Java `WaterTask.run()`.
    Drown(ObjectId),
    /// The falling damage task of `Player.isFalling`.
    FallingDamage(ObjectId),
}

/// The pending falling damage of one player (Java `_fallingDamage` and `_fallingDamageTask`).
#[derive(Debug, Clone, Copy)]
pub(crate) struct FallingDamage {
    pub damage: i32,
    pub task: TimerId,
}

impl World {
    /// Java `Formulas.calcFallDam(creature, fallHeight)`.
    pub fn calc_fall_dam(&self, id: ObjectId, fall_height: i32) -> f64 {
        if !self.settings.zones.enable_falling_damage || fall_height < 0 {
            return 0.0;
        }
        let Some(view) = self.stats(id) else {
            return 0.0;
        };
        // NOTE(java-quirk): `(fallHeight * getMaxHp()) / 1000` is an int division.
        let base = fall_height.wrapping_mul(view.max_hp()) / 1000;
        view.calc_stat(Stat::Fall, f64::from(base), None, None)
    }

    /// The falling damage part of `Player.isFalling`, after the fall was detected.
    pub fn schedule_falling_damage(&mut self, id: ObjectId, delta_z: i32) {
        let pending = self.zoning.falling.remove(&id);
        let mut damage = pending.map_or(0, |p| p.damage);
        // `if (_fallingDamage == 0)`: a damage that is still pending is kept.
        if damage == 0 {
            damage = self.calc_fall_dam(id, delta_z) as i32;
        }
        if let Some(pending) = pending {
            self.timers.cancel(pending.task);
        }
        let due = self.tick + TimerWheel::ms_to_ticks(FALLING_DAMAGE_DELAY_MS);
        let task = self
            .timers
            .schedule_at(due, TimerKind::Water(WaterTimer::FallingDamage(id)));
        self.zoning.falling.insert(id, FallingDamage { damage, task });
    }

    /// The falling damage a player has pending (0 = none).
    pub fn pending_falling_damage(&self, id: ObjectId) -> i32 {
        self.zoning.falling.get(&id).map_or(0, |p| p.damage)
    }

    /// Java `Player.startWaterTask()`.
    pub fn start_water_task(&mut self, id: ObjectId) {
        if self.is_dead(id) || self.zoning.water_tasks.contains_key(&id) {
            return;
        }
        let Some(view) = self.stats(id) else {
            return;
        };
        let base_breath = view.base_template().map_or(100, |t| t.breath);
        // "Calculate the total stat as a percentage of the base time."
        let breath_percentage = view.calc_stat(Stat::Breath, f64::from(base_breath), None, None) / 100.0;
        let time_in_water = (f64::from(BASE_TIME_IN_WATER_MS) * breath_percentage) as i32;
        self.send(id, &SetupGauge::new(id.0, SetupGauge::CYAN, time_in_water));
        let delay = u64::try_from(time_in_water).unwrap_or(0);
        let due = self.tick + TimerWheel::ms_to_ticks(delay);
        let task = self
            .timers
            .schedule_at(due, TimerKind::Water(WaterTimer::Drown(id)));
        self.zoning.water_tasks.insert(id, task);
    }

    /// Java `Player.stopWaterTask()`.
    pub fn stop_water_task(&mut self, id: ObjectId) {
        let Some(task) = self.zoning.water_tasks.remove(&id) else {
            return;
        };
        self.timers.cancel(task);
        self.send(id, &SetupGauge::new(id.0, SetupGauge::CYAN, 0));
    }

    /// Java `Player.isInWater()`: the water task runs.
    pub fn is_water_task_running(&self, id: ObjectId) -> bool {
        self.zoning.water_tasks.contains_key(&id)
    }

    /// Java `Player.checkWaterState()`.
    pub(crate) fn check_water_state(&mut self, id: ObjectId) {
        if self.is_inside_zone(id, ZoneId::Water) {
            self.start_water_task(id);
        } else {
            self.stop_water_task(id);
        }
    }

    /// `WaterZone.onEnter(player)`.
    pub(crate) fn water_zone_enter(&mut self, id: ObjectId) {
        self.set_inside(id, ZoneId::Water, true);
        self.transformed_player_entered_water(id);
    }

    /// `WaterZone.onExit(player)`.
    pub(crate) fn water_zone_exit(&mut self, id: ObjectId) {
        self.set_inside(id, ZoneId::Water, false);
        // "Mobius: Attempt to stop water task."
        if !self.is_inside_zone(id, ZoneId::Water) {
            self.stop_water_task(id);
        }
        if !self.is_teleporting(id) {
            self.broadcast_user_info(id);
        }
    }

    /// `Player.doDie`: `stopWaterTask()`.
    pub(crate) fn water_on_die(&mut self, id: ObjectId) {
        self.stop_water_task(id);
    }

    /// `Player.deleteMe`: the tasks stop without a packet to the leaving client.
    pub(crate) fn water_on_leave(&mut self, id: ObjectId) {
        if let Some(task) = self.zoning.water_tasks.remove(&id) {
            self.timers.cancel(task);
        }
        if let Some(pending) = self.zoning.falling.remove(&id) {
            self.timers.cancel(pending.task);
        }
    }

    pub(crate) fn on_water_timer(&mut self, timer: WaterTimer) {
        match timer {
            WaterTimer::Drown(id) => {
                if !self.zoning.water_tasks.contains_key(&id) || !self.players.contains_key(&id) {
                    return;
                }
                // The fixed rate: the next run first.
                let due = self.tick + TimerWheel::ms_to_ticks(WATER_PERIOD_MS);
                let next = self
                    .timers
                    .schedule_at(due, TimerKind::Water(WaterTimer::Drown(id)));
                self.zoning.water_tasks.insert(id, next);
                let Some(view) = self.stats(id) else {
                    return;
                };
                let one_percent = f64::from(view.max_hp()) / 100.0;
                let reduce_hp = if one_percent < 1.0 { 1.0 } else { one_percent };
                self.reduce_hp_special(id, reduce_hp, Some(id), false, false, false);
                // "reduced hp, becouse not rest"
                let message =
                    SystemMessage::new(SystemMessageId::YouHaveTakenS1DamageBecauseYouWereUnableToBreathe)
                        .with(SystemMessageParam::IntNumber(reduce_hp as i32));
                self.send(id, &message);
            }
            WaterTimer::FallingDamage(id) => {
                let Some(pending) = self.zoning.falling.remove(&id) else {
                    return;
                };
                let Some(cur_hp) = self.players.get(&id).map(|p| p.core.cur_hp) else {
                    return;
                };
                if pending.damage > 0 && !self.is_invul(id) {
                    let amount = f64::from(pending.damage).min(cur_hp - 1.0);
                    self.reduce_hp_special(id, amount, None, false, true, false);
                    let message = SystemMessage::new(SystemMessageId::YouReceivedS1FallingDamage)
                        .with(SystemMessageParam::IntNumber(pending.damage));
                    self.send(id, &message);
                }
            }
        }
    }
}
