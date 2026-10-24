//! `SiegeGuardAI`: the brain of a castle's siege defenders (and, with an empty allied list, of a
//! siegable hall's `SpecialSiegeGuardAI`). It attacks the enemies of the siege only, never the
//! defenders, heals its friends, calls its clan, and goes home when nobody is left.
// Java: ai/SiegeGuardAI.java, ai/SpecialSiegeGuardAI.java
// Java: ai/CreatureAI.java (SelfAnalysis)
// Java: entity/actor/instance/Defender.java (initAI, returnHome)
//
// # How it plugs in
// A `Defender` (and a `FortCommander`, which extends it) is an attackable, so it uses the think
// set, the aggro list and the intention machine of `attackable.rs`; the entry points there hand
// a siege guard over to this module: `notifyActionThink`, `notifyActionAttacked`,
// `notifyActionAggression`, `setIntentionIdle`, and the AI's creation in `World::wake_ai` (the
// constructor sets `_globalAggro = -10` and analyses the guard's skills). The few lines in which
// `FortSiegeGuardAI` differs are `fort_siege_guard.rs`.
//
// The AI fields that `AttackableAI` does not have are a [`SiegeGuardState`] in
// `AiWorld::siege_guards`, made with the AI and dropped with it (`detach_ai`).
//
// # Which AI a defender gets (`Defender.initAI`)
// No siegable hall within 10000 and no castle within 10000: `FortSiegeGuardAI`; a castle within
// 10000: `SiegeGuardAI`; else (a hall): `SpecialSiegeGuardAI`, whose allied list only scripts fill
// (D-05), so it thinks like `SiegeGuardAI`.
//
// # Java behaviour kept as is
// - NOTE(java-quirk): a guard only looks for enemies within its *physical attack range*
//   (`_attackRange`, taken when the AI is made): a sword guard notices a player 40 away, an
//   archer one 500+ away. Otherwise it waits to be hit, called by its clan or by a hit door.
// - NOTE(java-quirk): `World.addVisibleObject` runs before `onSpawn`, so a guard spawned in view of
//   a player has its AI when `Attackable.onSpawn` sets it `ACTIVE`; a guard spawned where nobody
//   is gets its AI later from a player coming into view, which only activates monsters, so it
//   stays `IDLE` until something wakes it as above.

use l2_data::SkillTemplate;
use l2_data::npc::AiType;
use l2_util::ObjectId;

use crate::effects::{EffectType, SkillKey};
use crate::movement::Intention;
use crate::objects::ObjectKind;
use crate::skills::checks::is_continuous;
use crate::stats::skills::java_hash_map_order;
use crate::world::World;

/// `SiegeGuardAI.MAX_ATTACK_TIMEOUT`: 300 game ticks, 30 seconds.
pub(crate) const MAX_ATTACK_TIMEOUT: i32 = 300;
/// The constructor's `_globalAggro = -10`: "10 seconds timeout of ATTACK after respawn".
const NEW_AI_GLOBAL_AGGRO: i32 = -10;
/// `notifyActionAggression`: the aggro counter after the last hate is gone.
const LOST_GLOBAL_AGGRO: i32 = -25;
/// `Npc.getCastle(10000)` / `getConquerableHall()` in `Defender.initAI`.
const RESIDENCE_RANGE: i32 = 10_000;
/// `1800 * 1800`: how far from its spawn a guard follows an enemy.
const HOME_RANGE_SQ: f64 = 3_240_000.0;
/// `factionNotifyAndSupport`: the range of the clan call and of the heals.
const FACTION_RANGE: i32 = 1000;
/// "Temporary hack for preventing guards jumping off towers": `170 * 170`.
const MAX_DZ_SQ: f64 = 28_900.0;

/// Which of the three AIs of `Defender.initAI` a guard has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SiegeGuardAiKind {
    /// `SiegeGuardAI`: a castle within 10000.
    Castle,
    /// `FortSiegeGuardAI`: neither a castle nor a siegable hall within 10000.
    Fort,
    /// `SpecialSiegeGuardAI`: a siegable hall within 10000 and no castle.
    Hall,
}

