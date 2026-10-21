//! What players are told about a summon: `PetInfo` and `PetStatusUpdate` to its owner,
//! `SummonInfo` to everybody else, the buff icons (`PartySpelled`) to the owner.
// Java: entity/actor/Summon.java (sendInfo, updateAndBroadcastStatus, broadcastNpcInfo,
//       updateAbnormalEffect, broadcastStatusUpdate, getWeapon, getArmor)
// Java: network/serverpackets/{PetInfo,PetStatusUpdate,AbstractNpcInfo}.java (SummonInfo)
// Java: entity/actor/holders/creature/EffectList.java (updateEffectIcons, the summon branch)
// Java: dist/game/data/scripts/handlers/actions/click/SummonClick.java (see `actions.rs`)

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{PetInfo, PetStatusUpdate, SummonInfo, SystemMessage, SystemMessageParam};
use l2_util::ObjectId;

use super::SummonKind;
use crate::stats::java_round;
use crate::timer::{TimerKind, TimerWheel};
use crate::world::World;

/// `Summon.updateAbnormalEffect()`'s task delay.
const ABNORMAL_EFFECT_DELAY_MS: u64 = 50;

/// The values `AbstractNpcInfo`'s constructor and `PetInfo`'s take from the creature alike.
struct Common {
    m_atk_spd: i32,
    p_atk_spd: i32,
    move_multiplier: f64,
    run_speed: i32,
    walk_speed: i32,
    swim_run_speed: i32,
    swim_walk_speed: i32,
    fly_run_speed: i32,
    fly_walk_speed: i32,
    attack_speed_multiplier: f32,
    collision_radius: f32,
    collision_height: f32,
    water_or_flying: u8,
    spawn_state: u8,
}

impl World {
    /// The speeds and sizes both info packets show (`_isSummoned ? 2 : value` included).
    fn summon_common(&self, summon: ObjectId, value: u8) -> Option<Common> {
        let npc = self.npcs.get(&summon)?;
        let state = self.summon(summon)?;
        let template = self.npc_world.template(npc.npc_id)?;
        let stats = self.stats(summon)?;
        // `getMovementSpeedMultiplier()` and the speeds divided by it, `(int) Math.round(...)`.
        let move_multiplier = stats.movement_speed_multiplier();
        let speed = |value: f64| java_round(value / move_multiplier) as i32;
        let (run_speed, walk_speed) = (speed(stats.run_speed()), speed(stats.walk_speed()));
        // `isInsideZone(ZoneId.WATER) ? 1 : isFlying() ? 2 : 0`.
        let water_or_flying = if npc.in_water {
            1
        } else if template.flying {
            2
        } else {
            0
        };
        Some(Common {
            m_atk_spd: stats.m_atk_spd(),
            p_atk_spd: stats.p_atk_spd() as i32,
            move_multiplier,
            run_speed,
            walk_speed,
            swim_run_speed: speed(stats.swim_run_speed()),
            swim_walk_speed: speed(stats.swim_walk_speed()),
            fly_run_speed: if template.flying { run_speed } else { 0 },
            fly_walk_speed: if template.flying { walk_speed } else { 0 },
            attack_speed_multiplier: stats.attack_speed_multiplier(),
            collision_radius: template.collision_radius,
            collision_height: template.collision_height,
            water_or_flying,
            spawn_state: if state.show_summon_animation { 2 } else { value },
        })
    }

    /// The owner's PvP flag and karma (`Summon.getPvpFlag()` / `getKarma()`).
    fn summon_flag_and_karma(&self, summon: ObjectId) -> (i32, i32) {
        match self.summon_owner(summon) {
            Some(owner) => (
                self.pvp_flag_of(owner),
                self.players.get(&owner).map_or(0, |p| p.data.karma),
            ),
            None => (0, 0),
        }
    }

    /// A pet's food or a servitor's life time: `(current, max)`.
    fn summon_fed(&self, summon: ObjectId) -> (i32, i32) {
        // A pet's food is PET-01's.
        self.summon(summon)
            .and_then(|s| s.servitor.as_ref())
            .map_or((0, 0), |s| (s.life_time_remaining, s.life_time))
    }

