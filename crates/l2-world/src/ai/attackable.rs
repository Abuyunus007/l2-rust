//! `AttackableAI`: the brain of monsters (and the other attackable NPCs): aggro, chase, attack,
//! return home, minions and the faction call.
// Java: ai/AttackableAI.java (the parts that need no skills)
// Java: ai/CreatureAI.java (the NPC side: setIntention*, notifyAction*), ai/AbstractAI.java
// Java: entity/actor/Attackable.java (aggro list: getMostHated, getHating, stopHating,
//       clearAggroList, returnHome, onSpawn), entity/actor/holders/npc/{AggroInfo,MinionList}.java
// Java: taskmanagers/AttackableThinkTaskManager.java
// Java: entity/WorldRegion.java (switchAI), entity/World.java (the AI calls of addVisibleObject
//       and switchRegion)
//
// # When does an NPC think
// - An NPC gets an AI the way Java's `getAI()` makes one: when something asks for it (a player
//   comes into view, the NPC is hit, it arrives somewhere...), see `World::wake_ai`.
// - A monster in view of a player is set to `ACTIVE` (`World::wake_npc_in_view`), which starts
//   its "AI task": it joins the think set. The set is walked once per world tick, one twentieth
//   of it each (`THINK_PERIOD_TICKS`), so every member thinks once a second like Java's
//   `AttackableThinkTaskManager` (`TASK_DELAY = 1000`) but the cost is spread over the ticks.
//   There is no timer per NPC.
// - A region that switches off (`switch_ai`) sends its attackables home and takes them out of
//   the set; an NPC that spawns in an inactive region is not put in. So only NPCs in active
//   regions are ever walked: of the ~42,000 NPCs, those near players.
//
// # Skills
// The skill branches of the think (`AISkillScope`: buff, heal, res, suicide, short and long range,
// the immobilised NPC's `tryCast` chain, `thinkCast`) are `npc_skills.rs`; the calls into it are
// at the places where Java has the branches.
//
// # Left out (each with its card)
// - Scripts: `ON_NPC_HATE`, `ON_ATTACKABLE_FACTION_CALL` and the other events (D-05);
//   `isCoreAIDisabled` is always false.
// - Fake players, the faction system, dimensional rifts, festival monsters,
//   champions. (`WalkingManager` is `walking.rs`, M8-MISC-01.)
// - The siege guards' own AIs (`SiegeGuardAI`, `FortSiegeGuardAI`): `siege_guard.rs` and
//   `fort_siege_guard.rs` (M7-SIEGE-02); the entry points here hand a defender over to them.
//
// # Guards and minions
// The `Guard` branches of the think are here (`isAggressiveTowards`, `targetReconsider`), what a
// guard adds to an `Attackable` is `guards.rs`, the life of a master's minions is `minions.rs`.

use std::collections::{BTreeSet, HashMap};

use l2_data::NpcTemplate;
use l2_data::npc::{AiType, NpcParam};
use l2_data::spawn::{NpcSpawn, SpawnTerritory};
use l2_geo::Location;
use l2_protocol_game::server::{ChangeMoveType, MoveToLocation};
use l2_util::ObjectId;

use crate::combat::AggroInfo;
use crate::movement::Intention;
use crate::objects::ObjectKind;
use crate::region::RegionId;
use crate::target::{distance_2d, distance_3d};
use crate::teleport::TeleportTarget;
use crate::world::World;

/// `AttackableThinkTaskManager.TASK_DELAY` (1000 ms) in world ticks.
pub const THINK_PERIOD_TICKS: u64 = 20;
/// `AttackableAI.RANDOM_WALK_RATE`.
const RANDOM_WALK_RATE: i32 = 30;
/// `AttackableAI.MAX_ATTACK_TIMEOUT`, in game ticks (2 minutes).
const MAX_ATTACK_TIMEOUT: i32 = 1200;
/// `AttackableAI.WANDER_ATTEMPTS`.
const WANDER_ATTEMPTS: i32 = 3;
/// `AttackableAI.MINION_LEASH_INTERVAL`, ms.
const MINION_LEASH_INTERVAL_MS: i64 = 120_000;
/// `AttackableAI.MINION_LEASH_OFFSET`.
const MINION_LEASH_OFFSET: i32 = 100;
/// `MinionList` constants.
const SPAWN_OFFSET_RADIUS: i32 = 200;
const MINIMUM_SPAWN_RADIUS: i32 = 30;
const BASE_AGGRO_AMOUNT: i64 = 1;
const MASTER_AGGRO_AMOUNT: i64 = 10;
const RAID_AGGRO_MULTIPLIER: i64 = 10;
/// "Don't call npcs spawned within the last 7 seconds."
const FACTION_CALL_SPAWN_GRACE_MS: i64 = 7000;

/// The `.ini` values the AI reads (Java `NpcConfig`). `Default` is the Java default of each key.
#[derive(Debug, Clone, PartialEq)]
pub struct AiSettings {
    /// `NpcConfig.MAX_DRIFT_RANGE` (`MaxDriftRange`).
    pub max_drift_range: i32,
    /// `AGGRO_DISTANCE_CHECK_ENABLED` (`AggroDistanceCheckEnabled`).
    pub aggro_distance_check_enabled: bool,
    /// `AGGRO_DISTANCE_CHECK_RANGE` (`AggroDistanceCheckRange`).
    pub aggro_distance_check_range: i32,
    /// `AGGRO_DISTANCE_CHECK_RAIDS` (`AggroDistanceCheckRaids`).
    pub aggro_distance_check_raids: bool,
    /// `AGGRO_DISTANCE_CHECK_RAID_RANGE` (`AggroDistanceCheckRaidRange`).
    pub aggro_distance_check_raid_range: i32,
    /// `AGGRO_DISTANCE_CHECK_INSTANCES` (`AggroDistanceCheckInstances`).
    pub aggro_distance_check_instances: bool,
    /// `AGGRO_DISTANCE_CHECK_RESTORE_LIFE` (`AggroDistanceCheckRestoreLife`).
    pub aggro_distance_check_restore_life: bool,
    /// `ALT_MOB_AGRO_IN_PEACEZONE` (`AltMobAgroInPeaceZone`).
    pub alt_mob_agro_in_peacezone: bool,
    /// `ATTACKABLES_CAMP_PLAYER_CORPSES` (`AttackablesCampPlayerCorpses`).
    pub attackables_camp_player_corpses: bool,
    /// `RAID_CHAOS_TIME`, `GRAND_CHAOS_TIME`, `MINION_CHAOS_TIME`.
    pub raid_chaos_time: i32,
    pub grand_chaos_time: i32,
    pub minion_chaos_time: i32,
    /// `GUARD_ATTACK_AGGRO_MOB` (`GuardAttackAggroMob`): guards attack aggressive monsters.
    pub guard_attack_aggro_mob: bool,
    /// `RAID_MINION_RESPAWN_TIMER` (`RaidMinionRespawnTime`), ms: how long a raid boss' minion
    /// stays dead.
    pub raid_minion_respawn_timer: i32,
    /// `MINIONS_RESPAWN_TIME` (`CustomMinionsRespawnTime`): minion NPC id -> respawn time in
    /// seconds (0: never).
    pub minions_respawn_time: HashMap<i32, i32>,
    /// `FORCE_DELETE_MINIONS` (`ForceDeleteMinions`): any master takes its minions with it.
    pub force_delete_minions: bool,
}

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            max_drift_range: 300,
            aggro_distance_check_enabled: true,
            aggro_distance_check_range: 1500,
            aggro_distance_check_raids: false,
            aggro_distance_check_raid_range: 3000,
            aggro_distance_check_instances: false,
            aggro_distance_check_restore_life: true,
            alt_mob_agro_in_peacezone: true,
            attackables_camp_player_corpses: false,
            raid_chaos_time: 10,
            grand_chaos_time: 10,
            minion_chaos_time: 10,
            guard_attack_aggro_mob: false,
            raid_minion_respawn_timer: 300_000,
            minions_respawn_time: HashMap::new(),
            force_delete_minions: false,
        }
    }
}

/// The think set (Java `AttackableThinkTaskManager.POOLS`) and the minion index.
#[derive(Debug, Default)]
pub struct AttackableWorld {
    pub(crate) settings: AiSettings,
    /// The attackables that think: bucket `n` is walked on the ticks where `tick % 20 == n`.
    buckets: [BTreeSet<ObjectId>; THINK_PERIOD_TICKS as usize],
    /// Which bucket an attackable is in.
    members: HashMap<ObjectId, usize>,
    /// Java `Monster.getMinionList().getSpawnedMinions()`, by master. A master has an entry once
    /// it had a minion (Java `hasMinions()`), see `minions.rs`.
    pub(super) minions: HashMap<ObjectId, Vec<ObjectId>>,
    /// The `MinionRespawnTask`s that wait (`minions.rs`).
    pub(super) minion_respawns: super::minions::MinionRespawns,
    /// The AI skill lists of the NPC templates, built on first use (`npc_skills.rs`).
    pub(crate) ai_skills: super::npc_skills::AiSkillCache,
}

impl AttackableWorld {
    /// How many attackables are in the think set.
    pub fn len(&self) -> usize {
        self.members.len()
    }

    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }
}

/// Is `(x, y)` inside the territory, looking at the XY plane only (Java
/// `NpcSpawnTerritory.isInsideZone(x, y)`: the shape's low Z, banned areas excluded)?
fn territory_contains_2d(territory: &SpawnTerritory, x: i32, y: i32) -> bool {
    let Some(shape) = territory.shape() else {
        return false;
    };
    let z = shape.low_z();
    if !shape.contains(x, y, z) {
        return false;
    }
    !territory
        .banned()
        .iter()
        .any(|banned| banned.as_ref().is_some_and(|b| b.contains(x, y, z)))
}

impl World {
    // ------------------------------------------------------------------ settings and lookups

    /// Installs the configuration values the AI reads.
    pub fn set_ai_settings(&mut self, settings: AiSettings) {
        self.ai.attackable.settings = settings;
    }

    /// The configuration values the AI reads.
    pub fn ai_settings(&self) -> &AiSettings {
        &self.ai.attackable.settings
    }

    /// How many attackables are in the think set (the NPCs the AI walks every second).
    pub fn thinking_npc_count(&self) -> usize {
        self.ai.attackable.len()
    }

    /// Whether `id` is in the think set.
    pub fn is_thinking_npc(&self, id: ObjectId) -> bool {
        self.ai.attackable.members.contains_key(&id)
    }

    pub(crate) fn npc_template(&self, id: ObjectId) -> Option<&NpcTemplate> {
        self.npcs.get(&id).and_then(|n| self.npc_world.template(n.npc_id))
    }

