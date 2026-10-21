//! `Creature.doAttack` and the four ways a weapon attacks (simple, bow/crossbow, dual, pole).
// Java: entity/actor/Creature.java (doAttack, canUseRangeWeapon, doAttackHitByBow,
//       doAttackHitByCrossBow, doAttackHitByDual, doAttackHitByPole, doAttackHitSimple,
//       abortAttack, breakAttack, calculateTimeBetweenAttacks, calculateReuseTime)
// Java: entity/actor/Player.java (doAttack, checkAndEquipArrows, reduceArrowCount)
//
// # Time
// Java keeps `_attackEndTime` in `System.nanoTime()` and schedules the hit and the "ready to
// act" notification on its thread pool. Here the end time is in world milliseconds
// (`World::world_ms`, 50 ms steps) and both are timer-wheel events, rounded up to the next tick
// (`TimerWheel::ms_to_ticks`). The bow lock (`_disableBowAttackEndTime`) is in game ticks
// (100 ms), exactly as Java computes it.
//
// # Left out (each has its card)
// - Script listeners `ON_CREATURE_ATTACK(ED)` (D-05), fake players, cursed weapons, observer mode,
//   siege friends, transformations, effects (`POLEARM_SINGLE_TARGET`).
// - PvP flagging after the attack (`updatePvPStatus`) is `pvp.rs`.

use l2_data::Stat;
use l2_data::item::{CrystalType, EtcItemType, WeaponType};
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::charscreen::paperdoll;
use l2_protocol_game::server::combat::{HITFLAG_CRIT, HITFLAG_MISS, HITFLAG_SHLD, HITFLAG_USESS};
use l2_protocol_game::server::{ActionFailed, Attack, AttackHit, SetupGauge, SystemMessage};
use l2_util::ObjectId;

use super::HitTask;
use super::hit::{
    HitOutcome, calc_crit_for, calc_hit_miss_for, calc_hit_simple, calc_phys_dam_for, calc_shld_use_for,
};
use crate::formulas::{
    ReuseAttackType, ReuseInput, SHIELD_DEFENSE_PERFECT_BLOCK, SHIELD_DEFENSE_SUCCEED, calculate_reuse_time,
    calculate_time_between_attacks,
};
use crate::inventory::handlers::shots::ShotType;
use crate::movement::{heading_from_points, heading_to_degree};
use crate::objects::ObjectKind;
use crate::target::distance_3d;
use crate::timer::{TimerKind, TimerWheel};
use crate::world::World;
use crate::zones::ZoneId;

/// `GameTimeTaskManager.TICKS_PER_SECOND`.
const TICKS_PER_SECOND: i32 = 10;
/// `GameTimeTaskManager.MILLIS_IN_TICK`.
const MILLIS_IN_TICK: i32 = 100;
/// The delay of the "ready to act" retry after a refused bow shot (Java: 1000 ms).
const BOW_RETRY_MS: u64 = 1000;

/// The `Attack` packet being built (Java `Attack`): the hits in order.
struct AttackBuilder {
    soulshot: bool,
    ss_grade: i32,
    hits: Vec<AttackHit>,
}

impl AttackBuilder {
    /// Java `Attack.addHit` / `new Hit(...)`: the flag byte.
    fn add_hit(&mut self, target: ObjectId, outcome: &HitOutcome) {
        let mut flags: i32 = 0;
        if self.soulshot {
            flags |= i32::from(HITFLAG_USESS) | self.ss_grade;
        }
        if outcome.crit {
            flags |= i32::from(HITFLAG_CRIT);
        }
        if outcome.shld > 0 {
            flags |= i32::from(HITFLAG_SHLD);
        }
        if outcome.miss {
            flags |= i32::from(HITFLAG_MISS);
        }
        self.hits.push(AttackHit {
            target_id: target.0,
            damage: outcome.damage,
            // `writeByte(flags)`: the low 8 bits.
            flags: flags as u8,
        });
    }
}

