//! Creature events for the trigger skill effects, and `Creature.makeTriggerCast`.
// Java: entity/actor/Creature.java (makeTriggerCast, notifyDamageReceived, notifyAttackAvoid,
//       addListener / removeListenerIf)
// Java: mechanics/events/impl (EventDispatcher): the listeners the trigger effects register
//
// Java registers a `ConsumerEventListener` with the creature (`effected.addListener(..)`) when a
// `TriggerSkillBy*` effect starts and removes it (`removeListenerIf(owner == this)`) when it ends.
// Here the creature's listeners are a small side table (`EffectsWorld::listeners`); a listener
// holds the plain parameters of its effect and the `EffectKey` of the `<effect>` that owns it
// (Java compares the effect object). The "event" is a plain call at the place Java fires it:
//
// - `notify_attack_avoid`: `Creature.onHitTimer` for a miss (`ON_CREATURE_ATTACK_AVOID`).
// - `notify_damage_received_by`: every hit and damage effect (`ON_CREATURE_DAMAGE_DEALT` for the
//   attacker, `ON_CREATURE_DAMAGE_RECEIVED` for the target).
// - `notify_skill_use`: `Creature.beginCast` (`ON_CREATURE_SKILL_USE`).
//
// NOTE: Java dispatches `ON_CREATURE_DAMAGE_RECEIVED` asynchronously (`notifyEventAsync`); here it
// runs at once, in the same call.

use std::collections::HashMap;

use l2_data::SkillTemplate;
use l2_data::skill::TargetType;
use l2_protocol_game::server::{MagicSkillLaunched, MagicSkillUse, SkillPosition};
use l2_util::ObjectId;
use tracing::warn;

use super::handlers::trigger_skill::{ByAvoid, ByDamageDealt, ByDamageReceived, BySkill};
use super::registry::EffectCx;
use super::{EffectKey, SkillKey};
use crate::world::World;

/// What a registered listener does when its event comes (the parameters of its effect).
#[derive(Debug, Clone)]
pub(crate) enum TriggerKind {
    /// `TriggerSkillByAvoid`.
    Avoid(ByAvoid),
    /// `TriggerSkillByDamageDealt`.
    DamageDealt(ByDamageDealt),
    /// `TriggerSkillByDamageReceived`.
    DamageReceived(ByDamageReceived),
    /// `TriggerSkillBySkill`.
    SkillUse(BySkill),
}

/// One `ConsumerEventListener` of a creature.
#[derive(Debug, Clone)]
pub(crate) struct Listener {
    /// The effect that registered it (Java `listener.getOwner() == this`).
    pub owner: EffectKey,
    pub kind: TriggerKind,
}

/// The listeners of every creature (`EffectsWorld::listeners`).
pub(crate) type ListenerTable = HashMap<ObjectId, Vec<Listener>>;

/// What a damage listener hears (Java `OnCreatureDamageDealt` / `OnCreatureDamageReceived`).
#[derive(Debug, Clone, Copy)]
pub(crate) struct DamageEvent {
    /// `None` for damage with no attacker.
    pub attacker: Option<ObjectId>,
    pub target: ObjectId,
    pub damage: f64,
    /// The skill that did it (`None` for a normal attack).
    pub skill: Option<SkillKey>,
    pub critical: bool,
    pub damage_over_time: bool,
}

/// The `EffectKey` of the `<effect>` a hook is running for (the identity of Java's effect object).
pub(crate) fn owner_of(cx: &EffectCx<'_>) -> EffectKey {
    let list = cx.skill.effects.get(cx.effect.scope);
    let index = list
        .iter()
        .position(|e| std::ptr::eq(e, cx.effect))
        .or_else(|| list.iter().position(|e| e == cx.effect))
        .unwrap_or(0);
    EffectKey {
        skill: SkillKey::of(cx.skill),
        scope: cx.effect.scope,
        index,
    }
}

impl World {
    // ------------------------------------------------------------------ the listener table

    /// Java `creature.addListener(new ConsumerEventListener(creature, type, consumer, owner))`.
    pub(crate) fn add_trigger_listener(&mut self, id: ObjectId, owner: EffectKey, kind: TriggerKind) {
        self.effects
            .listeners
            .entry(id)
            .or_default()
            .push(Listener { owner, kind });
    }

