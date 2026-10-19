//! The combat stance: `AutoAttackStop` 15 s after the last attack or hit.
// Java: taskmanagers/AttackStanceTaskManager.java
//
// Java runs one task every second over every creature in a stance and ends the ones refreshed
// more than `COMBAT_TIME` ago. Here one timer-wheel event does the same run, and only while some
// creature is in a stance (nothing to do otherwise). Java's run is not tied to the refresh either,
// so a stance ends between 15 and 16 seconds after its last refresh in both.

use l2_protocol_game::server::AutoAttackStop;
use l2_util::ObjectId;

use crate::timer::{TimerKind, TimerWheel};
use crate::world::World;

/// `AttackStanceTaskManager.COMBAT_TIME`, ms.
pub const COMBAT_TIME_MS: i64 = 15_000;
/// The manager runs every second.
const CHECK_INTERVAL_MS: u64 = 1000;

impl World {
    /// Java `AttackStanceTaskManager.addAttackStanceTask(creature)`: (re)starts the stance.
    pub fn add_attack_stance_task(&mut self, id: ObjectId) {
        // Cubics act here (M5).
        let now = self.world_ms();
        self.combat.stances.insert(id, now);
        if self.combat.stance_timer.is_none() {
            let due = self.tick + TimerWheel::ms_to_ticks(CHECK_INTERVAL_MS);
            self.combat.stance_timer = Some(self.timers.schedule_at(due, TimerKind::AttackStanceCheck));
        }
    }

    /// Java `AttackStanceTaskManager.removeAttackStanceTask(creature)`.
    pub fn remove_attack_stance_task(&mut self, id: ObjectId) {
        self.combat.stances.remove(&id);
    }

    /// Java `AttackStanceTaskManager.hasAttackStanceTask(creature)` (Java `isInCombat()`).
    pub fn has_attack_stance_task(&self, id: ObjectId) -> bool {
        self.combat.stances.contains_key(&id)
    }

    /// One run of the manager.
    pub(crate) fn on_attack_stance_check(&mut self) {
        self.combat.stance_timer = None;
        let now = self.world_ms();
        // Java iterates a hash map; the order of the stops is made stable by id here.
        let mut expired: Vec<ObjectId> = self
            .combat
            .stances
            .iter()
            .filter(|(_, start)| now - **start > COMBAT_TIME_MS)
            .map(|(id, _)| *id)
            .collect();
        expired.sort_unstable();
        for id in expired {
            self.combat.stances.remove(&id);
            if self.objects.contains_key(&id) {
                self.broadcast_creature_packet(id, &AutoAttackStop { target_id: id.0 });
                self.set_auto_attacking(id, false);
                // The summon's `AutoAttackStop`: summons are M5.
            }
        }
        if !self.combat.stances.is_empty() {
            let due = self.tick + TimerWheel::ms_to_ticks(CHECK_INTERVAL_MS);
            self.combat.stance_timer = Some(self.timers.schedule_at(due, TimerKind::AttackStanceCheck));
        }
    }
}
