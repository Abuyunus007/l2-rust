//! A hit lands: `Creature.onHitTimer`, the damage messages, `reduceCurrentHp` (player, NPC and
//! attackable variants), the aggro list's damage and hate, and the death hook.
// Java: entity/actor/Creature.java (onHitTimer, sendDamageMessage, reduceCurrentHp)
// Java: entity/actor/Player.java (sendDamageMessage, reduceCurrentHp)
// Java: entity/actor/Attackable.java (reduceCurrentHp, addDamage, addDamageHate, getHateRatio)
// Java: entity/actor/holders/npc/AggroInfo.java
// Java: entity/actor/status/{CreatureStatus,NpcStatus,AttackableStatus,PlayerStatus}.java
//       (reduceHp)
//
// # Left out (each has its card)
// - Effects (M5): the invulnerability *effect* (`EffectFlag.INVUL`) and reflect/absorb come from
//   effects and passives, so they are read from the stat engine (passive funcs do apply). The
//   invulnerability flag itself (`//invul`, `Creature.setInvul`) is honoured by `status.rs`; stun
//   breaking, `stopEffectsOnDamage`, fake death, trigger skills, the weapon's on-critical skill,
//   the raid curse (skill 4515).
// - Summons (damage transfer), mana shield messages, duels, olympiad, offline traders, the
//   tutorial quest: not ported yet.
// - Minions calling for help (`MinionList.onAssist`), `addAttackerToAttackByList`, the aggro-range
//   script events: M4-AI-01.

use l2_data::Stat;
use l2_data::item::WeaponType;
use l2_data::npc::{NpcParam, NpcRace};
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ActionFailed, SystemMessage, SystemMessageParam};
use l2_util::ObjectId;

use super::HitTask;
use super::hit::calc_atk_break_for;
use crate::objects::ObjectKind;
use crate::world::World;

/// Java `AggroInfo.LIMIT`.
const AGGRO_LIMIT: i64 = 1_000_000_000_000_000;

/// One entry of an attackable's aggro list (Java `AggroInfo`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AggroInfo {
    pub attacker: ObjectId,
    /// `_damageTotal`.
    pub damage: i64,
    /// `_hateTotal`.
    pub hate: i64,
}

impl AggroInfo {
    fn new(attacker: ObjectId) -> Self {
        Self {
            attacker,
            damage: 0,
            hate: 0,
        }
    }

    /// Java `AggroInfo.addDamage`: capped at `LIMIT`.
    pub fn add_damage(&mut self, value: i64) {
        self.damage = add_capped(self.damage, value);
    }

    /// Java `AggroInfo.addHate`: capped at `LIMIT`.
    pub fn add_hate(&mut self, value: i64) {
        self.hate = add_capped(self.hate, value);
    }
}

fn add_capped(current: i64, value: i64) -> i64 {
    if value == 0 {
        return current;
    }
    if value > 0 {
        return if current >= AGGRO_LIMIT - value {
            AGGRO_LIMIT
        } else {
            current + value
        };
    }
    current.wrapping_add(value)
}

impl World {
    /// The `HitTask` timer.
    pub(crate) fn on_hit_task(&mut self, task: HitTask) {
        if !self.objects.contains_key(&task.attacker) {
            // Java holds the attacker in the task; one that left the world does nothing visible.
            return;
        }
        self.on_hit_timer(task);
    }