impl World {
    /// Java `Creature.doAttack(target)` (with `Player.doAttack`): checks, the hits, `Attack` and
    /// the "ready to act" notification at the end of the attack.
    pub fn do_attack(&mut self, id: ObjectId, target: ObjectId) {
        // Java takes `_attackLock` here; the world thread is the only one that attacks.
        if !self.objects.contains_key(&target) || self.is_attack_disabled(id) || !self.is_targetable(target) {
            return;
        }
        // Java: the ON_CREATURE_ATTACK / ON_CREATURE_ATTACKED script listeners (D-05).

        // An NPC does not hit a corpse, nobody hits outside the surrounding regions, and a player
        // does not hit the dead (Java's two branches, which end the same way).
        if !self.is_alike_dead(id)
            && ((self.is_npc(id) && self.is_alike_dead(target))
                || !self.is_in_surrounding_region(id, target)
                || (self.is_player(id) && self.is_dead(target)))
        {
            self.set_intention_active(id);
            self.send(id, &ActionFailed);
            return;
        }
        // Transformations that cannot attack (a player only).
        if !self.is_alike_dead(id)
            && self
                .stats(id)
                .is_some_and(|v| v.is_transformed() && v.transformation().is_some_and(|t| !t.can_attack))
        {
            self.send(id, &ActionFailed);
            return;
        }

        // Check if attacker's weapon can attack.
        let weapon = self.stats(id).and_then(|s| s.active_weapon_item().cloned());
        if let Some(w) = weapon.as_ref().and_then(|t| t.weapon())
            && !w.attack_weapon
            && !self.is_gm(id)
        {
            let message = if w.weapon_type == WeaponType::FishingRod {
                SystemMessageId::YouLookOddlyAtTheFishingPoleInDisbeliefAndRealizeThatYouCanTAttackAnythingWithThis
            } else {
                SystemMessageId::ThatWeaponCannotPerformAnyAttacks
            };
            self.send(id, &SystemMessage::new(message));
            self.send(id, &ActionFailed);
            return;
        }

        if self.is_player(id) {
            // Observer mode and siege friends: M5.
            // Checking if target has moved to peace zone.
            if self.target_is_inside_peace_zone(target, id) {
                self.set_intention_active(id);
                self.send(id, &ActionFailed);
                return;
            }
        } else if self.is_inside_peace_zone(id, id, target) {
            self.set_intention_active(id);
            return;
        }

        self.stop_effects_on_action(id);

        // GeoData Los Check here (or dz > 1000).
        if !self.can_see_target(id, target) {
            self.send(id, &SystemMessage::new(SystemMessageId::CannotSeeTarget));
            self.set_intention_active(id);
            self.send(id, &ActionFailed);
            return;
        }

        // Mobius: Do not move when attack is launched.
        if self.is_moving(id)
            && let Some(core) = self.core_of(id)
        {
            let loc = (core.x, core.y, core.z, core.heading);
            self.stop_move(id, Some(loc));
        }

        let p_atk_spd = self.stats(id).map_or(0.0, |s| s.p_atk_spd());
        let time_atk = calculate_time_between_attacks(p_atk_spd);
        let time_to_hit = time_atk / 2;
        let soulshot = self.is_charged_shot(id, ShotType::Soulshots);
        let ss_grade = weapon.as_ref().map_or(0, |t| t.crystal_type_plus().level());
        let (Some(attacker_pos), Some(target_pos)) = (self.position_of(id), self.position_of(target)) else {
            return;
        };
        let mut attack = AttackBuilder {
            soulshot,
            ss_grade,
            hits: Vec::new(),
        };
        self.set_heading(
            id,
            heading_from_points(attacker_pos.0, attacker_pos.1, target_pos.0, target_pos.1),
        );
        let reuse = self.reuse_time(id);
        let now = self.world_ms();
        let attack_type = self.stats(id).map(|s| s.attack_type());
        let is_player = self.is_player(id);
        let hitted = match attack_type {
            Some(WeaponType::Bow) => {
                if !self.can_use_range_weapon(id) {
                    return;
                }
                self.set_attack_end(id, now + i64::from(time_to_hit + (reuse / 2)));
                self.do_attack_hit_by_bow(&mut attack, id, target, time_atk, reuse, false)
            }
            Some(WeaponType::CrossBow) => {
                if !self.can_use_range_weapon(id) {
                    return;
                }
                self.set_attack_end(id, now + i64::from(time_to_hit + (reuse / 2)));
                self.do_attack_hit_by_bow(&mut attack, id, target, time_atk, reuse, true)
            }
            Some(WeaponType::Pole) => {
                self.set_attack_end(id, now + i64::from(time_atk));
                self.do_attack_hit_by_pole(&mut attack, id, target, time_to_hit)
            }
            Some(WeaponType::Fist) if !is_player => {
                self.set_attack_end(id, now + i64::from(time_atk));
                self.do_attack_hit_simple(&mut attack, id, target, 100.0, time_to_hit, true)
            }
            // NOTE(java-quirk): a player's FIST falls through to the dual attack, so bare hands
            // (and fist weapons) hit twice.
            Some(WeaponType::Fist | WeaponType::Dual | WeaponType::DualFist | WeaponType::DualDagger) => {
                self.set_attack_end(id, now + i64::from(time_atk));
                self.do_attack_hit_by_dual(&mut attack, id, target, time_to_hit)
            }
            _ => {
                self.set_attack_end(id, now + i64::from(time_atk));
                self.do_attack_hit_simple(&mut attack, id, target, 100.0, time_to_hit, true)
            }
        };

        // Flag the attacker if it's a Player outside a PvP area (`asPlayer()`: a summon's owner).
        let flag_on_raid_or_champion = (self.is_raid(target) && self.settings.pvp.flag_player_on_raid_attack)
            || (self.stats(target).is_some_and(|v| v.is_champion())
                && self.settings.pvp.flag_player_on_champion_attack);
        let acting = self.acting_player(id);
        if let Some(player) = acting.filter(|p| !self.is_inside_zone(*p, ZoneId::Pvp) && *p != target) {
            self.add_attack_stance_task(player);
            // Attacking the own pet does not flag.
            if self.summon_of(player) != Some(target) {
                if flag_on_raid_or_champion {
                    self.update_pvp_status(player);
                } else {
                    self.update_pvp_status_target(player, target);
                }
            }
        } else if self.settings.pvp.flag_player_on_summon_pet_raid_champion_attack
            && self.is_summon(id)
            && let Some(owner) = self.summon_owner(id)
            && !self.is_inside_zone(owner, ZoneId::Pvp)
            && owner != target
            && self.summon_of(owner) != Some(target)
            && flag_on_raid_or_champion
        {
            // NOTE(java-quirk): `asPlayer()` of a summon is its owner, so the branch above takes
            // every case this one could see; it is kept as Java has it.
            self.add_attack_stance_task(owner);
            self.update_pvp_status(owner);
        }

        if !hitted {
            // Abort the attack of the Creature and send Server->Client ActionFailed packet.
            self.abort_attack(id);
        } else {
            // If we didn't miss the hit, discharge the shoulshots, if any.
            self.set_charged_shot(id, ShotType::Soulshots, false);
            // M5: cursed weapons and heroes zero the target's CP.
        }

        // If the Server->Client packet Attack contains at least 1 hit, send it to the Creature
        // AND to all Player in the _KnownPlayers of the Creature.
        let mut hits = attack.hits.into_iter();
        if let Some(first_hit) = hits.next() {
            let packet = Attack {
                attacker_id: id.0,
                attacker_x: attacker_pos.0,
                attacker_y: attacker_pos.1,
                attacker_z: attacker_pos.2,
                first_hit,
                other_hits: hits.collect(),
                target_x: target_pos.0,
                target_y: target_pos.1,
                target_z: target_pos.2,
            };
            self.broadcast_creature_packet(id, &packet);
        }

        // Notify AI with READY_TO_ACT.
        self.schedule_ready_to_act(id, u64::try_from(time_atk.saturating_add(reuse)).unwrap_or(0));
    }

