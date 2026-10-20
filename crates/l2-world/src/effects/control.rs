//! What the status effects do to a creature outside its effect list: the checks that read the
//! effect flags (`isStunned`, `isMovementDisabled`, ...), the AI notifications the handlers call,
//! and the creature state Java keeps on `Creature`/`Player` that effects change (fake death,
//! sitting, immobilization, the PvP flag, invisibility, an NPC's target, charges and souls).
// Java: entity/actor/Creature.java (isStunned, isSleeping, isRooted, isParalyzed, isMuted,
//       isPhysicalMuted, isPhysicalAttackMuted, isConfused, isControlBlocked, isOutOfControl,
//       isAllSkillsDisabled, isDisabled, isImmobilized, startStunning, stopStunning, startParalyze,
//       startFakeDeath, stopFakeDeath, setRunning, abortAttack)
// Java: ai/CreatureAI.java (notifyActionStunned, ...Paralyzed, ...Sleeping, ...Rooted, ...Confused,
//       ...Muted, ...FakeDeath, ...Afraid)
// Java: ai/AttackableAI.java (notifyActionAfraid: the fear task)
// Java: entity/actor/Player.java (sitDown, standUp, isFakeDeath, isRecentFakeDeath, updatePvPFlag,
//       increaseCharges, decreaseCharges, increaseSouls, ...)
// Java: entity/actor/tasks/player/{SitDownTask,StandUpTask}.java
// Java: entity/WorldObject.java (setInvisible)
//
// Java keeps these on the creature object. Here they are one `CreatureControl` per creature in
// `EffectsWorld::control`, made when something first sets one (a creature without an entry has
// every value at Java's default).

use std::collections::HashMap;

use l2_data::skill::AbnormalVisualEffect;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    AutoAttackStop, ChangeMoveType, ChangeWaitType, DeleteObject, Revive, SystemMessage,
};
use l2_util::ObjectId;

use super::registry::{EffectType, effect_flag};
use super::traits::TraitStats;
use crate::formulas::Rnd;
use crate::movement::{Intention, heading_to_degree};
use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::World;

/// `CreatureAI.FEAR_RANGE`.
const FEAR_RANGE: f64 = 500.0;
/// `AttackableAI.FEAR_TICKS`: seconds between two runs of the fear task, and what one run takes
/// off the fear time.
const FEAR_TICKS: i32 = 5;
/// `Player.sitDown` / `standUp` schedule their task 2.5 s later.
const SIT_STAND_MS: u64 = 2500;
/// `BotReportTable.ATTACK_ACTION_BLOCK_ID`, `ACTION_BLOCK_ID`: what `Action` and `AttackRequest`
/// ask the bot penalty effects (`BlockAction`).
pub(crate) const ATTACK_ACTION_BLOCK_ID: i32 = -1;
pub(crate) const ACTION_BLOCK_ID: i32 = -4;
/// `ValidatePosition`: a blink lets the next out-of-sync position pass for 100 ms.
const BLINK_MS: u64 = 100;

/// The timers of this module (one `TimerKind::Control`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlTimer {
    /// Java `ResetChargesTask`.
    ResetCharges(ObjectId),
    /// Java `ResetSoulsTask`.
    ResetSouls(ObjectId),
    /// Java `SitDownTask`.
    SitDown(ObjectId),
    /// Java `StandUpTask`.
    StandUp(ObjectId),
    /// Java `AttackableAI._fearTask`: one run of the fixed-rate task.
    Fear(ObjectId),
    /// Java `ThreadPool.schedule(() -> player.setBlinkActive(false), 100)` of `ValidatePosition`.
    BlinkOff(ObjectId),
}

/// The fear of an NPC (Java `AttackableAI._fearTime`, `_fearTask`).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Fear {
    /// `_fearTime` in seconds.
    pub time: i32,
    pub task: Option<FearTask>,
}

/// The running fear task and what its lambda captured.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FearTask {
    pub timer: TimerId,
    pub effector: ObjectId,
    pub start: bool,
}

/// Creature state that effects change (see the module comment).
#[derive(Debug, Default)]
pub(crate) struct CreatureControl {
    /// `Creature._isImmobilized` (`ImmobileBuff`).
    pub immobilized: bool,
    /// `Creature._isParalyzed`: the sitting animation's lock (`SitDownTask`), observer mode.
    pub paralyzed: bool,
    /// `Player._isFakeDeath`.
    pub fake_death: bool,
    /// `Player._recentFakeDeathEndTime`, in game ticks (0 = none).
    pub recent_fake_death_end: i32,
    /// `Player._waitTypeSitting`.
    pub sitting: bool,
    /// `Player._sittingInProgress`.
    pub sitting_in_progress: bool,
    pub sit_timer: Option<TimerId>,
    /// `Player._pvpFlag`.
    pub pvp_flag: i32,
    /// `Player._charges` and its reset task.
    pub charges: i32,
    pub charge_timer: Option<TimerId>,
    /// `Player._souls` and its reset task.
    pub souls: i32,
    pub soul_timer: Option<TimerId>,
    /// `PlayerStat._talismanSlots`, `_cloakSlot` and `Player._expertisePenaltyBonus`.
    pub talisman_slots: i32,
    pub cloak_slot: bool,
    pub expertise_penalty_bonus: i32,
    /// An NPC's `Creature._target` (a player's is `Player::target`).
    pub target: Option<ObjectId>,
    /// The fear of an attackable NPC.
    pub fear: Option<Fear>,
    /// `DistrustAI._forcedTarget`: the monster fights this one instead of its hate list.
    pub distrust: Option<ObjectId>,
    /// The attack and defence traits (`CreatureStat`).
    pub traits: TraitStats,
    /// `Disarm._disarmedPlayers`: the weapon the player has to get back.
    pub disarmed_item: Option<ObjectId>,
    /// The chat ban and the party ban of the bot report debuffs (`BlockChat`, `BlockAction`).
    pub chat_banned: bool,
    pub party_banned: bool,
    /// The `SoulEating` listeners of the player: (skill id, level) and `expNeeded`.
    pub soul_eating: Vec<((i32, i32), i32)>,
    /// `Player._blinkActive`.
    pub blink_active: bool,
}

