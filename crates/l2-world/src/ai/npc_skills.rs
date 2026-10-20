//! The skill side of `AttackableAI`: the AI skill lists of an NPC template (`AISkillScope`), the
//! chance rules, and every place where the think loop casts (buffs while active, suicide, heal,
//! resurrection, short and long range skills, the immobilised branch, `thinkCast`).
// Java: ai/AttackableAI.java (thinkActive/thinkAttack skill parts, cast, tryCast,
//       checkSkillCastConditions, effectTargetReconsider, skillTargetReconsider, thinkCast,
//       allowsParameterizedCast)
// Java: ai/CreatureAI.java (canAura, canAOE, canParty, isParty)
// Java: data/xml/NpcData.java (the skill list -> AISkillScope lists)
// Java: entity/actor/Npc.java (hasSkillChance, isInMyClan), util/LocationUtil.java (checkIfInRange)
//
// # How the AI casts
// An NPC casts through the casting entry points of `skills/cast.rs`, the way Java's AI does:
// `npc.setTarget(target)` is [`World::set_npc_target`] and `npc.doCast(skill)` is
// [`World::do_cast`]. The AI's own target (`AbstractAI._target`) and the creature's target
// (`Creature._target`) are two fields in Java; here the AI's is `PawnState::ai_target`, and the
// creature's is the `target` of the NPC's control state, which `do_cast` reads first. The pieces
// that "put the old target back" (`final WorldObject target = npc.getTarget(); ...;
// npc.setTarget(target)`) read and restore exactly that field ([`World::npc_cast_target`]).
//
// # The AI skill lists
// Java's `NpcData` sorts every non-passive skill of the template into scopes when the template
// is loaded. The scope of a skill depends on its effect types, which only the world's effect
// registry knows, so the lists are built here, once per NPC id, the first time such an NPC
// thinks (`AttackableWorld::ai_skills`), in the order of the template's skill map (Java's
// `HashMap`). An NPC id without skills costs one template lookup per think.
//
// # Left out
// - Guards (`Guard` is not an attackable class in `NpcClass` yet) and summons (M6) as targets.
// - Scripts: the NPC AI parameters that only scripts read (D-05).

use std::collections::BTreeMap;
use std::sync::Arc;

use l2_data::npc::{AiType, NpcParam};
use l2_data::skill::TargetType;
use l2_data::stats::AbnormalType;
use l2_data::{SkillTemplate, SkillTemplates};
use l2_util::ObjectId;

use crate::effects::{EffectType, SkillKey};
use crate::movement::Intention;
use crate::skills::checks::is_continuous;
use crate::stats::skills::java_hash_map_order;
use crate::world::World;

/// `AttackableAI.SKILL_PROBABILITY_SCALE`.
const SKILL_PROBABILITY_SCALE: i32 = 10_000;
/// `AttackableAI.MAX_PARAMETERIZED_SKILL_SLOTS`.
const MAX_PARAMETERIZED_SKILL_SLOTS: i32 = 6;
/// The range up to which the short range skills are tried.
const SHORT_RANGE: f64 = 150.0;

/// Java `AISkillScope`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiSkillScope {
    Buff,
    Debuff,
    Negative,
    Attack,
    Immobilize,
    Heal,
    Res,
    Cot,
    Universal,
    LongRange,
    ShortRange,
    General,
    Suicide,
}

impl AiSkillScope {
    const COUNT: usize = 13;
}

/// Java `NpcTemplate._aiSkillLists`: the skills of an NPC template by scope, in list order.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct NpcAiSkills {
    lists: [Vec<SkillKey>; AiSkillScope::COUNT],
}

impl NpcAiSkills {
    /// Java `NpcTemplate.getAISkills(scope)`.
    pub fn get(&self, scope: AiSkillScope) -> &[SkillKey] {
        &self.lists[scope as usize]
    }

    /// Whether no scope has a skill.
    pub fn is_empty(&self) -> bool {
        self.lists.iter().all(Vec::is_empty)
    }

    fn push(&mut self, scope: AiSkillScope, key: SkillKey) {
        self.lists[scope as usize].push(key);
    }
}

/// The AI skill lists built so far, by NPC id (`None`: the template has no active skill).
#[derive(Debug, Default)]
pub struct AiSkillCache {
    /// The skill table the lists were built from; a new table clears the cache.
    table: usize,
    by_npc: std::collections::HashMap<i32, Option<Arc<NpcAiSkills>>>,
}

fn is_aura(target_type: TargetType) -> bool {
    matches!(
        target_type,
        TargetType::Aura | TargetType::BehindAura | TargetType::FrontAura | TargetType::AuraCorpseMob
    )
}

/// `AURA`, `BEHIND_AURA`, `FRONT_AURA` (without `AURA_CORPSE_MOB`).
fn is_aura_plain(target_type: TargetType) -> bool {
    matches!(
        target_type,
        TargetType::Aura | TargetType::BehindAura | TargetType::FrontAura
    )
}

fn is_area(target_type: TargetType) -> bool {
    matches!(
        target_type,
        TargetType::Area | TargetType::BehindArea | TargetType::FrontArea
    )
}

/// Java `Skill.getCastRange() <= 150 ? SHORT_RANGE : LONG_RANGE`.
fn short_or_long(skill: &SkillTemplate) -> AiSkillScope {
    if skill.cast_range <= 150 {
        AiSkillScope::ShortRange
    } else {
        AiSkillScope::LongRange
    }
}

/// `StatSet.getInt(key, default)` of an NPC parameter (`Integer.parseInt`, no trimming).
// NOTE(java-quirk): a value that is no integer makes Java throw out of the think; here it reads as
// the default.
fn param_int(params: &BTreeMap<String, NpcParam>, key: &str, default: i32) -> i32 {
    match params.get(key) {
        Some(NpcParam::Text(text)) => text.parse().unwrap_or(default),
        _ => default,
    }
}

impl World {
    // ------------------------------------------------------------------ the skill lists

    /// Java `npc.getTemplate().getAISkills(..)` for all scopes: the lists of an NPC, built on
    /// first use. `None` when the NPC has no active skill (the common case, one lookup).
    pub(crate) fn ai_skills_of(&mut self, id: ObjectId) -> Option<Arc<NpcAiSkills>> {
        let npc_id = self.npcs.get(&id)?.npc_id;
        if self.npc_world.template(npc_id)?.skills.is_empty() {
            return None;
        }
        let table = self.skill_table()?;
        let table_ptr = Arc::as_ptr(&table) as usize;
        let cache = &mut self.ai.attackable.ai_skills;
        if cache.table != table_ptr {
            cache.table = table_ptr;
            cache.by_npc.clear();
        }
        if let Some(cached) = cache.by_npc.get(&npc_id) {
            return cached.clone();
        }
        let built = self.build_ai_skills(&table, npc_id);
        self.ai.attackable.ai_skills.by_npc.insert(npc_id, built.clone());
        built
    }