/// Java `CreatureAI.SelfAnalysis`: what the guard's AI type and skills make of it. Only the
/// parts the siege guard AIs read are kept (the other skill lists are counted, not stored).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SelfAnalysis {
    pub is_mage: bool,
    pub is_balanced: bool,
    pub is_archer: bool,
    pub is_healer: bool,
    pub is_fighter: bool,
    /// `healSkills`, in the order of the guard's skill map.
    pub heal_skills: Vec<SkillKey>,
    pub has_heal_or_resurrect: bool,
    pub has_long_range_skills: bool,
    pub has_long_range_damage_skills: bool,
    pub max_cast_range: i32,
}

/// The fields of a `SiegeGuardAI` / `FortSiegeGuardAI` that `AttackableAI` does not have.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiegeGuardState {
    pub kind: SiegeGuardAiKind,
    /// `_attackRange`: `getPhysicalAttackRange()` when the AI was made.
    pub attack_range: i32,
    pub analysis: SelfAnalysis,
}

/// What one skill block of `attackPrepare` did.
enum SkillTry {
    /// The guard cast the skill.
    Cast,
    /// A heal while the guard is healthy: `break` out of the skill loop.
    Break,
}

impl World {
    /// Java `instanceof Defender`: the NPCs that get a siege guard AI (`FortCommander` extends
    /// `Defender`).
    pub(crate) fn is_siege_guard(&self, id: ObjectId) -> bool {
        self.npcs.get(&id).is_some_and(|n| n.class.is_defender())
    }

    /// The siege guard fields of a guard's AI, while it has one.
    pub fn siege_guard_state(&self, id: ObjectId) -> Option<&SiegeGuardState> {
        self.ai.siege_guards.get(&id)
    }

    /// The constructor of the guard's AI (`Defender.initAI` picks it): the analysis of its skills
    /// and its attack range. Returns the first `_globalAggro`.
    pub(crate) fn siege_guard_ai_created(&mut self, id: ObjectId) -> i32 {
        let kind = self.siege_guard_kind(id);
        let attack_range = self.stats(id).map_or(0, |v| v.physical_attack_range());
        let analysis = self.self_analysis(id);
        self.ai.siege_guards.insert(
            id,
            SiegeGuardState {
                kind,
                attack_range,
                analysis,
            },
        );
        NEW_AI_GLOBAL_AGGRO
    }

    /// `Defender.initAI()`.
    fn siege_guard_kind(&self, id: ObjectId) -> SiegeGuardAiKind {
        let Some((x, y, z)) = self.position_of(id) else {
            return SiegeGuardAiKind::Fort;
        };
        let castle = self.nearest_castle(x, y, z, i64::from(RESIDENCE_RANGE));
        let hall = self.nearby_siegable_hall(x, y, RESIDENCE_RANGE);
        if hall.is_none() && castle.is_none() {
            SiegeGuardAiKind::Fort
        } else if castle.is_some() {
            SiegeGuardAiKind::Castle
        } else {
            SiegeGuardAiKind::Hall
        }
    }

    fn siege_guard_kind_of(&self, id: ObjectId) -> SiegeGuardAiKind {
        self.ai
            .siege_guards
            .get(&id)
            .map_or(SiegeGuardAiKind::Castle, |s| s.kind)
    }

    /// The guard's skills (`getAllSkills()`) in the order of Java's skill map.
    fn siege_guard_skills(&self, id: ObjectId) -> Vec<SkillKey> {
        let Some(core) = self.core_of(id) else {
            return Vec::new();
        };
        let ids: Vec<i32> = core.skills.iter().map(|(skill, _)| *skill).collect();
        java_hash_map_order(&ids)
            .into_iter()
            .filter_map(|skill| {
                core.skills
                    .iter()
                    .find(|(s, _)| *s == skill)
                    .map(|&(id, level)| SkillKey { id, level })
            })
            .collect()
    }