    /// Java `Creature.broadcastPacket`: a player also gets its own packet, first.
    pub(crate) fn broadcast_creature_packet(&self, id: ObjectId, packet: &impl l2_net::Encode) {
        if self.is_player(id) {
            self.broadcast_with_self(id, packet);
        } else {
            self.broadcast_packet(id, packet);
        }
    }

    fn set_attack_end(&mut self, id: ObjectId, end_ms: i64) {
        self.combat_state_mut(id).attack_end_ms = end_ms;
    }

    /// `ThreadPool.schedule(() -> getAI().notifyActionReadyToAct(), delay)`.
    pub(crate) fn schedule_ready_to_act(&mut self, id: ObjectId, delay_ms: u64) {
        let due = self.tick + TimerWheel::ms_to_ticks(delay_ms);
        self.timers.schedule_at(due, TimerKind::ReadyToAct(id));
    }

    fn schedule_hit(&mut self, task: HitTask, delay_ms: i32) {
        let due = self.tick + TimerWheel::ms_to_ticks(u64::try_from(delay_ms).unwrap_or(0));
        self.timers.schedule_at(due, TimerKind::Hit(task));
    }

    /// Java `Creature.calculateReuseTime(getActiveWeaponItem())`.
    fn reuse_time(&self, id: ObjectId) -> i32 {
        let Some(view) = self.stats(id) else {
            return 0;
        };
        let weapon = view.active_weapon_item();
        calculate_reuse_time(&ReuseInput {
            p_atk_spd: view.p_atk_spd(),
            weapon_reuse_delay: weapon.map(|t| t.reuse_delay),
            is_transformed: view.is_transformed(),
            attack_type: match view.attack_type() {
                WeaponType::Bow => ReuseAttackType::Bow,
                WeaponType::CrossBow => ReuseAttackType::Crossbow,
                _ => ReuseAttackType::Other,
            },
            weapon_reuse_modifier: 1.0,
        })
    }