    /// Java `npc.getTemplate().getAISkills(scope)`, as skill ids (empty for an NPC without an
    /// active skill).
    pub fn ai_skill_ids(&mut self, npc: ObjectId, scope: AiSkillScope) -> Vec<i32> {
        self.ai_skills_of(npc)
            .map_or_else(Vec::new, |lists| lists.get(scope).iter().map(|k| k.id).collect())
    }

    /// The loop of `NpcData` that fills `aiSkillLists` from the template's skill map.
    fn build_ai_skills(&mut self, table: &SkillTemplates, npc_id: i32) -> Option<Arc<NpcAiSkills>> {
        let refs = self.npc_world.template(npc_id)?.skills.clone();
        // `skills.put(skill.getId(), skill)`: the last level of each id, in HashMap order.
        let mut order: Vec<i32> = Vec::new();
        let mut levels: std::collections::HashMap<i32, i32> = std::collections::HashMap::new();
        for s in &refs {
            let Some(skill) = table.get(s.id, s.level) else {
                continue;
            };
            if levels.insert(skill.id, skill.level).is_none() {
                order.push(skill.id);
            }
        }
        let mut lists = NpcAiSkills::default();
        for skill_id in java_hash_map_order(&order) {
            let Some(&level) = levels.get(&skill_id) else {
                continue;
            };
            let Some(skill) = table.get(skill_id, level) else {
                continue;
            };
            if skill.is_passive() {
                continue;
            }
            for scope in self.ai_skill_scopes(skill) {
                lists.push(scope, SkillKey::of(skill));
            }
        }
        if lists.is_empty() {
            return None;
        }
        Some(Arc::new(lists))
    }

    /// The scopes one skill belongs to (`NpcData`'s `aiSkillScopes`).
    fn ai_skill_scopes(&mut self, skill: &SkillTemplate) -> Vec<AiSkillScope> {
        let mut scopes = Vec::new();
        let range_scope = short_or_long(skill);
        if skill.is_suicide_attack {
            scopes.push(AiSkillScope::Suicide);
            return scopes;
        }
        scopes.push(AiSkillScope::General);
        if is_continuous(skill) {
            if !skill.is_debuff {
                scopes.push(AiSkillScope::Buff);
            } else {
                scopes.push(AiSkillScope::Debuff);
                scopes.push(AiSkillScope::Cot);
                scopes.push(range_scope);
            }
        } else if self.skill_has_effect_type(skill, &[EffectType::Dispel, EffectType::DispelBySlot]) {
            scopes.push(AiSkillScope::Negative);
            scopes.push(range_scope);
        } else if self.skill_has_effect_type(skill, &[EffectType::Heal]) {
            scopes.push(AiSkillScope::Heal);
        } else if self.skill_has_effect_type(
            skill,
            &[
                EffectType::PhysicalAttack,
                EffectType::PhysicalAttackHpLink,
                EffectType::MagicalAttack,
                EffectType::DeathLink,
                EffectType::HpDrain,
            ],
        ) {
            scopes.push(AiSkillScope::Attack);
            scopes.push(AiSkillScope::Universal);
            scopes.push(range_scope);
        } else if self.skill_has_effect_type(skill, &[EffectType::Sleep]) {
            scopes.push(AiSkillScope::Immobilize);
        } else if self.skill_has_effect_type(skill, &[EffectType::Stun, EffectType::Root]) {
            scopes.push(AiSkillScope::Immobilize);
            scopes.push(range_scope);
        } else if self.skill_has_effect_type(skill, &[EffectType::Mute, EffectType::Fear]) {
            scopes.push(AiSkillScope::Cot);
            scopes.push(range_scope);
        } else if self.skill_has_effect_type(skill, &[EffectType::Paralyze]) {
            scopes.push(AiSkillScope::Immobilize);
            scopes.push(range_scope);
        } else if self
            .skill_has_effect_type(skill, &[EffectType::DmgOverTime, EffectType::DmgOverTimePercent])
        {
            scopes.push(range_scope);
        } else if self.skill_has_effect_type(skill, &[EffectType::Resurrection]) {
            scopes.push(AiSkillScope::Res);
        } else {
            scopes.push(AiSkillScope::Universal);
        }
        scopes
    }

    // ------------------------------------------------------------------ small helpers

    /// Java `Creature.getTarget()` of an NPC: the target its skills set (not the AI's).
    pub(crate) fn npc_cast_target(&self, id: ObjectId) -> Option<ObjectId> {
        self.control(id).and_then(|c| c.target)
    }

    /// `npc.setTarget(old)` after a cast: puts the creature's own target back, `None` included
    /// (which, unlike `set_creature_target(None)`, leaves the AI's target alone).
    fn restore_npc_target(&mut self, id: ObjectId, old: Option<ObjectId>) {
        match old {
            Some(target) => self.set_npc_target(id, Some(target)),
            None => {
                if let Some(c) = self.control_mut(id) {
                    c.target = None;
                }
            }
        }
    }

    /// `clientStopMoving(null)`, `npc.setTarget(target)`, `npc.doCast(skill)`, `npc.setTarget(back)`.
    fn cast_at_and_restore(&mut self, id: ObjectId, target: ObjectId, skill: &SkillTemplate, back: ObjectId) {
        self.client_stop_moving(id, None);
        self.set_npc_target(id, Some(target));
        self.do_cast(id, SkillKey::of(skill));
        self.set_npc_target(id, Some(back));
    }

    /// `clientStopMoving(null)`, `npc.doCast(skill)`: the cast at whatever the NPC targets.
    fn cast_now(&mut self, id: ObjectId, skill: &SkillTemplate) {
        self.client_stop_moving(id, None);
        self.do_cast(id, SkillKey::of(skill));
    }

    /// Java `Attackable.isMinion()`.
    fn is_minion(&self, id: ObjectId) -> bool {
        self.leader_of(id).is_some()
    }

    /// `getCurrentHp() / getMaxHp()` as Java computes it (a double).
    fn hp_fraction(&self, id: ObjectId) -> f64 {
        let cur = self.core_of(id).map_or(0.0, |c| c.cur_hp);
        let max = f64::from(self.stats(id).map_or(0, |s| s.max_hp()));
        cur / max
    }

    fn is_chaos(&self, id: ObjectId) -> bool {
        self.npc_template(id).is_some_and(|t| t.is_chaos)
    }

