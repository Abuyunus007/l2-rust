//! The skill trap: a hidden NPC that casts its skill at the first creature that comes within 150.
// Java: entity/actor/instance/Trap.java, entity/actor/tasks/npc/trap/*.java
// Java: dist/game/data/scripts/handlers/skill/effects/{SummonTrap,TrapDetect,TrapRemove}.java
// Java: mechanics/script/Quest.java (`addTrap`), network/serverpackets/AbstractNpcInfo.java (`TrapInfo`)

use std::collections::BTreeSet;

use l2_data::NpcTemplate;
use l2_data::npc::NpcParam;
use l2_data::skill::TargetType;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SocialAction, SystemMessage, TrapInfo};
use l2_util::ObjectId;

use super::{DecoyTrapTimer, TrapState};
use crate::effects::SkillKey;
use crate::npc::NpcClass;
use crate::script::{ScriptEvent, ScriptNpc, TrapAction};
use crate::skills::TargetContext;
use crate::timer::{TimerKind, TimerWheel};
use crate::world::World;
use crate::zones::ZoneId;

/// `Trap.TICK`.
const TICK_MS: u64 = 1000;
/// `checkTarget`: "Range seems to be reduced from Freya(300) to H5(150)."
const TRIGGER_RANGE: f64 = 150.0;

impl World {
    /// Java `new Trap(template, owner, lifeTime)` / `new Trap(template, instanceId, lifeTime)`,
    /// then `fullRestore()`, `setInvul(true)`, the heading and `spawnMe(x, y, z)`. `owner` is none
    /// for a script's trap (`Quest.addTrap`, `life_time` -1: it never runs out).
    ///
    /// Does not set `Player._trap`; [`Self::summon_trap`] does.
    pub fn spawn_trap(
        &mut self,
        npc_id: i32,
        owner: Option<ObjectId>,
        life_time: i32,
        (x, y, z): (i32, i32, i32),
        heading: i32,
        instance: i32,
    ) -> Option<ObjectId> {
        let templates = self.npc_world.templates_arc()?;
        let template = templates.get(npc_id)?;
        let id = self.make_special_npc(template, NpcClass::Trap, (x, y, z), heading, instance)?;
        let skill = trap_skill_of(template);
        let state = TrapState {
            owner,
            skill,
            has_life_time: life_time >= 0,
            life_time: if life_time != 0 { life_time } else { 30000 },
            remaining_time: if life_time != 0 { life_time } else { 30000 },
            is_in_arena: false,
            triggered: false,
            detected: BTreeSet::new(),
            task: None,
        };
        // `if (_skill != null) _trapTask = scheduleAtFixedRate(new TrapTask(this), TICK, TICK)`.
        let task = skill.map(|_| {
            self.timers.schedule_at(
                self.tick + TimerWheel::ms_to_ticks(TICK_MS),
                TimerKind::DecoyTrap(DecoyTrapTimer::TrapTick(id)),
            )
        });
        self.decoy_trap.traps.insert(id, TrapState { task, ..state });
        // `setInvul(true)` (both callers).
        self.set_invul(id, true);
        self.spawn_at(id, x, y, z);
        if !self.npcs.contains_key(&id) {
            return None;
        }
        // `Trap.onSpawn`: `_isInArena = isInsideZone(PVP) && !isInsideZone(SIEGE)`; the detectors
        // are forgotten.
        let in_arena = self.is_inside_zone(id, ZoneId::Pvp) && !self.is_inside_zone(id, ZoneId::Siege);
        if let Some(state) = self.decoy_trap.traps.get_mut(&id) {
            state.is_in_arena = in_arena;
            state.detected.clear();
        }
        Some(id)
    }

    /// `SummonTrap.onStart` from the checks on: the previous trap goes, the new one is placed at
    /// the player. `None` when nothing was made.
    pub fn summon_trap(&mut self, player: ObjectId, npc_id: i32, despawn_time: i32) -> Option<ObjectId> {
        // Unsummon previous trap.
        if let Some(old) = self.trap_of(player) {
            self.trap_unsummon(old);
        }
        self.npc_world.template(npc_id)?;
        let (x, y, z) = self.position_of(player)?;
        let heading = self.players.get(&player)?.core.heading;
        let instance = self.objects.get(&player).map_or(0, |e| e.instance_id);
        let trap = self.spawn_trap(npc_id, Some(player), despawn_time, (x, y, z), heading, instance)?;
        self.decoy_trap.trap_of.insert(player, trap);
        Some(trap)
    }