    /// Java `Creature.disableCoreAI(value)`.
    pub(crate) fn disable_core_ai(&mut self, id: ObjectId, value: bool) {
        if value {
            self.npc_world.core_ai_disabled.insert(id);
        } else {
            self.npc_world.core_ai_disabled.remove(&id);
        }
    }

    /// Java `Creature.isCoreAIDisabled()`: the AI does not pick targets or start fights. A
    /// territory ward is disabled by its constructor.
    pub(crate) fn is_core_ai_disabled(&self, id: ObjectId) -> bool {
        self.npc_world.core_ai_disabled.contains(&id)
            || self.npc_template(id).is_some_and(|t| t.is_type("TerritoryWard"))
    }

    /// Java `Npc.getSpawn()`.
    fn npc_spawn(&self, id: ObjectId) -> Option<&NpcSpawn> {
        let index = self.npcs.get(&id)?.spawn?;
        self.npc_world.spawn_data()?.spawns().get(index)
    }

    /// Java `Spawn.getSpawnTerritory()` of the NPC's spawn.
    fn npc_territory(&self, id: ObjectId) -> Option<&SpawnTerritory> {
        let name = self.npc_spawn(id)?.territory.as_deref()?;
        self.npc_world.spawn_data()?.territory(name)
    }

    pub(crate) fn dist_2d(&self, a: ObjectId, b: ObjectId) -> f64 {
        match (self.position_of(a), self.position_of(b)) {
            (Some(a), Some(b)) => distance_2d(a, b),
            _ => f64::MAX,
        }
    }

    pub(crate) fn dist_3d(&self, a: ObjectId, b: ObjectId) -> f64 {
        match (self.position_of(a), self.position_of(b)) {
            (Some(a), Some(b)) => distance_3d(a, b),
            _ => f64::MAX,
        }
    }

    /// Java `Monster.isAggressive()`.
    pub(crate) fn is_aggressive_npc(&self, id: ObjectId) -> bool {
        self.npcs.get(&id).is_some_and(|n| n.class.is_monster())
            && self.npc_template(id).is_some_and(|t| t.is_aggressive)
            && !self.is_affected(id, crate::effects::effect_flag::PASSIVE)
    }

    /// Java `Npc.getAggroRange()`: the template's, or the `aggroRange` of the spawn's `AIData`.
    fn aggro_range_of(&self, id: ObjectId) -> i32 {
        if let Some(spawn) = self.npc_spawn(id)
            && spawn.name.is_some()
            && let Some(range) = spawn.ai_data.as_ref().and_then(|d| d.get("aggroRange"))
        {
            return *range;
        }
        self.npc_template(id).map_or(0, |t| t.aggro_range)
    }

    /// Java `Npc.isRandomWalkingEnabled()`: the template's `isRandomWalkEnabled()` (the
    /// `randomWalk` flag and `MoveArounding`), unless the spawn's `AIData` says `disableRandomWalk`.
    fn is_random_walking_enabled(&self, id: ObjectId) -> bool {
        // `Npc.onSpawn`: `!WalkingManager.isTargeted(this) && ...` (a route walker never wanders).
        if self
            .npcs
            .get(&id)
            .is_some_and(|n| self.walking_is_targeted(n.npc_id))
        {
            return false;
        }
        // An instance spawn sets the flag itself (`spawnDat.setRandomWalking(...)` of
        // `Instance.spawnGroup`, M8-INST-01).
        if let Some(walk) = self
            .npcs
            .get(&id)
            .and_then(|n| n.spawn)
            .and_then(|spawn| self.instance_spawn_random_walk(spawn))
        {
            return walk;
        }
        if let Some(spawn) = self.npc_spawn(id)
            && let Some(value) = spawn.ai_data.as_ref().and_then(|d| d.get("disableRandomWalk"))
        {
            return *value == 0;
        }
        self.npc_template(id)
            .is_some_and(NpcTemplate::is_random_walk_enabled)
    }

    /// Java `Attackable.getLeader()` (`Monster._master`).
    pub(crate) fn leader_of(&self, id: ObjectId) -> Option<ObjectId> {
        self.npcs.get(&id).and_then(|n| n.leader)
    }

    /// Java `Attackable.isRaidMinion()`: set on spawn for a minion of a raid boss.
    pub(crate) fn is_raid_minion(&self, id: ObjectId) -> bool {
        self.leader_of(id)
            .is_some_and(|leader| self.npcs.get(&leader).is_some_and(|n| n.class.is_raid()))
    }

    /// Java `Attackable.isRaid()`: a raid boss, and a raid boss's minions (`setIsRaidMinion`).
    pub(crate) fn is_raid_or_minion(&self, id: ObjectId) -> bool {
        self.is_raid(id) || self.is_raid_minion(id)
    }

    /// Java `isInCombat()`: `hasAI() && (attackTarget != null || isAutoAttacking())`.
    pub fn is_in_combat(&self, id: ObjectId) -> bool {
        self.ai
            .creatures
            .get(&id)
            .is_some_and(|a| a.attack_target.is_some() || a.client_auto_attacking)
    }

    // Java `WorldObject.isInInstance()` is `World::is_in_instance` (`instance/`).

    /// `AbstractAI._target`.
    fn ai_target_of(&self, id: ObjectId) -> Option<ObjectId> {
        self.pawn_of(id).and_then(|p| p.ai_target)
    }

    pub(crate) fn set_ai_target(&mut self, id: ObjectId, target: Option<ObjectId>) {
        if let Some(pawn) = self.pawn_mut(id) {
            pawn.ai_target = target;
        }
    }

    pub(crate) fn set_attack_timeout(&mut self, id: ObjectId, value: i32) {
        if let Some(state) = self.ai.creatures.get_mut(&id) {
            state.attack_timeout = value;
        }
    }

    pub(crate) fn renew_attack_timeout(&mut self, id: ObjectId) {
        // `SiegeGuardAI.MAX_ATTACK_TIMEOUT` is 30 seconds (M7-SIEGE-02).
        let max = if self.is_siege_guard(id) {
            super::siege_guard::MAX_ATTACK_TIMEOUT
        } else {
            MAX_ATTACK_TIMEOUT
        };
        let value = max.saturating_add(self.game_ticks());
        self.set_attack_timeout(id, value);
    }

    /// Java `World.getFirstVisibleObject(npc, Player.class)` as a list: the players in the
    /// surrounding regions of `id`, same instance, in id order. Walks the online players, not the
    /// regions: there are far fewer players than objects around an NPC.
    fn players_in_view(&self, id: ObjectId) -> Vec<ObjectId> {
        let Some(region) = self.objects.get(&id).and_then(|e| e.region) else {
            return Vec::new();
        };
        let instance = self.objects.get(&id).map_or(0, |e| e.instance_id);
        let mut out: Vec<ObjectId> = self
            .players
            .keys()
            .copied()
            .filter(|player| {
                self.objects.get(player).is_some_and(|e| {
                    e.spawned && e.instance_id == instance && e.region.is_some_and(|r| region.lists(r))
                })
            })
            .collect();
        out.sort_unstable();
        out
    }

    pub(crate) fn has_visible_player(&self, id: ObjectId) -> bool {
        let Some(region) = self.objects.get(&id).and_then(|e| e.region) else {
            return false;
        };
        let instance = self.objects.get(&id).map_or(0, |e| e.instance_id);
        self.players.keys().any(|player| {
            self.objects.get(player).is_some_and(|e| {
                e.spawned && e.instance_id == instance && e.region.is_some_and(|r| region.lists(r))
            })
        })
    }

    /// The creatures `thinkActive` offers to `isAggressiveTowards`: the players in view, and for a
    /// guard that attacks aggressive monsters (`GuardAttackAggroMob`) the monsters in view.
    fn aggro_candidates(&self, id: ObjectId) -> Vec<ObjectId> {
        let mut candidates = self.players_in_view(id);
        if self.ai.attackable.settings.guard_attack_aggro_mob && self.is_guard(id) {
            candidates.extend(
                self.visible_objects(id)
                    .into_iter()
                    .filter(|o| self.is_monster(*o)),
            );
        }
        candidates
    }

    // ------------------------------------------------------------------ the AI object

    /// The `AttackableAI` constructor's `rollGlobalAggro()`.
    pub(crate) fn roll_global_aggro(&mut self, id: ObjectId) -> i32 {
        let aggressive_time =
            self.npc_template(id)
                .and_then(|t| match t.parameters.get("SetAggressiveTime") {
                    Some(NpcParam::Text(text)) => text.trim().parse::<i32>().ok(),
                    _ => None,
                });
        match aggressive_time.unwrap_or(-1) {
            0 => 0,
            // The counter is tested on the tick that raises it, so it is set one past the wanted
            // delay.
            time if time > 0 => -(time + 1),
            _ => -(self.rng().below(5) + 4),
        }
    }

    /// Java `AttackableAI.startAITask()`: the attackable joins the think set.
    pub(crate) fn start_ai_task(&mut self, id: ObjectId) {
        if self.ai.attackable.members.contains_key(&id) {
            return;
        }
        let bucket = usize::try_from(self.tick % THINK_PERIOD_TICKS).unwrap_or(0);
        self.ai.attackable.buckets[bucket].insert(id);
        self.ai.attackable.members.insert(id, bucket);
    }

    /// Java `AttackableAI.stopAITask()`.
    pub(crate) fn stop_ai_task(&mut self, id: ObjectId) {
        if let Some(bucket) = self.ai.attackable.members.remove(&id) {
            self.ai.attackable.buckets[bucket].remove(&id);
        }
        self.stop_follow(id);
    }

    /// Java `Creature.detachAI()`: the AI object is gone; the next `getAI()` makes a new one.
    pub(crate) fn detach_ai(&mut self, id: ObjectId) {
        self.stop_ai_task(id);
        self.ai.creatures.remove(&id);
        self.ai.siege_guards.remove(&id);
        if let Some(npc) = self.npcs.get_mut(&id) {
            npc.movement.pawn = crate::interact::PawnState::default();
        }
    }

    /// Forgets everything the AI keeps about an NPC that leaves the world.
    pub(crate) fn forget_npc_ai(&mut self, id: ObjectId) {
        self.stop_ai_task(id);
        self.ai.creatures.remove(&id);
        self.ai.siege_guards.remove(&id);
        self.combat.creatures.remove(&id);
        if let Some(master) = self.leader_of(id)
            && let Some(list) = self.ai.attackable.minions.get_mut(&master)
        {
            list.retain(|m| *m != id);
        }
        // Java's `MinionList` belongs to the master object and outlives its death and decay: a
        // master that respawns still has the minions that survived it. Drop the list only when
        // none of them is left in the world.
        let any_alive = self
            .ai
            .attackable
            .minions
            .get(&id)
            .is_some_and(|list| list.iter().any(|m| self.npcs.contains_key(m)));
        if !any_alive {
            self.ai.attackable.minions.remove(&id);
        }
    }

