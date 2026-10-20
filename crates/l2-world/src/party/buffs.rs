//! What the party buffs do to the party: `RebalanceHP` and the damage `TransferDamage` passes on.
// Java: dist/game/data/scripts/handlers/skill/effects/RebalanceHP.java
// Java: dist/game/data/scripts/handlers/skill/effects/TransferDamage.java
// Java: entity/actor/status/PlayerStatus.java (reduceHp: the damage transferred to a party member)

use l2_data::SkillTemplate;
use l2_data::Stat;
use l2_util::ObjectId;

use crate::world::World;

/// The range of the damage transfer (`LocationUtil.checkIfInRange(1000, ...)`).
const TRANSFER_RANGE: i32 = 1000;

impl World {
    /// Java `RebalanceHP.onStart`: the members of the party in range of the caster end up with the
    /// same share of their maximum HP, the sum of what they have over the sum of what they could
    /// have. A healed member stops at its maximum recoverable HP.
    pub(crate) fn rebalance_party_hp(&mut self, effector: ObjectId, skill: &SkillTemplate) {
        if !self.is_player(effector) || !self.is_in_party(effector) {
            return;
        }
        let range = skill.affect_range;
        let in_range = |world: &World, member: ObjectId| {
            !world.is_dead(member) && world.check_if_in_range(range, effector, member, true)
        };
        // `skill.getTargetList(effector, false)`, once to add the HP up and once to share it.
        let mut full_hp = 0.0;
        let mut current_hps = 0.0;
        for member in self.rebalance_targets(effector, skill) {
            if self.is_player(member) && in_range(self, member) {
                if let (Some(view), Some(core)) = (self.stats(member), self.core_of(member)) {
                    full_hp += f64::from(view.max_hp());
                    current_hps += core.cur_hp;
                }
            }
            // M6: the member's summon counts, too (SUMMON-01).
        }
        let percent_hp = current_hps / full_hp;
        for member in self.rebalance_targets(effector, skill) {
            if self.is_player(member) && in_range(self, member) {
                let (Some(view), Some(core)) = (self.stats(member), self.core_of(member)) else {
                    continue;
                };
                let current = core.cur_hp;
                let mut new_hp = f64::from(view.max_hp()) * percent_hp;
                let max_recoverable = f64::from(view.max_recoverable_hp());
                // The target gets healed.
                if new_hp > current {
                    // The heal will be blocked if the current HP passes the limit.
                    if current > max_recoverable {
                        new_hp = current;
                    } else if new_hp > max_recoverable {
                        new_hp = max_recoverable;
                    }
                }
                self.set_current_hp(member, new_hp, true);
            }
            // M6: the member's summon is shared the same way (SUMMON-01).
        }
    }

    fn rebalance_targets(&mut self, effector: ObjectId, skill: &SkillTemplate) -> Vec<ObjectId> {
        let ctx = self.cast_target_context(effector);
        let result = self.skill_target_list_current(skill, effector, false, &ctx);
        self.apply_target_result(effector, skill, &result);
        result.targets
    }

    /// The part of Java `PlayerStatus.reduceHp` that hands damage to the caster of
    /// `TransferDamage`: the caster, a living member of the same party within 1000, takes
    /// `transDamToPlayer` percent of the damage (the CP first, then HP shared by the other members
    /// in range of it). Returns the damage left for `player`, or `None` when nothing was passed
    /// on.
    pub(crate) fn party_transfer_damage(
        &mut self,
        player: ObjectId,
        attacker: ObjectId,
        amount: f64,
    ) -> Option<f64> {
        let caster = self.transfer_damage_to(player)?;
        let party = self.party_of(player)?;
        if caster == player
            || !self.check_if_in_range(TRANSFER_RANGE, player, caster, true)
            || self.is_dead(caster)
            || !self.party_members(party).contains(&caster)
        {
            return None;
        }
        let percent = self.stats(player).map_or(0.0, |v| {
            v.calc_stat(Stat::TransferDamageToPlayer, 0.0, None, None)
        }) as i32;
        let caster_hp = self.core_of(caster).map_or(0.0, |c| c.cur_hp) as i32;
        // NOTE(java-quirk): `(int) amount * (int) percent` truncates the damage before it is
        // multiplied.
        let mut transfer_damage = (caster_hp - 1).min((amount as i32).wrapping_mul(percent) / 100);
        if transfer_damage <= 0 {
            return None;
        }
        let members_in_range = self
            .party_members(party)
            .into_iter()
            .filter(|m| *m != caster && self.check_if_in_range(TRANSFER_RANGE, *m, caster, false))
            .count();

        // `attacker.isPlayable() || attacker.isFakePlayer()`.
        let caster_cp = self.core_of(caster).map_or(0.0, |c| c.cur_cp);
        if self.is_player(attacker) && caster_cp > 0.0 {
            if caster_cp > f64::from(transfer_damage) {
                // `reduceCp(transferDmg)`.
                self.set_current_cp(caster, caster_cp - f64::from(transfer_damage), true);
            } else {
                transfer_damage = (f64::from(transfer_damage) - caster_cp) as i32;
                self.set_current_cp(caster, 0.0, true);
            }
        }

        if members_in_range > 0 {
            let share = transfer_damage / i32::try_from(members_in_range).unwrap_or(i32::MAX);
            self.reduce_current_hp(caster, f64::from(share), Some(attacker));
            return Some(amount - f64::from(transfer_damage));
        }
        None
    }
}