    /// Java `SelfAnalysis.init()`.
    // The water movement analysis (`cannotMoveOnLand`) is not read by the siege guard AIs.
    fn self_analysis(&mut self, id: ObjectId) -> SelfAnalysis {
        let mut a = SelfAnalysis::default();
        match self.npc_template(id).map(|t| t.ai_type) {
            Some(AiType::Mage) => a.is_mage = true,
            Some(AiType::Corpse | AiType::Balanced) => a.is_balanced = true,
            Some(AiType::Archer) => a.is_archer = true,
            Some(AiType::Healer) => a.is_healer = true,
            Some(AiType::Fighter) | None => a.is_fighter = true,
        }
        let Some(table) = self.skill_table() else {
            return a;
        };
        let mut general_skills = 0;
        for key in self.siege_guard_skills(id) {
            let Some(skill) = table.get(key.id, key.level) else {
                continue;
            };
            if skill.is_passive() {
                continue;
            }
            let cast_range = skill.cast_range;
            let mut has_long_range_damage_skill = false;
            if is_continuous(skill) {
                // Buffs and debuffs go to their own lists, which these AIs do not read.
                continue;
            }
            if self.skill_has_effect_type(skill, &[EffectType::Dispel, EffectType::DispelBySlot]) {
                // cancelSkills
            } else if self.skill_has_effect_type(skill, &[EffectType::Heal]) {
                a.heal_skills.push(key);
                a.has_heal_or_resurrect = true;
            } else if self.skill_has_effect_type(skill, &[EffectType::Sleep])
                || self.skill_has_effect_type(skill, &[EffectType::Stun, EffectType::Paralyze])
                || self.skill_has_effect_type(skill, &[EffectType::Root])
                || self.skill_has_effect_type(skill, &[EffectType::Fear])
                || self.skill_has_effect_type(skill, &[EffectType::Mute])
            {
                // sleepSkills, generalDisablers, rootSkills, debuffSkills, muteSkills
            } else if self.skill_has_effect_type(skill, &[EffectType::Resurrection]) {
                a.has_heal_or_resurrect = true;
            } else {
                general_skills += 1;
                has_long_range_damage_skill = true;
            }
            if cast_range > 150 {
                a.has_long_range_skills = true;
                if has_long_range_damage_skill {
                    a.has_long_range_damage_skills = true;
                }
            }
            if cast_range > a.max_cast_range {
                a.max_cast_range = cast_range;
            }
        }
        // Because of missing skills, some mages/balanced cannot play like mages.
        if !a.has_long_range_damage_skills && a.is_mage {
            a.is_balanced = true;
            a.is_mage = false;
            a.is_fighter = false;
        }
        if !a.has_long_range_skills && (a.is_mage || a.is_balanced) {
            a.is_balanced = false;
            a.is_mage = false;
            a.is_fighter = true;
        }
        if general_skills == 0 && a.is_mage {
            a.is_balanced = true;
            a.is_mage = false;
        }
        a
    }

    fn siege_guard_analysis(&self, id: ObjectId) -> SelfAnalysis {
        self.ai
            .siege_guards
            .get(&id)
            .map(|s| s.analysis.clone())
            .unwrap_or_default()
    }

    // ------------------------------------------------------------------ the think

    /// Java `SiegeGuardAI.notifyActionThink()` (`FortSiegeGuardAI`'s is the same).
    pub(crate) fn siege_guard_notify_think(&mut self, id: ObjectId) {
        // Check if the thinking action is already in progress.
        if self.ai.creatures.get(&id).is_none_or(|a| a.thinking)
            || self.is_casting_now(id)
            || self.is_all_skills_disabled(id)
        {
            return;
        }
        if let Some(state) = self.ai.creatures.get_mut(&id) {
            state.thinking = true;
        }
        match self.intention_of(id) {
            Intention::Active => self.siege_guard_think_active(id),
            Intention::Attack => self.siege_guard_think_attack(id),
            _ => {}
        }
        if let Some(state) = self.ai.creatures.get_mut(&id) {
            state.thinking = false;
        }
    }

    /// Java `SiegeGuardAI.autoAttackCondition(target)` (`FortSiegeGuardAI`'s for a fortress
    /// guard).
    pub(crate) fn siege_guard_auto_attack_condition(&self, id: ObjectId, target: ObjectId) -> bool {
        match self.siege_guard_kind_of(id) {
            SiegeGuardAiKind::Fort => self.fort_siege_guard_auto_attack_condition(id, target),
            // `SpecialSiegeGuardAI`: `_allied.contains(target)` first; only scripts fill the
            // list (D-05).
            SiegeGuardAiKind::Castle | SiegeGuardAiKind::Hall => {
                self.castle_siege_guard_auto_attack_condition(id, target)
            }
        }
    }