impl CreatureControl {
    /// Every timer the entry holds (to cancel them when the creature goes).
    pub(crate) fn timers(&self) -> Vec<TimerId> {
        let mut timers = Vec::new();
        timers.extend(self.sit_timer);
        timers.extend(self.charge_timer);
        timers.extend(self.soul_timer);
        timers.extend(self.fear.and_then(|f| f.task).map(|t| t.timer));
        timers
    }
}

/// The side table (`EffectsWorld::control`).
pub(crate) type ControlTable = HashMap<ObjectId, CreatureControl>;

impl World {
    // ------------------------------------------------------------------ the table

    /// The control state of a creature, if something ever set one.
    pub(crate) fn control(&self, id: ObjectId) -> Option<&CreatureControl> {
        self.effects.control.get(&id)
    }

    /// The control state of a player or NPC, made on first use.
    pub(crate) fn control_mut(&mut self, id: ObjectId) -> Option<&mut CreatureControl> {
        if !self.players.contains_key(&id) && !self.npcs.contains_key(&id) {
            return None;
        }
        Some(self.effects.control.entry(id).or_default())
    }

    // ------------------------------------------------------------------ the flag checks

    /// Java `Creature.isStunned()`.
    pub fn is_stunned(&self, id: ObjectId) -> bool {
        self.is_affected(id, effect_flag::STUNNED)
    }

    /// Java `Creature.isSleeping()`.
    pub fn is_sleeping(&self, id: ObjectId) -> bool {
        self.is_affected(id, effect_flag::SLEEP)
    }

    /// Java `Creature.isRooted()`.
    pub fn is_rooted(&self, id: ObjectId) -> bool {
        self.is_affected(id, effect_flag::ROOTED)
    }

    /// Java `Creature.isParalyzed()`: `_isParalyzed || isAffected(PARALYZED)`.
    pub fn is_paralyzed(&self, id: ObjectId) -> bool {
        self.control(id).is_some_and(|c| c.paralyzed) || self.is_affected(id, effect_flag::PARALYZED)
    }

    /// Java `Creature.isMuted()`.
    pub fn is_muted(&self, id: ObjectId) -> bool {
        self.is_affected(id, effect_flag::MUTED)
    }

    /// Java `Creature.isPhysicalMuted()`.
    pub fn is_physical_muted(&self, id: ObjectId) -> bool {
        self.is_affected(id, effect_flag::PSYCHICAL_MUTED)
    }

    /// Java `Creature.isPhysicalAttackMuted()`.
    pub fn is_physical_attack_muted(&self, id: ObjectId) -> bool {
        self.is_affected(id, effect_flag::PSYCHICAL_ATTACK_MUTED)
    }

    /// Java `Creature.isConfused()`.
    pub fn is_confused(&self, id: ObjectId) -> bool {
        self.is_affected(id, effect_flag::CONFUSED)
    }

    /// Java `Creature.isBetrayed()`.
    pub fn is_betrayed(&self, id: ObjectId) -> bool {
        self.is_affected(id, effect_flag::BETRAYED)
    }

    /// Java `Creature.isDisarmed()`.
    pub fn is_disarmed(&self, id: ObjectId) -> bool {
        self.is_affected(id, effect_flag::DISARMED)
    }

    /// Java `Creature.isControlBlocked()`: `isAffected(FEAR)`.
    pub fn is_control_blocked(&self, id: ObjectId) -> bool {
        self.is_affected(id, effect_flag::FEAR)
    }

    /// Java `Creature.isOutOfControl()`.
    pub fn is_out_of_control(&self, id: ObjectId) -> bool {
        self.is_confused(id) || self.is_control_blocked(id)
    }

    /// Java `Creature.isAllSkillsDisabled()`: `_allSkillsDisabled || isStunned() || isSleeping() ||
    /// isParalyzed()` (`_allSkillsDisabled` is only set by events, which are scripts).
    pub fn is_all_skills_disabled(&self, id: ObjectId) -> bool {
        self.is_stunned(id) || self.is_sleeping(id) || self.is_paralyzed(id)
    }

    /// Java `Creature.isImmobilized()`.
    pub fn is_immobilized(&self, id: ObjectId) -> bool {
        self.control(id).is_some_and(|c| c.immobilized)
    }

    /// Java `Creature.setImmobilized(value)` (`ImmobileBuff`).
    pub fn set_immobilized(&mut self, id: ObjectId, value: bool) {
        if let Some(c) = self.control_mut(id) {
            c.immobilized = value;
        }
    }

    /// Java `Player.isFakeDeath()`.
    pub fn is_fake_death(&self, id: ObjectId) -> bool {
        self.control(id).is_some_and(|c| c.fake_death)
    }