    /// Java `Trap.getSkill()`.
    pub fn trap_skill(&self, trap: ObjectId) -> Option<SkillKey> {
        self.decoy_trap.traps.get(&trap)?.skill
    }

    /// Java `Trap.canBeSeen(creature)`.
    pub fn trap_can_be_seen(&self, trap: ObjectId, creature: Option<ObjectId>) -> bool {
        let Some(state) = self.decoy_trap.traps.get(&trap) else {
            return false;
        };
        if creature.is_some_and(|c| state.detected.contains(&c)) {
            return true;
        }
        let (Some(owner), Some(creature)) = (state.owner, creature) else {
            return false;
        };
        if creature == owner {
            return true;
        }
        if self.is_player(creature) {
            // observers can't see trap
            if self.in_observer_mode(creature) {
                return false;
            }
            // olympiad competitors can't see trap
            if self.is_in_olympiad_mode(owner)
                && self.is_in_olympiad_mode(creature)
                && self.olympiad_side(creature) != self.olympiad_side(owner)
            {
                return false;
            }
        }
        if state.is_in_arena {
            return true;
        }
        self.is_in_party(owner) && self.is_in_party(creature) && self.is_in_party_with(owner, creature)
    }

    /// `Trap.isAutoAttackable(attacker)`.
    pub(crate) fn trap_auto_attackable(&self, trap: ObjectId, attacker: Option<ObjectId>) -> bool {
        !self.trap_can_be_seen(trap, attacker)
    }

    /// `Trap.broadcastPacket(packet)`: only players who can see the trap, everybody once it went
    /// off.
    pub(crate) fn trap_broadcast(&self, trap: ObjectId, packet: &impl l2_net::Encode) {
        let triggered = self.trap_is_triggered(trap);
        let bytes = packet.to_bytes();
        for viewer in self.visible_players(trap) {
            if triggered || self.trap_can_be_seen(trap, Some(viewer)) {
                self.send_bytes(viewer, bytes.clone());
            }
        }
    }

    /// `new TrapInfo(trap, attacker)`; `attacker` none is Java's `null` of `triggerTrap`.
    pub fn trap_info(&self, trap: ObjectId, attacker: Option<ObjectId>) -> Option<TrapInfo> {
        let npc = self.npcs.get(&trap)?;
        let template = self.npc_world.template(npc.npc_id)?;
        let stats = self.stats(trap)?;
        let state = self.decoy_trap.traps.get(&trap)?;
        let move_multiplier = stats.movement_speed_multiplier();
        let speed = |value: f64| crate::stats::java_round(value / move_multiplier) as i32;
        let (run_speed, walk_speed) = (speed(stats.run_speed()), speed(stats.walk_speed()));
        let owner = state.owner;
        let title = owner
            .and_then(|o| self.players.get(&o))
            .map(|p| p.data.name.clone())
            .unwrap_or_default();
        Some(TrapInfo {
            object_id: trap.0,
            display_id: template.display_id,
            attackable: self.trap_auto_attackable(trap, attacker),
            x: npc.core.x,
            y: npc.core.y,
            z: npc.core.z,
            heading: npc.core.heading,
            m_atk_spd: stats.m_atk_spd(),
            p_atk_spd: stats.p_atk_spd() as i32,
            run_speed,
            walk_speed,
            swim_run_speed: speed(stats.swim_run_speed()),
            swim_walk_speed: speed(stats.swim_walk_speed()),
            fly_run_speed: if template.flying { run_speed } else { 0 },
            fly_walk_speed: if template.flying { walk_speed } else { 0 },
            move_multiplier,
            attack_speed_multiplier: stats.attack_speed_multiplier(),
            collision_radius: template.collision_radius,
            collision_height: template.collision_height,
            in_combat: self.is_in_combat(trap),
            alike_dead: self.is_dead(trap),
            show_summon_animation: false,
            name: if template.using_server_side_name {
                template.name.clone()
            } else {
                String::new()
            },
            title,
            pvp_flag: owner.map_or(0, |o| self.pvp_flag_of(o)),
            karma: owner.map_or(0, |o| self.karma_of_player(o)),
            abnormal_visual_effects: self.abnormal_visual_effects(trap).0,
            team: self.minigame_npc_team(trap),
        })
    }