    /// `new PetInfo(summon, value)`: the owner's view of its summon.
    pub fn pet_info(&self, summon: ObjectId, value: u8) -> Option<PetInfo> {
        let common = self.summon_common(summon, value)?;
        let npc = self.npcs.get(&summon)?;
        let state = self.summon(summon)?;
        let template = self.npc_world.template(npc.npc_id)?;
        let stats = self.stats(summon)?;
        let (pvp_flag, karma) = self.summon_flag_and_karma(summon);
        let (cur_fed, max_fed) = self.summon_fed(summon);
        let name = match state.kind {
            SummonKind::Pet => state.name.clone(),
            SummonKind::Servitor if template.using_server_side_name => state.name.clone(),
            SummonKind::Servitor => String::new(),
        };
        let effects = self.abnormal_visual_effects(summon);
        Some(PetInfo {
            summon_type: state.kind.summon_type(),
            object_id: summon.0,
            display_id: template.display_id,
            x: npc.core.x,
            y: npc.core.y,
            z: npc.core.z,
            heading: npc.core.heading,
            m_atk_spd: common.m_atk_spd,
            p_atk_spd: common.p_atk_spd,
            run_speed: common.run_speed,
            walk_speed: common.walk_speed,
            swim_run_speed: common.swim_run_speed,
            swim_walk_speed: common.swim_walk_speed,
            fly_run_speed: common.fly_run_speed,
            fly_walk_speed: common.fly_walk_speed,
            move_multiplier: common.move_multiplier,
            attack_speed_multiplier: f64::from(common.attack_speed_multiplier),
            collision_radius: f64::from(common.collision_radius),
            collision_height: f64::from(common.collision_height),
            // `getWeapon()` / `getArmor()`: 0 for a servitor (a pet's are PET-01's).
            weapon: 0,
            armor: 0,
            // "when pet is dead and player exit game, pet doesn't show master name"
            has_owner: self.players.contains_key(&state.owner),
            running: npc.movement.running,
            in_combat: self.summon_in_combat(summon),
            alike_dead: self.is_alike_dead(summon),
            spawn_state: common.spawn_state,
            name,
            title: state.title.clone(),
            pvp_flag,
            karma,
            cur_fed,
            max_fed,
            cur_hp: npc.core.cur_hp as i32,
            max_hp: stats.max_hp(),
            cur_mp: npc.core.cur_mp as i32,
            max_mp: stats.max_mp(),
            sp: state.sp as i32,
            level: stats.level(),
            exp: state.exp,
            exp_for_this_level: self.summon_exp_for_this_level(summon),
            exp_for_next_level: self.summon_exp_for_next_level(summon),
            // A servitor carries nothing (`isPet() ? inventory weight : 0`).
            weight: 0,
            max_load: stats.max_load(),
            p_atk: stats.p_atk(None) as i32,
            p_def: stats.p_def(None) as i32,
            m_atk: stats.m_atk(None, None) as i32,
            m_def: stats.m_def(None, None) as i32,
            accuracy: stats.accuracy(),
            evasion: stats.evasion_rate(None),
            critical: stats.critical_hit(None, None),
            move_speed: stats.move_speed() as i32,
            atk_speed: stats.p_atk_spd() as i32,
            casting_speed: stats.m_atk_spd(),
            abnormal_visual_effects: effects.0,
            // `isMountable()`: no servitor is (striders and wyverns are pets).
            mountable: false,
            water_or_flying: common.water_or_flying,
            // `getTeam()`: the owner's; no events set a team yet.
            team_id: 0,
            soul_shots_per_hit: self.soul_shots_per_hit(summon),
            spirit_shots_per_hit: self.spirit_shots_per_hit(summon),
            form_id: self.summon_form_id(summon),
            abnormal_visual_effect_special: effects.1,
        })
    }

    /// `new PetStatusUpdate(summon)`.
    pub fn pet_status_update(&self, summon: ObjectId) -> Option<PetStatusUpdate> {
        let npc = self.npcs.get(&summon)?;
        let state = self.summon(summon)?;
        let stats = self.stats(summon)?;
        let (cur_fed, max_fed) = self.summon_fed(summon);
        Some(PetStatusUpdate {
            summon_type: state.kind.summon_type(),
            object_id: summon.0,
            x: npc.core.x,
            y: npc.core.y,
            z: npc.core.z,
            title: state.title.clone(),
            cur_fed,
            max_fed,
            cur_hp: npc.core.cur_hp as i32,
            max_hp: stats.max_hp(),
            cur_mp: npc.core.cur_mp as i32,
            max_mp: stats.max_mp(),
            level: stats.level(),
            exp: state.exp,
            exp_for_this_level: self.summon_exp_for_this_level(summon),
            exp_for_next_level: self.summon_exp_for_next_level(summon),
        })
    }