    /// The `calcShldUse(..., sendSysMsg = true)` messages to a player target.
    fn send_shield_message(&self, target: ObjectId, shld: u8) {
        if !self.is_player(target) {
            return;
        }
        match shld {
            SHIELD_DEFENSE_SUCCEED => {
                self.send(
                    target,
                    &SystemMessage::new(SystemMessageId::YourShieldDefenseHasSucceeded),
                );
            }
            SHIELD_DEFENSE_PERFECT_BLOCK => {
                self.send(
                    target,
                    &SystemMessage::new(SystemMessageId::YourExcellentShieldDefenseWasASuccess),
                );
            }
            _ => {}
        }
    }

    /// One hit's rolls with the world RNG, and the shield message right after the shield roll
    /// (Java sends it from inside `calcShldUse`, before the crit and damage rolls; nothing else
    /// goes out in between, so sending it after the rolls keeps the packet order).
    fn roll_hit(
        &mut self,
        attacker: ObjectId,
        target: ObjectId,
        attack_percent: f64,
        ss: bool,
    ) -> HitOutcome {
        let outcome =
            self.with_rng(|world, rng| calc_hit_simple(world, attacker, target, attack_percent, ss, rng));
        if !outcome.miss {
            self.send_shield_message(target, outcome.shld);
        }
        outcome
    }

