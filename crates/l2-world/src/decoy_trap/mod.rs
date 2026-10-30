//! Decoys and skill traps: the two creature kinds `SummonNpc` ("Decoy" branch) and `SummonTrap`
//! (and the script API `addTrap`) put into the world.
// Java: entity/actor/instance/{Decoy,Trap}.java
// Java: entity/actor/tasks/npc/trap/{TrapTask,TrapTriggerTask,TrapUnsummonTask}.java
// Java: dist/game/data/scripts/handlers/skill/effects/{SummonNpc,SummonTrap,TrapDetect,TrapRemove}.java
// Java: dist/game/data/scripts/handlers/actions/click/{DecoyClick,TrapClick}.java
//
// # How they sit in the world (D-17)
// Both are entries of `World::npcs` and `ObjectKind::Npc`, told apart by [`NpcClass::Decoy`] and
// [`NpcClass::Trap`] (neither can be made from a spawn: `NpcClass::from_type` answers `None`).
// What Java keeps in their fields (owner, life task, detectors...) is [`DecoyTrapWorld`]; what
// Java keeps in `Player` (`_decoy`, `_trap`) is the two owner maps.
//
// # Left out, with the reason
// - `Decoy.sendPacket` / `sendPacket(SystemMessageId)` forward to the owner: the world sends to a
//   player id only, so the messages a decoy would get (casting, damage) go nowhere.
// - `Trap.sendDamageMessage` (the damage text and the Olympiad damage count of the owner) and
//   `Trap.asPlayer()` as the owner for the skill's hostility checks: the skill is cast by the trap
//   creature itself.
// - NPC zones are inert (`zones.rs`), so `Trap._isInArena` (`PVP` and not `SIEGE`) is false
//   unless a zone check ever answers for an NPC.

mod decoy;
mod trap;

use std::collections::{BTreeSet, HashMap};

use l2_data::NpcTemplate;
use l2_util::ObjectId;
use tracing::warn;

use crate::effects::SkillKey;
use crate::npc::NpcClass;
use crate::objects::{Npc, ObjectKind};
use crate::player::CreatureCore;
use crate::timer::TimerId;
use crate::world::World;

pub use decoy::DECOY_HATE_SKILL_ID;

/// The tasks of decoys and traps (Java's `ThreadPool` tasks of the two classes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecoyTrapTimer {
    /// `Decoy`'s life task: `unSummon(owner)`.
    DecoyLife(ObjectId),
    /// `Decoy.HateSpam`, every 5 s after 2 s.
    DecoyHate(ObjectId),
    /// `TrapTask`, every second.
    TrapTick(ObjectId),
    /// `TrapTriggerTask`: the trap casts its skill 500 ms after it was triggered.
    TrapTrigger(ObjectId),
    /// `TrapUnsummonTask`: the skill's hit time plus 300 ms later.
    TrapUnsummon(ObjectId),
}

/// The fields of a `Decoy`.
#[derive(Debug, Clone, Default)]
pub(crate) struct DecoyState {
    pub(crate) owner: Option<ObjectId>,
    pub(crate) life_timer: Option<TimerId>,
    pub(crate) hate_timer: Option<TimerId>,
}

/// The fields of a `Trap`.
#[derive(Debug, Clone, Default)]
pub(crate) struct TrapState {
    /// `_owner`: none for a trap a script made (`addTrap`) and after `unSummon`.
    pub(crate) owner: Option<ObjectId>,
    /// `_skill`, the `trap_skill` parameter of the template.
    pub(crate) skill: Option<SkillKey>,
    pub(crate) has_life_time: bool,
    pub(crate) life_time: i32,
    pub(crate) remaining_time: i32,
    pub(crate) is_in_arena: bool,
    pub(crate) triggered: bool,
    /// `_playersWhoDetectedMe`.
    pub(crate) detected: BTreeSet<ObjectId>,
    /// `_trapTask`.
    pub(crate) task: Option<TimerId>,
}

/// Everything the two creature kinds keep in the world.
#[derive(Debug, Default)]
pub(crate) struct DecoyTrapWorld {
    pub(crate) decoys: HashMap<ObjectId, DecoyState>,
    /// `Player._decoy`.
    pub(crate) decoy_of: HashMap<ObjectId, ObjectId>,
    pub(crate) traps: HashMap<ObjectId, TrapState>,
    /// `Player._trap`.
    pub(crate) trap_of: HashMap<ObjectId, ObjectId>,
}

impl World {
    /// Java `Player.getDecoy()`.
    pub fn decoy_of(&self, owner: ObjectId) -> Option<ObjectId> {
        self.decoy_trap.decoy_of.get(&owner).copied()
    }

    /// Java `Player.getTrap()`.
    pub fn trap_of(&self, owner: ObjectId) -> Option<ObjectId> {
        self.decoy_trap.trap_of.get(&owner).copied()
    }

    /// Java `isTrap()`.
    pub fn is_trap(&self, id: ObjectId) -> bool {
        self.npcs.get(&id).is_some_and(|n| n.class == NpcClass::Trap)
    }

    /// Java `instanceof Decoy`.
    pub fn is_decoy(&self, id: ObjectId) -> bool {
        self.npcs.get(&id).is_some_and(|n| n.class == NpcClass::Decoy)
    }

