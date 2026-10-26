//! What the damage effects share: `reduceCurrentHp` with a skill and as damage over time, the
//! attack break after a hit.
// Java: entity/actor/Creature.java (reduceCurrentHp (3 overloads), reduceCurrentHpByDOT)
// Java: entity/actor/Player.java, entity/actor/Attackable.java (reduceCurrentHp)
// Java: entity/actor/status/{CreatureStatus,PlayerStatus,NpcStatus}.java (reduceHp)
//
// `combat/damage.rs` and `status.rs` hold the reduceHp of an auto attack (not a damage over time,
// no skill). A skill hit goes through them as well; what only the effects need is here, so those
// files stay as they are: a damage over time (`isDOT`), the HP consumption of a toggle and the
// line-of-sight rule of `Creature.reduceCurrentHp`.
//
// Left out, as in `status.rs`: olympiad, offline traders, the summon / party damage
// transfer, the mana shield, GM `giveDamage` rights, champions, sitting / store mode standing up
// (none of them exist in the world yet), and `notifyDamageReceived`'s fake death and script events.

use l2_data::SkillTemplate;
use l2_util::ObjectId;

use crate::combat::hit::calc_atk_break_for;
use crate::world::World;

impl World {
    /// Java `Creature.reduceCurrentHp(amount, attacker, skill)`: a skill hit.
    pub(crate) fn reduce_current_hp_by_skill(
        &mut self,
        target: ObjectId,
        amount: f64,
        attacker: Option<ObjectId>,
        skill: &SkillTemplate,
    ) {
        let mut amount = amount;
        // `TerritoryWard.reduceCurrentHp`: no skill damage (`skill != null`, M7-TW-01).
        if self.territory_ward_refuses_skill_damage(target) {
            return;
        }
        // A ranged skill cannot hurt a player behind an obstacle.
        if self.is_player(target)
            && skill.cast_range > 0
            && let Some(attacker) = attacker
            && !self.can_see_target(attacker, target)
        {
            amount = 0.0;
        }
        // `Player.reduceCurrentHp` hands a toggle skill's damage over as HP consumption.
        if self.is_player(target) && skill.is_toggle() {
            self.reduce_hp_special(target, amount, attacker, true, false, true);
            return;
        }
        self.reduce_current_hp(target, amount, attacker);
    }

    /// Java `Creature.reduceCurrentHpByDOT(amount, attacker, skill)`:
    /// `reduceCurrentHp(amount, attacker, !skill.isToggle(), true, skill)`.
    pub(crate) fn reduce_current_hp_by_dot(
        &mut self,
        target: ObjectId,
        amount: f64,
        attacker: Option<ObjectId>,
        skill: &SkillTemplate,
    ) {
        // `Door.reduceCurrentHpByDOT`: "doors can't be damaged by DOTs" (M7-SIEGE-02).
        if self.doors.contains_key(&target) {
            return;
        }
        // `TerritoryWard.reduceCurrentHpByDOT`: "wards can't be damaged by DOTs" (M7-TW-01).
        if self.territory_ward_refuses_skill_damage(target) {
            return;
        }
        let consumption = self.is_player(target) && skill.is_toggle();
        self.reduce_hp_special(target, amount, attacker, !skill.is_toggle(), true, consumption);
    }