    /// Java `MinionList.onMinionSpawn`: the minion joins its master's list.
    pub(crate) fn register_minion(&mut self, master: ObjectId, minion: ObjectId) {
        let list = self.ai.attackable.minions.entry(master).or_default();
        if !list.contains(&minion) {
            list.push(minion);
        }
    }

    /// Java `Monster.getMinionList().getSpawnedMinions()`.
    pub fn minions_of(&self, master: ObjectId) -> &[ObjectId] {
        self.ai.attackable.minions.get(&master).map_or(&[], Vec::as_slice)
    }

    // ------------------------------------------------------------------ the think loop

    /// One tick of `AttackableThinkTaskManager`: a twentieth of the think set thinks.
    pub(crate) fn think_attackables(&mut self) {
        if self.ai.attackable.members.is_empty() {
            return;
        }
        let bucket = usize::try_from(self.tick % THINK_PERIOD_TICKS).unwrap_or(0);
        let ids: Vec<ObjectId> = self.ai.attackable.buckets[bucket].iter().copied().collect();
        for id in ids {
            if !self.ai.attackable.members.contains_key(&id) {
                continue;
            }
            if self.npcs.contains_key(&id) && self.has_ai(id) {
                self.notify_action_think(id);
            } else {
                // `iterator.remove()`.
                self.stop_ai_task(id);
            }
        }
    }

    /// Java `AttackableAI.notifyActionThink()`.
    pub(crate) fn attackable_notify_think(&mut self, id: ObjectId) {
        // Check if a thinking action is already in progress.
        if self.ai.creatures.get(&id).is_none_or(|a| a.thinking) {
            return;
        }
        // A siege defender runs `SiegeGuardAI` / `FortSiegeGuardAI` (`siege_guard.rs`).
        if self.is_siege_guard(id) {
            self.siege_guard_notify_think(id);
            return;
        }
        // Check if region and its neighbors are active.
        if !self.are_region_neighbors_active(id) {
            return;
        }
        // Check if the actor is all skills disabled.
        if self.is_all_skills_disabled(id) {
            return;
        }
        if let Some(state) = self.ai.creatures.get_mut(&id) {
            state.thinking = true;
        }
        // Send raid minions back to their master when they were dragged out of the master
        // territory.
        self.check_minion_leash(id);
        // Manage AI thinks of a Attackable.
        match self.intention_of(id) {
            Intention::Active => self.attackable_think_active(id),
            Intention::Attack => self.attackable_think_attack(id),
            Intention::Cast => self.attackable_think_cast(id),
            _ => {}
        }
        if let Some(state) = self.ai.creatures.get_mut(&id) {
            state.thinking = false;
        }
    }

    /// Java `isAggressiveTowards(target)`.
    fn is_aggressive_towards(&self, me: ObjectId, target: ObjectId) -> bool {
        // Check if the target isn't invulnerable (an invincible GM, or the summon of one, is
        // skipped, anybody else is attacked all the same).
        if self.is_invul(target)
            && ((self.is_player(target) && self.is_gm(target))
                || self.summon_owner(target).is_some_and(|o| self.is_gm(o)))
        {
            return false;
        }
        let Some(kind) = self.objects.get(&target).map(|e| e.kind) else {
            return false;
        };
        // Check if the target isn't a Folk or a Door.
        if kind == ObjectKind::Door {
            return false;
        }
        // `target.isPlayable()` for the range and silent move checks.
        let is_player = matches!(kind, ObjectKind::Player | ObjectKind::Summon);
        // Check if the target isn't dead, is in the Aggro range and is at the same height.
        if self.is_alike_dead(target) {
            return false;
        }
        if is_player && self.dist_3d(me, target) >= f64::from(self.aggro_range_of(me)) {
            return false;
        }
        // A player in silent move is not seen by the monsters that cannot see through it (only
        // scripts make an NPC see through, and a raid boss always does).
        if is_player
            && !self.is_raid_or_minion(me)
            && self.is_affected(target, crate::effects::effect_flag::SILENT_MOVE)
        {
            return false;
        }
        // `target.asPlayer()`: a summon's owner.
        if let Some(player) = self.acting_player(target) {
            let access_level = self.players.get(&player).map_or(0, |p| p.data.access_level);
            // Don't take the aggro if the GM has the access level that doesn't take aggro.
            if !self.data.can_take_aggro(access_level) {
                return false;
            }
            // Check if the target is within the grace period for JUST getting up from fake death.
            if self.is_recent_fake_death(player) {
                return false;
            }
            // The faction system's guards and the dimensional rift check are custom (not ported).
        }
        let class = self.npcs.get(&me).map(|n| n.class);
        if class == Some(crate::npc::NpcClass::FriendlyMob) {
            // Check if the target isn't another Npc.
            if kind == ObjectKind::Npc {
                return false;
            }
            // Check if the Player target has karma (=PK).
            if self.players.get(&target).is_some_and(|p| p.data.karma > 0) {
                return self.can_see_target(me, target); // Los Check
            }
            return false;
        }
        // Check if the actor is a GuardInstance.
        if class.is_some_and(crate::npc::NpcClass::is_guard) {
            // Check if the Player target has karma (=PK).
            if self.players.get(&target).is_some_and(|p| p.data.karma > 0) {
                return self.can_see_target(me, target); // Los Check
            }
            // Check if the Monster target is aggressive.
            // NOTE(java-quirk): the aggro range was only checked for playables above, so a
            // monster anywhere in the guard's surrounding regions counts.
            if kind == ObjectKind::Npc
                && self.is_monster(target)
                && self.ai.attackable.settings.guard_attack_aggro_mob
            {
                return self.is_aggressive_npc(target) && self.can_see_target(me, target);
            }
            return false;
        }
        if kind == ObjectKind::Npc {
            // Java: `if (target.isAttackable()) { if (!target.isAutoAttackable(me)) return false;
            // if (me.isChaos() ...) ... }` and then `if (target.isAttackable() || target
            // instanceof Npc) return false`. An attackable monster's `isAutoAttackable(attacker)`
            // is false for a monster attacker, so the chaos branch cannot be reached by the
            // classes that run this AI; every NPC target ends in `false`.
            return false;
        }
        // Depending on config, do not allow mobs to attack _new_ players in peacezones, unless
        // they are already following those players from outside the peacezone.
        if !self.ai.attackable.settings.alt_mob_agro_in_peacezone
            && self.is_inside_zone(target, crate::zones::ZoneId::Peace)
            && self.is_inside_zone(target, crate::zones::ZoneId::NoPvp)
        {
            return false;
        }
        // Champions are not rolled (`ChampionMonstersConfig`).
        // Check if the actor is Aggressive.
        self.is_aggressive_npc(me) && self.can_see_target(me, target)
    }

    /// Java `AttackableAI.shouldPromoteIdleToActive()`.
    fn should_promote_idle_to_active(&self, id: ObjectId) -> bool {
        if self.is_alike_dead(id) {
            return false;
        }
        if self.has_visible_player(id) {
            return true;
        }
        let Some(npc) = self.npcs.get(&id) else {
            return false;
        };
        if npc.spawn.is_none() {
            return false;
        }
        let (x, y, z) = (npc.core.x, npc.core.y, npc.core.z);
        match self.npc_territory(id) {
            Some(territory) => !territory_contains_2d(territory, x, y),
            None => {
                // `!npc.isInsideRadius3D(spawn.getLocation(), MAX_DRIFT_RANGE * 2)`.
                let range = f64::from(self.ai.attackable.settings.max_drift_range) * 2.0;
                distance_3d((x, y, z), npc.spawn_point) >= range
            }
        }
    }

    /// Java `AttackableAI.thinkActive()`.
    fn attackable_think_active(&mut self, id: ObjectId) {
        // Check if region and its neighbors are active.
        if !self.are_region_neighbors_active(id) {
            return;
        }
        let settings = self.ai.attackable.settings.clone();

        // Update every 1s the _globalAggro counter to come close to 0.
        let global_aggro = {
            let Some(state) = self.ai.creatures.get_mut(&id) else {
                return;
            };
            if state.global_aggro != 0 {
                if state.global_aggro < 0 {
                    state.global_aggro += 1;
                } else {
                    state.global_aggro -= 1;
                }
            }
            state.global_aggro
        };

        // Add all autoAttackable Creature in Attackable Aggro Range to its _aggroList with 0
        // damage and 1 hate. An Attackable isn't aggressive during 10s after its spawn because
        // _globalAggro is set to -10.
        if global_aggro >= 0 {
            // Only players can be aggressed (see `is_aggressive_towards`; a guard may also
            // aggress monsters), and they are far fewer than the objects around the NPC.
            let targets: Vec<ObjectId> = self
                .aggro_candidates(id)
                .into_iter()
                .filter(|target| self.is_aggressive_towards(id, *target))
                .collect();
            for target in targets {
                // D-05: `ON_NPC_HATE` can terminate the aggression here.
                if self.aggro_hating(id, target) == 0 {
                    self.add_damage_hate(id, target, 0, 0);
                }
            }

            // Chose a target from its aggroList.
            // A confused NPC takes its attack target instead.
            let hated = if self.is_confused(id) {
                self.attack_target_of(id)
            } else {
                self.aggro_most_hated(id)
            };

            // Order to the Attackable to attack the target (`!npc.isCoreAIDisabled()`).
            if let Some(hated) = hated.filter(|_| !self.is_core_ai_disabled(id)) {
                // Get the hate level of the Attackable against this Creature target contained in
                // _aggroList.
                let aggro = self.aggro_hating(id, hated);
                let global_aggro = self.ai.creatures.get(&id).map_or(0, |a| a.global_aggro);
                if aggro + i64::from(global_aggro) > 0 {
                    // Set the Creature movement type to run and send Server->Client packet
                    // ChangeMoveType to all others Player.
                    if !self.is_running(id) {
                        self.set_running(id, true);
                    }
                    // Set the AI Intention to ATTACK.
                    self.set_intention_attack(id, hated);
                }
                return;
            }
        }

        // Chance to forget attackers after some time.
        let full = self
            .core_of(id)
            .is_some_and(|c| c.cur_hp == c.max_hp && c.cur_mp == c.max_mp);
        if full && !self.attack_by_list(id).is_empty() && self.rng().below(500) == 0 {
            self.aggro_clear(id);
            self.attack_by_clear(id);
        }

        // Check if the mob should not return to spawn point.
        if !self.can_return_to_spawn_point(id) {
            return;
        }

        // Order this attackable to return to its spawn because there's no target to attack.
        let target = self.ai_target_of(id);
        if self.npcs.get(&id).is_some_and(|n| n.spawn.is_some()) {
            let away = match self.npc_territory(id) {
                Some(territory) => self
                    .position_of(id)
                    .is_some_and(|(x, y, _)| !territory_contains_2d(territory, x, y)),
                None => {
                    let spawn_point = self.npcs.get(&id).map_or((0, 0, 0), |n| n.spawn_point);
                    self.position_of(id)
                        .is_some_and(|p| distance_2d(p, spawn_point) > f64::from(settings.max_drift_range))
                }
            };
            let target_gone = match target {
                None => true,
                Some(t) => {
                    self.objects.get(&t).is_some_and(|e| e.invisible)
                        || (self.is_player(t)
                            && !settings.attackables_camp_player_corpses
                            && self.is_alike_dead(t))
                }
            };
            if !self.is_walker(id) && away && target_gone {
                self.set_running(id, false);
                self.return_home(id);
                return;
            }
        }

        // Do not leave dead player.
        if let Some(t) = target
            && self.is_player(t)
            && self.is_alike_dead(t)
        {
            return;
        }

        // Minions following leader.
        let leader = self.leader_of(id).filter(|l| !self.is_alike_dead(*l));
        if let Some(leader) = leader {
            let min_radius = 30;
            let offset = if self.is_raid_minion(id) {
                500 // For Raids.
            } else {
                200 // For normal minions.
            };
            let leader_running = self.is_running(leader);
            self.set_running(id, leader_running);
            if self.dist_2d(id, leader) > f64::from(offset) {
                let mut x1 = self.rng().get(min_radius * 2, offset * 2); // x
                let mut y1 = self.rng().get(x1, offset * 2); // distance
                y1 = f64::from(y1 * y1 - x1 * x1).sqrt() as i32; // y
                let Some((lx, ly, lz)) = self.position_of(leader) else {
                    return;
                };
                x1 = if x1 > offset + min_radius {
                    (lx + x1) - offset
                } else {
                    (lx - x1) + min_radius
                };
                y1 = if y1 > offset + min_radius {
                    (ly + y1) - offset
                } else {
                    (ly - y1) + min_radius
                };
                // Move the actor to Location (x,y,z) server side AND client side by sending
                // Server->Client packet MoveToLocation (broadcast).
                self.ai_move_to(id, x1, y1, lz);
                return;
            }
            if self.rng().below(RANDOM_WALK_RATE) == 0 {
                // The template's BUFF skills: the first one that is not on yet is cast.
                self.think_active_buffs(id);
            }
        }
        // Order to the Monster to random walk (1/100).
        else if self.npcs.get(&id).is_some_and(|n| n.spawn.is_some())
            && self.rng().below(RANDOM_WALK_RATE) == 0
            && self.is_random_walking_enabled(id)
        {
            // The template's BUFF skills are cast here instead of walking.
            if self.think_active_buffs(id) {
                return;
            }
            self.random_walk(id, &settings);
        }
    }