    /// The owner of a decoy (`Decoy.getOwner()`).
    pub fn decoy_owner(&self, decoy: ObjectId) -> Option<ObjectId> {
        self.decoy_trap.decoys.get(&decoy)?.owner
    }

    /// The owner of a trap (`Trap.getOwner()`): none for a script's trap and after `unSummon`.
    pub fn trap_owner(&self, trap: ObjectId) -> Option<ObjectId> {
        self.decoy_trap.traps.get(&trap)?.owner
    }

    /// Whether a trap went off (`Trap.isTriggered()`).
    pub fn trap_is_triggered(&self, trap: ObjectId) -> bool {
        self.decoy_trap.traps.get(&trap).is_some_and(|t| t.triggered)
    }

    /// The `Creature`/`Npc` constructor part both kinds share: a new object id, the template's
    /// skills, full hit points, a table entry and the instance, ready for `spawnMe`.
    fn make_special_npc(
        &mut self,
        template: &NpcTemplate,
        class: NpcClass,
        (x, y, z): (i32, i32, i32),
        heading: i32,
        instance: i32,
    ) -> Option<ObjectId> {
        let ids = self.npc_world.ids.clone()?;
        let id = match ids.allocate() {
            Ok(id) => id,
            Err(e) => {
                warn!("Cannot make NPC {}: {e}", template.id);
                return None;
            }
        };
        let npc = Npc {
            core: CreatureCore {
                object_id: id,
                x,
                y,
                z,
                heading,
                level: i32::from(template.level),
                cur_hp: 0.0,
                max_hp: 0.0,
                cur_mp: 0.0,
                max_mp: 0.0,
                cur_cp: 0.0,
                max_cp: 0.0,
                calculators: self.npc_calculators(),
                skills: Vec::new(),
            },
            npc_id: template.id,
            class,
            spawn: None,
            leader: None,
            decayed: false,
            raid_minion: false,
            in_water: self.npc_world.is_water_at(x, y, z),
            right_hand: 0,
            left_hand: 0,
            enchant_effect: template.weapon_enchant,
            display_effect: 0,
            collision_radius: f64::from(template.collision_radius),
            must_reward_exp_sp: false,
            special_drop: false,
            movement: crate::movement::NpcMovement::default(),
            spawn_point: (0, 0, 0),
            spawn_time_ms: 0,
        };
        self.npcs.insert(id, npc);
        self.forget_effects(id);
        self.forget_status(id);
        self.add_npc_template_skills(id);
        // `fullRestore()`.
        self.init_npc_stats(id);
        self.register_object(id, ObjectKind::Npc);
        self.set_instance_id(id, instance);
        Some(id)
    }

    /// A timer of this module fired.
    pub(crate) fn on_decoy_trap_timer(&mut self, timer: DecoyTrapTimer) {
        match timer {
            DecoyTrapTimer::DecoyLife(id) => self.decoy_life_task(id),
            DecoyTrapTimer::DecoyHate(id) => self.decoy_hate_spam(id),
            DecoyTrapTimer::TrapTick(id) => self.trap_task_run(id),
            DecoyTrapTimer::TrapTrigger(id) => self.trap_trigger_task(id),
            DecoyTrapTimer::TrapUnsummon(id) => self.trap_unsummon(id),
        }
    }

    /// The NPC left the world (`Npc.onDecay`): the state of a decoy or trap goes with it.
    pub(crate) fn decoy_trap_forget(&mut self, id: ObjectId) {
        if let Some(state) = self.decoy_trap.decoys.remove(&id) {
            for timer in [state.life_timer, state.hate_timer].into_iter().flatten() {
                self.timers.cancel(timer);
            }
            self.decoy_trap.decoy_of.retain(|_, d| *d != id);
        }
        if let Some(state) = self.decoy_trap.traps.remove(&id) {
            if let Some(timer) = state.task {
                self.timers.cancel(timer);
            }
            self.decoy_trap.trap_of.retain(|_, t| *t != id);
        }
    }
}

impl World {
    /// `SummonNpc.onStart`'s default case: a `Spawn` of the NPC at the player (shifted by 20 to 50
    /// when `randomOffset`), that does not respawn and is deleted after `despawn_delay`.
    // NOTE(java-quirk): `npcCount` is only checked, one NPC is made. `Spawn.doSpawn` has no instance
    // set, so the NPC is in the normal world also when the player is in an instance. The spawn
    // animation (`isSummonSpawn`) and the NPC's title (its own name) are not modelled.
    pub(crate) fn summon_npc_effect_spawn(
        &mut self,
        player: ObjectId,
        npc_id: i32,
        despawn_delay: i32,
        random_offset: bool,
    ) {
        let Some((mut x, mut y, z)) = self.position_of(player) else {
            return;
        };
        let heading = self.heading_of(player);
        if random_offset {
            x += if self.rng().next_bool() {
                self.rng().get(20, 50)
            } else {
                self.rng().get(-50, -20)
            };
            y += if self.rng().next_bool() {
                self.rng().get(20, 50)
            } else {
                self.rng().get(-50, -20)
            };
        }
        let Some(npc) = self.script_add_spawn(npc_id, (x, y, z), heading, false, i64::from(despawn_delay), 0)
        else {
            return;
        };
        self.set_npc_summoner(npc, player);
        self.broadcast_info(npc);
    }
}
