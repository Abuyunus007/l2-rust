//! `PlayerAI.thinkAttack`: walk up to the attack target, then attack it.
// Java: ai/PlayerAI.java (thinkAttack), ai/CreatureAI.java (maybeMoveToPawn: the attack parts)

use l2_util::ObjectId;

use crate::world::World;

impl World {
    /// Java `PlayerAI.thinkAttack()`.
    pub(crate) fn think_attack(&mut self, id: ObjectId) {
        let Some(target) = self.attack_target_of(id) else {
            return;
        };
        if self.check_target_lost_or_dead(id, Some(target)) {
            // Notify the target.
            self.set_attack_target(id, None);
            return;
        }
        let range = self.stats(id).map_or(0, |s| s.physical_attack_range());
        if self.maybe_move_to_pawn(id, target, range) {
            return;
        }
        self.client_stop_moving(id, None);
        self.do_attack(id, target);
    }
}