    /// The end of `thinkActive`: a step around the spawn point.
    // NOTE(java-quirk): Java's `int` products wrap, and the sqrt of a negative value is NaN, which
    // `(int)` makes 0. `wrapping_*` gives the same value; a plain `*` would panic in a debug build
    // once MaxDriftRange is above ~23000.
    fn random_walk(&mut self, id: ObjectId, settings: &AiSettings) {
        let drift = settings.max_drift_range;
        let Some((x, y, z)) = self.position_of(id) else {
            return;
        };
        let instance = self.objects.get(&id).map_or(0, |e| e.instance_id);
        let flying = self.npc_template(id).is_some_and(|t| t.flying);
        let has_territory = self.npc_territory(id).is_some();
        if has_territory {
            // Near a territory border most offsets fall outside, so give the roll a few attempts
            // before giving up.
            for _ in 0..WANDER_ATTEMPTS {
                let delta_x = self.rng().below(drift * 2); // x
                let mut delta_y = self.rng().get(delta_x, drift * 2); // distance
                delta_y = f64::from(
                    delta_y
                        .wrapping_mul(delta_y)
                        .wrapping_sub(delta_x.wrapping_mul(delta_x)),
                )
                .sqrt() as i32; // y
                let x2 = (delta_x + x) - drift;
                let y2 = (delta_y + y) - drift;
                let inside = |world: &World, px: i32, py: i32| {
                    world
                        .npc_territory(id)
                        .is_some_and(|t| territory_contains_2d(t, px, py))
                };
                if !inside(self, x2, y2) {
                    continue;
                }
                // Move the actor to Location (x,y,z) server side AND client side by sending
                // Server->Client packet MoveToLocation (broadcast).
                let to = if flying {
                    Location::new(x2, y2, z)
                } else {
                    self.geo_valid_location(Location::new(x, y, z), Location::new(x2, y2, z), instance)
                };
                if inside(self, to.x, to.y) {
                    self.ai_move_to(id, to.x, to.y, to.z);
                    break;
                }
            }
            return;
        }

        let (mut x1, mut y1, mut z1) = self.npcs.get(&id).map_or((0, 0, 0), |n| n.spawn_point);
        // `npc.isInsideRadius2D(x1, y1, 0, MAX_DRIFT_RANGE)`.
        if distance_2d((x, y, z), (x1, y1, 0)) < f64::from(drift) {
            let delta_x = self.rng().below(drift * 2); // x
            let mut delta_y = self.rng().get(delta_x, drift * 2); // distance
            delta_y = f64::from(
                delta_y
                    .wrapping_mul(delta_y)
                    .wrapping_sub(delta_x.wrapping_mul(delta_x)),
            )
            .sqrt() as i32; // y
            x1 = (delta_x + x1) - drift;
            y1 = (delta_y + y1) - drift;
            z1 = z;
        }
        // Move the actor to Location (x,y,z) server side AND client side by sending
        // Server->Client packet MoveToLocation (broadcast).
        let to = if flying {
            Location::new(x1, y1, z1)
        } else {
            self.geo_valid_location(Location::new(x, y, z), Location::new(x1, y1, z1), instance)
        };
        let spawn_point = self.npcs.get(&id).map_or((0, 0, 0), |n| n.spawn_point);
        if distance_2d(spawn_point, (to.x, to.y, to.z)) <= f64::from(drift) {
            self.ai_move_to(id, to.x, to.y, to.z);
        }
    }

    /// Java `AttackableAI.hasStrayedFromSpawn(npc, spawn)`.
    fn has_strayed_from_spawn(&self, id: ObjectId) -> bool {
        let Some(spawn) = self.npc_spawn(id) else {
            return false;
        };
        let settings = &self.ai.attackable.settings;
        let range = if spawn.chase_range > 0 {
            settings.max_drift_range.max(spawn.chase_range)
        } else if self.is_raid_or_minion(id) {
            settings.aggro_distance_check_raid_range
        } else {
            settings.aggro_distance_check_range
        };
        let Some(npc) = self.npcs.get(&id) else {
            return false;
        };
        let out_of_range =
            distance_2d((npc.core.x, npc.core.y, npc.core.z), npc.spawn_point) > f64::from(range);
        match self.npc_territory(id) {
            // Leaving the territory alone is not straying, the chase range still applies.
            Some(territory) => out_of_range && !territory_contains_2d(territory, npc.core.x, npc.core.y),
            None => out_of_range,
        }
    }