    /// Java `Creature.doAttackHitSimple(attack, target, attackpercent, sAtk, rechargeShots)`.
    fn do_attack_hit_simple(
        &mut self,
        attack: &mut AttackBuilder,
        id: ObjectId,
        target: ObjectId,
        attack_percent: f64,
        s_atk: i32,
        recharge_shots: bool,
    ) -> bool {
        let outcome = self.roll_hit(id, target, attack_percent, attack.soulshot);
        self.schedule_hit(
            hit_task(id, target, &outcome, attack.soulshot, recharge_shots),
            s_atk,
        );
        attack.add_hit(target, &outcome);
        !outcome.miss
    }

    /// Java `Creature.doAttackHitByBow` and `doAttackHitByCrossBow` (they differ in the bolts, the
    /// distance bonus and one message).
    fn do_attack_hit_by_bow(
        &mut self,
        attack: &mut AttackBuilder,
        id: ObjectId,
        target: ObjectId,
        s_atk: i32,
        reuse: i32,
        crossbow: bool,
    ) -> bool {
        let ss = attack.soulshot;
        let miss = self.with_rng(|world, rng| calc_hit_miss_for(world, id, target, rng));
        // Consume arrows.
        self.reduce_arrow_count(id, crossbow);
        // Java `_move = null`: the move data goes without a `StopMove`.
        if let Some(slot) = self.mv_slot(id) {
            *slot = None;
        }
        let mut outcome = HitOutcome {
            miss,
            shld: 0,
            crit: false,
            damage: 0,
        };
        if !miss {
            outcome.shld = self.with_rng(|world, rng| calc_shld_use_for(world, id, target, rng));
            self.send_shield_message(target, outcome.shld);
            outcome.crit = self.with_rng(|world, rng| calc_crit_for(world, id, target, rng));
            outcome.damage = self.with_rng(|world, rng| {
                calc_phys_dam_for(world, id, target, outcome.shld, outcome.crit, ss, rng)
            }) as i32;
            // Bows Ranged Damage Formula (Damage gradually decreases when 60% or lower than full
            // hit range, and increases when 60% or higher).
            if !crossbow && self.settings.combat.calculate_distance_bow_damage {
                let distance = match (self.position_of(id), self.position_of(target)) {
                    (Some(a), Some(b)) => distance_3d(a, b),
                    _ => 0.0,
                };
                // `damage1 *= double`: the product is cast back to the int.
                outcome.damage = (f64::from(outcome.damage) * ((distance / 4000.0) + 0.8)) as i32;
            }
        }

        if self.is_player(id) {
            if crossbow {
                self.send(
                    id,
                    &SystemMessage::new(SystemMessageId::YourCrossbowIsPreparingToFire),
                );
            }
            self.send(
                id,
                &SetupGauge::new(id.0, SetupGauge::RED, s_atk.wrapping_add(reuse)),
            );
        }

        self.schedule_hit(hit_task(id, target, &outcome, ss, true), s_atk);

        // Calculate and set the disable delay of the bow in function of the Attack Speed.
        let game_time = self.game_ticks();
        let mut end = game_time.wrapping_add(s_atk.wrapping_add(reuse) / MILLIS_IN_TICK);
        // Precaution. It happened in the past for _attackEndTime. Will not risk it.
        if end < game_time {
            end = i32::MAX;
        }
        self.combat_state_mut(id).disable_bow_attack_end_time = end;

        attack.add_hit(target, &outcome);
        !miss
    }