    /// `new SummonInfo(summon, viewer, value)`: what a player other than the owner sees.
    pub fn summon_info(&self, summon: ObjectId, viewer: ObjectId, value: u8) -> Option<SummonInfo> {
        let common = self.summon_common(summon, value)?;
        let npc = self.npcs.get(&summon)?;
        let state = self.summon(summon)?;
        let template = self.npc_world.template(npc.npc_id)?;
        let (pvp_flag, karma) = self.summon_flag_and_karma(summon);
        // `getOwner() != null && getOwner().isOnline() ? getOwner().getName() : ""`.
        let title = if !state.owner_offline && self.sinks.contains_key(&state.owner) {
            self.players
                .get(&state.owner)
                .map(|p| p.data.name.clone())
                .unwrap_or_default()
        } else {
            String::new()
        };
        let effects = self.abnormal_visual_effects(summon);
        // `_gmSeeInvis && isInvisible() ? effects | STEALTH : effects`.
        let invisible = self.objects.get(&summon).is_some_and(|e| e.invisible);
        let abnormal_visual_effects = if self.is_gm(viewer) && invisible {
            effects.0 | crate::player_view::STEALTH_MASK
        } else {
            effects.0
        };
        Some(SummonInfo {
            object_id: summon.0,
            display_id: template.display_id,
            // `cha.isAutoAttackable(attacker)` = the owner's (`Summon.isAutoAttackable`).
            attackable: self.summon_auto_attackable_by(summon, viewer),
            x: npc.core.x,
            y: npc.core.y,
            z: npc.core.z,
            heading: npc.core.heading,
            m_atk_spd: common.m_atk_spd,
            p_atk_spd: common.p_atk_spd,
            run_speed: common.run_speed,
            walk_speed: common.walk_speed,
            swim_run_speed: common.swim_run_speed,
            swim_walk_speed: common.swim_walk_speed,
            fly_run_speed: common.fly_run_speed,
            fly_walk_speed: common.fly_walk_speed,
            move_multiplier: common.move_multiplier,
            attack_speed_multiplier: common.attack_speed_multiplier,
            collision_radius: common.collision_radius,
            collision_height: common.collision_height,
            weapon: 0,
            armor: 0,
            in_combat: self.summon_in_combat(summon),
            alike_dead: self.is_alike_dead(summon),
            spawn_state: common.spawn_state,
            name: state.name.clone(),
            title,
            pvp_flag,
            karma,
            abnormal_visual_effects,
            water_or_flying: common.water_or_flying,
            team: 0,
            enchant_effect: template.weapon_enchant,
            form_id: self.summon_form_id(summon),
            abnormal_visual_effect_special: effects.1,
        })
    }

    /// Java `Summon.isAutoAttackable(attacker)`: `owner != null && owner.isAutoAttackable(attacker)`.
    pub(crate) fn summon_auto_attackable_by(&self, summon: ObjectId, attacker: ObjectId) -> bool {
        let Some(owner) = self.summon_owner(summon) else {
            return false;
        };
        if self.players.contains_key(&attacker) {
            return self.is_auto_attackable_by(owner, attacker);
        }
        self.is_auto_attackable_for(owner, attacker)
    }

    /// Java `Summon.sendInfo(player)`: the owner gets `PetInfo` (and the buff icons again, which
    /// `PetInfo` wipes), anybody else `SummonInfo`.
    pub(crate) fn send_summon_info(&mut self, summon: ObjectId, viewer: ObjectId) {
        let Some(owner) = self.summon_owner(summon) else {
            return;
        };
        if viewer == owner {
            if let Some(info) = self.pet_info(summon, 0) {
                self.send(viewer, &info);
            }
            // The PetInfo packet wipes the PartySpelled (list of active spells' icons). Re-add
            // them. (A pet's `PetItemList` is PET-01's.)
            self.summon_update_effect_icons(summon);
        } else if let Some(info) = self.summon_info(summon, viewer, 0) {
            self.send(viewer, &info);
        }
    }

    /// Java `Summon.updateAndBroadcastStatus(value)`: `PetInfo` and `PetStatusUpdate` to the
    /// owner, `SummonInfo` to everybody else who sees it, the icons again.
    pub fn update_and_broadcast_summon_status(&mut self, summon: ObjectId, value: u8) {
        let Some(owner) = self.summon_owner(summon) else {
            return;
        };
        if !self.objects.get(&summon).is_some_and(|e| e.spawned) {
            return;
        }
        if let Some(info) = self.pet_info(summon, value) {
            self.send(owner, &info);
        }
        if let Some(update) = self.pet_status_update(summon) {
            self.send(owner, &update);
        }
        self.broadcast_summon_npc_info(summon, value);
        self.summon_party_window_update(summon);
        self.summon_update_effect_icons(summon);
    }