    /// Java `Creature.onHitTimer(target, damage, crit, miss, shld, soulshot, rechargeShots)`.
    pub fn on_hit_timer(&mut self, task: HitTask) {
        let HitTask {
            attacker: id,
            target,
            damage,
            crit,
            miss,
            ..
        } = task;
        // If the attacker/target is dead or use fake death, notify the AI with CANCEL.
        if !self.objects.contains_key(&target) || self.is_alike_dead(id) {
            self.notify_action_cancel(id);
            return;
        }
        // Fake players: not ported.
        let is_door = self.objects.get(&id).map(|e| e.kind) == Some(ObjectKind::Door);
        if (self.is_npc(id) && self.is_alike_dead(target))
            || self.is_dead(target)
            || (!self.is_in_surrounding_region(id, target) && !is_door)
        {
            // Some times attack is processed but target die before the hit.
            // So we need to recharge shot for next attack.
            if task.recharge_shots {
                self.recharge_shots(id, true, false);
            }
            self.notify_action_cancel(id);
            self.send(id, &ActionFailed);
            return;
        }

        if miss {
            // Notify target AI.
            if self.has_ai(target) {
                self.notify_action_evaded(target, id);
            }
            // `notifyAttackAvoid`: the ON_CREATURE_ATTACK_AVOID event (`TriggerSkillByAvoid`).
            self.notify_attack_avoid(id, target, false);
        }

        // Send message about damage/crit or miss.
        self.send_damage_message(id, target, damage, false, crit, miss);

        // M5: the raid curse (skill 4515) for attackers more than 8 levels above a raid boss.

        // If Creature target is a Player, it enters combat.
        if self.is_player(target) {
            self.client_start_auto_attack(target);
        }

        if !miss && damage > 0 {
            let is_bow = self
                .stats(id)
                .and_then(|s| {
                    s.active_weapon_item()
                        .and_then(|t| t.weapon())
                        .map(|w| w.weapon_type)
                })
                .is_some_and(|t| matches!(t, WeaponType::Bow | WeaponType::CrossBow));
            let mut reflected_damage = 0;
            // Do not reflect if weapon is of type bow or target is invulnerable.
            if !is_bow && !self.is_invul(target) {
                // Quick fix for no drop from raid if boss attack high-level char with damage
                // reflection.
                let attacker_level = self.players.get(&id).map(|p| p.core.level);
                let target_level = self.stats(target).map_or(0, |s| s.level());
                if !self.is_raid(target) || attacker_level.is_none_or(|level| level <= target_level + 8) {
                    // Reduce HP of the target and calculate reflection damage to reduce HP of
                    // attacker if necessary.
                    let reflect_percent = self
                        .stats(target)
                        .map_or(0.0, |s| s.calc(Stat::ReflectDamagePercent, 0.0));
                    if reflect_percent > 0.0 {
                        reflected_damage = ((reflect_percent / 100.0) * f64::from(damage)) as i32;
                        let max_hp = self.stats(target).map_or(0, |s| s.max_hp());
                        if reflected_damage > max_hp {
                            reflected_damage = max_hp;
                        }
                    }
                }
            }

            // Reduce targets HP.
            self.reduce_current_hp(target, f64::from(damage), Some(id));
            // `notifyDamageReceived`: a fake dead player stands up, the damage listeners hear it.
            self.notify_damage_received_by(target, f64::from(damage), Some(id), None, crit, false);
            if reflected_damage > 0 {
                self.reduce_current_hp(id, f64::from(reflected_damage), Some(target));
                self.notify_damage_received_by(
                    id,
                    f64::from(reflected_damage),
                    Some(target),
                    None,
                    crit,
                    false,
                );
            }

            // Do not absorb if weapon is of type bow.
            if !is_bow {
                self.absorb(id, damage);
            }

            // Notify AI with ATTACKED.
            if self.has_ai(target) {
                self.notify_action_attacked(target, id);
            }
            self.client_start_auto_attack(id);
            // Summons: the owner enters combat too (no summons yet).

            // Manage attack or cast break of the target (calculating rate, sending message...).
            if !self.is_raid(target)
                && self.with_rng(|world, rng| calc_atk_break_for(world, target, f64::from(damage), rng))
            {
                self.break_attack(target);
                self.break_cast(target);
            }
            // M5: trigger skills (`_triggerSkills`) and the weapon's on-critical skill.
        }
        // Recharge any active auto-soulshot tasks for current creature.
        if task.recharge_shots {
            self.recharge_shots(id, true, false);
        }
    }

    /// The HP and MP absorb of `onHitTimer` (`ABSORB_DAMAGE_PERCENT`, `ABSORB_MANA_DAMAGE_PERCENT`).
    fn absorb(&mut self, id: ObjectId, damage: i32) {
        let Some(view) = self.stats(id) else {
            return;
        };
        let hp_percent = view.calc(Stat::AbsorbDamagePercent, 0.0);
        let mp_percent = view.calc(Stat::AbsorbManaDamagePercent, 0.0);
        let (max_hp, max_mp) = (view.max_recoverable_hp(), view.max_recoverable_mp());
        let (cur_hp, cur_mp) = (view.core().cur_hp, view.core().cur_mp);
        if hp_percent > 0.0 {
            let max_can_absorb = (f64::from(max_hp) - cur_hp) as i32;
            let absorb = (((hp_percent / 100.0) * f64::from(damage)) as i32).min(max_can_absorb);
            if absorb > 0 {
                self.set_current_hp(id, cur_hp + f64::from(absorb), true);
            }
        }
        if mp_percent > 0.0 {
            let max_can_absorb = (f64::from(max_mp) - cur_mp) as i32;
            let absorb = (((mp_percent / 100.0) * f64::from(damage)) as i32).min(max_can_absorb);
            if absorb > 0 {
                self.set_current_mp(id, cur_mp + f64::from(absorb), true);
            }
        }
    }

