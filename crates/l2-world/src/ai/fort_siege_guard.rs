//! `FortSiegeGuardAI`: the fortress defenders' AI. It is `SiegeGuardAI` (`siege_guard.rs`) with a
//! few lines changed; those lines are here, the rest is shared there (`SiegeGuardAiKind::Fort`).
// Java: ai/FortSiegeGuardAI.java
// Java: entity/actor/instance/{Defender,FortCommander}.java (returnHome)
//
// What differs from `SiegeGuardAI`:
// - `autoAttackCondition`: a player (or a summon's owner) of the clan that owns the fortress is
//   never a target ([`World::fort_siege_guard_auto_attack_condition`]).
// - `thinkActive` goes home only `if (getWalkSpeed() >= 0)`.
// - `attackPrepare` asks the fortress's siege whether a player defends (`getFort().getSiege()`),
//   does not give up a target it cannot see, has no healer exceptions, and a guard that cannot
//   walk does not chase (`getWalkSpeed() <= 0`). Those branches are in `siege_guard.rs`, marked
//   by `fort`.

use l2_util::ObjectId;

use crate::world::World;

impl World {
    /// Java `FortSiegeGuardAI.autoAttackCondition(target)`.
    pub(crate) fn fort_siege_guard_auto_attack_condition(&self, id: ObjectId, target: ObjectId) -> bool {
        if !self.objects.contains_key(&target) {
            return false;
        }
        // Check if the target isn't another guard, folk or a door.
        // NOTE(java-quirk): `isPlayable()` is in the same condition, so any player or summon goes
        // into the block too, and only the clan check below sends it away; a dead player whose
        // clan does not own the fortress passes this check (`isAlikeDead` is in the same `||`).
        let is_door = self.doors.contains_key(&target);
        if self.is_siege_guard(target)
            || self.is_npc(target)
            || is_door
            || self.is_alike_dead(target)
            || self.is_playable(target)
        {
            // `target.asPlayer()` of a player, the owner of a summon, else none.
            let player = if self.is_player(target) {
                Some(target)
            } else {
                self.summon_owner(target)
            };
            let Some(player) = player else {
                return false;
            };
            // `player.getClan().getFortId() == _actor.getFort().getResidenceId()`.
            let guard_fort = self.npc_fort(id).map_or(0, |f| f.0);
            if self
                .clan_of(player)
                .is_some_and(|clan| clan.fort_id == guard_fort)
            {
                return false;
            }
        }
        self.siege_guard_target_checks(id, target)
    }

    /// The end of `FortSiegeGuardAI.thinkActive()`: "Order to the SiegeGuard to return to its
    /// home location because there's no target to attack", `Defender.returnHome()` or
    /// `FortCommander.returnHome()`.
    pub(crate) fn fort_siege_guard_return_home(&mut self, id: ObjectId) {
        if self.stats(id).is_some_and(|v| v.walk_speed() >= 0.0) {
            self.return_home(id);
        }
    }
}