    /// Java `Player.isRecentFakeDeath()`: `_recentFakeDeathEndTime > gameTicks`.
    pub fn is_recent_fake_death(&self, id: ObjectId) -> bool {
        self.control(id)
            .is_some_and(|c| c.recent_fake_death_end > self.game_ticks())
    }

    /// Java `Player.setRecentFakeDeath(protect)`: the grace period against aggressive monsters
    /// after getting up.
    pub fn set_recent_fake_death(&mut self, id: ObjectId, protect: bool) {
        // `PLAYER_FAKEDEATH_UP_PROTECTION * GameTimeTaskManager.TICKS_PER_SECOND` (10 game ticks).
        let end = if protect {
            self.game_ticks()
                .saturating_add(self.settings.effects.fake_death_up_protection.saturating_mul(10))
        } else {
            0
        };
        if let Some(c) = self.control_mut(id) {
            c.recent_fake_death_end = end;
        }
    }

    /// Java `Player.isSitting()`.
    pub fn is_sitting(&self, id: ObjectId) -> bool {
        self.control(id).is_some_and(|c| c.sitting)
    }

    // ------------------------------------------------------------------ the CAST-01 seams

    /// Java `Creature.isCastingNow()`.
    // M5-CAST-01: reads the cast state when casting lands; nobody casts before it.
    pub(crate) fn is_casting_now_for_effects(&self, _id: ObjectId) -> bool {
        false
    }

    /// Java `Creature.canAbortCast()`.
    // M5-CAST-01: the cast state's flag.
    pub(crate) fn can_abort_cast_for_effects(&self, _id: ObjectId) -> bool {
        true
    }

    /// Java `Creature.abortCast()`.
    // M5-CAST-01: breaks the running cast (`MagicSkillCanceled`, `ActionFailed`).
    pub(crate) fn abort_cast_for_effects(&mut self, _id: ObjectId) {}

    /// Java `Creature.breakCast()`.
    // M5-CAST-01: breaks the cast with its system message.
    pub(crate) fn break_cast_for_effects(&mut self, _id: ObjectId) {}

    // ------------------------------------------------------------------ picks and checks

    /// Java `World.getRandomVisibleObject(creature, Creature.class)`: a uniform pick (reservoir
    /// sampling, one `Rnd.get(count)` per candidate) among the creatures the creature sees.
    pub(crate) fn random_visible_creature(&mut self, id: ObjectId) -> Option<ObjectId> {
        let candidates = self.visible_creatures_of(id);
        self.reservoir_pick(&candidates)
    }

    /// `World.getRandomVisibleObjectInRange(monster, Monster.class, range, m -> m != monster &&
    /// !m.isDead() && m.isAttackable() && !(m instanceof Chest) && !m.isRaid() &&
    /// !m.isRaidMinion())`.
    pub(crate) fn random_distrust_target(&mut self, monster: ObjectId, range: i32) -> Option<ObjectId> {
        let candidates: Vec<ObjectId> = self
            .visible_objects_in_range(monster, range)
            .into_iter()
            .filter(|&m| {
                m != monster
                    && self.is_monster(m)
                    && !self.is_dead(m)
                    && self.is_attackable_npc(m)
                    && !self.is_chest(m)
                    && !self.is_raid_or_minion(m)
            })
            .collect();
        self.reservoir_pick(&candidates)
    }

    /// The single-pass reservoir sampling of `World.getRandomVisibleObject*`.
    fn reservoir_pick(&mut self, candidates: &[ObjectId]) -> Option<ObjectId> {
        let mut selected = None;
        for (index, &candidate) in candidates.iter().enumerate() {
            let count = i32::try_from(index + 1).unwrap_or(i32::MAX);
            if self.rng().get_int(count) == 0 {
                selected = Some(candidate);
            }
        }
        selected
    }

    /// Java `instanceof Chest`.
    pub(crate) fn is_chest(&self, id: ObjectId) -> bool {
        self.npc_template(id).is_some_and(|t| t.npc_type == "Chest")
    }

    /// The `BuffInfo` of `AbnormalType.BOT_PENALTY` asks each of its effects whether the action
    /// is blocked (`BlockAction.checkCondition`): the check of `Action`, `AttackRequest`,
    /// `RequestActionUse` and `TradeRequest`. `false` (and the message) when it is.
    pub(crate) fn check_bot_penalty(&mut self, id: ObjectId, action_block_id: i32) -> bool {
        use l2_data::stats::AbnormalType;
        let blocked = self
            .effect_list(id)
            .and_then(|l| l.info_by_abnormal_type(AbnormalType::BotPenalty))
            .is_some_and(|info| {
                info.effects()
                    .iter()
                    .any(|e| !e.handler.check_condition(action_block_id))
            });
        if blocked {
            self.send(
                id,
                &SystemMessage::new(
                    SystemMessageId::YouHaveBeenReportedAsAnIllegalProgramUserSoYourActionsHaveBeenRestricted,
                ),
            );
            self.send(id, &l2_protocol_game::server::ActionFailed);
        }
        !blocked
    }

    /// `PlayableAI.setIntentionAttack`: the newbie protection blessing keeps a chaotic player ten
    /// or more levels above away from a protected one, and the other way round (not in a PvP
    /// zone).
    pub(crate) fn protection_blessing_refuses(&self, player: ObjectId, target: ObjectId) -> bool {
        let (Some(p), Some(t)) = (self.players.get(&player), self.players.get(&target)) else {
            return false;
        };
        if self.is_inside_zone(target, crate::zones::ZoneId::Pvp) {
            return false;
        }
        let blessed = |id| self.is_affected(id, effect_flag::PROTECTION_BLESSING);
        (blessed(target) && p.core.level - t.core.level >= 10 && p.data.karma > 0)
            || (blessed(player) && t.core.level - p.core.level >= 10 && t.data.karma > 0)
    }

