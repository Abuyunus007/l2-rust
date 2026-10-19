//! Random animation of NPCs while they idle: the `SocialAction` a monster or an NPC plays now and
//! then, for the players who see it.
// Java: taskmanagers/RandomAnimationTaskManager.java (run, add, remove)
// Java: entity/actor/Npc.java (onRandomAnimation, hasRandomAnimation, isRandomAnimationEnabled,
//       broadcastSocialAction of Creature.java)
// Java: entity/actor/Attackable.java (hasRandomAnimation), entity/WorldRegion.java (switchAI)
//
// # Which NPCs animate
// - Java adds an NPC to the animation registry only when its region switches on
//   (`WorldRegion.switchAI(true)`), and removes it when the region switches off. An NPC that
//   spawns into a region that is already on is not added, nothing else calls `add`, so it waits
//   for the next switch. Only [`World::start_random_animation_in_region`] and
//   [`World::stop_random_animation_in_region`] add and remove.
// - `hasRandomAnimation` decides whether an entry is made at all: the config maximum (0 turns it
//   off), `randomAnimation` of the template or `disableRandomAnimation` of the spawn's `AIData`,
//   and for a non-attackable NPC not a corpse, for an attackable not a grand boss.
//
// # How the check runs
// - Java polls the whole registry once a second. Here each entry is a timer on the timer wheel
//   (`TimerKind::RandomAnimation`), due after the random interval, so an NPC is only ever looked
//   at when its check is due and nothing is polled.
// - An NPC whose region is off, or that left the world (a corpse that decayed), has no timer. Its
//   entry stays, as Java's does: a respawn comes back as the same object (`Spawn.respawnNpc`), and
//   Java's entry then animates it again when its region is on ([`World::resume_random_animation`]).
// - Stun, sleep and paralysis (the checks of Java's `RandomAnimationTaskManager.run`) are M5
//   effects; nothing has them yet.

use l2_data::npc::AiType;
use l2_protocol_game::server::SocialAction;
use l2_util::ObjectId;

use crate::npc::NpcClass;
use crate::region::RegionId;
use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::World;

/// `Npc.MINIMUM_SOCIAL_INTERVAL`: an NPC plays a social action at most once per 6 seconds.
const MINIMUM_SOCIAL_INTERVAL_MS: i64 = 6000;

/// One entry of Java's `RandomAnimationTaskManager.PENDING_ANIMATIONS`, with the NPC's own
/// `_lastSocialBroadcast` (on the object in Java, so it survives leaving the registry).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct AnimationState {
    /// The NPC is in the registry (Java: its key is in the map).
    pub(crate) pending: bool,
    /// The timer of the next check, when one is scheduled.
    pub(crate) timer: Option<TimerId>,
    /// `_lastSocialBroadcast` in world milliseconds. `None` is Java's 0, which always passes the
    /// interval check.
    pub(crate) last_social_ms: Option<i64>,
}

impl World {
    /// Java `Npc.hasRandomAnimation()`, `Attackable.hasRandomAnimation()`.
    pub(crate) fn npc_has_random_animation(&self, id: ObjectId) -> bool {
        let Some(npc) = self.npcs.get(&id) else {
            return false;
        };
        let Some(template) = self.npc_world.template(npc.npc_id) else {
            return false;
        };
        let settings = &self.npc_world.settings;
        // Java `isRandomAnimationEnabled()`: not a fake player, and the spawn's `AIData` overrides
        // the template, as `NpcPersonalAIData.initializeNpcParameters` runs after `onSpawn`.
        let enabled = match self.spawn_ai_value(id, "disableRandomAnimation") {
            Some(value) => value == 0,
            None => template.random_animation,
        };
        if !enabled || template.fake_player {
            return false;
        }
        if self.is_attackable_npc(id) {
            // Java `Attackable.hasRandomAnimation()`: no grand boss, and the subclasses that
            // override it to false (Chest and FestivalMonster here; the siege NPCs are M5).
            // NOTE(java-quirk): Chest and FestivalMonster are `NpcClass::Monster` here, so the
            // template type is what tells them apart.
            settings.max_monster_animation > 0
                && npc.class != NpcClass::GrandBoss
                && !matches!(
                    template.npc_type.as_str(),
                    "Chest"
                        | "FestivalMonster"
                        | "Defender"
                        | "FortCommander"
                        | "FortLogistics"
                        | "TerritoryWard"
                )
        } else {
            settings.max_npc_animation > 0 && template.ai_type != AiType::Corpse
        }
    }

    /// The `AIData` value of the NPC's spawn for `key` (Java `Npc.getAIValue`), if it has one.
    fn spawn_ai_value(&self, id: ObjectId, key: &str) -> Option<i32> {
        let index = self.npcs.get(&id)?.spawn?;
        let spawn = self.npc_world.spawn_data()?.spawns().get(index)?;
        spawn.ai_data.as_ref()?.get(key).copied()
    }