    /// Java `Npc.isInMyClan(other)`: `getTemplate().isClan(other.getTemplate().getClans())`. An
    /// empty clan set is Java's `null`.
    fn is_in_my_clan(&self, me: ObjectId, other: ObjectId) -> bool {
        let (Some(mine), Some(theirs)) = (self.npc_template(me), self.npc_template(other)) else {
            return false;
        };
        if mine.clans.is_empty() || theirs.clans.is_empty() {
            return false;
        }
        let generic = self.npc_world.generic_clan_id();
        mine.clans.contains(&generic) || theirs.clans.iter().any(|c| mine.clans.contains(c))
    }

    /// The creatures `World.getFirstVisibleObjectInRange(npc, Creature.class, range, ..)` goes
    /// through.
    fn ai_visible_creatures_in_range(&self, id: ObjectId, range: i32) -> Vec<ObjectId> {
        self.visible_objects_in_range(id, range)
            .into_iter()
            .filter(|o| self.objects.get(o).is_some_and(|e| e.kind.is_creature()))
            .collect()
    }

    /// The same for `Attackable.class`.
    fn ai_visible_attackables_in_range(&self, id: ObjectId, range: i32) -> Vec<ObjectId> {
        self.visible_objects_in_range(id, range)
            .into_iter()
            .filter(|o| self.is_attackable_npc(*o))
            .collect()
    }

    /// Java `Npc.hasSkillChance()`: `Rnd.get(100) < Rnd.get(minSkillChance, maxSkillChance)`.
    pub(crate) fn has_skill_chance(&mut self, id: ObjectId) -> bool {
        let (min, max) = self
            .npc_template(id)
            .map_or((0, 0), |t| (t.min_skill_chance, t.max_skill_chance));
        let roll = self.rng().below(100);
        roll < self.rng().get(min, max)
    }

    /// Java `AttackableAI.checkSkillCastConditions(caster, skill)`.
    fn check_skill_cast_conditions(&mut self, caster: ObjectId, skill: &SkillTemplate) -> bool {
        if self.is_casting_now(caster) && !skill.simultaneous_cast {
            return false;
        }
        // Not enough MP.
        let mp = self.core_of(caster).map_or(0.0, |c| c.cur_mp);
        if f64::from(skill.mp_consume) >= mp {
            return false;
        }
        // Character is in "skill disabled" mode.
        if self.is_skill_disabled(caster, skill) {
            return false;
        }
        // If is a static skill and magic skill and character is muted or is a physical skill
        // muted and character is physically muted.
        if !skill.is_static()
            && ((skill.is_magic() && self.is_muted(caster)) || self.is_physical_muted(caster))
        {
            return false;
        }
        true
    }

    // ------------------------------------------------------------------ parameterized skills

    /// Java `AttackableAI.allowsParameterizedCast(caster, skill, target)`: the `Skill0n_*`
    /// parameters of the template gate the skills they declare.
    fn allows_parameterized_cast(
        &mut self,
        caster: ObjectId,
        skill: &SkillTemplate,
        target: Option<ObjectId>,
    ) -> bool {
        let Some(template) = self.npc_template(caster) else {
            return true;
        };
        if template.parameters.is_empty() {
            return true;
        }
        let slot = (1..=MAX_PARAMETERIZED_SKILL_SLOTS).find(|slot| {
            matches!(
                template.parameters.get(&format!("Skill0{slot}_ID")),
                Some(NpcParam::Skill { id, .. }) if *id == skill.id
            )
        });
        let Some(slot) = slot else {
            return true;
        };
        let prefix = format!("Skill0{slot}_");
        let p = &template.parameters;
        // Retail AI classes spell this parameter three different ways, all present in npcdata.
        let probability = {
            let prob = param_int(p, &format!("{prefix}Prob"), 0);
            let probability = param_int(p, &format!("{prefix}Probability"), prob);
            param_int(p, &format!("{prefix}Probablity"), probability)
        };
        let checks_distance = param_int(p, &format!("{prefix}Check_Dist"), 0) == 1;
        let high_hp = param_int(p, &format!("{prefix}HighHP"), 0);
        let has_attack_hint = param_int(p, &format!("{prefix}AttackSplash"), 0) == 1
            || param_int(p, &format!("{prefix}MainAttack"), 0) == 1
            || param_int(p, &format!("{prefix}Target"), 0) > 0;
        let dist_min = param_int(p, &format!("{prefix}Dist_Min"), 0);
        let dist_max = param_int(p, &format!("{prefix}Dist_Max"), i32::MAX);
        let hp_target = param_int(p, &format!("{prefix}HPTarget"), 0);

        if probability == 0 {
            if !checks_distance && high_hp == 0 && !has_attack_hint {
                return false;
            }
        } else if self.rng().below(SKILL_PROBABILITY_SCALE) >= probability {
            return false;
        }

        if checks_distance && let Some(target) = target {
            let distance = self.dist_2d(caster, target) as i32;
            if distance < dist_min || distance > dist_max {
                return false;
            }
        }

        if high_hp > 0 {
            let source = if hp_target == 1 { Some(caster) } else { target };
            let Some(source) = source else {
                return false;
            };
            let percent = self.hp_fraction(source) * 100.0;
            if percent <= f64::from(high_hp) {
                return false;
            }
        }
        true
    }

    // ------------------------------------------------------------------ CreatureAI helpers

    /// Java `CreatureAI.isParty(skill)`.
    fn is_party(skill: &SkillTemplate) -> bool {
        skill.target_type == TargetType::Party
    }

    /// Java `CreatureAI.canAura(skill)`: an aura skill whose area holds the attack target.
    fn can_aura(&self, id: ObjectId, skill: &SkillTemplate) -> bool {
        if !is_aura(skill.target_type) {
            return false;
        }
        let Some(target) = self.attack_target_of(id) else {
            return false;
        };
        self.ai_visible_creatures_in_range(id, skill.affect_range)
            .contains(&target)
    }

    /// Java `CreatureAI.canAOE(skill)`.
    fn can_aoe(&mut self, id: ObjectId, skill: &SkillTemplate) -> bool {
        let chaos = self.is_chaos(id);
        let target_type = skill.target_type;
        let usable = |world: &World, target: ObjectId| {
            world.can_see_target(id, target) && !(world.is_attackable_npc(target) && !chaos)
        };
        if self.skill_has_effect_type(skill, &[EffectType::Dispel, EffectType::DispelBySlot]) {
            if is_aura(target_type) {
                let cancast = !self
                    .ai_visible_creatures_in_range(id, skill.affect_range)
                    .into_iter()
                    .any(|t| usable(self, t) && self.is_affected_by_skill(t, skill.id));
                return cancast;
            }
            if is_area(target_type) {
                // NOTE(java-quirk): `cancast` starts `true` and the loop only ever sets it `true`.
                return true;
            }
            return false;
        }
        if is_aura(target_type) {
            return self
                .ai_visible_creatures_in_range(id, skill.affect_range)
                .into_iter()
                .any(|t| usable(self, t) && self.effect_list(t).is_some_and(|l| !l.is_empty()));
        }
        if is_area(target_type) {
            let Some(attack_target) = self.attack_target_of(id) else {
                return false;
            };
            return !self
                .ai_visible_creatures_in_range(attack_target, skill.affect_range)
                .into_iter()
                .any(|t| usable(self, t) && self.is_affected_by_skill(t, skill.id));
        }
        false
    }