    /// Java `getStat().getAttackTrait(type)`.
    pub fn attack_trait(&self, id: ObjectId, t: l2_data::skill::TraitType) -> f32 {
        self.control(id).map_or(1.0, |c| c.traits.attack_trait(t))
    }

    /// Java `getStat().getDefenceTrait(type)`.
    pub fn defence_trait(&self, id: ObjectId, t: l2_data::skill::TraitType) -> f32 {
        self.control(id).map_or(1.0, |c| c.traits.defence_trait(t))
    }

    /// Java `getStat().isTraitInvul(type)`.
    pub fn is_trait_invul(&self, id: ObjectId, t: l2_data::skill::TraitType) -> bool {
        self.control(id).is_some_and(|c| c.traits.is_trait_invul(t))
    }

    /// The creature a `Distrust`ed monster is forced to fight (`DistrustAI._forcedTarget`).
    pub fn distrust_target(&self, id: ObjectId) -> Option<ObjectId> {
        self.control(id).and_then(|c| c.distrust)
    }

    /// `AttackableAI._fearTime`.
    pub fn fear_time(&self, id: ObjectId) -> i32 {
        self.control(id).and_then(|c| c.fear).map_or(0, |f| f.time)
    }

    /// `getFirstEffect(type) != null`.
    pub(crate) fn is_affected_by_effect_type(&self, id: ObjectId, effect_type: EffectType) -> bool {
        self.effect_list(id)
            .is_some_and(|l| l.first_effect(effect_type).is_some())
    }

    /// `SoulEating.onExperienceReceived`, for every `SoulEating` effect of the player: a soul for
    /// an experience gain of at least `expNeeded`.
    pub(crate) fn soul_eating_on_exp_changed(&mut self, id: ObjectId, exp: i64) {
        let needed: Vec<i32> = match self.control(id) {
            Some(c) if !c.soul_eating.is_empty() => c.soul_eating.iter().map(|(_, n)| *n).collect(),
            _ => return,
        };
        for exp_needed in needed {
            if !self.is_player(id) || exp < i64::from(exp_needed) {
                continue;
            }
            let max_souls = self
                .stats(id)
                .map_or(0, |v| v.calc(crate::stats::Stat::MaxSouls, 0.0) as i32);
            if self.charged_souls_of(id) >= max_souls {
                self.send(
                    id,
                    &SystemMessage::new(SystemMessageId::SoulCannotBeAbsorbedAnymore),
                );
                continue;
            }
            self.increase_souls(id, 1);
            if let Some(npc) = self.target_of(id).filter(|t| self.npcs.contains_key(t)) {
                self.broadcast_creature_packet(
                    id,
                    &l2_protocol_game::server::ExSpawnEmitter {
                        player_object_id: id.0,
                        npc_object_id: npc.0,
                    },
                );
            }
        }
    }

    /// `DistrustAI.thinkAttack()`: the monster attacks the creature it was forced onto, or goes
    /// back to being active when that is dead.
    pub(crate) fn distrust_think_attack(&mut self, id: ObjectId, forced: ObjectId) {
        if !self.objects.contains_key(&forced) || self.is_dead(forced) {
            self.set_creature_target(id, None);
            self.set_intention_active(id);
            return;
        }
        self.set_creature_target(id, Some(forced));
        self.set_intention_attack(id, forced);
        let range =
            self.stats(id).map_or(0, |s| s.physical_attack_range()) + self.collision_radius_of(forced);
        let distance = match (self.position_of(id), self.position_of(forced)) {
            (Some(a), Some(b)) => crate::target::distance_3d(a, b),
            _ => f64::MAX,
        };
        if distance > f64::from(range) {
            self.move_to_pawn(id, forced, range);
            return;
        }
        if !self.can_see_target(id, forced) {
            return;
        }
        self.do_attack(id, forced);
    }

    /// Java `Player.isChatBanned()` for the bans effects start.
    pub fn is_chat_banned(&self, id: ObjectId) -> bool {
        self.control(id).is_some_and(|c| c.chat_banned)
    }

    /// Java `Player.isPartyBanned()` for the bans effects start.
    pub fn is_party_banned(&self, id: ObjectId) -> bool {
        self.control(id).is_some_and(|c| c.party_banned)
    }

    // ------------------------------------------------------------------ the AI notifications

    /// Java `CreatureAI.canHandleAction()` after `getAI()` made the AI of an NPC.
    fn ai_can_handle(&mut self, id: ObjectId) -> bool {
        self.wake_ai(id);
        self.can_handle_action(id)
    }

    /// The part `notifyActionStunned`, `...Paralyzed` and `...Sleeping` share: the auto attack
    /// stops (client and server) and the movement stops.
    fn ai_stop_attack_and_move(&mut self, id: ObjectId) {
        // Stop the actor auto-attack client side by sending Server->Client packet AutoAttackStop.
        self.broadcast_creature_packet(id, &AutoAttackStop { target_id: id.0 });
        if self.has_attack_stance_task(id) {
            self.remove_attack_stance_task(id);
        }
        // Stop Server AutoAttack also.
        self.set_auto_attacking(id, false);
        // Stop the actor movement server side AND client side.
        self.client_stop_moving(id, None);
        // `NextAction` (callbacks of the client packets of skills) is not ported.
    }