    /// Java `RandomAnimationTaskManager.add(npc)`: the NPC joins the registry when it has an
    /// animation, and its first check is due after a random interval (`putIfAbsent`).
    fn add_random_animation(&mut self, id: ObjectId) {
        if !self.npc_has_random_animation(id) {
            return;
        }
        self.npc_world.animations.entry(id).or_default().pending = true;
        let scheduled = self
            .npc_world
            .animations
            .get(&id)
            .is_some_and(|state| state.timer.is_some());
        if !scheduled {
            self.schedule_random_animation(id);
        }
    }

    /// Java `RandomAnimationTaskManager.remove(npc)`: the NPC leaves the registry and its check
    /// never runs. Its `_lastSocialBroadcast` is kept.
    fn remove_random_animation(&mut self, id: ObjectId) {
        let Some(state) = self.npc_world.animations.get_mut(&id) else {
            return;
        };
        state.pending = false;
        if let Some(timer) = state.timer.take() {
            self.timers.cancel(timer);
        }
    }

    /// Schedules the next check of `id`: `Rnd.get(min, max) * 1000` ms, the Monster bounds for an
    /// attackable and the NPC bounds otherwise.
    fn schedule_random_animation(&mut self, id: ObjectId) {
        let attackable = self.is_attackable_npc(id);
        let settings = &self.npc_world.settings;
        let (min, max) = if attackable {
            (settings.min_monster_animation, settings.max_monster_animation)
        } else {
            (settings.min_npc_animation, settings.max_npc_animation)
        };
        let seconds = self.rng().get(min, max);
        let ms = u64::try_from(i64::from(seconds).max(0) * 1000).unwrap_or(0);
        let due = self.tick + TimerWheel::ms_to_ticks(ms);
        let timer = self.timers.schedule_at(due, TimerKind::RandomAnimation(id));
        self.npc_world.animations.entry(id).or_default().timer = Some(timer);
    }

    /// Java `WorldRegion.switchAI(true)`, the animation part: every NPC of the region joins the
    /// registry.
    pub(crate) fn start_random_animation_in_region(&mut self, region: RegionId) {
        let npcs: Vec<ObjectId> = self
            .regions
            .objects(region)
            .filter(|id| self.npcs.contains_key(id))
            .collect();
        for id in npcs {
            self.add_random_animation(id);
        }
    }

    /// Java `WorldRegion.switchAI(false)`, the animation part: every NPC of the region leaves the
    /// registry.
    pub(crate) fn stop_random_animation_in_region(&mut self, region: RegionId) {
        let npcs: Vec<ObjectId> = self
            .regions
            .objects(region)
            .filter(|id| self.npcs.contains_key(id))
            .collect();
        for id in npcs {
            self.remove_random_animation(id);
        }
    }

    /// A respawn of `id` (the same object, Java `Spawn.respawnNpc`): if its entry is still in the
    /// registry and its region is on, the next check is scheduled again.
    pub(crate) fn resume_random_animation(&mut self, id: ObjectId) {
        let waiting = self
            .npc_world
            .animations
            .get(&id)
            .is_some_and(|state| state.pending && state.timer.is_none());
        if !waiting || !self.npc_has_random_animation(id) {
            return;
        }
        let active = self
            .objects
            .get(&id)
            .and_then(|e| e.region)
            .is_some_and(|region| self.is_region_active(region));
        if active {
            self.schedule_random_animation(id);
        }
    }

    /// The timer of `id`'s check fired: Java `RandomAnimationTaskManager.run()` for one NPC.
    pub(crate) fn on_random_animation_timer(&mut self, id: ObjectId) {
        let Some(state) = self.npc_world.animations.get_mut(&id) else {
            return;
        };
        // The timer is spent. It is scheduled again below if the NPC still animates.
        state.timer = None;
        if !state.pending {
            return;
        }
        // Java `npc.isInActiveRegion()`: an NPC out of the world, or in a region that is off,
        // has no check and no timer. Its entry stays until the region switches on.
        let active = self
            .objects
            .get(&id)
            .and_then(|e| e.region)
            .is_some_and(|region| self.is_region_active(region));
        if !active {
            return;
        }
        // Java: `!isDead() && !isInCombat() && !isMoving()` (stun, sleep and paralysis are M5).
        if !self.is_dead(id) && !self.is_in_combat(id) && !self.is_moving(id) {
            let animation = self.rng().get(2, 3);
            self.npc_on_random_animation(id, animation);
        }
        self.schedule_random_animation(id);
    }

    /// Java `Npc.onRandomAnimation(animationId)`: the social action goes out at most once per
    /// [`MINIMUM_SOCIAL_INTERVAL_MS`], and only while the neighbouring regions are on
    /// (`broadcastSocialAction`).
    pub(crate) fn npc_on_random_animation(&mut self, id: ObjectId, animation: i32) {
        let now = self.world_ms();
        // The greeting (`target.rs`) animates NPCs that are not in the registry: the throttle is
        // the NPC's either way.
        let state = self.npc_world.animations.entry(id).or_default();
        if state
            .last_social_ms
            .is_some_and(|last| now - last <= MINIMUM_SOCIAL_INTERVAL_MS)
        {
            return;
        }
        state.last_social_ms = Some(now);
        if self.are_region_neighbors_active(id) {
            self.broadcast_packet(
                id,
                &SocialAction {
                    object_id: id.0,
                    action_id: animation,
                },
            );
        }
    }
}