    /// Java `CreatureAI.canParty(skill)`.
    fn can_party(&self, id: ObjectId, skill: &SkillTemplate) -> bool {
        if !Self::is_party(skill) {
            return false;
        }
        let mut count = 0;
        let mut affected = 0;
        for target in self.ai_visible_attackables_in_range(id, skill.affect_range) {
            if !self.can_see_target(id, target) {
                continue;
            }
            if self.is_in_my_clan(target, id) {
                count += 1;
                if self.is_affected_by_skill(target, skill.id) {
                    affected += 1;
                }
            }
        }
        affected < count
    }

    // ------------------------------------------------------------------ target reconsider

    /// Java `AttackableAI.effectTargetReconsider(skill, positive)`.
    fn effect_target_reconsider(
        &mut self,
        actor: ObjectId,
        skill: &SkillTemplate,
        positive: bool,
    ) -> Option<ObjectId> {
        let attack_target = self.attack_target_of(actor)?;
        let actor_collision = self.collision_radius_of(actor);
        let target_collision = self.collision_radius_of(attack_target);
        let is_dispel = self.skill_has_effect_type(skill, &[EffectType::Dispel, EffectType::DispelBySlot]);
        if !is_dispel {
            if !positive {
                let mut range = 0;
                for obj in self.attack_by_list(actor).to_vec() {
                    if self.is_dead(obj) || !self.can_see_target(actor, obj) || obj == attack_target {
                        continue;
                    }
                    self.set_npc_target(actor, Some(attack_target));
                    let dist = self.dist_2d(actor, obj);
                    let mut dist2 = dist - f64::from(actor_collision);
                    range = skill.cast_range + actor_collision + self.collision_radius_of(obj);
                    if self.is_moving(obj) {
                        dist2 -= 70.0;
                    }
                    if dist2 <= f64::from(range) && !self.is_affected_by_skill(attack_target, skill.id) {
                        return Some(obj);
                    }
                }

                // If there is nearby Target with aggro, start going on random target that is
                // attackable.
                // NOTE(java-quirk): `range` is whatever the loop above computed last, 0 when
                // nobody attacked the NPC.
                for obj in self.ai_visible_creatures_in_range(actor, range) {
                    if self.is_dead(obj) || !self.can_see_target(actor, obj) {
                        continue;
                    }
                    self.set_npc_target(actor, Some(attack_target));
                    let local_dist = self.dist_2d(actor, obj);
                    let local_range = skill.cast_range + actor_collision + self.collision_radius_of(obj);
                    let local_dist2 = if self.is_moving(obj) {
                        local_dist - 70.0
                    } else {
                        local_dist
                    };
                    // (`obj.isPlayer() || obj.isSummon()`: summons are M6.)
                    if self.is_player(obj)
                        && local_dist2 <= f64::from(local_range)
                        && !self.is_affected_by_skill(attack_target, skill.id)
                    {
                        return Some(obj);
                    }
                }
            } else {
                let range = skill.cast_range + actor_collision;
                for target in self.ai_visible_attackables_in_range(actor, range) {
                    if self.is_dead(target)
                        || !self.can_see_target(actor, target)
                        || !self.is_in_my_clan(target, actor)
                    {
                        continue;
                    }
                    self.set_npc_target(actor, Some(attack_target));
                    let local_dist = self.dist_2d(actor, target);
                    let local_range = skill.cast_range + actor_collision + self.collision_radius_of(target);
                    let base = local_dist - f64::from(actor_collision);
                    let local_dist2 = if self.is_moving(target) { base - 70.0 } else { base };
                    if local_dist2 <= f64::from(local_range) && !self.is_affected_by_skill(target, skill.id) {
                        return Some(target);
                    }
                }
            }
        } else {
            let range = skill.cast_range + actor_collision + target_collision;
            for obj in self.ai_visible_creatures_in_range(actor, range) {
                if self.is_dead(obj) || !self.can_see_target(actor, obj) {
                    continue;
                }
                self.set_npc_target(actor, Some(attack_target));
                let local_dist = self.dist_2d(actor, obj);
                let local_range = skill.cast_range + actor_collision + self.collision_radius_of(obj);
                let base = local_dist - f64::from(actor_collision);
                let local_dist2 = if self.is_moving(obj) { base - 70.0 } else { base };
                if self.is_player(obj)
                    && local_dist2 <= f64::from(local_range)
                    && self.is_affected_by_effect_type(attack_target, EffectType::Buff)
                {
                    return Some(obj);
                }
            }
        }
        None
    }

    /// Java `AttackableAI.skillTargetReconsider(skill)`.
    fn skill_target_reconsider(&mut self, actor: ObjectId, skill: &SkillTemplate) -> Option<ObjectId> {
        let attack_target = self.attack_target_of(actor);
        let actor_collision = self.collision_radius_of(actor);
        if let Some(hate_list) = self.aggro_hate_list(actor) {
            for obj in hate_list {
                if !self.can_see_target(actor, obj) || self.is_dead(obj) {
                    continue;
                }
                // (`getAttackTarget()` null throws a NullPointerException that skips the entry.)
                let Some(attack_target) = attack_target else {
                    continue;
                };
                self.set_npc_target(actor, Some(attack_target));
                let dist = self.dist_2d(actor, obj);
                let dist2 = dist - f64::from(actor_collision);
                let range = skill.cast_range + actor_collision + self.collision_radius_of(attack_target);
                if dist2 <= f64::from(range) {
                    return Some(obj);
                }
            }
        }

        // (`!(actor instanceof Guard)`: guards are not attackable here.)
        let chaos = self.is_chaos(actor);
        for obj in self.ai_visible_creatures(actor) {
            let Some(attack_target) = attack_target else {
                continue;
            };
            self.set_npc_target(actor, Some(attack_target));
            let local_dist = self.dist_2d(actor, obj);
            let local_range = skill.cast_range + actor_collision + self.collision_radius_of(attack_target);
            if local_dist > f64::from(local_range) {
                continue;
            }
            if !self.can_see_target(actor, obj) {
                continue;
            }
            if self.is_player(obj) {
                return Some(obj);
            }
            if self.is_attackable_npc(obj) && chaos {
                if !self.is_in_my_clan(obj, actor) {
                    return Some(obj);
                }
                continue;
            }
            // (`obj.isSummon()`: summons are M6.)
        }
        None
    }