    /// Java `CreatureAI.notifyActionStunned()`.
    pub fn notify_action_stunned(&mut self, id: ObjectId) {
        if self.ai_can_handle(id) {
            self.ai_stop_attack_and_move(id);
        }
    }

    /// Java `CreatureAI.notifyActionParalyzed()`.
    pub fn notify_action_paralyzed(&mut self, id: ObjectId) {
        if self.ai_can_handle(id) {
            self.ai_stop_attack_and_move(id);
        }
    }

    /// Java `CreatureAI.notifyActionSleeping()`.
    pub fn notify_action_sleeping(&mut self, id: ObjectId) {
        if self.ai_can_handle(id) {
            self.ai_stop_attack_and_move(id);
        }
    }

    /// Java `CreatureAI.notifyActionRooted()`: only the movement stops.
    pub fn notify_action_rooted(&mut self, id: ObjectId) {
        if self.ai_can_handle(id) {
            self.client_stop_moving(id, None);
        }
    }

    /// Java `CreatureAI.notifyActionConfused()`: only the movement stops.
    pub fn notify_action_confused(&mut self, id: ObjectId) {
        if self.ai_can_handle(id) {
            self.client_stop_moving(id, None);
        }
    }

    /// Java `CreatureAI.notifyActionMuted()`: only runs the `NextAction`, which is not ported.
    pub fn notify_action_muted(&mut self, id: ObjectId) {
        let _ = self.ai_can_handle(id);
    }

    /// Java `CreatureAI.notifyActionFakeDeath()`: the AI forgets everything.
    pub fn notify_action_fake_death(&mut self, id: ObjectId) {
        if !self.ai_can_handle(id) {
            return;
        }
        // Stop an AI Follow Task.
        self.stop_follow(id);
        // Stop the actor movement and send Server->Client packet StopMove/StopRotation.
        self.client_stop_moving(id, None);
        // Init AI.
        self.set_intention_value(id, Intention::Idle);
        if let Some(pawn) = self.pawn_mut(id) {
            pawn.ai_target = None;
        }
        self.set_attack_target(id, None);
    }

    /// Java `CreatureAI.notifyActionAfraid(effector, start)` (`AttackableAI`'s version for an
    /// attackable NPC with a fear time).
    pub fn notify_action_afraid(&mut self, id: ObjectId, effector: ObjectId, start: bool) {
        if self.is_attackable_npc(id) {
            self.attackable_notify_action_afraid(id, effector, start);
        } else {
            self.creature_notify_action_afraid(id, effector, start);
        }
    }

    /// Java `CreatureAI.notifyActionAfraid`: the creature runs `FEAR_RANGE` away from the
    /// effector (or straight on when this is a later tick of the fear).
    fn creature_notify_action_afraid(&mut self, id: ObjectId, effector: ObjectId, start: bool) {
        if !self.ai_can_handle(id) {
            return;
        }
        let Some((x, y, z)) = self.position_of(id) else {
            return;
        };
        let effector_pos = self
            .objects
            .get(&effector)
            .filter(|e| e.kind.is_creature())
            .and_then(|_| self.position_of(effector));
        let degrees = match effector_pos {
            // `LocationUtil.calculateAngleFrom(effector, actor)`.
            Some((ex, ey, _)) if start => {
                let mut angle = f64::from(y.wrapping_sub(ey))
                    .atan2(f64::from(x.wrapping_sub(ex)))
                    .to_degrees();
                if angle < 0.0 {
                    angle += 360.0;
                }
                angle
            }
            _ => heading_to_degree(self.heading_of(id)),
        };
        let radians = degrees.to_radians();
        let pos_x = (f64::from(x) + (FEAR_RANGE * radians.cos())) as i32;
        let pos_y = (f64::from(y) + (FEAR_RANGE * radians.sin())) as i32;
        // `if (!_actor.isPet()) _actor.setRunning()`.
        self.set_running_effect(id, true);
        // If pathfinding enabled the creature will go to the destination or it will go to the
        // nearest obstacle.
        let (to_x, to_y, to_z) = if self.pathfinding_enabled() {
            let instance = self.objects.get(&id).map_or(0, |e| e.instance_id);
            let to = self.geo_valid_location(
                l2_geo::Location::new(x, y, z),
                l2_geo::Location::new(pos_x, pos_y, z),
                instance,
            );
            (to.x, to.y, to.z)
        } else {
            (pos_x, pos_y, z)
        };
        if self.is_player(id) {
            self.set_intention_move_to(id, to_x, to_y, to_z);
        } else {
            self.npc_set_intention_move_to(id, to_x, to_y, to_z);
        }
    }

    /// Java `AttackableAI.notifyActionAfraid`: with a fear time, the first call starts the fear
    /// task (and the flee effect); later calls move the NPC and end the fear when it ran out.
    fn attackable_notify_action_afraid(&mut self, id: ObjectId, effector: ObjectId, start: bool) {
        self.wake_ai(id);
        let (time, running) = self
            .control(id)
            .and_then(|c| c.fear)
            .map_or((0, false), |f| (f.time, f.task.is_some()));
        if time > 0 && !running {
            // `scheduleAtFixedRate(.., 0, FEAR_TICKS * 1000)`: the first run is due at once.
            let timer = self
                .timers
                .schedule_at(self.tick + 1, TimerKind::Control(ControlTimer::Fear(id)));
            if let Some(fear) = self.control_mut(id).and_then(|c| c.fear.as_mut()) {
                fear.task = Some(FearTask {
                    timer,
                    effector,
                    start,
                });
            }
            self.start_abnormal_visual_effect(id, &[AbnormalVisualEffect::TurnFlee]);
        } else {
            self.creature_notify_action_afraid(id, effector, start);
            let ran_out = self.is_dead(id) || time <= 0;
            let task = self.control(id).and_then(|c| c.fear).and_then(|f| f.task);
            if ran_out && let Some(task) = task {
                self.timers.cancel(task.timer);
                if let Some(fear) = self.control_mut(id).and_then(|c| c.fear.as_mut()) {
                    fear.task = None;
                }
                self.stop_abnormal_visual_effect(id, &[AbnormalVisualEffect::TurnFlee]);
                self.set_intention_idle(id);
            }
        }
    }