    /// Java `AttackableAI.thinkAttack()`.
    fn attackable_think_attack(&mut self, id: ObjectId) {
        // `DistrustAI`: the monster fights the creature it was turned against.
        if let Some(forced) = self.control(id).and_then(|c| c.distrust) {
            self.distrust_think_attack(id, forced);
            return;
        }
        if self.is_casting_now(id) {
            return;
        }
        let settings = self.ai.attackable.settings.clone();
        let Some(class) = self.npcs.get(&id).map(|n| n.class) else {
            return;
        };

        // (`GrandBoss` is exempt.)
        if class.is_monster()
            && !self.is_walker(id)
            && class != crate::npc::NpcClass::GrandBoss
            && (if self.is_raid_or_minion(id) {
                settings.aggro_distance_check_raids
            } else {
                settings.aggro_distance_check_enabled
            })
            && self.npcs.get(&id).is_some_and(|n| n.spawn.is_some())
            && self.has_strayed_from_spawn(id)
            && (settings.aggro_distance_check_instances || !self.is_in_instance(id))
        {
            self.return_stray_monster(id, &settings);
            return;
        }

        if self.is_core_ai_disabled(id) {
            return;
        }

        let Some(mut most_hate) = self.aggro_most_hated(id) else {
            self.set_intention_active(id);
            return;
        };

        if self.attack_target_of(id) != Some(most_hate) {
            self.set_attack_target(id, Some(most_hate));
        }
        if self.ai_target_of(id) != Some(most_hate) {
            self.set_ai_target(id, Some(most_hate));
        }

        // Immobilize condition
        if self.is_movement_disabled(id) {
            self.movement_disable(id);
            return;
        }

        // Check if target is dead or if timeout is expired to stop this attack.
        if self.is_alike_dead(most_hate) {
            // Stop hating this target after the attack timeout or if target is dead.
            self.aggro_stop_hating(id, most_hate);
            return;
        }

        let timeout = self.ai.creatures.get(&id).map_or(i32::MAX, |a| a.attack_timeout);
        if timeout < self.game_ticks() {
            // Remember the combat state, the intention change below clears it.
            let was_in_combat = self.is_in_combat(id);
            // Set the AI Intention to ACTIVE.
            self.set_intention_active(id);
            // Lose target and walk to spawn.
            self.set_ai_target(id, None);
            self.aggro_clear(id);
            self.attack_by_clear(id);
            self.set_running(id, false);
            // Monster teleport to spawn.
            if class.is_monster()
                && self.npcs.get(&id).is_some_and(|n| n.spawn.is_some())
                && !self.is_in_instance(id)
                && (was_in_combat || !self.has_visible_player(id))
            {
                self.teleport_to_spawn(id, false);
            }
            return;
        }

        // Actor should be able to see target.
        if !self.can_see_target(id, most_hate) {
            if self.dist_3d(id, most_hate) < 6000.0
                && let Some((x, y, z)) = self.position_of(most_hate)
            {
                self.ai_move_to(id, x, y, z);
            }
            return;
        }

        // The SUICIDE skills (below 30% HP).
        let ai_skills = self.ai_skills_of(id);
        if let Some(skills) = &ai_skills
            && self.think_attack_suicide(id, most_hate, skills)
        {
            return;
        }

        // In case many mobs are trying to hit from same place, move a bit, circling around the
        // target.
        // Note from Gnacik: on l2js because of that sometimes mobs don't attack player only
        // running around player without any sense, so decrease chance for now.
        let collision = self.npc_template(id).map_or(0, NpcTemplate::collision_radius_int);
        let combined_collision = collision + self.collision_radius_of(most_hate);
        if !self.is_movement_disabled(id) && self.rng().below(100) <= 3 {
            let (x, y, z) = self.position_of(id).unwrap_or_default();
            let crowded = self.visible_objects(id).into_iter().any(|nearby| {
                nearby != most_hate
                    && self.is_attackable_npc(nearby)
                    && self
                        .position_of(nearby)
                        .is_some_and(|p| distance_2d((x, y, z), p) < f64::from(collision))
            });
            if crowded {
                let (mx, my, _) = self.position_of(most_hate).unwrap_or_default();
                let mut new_x = combined_collision + self.rng().below(40);
                new_x = if self.rng().next_bool() {
                    mx + new_x
                } else {
                    mx - new_x
                };
                let mut new_y = combined_collision + self.rng().below(40);
                new_y = if self.rng().next_bool() {
                    my + new_y
                } else {
                    my - new_y
                };
                if distance_2d((x, y, z), (new_x, new_y, 0)) >= f64::from(collision) {
                    let new_z = z + 30;
                    // Mobius: Verify destination. Prevents wall collision issues and fixes
                    // monsters not avoiding obstacles.
                    let instance = self.objects.get(&id).map_or(0, |e| e.instance_id);
                    let to = self.geo_valid_location(
                        Location::new(x, y, z),
                        Location::new(new_x, new_y, new_z),
                        instance,
                    );
                    self.ai_move_to(id, to.x, to.y, to.z);
                }
                return;
            }
        }

        let ai_type = self.npc_template(id).map(|t| t.ai_type);

        // Calculate Archer movement.
        if !self.is_movement_disabled(id) && ai_type == Some(AiType::Archer) && self.rng().below(100) < 15 {
            let distance = self.dist_2d(id, most_hate);
            if distance <= f64::from(60 + combined_collision) {
                let (x, y, z) = self.position_of(id).unwrap_or_default();
                let (mx, my, _) = self.position_of(most_hate).unwrap_or_default();
                let pos_x = if mx < x { x + 300 } else { x - 300 };
                let pos_y = if my < y { y + 300 } else { y - 300 };
                let pos_z = z + 30;
                let instance = self.objects.get(&id).map_or(0, |e| e.instance_id);
                if self.geo_can_move_to_target(
                    Location::new(x, y, z),
                    Location::new(pos_x, pos_y, pos_z),
                    instance,
                ) {
                    self.npc_set_intention_move_to(id, pos_x, pos_y, pos_z);
                }
                return;
            }
        }

        // BOSS/Raid Minion Target Reconsider.
        let mut reconsidered_none = false;
        if self.is_raid_or_minion(id) {
            let chaos_time = {
                let Some(state) = self.ai.creatures.get_mut(&id) else {
                    return;
                };
                state.chaos_time += 1;
                state.chaos_time
            };
            let (hp, max_hp) = self.core_of(id).map_or((0.0, 1.0), |c| {
                (c.cur_hp, f64::from(self.stats(id).map_or(1, |s| s.max_hp())))
            });
            let mut change_target = false;
            if class == crate::npc::NpcClass::RaidBoss && chaos_time > settings.raid_chaos_time {
                let multiplier = if self.minions_of(id).is_empty() {
                    100.0
                } else {
                    200.0
                };
                change_target = f64::from(self.rng().below(100)) <= 100.0 - ((hp * multiplier) / max_hp);
            } else if class == crate::npc::NpcClass::GrandBoss && chaos_time > settings.grand_chaos_time {
                let chaos_rate = 100.0 - ((hp * 300.0) / max_hp);
                change_target = (chaos_rate <= 10.0 && self.rng().below(100) <= 10)
                    || (chaos_rate > 10.0 && f64::from(self.rng().below(100)) <= chaos_rate);
            } else if chaos_time > settings.minion_chaos_time {
                change_target = f64::from(self.rng().below(100)) <= 100.0 - ((hp * 200.0) / max_hp);
            }
            if change_target {
                match self.target_reconsider_random(id, true) {
                    Some(new_target) => {
                        self.set_attack_target(id, Some(new_target));
                        self.set_ai_target(id, Some(new_target));
                        if let Some(state) = self.ai.creatures.get_mut(&id) {
                            state.chaos_time = 0;
                        }
                        return;
                    }
                    None => reconsidered_none = true,
                }
            }
        }

        if reconsidered_none {
            // Java assigns the null result to `mostHate`: the next block looks for a target.
            let Some(new_target) = self.target_reconsider_random(id, false) else {
                return;
            };
            most_hate = new_target;
            self.set_attack_target(id, Some(most_hate));
            self.set_ai_target(id, Some(most_hate));
        }

        // Cast skills.
        if let Some(skills) = &ai_skills
            && (!self.is_moving(id) || ai_type == Some(AiType::Mage))
            && self.think_attack_skills(id, most_hate, collision, skills)
        {
            return;
        }

        // Check if target is within range or move.
        let mut range = self.stats(id).map_or(0, |s| s.physical_attack_range()) + combined_collision;
        if self.is_moving(id) {
            range *= 2;
        }
        if ai_type == Some(AiType::Archer) {
            range = 850 + combined_collision; // Base bow range for NPCs.
        }

        if self.dist_2d(id, most_hate) > f64::from(range) {
            if self.check_target(id, most_hate) {
                self.move_to_pawn(id, most_hate, range);
                return;
            }
            let Some(new_target) = self.target_reconsider_random(id, false) else {
                return;
            };
            most_hate = new_target;
            self.set_attack_target(id, Some(most_hate));
            self.set_ai_target(id, Some(most_hate));
        }

        // Attacks target
        if let Some(target) = self.attack_target_of(id) {
            self.do_attack(id, target);
        }
    }

    /// The first branch of `thinkAttack`: the monster went further from its spawn than it may.
    fn return_stray_monster(&mut self, id: ObjectId, settings: &AiSettings) {
        let Some(npc) = self.npcs.get(&id) else {
            return;
        };
        let spawn_point = npc.spawn_point;
        let heading = self.npc_spawn(id).map_or(0, |s| s.heading);
        let minions: Vec<ObjectId> = self.minions_of(id).to_vec();
        let send_home = |world: &mut World, creature: ObjectId, random_offset: bool| {
            if settings.aggro_distance_check_restore_life {
                let (max_hp, max_mp) = world.stats(creature).map_or((0, 0), |s| (s.max_hp(), s.max_mp()));
                world.set_current_hp(creature, f64::from(max_hp), true);
                world.set_current_mp(creature, f64::from(max_mp), true);
            }
            world.abort_attack(creature);
            world.aggro_clear(creature);
            world.attack_by_clear(creature);
            if world.has_ai(creature) {
                world.npc_set_intention_move_to(creature, spawn_point.0, spawn_point.1, spawn_point.2);
            } else {
                world.teleport_to_point(creature, spawn_point, heading, random_offset);
            }
        };
        send_home(self, id, true);
        // Minions should return as well.
        for minion in minions {
            send_home(self, minion, true);
        }
    }

    /// Java `Creature.teleToLocation(spawn, randomOffset)` of an NPC with a spawn.
    fn teleport_to_spawn(&mut self, id: ObjectId, random_offset: bool) {
        let Some(npc) = self.npcs.get(&id) else {
            return;
        };
        let point = npc.spawn_point;
        let heading = self.npc_spawn(id).map_or(0, |s| s.heading);
        self.teleport_to_point(id, point, heading, random_offset);
    }

    fn teleport_to_point(&mut self, id: ObjectId, point: (i32, i32, i32), heading: i32, random_offset: bool) {
        let offset = if random_offset {
            self.settings.teleport.max_offset_on_teleport
        } else {
            0
        };
        self.tele_to_location(
            id,
            TeleportTarget::at(point.0, point.1, point.2)
                .with_heading(heading)
                .with_instance(0)
                .with_random_offset(offset),
        );
    }

    /// Java `Creature.updatePosition`: "Prevent attackables teleporting to another ground layer
    /// while moving."
    pub(crate) fn attackable_back_to_spawn(&mut self, id: ObjectId) {
        if self.npcs.get(&id).is_none_or(|n| n.spawn.is_none()) {
            return;
        }
        self.teleport_to_spawn(id, false);
        self.attack_by_clear(id);
        self.aggro_clear(id);
        self.wake_ai(id);
        self.set_intention_idle(id);
    }

    /// Java `AttackableAI.checkTarget(target)`.
    fn check_target(&self, id: ObjectId, target: ObjectId) -> bool {
        let Some(kind) = self.objects.get(&target).map(|e| e.kind) else {
            return false;
        };
        if kind.is_creature() {
            if self.is_dead(target) {
                return false;
            }
            if self.is_movement_disabled(id) {
                let range = self.stats(id).map_or(0, |s| s.physical_attack_range())
                    + self.npc_template(id).map_or(0, NpcTemplate::collision_radius_int)
                    + self.collision_radius_of(target);
                if self.dist_2d(id, target) >= f64::from(range) {
                    return false;
                }
                if !self.can_see_target(id, target) {
                    return false;
                }
            }
            // `target.isAutoAttackable(npc)`: a monster attacks players (not other monsters or
            // NPCs), a guard players with karma and monsters, a monster guards.
            // (A door is not attackable by a monster.)
            if !matches!(kind, ObjectKind::Player | ObjectKind::Npc | ObjectKind::Summon)
                || !self.is_auto_attackable_for(target, id)
            {
                return false;
            }
        }
        // Fixes monsters not avoiding obstacles.
        true
    }

    /// Java `AttackableAI.targetReconsider(randomTarget)`.
    fn target_reconsider_random(&mut self, id: ObjectId, random_target: bool) -> Option<ObjectId> {
        if random_target {
            let mut result: Vec<ObjectId> = Vec::new();
            let attackers: Vec<ObjectId> = self.aggro_list(id).iter().map(|a| a.attacker).collect();
            for attacker in attackers {
                if self.check_target(id, attacker) {
                    result.push(attacker);
                }
            }
            // If npc is aggressive, add characters within aggro range too.
            if self.is_aggressive_npc(id) {
                let range = self.aggro_range_of(id);
                for creature in self.visible_objects_in_range(id, range) {
                    if self.check_target(id, creature) {
                        result.push(creature);
                    }
                }
            }
            if !result.is_empty() {
                let index =
                    usize::try_from(self.rng().below(i32::try_from(result.len()).unwrap_or(1))).unwrap_or(0);
                return result.get(index).copied();
            }
        }

        // The hate-ordered pick of Java: the attacker with the highest hate that passes
        // `checkTarget`.
        let picked = self
            .aggro_list(id)
            .iter()
            .any(|aggro| aggro.hate > i64::MIN && self.check_target(id, aggro.attacker));
        if !picked && self.is_aggressive_npc(id) {
            let range = self.aggro_range_of(id);
            let found = self
                .visible_objects_in_range(id, range)
                .into_iter()
                .find(|nearby| self.check_target(id, *nearby));
            if found.is_some() {
                return found;
            }
        }
        // NOTE(java-quirk): Java ends with `return null;`, not `return creature;`: the
        // hate-ordered pick above is computed and dropped, only the aggressive-range match is
        // ever returned.
        None
    }