    fn castle_siege_guard_auto_attack_condition(&self, id: ObjectId, target: ObjectId) -> bool {
        let Some(kind) = self.objects.get(&target).map(|e| e.kind) else {
            return false;
        };
        // Check if the target isn't another guard, folk or a door.
        if self.is_siege_guard(target)
            || self.is_npc(target)
            || kind == ObjectKind::Door
            || self.is_alike_dead(target)
        {
            return false;
        }
        self.siege_guard_target_checks(id, target)
    }

    /// The part of `autoAttackCondition` both guard AIs share: an invulnerable GM (or his
    /// summon) is left alone, a summon near its owner stands for the owner, a silent mover is
    /// seen only within 250, then `isAutoAttackable` and the line of sight.
    pub(crate) fn siege_guard_target_checks(&self, id: ObjectId, target: ObjectId) -> bool {
        // Check if the target isn't invulnerable.
        if self.is_invul(target)
            && ((self.is_player(target) && self.is_gm(target))
                || self.summon_owner(target).is_some_and(|o| self.is_gm(o)))
        {
            return false;
        }
        // Get the owner if the target is a summon.
        let mut current = target;
        if let Some(owner) = self.summon_owner(target)
            && self.dist_3d(id, owner) < 1000.0
        {
            current = owner;
        }
        // Check if the target isn't in silent move mode AND too far (>100).
        if self.is_playable(current)
            && self.is_affected(current, crate::effects::effect_flag::SILENT_MOVE)
            && self.dist_2d(id, current) >= 250.0
        {
            return false;
        }
        // Los Check Here
        self.is_auto_attackable_by(id, current) && self.can_see_target(id, current)
    }