    /// One run of `AttackableAI._fearTask`.
    fn on_fear_timer(&mut self, id: ObjectId) {
        let Some(fear) = self.control(id).and_then(|c| c.fear) else {
            return;
        };
        let Some(task) = fear.task else {
            return;
        };
        if !self.npcs.contains_key(&id) {
            return;
        }
        // The next run of the fixed-rate task.
        let period = TimerWheel::ms_to_ticks(u64::try_from(FEAR_TICKS).unwrap_or(5) * 1000);
        let timer = self
            .timers
            .schedule_at(self.tick + period, TimerKind::Control(ControlTimer::Fear(id)));
        if let Some(f) = self.control_mut(id).and_then(|c| c.fear.as_mut()) {
            f.time -= FEAR_TICKS;
            if let Some(t) = f.task.as_mut() {
                t.timer = timer;
            }
        }
        self.notify_action_afraid(id, task.effector, task.start);
    }

    /// Java `AttackableAI.setFearTime(time)`.
    pub fn set_fear_time(&mut self, id: ObjectId, time: i32) {
        if let Some(c) = self.control_mut(id) {
            match c.fear.as_mut() {
                Some(f) => f.time = time,
                None => c.fear = Some(Fear { time, task: None }),
            }
        }
    }

    // ------------------------------------------------------------------ Creature.start*/stop*

    /// `new FlyToLocation(creature, dest, type)` broadcast: the creature flies from where it
    /// stands to `dest` (the caller moves it). A player's next out-of-sync position report is
    /// let through (`setBlinkActive(true)`, see `ControlTimer::BlinkOff`).
    pub(crate) fn fly_to_location(
        &mut self,
        id: ObjectId,
        x: i32,
        y: i32,
        z: i32,
        fly_type: l2_data::skill::FlyType,
    ) {
        let Some((org_x, org_y, org_z)) = self.position_of(id) else {
            return;
        };
        if self.is_player(id)
            && let Some(c) = self.control_mut(id)
        {
            c.blink_active = true;
        }
        self.broadcast_creature_packet(
            id,
            &l2_protocol_game::server::FlyToLocation {
                object_id: id.0,
                dest_x: x,
                dest_y: y,
                dest_z: z,
                org_x,
                org_y,
                org_z,
                fly_type: i32::try_from(fly_type.ordinal()).unwrap_or(0),
            },
        );
    }

    /// `ValidatePosition`'s check of `isBlinkActive()`: when a blink is active, the correction is
    /// skipped and the flag ends 100 ms later. `true` when the correction is to be skipped.
    pub(crate) fn consume_blink(&mut self, id: ObjectId) -> bool {
        if !self.control(id).is_some_and(|c| c.blink_active) {
            return false;
        }
        let due = self.tick + TimerWheel::ms_to_ticks(BLINK_MS);
        self.timers
            .schedule_at(due, TimerKind::Control(ControlTimer::BlinkOff(id)));
        true
    }

    /// Java `Creature.notifyDamageReceived(damage, ...)`: an auto attack makes a fake dead player
    /// stand up (script events: D-05).
    pub fn notify_damage_received(&mut self, id: ObjectId, damage: f64) {
        if self.is_fake_death(id) && self.settings.effects.fake_death_damage_stand && damage > 0.0 {
            self.stop_fake_death(id, true);
        }
    }

    /// What `CreatureStatus.reduceHp` / `PlayerStatus.reduceHp` do after `stopEffectsOnDamage`
    /// for a hit that is no damage over time: a sitting player stands up, a stun may break (one
    /// in ten).
    pub(crate) fn break_states_on_damage(&mut self, id: ObjectId) {
        if self.is_player(id) && self.is_sitting(id) {
            self.stand_up(id);
        }
        if self.is_stunned(id) && self.rng().get_int(10) == 0 {
            self.stop_stunning(id, true);
        }
    }

    /// Java `Creature.startStunning()`.
    pub fn start_stunning(&mut self, id: ObjectId) {
        // Aborts any attacks/casts if stunned.
        self.abort_attack(id);
        self.abort_cast_for_effects(id);
        self.stop_move(id, None);
        self.notify_action_stunned(id);
        // `!isSummon()`: there are no summons yet.
        self.set_intention_idle(id);
        self.update_abnormal_effect(id);
    }

    /// Java `Creature.stopStunning(removeEffects)`.
    pub fn stop_stunning(&mut self, id: ObjectId, remove_effects: bool) {
        if remove_effects {
            self.stop_effects(id, EffectType::Stun);
        }
        if !self.is_player(id) {
            self.notify_action_think(id);
        }
        self.update_abnormal_effect(id);
    }

