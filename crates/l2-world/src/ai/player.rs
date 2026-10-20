//! `PlayerAI.thinkAttack` and `thinkCast`: walk up to the target, then attack it or cast at it.
// Java: ai/PlayerAI.java (thinkAttack, thinkCast), ai/CreatureAI.java (maybeMoveToPawn: the attack
//       parts, maybeMoveToPosition, checkTargetLost)

use l2_data::SkillTemplate;
use l2_data::skill::TargetType;
use l2_util::ObjectId;

use crate::effects::apply::has_negative_effect;
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

    /// Java `PlayerAI.thinkCast()`: walk into the skill's range (the casting flag goes down
    /// while walking, the arrival thinks again), then `doCast` at the cast target.
    pub(crate) fn think_cast(&mut self, id: ObjectId) {
        let Some(key) = self.cast_skill_of(id) else {
            return;
        };
        let Some(table) = self.skill_table() else {
            return;
        };
        let Some(skill) = table.get(key.id, key.level) else {
            return;
        };
        let target = self.cast_target_of(id);
        let range = self.magical_attack_range_for(id, skill);
        if skill.target_type == TargetType::Ground && self.is_player(id) {
            let position = self.cast_state(id).and_then(|s| s.current_skill_world_position);
            if self.maybe_move_to_position(id, position, range) {
                self.set_casting_now(id, false);
                return;
            }
        } else {
            if self.check_target_lost(id, target, skill) {
                if has_negative_effect(skill) && self.attack_target_of(id).is_some() {
                    // Notify the target.
                    self.set_cast_target(id, None);
                }
                self.set_casting_now(id, false);
                return;
            }
            // NOTE(java-quirk): `useMagic` raised the casting flag, so the first `moveToPawn` of
            // the follow task started here refuses (`ActionFailed`); the walk begins with the
            // task's next run.
            if let Some(t) = target
                && self.maybe_move_to_pawn(id, t, range)
            {
                self.set_casting_now(id, false);
                return;
            }
        }

        if skill.hit_time > 50 && !skill.simultaneous_cast {
            self.client_stop_moving(id, None);
        }

        // Check if target has changed: the cast goes to the cast target, then the old target is
        // selected again.
        let current = self.target_of(id);
        if current != target
            && let (Some(current), Some(target)) = (current, target)
        {
            self.set_target(id, Some(target));
            self.do_cast(id, key);
            self.set_target(id, Some(current));
            return;
        }
        self.do_cast(id, key);
    }

    /// Java `CreatureAI.checkTargetLost(target)` with the AI's `_skill`: `true` (and the
    /// intention goes active) when there is no target, or an offensive area skill can no longer
    /// reach it.
    pub(crate) fn check_target_lost(
        &mut self,
        id: ObjectId,
        target: Option<ObjectId>,
        skill: &SkillTemplate,
    ) -> bool {
        let Some(target) = target else {
            self.set_intention_active(id);
            return true;
        };
        if has_negative_effect(skill) && skill.affect_range > 0 {
            if self.is_player(id) && self.is_moving(id) {
                let (Some(from), Some(to)) = (self.position_of(id), self.position_of(target)) else {
                    self.set_intention_active(id);
                    return true;
                };
                let instance = self.objects.get(&id).map_or(0, |e| e.instance_id);
                let can_move = self.geo_can_move_to_target(
                    l2_geo::Location::new(from.0, from.1, from.2),
                    l2_geo::Location::new(to.0, to.1, to.2),
                    instance,
                );
                if !can_move {
                    self.set_intention_active(id);
                    return true;
                }
            } else if !self.can_see_target(id, target) {
                self.set_intention_active(id);
                return true;
            }
        }
        // Summons: M6.
        false
    }

    /// Java `CreatureAI.maybeMoveToPosition(worldPosition, offset)`: walk to `offset - 5` of the
    /// point when the player is farther than `offset` plus its collision radius.
    pub(crate) fn maybe_move_to_position(
        &mut self,
        id: ObjectId,
        position: Option<(i32, i32, i32)>,
        offset: i32,
    ) -> bool {
        let Some(position) = position else {
            return false;
        };
        if offset < 0 {
            // Skill radius -1.
            return false;
        }
        let radius = offset.wrapping_add(self.collision_radius_of(id));
        if !self.inside_radius_2d(id, position, radius) {
            if self.is_movement_disabled(id) {
                return true;
            }
            // `setRunning()` is not for PlayerAI.
            self.stop_follow(id);
            let Some((mut x, mut y, _)) = self.position_of(id) else {
                return true;
            };
            let dx = f64::from(position.0) - f64::from(x);
            let dy = f64::from(position.1) - f64::from(y);
            let mut dist = (dx * dx + dy * dy).sqrt();
            let sin = dy / dist;
            let cos = dx / dist;
            dist -= f64::from(offset - 5);
            x = x.wrapping_add((dist * cos) as i32);
            y = y.wrapping_add((dist * sin) as i32);
            self.ai_move_to(id, x, y, position.2);
            return true;
        }
        if self.is_following(id) {
            self.stop_follow(id);
        }
        false
    }
}