    /// Java `AttackableAI.movementDisable()` (without its skill branches).
    fn movement_disable(&mut self, id: ObjectId) {
        let Some(target) = self.attack_target_of(id) else {
            return;
        };
        // The Creature's own target is the one skills aim at.
        if self.npc_cast_target(id).is_none() {
            self.set_npc_target(id, Some(target));
        }
        let dist = self.dist_2d(id, target);

        // TODO(Zoey76): Review this "magic changes".
        let random = self.rng().below(100);
        // IMMOBILIZE (15), COT (20), DEBUFF (30), NEGATIVE (40), ATTACK (movement disabled, MAGE
        // or HEALER) and UNIVERSAL skills are tried here.
        if self.movement_disable_skills(id, target, random, dist) {
            return;
        }

        // If cannot cast, try to attack.
        let range = self.stats(id).map_or(0, |s| s.physical_attack_range())
            + self.npc_template(id).map_or(0, NpcTemplate::collision_radius_int)
            + self.collision_radius_of(target);
        if dist <= f64::from(range) && self.can_see_target(id, target) {
            self.do_attack(id, target);
            return;
        }

        // If cannot cast nor attack, find a new target.
        self.target_reconsider_hate_list(id);
    }

    /// Java `AttackableAI.targetReconsider()` (no argument): the most hated creature in sight and
    /// in reach becomes the target again.
    fn target_reconsider_hate_list(&mut self, id: ObjectId) {
        let most_hate = self.aggro_most_hated(id);
        if let Some(list) = self.aggro_hate_list(id) {
            for obj in list {
                if !self.can_see_target(id, obj) || self.is_dead(obj) || Some(obj) != most_hate || obj == id {
                    continue;
                }
                let dist = self.dist_2d(id, obj);
                let collision = self.npc_template(id).map_or(0, NpcTemplate::collision_radius_int);
                let mut dist2 = dist - f64::from(collision);
                let range = self.stats(id).map_or(0, |s| s.physical_attack_range())
                    + collision
                    + self.collision_radius_of(obj);
                if self.is_moving(obj) {
                    dist2 -= 70.0;
                }
                if dist2 <= f64::from(range) {
                    let hate = most_hate.map_or(2000, |m| self.aggro_hating(id, m));
                    self.add_damage_hate(id, obj, 0, hate);
                    self.set_attack_target(id, Some(obj));
                    return;
                }
            }
        }
        if self.is_guard(id) {
            // `if (!(actor instanceof Guard))`: a guard only picks from its hate list.
            return;
        }
        let candidates = self.visible_objects(id);
        let current = self.attack_target_of(id);
        for obj in candidates {
            if !self.can_see_target(id, obj)
                || self.is_dead(obj)
                || Some(obj) != most_hate
                || obj == id
                || Some(obj) == current
            {
                continue;
            }
            // Players and summons; attackable targets: monsters do not attack each other here
            // (chaos is not reachable, see `is_aggressive_towards`).
            if self.is_player(obj) || self.is_summon(obj) {
                let hate = most_hate.map_or(2000, |m| self.aggro_hating(id, m));
                self.add_damage_hate(id, obj, 0, hate);
                self.set_attack_target(id, Some(obj));
            }
        }
    }

    /// Java `AttackableAI.checkMinionLeash()`: a raid minion that was dragged out of its
    /// master's territory is sent back to its master every 2 minutes.
    fn check_minion_leash(&mut self, id: ObjectId) {
        if !self.is_raid_minion(id) || self.is_dead(id) || self.is_moving(id) {
            return;
        }
        let now = self.world_ms();
        let Some(state) = self.ai.creatures.get_mut(&id) else {
            return;
        };
        if state.minion_leash_ms == 0 {
            state.minion_leash_ms = now + MINION_LEASH_INTERVAL_MS;
            return;
        }
        if state.minion_leash_ms > now {
            return;
        }
        state.minion_leash_ms = now + MINION_LEASH_INTERVAL_MS;
        let Some(master) = self.leader_of(id) else {
            return;
        };
        if self.is_dead(master) {
            return;
        }
        // `isInsideMasterTerritory`: the master's spawn territory, else its boss zone (boss
        // zones are not loaded: "no territory could be resolved" is inside).
        let inside = match (self.position_of(id), self.npc_territory(master)) {
            (Some((x, y, z)), Some(territory)) => {
                territory.shape().is_some_and(|shape| shape.contains(x, y, z))
            }
            _ => true,
        };
        if inside {
            return;
        }
        self.aggro_clear(id);
        self.attack_by_clear(id);
        if let Some((mx, my, mz)) = self.position_of(master) {
            self.tele_to_location(
                id,
                TeleportTarget::at(mx, my, mz).with_random_offset(MINION_LEASH_OFFSET),
            );
        }
    }

    // ------------------------------------------------------------------ intentions

    /// Java `AttackableAI.setIntentionIdle()`.
    pub(crate) fn attackable_set_intention_idle(&mut self, id: ObjectId) {
        // `SiegeGuardAI.setIntentionIdle` promotes on a visible player only (M7-SIEGE-02).
        if self.is_siege_guard(id) {
            self.siege_guard_set_intention_idle(id);
            return;
        }
        if self.should_promote_idle_to_active(id) {
            self.set_intention_active(id);
            return;
        }
        self.creature_set_intention_idle(id);
        // Stop AI task and detach AI from NPC.
        self.stop_ai_task(id);
        // Cancel the AI
        self.detach_ai(id);
    }

    /// Java `AttackableAI.setIntentionActive()`.
    pub(crate) fn attackable_set_intention_active(&mut self, id: ObjectId) {
        // Cancel attack timeout (`SiegeGuardAI.setIntentionActive` keeps it).
        if !self.is_siege_guard(id) {
            self.set_attack_timeout(id, i32::MAX);
        }
        self.creature_set_intention_active(id);
        self.start_ai_task(id);
    }

    /// Java `CreatureAI.setIntentionMoveTo(loc)` of an NPC (`AttackableAI` starts its task after).
    pub(crate) fn npc_set_intention_move_to(&mut self, id: ObjectId, x: i32, y: i32, z: i32) {
        self.wake_ai(id);
        self.stop_follow(id);
        if self.intention_of(id) != Intention::Rest
            && !self.is_all_skills_disabled(id)
            && !self.is_casting_now_for_effects(id)
        {
            // Set the Intention of this AbstractAI to MOVE_TO.
            self.set_intention_value(id, Intention::MoveTo);
            // Stop the actor auto-attack client side by sending Server->Client packet
            // AutoAttackStop (broadcast).
            self.client_stop_auto_attack(id);
            // Abort the attack of the Creature and send Server->Client ActionFailed packet.
            self.abort_attack(id);
            // Move the actor to Location (x,y,z) server side AND client side by sending
            // Server->Client packet MoveToLocation (broadcast).
            self.ai_move_to(id, x, y, z);
        }
        if self.is_attackable_npc(id) {
            self.start_ai_task(id);
        }
    }

    /// Java `AttackableAI.notifyActionReadyToAct()`'s first lines.
    pub(crate) fn attackable_ready_to_act(&mut self, id: ObjectId) {
        // A landed attack keeps the fight alive, the timeout is only for targets that cannot be
        // reached.
        // `SiegeGuardAI` keeps `CreatureAI`'s (M7-SIEGE-02).
        if self.is_attackable_npc(id)
            && !self.is_siege_guard(id)
            && self.intention_of(id) == Intention::Attack
        {
            self.renew_attack_timeout(id);
        }
    }

    /// Java `Attackable.returnHome()`.
    pub fn return_home(&mut self, id: ObjectId) {
        // `Defender.returnHome` and `FortCommander.returnHome` replace it.
        if self.defender_return_home(id) {
            return;
        }
        self.aggro_clear(id);
        if self.has_ai(id)
            && let Some(npc) = self.npcs.get(&id)
            && npc.spawn.is_some()
        {
            let (x, y, z) = npc.spawn_point;
            self.npc_set_intention_move_to(id, x, y, z);
        }
    }

    /// Java `Creature.setRunning(value)` of an NPC: `ChangeMoveType` to those who see it, then
    /// their `NpcInfo` again.
    pub(crate) fn set_running(&mut self, id: ObjectId, value: bool) {
        let Some(npc) = self.npcs.get_mut(&id) else {
            return;
        };
        if npc.movement.running == value {
            return;
        }
        npc.movement.running = value;
        if self.stats(id).is_some_and(|s| s.run_speed() != 0.0) {
            self.broadcast_creature_packet(
                id,
                &ChangeMoveType {
                    object_id: id.0,
                    running: value,
                },
            );
        }
        self.broadcast_info(id);
    }

    // ------------------------------------------------------------------ events

    /// Java `AttackableAI.notifyActionAttacked(attacker)` (and `CreatureAI`'s, for the other
    /// NPCs).
    pub(crate) fn npc_notify_attacked(&mut self, id: ObjectId, attacker: ObjectId) {
        if self.is_attackable_npc(id) && self.objects.get(&attacker).is_some_and(|e| e.kind.is_creature()) {
            self.attackable_notify_attacked(id, attacker);
        }
        // CreatureAI.notifyActionAttacked
        if !self.can_handle_action(id) {
            return;
        }
        self.client_start_auto_attack(id);
    }

    fn attackable_notify_attacked(&mut self, id: ObjectId, attacker: ObjectId) {
        // `SiegeGuardAI.notifyActionAttacked` (M7-SIEGE-02).
        if self.is_siege_guard(id) {
            self.siege_guard_notify_attacked(id, attacker);
            return;
        }
        // Calculate the attack timeout.
        self.renew_attack_timeout(id);

        // Set the _globalAggro to 0 to permit attack even just after spawn.
        if let Some(state) = self.ai.creatures.get_mut(&id)
            && state.global_aggro < 0
        {
            state.global_aggro = 0;
        }

        // Add the attacker to the _aggroList of the actor if not present.
        if !self.aggro_contains(id, attacker) {
            self.add_damage_hate(id, attacker, 0, 1);
        }

        // Set the Creature movement type to run and send Server->Client packet ChangeMoveType to
        // all others Player.
        if !self.is_running(id) {
            self.set_running(id, true);
        }

        // (`!me.isCoreAIDisabled()`: a territory ward never fights back, M7-TW-01)
        // Set the Intention to ATTACK.
        let core_ai_disabled = self.is_core_ai_disabled(id);
        if !core_ai_disabled && self.intention_of(id) != Intention::Attack {
            self.set_intention_attack(id, attacker);
        } else if !core_ai_disabled && self.aggro_most_hated(id) != self.attack_target_of(id) {
            // NOTE(java-quirk): the attacker becomes the target, not the most hated.
            self.set_intention_attack(id, attacker);
        }

        if self.is_monster(id) {
            self.minions_assist(id, attacker);
        }

        // Handle all WorldObject of its Faction inside the Faction Range.
        self.faction_call(id, attacker);
    }