    /// Every creature `World.getFirstVisibleObject(actor, Creature.class, ..)` goes through.
    fn ai_visible_creatures(&self, id: ObjectId) -> Vec<ObjectId> {
        self.visible_objects(id)
            .into_iter()
            .filter(|o| self.objects.get(o).is_some_and(|e| e.kind.is_creature()))
            .collect()
    }

    // ------------------------------------------------------------------ cast(skill)

    /// Java `AttackableAI.cast(skill)`: tries to use the skill on the attack target (or around
    /// it); `true` when the NPC started a cast.
    pub(crate) fn ai_cast(&mut self, caster: ObjectId, skill: &SkillTemplate) -> bool {
        if !self.check_skill_cast_conditions(caster, skill) {
            return false;
        }
        let first_target = self.attack_target_of(caster);
        if !self.allows_parameterized_cast(caster, skill, first_target) {
            return false;
        }
        if first_target.is_none()
            && let Some(most_hated) = self.aggro_most_hated(caster)
        {
            self.set_attack_target(caster, Some(most_hated));
        }
        let Some(attack_target) = self.attack_target_of(caster) else {
            return false;
        };

        let dist = self.dist_2d(caster, attack_target);
        let mut dist2 = dist - f64::from(self.collision_radius_of(attack_target));
        let srange = f64::from(skill.cast_range + self.collision_radius_of(caster));
        if self.is_moving(attack_target) {
            dist2 -= 30.0;
        }
        let target_type = skill.target_type;
        let caster_collision = self.collision_radius_of(caster);

        if is_continuous(skill) {
            if !skill.is_debuff {
                if !self.is_affected_by_skill(caster, skill.id) {
                    self.cast_at_and_restore(caster, caster, skill, attack_target);
                    return true;
                }

                // If actor already have buff, start looking at others same faction mob to cast.
                if target_type == TargetType::SelfTarget {
                    return false;
                }

                if target_type == TargetType::One
                    && let Some(target) = self.effect_target_reconsider(caster, skill, true)
                {
                    self.cast_at_and_restore(caster, target, skill, attack_target);
                    return true;
                }

                if self.can_party(caster, skill) {
                    self.cast_at_and_restore(caster, caster, skill, attack_target);
                    return true;
                }
            } else {
                let aoe = self.can_aoe(caster, skill);
                if self.can_see_target(caster, attack_target)
                    && !aoe
                    && !self.is_dead(attack_target)
                    && dist2 <= srange
                {
                    if !self.is_affected_by_skill(attack_target, skill.id) {
                        self.cast_now(caster, skill);
                        return true;
                    }
                } else if aoe {
                    if is_aura(target_type) {
                        self.cast_now(caster, skill);
                        return true;
                    }
                    if is_area(target_type)
                        && self.can_see_target(caster, attack_target)
                        && !self.is_dead(attack_target)
                        && dist2 <= srange
                    {
                        self.cast_now(caster, skill);
                        return true;
                    }
                } else if target_type == TargetType::One
                    && self.effect_target_reconsider(caster, skill, false).is_some()
                {
                    self.cast_now(caster, skill);
                    return true;
                }
            }
        }

        if self.skill_has_effect_type(skill, &[EffectType::Dispel, EffectType::DispelBySlot]) {
            if target_type == TargetType::One {
                if self.is_affected_by_effect_type(attack_target, EffectType::Buff)
                    && self.can_see_target(caster, attack_target)
                    && !self.is_dead(attack_target)
                    && dist2 <= srange
                {
                    self.cast_now(caster, skill);
                    return true;
                }

                if let Some(target) = self.effect_target_reconsider(caster, skill, false) {
                    self.cast_at_and_restore(caster, target, skill, attack_target);
                    return true;
                }
            } else if self.can_aoe(caster, skill) {
                // (Java writes the aura and the area case as two branches with the same body.)
                let sees = self.can_see_target(caster, attack_target);
                if (is_aura_plain(target_type) && sees)
                    || (is_area(target_type) && sees && !self.is_dead(attack_target) && dist2 <= srange)
                {
                    self.cast_now(caster, skill);
                    return true;
                }
            }
        }

        if self.skill_has_effect_type(skill, &[EffectType::Heal]) {
            if self.is_minion(caster)
                && target_type != TargetType::SelfTarget
                && let Some(leader) = self.leader_of(caster)
                && !self.is_dead(leader)
                && f64::from(self.rng().below(100)) > self.hp_fraction(leader) * 100.0
            {
                let reach = skill.cast_range + caster_collision + self.collision_radius_of(leader);
                if !self.check_if_in_range(reach, caster, leader, false)
                    && !Self::is_party(skill)
                    && !self.is_movement_disabled(caster)
                {
                    self.move_to_pawn(caster, leader, reach);
                }

                if self.can_see_target(caster, leader) {
                    self.cast_at_and_restore(caster, leader, skill, attack_target);
                    return true;
                }
            }

            let percentage = self.hp_fraction(caster) * 100.0;
            if f64::from(self.rng().below(100)) < (100.0 - percentage) / 3.0 {
                self.cast_at_and_restore(caster, caster, skill, attack_target);
                return true;
            }

            if target_type == TargetType::One {
                let range = skill.cast_range + caster_collision;
                let mut found = None;
                for obj in self.ai_visible_attackables_in_range(caster, range) {
                    if !self.is_dead(obj)
                        && self.is_in_my_clan(caster, obj)
                        && f64::from(self.rng().below(100)) < (100.0 - self.hp_fraction(obj) * 100.0) / 10.0
                        && self.can_see_target(caster, obj)
                    {
                        found = Some(obj);
                        break;
                    }
                }
                if let Some(heal_target) = found {
                    self.cast_at_and_restore(caster, heal_target, skill, attack_target);
                    return true;
                }
            }

            if Self::is_party(skill) {
                let range = skill.affect_range + caster_collision;
                let mut found = false;
                for obj in self.ai_visible_attackables_in_range(caster, range) {
                    if self.is_in_my_clan(obj, caster)
                        && self.hp_below_max(obj)
                        && self.rng().below(100) <= 20
                    {
                        found = true;
                        break;
                    }
                }
                if found {
                    self.cast_at_and_restore(caster, caster, skill, attack_target);
                    return true;
                }
            }
        }

        if self.skill_has_effect_type(
            skill,
            &[
                EffectType::PhysicalAttack,
                EffectType::PhysicalAttackHpLink,
                EffectType::MagicalAttack,
                EffectType::DeathLink,
                EffectType::HpDrain,
            ],
        ) {
            if !self.can_aura(caster, skill) {
                if self.can_see_target(caster, attack_target)
                    && !self.is_dead(attack_target)
                    && dist2 <= srange
                {
                    self.cast_now(caster, skill);
                    return true;
                }

                if let Some(target) = self.skill_target_reconsider(caster, skill) {
                    self.cast_at_and_restore(caster, target, skill, attack_target);
                    return true;
                }
            } else {
                self.cast_now(caster, skill);
                return true;
            }
        }

        if self.skill_has_effect_type(skill, &[EffectType::Sleep]) {
            if target_type == TargetType::One {
                let range = self.stats(caster).map_or(0, |s| s.physical_attack_range())
                    + caster_collision
                    + self.collision_radius_of(attack_target);
                if !self.is_dead(attack_target)
                    && dist2 <= srange
                    && (dist2 > f64::from(range) || self.is_moving(attack_target))
                    && !self.is_affected_by_skill(attack_target, skill.id)
                {
                    self.cast_now(caster, skill);
                    return true;
                }

                if self.effect_target_reconsider(caster, skill, false).is_some() {
                    self.cast_now(caster, skill);
                    return true;
                }
            } else if self.can_aoe(caster, skill) {
                if is_aura_plain(target_type) {
                    self.cast_now(caster, skill);
                    return true;
                }

                if is_area(target_type)
                    && self.can_see_target(caster, attack_target)
                    && !self.is_dead(attack_target)
                    && dist2 <= srange
                {
                    self.cast_now(caster, skill);
                    return true;
                }
            }
        }

        if self.skill_has_effect_type(
            skill,
            &[
                EffectType::Stun,
                EffectType::Root,
                EffectType::Paralyze,
                EffectType::Mute,
                EffectType::Fear,
            ],
        ) {
            let aoe = self.can_aoe(caster, skill);
            if self.can_see_target(caster, attack_target) && !aoe && dist2 <= srange {
                if !self.is_affected_by_skill(attack_target, skill.id) {
                    self.cast_now(caster, skill);
                    return true;
                }
            } else if aoe {
                if is_aura_plain(target_type) {
                    self.cast_now(caster, skill);
                    return true;
                }

                if is_area(target_type)
                    && self.can_see_target(caster, attack_target)
                    && !self.is_dead(attack_target)
                    && dist2 <= srange
                {
                    self.cast_now(caster, skill);
                    return true;
                }
            } else if target_type == TargetType::One
                && self.effect_target_reconsider(caster, skill, false).is_some()
            {
                self.cast_now(caster, skill);
                return true;
            }
        }

        if self.skill_has_effect_type(skill, &[EffectType::DmgOverTime, EffectType::DmgOverTimePercent]) {
            let aoe = self.can_aoe(caster, skill);
            if self.can_see_target(caster, attack_target)
                && !aoe
                && !self.is_dead(attack_target)
                && dist2 <= srange
            {
                if !self.is_affected_by_skill(attack_target, skill.id) {
                    self.cast_now(caster, skill);
                    return true;
                }
            } else if aoe {
                if is_aura(target_type) {
                    self.cast_now(caster, skill);
                    return true;
                }

                if is_area(target_type)
                    && self.can_see_target(caster, attack_target)
                    && !self.is_dead(attack_target)
                    && dist2 <= srange
                {
                    self.cast_now(caster, skill);
                    return true;
                }
            } else if target_type == TargetType::One
                && self.effect_target_reconsider(caster, skill, false).is_some()
            {
                self.cast_now(caster, skill);
                return true;
            }
        }

        if self.skill_has_effect_type(skill, &[EffectType::Resurrection]) {
            if !Self::is_party(skill) {
                if self.is_minion(caster)
                    && target_type != TargetType::SelfTarget
                    && let Some(leader) = self.leader_of(caster)
                {
                    let leader_collision = self.collision_radius_of(leader);
                    let reach = skill.cast_range + caster_collision + leader_collision;
                    if self.is_dead(leader)
                        && !self.check_if_in_range(reach, caster, leader, false)
                        && !Self::is_party(skill)
                        && !self.is_movement_disabled(caster)
                    {
                        self.move_to_pawn(caster, leader, reach);
                    }

                    if self.can_see_target(caster, leader) {
                        self.cast_at_and_restore(caster, leader, skill, attack_target);
                        return true;
                    }
                }

                let range = skill.cast_range + caster_collision;
                let mut found = None;
                for obj in self.ai_visible_attackables_in_range(caster, range) {
                    if self.is_dead(obj)
                        && self.is_in_my_clan(caster, obj)
                        && self.rng().below(100) < 10
                        && self.can_see_target(caster, obj)
                    {
                        found = Some(obj);
                        break;
                    }
                }
                if let Some(res_target) = found {
                    self.cast_at_and_restore(caster, res_target, skill, attack_target);
                    return true;
                }
            } else {
                let range = skill.affect_range + caster_collision;
                let mut found = false;
                for obj in self.visible_objects_in_range(caster, range) {
                    if self.is_npc(obj)
                        && self.is_in_my_clan(caster, obj)
                        && self.hp_below_max(obj)
                        && self.rng().below(100) <= 20
                    {
                        found = true;
                        break;
                    }
                }
                if found {
                    self.cast_at_and_restore(caster, caster, skill, attack_target);
                    return true;
                }
            }
        }

        if !self.can_aura(caster, skill) {
            if self.can_see_target(caster, attack_target) && !self.is_dead(attack_target) && dist2 <= srange {
                self.cast_now(caster, skill);
                return true;
            }

            if let Some(target) = self.skill_target_reconsider(caster, skill) {
                self.cast_at_and_restore(caster, target, skill, attack_target);
                return true;
            }
        } else {
            self.cast_now(caster, skill);
            return true;
        }

        false
    }