    /// Java `Creature.doAttackHitByDual`: two hits at half damage each, both rolled before either
    /// lands.
    fn do_attack_hit_by_dual(
        &mut self,
        attack: &mut AttackBuilder,
        id: ObjectId,
        target: ObjectId,
        s_atk: i32,
    ) -> bool {
        let ss = attack.soulshot;
        // Calculate if hits are missed or not (both first).
        let miss1 = self.with_rng(|world, rng| calc_hit_miss_for(world, id, target, rng));
        let miss2 = self.with_rng(|world, rng| calc_hit_miss_for(world, id, target, rng));
        let mut hits = [
            HitOutcome {
                miss: miss1,
                shld: 0,
                crit: false,
                damage: 0,
            },
            HitOutcome {
                miss: miss2,
                shld: 0,
                crit: false,
                damage: 0,
            },
        ];
        for hit in &mut hits {
            if !hit.miss {
                hit.shld = self.with_rng(|world, rng| calc_shld_use_for(world, id, target, rng));
                self.send_shield_message(target, hit.shld);
                hit.crit = self.with_rng(|world, rng| calc_crit_for(world, id, target, rng));
                let (shld, crit) = (hit.shld, hit.crit);
                hit.damage = self
                    .with_rng(|world, rng| calc_phys_dam_for(world, id, target, shld, crit, ss, rng))
                    as i32;
                hit.damage /= 2;
            }
        }
        self.schedule_hit(hit_task(id, target, &hits[0], ss, true), s_atk / 2);
        self.schedule_hit(hit_task(id, target, &hits[1], ss, false), s_atk);
        attack.add_hit(target, &hits[0]);
        attack.add_hit(target, &hits[1]);
        !miss1 || !miss2
    }

    /// Java `Creature.doAttackHitByPole`: the target, then up to `ATTACK_COUNT_MAX - 1` more
    /// creatures in front, each at 85 % of the last one's share.
    fn do_attack_hit_by_pole(
        &mut self,
        attack: &mut AttackBuilder,
        id: ObjectId,
        target: ObjectId,
        s_atk: i32,
    ) -> bool {
        // Perform the main target hit.
        let mut hitted = self.do_attack_hit_simple(attack, id, target, 100.0, s_atk, true);

        if self.is_affected(id, crate::effects::effect_flag::POLEARM_SINGLE_TARGET) {
            return hitted;
        }
        let Some(view) = self.stats(id) else {
            return hitted;
        };
        // Without Polearm Mastery (skill 216) max simultaneous attacks is 3 (1 by default + 2 in
        // skill 3599).
        let mut attack_count_max = view.calc(Stat::AttackCountMax, 1.0) as i32;
        if attack_count_max > 1 {
            let heading_angle = heading_to_degree(self.heading_of(id));
            let max_radius = view.physical_attack_range();
            // `PlayableStat.getPhysicalAttackAngle()`: the weapon's angle (0 without one);
            // `NpcStat`: the template's.
            let physical_attack_angle = if view.is_playable() {
                view.active_weapon_item()
                    .and_then(|t| t.weapon())
                    .map_or(0, |w| w.base_attack_angle)
            } else {
                view.physical_attack_angle()
            };
            let mut attack_percent = 85.0;
            let me = self.position_of(id).unwrap_or_default();
            let attacker_is_attackable = self.is_attackable_npc(id);
            let main_target_attackable = self.is_attackable_npc(target);
            for obj in self.visible_objects_in_range(id, max_radius) {
                // Skip main target.
                if obj == target {
                    continue;
                }
                if !matches!(
                    self.objects.get(&obj).map(|e| e.kind),
                    Some(ObjectKind::Player | ObjectKind::Npc)
                ) {
                    // Doors and static objects are creatures in Java too, but never auto
                    // attackable without a siege (M5).
                    continue;
                }
                // Skip dead or fake dead target.
                if self.is_alike_dead(obj) {
                    continue;
                }
                // Check if target is auto attackable.
                if !self.is_auto_attackable_for(obj, id) {
                    continue;
                }
                // Check if target is within attack angle.
                let Some(pos) = self.position_of(obj) else {
                    continue;
                };
                let direction = direction_to(me, pos);
                if (direction - heading_angle).abs() > f64::from(physical_attack_angle) {
                    continue;
                }
                // Pets: none yet.
                if attacker_is_attackable && self.is_player(obj) && main_target_attackable {
                    continue;
                }
                // `!asAttackable().isChaos()`: chaos monsters are not ported.
                if attacker_is_attackable && self.is_attackable_npc(obj) {
                    continue;
                }
                // Launch a simple attack against the additional target.
                hitted |= self.do_attack_hit_simple(attack, id, obj, attack_percent, s_atk, false);
                attack_percent /= 1.15;
                attack_count_max -= 1;
                if attack_count_max <= 0 {
                    break;
                }
            }
        }
        hitted
    }