    /// The two `onAssist` calls of `notifyActionAttacked`, `notifyActionAggression` and
    /// `Attackable.reduceCurrentHp`: this monster's minions, then its master's.
    pub(crate) fn minions_assist(&mut self, id: ObjectId, attacker: ObjectId) {
        if !self.minions_of(id).is_empty() {
            self.minion_list_on_assist(id, id, attacker);
        }
        if let Some(master) = self.leader_of(id)
            && !self.minions_of(master).is_empty()
        {
            self.minion_list_on_assist(master, id, attacker);
        }
    }

    /// Java `MinionList.onAssist(caller, attacker)` of `master`'s list.
    fn minion_list_on_assist(&mut self, master: ObjectId, caller: ObjectId, attacker: ObjectId) {
        if !self.is_alike_dead(master) && !self.is_in_combat(master) {
            self.add_damage_hate(master, attacker, 0, BASE_AGGRO_AMOUNT);
        }
        let caller_is_master = caller == master;
        let mut aggro_amount = if caller_is_master {
            MASTER_AGGRO_AMOUNT
        } else {
            BASE_AGGRO_AMOUNT
        };
        if self.is_raid(master) {
            aggro_amount *= RAID_AGGRO_MULTIPLIER;
        }
        for minion in self.minions_of(master).to_vec() {
            if !self.is_dead(minion) && (caller_is_master || !self.is_in_combat(minion)) {
                self.add_damage_hate(minion, attacker, 0, aggro_amount);
            }
        }
    }

    /// Java `MinionList.onMasterTeleported()`: the minions follow their master.
    pub(crate) fn on_master_teleported(&mut self, master: ObjectId) {
        if self.minions_of(master).is_empty() {
            return;
        }
        let Some((mx, my, mz)) = self.position_of(master) else {
            return;
        };
        let spawn_radius = SPAWN_OFFSET_RADIUS;
        let collision = self
            .npc_template(master)
            .map_or(0.0, |t| f64::from(t.collision_radius)) as i32;
        let minimum_radius = collision + MINIMUM_SPAWN_RADIUS;
        for minion in self.minions_of(master).to_vec() {
            if self.is_dead(minion) || self.is_movement_disabled(minion) {
                continue;
            }
            let mut new_x = self.rng().get(minimum_radius * 2, spawn_radius * 2); // x
            let mut new_y = self.rng().get(new_x, spawn_radius * 2); // distance
            new_y = f64::from(new_y * new_y - new_x * new_x).sqrt() as i32; // y
            new_x = if new_x > spawn_radius + minimum_radius {
                (mx + new_x) - spawn_radius
            } else {
                (mx - new_x) + minimum_radius
            };
            new_y = if new_y > spawn_radius + minimum_radius {
                (my + new_y) - spawn_radius
            } else {
                (my - new_y) + minimum_radius
            };
            self.tele_to_location(minion, TeleportTarget::at(new_x, new_y, mz));
        }
    }

    /// The faction call of `notifyActionAttacked`.
    fn faction_call(&mut self, id: ObjectId, attacker: ObjectId) {
        let Some(template) = self.npc_template(id) else {
            return;
        };
        if template.clans.is_empty() {
            return;
        }
        let clans = template.clans.clone();
        let generic_clan = self.npc_world.generic_clan_id();
        let faction_range = template.clan_help_range + template.collision_radius_int();
        let my_npc_id = template.id;
        // Call friendly npcs for help only if this NPC was attacked by the target creature.
        if !self.attack_by_list(id).contains(&attacker) {
            return;
        }
        let attacker_playable = self.is_player(attacker);
        let attacker_z = self.position_of(attacker).map_or(0, |p| p.2);
        let now = self.world_ms();
        for nearby in self.visible_objects_in_range(id, faction_range) {
            if !self.is_attackable_npc(nearby) {
                continue;
            }
            // Don't call dead npcs, npcs without ai or npcs which are too far away.
            if self.is_dead(nearby) || !self.has_ai(nearby) {
                continue;
            }
            let nearby_z = self.position_of(nearby).map_or(0, |p| p.2);
            if (attacker_z.wrapping_sub(nearby_z)).abs() > 600 {
                continue;
            }
            // Don't call npcs spawned within the last 7 seconds.
            let spawned_at = self.npcs.get(&nearby).map_or(0, |n| n.spawn_time_ms);
            if now - spawned_at < FACTION_CALL_SPAWN_GRACE_MS {
                continue;
            }
            // Don't call npcs who are already doing some action (e.g. attacking, casting).
            if !matches!(self.intention_of(nearby), Intention::Idle | Intention::Active) {
                continue;
            }
            // Don't call npcs who aren't in the same clan.
            let Some(nearby_template) = self.npc_template(nearby) else {
                continue;
            };
            let same_clan =
                clans.contains(&generic_clan) || nearby_template.clans.iter().any(|c| clans.contains(c));
            if !same_clan || nearby_template.ignore_clan_npc_ids.contains(&my_npc_id) {
                continue;
            }
            if attacker_playable {
                // By default, when a faction member calls for help, attack the caller's
                // attacker.
                if self.can_see_target(nearby, attacker) {
                    self.notify_action_aggression(nearby, attacker, 1);
                }
                // D-05: `ON_ATTACKABLE_FACTION_CALL` is sent to the scripts here.
            } else if self.intention_of(nearby) != Intention::Attack && self.can_see_target(nearby, attacker)
            {
                let hate = self.aggro_hating(id, attacker);
                self.add_damage_hate(nearby, attacker, 0, hate);
                self.set_intention_attack(nearby, attacker);
            }
        }
    }

    /// The "clan help range aggro on kill" of `Creature.doDie`: when a player who is not a GM
    /// kills an attackable, the members of its clan in range attack the killer.
    pub(crate) fn clan_help_on_kill(&mut self, id: ObjectId, killer: Option<ObjectId>) {
        let Some(killer) = killer.filter(|k| self.is_player(*k) && !self.is_gm(*k)) else {
            return;
        };
        if !self.is_attackable_npc(id) {
            return;
        }
        let Some(template) = self.npc_template(id) else {
            return;
        };
        if template.clans.is_empty() {
            return;
        }
        let clans = template.clans.clone();
        let range = template.clan_help_range;
        let generic_clan = self.npc_world.generic_clan_id();
        let killer_z = self.position_of(killer).map_or(0, |p| p.2);
        let now = self.world_ms();
        for called in self.visible_objects_in_range(id, range) {
            if !self.is_attackable_npc(called) || self.is_dead(called) || !self.has_ai(called) {
                continue;
            }
            let called_z = self.position_of(called).map_or(0, |p| p.2);
            if killer_z.wrapping_sub(called_z).abs() > 600 {
                continue;
            }
            // Don't call npcs spawned within the last 7 seconds.
            if now - self.npcs.get(&called).map_or(0, |n| n.spawn_time_ms) < FACTION_CALL_SPAWN_GRACE_MS {
                continue;
            }
            if !matches!(self.intention_of(called), Intention::Idle | Intention::Active) {
                continue;
            }
            let same_clan = self
                .npc_template(called)
                .is_some_and(|t| clans.contains(&generic_clan) || t.clans.iter().any(|c| clans.contains(c)));
            if !same_clan {
                continue;
            }
            // By default, when a faction member calls for help, attack the caller's attacker.
            self.notify_action_aggression(called, killer, 1);
            // D-05: `ON_ATTACKABLE_FACTION_CALL`.
        }
    }

    /// Java `AttackableAI.notifyActionAggression(target, aggro)`.
    pub(crate) fn notify_action_aggression(&mut self, id: ObjectId, target: ObjectId, aggro: i64) {
        // `SiegeGuardAI.notifyActionAggression` (M7-SIEGE-02).
        if self.is_siege_guard(id) {
            self.siege_guard_notify_aggression(id, target, aggro);
            return;
        }
        if !self.is_attackable_npc(id) {
            return;
        }
        if self.is_dead(id) || !self.objects.get(&target).is_some_and(|e| e.kind.is_creature()) {
            return;
        }
        // Add the target to the actor _aggroList or update hate if already present.
        self.add_damage_hate(id, target, 0, aggro);

        // Set the actor AI Intention to ATTACK.
        if self.intention_of(id) != Intention::Attack {
            // Set the Creature movement type to run and send Server->Client packet
            // ChangeMoveType to all others Player.
            if !self.is_running(id) {
                self.set_running(id, true);
            }
            self.set_intention_attack(id, target);
        }

        if self.is_monster(id) {
            self.minions_assist(id, target);
        }
    }

    /// Java `CreatureAI.notifyActionArrived()` of an NPC.
    pub(crate) fn npc_notify_action_arrived(&mut self, id: ObjectId) {
        self.wake_ai(id);
        if !self.can_handle_action(id) {
            return;
        }
        // Casting returns here. `revalidateZone(true)`: NPCs have no zones.
        if self.is_casting_now(id) || self.is_casting_simultaneously_now(id) {
            return;
        }
        if self.move_to_next_route_point(id) {
            return;
        }
        self.client_stopped_moving(id);
        // `WalkingManager.onArrived` (M8-MISC-01); `ON_NPC_MOVE_FINISHED` is a script event (D-05).
        self.walking_on_arrived(id);

        // If the Intention was MOVE_TO, set the Intention to ACTIVE.
        if self.intention_of(id) == Intention::MoveTo {
            self.set_intention_active(id);
        }
        // Launch actions corresponding to the Action Think.
        self.notify_action_think(id);
    }

    /// Java `CreatureAI.notifyActionForgetObject(object)` of an NPC: what it was doing with the
    /// object that left its view.
    pub(crate) fn npc_notify_forget_object(&mut self, id: ObjectId, object: ObjectId) {
        // `getAI()` makes the AI.
        self.wake_ai(id);
        if !self.can_handle_action(id) {
            return;
        }
        // If the object was targeted: forget it. (INTERACT / PICK_UP are not NPC intentions.)
        if self.ai_target_of(id) == Some(object) {
            self.set_ai_target(id, None);
        }
        // Check if the object was targeted to attack.
        if self.attack_target_of(id) == Some(object) {
            // Cancel attack target
            self.set_attack_target(id, None);
            // Set the Intention of this AbstractAI to ACTIVE (not for a corpse: fixes stop move
            // from cast target decay).
            if !self.is_alike_dead(object) {
                self.set_intention_active(id);
            }
        }
        // Check if the object was targeted to cast.
        if self.cast_target_of(id) == Some(object) {
            // Cancel cast target
            self.set_cast_target(id, None);
            // Set the Intention of this AbstractAI to ACTIVE (not for a corpse).
            if !self.is_alike_dead(object) {
                self.set_intention_active(id);
            }
        }
        // Check if the object was targeted to follow.
        if self.follow_target_of(id) == Some(object) {
            // Stop the actor movement server side AND client side by sending Server->Client packet
            // StopMove/StopRotation (broadcast).
            self.client_stop_moving(id, None);
            // Stop an AI Follow Task.
            self.stop_follow(id);
            // Set the Intention of this AbstractAI to ACTIVE.
            self.set_intention_active(id);
        }
    }

