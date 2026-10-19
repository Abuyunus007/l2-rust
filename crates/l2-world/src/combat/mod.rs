//! Auto attack: `Creature.doAttack`, the hit tasks, damage and the combat stance.
//! Design: `docs/design/m4-combat.md` ("Combat and AI rules").
// Java: entity/actor/Creature.java (doAttack, doAttackHitByBow/CrossBow/Dual/Pole/Simple,
//       canUseRangeWeapon, onHitTimer, abortAttack, breakAttack, isAttackingNow, reduceCurrentHp,
//       sendDamageMessage, doDie)
// Java: entity/actor/tasks/creature/HitTask.java
// Java: taskmanagers/AttackStanceTaskManager.java
// Java: network/clientpackets/AttackRequest.java
//
// # How an attack runs (no thread, no sleep)
// - `do_attack` ([`attack`]) does Java's checks, rolls every hit at once with the formulas
//   ([`hit`], the order of `doAttackHitSimple`), broadcasts `Attack` and schedules on the timer
//   wheel: one [`TimerKind::Hit`] per hit (Java `HitTask`) at Java's delay, and a
//   [`TimerKind::ReadyToAct`] at `timeAtk + reuse` (the AI acts again then, see `ai/`).
// - `on_hit_timer` ([`damage`]) is `Creature.onHitTimer`: messages, `reduce_current_hp`, the
//   attack stances, attack break.
// - [`stance`] is `AttackStanceTaskManager`: a stance ends 15 s after the last refresh; one 1 s
//   timer runs while any stance exists.
//
// # The per-creature state
// [`CombatState`] lives in a side table keyed by object id (D-11), made on first use and dropped
// when a player leaves (`World::forget_combat`). It holds what Java keeps in `Creature` fields:
// `_attackEndTime`, `_disableBowAttackEndTime`, `_isDead`, the `needHpUpdate` checks and, for
// attackable NPCs, the damage part of the aggro list.
//
// # Hooks left for other cards (grep the markers)
// - `// M4-DEATH-01:` `World::do_die` only marks the creature dead, stops its attack and resets
//   its AI; the `Die` packet, decay, respawn, rewards are DEATH-01's.
// - `// M4-AI-01:` what an attackable NPC does when attacked, its thinking and its aggro list use.
// - `// M4-REGEN-01:` where Java starts/stops the regeneration task when HP/MP/CP change.
// - `// M4-INV-01:` equipping arrows from the inventory, arrow consumption.
// - `// M5:` soulshots, skills, effects (invulnerability, traits, reflect/absorb), PvP flagging.

pub mod attack;
pub mod damage;
pub mod hit;
pub mod request;
pub mod stance;
mod vitals;

use std::collections::HashMap;
use std::sync::Arc;

use l2_data::HitConditionBonus;
use l2_util::ObjectId;

use crate::timer::TimerId;
use crate::world::World;

pub use damage::AggroInfo;
pub use hit::{HitOutcome, attacker_position};

/// One scheduled hit (Java `HitTask`'s fields).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HitTask {
    pub attacker: ObjectId,
    pub target: ObjectId,
    pub damage: i32,
    pub crit: bool,
    pub miss: bool,
    pub shld: u8,
    pub soulshot: bool,
    pub recharge_shots: bool,
}

/// The configuration values the attack code reads. `Default` is the Java default of each key.
#[derive(Debug, Clone, PartialEq)]
pub struct CombatSettings {
    /// `PlayerConfig.ALT_GAME_SHIELD_BLOCKS` (`AltShieldBlocks`).
    pub alt_game_shield_blocks: bool,
    /// `PlayerConfig.ALT_PERFECT_SHLD_BLOCK` (`AltPerfectShieldBlockRate`).
    pub alt_perfect_shld_block: i32,
    /// `PlayerConfig.RANDOMIZE_AUTO_ATTACK_DAMAGE` (`RandomizeAutoAttackDamage`).
    pub randomize_auto_attack_damage: bool,
    /// `PlayerConfig.ALT_GAME_CANCEL_CAST` (`AltGameCancelByHit` is `Cast` or `All`).
    pub alt_game_cancel_cast: bool,
    /// `PlayerConfig.ALT_GAME_CANCEL_BOW` (`AltGameCancelByHit` is `Bow` or `All`).
    pub alt_game_cancel_bow: bool,
    /// `PlayerConfig.CALCULATE_DISTANCE_BOW_DAMAGE` (`DistanceBowDamage`).
    pub calculate_distance_bow_damage: bool,
    /// `PlayerConfig.ALT_GAME_KARMA_PLAYER_CAN_BE_KILLED_IN_PEACEZONE`.
    pub karma_player_can_be_killed_in_peacezone: bool,
    /// `NpcConfig.MIN_NPC_LEVEL_DMG_PENALTY` (`MinNPCLevelForDmgPenalty`).
    pub min_npc_level_dmg_penalty: i32,
    /// `NpcConfig.NPC_DMG_PENALTY` (`DmgPenaltyForLvLDifferences`).
    pub npc_dmg_penalty: Vec<f32>,
    /// `NpcConfig.NPC_CRIT_DMG_PENALTY` (`CritDmgPenaltyForLvLDifferences`).
    pub npc_crit_dmg_penalty: Vec<f32>,
    /// `NpcConfig.ALT_ATTACKABLE_NPCS` (`AltAttackableNpcs`): `Npc.canBeAttacked()`.
    pub alt_attackable_npcs: bool,
}