    /// `obj.getCurrentHp() < obj.getMaxHp()`.
    fn hp_below_max(&self, id: ObjectId) -> bool {
        let cur = self.core_of(id).map_or(0.0, |c| c.cur_hp);
        cur < f64::from(self.stats(id).map_or(0, |s| s.max_hp()))
    }

    // ------------------------------------------------------------------ thinkActive

    /// The BUFF loop of `thinkActive` (the minion branch and the random walk branch): the first
    /// buff the NPC does not have, can pay for, and (a heal) needs, is cast on itself. `true`
    /// when one was cast.
    pub(crate) fn think_active_buffs(&mut self, id: ObjectId) -> bool {
        let Some(skills) = self.ai_skills_of(id) else {
            return false;
        };
        let Some(table) = self.skill_table() else {
            return false;
        };
        for key in skills.get(AiSkillScope::Buff) {
            let Some(skill) = table.get(key.id, key.level) else {
                continue;
            };
            // Skip buffs that are already active.
            if self.is_affected_by_skill(id, skill.id) {
                continue;
            }
            // Only consider skills if we have enough MP.
            let mp = self.core_of(id).map_or(0.0, |c| c.cur_mp);
            if mp <= f64::from(skill.mp_consume) {
                continue;
            }
            // Healing skills: only cast if HP is not full.
            if skill.abnormal_type == AbnormalType::LifeForceOthers && !self.hp_below_max(id) {
                continue; // Skip healing if at full HP.
            }
            self.set_npc_target(id, Some(id));
            self.do_cast(id, SkillKey::of(skill));
            return true;
        }
        false
    }