    /// Java `Creature.canUseRangeWeapon()`.
    fn can_use_range_weapon(&mut self, id: ObjectId) -> bool {
        // Transformations may always shoot.
        if self.is_transformed(id) {
            return true;
        }
        let game_ticks = self.game_ticks();
        let disabled_until = self
            .combat
            .creatures
            .get(&id)
            .map_or(0, |c| c.disable_bow_attack_end_time);
        if self.is_player(id) {
            let Some(weapon) = self.stats(id).and_then(|s| s.active_weapon_item().cloned()) else {
                return false;
            };
            let Some(info) = weapon.weapon() else {
                return false;
            };
            if !info.weapon_type.is_ranged() {
                return false;
            }
            // Equip arrows needed in left hand and send a Server->Client packet ItemList to the
            // Player then return True.
            // NOTE(java-quirk): crossbows also go through `checkAndEquipArrows`, so they pick up
            // arrows (not bolts) when the left hand is empty.
            if !self.check_and_equip_arrows(id, weapon.crystal_type_plus()) {
                // Cancel the action because the Player have no arrow.
                self.set_intention_idle(id);
                self.send(id, &ActionFailed);
                let message = if info.weapon_type == WeaponType::Bow {
                    SystemMessageId::YouHaveRunOutOfArrows
                } else {
                    SystemMessageId::NotEnoughBolts
                };
                self.send(id, &SystemMessage::new(message));
                return false;
            }

            // Verify if the bow can be use.
            if disabled_until <= game_ticks {
                // Verify if Player owns enough MP.
                let mut mp_consume = info.mp_consume;
                if info.reduced_mp_consume > 0 {
                    let chance = info.reduced_mp_consume_chance;
                    if self.rng().below(100) < chance {
                        mp_consume = info.reduced_mp_consume;
                    }
                }
                let mp_consume = self.stats(id).map_or(0, |s| {
                    s.calc_stat(Stat::BowMpConsumeRate, f64::from(mp_consume), None, None) as i32
                });
                let cur_mp = self.core_of(id).map_or(0.0, |c| c.cur_mp);
                if cur_mp < f64::from(mp_consume) {
                    // If Player doesn't have enough MP, stop the attack.
                    self.schedule_ready_to_act(id, BOW_RETRY_MS);
                    self.send(id, &SystemMessage::new(SystemMessageId::NotEnoughMp));
                    self.send(id, &ActionFailed);
                    return false;
                }
                // If Player have enough MP, the bow consumes it.
                if mp_consume > 0 {
                    self.reduce_mp(id, f64::from(mp_consume));
                }
                // Set the period of bow no re-use.
                self.combat_state_mut(id).disable_bow_attack_end_time =
                    (5 * TICKS_PER_SECOND).wrapping_add(game_ticks);
            } else {
                // Cancel the action because the bow can't be re-use at this moment.
                self.schedule_ready_to_act(id, BOW_RETRY_MS);
                self.send(id, &ActionFailed);
                return false;
            }
        } else if self.is_npc(id) && disabled_until > game_ticks {
            return false;
        }
        true
    }

    /// Java `Player.checkAndEquipArrows()`: arrows in the left hand, or the first arrows of the
    /// bow's grade from the inventory put there.
    pub(crate) fn check_and_equip_arrows(&mut self, id: ObjectId, grade: CrystalType) -> bool {
        let Some(view) = self.stats(id) else {
            return false;
        };
        if view.paperdoll(paperdoll::LHAND).is_some() {
            return true;
        }
        let Some(data) = view.stat_data() else {
            return false;
        };
        let Some(player) = view.player() else {
            return false;
        };
        let arrow = player.data.items.iter().find(|item| {
            item.loc == "INVENTORY"
                && data.items.get(item.item_id).is_some_and(|t| {
                    t.crystal_type_plus() == grade
                        && t.etc().is_some_and(|e| e.etc_type == EtcItemType::Arrow)
                })
        });
        let Some(arrow) = arrow.map(|i| i.object_id) else {
            return false;
        };
        self.equip_arrows(id, arrow);
        true
    }