    /// Java `creature.removeListenerIf(type, listener -> listener.getOwner() == this)`: the
    /// listeners of one event kind that `owner` registered.
    pub(crate) fn remove_trigger_listeners(
        &mut self,
        id: ObjectId,
        owner: EffectKey,
        is_kind: fn(&TriggerKind) -> bool,
    ) {
        if let Some(list) = self.effects.listeners.get_mut(&id) {
            list.retain(|l| !(l.owner == owner && is_kind(&l.kind)));
            if list.is_empty() {
                self.effects.listeners.remove(&id);
            }
        }
    }

    /// How many trigger listeners a creature has (tests).
    pub fn trigger_listener_count(&self, id: ObjectId) -> usize {
        self.effects.listeners.get(&id).map_or(0, Vec::len)
    }

    /// The kinds of a creature's listeners, copied so the handlers can change the world.
    fn trigger_kinds(&self, id: ObjectId) -> Vec<TriggerKind> {
        self.effects
            .listeners
            .get(&id)
            .map(|list| list.iter().map(|l| l.kind.clone()).collect())
            .unwrap_or_default()
    }

    // ------------------------------------------------------------------ the events

    /// Java `attacker.notifyAttackAvoid(target, isDot)`: `target` avoided the attack of `attacker`.
    pub fn notify_attack_avoid(&mut self, attacker: ObjectId, target: ObjectId, is_dot: bool) {
        for kind in self.trigger_kinds(target) {
            if let TriggerKind::Avoid(listener) = kind {
                listener.on_avoid(self, attacker, target, is_dot);
            }
        }
        // The script listeners (S-API-04).
        self.script_fire(
            crate::script::ScriptEvent::CreatureAttackAvoid {
                attacker,
                target,
                damage_over_time: is_dot,
            },
            Some(target),
        );
    }

    /// Java `target.notifyDamageReceived(damage, attacker, skill, critical, damageOverTime)`: a
    /// fake dead player stands up, the attacker's `ON_CREATURE_DAMAGE_DEALT` listeners hear it,
    /// then the target's `ON_CREATURE_DAMAGE_RECEIVED` listeners.
    pub fn notify_damage_received_by(
        &mut self,
        target: ObjectId,
        damage: f64,
        attacker: Option<ObjectId>,
        skill: Option<SkillKey>,
        critical: bool,
        damage_over_time: bool,
    ) {
        // Auto attacks make you stand up.
        if self.is_fake_death(target) && self.settings.effects.fake_death_damage_stand && damage > 0.0 {
            self.stop_fake_death(target, true);
        }
        let event = DamageEvent {
            attacker,
            target,
            damage,
            skill,
            critical,
            damage_over_time,
        };
        if let Some(attacker) = attacker {
            for kind in self.trigger_kinds(attacker) {
                if let TriggerKind::DamageDealt(listener) = kind {
                    listener.on_damage_dealt(self, &event);
                }
            }
        }
        for kind in self.trigger_kinds(target) {
            if let TriggerKind::DamageReceived(listener) = kind {
                listener.on_damage_received(self, &event);
            }
        }
        // The script listeners (S-API-04): the attacker's `ON_CREATURE_DAMAGE_DEALT` at once, the
        // target's `ON_CREATURE_DAMAGE_RECEIVED` queued (Java `notifyEventAsync`).
        let info = crate::script::DamageInfo {
            attacker,
            target,
            damage,
            skill,
            critical,
            damage_over_time,
        };
        if let Some(attacker) = attacker {
            self.script_fire(
                crate::script::ScriptEvent::CreatureDamageDealt(info),
                Some(attacker),
            );
        }
        self.script_fire_async(
            crate::script::ScriptEvent::CreatureDamageReceived(info),
            Some(target),
        );
    }

    /// Java `ON_CREATURE_SKILL_USE` (`OnCreatureSkillUse`): `caster` starts to cast `skill` at
    /// `target`.
    pub fn notify_skill_use(&mut self, caster: ObjectId, skill: SkillKey, target: ObjectId) {
        for kind in self.trigger_kinds(caster) {
            if let TriggerKind::SkillUse(listener) = kind {
                listener.on_skill_use(self, caster, skill, target);
            }
        }
    }

    // ------------------------------------------------------------------ makeTriggerCast