    // ------------------------------------------------------------------ thinkAttack

    /// The SUICIDE branch of `thinkAttack`: below 30% HP the NPC may use one of its suicide
    /// skills on the most hated creature. `true` when the think is over.
    pub(crate) fn think_attack_suicide(
        &mut self,
        id: ObjectId,
        most_hate: ObjectId,
        skills: &NpcAiSkills,
    ) -> bool {
        let list = skills.get(AiSkillScope::Suicide);
        if list.is_empty() || (self.hp_fraction(id) * 100.0) as i32 >= 30 {
            return false;
        }
        let pick = usize::try_from(self.rng().below(i32::try_from(list.len()).unwrap_or(1))).unwrap_or(0);
        let Some(table) = self.skill_table() else {
            return false;
        };
        let Some(skill) = list.get(pick).and_then(|k| table.get(k.id, k.level)) else {
            return false;
        };
        self.check_if_in_range(skill.affect_range, id, most_hate, false)
            && self.has_skill_chance(id)
            && self.ai_cast(id, skill)
    }

    /// The "Cast skills" block of `thinkAttack` (the NPC does not move, or is a mage): heal and
    /// resurrection skills (only for an NPC with GENERAL skills), then the short and long range
    /// skills. `true` when the NPC started a cast or a walk and the think is over.
    pub(crate) fn think_attack_skills(
        &mut self,
        id: ObjectId,
        most_hate: ObjectId,
        collision: i32,
        skills: &NpcAiSkills,
    ) -> bool {
        let Some(table) = self.skill_table() else {
            return false;
        };
        let skill_of = |key: &SkillKey| table.get(key.id, key.level);

        if !skills.get(AiSkillScope::General).is_empty() {
            // Heal Condition
            let heal_skills = skills.get(AiSkillScope::Heal);
            if !heal_skills.is_empty() && self.think_attack_heal(id, collision, heal_skills, &table) {
                return true;
            }
            // Res Skill Condition
            let res_skills = skills.get(AiSkillScope::Res);
            if !res_skills.is_empty() && self.think_attack_res(id, collision, res_skills, &table) {
                return true;
            }
        }

        // Long/Short Range skill usage.
        let short = skills.get(AiSkillScope::ShortRange);
        if !short.is_empty() && self.has_skill_chance(id) && self.dist_2d(id, most_hate) <= SHORT_RANGE {
            let pick =
                usize::try_from(self.rng().below(i32::try_from(short.len()).unwrap_or(1))).unwrap_or(0);
            if let Some(skill) = short.get(pick).and_then(skill_of)
                && self.try_ranged_skill(id, most_hate, skill)
            {
                return true;
            }
        }

        let long = skills.get(AiSkillScope::LongRange);
        if !long.is_empty() && self.has_skill_chance(id) {
            let pick = usize::try_from(self.rng().below(i32::try_from(long.len()).unwrap_or(1))).unwrap_or(0);
            if let Some(skill) = long.get(pick).and_then(skill_of)
                && self.try_ranged_skill(id, most_hate, skill)
            {
                return true;
            }
        }
        false
    }

    /// The common tail of the short and long range branches.
    fn try_ranged_skill(&mut self, id: ObjectId, most_hate: ObjectId, skill: &SkillTemplate) -> bool {
        let cast_range = skill.cast_range;
        if (cast_range < 1 || self.dist_3d(id, most_hate) < f64::from(cast_range))
            && self.check_skill_cast_conditions(id, skill)
            && self.allows_parameterized_cast(id, skill, Some(most_hate))
        {
            self.client_stop_moving(id, None);
            self.set_npc_target(id, Some(most_hate));
            self.do_cast(id, SkillKey::of(skill));
            return true;
        }
        false
    }

    /// The HEAL half of the "Cast skills" block.
    fn think_attack_heal(
        &mut self,
        id: ObjectId,
        collision: i32,
        heal_skills: &[SkillKey],
        table: &SkillTemplates,
    ) -> bool {
        if self.is_minion(id)
            && let Some(leader) = self.leader_of(id)
            && !self.is_dead(leader)
            && f64::from(self.rng().below(100)) > self.hp_fraction(leader) * 100.0
        {
            for key in heal_skills {
                let Some(skill) = table.get(key.id, key.level) else {
                    continue;
                };
                if skill.target_type == TargetType::SelfTarget {
                    continue;
                }
                if !self.check_skill_cast_conditions(id, skill) {
                    continue;
                }
                let reach = skill.cast_range + collision + self.collision_radius_of(leader);
                if !self.check_if_in_range(reach, id, leader, false)
                    && !Self::is_party(skill)
                    && !self.is_movement_disabled(id)
                {
                    self.move_to_pawn(id, leader, reach);
                    return true;
                }
                if self.can_see_target(id, leader) {
                    self.client_stop_moving(id, None);
                    let old = self.npc_cast_target(id);
                    self.set_npc_target(id, Some(leader));
                    self.do_cast(id, SkillKey::of(skill));
                    self.restore_npc_target(id, old);
                    return true;
                }
            }
        }

        let percentage = self.hp_fraction(id) * 100.0;
        if f64::from(self.rng().below(100)) < (100.0 - percentage) / 3.0 {
            for key in heal_skills {
                let Some(skill) = table.get(key.id, key.level) else {
                    continue;
                };
                if !self.check_skill_cast_conditions(id, skill) {
                    continue;
                }
                self.client_stop_moving(id, None);
                let old = self.npc_cast_target(id);
                self.set_npc_target(id, Some(id));
                self.do_cast(id, SkillKey::of(skill));
                self.restore_npc_target(id, old);
                return true;
            }
        }

        for key in heal_skills {
            let Some(skill) = table.get(key.id, key.level) else {
                continue;
            };
            if !self.check_skill_cast_conditions(id, skill) {
                continue;
            }
            if skill.target_type == TargetType::One {
                // NOTE(java-quirk): this branch looks for a clan member that is *dead* (`obj
                // .isDead()`), the `cast(skill)` heal looks for a living one.
                let range = skill.cast_range + collision;
                let mut found = None;
                for obj in self.ai_visible_attackables_in_range(id, range) {
                    if self.is_dead(obj)
                        && self.is_in_my_clan(obj, id)
                        && f64::from(self.rng().below(100)) < (100.0 - self.hp_fraction(obj) * 100.0) / 10.0
                        && self.can_see_target(id, obj)
                    {
                        found = Some(obj);
                        break;
                    }
                }
                if let Some(heal_target) = found {
                    self.client_stop_moving(id, None);
                    let old = self.npc_cast_target(id);
                    self.set_npc_target(id, Some(heal_target));
                    self.do_cast(id, SkillKey::of(skill));
                    self.restore_npc_target(id, old);
                    return true;
                }
            }
            if Self::is_party(skill) {
                self.client_stop_moving(id, None);
                self.do_cast(id, SkillKey::of(skill));
                return true;
            }
        }
        false
    }