    /// `Inventory.setPaperdollItem(PAPERDOLL_LHAND, arrows)` and `sendItemList(false)` (the
    /// inventory's paperdoll path: listeners, the database row, the delayed item list).
    fn equip_arrows(&mut self, id: ObjectId, arrow: ObjectId) {
        let mut changed = Vec::new();
        self.set_paperdoll_item(id, paperdoll::LHAND, Some(arrow), &mut changed);
        // Send a Server->Client packet ItemList to this Player to update left hand equipment.
        self.send_item_list(id, false);
    }

    /// Java `Player.reduceArrowCount(bolts)`: one arrow (bolt) less; the last one is destroyed and
    /// leaves the left hand.
    // Java clears `_arrowItem` / `_boltItem` here; the left hand is the only record of them in
    // Rust, so `bolts` changes nothing.
    fn reduce_arrow_count(&mut self, id: ObjectId, bolts: bool) {
        let _ = bolts;
        if !self.is_player(id) {
            return;
        }
        let Some(arrows) = self.paperdoll_object(id, paperdoll::LHAND) else {
            let mut changed = Vec::new();
            self.set_paperdoll_item(id, paperdoll::LHAND, None, &mut changed);
            self.send_item_list(id, false);
            return;
        };
        let count = self.inventory_item(id, arrows).map_or(0, |i| i.count);
        if count > 1 {
            // Adjust item quantity: `changeCount(-1)` (written with the next store, like Java's
            // `changeCount`), `refreshWeight()`, then the `InventoryUpdate`.
            self.destroy_inventory_item(id, arrows, 1);
            if let Some(entry) = self.modified_entry(id, arrows) {
                self.send_inventory_update(id, entry);
            }
        } else {
            // Destroy entire item and save to database.
            self.destroy_inventory_item(id, arrows, count);
            let mut changed = Vec::new();
            self.set_paperdoll_item(id, paperdoll::LHAND, None, &mut changed);
            self.send_item_list(id, false);
        }
    }

    /// Java `Creature.abortAttack()`: only a client in the middle of an attack hears about it.
    pub fn abort_attack(&mut self, id: ObjectId) {
        if self.is_attacking_now(id) {
            self.send(id, &ActionFailed);
        }
    }

    /// Java `Creature.breakAttack()`.
    pub fn break_attack(&mut self, id: ObjectId) {
        if self.is_attacking_now(id) {
            // Abort the attack of the Creature and send Server->Client ActionFailed packet.
            self.abort_attack(id);
            if self.is_player(id) {
                self.send(id, &SystemMessage::new(SystemMessageId::YourAttackHasFailed));
            }
        }
    }
}

/// The `HitTask` of one rolled hit.
fn hit_task(
    attacker: ObjectId,
    target: ObjectId,
    outcome: &HitOutcome,
    soulshot: bool,
    recharge_shots: bool,
) -> HitTask {
    HitTask {
        attacker,
        target,
        damage: outcome.damage,
        crit: outcome.crit,
        miss: outcome.miss,
        shld: outcome.shld,
        soulshot,
        recharge_shots,
    }
}

/// Java `calculateDirectionTo(target)`: the angle in degrees [0, 360) from `from` to `to`.
fn direction_to(from: (i32, i32, i32), to: (i32, i32, i32)) -> f64 {
    let mut angle = f64::from(to.1.wrapping_sub(from.1))
        .atan2(f64::from(to.0.wrapping_sub(from.0)))
        .to_degrees();
    if angle < 0.0 {
        angle += 360.0;
    }
    angle
}