    /// The loop all four trigger effects share: the handler of `target_type` lists the targets of
    /// `trigger` for `caster` (aiming at `target`), and `caster` triggers the skill on every
    /// creature of the list that is not invulnerable.
    pub(crate) fn trigger_skill_on_targets(
        &mut self,
        trigger: SkillKey,
        target_type: TargetType,
        caster: ObjectId,
        target: Option<ObjectId>,
    ) {
        let Some(table) = self.skill_table() else {
            return;
        };
        // `SkillHolder.getSkill()`: `SkillData.getSkill(id, Math.max(level, 1))`.
        let Some(skill) = table.get_skill(trigger.id, trigger.level.max(1)) else {
            return;
        };
        let key = SkillKey::of(skill);
        let ctx = self.cast_target_context(caster);
        let result = self.skill_target_list_as(skill, target_type, caster, false, target, &ctx);
        self.apply_target_result(caster, skill, &result);
        for trigger_target in result.targets {
            if !self
                .objects
                .get(&trigger_target)
                .is_some_and(|e| e.kind.is_creature())
            {
                continue;
            }
            if !self.is_invul(trigger_target) {
                self.make_trigger_cast(caster, key, trigger_target, false);
            }
        }
    }

    /// Java `Creature.makeTriggerCast(skill, target, ignoreTargetType)`: casts `skill` without a
    /// cast time, checks or costs but its conditions and its reuse.
    pub fn make_trigger_cast(
        &mut self,
        caster: ObjectId,
        key: SkillKey,
        target: ObjectId,
        ignore_target_type: bool,
    ) {
        let Some(table) = self.skill_table() else {
            return;
        };
        let Some(skill) = table.get(key.id, key.level) else {
            return;
        };
        if !self.check_skill_condition(caster, target, key) {
            return;
        }
        if self.is_skill_disabled(caster, skill) {
            return;
        }
        if skill.reuse_delay > 0 {
            self.disable_skill(caster, skill, i64::from(skill.reuse_delay));
        }
        let targets = if ignore_target_type {
            vec![target]
        } else {
            self.cast_target_list(skill, caster, false, Some(target))
        };
        if targets.is_empty() {
            return;
        }
        let skill_target = targets
            .iter()
            .copied()
            .find(|t| self.objects.get(t).is_some_and(|e| e.kind.is_creature()))
            .unwrap_or(target);
        if self.settings.effects.alt_validate_trigger_skills
            && self.is_playable(caster)
            && self.is_playable(skill_target)
        {
            let ctx = self.cast_target_context(caster);
            if !self.check_pvp_skill(caster, skill_target, skill, &ctx) {
                return;
            }
        }
        self.broadcast_trigger_cast(caster, skill_target, skill, &targets);
        // Launch the magic skill and calculate its effects.
        self.activate_skill(caster, &targets, key);
    }

    /// The `MagicSkillUse` and `MagicSkillLaunched` of `makeTriggerCast`.
    // NOTE(java-quirk): both packets name `skill.getLevel()`, not the display level `beginCast`
    // sends.
    fn broadcast_trigger_cast(
        &mut self,
        caster: ObjectId,
        skill_target: ObjectId,
        skill: &SkillTemplate,
        targets: &[ObjectId],
    ) {
        let (Some(me), Some(at)) = (self.position_of(caster), self.position_of(skill_target)) else {
            return;
        };
        let ground = if self.is_player(caster) {
            self.cast
                .creatures
                .get(&caster)
                .and_then(|s| s.current_skill_world_position)
                .map(|(x, y, z)| SkillPosition { x, y, z })
        } else {
            None
        };
        self.broadcast_creature_packet(
            caster,
            &MagicSkillUse {
                creature_id: caster.0,
                target_id: skill_target.0,
                skill_id: skill.display_id,
                skill_level: skill.level,
                hit_time: 0,
                reuse_delay: 0,
                creature: SkillPosition {
                    x: me.0,
                    y: me.1,
                    z: me.2,
                },
                ground_location: ground,
                target: SkillPosition {
                    x: at.0,
                    y: at.1,
                    z: at.2,
                },
            },
        );
        self.broadcast_creature_packet(
            caster,
            &MagicSkillLaunched {
                creature_id: caster.0,
                skill_id: skill.display_id,
                skill_level: skill.level,
                target_ids: targets.iter().map(|t| t.0).collect(),
            },
        );
    }
}

/// Warns that the target type of a trigger effect has no handler (Java logs and returns).
pub(crate) fn warn_no_handler(target_type: TargetType) {
    warn!("Handler for target type: {target_type:?} does not exist.");
}