impl Default for CombatSettings {
    fn default() -> Self {
        Self {
            alt_game_shield_blocks: false,
            alt_perfect_shld_block: 10,
            randomize_auto_attack_damage: true,
            alt_game_cancel_cast: true,
            alt_game_cancel_bow: false,
            calculate_distance_bow_damage: false,
            karma_player_can_be_killed_in_peacezone: false,
            min_npc_level_dmg_penalty: 78,
            npc_dmg_penalty: vec![0.7, 0.6, 0.6, 0.55],
            npc_crit_dmg_penalty: vec![0.75, 0.65, 0.6, 0.58],
            alt_attackable_npcs: true,
        }
    }
}

/// Java `Creature.needHpUpdate()`'s fields (`_hpUpdateIncCheck`, `_hpUpdateDecCheck`,
/// `_hpUpdateInterval`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HpUpdateChecks {
    pub inc_check: f64,
    pub dec_check: f64,
    pub interval: f64,
}

/// The combat fields of one creature (Java `Creature`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CombatState {
    /// Java `_attackEndTime`, in world milliseconds ([`World::world_ms`]).
    pub attack_end_ms: i64,
    /// Java `_disableBowAttackEndTime`, in game ticks ([`World::game_ticks`]).
    pub disable_bow_attack_end_time: i32,
    /// Java `_isDead`.
    pub dead: bool,
    /// `needHpUpdate`'s state; made from the maximum HP the first time it is needed.
    pub hp_update: Option<HpUpdateChecks>,
    /// Java `Attackable._aggroList`, in the order attackers were first added (only the damage
    /// and hate this card adds; M4-AI-01 reads and extends it).
    pub aggro: Vec<AggroInfo>,
}

/// The combat tables of the world.
#[derive(Debug, Default)]
pub struct CombatWorld {
    pub(crate) creatures: HashMap<ObjectId, CombatState>,
    /// Java `AttackStanceTaskManager.CREATURE_ATTACK_STANCES`: creature -> world ms of the last
    /// refresh.
    pub(crate) stances: HashMap<ObjectId, i64>,
    /// The pending 1 s stance check, while any stance exists.
    pub(crate) stance_timer: Option<TimerId>,
    /// Java `Player._broadcastStatusUpdateTask`.
    pub(crate) status_update_timers: HashMap<ObjectId, TimerId>,
    /// `data/stats/hitConditionBonus.xml` (Java `HitConditionBonusData`).
    pub(crate) hit_condition_bonus: Arc<HitConditionBonus>,
}

impl World {
    /// Installs `hitConditionBonus.xml`. Without it every bonus is 0 (an empty file in Java).
    pub fn set_hit_condition_bonus(&mut self, bonus: Arc<HitConditionBonus>) {
        self.combat.hit_condition_bonus = bonus;
    }

    /// The combat state of a creature, if it has one yet.
    pub fn combat_state(&self, id: ObjectId) -> Option<&CombatState> {
        self.combat.creatures.get(&id)
    }

    pub(crate) fn combat_state_mut(&mut self, id: ObjectId) -> &mut CombatState {
        self.combat.creatures.entry(id).or_default()
    }

    /// Drops everything combat knows about a creature that left the world.
    pub(crate) fn forget_combat(&mut self, id: ObjectId) {
        self.combat.creatures.remove(&id);
        self.combat.stances.remove(&id);
        if let Some(timer) = self.combat.status_update_timers.remove(&id) {
            self.timers.cancel(timer);
        }
        self.ai.creatures.remove(&id);
    }

    /// Java `Creature.isDead()`. A player loaded with less than 0.5 HP counts as dead too, as
    /// Java's restore marks it.
    pub fn is_dead(&self, id: ObjectId) -> bool {
        if self.combat.creatures.get(&id).is_some_and(|c| c.dead) {
            return true;
        }
        self.players.get(&id).is_some_and(|p| p.core.cur_hp < 0.5)
    }

    /// Java `Creature.isAttackingNow()`: an attack is still in its animation.
    pub fn is_attacking_now(&self, id: ObjectId) -> bool {
        let now = self.world_ms();
        self.combat
            .creatures
            .get(&id)
            .is_some_and(|c| c.attack_end_ms > now)
    }

    /// Java `Creature.isBowAttackDisabled()` ... `_disableBowAttackEndTime > gameTicks`.
    pub fn is_range_attacking_now(&self, id: ObjectId) -> bool {
        let now = self.game_ticks();
        self.combat
            .creatures
            .get(&id)
            .is_some_and(|c| c.disable_bow_attack_end_time > now)
    }

