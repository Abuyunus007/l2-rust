//! The force charges and the souls of a player (`FocusEnergy`, `FocusMaxEnergy`, `FocusSouls`,
//! `SoulEating`), with the ten minute reset task and the `EtcStatusUpdate` they show.
// Java: entity/actor/Player.java (getCharges, increaseCharges, decreaseCharges, clearCharges,
//       restartChargeTask, stopChargeTask, getChargedSouls, increaseSouls, decreaseSouls,
//       clearSouls, restartSoulTask, stopSoulTask)
// Java: entity/actor/tasks/player/{ResetChargesTask,ResetSoulsTask}.java

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;

use super::control::ControlTimer;
use crate::timer::{TimerKind, TimerWheel};
use crate::world::World;

/// `restartChargeTask` / `restartSoulTask`: ten minutes.
const RESET_MS: u64 = 600_000;

impl World {
    /// Java `Player.getCharges()`.
    pub fn charges_of(&self, id: ObjectId) -> i32 {
        self.control(id).map_or(0, |c| c.charges)
    }

    /// Java `Player.getChargedSouls()`.
    pub fn charged_souls_of(&self, id: ObjectId) -> i32 {
        self.control(id).map_or(0, |c| c.souls)
    }

    /// Java `Player.increaseCharges(count, max)`.
    pub fn increase_charges(&mut self, id: ObjectId, count: i32, max: i32) {
        if !self.players.contains_key(&id) {
            return;
        }
        if self.charges_of(id) >= max {
            self.send(id, &SystemMessage::new(SystemMessageId::YourForceHasReachedMaximumCapacity));
            return;
        }
        // Charge clear task should be reset every time a charge is increased.
        self.restart_charge_task(id);
        let total = self.charges_of(id).wrapping_add(count);
        if let Some(c) = self.control_mut(id) {
            c.charges = total;
        }
        if total >= max {
            if let Some(c) = self.control_mut(id) {
                c.charges = max;
            }
            self.send(id, &SystemMessage::new(SystemMessageId::YourForceHasReachedMaximumCapacity));
        } else {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YourForceHasIncreasedToLevelS1)
                    .with(SystemMessageParam::IntNumber(total)),
            );
        }
        self.send_etc_status_update(id);
    }

    /// Java `Player.decreaseCharges(count)`: `false` when there are not enough.
    pub fn decrease_charges(&mut self, id: ObjectId, count: i32) -> bool {
        if self.charges_of(id) < count {
            return false;
        }
        // Charge clear task should be reset every time a charge is decreased and stopped when
        // charges become 0.
        let left = self.charges_of(id) - count;
        if let Some(c) = self.control_mut(id) {
            c.charges = left;
        }
        if left == 0 {
            self.stop_charge_task(id);
        } else {
            self.restart_charge_task(id);
        }
        self.send_etc_status_update(id);
        true
    }

    /// Java `Player.clearCharges()`.
    pub fn clear_charges(&mut self, id: ObjectId) {
        if let Some(c) = self.control_mut(id) {
            c.charges = 0;
        }
        self.send_etc_status_update(id);
    }

    /// Java `Player.restartChargeTask()`.
    fn restart_charge_task(&mut self, id: ObjectId) {
        self.stop_charge_task(id);
        let due = self.tick + TimerWheel::ms_to_ticks(RESET_MS);
        let timer = self
            .timers
            .schedule_at(due, TimerKind::Control(ControlTimer::ResetCharges(id)));
        if let Some(c) = self.control_mut(id) {
            c.charge_timer = Some(timer);
        }
    }

    /// Java `Player.stopChargeTask()`.
    pub fn stop_charge_task(&mut self, id: ObjectId) {
        let timer = self.effects.control.get_mut(&id).and_then(|c| c.charge_timer.take());
        if let Some(timer) = timer {
            self.timers.cancel(timer);
        }
    }

    /// Java `Player.increaseSouls(count)`.
    pub fn increase_souls(&mut self, id: ObjectId, count: i32) {
        if !self.players.contains_key(&id) {
            return;
        }
        let souls = self.charged_souls_of(id).wrapping_add(count);
        if let Some(c) = self.control_mut(id) {
            c.souls = souls;
        }
        self.send(
            id,
            &SystemMessage::new(SystemMessageId::YourSoulCountHasIncreasedByS1ItIsNowAtS2)
                .with(SystemMessageParam::IntNumber(count))
                .with(SystemMessageParam::IntNumber(souls)),
        );
        self.restart_soul_task(id);
        self.send_etc_status_update(id);
    }

    /// Java `Player.decreaseSouls(count)`.
    pub fn decrease_souls(&mut self, id: ObjectId, count: i32) -> bool {
        let souls = (self.charged_souls_of(id).wrapping_sub(count)).max(0);
        if let Some(c) = self.control_mut(id) {
            c.souls = souls;
        }
        if souls == 0 {
            self.stop_soul_task(id);
        } else {
            self.restart_soul_task(id);
        }
        self.send_etc_status_update(id);
        true
    }

    /// Java `Player.clearSouls()`.
    pub fn clear_souls(&mut self, id: ObjectId) {
        if let Some(c) = self.control_mut(id) {
            c.souls = 0;
        }
        self.stop_soul_task(id);
        self.send_etc_status_update(id);
    }

    /// Java `Player.restartSoulTask()`.
    fn restart_soul_task(&mut self, id: ObjectId) {
        self.stop_soul_task(id);
        let due = self.tick + TimerWheel::ms_to_ticks(RESET_MS);
        let timer = self
            .timers
            .schedule_at(due, TimerKind::Control(ControlTimer::ResetSouls(id)));
        if let Some(c) = self.control_mut(id) {
            c.soul_timer = Some(timer);
        }
    }

    /// Java `Player.stopSoulTask()`.
    pub fn stop_soul_task(&mut self, id: ObjectId) {
        let timer = self.effects.control.get_mut(&id).and_then(|c| c.soul_timer.take());
        if let Some(timer) = timer {
            self.timers.cancel(timer);
        }
    }
}