    /// The RES half of the "Cast skills" block.
    fn think_attack_res(
        &mut self,
        id: ObjectId,
        collision: i32,
        res_skills: &[SkillKey],
        table: &SkillTemplates,
    ) -> bool {
        if self.is_minion(id)
            && let Some(leader) = self.leader_of(id)
            && self.is_dead(leader)
        {
            for key in res_skills {
                let Some(skill) = table.get(key.id, key.level) else {
                    continue;
                };
                if skill.target_type == TargetType::SelfTarget {
                    continue;
                }
                if !self.check_skill_cast_conditions(id, skill) {
                    continue;
                }
                let reach = skill.cast_range + collision + self.collision_radius_of(leader);
                if !self.check_if_in_range(reach, id, leader, false)
                    && !Self::is_party(skill)
                    && !self.is_movement_disabled(id)
                {
                    self.move_to_pawn(id, leader, reach);
                    return true;
                }
                if self.can_see_target(id, leader) {
                    self.client_stop_moving(id, None);
                    let old = self.npc_cast_target(id);
                    self.set_npc_target(id, Some(leader));
                    self.do_cast(id, SkillKey::of(skill));
                    self.restore_npc_target(id, old);
                    return true;
                }
            }
        }

        for key in res_skills {
            let Some(skill) = table.get(key.id, key.level) else {
                continue;
            };
            if !self.check_skill_cast_conditions(id, skill) {
                continue;
            }
            if skill.target_type == TargetType::One {
                let range = skill.cast_range + collision;
                let mut found = None;
                for obj in self.ai_visible_attackables_in_range(id, range) {
                    if self.is_dead(obj)
                        && self.is_in_my_clan(id, obj)
                        && self.rng().below(100) < 10
                        && self.can_see_target(id, obj)
                    {
                        found = Some(obj);
                        break;
                    }
                }
                if let Some(res_target) = found {
                    self.client_stop_moving(id, None);
                    let old = self.npc_cast_target(id);
                    self.set_npc_target(id, Some(res_target));
                    self.do_cast(id, SkillKey::of(skill));
                    self.restore_npc_target(id, old);
                    return true;
                }
            }
            if Self::is_party(skill) {
                self.client_stop_moving(id, None);
                let old = self.npc_cast_target(id);
                self.set_npc_target(id, Some(id));
                self.do_cast(id, SkillKey::of(skill));
                self.restore_npc_target(id, old);
                return true;
            }
        }
        false
    }

    // ------------------------------------------------------------------ movementDisable

    /// The skill part of `movementDisable()` (an NPC that cannot move): tries the scopes in
    /// Java's order with `random` (one `Rnd.get(100)` for all of them). `true` when it cast.
    pub(crate) fn movement_disable_skills(
        &mut self,
        id: ObjectId,
        target: ObjectId,
        random: i32,
        dist: f64,
    ) -> bool {
        let Some(skills) = self.ai_skills_of(id) else {
            return false;
        };
        if !self.is_immobilized(target)
            && random < 15
            && self.try_cast(id, target, &skills, AiSkillScope::Immobilize, dist)
        {
            return true;
        }
        if random < 20 && self.try_cast(id, target, &skills, AiSkillScope::Cot, dist) {
            return true;
        }
        if random < 30 && self.try_cast(id, target, &skills, AiSkillScope::Debuff, dist) {
            return true;
        }
        if random < 40 && self.try_cast(id, target, &skills, AiSkillScope::Negative, dist) {
            return true;
        }
        let ai_type = self.npc_template(id).map(|t| t.ai_type);
        if (self.is_movement_disabled(id) || ai_type == Some(AiType::Mage) || ai_type == Some(AiType::Healer))
            && self.try_cast(id, target, &skills, AiSkillScope::Attack, dist)
        {
            return true;
        }
        self.try_cast(id, target, &skills, AiSkillScope::Universal, dist)
    }

    /// Java `AttackableAI.tryCast(npc, target, scope, dist)`.
    fn try_cast(
        &mut self,
        id: ObjectId,
        target: ObjectId,
        skills: &NpcAiSkills,
        scope: AiSkillScope,
        dist: f64,
    ) -> bool {
        let Some(table) = self.skill_table() else {
            return false;
        };
        for key in skills.get(scope) {
            let Some(skill) = table.get(key.id, key.level) else {
                continue;
            };
            if !self.check_skill_cast_conditions(id, skill)
                || (f64::from(skill.cast_range + self.collision_radius_of(target)) <= dist
                    && !self.can_aura(id, skill))
            {
                continue;
            }
            if !self.can_see_target(id, target) {
                continue;
            }
            if !self.allows_parameterized_cast(id, skill, Some(target)) {
                continue;
            }
            self.client_stop_moving(id, None);
            self.do_cast(id, SkillKey::of(skill));
            return true;
        }
        false
    }

    // ------------------------------------------------------------------ thinkCast

    /// Java `AttackableAI.thinkCast()`: reached through `setIntentionCast` (scripts, D-05):
    /// walk into the skill's range, then cast at the cast target.
    pub(crate) fn attackable_think_cast(&mut self, id: ObjectId) {
        let Some(key) = self.cast_skill_of(id) else {
            return;
        };
        let Some(table) = self.skill_table() else {
            return;
        };
        let Some(skill) = table.get(key.id, key.level) else {
            return;
        };
        let target = self.cast_target_of(id);
        if self.check_target_lost(id, target, skill) {
            self.set_cast_target(id, None);
            return;
        }
        let range = self.magical_attack_range_for(id, skill);
        if let Some(target) = target
            && self.maybe_move_to_pawn(id, target, range)
        {
            return;
        }
        self.client_stop_moving(id, None);
        self.set_intention_active(id);
        self.do_cast(id, key);
    }

    /// Java `AttackableAI.setIntentionCast(skill, target)`: the part before `super`.
    pub(crate) fn attackable_before_intention_cast(&mut self, id: ObjectId, target: Option<ObjectId>) {
        if target.is_some() {
            self.set_ai_target(id, target);
        }
    }

    /// Java `AttackableAI.notifyActionFinishCasting()`: a finished cast keeps the fight alive.
    pub(crate) fn attackable_finish_casting(&mut self, id: ObjectId) {
        if self.is_attackable_npc(id) && self.intention_of(id) == Intention::Attack {
            self.renew_attack_timeout(id);
        }
    }
}