    /// Java `Creature.isDisabled()`: AI disabled, (alike) dead, attack muted, stunned, asleep,
    /// paralyzed. The effects are M5, so only death counts.
    pub fn is_disabled(&self, id: ObjectId) -> bool {
        self.is_dead(id)
    }

    /// Java `Creature.isAttackDisabled()`.
    pub fn is_attack_disabled(&self, id: ObjectId) -> bool {
        self.is_attacking_now(id) || self.is_disabled(id)
    }

    /// `getName()` of a creature: the character name, or the NPC template's name.
    pub(crate) fn creature_name(&self, id: ObjectId) -> String {
        if let Some(p) = self.players.get(&id) {
            return p.data.name.clone();
        }
        self.npcs
            .get(&id)
            .and_then(|n| self.npc_world.template(n.npc_id))
            .map(|t| t.name.clone())
            .unwrap_or_default()
    }

    /// `getHeading()` of a player or NPC (0 for anything else).
    pub(crate) fn heading_of(&self, id: ObjectId) -> i32 {
        if let Some(p) = self.players.get(&id) {
            return p.core.heading;
        }
        self.npcs.get(&id).map_or(0, |n| n.core.heading)
    }

    pub(crate) fn set_heading(&mut self, id: ObjectId, heading: i32) {
        if let Some(p) = self.players.get_mut(&id) {
            p.core.heading = heading;
        } else if let Some(n) = self.npcs.get_mut(&id) {
            n.core.heading = heading;
        }
    }

    /// The `CreatureCore` of a player or NPC.
    pub(crate) fn core_of(&self, id: ObjectId) -> Option<&crate::player::CreatureCore> {
        if let Some(p) = self.players.get(&id) {
            return Some(&p.core);
        }
        self.npcs.get(&id).map(|n| &n.core)
    }

    pub(crate) fn core_of_mut(&mut self, id: ObjectId) -> Option<&mut crate::player::CreatureCore> {
        if let Some(p) = self.players.get_mut(&id) {
            return Some(&mut p.core);
        }
        self.npcs.get_mut(&id).map(|n| &mut n.core)
    }

    /// Java `isPlayer()`.
    pub(crate) fn is_player(&self, id: ObjectId) -> bool {
        self.players.contains_key(&id)
    }

    /// Java `isNpc()`.
    pub(crate) fn is_npc(&self, id: ObjectId) -> bool {
        self.npcs.contains_key(&id)
    }

    /// Java `isAttackable()` of an NPC (monsters, guards, ...; see `StatView::is_attackable`).
    pub(crate) fn is_attackable_npc(&self, id: ObjectId) -> bool {
        self.stats(id).is_some_and(|s| s.is_attackable())
    }

    /// Java `isMonster()`.
    pub(crate) fn is_monster(&self, id: ObjectId) -> bool {
        self.npcs.get(&id).is_some_and(|n| n.class.is_monster())
    }

    /// Java `isRaid()`.
    pub(crate) fn is_raid(&self, id: ObjectId) -> bool {
        self.npcs.get(&id).is_some_and(|n| n.class.is_raid())
    }

    /// Java `isInSurroundingRegion(other)`: both are in the grid and their regions are
    /// neighbours.
    pub(crate) fn is_in_surrounding_region(&self, id: ObjectId, other: ObjectId) -> bool {
        let (Some(a), Some(b)) = (
            self.objects.get(&id).and_then(|e| e.region),
            self.objects.get(&other).and_then(|e| e.region),
        ) else {
            return false;
        };
        b.is_surrounding(a)
    }

    /// Java `GeoEngine.canSeeTarget(creature, target)`: doors are always visible; otherwise the
    /// same instance, no door or fence on the line, and the geodata lets sight through.
    pub(crate) fn can_see_target(&self, id: ObjectId, target: ObjectId) -> bool {
        use l2_geo::DynamicObstacles;

        let Some(kind) = self.objects.get(&target).map(|e| e.kind) else {
            return false;
        };
        // `target.isArtefact()`: castle artefacts are siege NPCs (M5).
        if kind == crate::objects::ObjectKind::Door {
            return true;
        }
        let (Some(from), Some(to)) = (self.position_of(id), self.position_of(target)) else {
            return false;
        };
        let instance = self.objects.get(&id).map_or(0, |e| e.instance_id);
        let target_instance = self.objects.get(&target).map_or(0, |e| e.instance_id);
        if instance != target_instance {
            return false;
        }
        let from = l2_geo::Location::new(from.0, from.1, from.2);
        let to = l2_geo::Location::new(to.0, to.1, to.2);
        let obstacles = self.obstacles();
        if obstacles.doors_between(from, to, instance, true) || obstacles.fence_between(from, to, instance) {
            return false;
        }
        self.geo.as_ref().is_none_or(|geo| geo.can_see_target(from, to))
    }
}