    /// Java `sendDamageMessage(target, damage, mcrit, pcrit, miss)`: `Player`'s version for a
    /// player attacker, `Creature`'s (only the evade message) for anybody else.
    pub(crate) fn send_damage_message(
        &mut self,
        id: ObjectId,
        target: ObjectId,
        damage: i32,
        mcrit: bool,
        pcrit: bool,
        miss: bool,
    ) {
        let attacker_name = self.creature_name(id);
        if miss && self.is_player(target) {
            let message = SystemMessage::new(SystemMessageId::C1HasEvadedC2SAttack)
                .with(SystemMessageParam::PlayerName(self.creature_name(target)))
                .with(SystemMessageParam::Text(attacker_name.clone()));
            self.send(target, &message);
        }
        if !self.is_player(id) {
            return;
        }
        // Check if hit is missed.
        if miss {
            let message = SystemMessage::new(SystemMessageId::C1SAttackWentAstray)
                .with(SystemMessageParam::PlayerName(attacker_name));
            self.send(id, &message);
            return;
        }
        // Check if hit is critical.
        if pcrit {
            let message = SystemMessage::new(SystemMessageId::C1LandedACriticalHit)
                .with(SystemMessageParam::PlayerName(attacker_name.clone()));
            self.send(id, &message);
        }
        if mcrit {
            self.send(id, &SystemMessage::new(SystemMessageId::MagicCriticalHit));
        }
        // Olympiad damage counting: not ported.
        let target_kind = self.objects.get(&target).map(|e| e.kind);
        // Java: an invulnerable target that is not an NPC makes the attack "blocked" (the target
        // itself takes no damage, `status.rs`).
        if self.is_invul(target) && target_kind != Some(ObjectKind::Npc) {
            self.send(id, &SystemMessage::new(SystemMessageId::TheAttackHasBeenBlocked));
        } else if target_kind == Some(ObjectKind::Door) {
            // (and `ControlTower`, a siege NPC: M5)
            let message = SystemMessage::new(SystemMessageId::YouHitForS1Damage)
                .with(SystemMessageParam::IntNumber(damage));
            self.send(id, &message);
        } else if id != target {
            // `MultilingualSupport` NPC name localisation is not ported.
            let message = SystemMessage::new(SystemMessageId::C1HasDoneS3PointsOfDamageToC2)
                .with(SystemMessageParam::PlayerName(attacker_name))
                .with(SystemMessageParam::Text(self.creature_name(target)))
                .with(SystemMessageParam::IntNumber(damage));
            self.send(id, &message);
        }
    }

    /// Java `target.reduceCurrentHp(amount, attacker, null)` with the overrides of `Player` and
    /// `Attackable`, down to the status classes' `reduceHp`.
    pub fn reduce_current_hp(&mut self, target: ObjectId, amount: f64, attacker: Option<ObjectId>) {
        if self.is_player(target) {
            self.player_reduce_hp(target, amount, attacker);
            return;
        }
        if !self.is_npc(target) {
            return;
        }
        if self.is_attackable_npc(target) {
            // Raid command channel looting rights: parties are M5.
            // Add damage and hate to the attacker AggroInfo of the Attackable _aggroList.
            if let Some(attacker) = attacker {
                self.attackable_add_damage(target, attacker, amount as i32);
            }
            // If this Attackable is a Monster and it has spawned minions, call its minions to
            // battle.
            if let Some(attacker) = attacker
                && self.is_monster(target)
            {
                self.minions_assist(target, attacker);
            }
            // Champions are not rolled (`ChampionMonstersConfig`), so the HP is not divided.
            // AttackableStatus.reduceHp: over-hit is a skill effect (M5).
        }
        // `CreatureStatus` / `NpcStatus` / `AttackableStatus.reduceHp` (`status.rs`).
        self.reduce_hp(target, amount, attacker);
    }