    /// Java `Creature.startParalyze()`.
    pub fn start_paralyze(&mut self, id: ObjectId) {
        // Aborts any attacks/casts if paralyzed.
        self.abort_attack(id);
        self.abort_cast_for_effects(id);
        self.stop_move(id, None);
        self.notify_action_paralyzed(id);
    }

    /// Java `Creature.startFakeDeath()`: only a player plays dead.
    pub fn start_fake_death(&mut self, id: ObjectId) {
        if !self.is_player(id) {
            return;
        }
        if let Some(c) = self.control_mut(id) {
            c.fake_death = true;
        }
        // Aborts any attacks/casts if fake dead.
        self.abort_attack(id);
        self.abort_cast_for_effects(id);
        self.stop_move(id, None);
        self.notify_action_fake_death(id);
        self.broadcast_wait_type(id, ChangeWaitType::WT_START_FAKEDEATH);
        // Remove target from those that have the untargetable creature on target.
        if self.settings.effects.fake_death_untarget {
            for creature in self.visible_creatures_of(id) {
                if self.creature_target(creature) == Some(id) {
                    self.set_creature_target(creature, None);
                }
            }
        }
    }

    /// Java `Creature.stopFakeDeath(removeEffects)`.
    pub fn stop_fake_death(&mut self, id: ObjectId, remove_effects: bool) {
        if remove_effects {
            self.stop_effects(id, EffectType::FakeDeath);
        }
        // If this is a player instance, start the grace period for this character (grace from
        // mobs only)!
        if self.is_player(id) {
            if let Some(c) = self.control_mut(id) {
                c.fake_death = false;
            }
            self.set_recent_fake_death(id, true);
        }
        self.broadcast_wait_type(id, ChangeWaitType::WT_STOP_FAKEDEATH);
        // NOTE(java-quirk): "Temp hack: players see FD on ppl that are moving"; Java sends a
        // `Revive` to everybody on top of the wait type.
        self.broadcast_creature_packet(id, &Revive { object_id: id.0 });
    }

    /// `broadcastPacket(new ChangeWaitType(creature, type))`.
    pub(crate) fn broadcast_wait_type(&self, id: ObjectId, move_type: i32) {
        let Some((x, y, z)) = self.position_of(id) else {
            return;
        };
        self.broadcast_creature_packet(
            id,
            &ChangeWaitType {
                object_id: id.0,
                move_type,
                x,
                y,
                z,
            },
        );
    }

    /// `World.forEachVisibleObject(creature, Creature.class, ...)`.
    pub(crate) fn visible_creatures_of(&self, id: ObjectId) -> Vec<ObjectId> {
        self.visible_objects(id)
            .into_iter()
            .filter(|o| self.objects.get(o).is_some_and(|e| e.kind.is_creature()))
            .collect()
    }

    // ------------------------------------------------------------------ sitting

    /// Java `Player.sitDown(checkCast)`.
    pub fn sit_down(&mut self, id: ObjectId, check_cast: bool) {
        if !self.is_player(id) {
            return;
        }
        if self.control(id).is_some_and(|c| c.sitting_in_progress) {
            return;
        }
        if check_cast && self.is_casting_now_for_effects(id) {
            self.send_message(id, "Cannot sit while casting");
            return;
        }
        if self.is_sitting(id)
            || self.is_attack_disabled(id)
            || self.is_out_of_control(id)
            || self.is_immobilized(id)
        {
            return;
        }
        self.break_attack(id);
        if let Some(c) = self.control_mut(id) {
            c.sitting = true;
            c.sitting_in_progress = true;
        }
        // `PlayerAI.setIntentionRest()`.
        self.set_intention_rest(id);
        self.broadcast_wait_type(id, ChangeWaitType::WT_SITTING);
        // Schedule a sit down task to wait for the animation to finish.
        let due = self.tick + TimerWheel::ms_to_ticks(SIT_STAND_MS);
        let timer = self
            .timers
            .schedule_at(due, TimerKind::Control(ControlTimer::SitDown(id)));
        if let Some(c) = self.control_mut(id) {
            c.sit_timer = Some(timer);
        }
    }

    /// Java `PlayerAI.setIntentionRest()`.
    fn set_intention_rest(&mut self, id: ObjectId) {
        if self.intention_of(id) == Intention::Rest {
            return;
        }
        self.set_intention_value(id, Intention::Rest);
        if let Some(pawn) = self.pawn_mut(id) {
            pawn.ai_target = None;
        }
        if let Some(p) = self.players.get_mut(&id) {
            p.target = None;
        }
        self.set_attack_target(id, None);
        self.client_stop_moving(id, None);
    }

    /// Java `Player.standUp()`.
    pub fn stand_up(&mut self, id: ObjectId) {
        if !self.is_player(id) {
            return;
        }
        if self.control(id).is_some_and(|c| c.sitting_in_progress) {
            return;
        }
        // `!isInStoreMode()`: no private stores yet.
        if self.is_sitting(id) && !self.is_alike_dead(id) {
            if let Some(c) = self.control_mut(id) {
                c.sitting_in_progress = true;
            }
            if self.is_affected(id, effect_flag::RELAXING) {
                self.stop_effects(id, EffectType::Relaxing);
            }
            self.broadcast_wait_type(id, ChangeWaitType::WT_STANDING);
            // Schedule a stand up task to wait for the animation to finish.
            let due = self.tick + TimerWheel::ms_to_ticks(SIT_STAND_MS);
            let timer = self
                .timers
                .schedule_at(due, TimerKind::Control(ControlTimer::StandUp(id)));
            if let Some(c) = self.control_mut(id) {
                c.sit_timer = Some(timer);
            }
        }
    }