    /// `Trap.sendInfo(player)`.
    pub(crate) fn send_trap_info(&mut self, trap: ObjectId, viewer: ObjectId) {
        if !self.is_visible_for(trap, viewer) {
            return;
        }
        if self.trap_is_triggered(trap) || self.trap_can_be_seen(trap, Some(viewer)) {
            if let Some(info) = self.trap_info(trap, Some(viewer)) {
                self.send(viewer, &info);
            }
        }
    }

    /// `Trap.checkTarget(target)`.
    fn trap_check_target(&self, trap: ObjectId, target: ObjectId) -> bool {
        let Some(state) = self.decoy_trap.traps.get(&trap) else {
            return false;
        };
        let Some(skill_key) = state.skill else {
            return false;
        };
        if !self.is_inside_radius_2d(trap, target, TRIGGER_RANGE) {
            return false;
        }
        let Some(table) = self.skill_table() else {
            return false;
        };
        let Some(skill) = table.get(skill_key.id, skill_key.level) else {
            return false;
        };
        if !self.check_for_area_offensive_skills(
            trap,
            target,
            skill,
            state.is_in_arena,
            &TargetContext::default(),
        ) {
            return false;
        }
        // observers
        let player = self.acting_player(target);
        if self.is_player(target) && self.in_observer_mode(target) {
            return false;
        }
        // olympiad own team and their summons not attacked
        if let (Some(owner), Some(player)) = (state.owner, player)
            && self.is_in_olympiad_mode(owner)
            && self.is_in_olympiad_mode(player)
            && self.olympiad_side(player) == self.olympiad_side(owner)
        {
            return false;
        }
        if state.is_in_arena {
            return true;
        }
        // trap owned by players not attack non-flagged players
        if state.owner.is_some() {
            if self.is_attackable_npc(target) {
                return true;
            }
            if player.is_none_or(|p| self.pvp_flag_of(p) == 0 && self.karma_of_player(p) == 0) {
                return false;
            }
        }
        true
    }

    /// `TrapTask.run()`.
    pub(crate) fn trap_task_run(&mut self, trap: ObjectId) {
        let Some(state) = self.decoy_trap.traps.get_mut(&trap) else {
            return;
        };
        // Fixed rate: the next run is scheduled first.
        state.task = Some(self.timers.schedule_at(
            self.tick + TimerWheel::ms_to_ticks(TICK_MS),
            TimerKind::DecoyTrap(DecoyTrapTimer::TrapTick(trap)),
        ));
        if state.triggered {
            return;
        }
        if state.has_life_time {
            state.remaining_time -= TICK_MS as i32;
            let (remaining, life_time, skill) = (state.remaining_time, state.life_time, state.skill);
            if remaining < life_time - 15000 {
                self.trap_broadcast(
                    trap,
                    &SocialAction {
                        object_id: trap.0,
                        action_id: 2,
                    },
                );
            }
            if remaining <= 0 {
                let target_type = skill.and_then(|k| {
                    self.skill_table()
                        .and_then(|t| t.get(k.id, k.level).map(|s| s.target_type))
                });
                match target_type {
                    Some(TargetType::Aura | TargetType::FrontAura | TargetType::BehindAura) => {
                        self.trap_trigger(trap, trap);
                    }
                    _ => self.trap_unsummon(trap),
                }
                return;
            }
        }
        // `World.forFirstVisibleObject(_trap, Creature.class, checkTarget, triggerTrap)`.
        let found = self
            .visible_objects(trap)
            .into_iter()
            .filter(|o| self.objects.get(o).is_some_and(|e| e.kind.is_creature()))
            .find(|o| self.trap_check_target(trap, *o));
        if let Some(target) = found {
            self.trap_trigger(trap, target);
        }
    }

    /// `Trap.triggerTrap(target)`.
    pub fn trap_trigger(&mut self, trap: ObjectId, target: ObjectId) {
        let Some(state) = self.decoy_trap.traps.get_mut(&trap) else {
            return;
        };
        if let Some(task) = state.task.take() {
            self.timers.cancel(task);
        }
        state.triggered = true;
        if let Some(info) = self.trap_info(trap, None) {
            self.trap_broadcast(trap, &info);
        }
        self.set_npc_target(trap, Some(target));
        self.trap_fire(trap, target, TrapAction::TrapTriggered);
        let due = self.tick + TimerWheel::ms_to_ticks(500);
        self.timers
            .schedule_at(due, TimerKind::DecoyTrap(DecoyTrapTimer::TrapTrigger(trap)));
    }