    /// Java `SiegeGuardAI.thinkActive()`.
    fn siege_guard_think_active(&mut self, id: ObjectId) {
        // Update every 1s the _globalAggro counter to come close to 0.
        let global_aggro = {
            let Some(state) = self.ai.creatures.get_mut(&id) else {
                return;
            };
            if state.global_aggro < 0 {
                state.global_aggro += 1;
            } else if state.global_aggro > 0 {
                state.global_aggro -= 1;
            }
            state.global_aggro
        };
        // Add all autoAttackable Creature in Attackable Aggro Range to its _aggroList with 0
        // damage and 1 hate. A guard isn't aggressive during 10s after its spawn because
        // _globalAggro is set to -10.
        if global_aggro >= 0 {
            let range = self.ai.siege_guards.get(&id).map_or(0, |s| s.attack_range);
            let creatures: Vec<ObjectId> = self
                .visible_objects_in_range(id, range)
                .into_iter()
                .filter(|o| self.objects.get(o).is_some_and(|e| e.kind.is_creature()))
                .collect();
            for target in creatures {
                // check aggression
                if self.siege_guard_auto_attack_condition(id, target) && self.aggro_hating(id, target) == 0 {
                    self.add_damage_hate(id, target, 0, 1);
                }
            }
            // Chose a target from its aggroList.
            let hated = if self.is_confused(id) {
                self.attack_target_of(id)
            } else {
                self.aggro_most_hated(id)
            };
            // Order to the guard to attack the target.
            if let Some(hated) = hated {
                // Get the hate level of the guard against this Creature target contained in
                // _aggroList.
                let aggro = self.aggro_hating(id, hated);
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
        // Order to the Defender to return to its home location because there's no target to
        // attack.
        if self.siege_guard_kind_of(id) == SiegeGuardAiKind::Fort {
            self.fort_siege_guard_return_home(id);
        } else {
            self.return_home(id);
        }
    }

    /// Java `SiegeGuardAI.thinkAttack()`.
    fn siege_guard_think_attack(&mut self, id: ObjectId) {
        let ticks = self.game_ticks();
        let timeout = self.ai.creatures.get(&id).map_or(i32::MAX, |a| a.attack_timeout);
        if timeout < ticks && self.is_running(id) {
            // Set the actor movement type to walk and send Server->Client packet ChangeMoveType
            // to all others Player.
            self.set_running(id, false);
            // Calculate a new attack timeout.
            self.renew_attack_timeout(id);
        }
        let target = self.attack_target_of(id);
        let timeout = self.ai.creatures.get(&id).map_or(i32::MAX, |a| a.attack_timeout);
        // Check if target is dead or if timeout is expired to stop this attack.
        let lost = match target {
            None => true,
            Some(t) => {
                !self.objects.get(&t).is_some_and(|e| e.kind.is_creature()) || self.is_alike_dead(t)
            }
        };
        if lost || timeout < ticks {
            // Stop hating this target after the attack timeout or if target is dead.
            if let Some(t) = target {
                self.aggro_stop_hating(id, t);
            }
            // Cancel target and timeout.
            self.set_attack_timeout(id, i32::MAX);
            self.set_attack_target(id, None);
            // Set the AI Intention to ACTIVE.
            self.set_intention_active(id);
            self.set_running(id, false);
            return;
        }
        self.siege_guard_faction_notify_and_support(id);
        self.siege_guard_attack_prepare(id);
    }

    /// `_actor.getCastle().getSiege().checkIsDefender(player.getClan())` (castle guards) or
    /// `_actor.getFort().getSiege().checkIsDefender(...)` (fortress guards).
    // NOTE(java-quirk): Java throws when the guard has no castle (fortress); that reads as "no
    // defender" here.
    pub(crate) fn siege_guard_is_defender(&self, id: ObjectId, player: ObjectId) -> bool {
        let clan = self.clan_of(player).map(|c| c.id);
        if self.siege_guard_kind_of(id) == SiegeGuardAiKind::Fort {
            return self
                .npc_fort(id)
                .is_some_and(|fort| clan.is_some_and(|c| self.fort_check_is_defender(fort, c)));
        }
        self.npc_castle(id)
            .and_then(|castle| self.castle_siege(castle))
            .is_some_and(|siege| siege.is_defender(clan))
    }

    /// `getCurrentHp() < getMaxHp() * 0.6` and the guard's own HP and MP above a half, the
    /// friend in combat: the heal conditions of `factionNotifyAndSupport`.
    fn siege_guard_should_heal(&self, id: ObjectId, friend: ObjectId) -> bool {
        let hp = |c: ObjectId| self.core_of(c).map_or(0.0, |core| core.cur_hp);
        let mp = |c: ObjectId| self.core_of(c).map_or(0.0, |core| core.cur_mp);
        let max_hp = |c: ObjectId| self.stats(c).map_or(0, |v| v.max_hp());
        let max_mp = |c: ObjectId| self.stats(c).map_or(0, |v| v.max_mp());
        !self.is_attack_disabled(id)
            && hp(friend) < f64::from(max_hp(friend)) * 0.6
            // `getMaxHp() / 2`: an int division.
            && hp(id) > f64::from(max_hp(id) / 2)
            && mp(id) > f64::from(max_mp(id) / 2)
            && self.is_in_combat(friend)
    }

    /// The heal loop of `factionNotifyAndSupport` on `friend`; `true` when the guard cast.
    fn siege_guard_heal(&mut self, id: ObjectId, friend: ObjectId, chance: i32) -> bool {
        let Some(table) = self.skill_table() else {
            return false;
        };
        for key in self.siege_guard_analysis(id).heal_skills {
            let Some(skill) = table.get(key.id, key.level) else {
                continue;
            };
            let mp = self.core_of(id).map_or(0.0, |c| c.cur_mp);
            if mp < f64::from(skill.mp_consume)
                || self.is_skill_disabled(id, skill)
                || !self.check_if_in_range(skill.cast_range, id, friend, true)
            {
                continue;
            }
            if chance >= self.rng().below(100) {
                continue;
            }
            if !self.can_see_target(id, friend) {
                break;
            }
            let old = self.npc_cast_target(id);
            self.set_npc_target(id, Some(friend));
            self.client_stop_moving(id, None);
            self.do_cast(id, key);
            self.restore_npc_target(id, old);
            return true;
        }
        false
    }

    /// Java `SiegeGuardAI.factionNotifyAndSupport()`: heal a hurt defender, or call one idle
    /// member of the guard's clan to the target.
    fn siege_guard_faction_notify_and_support(&mut self, id: ObjectId) {
        let target = self.attack_target_of(id);
        // Call all WorldObject of its Faction inside the Faction Range.
        let no_clans = self.npc_template(id).is_none_or(|t| t.clans.is_empty());
        let Some(target) = target else {
            return;
        };
        if no_clans || self.is_invul(target) {
            return;
        }
        let analysis = self.siege_guard_analysis(id);
        // Go through all Creature that belong to its faction.
        let creatures: Vec<ObjectId> = self
            .visible_objects_in_range(id, FACTION_RANGE)
            .into_iter()
            .filter(|o| self.objects.get(o).is_some_and(|e| e.kind.is_creature()))
            .collect();
        for creature in creatures {
            if !self.is_npc(creature) {
                if analysis.has_heal_or_resurrect
                    && self.is_player(creature)
                    && self.siege_guard_is_defender(id, creature)
                    && self.siege_guard_should_heal(id, creature)
                    && self.siege_guard_heal(id, creature, 5)
                {
                    return;
                }
                continue;
            }
            if !self.is_in_my_clan(creature, id) {
                continue;
            }
            // `npc.getAI() != null`: `getAI()` makes the AI.
            self.wake_ai(creature);
            let target_z = self.position_of(target).map_or(0, |p| p.2);
            let npc_z = self.position_of(creature).map_or(0, |p| p.2);
            if !self.is_dead(creature)
                && target_z.wrapping_sub(npc_z).abs() < 600
                && matches!(self.intention_of(creature), Intention::Idle | Intention::Active)
                && self.dist_3d(target, creature) < 1500.0
                && self.can_see_target(creature, target)
            {
                // Notify the WorldObject AI with AGGRESSION.
                self.notify_action_aggression(creature, target, 1);
                return;
            }
            // heal friends
            if analysis.has_heal_or_resurrect
                && self.siege_guard_should_heal(id, creature)
                && self.siege_guard_heal(id, creature, 4)
            {
                return;
            }
        }
    }

    /// The skill block of `attackPrepare`: a buff or a heal goes on the guard itself (unless it
    /// already has the buff), anything else at its target.
    fn siege_guard_cast(&mut self, id: ObjectId, skill: &SkillTemplate) -> SkillTry {
        let old = self.npc_cast_target(id);
        let heal = self.skill_has_effect_type(skill, &[EffectType::Heal]);
        let buff = is_continuous(skill) && !skill.is_debuff;
        if buff || heal {
            let max_hp = self.stats(id).map_or(0, |v| v.max_hp());
            let hp = self.core_of(id).map_or(0.0, |c| c.cur_hp);
            // `(int) (getMaxHp() / 1.5)`.
            if heal && hp > f64::from((f64::from(max_hp) / 1.5) as i32) {
                return SkillTry::Break;
            }
            let use_skill_self = !(buff && self.is_affected_by_skill(id, skill.id));
            if use_skill_self {
                self.set_npc_target(id, Some(id));
            }
        }
        self.client_stop_moving(id, None);
        self.do_cast(id, SkillKey::of(skill));
        self.restore_npc_target(id, old);
        SkillTry::Cast
    }

    /// `_actor.setTarget(null); setIntentionIdle();`.
    fn siege_guard_give_up(&mut self, id: ObjectId) {
        self.set_npc_target(id, None);
        self.set_intention_idle(id);
    }

    /// Java `SiegeGuardAI.attackPrepare()` / `FortSiegeGuardAI.attackPrepare()`: skills from
    /// afar, walking up to the target (archers and mages keep their distance), the attack.
    fn siege_guard_attack_prepare(&mut self, id: ObjectId) {
        let fort = self.siege_guard_kind_of(id) == SiegeGuardAiKind::Fort;
        // Get all information needed to choose between physical or magical attack.
        let Some(mut target) = self.attack_target_of(id).filter(|t| self.objects.contains_key(t)) else {
            // `catch (NullPointerException e)`.
            self.siege_guard_give_up(id);
            return;
        };
        self.set_npc_target(id, Some(target));
        let skills = self.siege_guard_skills(id);
        let distance = self.dist_2d(id, target);
        let mut range = self.stats(id).map_or(0, |v| v.physical_attack_range())
            + self.collision_radius_of(id)
            + self.collision_radius_of(target);
        if self.is_moving(target) {
            range += 50;
        }
        let analysis = self.siege_guard_analysis(id);

        // never attack defenders
        let hall = self.is_siege_guard_near_hall(id);
        if self.is_player(target) && (fort || !hall) && self.siege_guard_is_defender(id, target) {
            // Cancel the target
            self.aggro_stop_hating(id, target);
            self.siege_guard_give_up(id);
            return;
        }
        if !fort && !self.can_see_target(id, target) {
            // Siege guards differ from normal mobs currently: if target cannot seen, don't
            // attack any more.
            self.aggro_stop_hating(id, target);
            self.siege_guard_give_up(id);
            return;
        }
        let Some(table) = self.skill_table() else {
            return;
        };
        let distance_i = distance;
        let range_f = f64::from(range);
        let muted = self.is_muted(id);
        // Check if the actor isn't muted and if it is far from target.
        if !muted && distance_i > range_f {
            // Check for long ranged skills and heal/buff skills.
            for key in &skills {
                let Some(skill) = table.get(key.id, key.level) else {
                    continue;
                };
                let cast_range = skill.cast_range;
                let mp = self.core_of(id).map_or(0.0, |c| c.cur_mp);
                if distance_i <= f64::from(cast_range)
                    && cast_range > 70
                    && !self.is_skill_disabled(id, skill)
                    && mp >= f64::from(self.mp_consume(id, skill))
                    && !skill.is_passive()
                {
                    match self.siege_guard_cast(id, skill) {
                        SkillTry::Cast => return,
                        SkillTry::Break => break,
                    }
                }
            }
            let run_speed = self.stats(id).map_or(0.0, |v| v.run_speed());
            // Check if the SiegeGuard is attacking, knows the target and can't run.
            if !self.is_attacking_now(id) && run_speed == 0.0 && self.is_in_surrounding_region(id, target) {
                // Cancel the target
                self.siege_guard_give_up(id);
                return;
            }
            // `else if (sGuard.getSpawn() != null)` (the fortress AI has no check and would
            // throw without a spawn).
            let Some(spawn) = self.npcs.get(&id).filter(|n| n.spawn.is_some()).map(|n| n.spawn_point) else {
                return;
            };
            let (Some(me), Some(at)) = (self.position_of(id), self.position_of(target)) else {
                return;
            };
            let dx = f64::from(me.0) - f64::from(at.0);
            let dy = f64::from(me.1) - f64::from(at.1);
            let dz = f64::from(me.2) - f64::from(at.2);
            let home_x = f64::from(at.0) - f64::from(spawn.0);
            let home_y = f64::from(at.1) - f64::from(spawn.1);
            // Check if the SiegeGuard isn't too far from it's home location.
            if (dx * dx) + (dy * dy) > 10_000.0
                && (home_x * home_x) + (home_y * home_y) > HOME_RANGE_SQ
                && self.is_in_surrounding_region(id, target)
            {
                // Cancel the target
                self.siege_guard_give_up(id);
            } else if dz * dz < MAX_DZ_SQ {
                // Temporary hack for preventing guards jumping off towers, before replacing this
                // with effective geodata checks and AI modification.
                if !fort && analysis.is_healer {
                    return;
                }
                if analysis.is_mage {
                    range = analysis.max_cast_range - 50;
                }
                if fort && self.stats(id).is_none_or(|v| v.walk_speed() <= 0.0) {
                    return;
                }
                let offset = if self.is_moving(target) { range - 70 } else { range };
                self.move_to_pawn(id, target, offset);
            }
            return;
        }
        if muted && distance_i > range_f && (fort || !analysis.is_healer) {
            // Temporary hack for preventing guards jumping off towers, before replacing this with
            // effective geodata checks and AI modification.
            let dz = match (self.position_of(id), self.position_of(target)) {
                (Some(me), Some(at)) => f64::from(me.2) - f64::from(at.2),
                _ => return,
            };
            if dz * dz < MAX_DZ_SQ {
                if analysis.is_mage {
                    range = analysis.max_cast_range - 50;
                }
                if fort && self.stats(id).is_none_or(|v| v.walk_speed() <= 0.0) {
                    return;
                }
                let offset = if self.is_moving(target) { range - 70 } else { range };
                self.move_to_pawn(id, target, offset);
            }
            return;
        }
        if distance_i <= range_f {
            let hated = if self.is_confused(id) {
                Some(target)
            } else {
                self.aggro_most_hated(id)
            };
            let Some(hated) = hated else {
                self.set_intention_active(id);
                return;
            };
            if hated != target {
                target = hated;
            }
            self.renew_attack_timeout(id);
            // check for close combat skills && heal/buff skills
            if !self.is_muted(id) && self.rng().below(100) <= 5 {
                for key in &skills {
                    let Some(skill) = table.get(key.id, key.level) else {
                        continue;
                    };
                    let mp = self.core_of(id).map_or(0.0, |c| c.cur_mp);
                    if f64::from(skill.cast_range) >= distance_i
                        && !skill.is_passive()
                        && mp >= f64::from(self.mp_consume(id, skill))
                        && !self.is_skill_disabled(id, skill)
                    {
                        match self.siege_guard_cast(id, skill) {
                            SkillTry::Cast => return,
                            SkillTry::Break => break,
                        }
                    }
                }
            }
            // Finally, do the physical attack itself.
            if fort || !analysis.is_healer {
                self.do_attack(id, target);
            }
        }
    }

    /// `sGuard.getConquerableHall() != null`: a siegable hall within 10000.
    fn is_siege_guard_near_hall(&self, id: ObjectId) -> bool {
        self.position_of(id)
            .is_some_and(|(x, y, _)| self.nearby_siegable_hall(x, y, RESIDENCE_RANGE).is_some())
    }

    // ------------------------------------------------------------------ intentions and events

    /// Java `SiegeGuardAI.setIntentionIdle()`: active again while a player sees the guard, else
    /// idle, the AI task stops and the AI goes.
    pub(crate) fn siege_guard_set_intention_idle(&mut self, id: ObjectId) {
        // `shouldPromoteIdleToActive()`.
        if !self.is_alike_dead(id) && self.has_visible_player(id) {
            self.set_intention_active(id);
            return;
        }
        self.creature_set_intention_idle(id);
        // Stop AI task and detach AI from NPC.
        self.stop_ai_task(id);
        // Cancel the AI
        self.detach_ai(id);
    }

    /// Java `SiegeGuardAI.notifyActionAttacked(attacker)` before `CreatureAI`'s.
    pub(crate) fn siege_guard_notify_attacked(&mut self, id: ObjectId, attacker: ObjectId) {
        // Calculate the attack timeout.
        self.renew_attack_timeout(id);
        // Set the _globalAggro to 0 to permit attack even just after spawn.
        if let Some(state) = self.ai.creatures.get_mut(&id)
            && state.global_aggro < 0
        {
            state.global_aggro = 0;
        }
        // Add the attacker to the _aggroList of the actor.
        self.add_damage_hate(id, attacker, 0, 1);
        // Set the Creature movement type to run and send Server->Client packet ChangeMoveType to
        // all others Player.
        if !self.is_running(id) {
            self.set_running(id, true);
        }
        // Set the Intention to ATTACK.
        if self.intention_of(id) != Intention::Attack {
            self.set_intention_attack(id, attacker);
        }
    }

    /// Java `SiegeGuardAI.notifyActionAggression(target, aggro)` with a target. The branch
    /// without one ("currently only for setting lower general aggro") has no caller yet.
    pub(crate) fn siege_guard_notify_aggression(&mut self, id: ObjectId, target: ObjectId, aggro: i64) {
        if !self.objects.get(&target).is_some_and(|e| e.kind.is_creature()) {
            return;
        }
        // Add the target to the actor _aggroList or update hate if already present.
        self.add_damage_hate(id, target, 0, aggro);
        // Get the hate of the actor against the target.
        if self.aggro_hating(id, target) <= 0 {
            if self.aggro_most_hated(id).is_none() {
                if let Some(state) = self.ai.creatures.get_mut(&id) {
                    state.global_aggro = LOST_GLOBAL_AGGRO;
                }
                self.aggro_clear(id);
                self.set_intention_idle(id);
            }
            return;
        }
        // Set the actor AI Intention to ATTACK.
        if self.intention_of(id) != Intention::Attack {
            // Set the Creature movement type to run and send Server->Client packet
            // ChangeMoveType to all others Player.
            if !self.is_running(id) {
                self.set_running(id, true);
            }
            // NOTE(java-quirk): Java reads `getSpawn()` without a check; a guard without a spawn
            // throws there and does nothing here.
            let Some(spawn) = self.npcs.get(&id).filter(|n| n.spawn.is_some()).map(|n| n.spawn_point) else {
                return;
            };
            let Some(at) = self.position_of(target) else {
                return;
            };
            let home_x = f64::from(at.0) - f64::from(spawn.0);
            let home_y = f64::from(at.1) - f64::from(spawn.1);
            // Check if the SiegeGuard is not too far from its home location.
            if (home_x * home_x) + (home_y * home_y) < HOME_RANGE_SQ {
                self.set_intention_attack(id, target);
            }
        }
    }
}