    /// `Attackable.reduceCurrentHp(.., awake, isDOT, ..)` / `Player.reduceCurrentHp` down to
    /// `CreatureStatus.reduceHp` / `PlayerStatus.reduceHp`, for the cases `status.rs` does not
    /// cover: `is_dot` and `is_hp_consumption`.
    fn reduce_hp_special(
        &mut self,
        id: ObjectId,
        amount: f64,
        attacker: Option<ObjectId>,
        awake: bool,
        is_dot: bool,
        is_hp_consumption: bool,
    ) {
        if self.is_dead(id) {
            return;
        }
        if self.is_player(id) {
            self.player_reduce_hp_special(id, amount, attacker, awake, is_dot, is_hp_consumption);
            return;
        }
        if !self.is_npc(id) {
            return;
        }
        // Attackable.reduceCurrentHp: aggro, then the minions.
        if self.is_attackable_npc(id) {
            if let Some(attacker) = attacker {
                self.attackable_add_damage(id, attacker, amount as i32);
            }
            if let Some(attacker) = attacker
                && self.is_monster(id)
            {
                self.minions_assist(id, attacker);
            }
        }
        // NpcStatus.reduceHp: a duelist who hits an NPC leaves the duel; the attacker joins the
        // attack-by list.
        if self.is_dead(id) {
            return;
        }
        if let Some(attacker) = attacker {
            self.duel_interrupt_on_hit(id, attacker);
            self.add_attacker_to_attack_by_list(id, attacker);
        }
        // CreatureStatus.reduceHp: invul handling.
        if self.is_invul(id) && !(is_dot || is_hp_consumption) {
            return;
        }
        if !is_dot && !is_hp_consumption {
            self.stop_effects_on_damage(id, awake);
        }
        if amount > 0.0 {
            let current = self.core_of(id).map_or(0.0, |c| c.cur_hp);
            self.set_current_hp(id, (current - amount).max(0.0), true);
        }
        // `isMortal()`: nothing makes a creature immortal before scripts.
        if self.core_of(id).is_some_and(|c| c.cur_hp < 0.5) {
            self.abort_attack(id);
            self.abort_cast(id);
            self.do_die(id, attacker);
        }
    }

    /// `PlayerStatus.reduceHp(value, attacker, awake, isDOT, isHPConsumption, false)`.
    fn player_reduce_hp_special(
        &mut self,
        id: ObjectId,
        value: f64,
        attacker: Option<ObjectId>,
        awake: bool,
        is_dot: bool,
        is_hp_consumption: bool,
    ) {
        if self.is_invul(id) && !(is_dot || is_hp_consumption) {
            return;
        }
        if !is_hp_consumption {
            self.stop_effects_on_damage(id, awake);
            // Sitting and store mode stand up, a stun breaks on a hit that is no DOT: not
            // modelled yet (`status.rs`).
        }
        let mut amount = value;
        if let Some(attacker) = attacker.filter(|a| *a != id) {
            // A duelist who is beaten or has won takes no damage; a hit by somebody outside the
            // duel interrupts it.
            if self.duel_ignores_damage(id, attacker) {
                return;
            }
            if self.is_player(attacker) {
                // `attacker.isPlayable()`: CP takes the hit first.
                let cp = self.players.get(&id).map_or(0.0, |p| p.core.cur_cp);
                if cp >= amount {
                    self.set_current_cp(id, cp - amount, true);
                    amount = 0.0;
                } else {
                    amount -= cp;
                    self.set_current_cp(id, 0.0, false);
                }
            }
            // The "has received damage" message is for hits that are no DOT; a DOT's amount is
            // not announced.
            if (value as i32) > 0 && !is_dot {
                let message = l2_protocol_game::server::SystemMessage::new(
                    l2_protocol_game::SystemMessageId::C1HasReceivedS3DamageFromC2,
                )
                .with(l2_protocol_game::server::SystemMessageParam::Text(
                    self.creature_name(id),
                ))
                .with(l2_protocol_game::server::SystemMessageParam::Text(
                    self.creature_name(attacker),
                ))
                .with(l2_protocol_game::server::SystemMessageParam::IntNumber(
                    value as i32,
                ));
                self.send(id, &message);
            }
        }
        if amount > 0.0 {
            let current = self.core_of(id).map_or(0.0, |c| c.cur_hp);
            let mut new_hp = (current - amount).max(0.0);
            // Duels end at 1 HP: the duelist is beaten, not killed.
            if current - amount <= 0.0 && self.duel_defeats_instead_of_dying(id, attacker) {
                new_hp = 1.0;
            }
            self.set_current_hp(id, new_hp, true);
        }
        if self.core_of(id).is_some_and(|c| c.cur_hp < 0.5) && !is_hp_consumption {
            self.abort_attack(id);
            self.abort_cast(id);
            // Olympiad matches end without death (M6).
            self.do_die(id, attacker);
        }
    }

    /// The "manage attack or cast break of the target" lines after a hit:
    /// `if (!effected.isRaid() && Formulas.calcAtkBreak(effected, damage)) { breakAttack();
    /// breakCast(); }`.
    pub(crate) fn skill_atk_break(&mut self, target: ObjectId, damage: f64) {
        if !self.is_raid(target) && self.with_rng(|world, rng| calc_atk_break_for(world, target, damage, rng))
        {
            self.break_attack(target);
            self.break_cast(target);
        }
    }
}