    /// Java `AbstractAI.describeStateToPlayer(player)`: a creature that moves when it comes into
    /// view tells the player where it goes.
    pub(crate) fn describe_state_to_player(&mut self, subject: ObjectId, viewer: ObjectId) {
        if !self.is_visible_for(subject, viewer) || !self.is_moving(subject) {
            return;
        }
        let Some(core) = self.core_of(subject) else {
            return;
        };
        let (x, y, z) = (core.x, core.y, core.z);
        // `_clientMovingToPawnOffset != 0 && isFollowing()`: nothing follows but players.
        if let Some(mv) = self.mv_of(subject) {
            let packet = MoveToLocation {
                object_id: subject.0,
                dest_x: mv.x_destination,
                dest_y: mv.y_destination,
                dest_z: mv.z_destination,
                x,
                y,
                z,
            };
            self.send(viewer, &packet);
        }
    }

    /// The AI part of `World.addVisibleObject` / `switchRegion` for the object that `viewer`
    /// sees: `getAI()`, `describeStateToPlayer`, and a monster that was idle becomes active.
    pub(crate) fn wake_npc_in_view(&mut self, subject: ObjectId, viewer: ObjectId) {
        let Some(kind) = self.objects.get(&subject).map(|e| e.kind) else {
            return;
        };
        // A door is a creature too: `getAI()` makes its `DoorAI` (M7-SIEGE-02). A door never
        // moves, so `describeStateToPlayer` has nothing to tell.
        if kind == ObjectKind::Door {
            if let Some(door) = self.doors.get_mut(&subject) {
                door.has_ai = true;
            }
            return;
        }
        // `BoatAI` / `AirShipAI.describeStateToPlayer` (M8-VEHICLE-01).
        if kind == ObjectKind::Vehicle {
            self.vehicle_describe_state_to_player(subject, viewer);
            return;
        }
        if !matches!(kind, ObjectKind::Player | ObjectKind::Npc | ObjectKind::Summon) {
            return;
        }
        self.wake_ai(subject);
        self.describe_state_to_player(subject, viewer);
        if self.is_monster(subject) && self.intention_of(subject) == Intention::Idle {
            self.set_intention_active(subject);
        }
    }

    /// Java `Attackable.onSpawn()` (after `Creature.onSpawn`): a fresh start.
    pub(crate) fn npc_on_spawn(&mut self, id: ObjectId) {
        if !self.is_attackable_npc(id) {
            return;
        }
        // Keep track of the spawn time.
        let now = self.world_ms();
        if let Some(npc) = self.npcs.get_mut(&id) {
            npc.spawn_time_ms = now;
        }
        // Clear all aggro char from list
        self.aggro_clear(id);
        self.set_running(id, false);
        // Check the region where this mob is, do not activate the AI if region is inactive.
        if self.has_ai(id) {
            // Roll the aggro timeout again, the AI object survives a respawn
            // (`getAI() instanceof AttackableAI`: not the siege guards' AIs).
            if !self.is_siege_guard(id) {
                let roll = self.roll_global_aggro(id);
                if let Some(state) = self.ai.creatures.get_mut(&id) {
                    state.global_aggro = roll;
                }
            }
            // Set the intention of the Attackable to ACTIVE.
            self.set_intention_active(id);
            // Check the region where this mob is, do not activate the AI if region is inactive.
            let active = self
                .objects
                .get(&id)
                .and_then(|e| e.region)
                .is_some_and(|r| self.is_region_active(r));
            if !active {
                self.stop_ai_task(id);
            }
        }
        // `Guard.onSpawn` goes on from here.
        self.guard_on_spawn(id);
    }

    /// Java `WorldRegion.switchAI(isOn)`.
    pub(crate) fn switch_ai(&mut self, region: RegionId, on: bool) {
        if on {
            // Start HP/MP/CP regeneration task.
            self.start_regeneration_in_region(region);
            // Java: RandomAnimationTaskManager.add for each NPC of the region (`ai/idle.rs`).
            self.start_random_animation_in_region(region);
            // The guards that were waiting for a region around them to turn on (`guards.rs`).
            self.wake_guards_around(region);
            return;
        }
        // Java: RandomAnimationTaskManager.remove for each NPC of the region (`ai/idle.rs`).
        self.stop_random_animation_in_region(region);
        let max_drift = f64::from(self.ai.attackable.settings.max_drift_range);
        let mobs: Vec<ObjectId> = self
            .regions
            .objects(region)
            .filter(|id| self.is_attackable_npc(*id))
            .collect();
        for mob in mobs {
            // Set target to null and cancel attack or cast. (The Creature's own target is for
            // skills.)
            // Stop movement. (Only a creature that moves has a `StopMove` to broadcast; nobody
            // is in view of a region that switches off anyway.)
            self.clear_pending_path(mob);
            if self.is_moving(mob) {
                self.stop_move(mob, None);
            }
            // Stop all active skills effects in progress on the Creature.
            self.stop_all_effects(mob);
            self.aggro_clear(mob);
            self.attack_by_clear(mob);
            // Teleport to spawn when too far away.
            let far = self.npcs.get(&mob).is_some_and(|n| {
                n.spawn.is_some() && distance_2d((n.core.x, n.core.y, n.core.z), n.spawn_point) > max_drift
            });
            if far {
                self.teleport_to_spawn_exact(mob);
            }
            // Stop the AI tasks.
            if self.has_ai(mob) {
                self.set_intention_idle(mob);
                self.stop_ai_task(mob);
            }
            // Stop attack task.
            self.abort_attack(mob);
        }
    }

    /// `mob.teleToLocation(spawn)`: no random offset.
    fn teleport_to_spawn_exact(&mut self, id: ObjectId) {
        self.teleport_to_spawn(id, false);
    }

    // ------------------------------------------------------------------ the aggro list

    /// Java `Attackable.getAggroList()` is `World::aggro_list`; `getAttackByList()`.
    pub fn attack_by_list(&self, id: ObjectId) -> &[ObjectId] {
        self.combat
            .creatures
            .get(&id)
            .map_or(&[], |c| c.attack_by.as_slice())
    }

    /// `getAttackByList().clear()`.
    pub(crate) fn attack_by_clear(&mut self, id: ObjectId) {
        if let Some(state) = self.combat.creatures.get_mut(&id) {
            state.attack_by.clear();
        }
    }

    /// Java `Attackable.addAttackerToAttackByList(creature)`.
    pub(crate) fn add_attacker_to_attack_by_list(&mut self, id: ObjectId, creature: ObjectId) {
        if creature == id || !self.is_attackable_npc(id) {
            return;
        }
        let state = self.combat_state_mut(id);
        if !state.attack_by.contains(&creature) {
            state.attack_by.push(creature);
        }
    }

    /// Java `Attackable.clearAggroList()`.
    pub(crate) fn aggro_clear(&mut self, id: ObjectId) {
        if let Some(state) = self.combat.creatures.get_mut(&id) {
            state.aggro.clear();
        }
    }

    /// Java `Attackable.isInAggroList(creature)`.
    pub fn aggro_contains(&self, id: ObjectId, creature: ObjectId) -> bool {
        self.aggro_list(id).iter().any(|a| a.attacker == creature)
    }

    /// Java `AggroInfo.checkHate(owner)` on every entry: hate is lost for an attacker that is
    /// dead, gone or out of the owner's surrounding regions.
    fn aggro_check_hate(&mut self, id: ObjectId) {
        let entries: Vec<ObjectId> = self.aggro_list(id).iter().map(|a| a.attacker).collect();
        let lost: Vec<ObjectId> = entries
            .into_iter()
            .filter(|attacker| {
                self.is_alike_dead(*attacker)
                    || !self.objects.get(attacker).is_some_and(|e| e.spawned)
                    || !self.is_in_surrounding_region(id, *attacker)
            })
            .collect();
        if lost.is_empty() {
            return;
        }
        if let Some(state) = self.combat.creatures.get_mut(&id) {
            for info in &mut state.aggro {
                if lost.contains(&info.attacker) {
                    info.hate = 0;
                }
            }
        }
    }

    /// Java `Attackable.getMostHated()`.
    pub fn aggro_most_hated(&mut self, id: ObjectId) -> Option<ObjectId> {
        if self.aggro_list(id).is_empty() || self.is_alike_dead(id) {
            return None;
        }
        self.aggro_check_hate(id);
        let mut most_hated = None;
        let mut max_hate = 0;
        for info in self.aggro_list(id) {
            if info.hate > max_hate {
                most_hated = Some(info.attacker);
                max_hate = info.hate;
            }
        }
        most_hated
    }

    /// Java `Attackable.getHateList()`: `None` when the list is empty or the NPC is dead.
    pub(crate) fn aggro_hate_list(&mut self, id: ObjectId) -> Option<Vec<ObjectId>> {
        if self.aggro_list(id).is_empty() || self.is_alike_dead(id) {
            return None;
        }
        self.aggro_check_hate(id);
        Some(self.aggro_list(id).iter().map(|a| a.attacker).collect())
    }

    /// Java `Attackable.getHating(target)`.
    pub fn aggro_hating(&mut self, id: ObjectId, target: ObjectId) -> i64 {
        if self.aggro_list(id).is_empty() {
            return 0;
        }
        let Some(info) = self.aggro_list(id).iter().find(|a| a.attacker == target).copied() else {
            return 0;
        };
        let AggroInfo { hate, .. } = info;
        let invisible = self.objects.get(&target).is_some_and(|e| e.invisible);
        let spawned = self.objects.get(&target).is_some_and(|e| e.spawned);
        // Players: `isInvul()` joins the invisible check (`isSpawnProtected()` is the teleport
        // protection, which is not modelled).
        if invisible || !spawned || (self.is_player(target) && self.is_invul(target)) {
            // Remove Object Should Use This Method and Can be Blocked While Interacting.
            if let Some(state) = self.combat.creatures.get_mut(&id) {
                state.aggro.retain(|a| a.attacker != target);
            }
            return 0;
        }
        if self.is_alike_dead(target) {
            self.aggro_stop_hating(id, target);
            return 0;
        }
        hate
    }

    /// Java `Attackable.stopHating(target)`: clears the hate without removing the entry.
    pub(crate) fn aggro_stop_hating(&mut self, id: ObjectId, target: ObjectId) {
        if let Some(state) = self.combat.creatures.get_mut(&id)
            && let Some(info) = state.aggro.iter_mut().find(|a| a.attacker == target)
        {
            info.hate = 0;
        }
    }
}