    /// `TrapTriggerTask.run()`: the cast, then the unsummon after the hit time and 300 ms.
    pub(crate) fn trap_trigger_task(&mut self, trap: ObjectId) {
        let Some(skill_key) = self.trap_skill(trap) else {
            self.trap_unsummon(trap);
            return;
        };
        self.do_cast(trap, skill_key);
        let hit_time = self
            .skill_table()
            .and_then(|t| t.get(skill_key.id, skill_key.level).map(|s| s.hit_time))
            .unwrap_or(0);
        let ms = u64::try_from(hit_time.saturating_add(300)).unwrap_or(300);
        let due = self.tick + TimerWheel::ms_to_ticks(ms);
        self.timers
            .schedule_at(due, TimerKind::DecoyTrap(DecoyTrapTimer::TrapUnsummon(trap)));
    }

    /// `Trap.setDetected(detector)`.
    pub fn trap_set_detected(&mut self, trap: ObjectId, detector: ObjectId) {
        let Some(state) = self.decoy_trap.traps.get(&trap) else {
            return;
        };
        let (in_arena, owner) = (state.is_in_arena, state.owner);
        let viewer = self.acting_player(detector);
        if in_arena {
            if let Some(player) = viewer {
                self.send_trap_info(trap, player);
            }
            return;
        }
        if owner.is_some_and(|o| self.pvp_flag_of(o) == 0 && self.karma_of_player(o) == 0) {
            return;
        }
        if let Some(state) = self.decoy_trap.traps.get_mut(&trap) {
            state.detected.insert(detector);
        }
        // Notify to scripts.
        self.trap_fire(trap, detector, TrapAction::TrapDetected);
        if let Some(player) = viewer {
            self.send_trap_info(trap, player);
        }
    }

    /// `Trap.unSummon()`: the task stops, the owner forgets the trap, a living trap is deleted.
    pub fn trap_unsummon(&mut self, trap: ObjectId) {
        let Some(state) = self.decoy_trap.traps.get_mut(&trap) else {
            return;
        };
        if let Some(task) = state.task.take() {
            self.timers.cancel(task);
        }
        state.owner = None;
        self.decoy_trap.trap_of.retain(|_, t| *t != trap);
        let spawned = self.objects.get(&trap).is_some_and(|e| e.spawned);
        if spawned && !self.is_dead(trap) {
            self.delete_npc(trap);
        }
    }

    /// `EventDispatcher.notifyEventAsync(new OnTrapAction(trap, trigger, action), trap)`.
    pub(crate) fn trap_fire(&mut self, trap: ObjectId, trigger: ObjectId, action: TrapAction) {
        let Some(npc) = self.npcs.get(&trap).map(|n| ScriptNpc {
            object_id: trap,
            id: n.npc_id,
        }) else {
            return;
        };
        self.script_fire_async(
            ScriptEvent::TrapAction {
                trap: npc,
                trigger,
                action,
            },
            Some(trap),
        );
    }

    /// `TrapDetect.onStart` for one target.
    pub(crate) fn trap_detect_effect(&mut self, effector: ObjectId, effected: ObjectId, power: i32) {
        if !self.is_trap(effected) || self.is_alike_dead(effected) {
            return;
        }
        let level = self.npcs.get(&effected).map_or(0, |n| n.core.level);
        if level <= power {
            self.trap_set_detected(effected, effector);
        }
    }

    /// `TrapRemove.onStart` for one target.
    pub(crate) fn trap_remove_effect(&mut self, effector: ObjectId, effected: ObjectId, power: i32) {
        if !self.is_trap(effected) {
            return;
        }
        if self.is_alike_dead(effected) {
            return;
        }
        if !self.trap_can_be_seen(effected, Some(effector)) {
            if self.is_player(effector) {
                self.send(effector, &SystemMessage::new(SystemMessageId::InvalidTarget));
            }
            return;
        }
        let level = self.npcs.get(&effected).map_or(0, |n| n.core.level);
        if level > power {
            return;
        }
        // Notify to scripts
        self.trap_fire(effected, effector, TrapAction::TrapDisarmed);
        self.trap_unsummon(effected);
        if self.is_player(effector) {
            self.send(
                effector,
                &SystemMessage::new(SystemMessageId::TheTrapDeviceHasBeenStopped),
            );
        }
    }
}

/// `getTemplate().getParameters().getObject("trap_skill", SkillHolder.class)`.
fn trap_skill_of(template: &NpcTemplate) -> Option<SkillKey> {
    match template.parameters.get("trap_skill") {
        Some(NpcParam::Skill { id, level }) => Some(SkillKey::new(*id, *level)),
        _ => None,
    }
}