    /// Java `Attackable.addDamage(attacker, damage, skill)` and `addDamageHate`: the AI hears
    /// about the attack, the attacker's damage and hate grow.
    pub(crate) fn attackable_add_damage(&mut self, npc: ObjectId, attacker: ObjectId, damage: i32) {
        if self.is_dead(npc) {
            return;
        }
        // Walkers stop walking: the walking manager is not ported.
        self.notify_action_attacked(npc, attacker);
        // Calculate the amount of hate this attackable receives from this attack.
        let level = self.stats(npc).map_or(0, |s| s.level());
        let hate_value = (((i64::from(damage) * 100) / i64::from(level + 7)) as f64
            * self.hate_ratio(npc, attacker)) as i64;
        // `addDamageHate(attacker, damage, (int) hateValue)`: the int cast keeps the low 32 bits.
        self.add_damage_hate(npc, attacker, i64::from(damage), i64::from(hate_value as i32));
        // ON_ATTACKABLE_ATTACK: script event (D-05).
    }

    /// Java `Attackable.addDamageHate(attacker, damage, aggroValue)`.
    pub fn add_damage_hate(&mut self, npc: ObjectId, attacker: ObjectId, damage: i64, aggro_value: i64) {
        if attacker == npc {
            return;
        }
        let attacker_is_player = self.is_player(attacker);
        let state = self.combat_state_mut(npc);
        let index = match state.aggro.iter().position(|a| a.attacker == attacker) {
            Some(index) => index,
            None => {
                state.aggro.push(AggroInfo::new(attacker));
                state.aggro.len() - 1
            }
        };
        let mut aggro = aggro_value;
        // Traps do not cause aggro: there are no traps yet.
        state.aggro[index].add_damage(damage);
        state.aggro[index].add_hate(aggro);
        if attacker_is_player && aggro == 0 {
            self.add_damage_hate(npc, attacker, 0, 1);
            // Set the intention to the Attackable to ACTIVE.
            if self.intention_of(npc) == crate::movement::Intention::Idle {
                self.set_intention_active(npc);
            }
            // ON_ATTACKABLE_AGGRO_RANGE_ENTER: script event (D-05).
        } else if !attacker_is_player && aggro == 0 {
            aggro = 1;
            if let Some(info) = self.combat_state_mut(npc).aggro.get_mut(index) {
                info.add_hate(1);
            }
        }
        // Set the intention to the Attackable to ACTIVE.
        if aggro != 0 && self.intention_of(npc) == crate::movement::Intention::Idle {
            self.set_intention_active(npc);
        }
    }

    /// Java `Attackable.getHateRatio(attacker)`: `SetHateGroup`, `SetHateOccupation` and
    /// `SetHateRace` of the template's AI parameters.
    pub(crate) fn hate_ratio(&self, npc: ObjectId, attacker: ObjectId) -> f64 {
        let Some(player) = self.players.get(&attacker) else {
            return 1.0;
        };
        let Some(template) = self
            .npcs
            .get(&npc)
            .and_then(|n| self.npc_world.template(n.npc_id))
        else {
            return 1.0;
        };
        if template.parameters.is_empty() {
            return 1.0;
        }
        let int_param = |key: &str, default: i32| match template.parameters.get(key) {
            Some(NpcParam::Text(text)) => text.trim().parse().unwrap_or(default),
            _ => default,
        };
        let mut ratio = 0;
        // M4-AI-01: `SetHateGroup` needs `CategoryData` (class categories), which is not loaded;
        // no class is in any category, so the group ratio never applies.
        if player.data.class_id == int_param("SetHateOccupation", -1) {
            ratio += int_param("SetHateOccupationRatio", 0);
        }
        let hate_race = match template.parameters.get("SetHateRace") {
            Some(NpcParam::Text(text)) => NpcRace::from_name(text.trim()),
            _ => None,
        };
        let player_race = usize::try_from(player.data.race)
            .ok()
            .and_then(|r| NpcRace::ALL.get(r))
            .copied();
        if hate_race.is_some() && player_race == hate_race {
            ratio += int_param("SetHateRaceRatio", 0);
        }
        1.0 + (f64::from(ratio) / 100.0)
    }

    /// The aggro list of an attackable (Java `getAggroList()`), in the order attackers came.
    pub fn aggro_list(&self, npc: ObjectId) -> &[AggroInfo] {
        self.combat
            .creatures
            .get(&npc)
            .map_or(&[], |c| c.aggro.as_slice())
    }

    /// Java `Creature.doDie(killer)`: `false` when the creature was already dead.
    ///
    /// The class chain (`Playable`/`Player`, or `Creature`/`Npc`/`Attackable`) is in `death.rs`;
    /// the `Die` packet is `notifyActionDeath`'s (`ai/mod.rs`).
    pub fn do_die(&mut self, id: ObjectId, killer: Option<ObjectId>) -> bool {
        self.do_die_chain(id, killer)
    }
}