    // ------------------------------------------------------------------ running, targets, flags

    /// Java `Creature.setRunning()` (to run): `ChangeMoveType`, then the new info to those who see
    /// the creature.
    pub(crate) fn set_running_effect(&mut self, id: ObjectId, value: bool) {
        if self.npcs.contains_key(&id) {
            self.set_running(id, value);
            return;
        }
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        if player.movement.running == value {
            return;
        }
        player.movement.running = value;
        if self.stats(id).is_some_and(|s| s.run_speed() != 0.0) {
            self.broadcast_creature_packet(
                id,
                &ChangeMoveType {
                    object_id: id.0,
                    running: value,
                },
            );
        }
        self.broadcast_user_info(id);
    }

    /// Java `Creature.getTarget()`. A player's is `Player::target`; an NPC's is the target its
    /// skills set, or the one its AI is working on.
    // NOTE: Java's attackable AI sets `Creature._target` in many places; here the AI's own target
    // (`AbstractAI._target`) stands in for it until the NPC AI casts (M5-AI-03).
    pub fn creature_target(&self, id: ObjectId) -> Option<ObjectId> {
        if self.players.contains_key(&id) {
            return self.target_of(id);
        }
        self.control(id)
            .and_then(|c| c.target)
            .or_else(|| self.pawn_of(id).and_then(|p| p.ai_target))
    }

    /// Java `Creature.setTarget(target)`.
    pub fn set_creature_target(&mut self, id: ObjectId, target: Option<ObjectId>) {
        if self.players.contains_key(&id) {
            self.set_target(id, target);
            return;
        }
        if let Some(c) = self.control_mut(id) {
            c.target = target;
        }
        if target.is_none()
            && let Some(pawn) = self.pawn_mut(id)
        {
            pawn.ai_target = None;
        }
    }

    /// Java `WorldObject.setInvisible(invisible)`: those who may no longer see the object get a
    /// `DeleteObject`, then the object is announced to those who may.
    pub fn set_invisible(&mut self, id: ObjectId, invisible: bool) {
        let Some(entry) = self.objects.get_mut(&id) else {
            return;
        };
        entry.invisible = invisible;
        if invisible {
            let packet = DeleteObject { object_id: id.0 };
            for viewer in self.visible_players(id) {
                if !self.is_visible_for(id, viewer) {
                    self.send(viewer, &packet);
                }
            }
        }
        // Broadcast information regarding the object to those which are suppose to see.
        self.broadcast_info(id);
    }

    /// Java `Player.getPvpFlag()`.
    pub fn pvp_flag_of(&self, id: ObjectId) -> i32 {
        self.control(id).map_or(0, |c| c.pvp_flag)
    }

    /// Java `Player.updatePvPFlag(value)`: the new flag goes to the player and to everybody who
    /// sees it.
    // NOTE: the relation masks and `isAutoAttackable` of the `RelationChanged` are still the
    // M3 stubs (`player_view.rs`); only the PvP flag in `UserInfo` and `CharInfo` changes.
    pub fn update_pvp_flag(&mut self, id: ObjectId, value: i32) {
        if !self.is_player(id) || self.pvp_flag_of(id) == value {
            return;
        }
        if let Some(c) = self.control_mut(id) {
            c.pvp_flag = value;
        }
        self.broadcast_user_info(id);
    }

    // ------------------------------------------------------------------ the timers

    /// Dispatch of `TimerKind::Control`.
    pub(crate) fn on_control_timer(&mut self, timer: ControlTimer) {
        match timer {
            ControlTimer::ResetCharges(id) => {
                if let Some(c) = self.effects.control.get_mut(&id) {
                    c.charge_timer = None;
                }
                self.clear_charges(id);
            }
            ControlTimer::ResetSouls(id) => {
                if let Some(c) = self.effects.control.get_mut(&id) {
                    c.soul_timer = None;
                }
                self.clear_souls(id);
            }
            ControlTimer::SitDown(id) => {
                // `SitDownTask`: the sitting player is held in place.
                if let Some(c) = self.effects.control.get_mut(&id) {
                    c.sit_timer = None;
                    c.paralyzed = true;
                    c.sitting_in_progress = false;
                }
            }
            ControlTimer::StandUp(id) => {
                if let Some(c) = self.effects.control.get_mut(&id) {
                    c.sit_timer = None;
                    c.paralyzed = false;
                    c.sitting = false;
                    c.sitting_in_progress = false;
                }
                self.set_intention_idle(id);
            }
            ControlTimer::Fear(id) => self.on_fear_timer(id),
            ControlTimer::BlinkOff(id) => {
                if let Some(c) = self.effects.control.get_mut(&id) {
                    c.blink_active = false;
                }
            }
        }
    }

    /// Cancels a creature's control timers and drops its entry (the creature left the world).
    pub(crate) fn forget_control(&mut self, id: ObjectId) {
        if let Some(control) = self.effects.control.remove(&id) {
            for timer in control.timers() {
                self.timers.cancel(timer);
            }
        }
    }

    // ------------------------------------------------------------------ messages

    /// "Your skill was deactivated due to lack of MP." for the toggles that ran dry.
    pub(crate) fn send_lack_of_mp(&self, id: ObjectId) {
        self.send(
            id,
            &SystemMessage::new(SystemMessageId::YourSkillWasDeactivatedDueToLackOfMp),
        );
    }
}