    /// Java `Summon.broadcastNpcInfo(value)`: `SummonInfo` to every player who sees the summon,
    /// but its owner. (No visibility check, as in Java.)
    pub fn broadcast_summon_npc_info(&mut self, summon: ObjectId, value: u8) {
        let owner = self.summon_owner(summon);
        for viewer in self.visible_players(summon) {
            if Some(viewer) == owner {
                continue;
            }
            if let Some(info) = self.summon_info(summon, viewer, value) {
                self.send(viewer, &info);
            }
        }
    }

    /// `updateEffectIcons()` (`partyOnly` false) of a summon.
    pub(crate) fn summon_update_effect_icons_plain(&mut self, summon: ObjectId) {
        let _ = self.effect_list_mut(summon);
        self.update_effect_icons(summon, false);
    }

    /// `updateEffectIcons(true)` of a summon. Java makes the effect list with the creature, so
    /// even a summon without buffs sends its (empty) icon list.
    pub(crate) fn summon_update_effect_icons(&mut self, summon: ObjectId) {
        let _ = self.effect_list_mut(summon);
        self.update_effect_icons(summon, true);
    }

    /// Java `Summon.updateAbnormalEffect()`: `SummonInfo(value 1)` to every player who sees it
    /// (the owner too), 50 ms later, once.
    pub(crate) fn summon_update_abnormal_effect(&mut self, summon: ObjectId) {
        let Some(state) = self.summon(summon) else {
            return;
        };
        if state.abnormal_effect_timer.is_some() {
            return;
        }
        let due = self.tick + TimerWheel::ms_to_ticks(ABNORMAL_EFFECT_DELAY_MS);
        let timer = self
            .timers
            .schedule_at(due, TimerKind::Summon(super::SummonTimer::AbnormalEffect(summon)));
        if let Some(state) = self.summon_mut(summon) {
            state.abnormal_effect_timer = Some(timer);
        }
    }

    pub(crate) fn on_summon_abnormal_effect_timer(&mut self, summon: ObjectId) {
        let Some(state) = self.summon_mut(summon) else {
            return;
        };
        state.abnormal_effect_timer = None;
        if !self.objects.get(&summon).is_some_and(|e| e.spawned) {
            return;
        }
        for viewer in self.visible_players(summon) {
            if let Some(info) = self.summon_info(summon, viewer, 1) {
                self.send(viewer, &info);
            }
        }
    }

    /// Java `SystemMessage.addNpcName(Summon)`: the template id (`getId()`).
    pub(crate) fn summon_name_param(&self, summon: ObjectId) -> SystemMessageParam {
        SystemMessageParam::NpcName(self.summon_npc_id(summon).unwrap_or(0))
    }

    /// Java `Summon.sendDamageMessage(target, damage, mcrit, pcrit, miss)`: the owner hears about
    /// the summon's hit (not about a hit on itself, and nothing about a miss).
    pub(crate) fn summon_send_damage_message(
        &mut self,
        summon: ObjectId,
        target: ObjectId,
        damage: i32,
        mcrit: bool,
        pcrit: bool,
        miss: bool,
    ) {
        let Some(owner) = self.summon_owner(summon) else {
            return;
        };
        if miss {
            return;
        }
        // Prevents the double spam of system messages, if the target is the owning player.
        if target == owner {
            return;
        }
        if pcrit || mcrit {
            let message = if self.is_servitor(summon) {
                SystemMessageId::SummonedMonsterSCriticalHit
            } else {
                SystemMessageId::PetSCriticalHit
            };
            self.send(owner, &SystemMessage::new(message));
        }
        // Olympiad damage counting: no olympiad yet.
        let message = if self.is_invul(target) && !self.is_npc(target) {
            SystemMessage::new(SystemMessageId::TheAttackHasBeenBlocked)
        } else {
            SystemMessage::new(SystemMessageId::C1HasDoneS3PointsOfDamageToC2)
                .with(self.summon_name_param(summon))
                .with(SystemMessageParam::Text(self.creature_name(target)))
                .with(SystemMessageParam::IntNumber(damage))
        };
        self.send(owner, &message);
    }
}
