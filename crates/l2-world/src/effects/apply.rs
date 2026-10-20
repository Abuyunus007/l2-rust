//! How a skill puts its effects on creatures: the entry points casting (M5-CAST-01), items
//! (M5-ITEM-01), passive skills and everything else call.
// Java: mechanics/skill/Skill.java (applyEffects (3 overloads), applyEffectScope, activateSkill,
//       hasEffectType, hasNegativeEffect, checkCondition)
// Java: mechanics/stats/Formulas.java (calcEffectSuccess, calcEffectAbnormalTime,
//       calcBuffDebuffReflection: the parts that need no M5-FORM-02 formula)
//
// The formulas M5-FORM-02 ported (vector-checked, `crate::formulas::effect`) are wired in here: the
// landing rate of `calcEffectSuccess`, the skill mastery and debuff resistance parts of
// `calcEffectAbnormalTime`. The inputs the stat view cannot give yet are marked `M5:`.

use l2_data::SkillTemplate;
use l2_data::skill::{BaseStat as SkillBaseStat, EffectScope, TargetType};
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;

use super::buff_info::{BuffInfo, skill_name_param};
use super::registry::{EffectCx, EffectType};
use super::{SkillFinishType, SkillKey};
use crate::formulas::{
    AbnormalTimeInput, AbnormalTimeResists, EffectSuccessInput, Rnd, SkillMasteryInput, attribute_element,
    calc_attribute_bonus_skill, calc_effect_abnormal_time, calc_effect_success, calc_lvl_bonus_mod,
};
use crate::inventory::handlers::shots::ShotType;
use crate::stats::conditions::{self, CondEnv};
use crate::stats::skills::calc_skill;
use crate::stats::{BaseStat, Stat, StatView};
use crate::world::World;

/// The options of Java's six-argument `Skill.applyEffects`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApplyMode {
    /// `self`: the self effects go on the caster.
    pub self_effects: bool,
    /// `passive`: the passive effects go on the caster.
    pub passive: bool,
    /// `instant`: the instant effects are applied to the effected.
    pub instant: bool,
    /// `abnormalTime`: a custom abnormal time in seconds (ignored when 0 or less).
    pub abnormal_time: i32,
}

impl ApplyMode {
    /// `applyEffects(effector, effected)`.
    pub const DEFAULT: ApplyMode = ApplyMode {
        self_effects: false,
        passive: false,
        instant: true,
        abnormal_time: 0,
    };
    /// `applyEffects(this, this, false, true, false, 0)`: a passive skill.
    pub const PASSIVE: ApplyMode = ApplyMode {
        self_effects: false,
        passive: true,
        instant: false,
        abnormal_time: 0,
    };
    /// `applyEffects(caster, caster, true, false, true, 0)`: the self effects of a cast.
    pub const SELF: ApplyMode = ApplyMode {
        self_effects: true,
        passive: false,
        instant: true,
        abnormal_time: 0,
    };
}

/// Java `Skill.hasNegativeEffect()`.
pub(crate) fn has_negative_effect(skill: &SkillTemplate) -> bool {
    skill.effect_point < 0 && skill.target_type != TargetType::SelfTarget
}

impl World {
    /// Java `Skill.applyEffects(effector, effected)`: the skill's effects land on `effected`
    /// (instant ones at once, continuous ones as a buff info).
    pub fn apply_effects(&mut self, effector: ObjectId, effected: ObjectId, skill: SkillKey) {
        self.apply_effects_with(effector, effected, skill, ApplyMode::DEFAULT);
    }

    /// Java `Skill.applyEffects(effector, effected, instant, abnormalTime)`.
    pub fn apply_effects_timed(
        &mut self,
        effector: ObjectId,
        effected: ObjectId,
        skill: SkillKey,
        instant: bool,
        abnormal_time: i32,
    ) {
        self.apply_effects_with(
            effector,
            effected,
            skill,
            ApplyMode {
                instant,
                abnormal_time,
                ..ApplyMode::DEFAULT
            },
        );
    }

    /// Java `Skill.applyEffects(effector, effected, self, passive, instant, abnormalTime)`.
    pub fn apply_effects_with(
        &mut self,
        effector: ObjectId,
        effected: ObjectId,
        key: SkillKey,
        mode: ApplyMode,
    ) {
        // Null targets cannot receive any effects.
        if !self.is_creature(effected) {
            return;
        }
        let Some(skills) = self.skill_table() else {
            return;
        };
        let Some(skill) = skills.get(key.id, key.level) else {
            return;
        };
        // Check negative effect skills against target.
        // NOTE: the access level's `canGiveDamage` is not loaded (every level may give damage,
        // the shipped `AccessLevels.xml` default).
        if effector != effected && has_negative_effect(skill) && self.is_invul(effected) {
            return;
        }
        if self.is_invul_against(effected, skill.id, skill.level) {
            return;
        }
        let op = skill.operate_type;
        let mut add_continuous = !mode.passive
            && (op.is_toggle()
                || (op.is_continuous() && self.calc_effect_success(effector, effected, skill)));
        if !mode.self_effects && !mode.passive {
            let mut info = self.new_buff_info(effector, effected, skill);
            if add_continuous && mode.abnormal_time > 0 {
                info.abnormal_time = mode.abnormal_time;
            }
            self.apply_effect_scope(EffectScope::General, &mut info, mode.instant, add_continuous);
            if let Some(scope) = self.pvp_or_pve_scope(effector, effected) {
                self.apply_effect_scope(scope, &mut info, mode.instant, add_continuous);
            }
            self.apply_effect_scope(EffectScope::Channeling, &mut info, mode.instant, add_continuous);
            if add_continuous {
                self.add_buff_info(info);
            }
            // The buff sharing with a servitor is commented out in Java.
        }
        if mode.self_effects {
            add_continuous = !mode.passive
                && (op.is_toggle()
                    || ((op.is_continuous() || op.is_self_continuous())
                        && self.calc_effect_success(effector, effector, skill)));
            let mut info = self.new_buff_info(effector, effector, skill);
            if add_continuous && mode.abnormal_time > 0 {
                info.abnormal_time = mode.abnormal_time;
            }
            self.apply_effect_scope(EffectScope::SelfScope, &mut info, mode.instant, add_continuous);
            // "TODO : Need to be done better after skill rework"
            if add_continuous && self.skill_has_effect_type(skill, &[EffectType::Buff]) {
                self.add_buff_info(info);
            }
        }
        if mode.passive {
            let mut info = self.new_buff_info(effector, effector, skill);
            self.apply_effect_scope(EffectScope::Passive, &mut info, false, true);
            self.add_buff_info(info);
        }
    }

    /// Java `Skill.activateSkill(caster, targets)`: the skill's effects on every target (a
    /// reflected debuff turns around), then its self effects on the caster.
    pub fn activate_skill(&mut self, caster: ObjectId, targets: &[ObjectId], key: SkillKey) {
        let Some(skills) = self.skill_table() else {
            return;
        };
        let Some(skill) = skills.get(key.id, key.level) else {
            return;
        };
        for &target in targets {
            if !self.is_creature(target) {
                continue;
            }
            if self.calc_buff_debuff_reflection(target, skill) {
                // If skill is reflected instant effects should be casted on target and continuous
                // effects on caster.
                self.apply_effects_timed(target, caster, key, false, 0);
                let mut info = self.new_buff_info(caster, target, skill);
                self.apply_effect_scope(EffectScope::General, &mut info, true, false);
                if let Some(scope) = self.pvp_or_pve_scope(caster, target) {
                    self.apply_effect_scope(scope, &mut info, true, false);
                }
                self.apply_effect_scope(EffectScope::Channeling, &mut info, true, false);
            } else {
                self.apply_effects(caster, target, key);
            }
        }
        // Self Effect.
        if !skill.effects.get(EffectScope::SelfScope).is_empty() {
            if self.is_affected_by_skill(caster, skill.id) {
                self.stop_skill_effects(caster, SkillFinishType::Removed, skill.id);
            }
            self.apply_effects_with(caster, caster, key, ApplyMode::SELF);
        }
        // No cubic casts yet (M6), so the shots the skill uses are used up here.
        self.discharge_skill_shots(caster, skill);
        if skill.is_suicide_attack {
            self.do_die(caster, Some(caster));
        }
    }

    /// Java `Skill.applyEffectScope(scope, info, applyInstantEffects, addContinuousEffects)`:
    /// instant effects of the scope land (`calcSuccess`, `onStart`), continuous ones that may
    /// start join the info.
    pub(crate) fn apply_effect_scope(
        &mut self,
        scope: EffectScope,
        info: &mut BuffInfo,
        apply_instant: bool,
        add_continuous: bool,
    ) {
        let Some(skills) = self.skill_table() else {
            return;
        };
        let Some(skill) = skills.get(info.skill.id, info.skill.level) else {
            return;
        };
        for (index, template) in skill.effects.get(scope).iter().enumerate() {
            let Some(handler) = self.effect_handler(skill, scope, index) else {
                continue;
            };
            let cx = EffectCx {
                effector: info.effector,
                effected: info.effected,
                skill,
                effect: template,
                info: None,
            };
            if handler.is_instant() {
                if apply_instant && handler.calc_success(self, &cx) {
                    handler.on_start(self, &cx);
                }
            } else if add_continuous && handler.can_start(self, &cx) {
                info.add_effect(scope, index, handler);
            }
        }
    }

    /// `applyEffectScope(scope, info, true, false)` without keeping the info (the END effects of
    /// an info that ended).
    pub(crate) fn apply_effect_scope_instant(
        &mut self,
        scope: EffectScope,
        effector: ObjectId,
        effected: ObjectId,
        key: SkillKey,
    ) {
        let Some(skills) = self.skill_table() else {
            return;
        };
        let Some(skill) = skills.get(key.id, key.level) else {
            return;
        };
        if skill.effects.get(scope).is_empty() {
            return;
        }
        let id = self.effects.next_info_id();
        let mut info = BuffInfo::new(id, effector, effected, skill, 0, self.tick);
        self.apply_effect_scope(scope, &mut info, true, false);
    }

    /// `new BuffInfo(effector, effected, skill)`: the abnormal time comes from
    /// `Formulas.calcEffectAbnormalTime`.
    fn new_buff_info(&mut self, effector: ObjectId, effected: ObjectId, skill: &SkillTemplate) -> BuffInfo {
        let time = self.calc_effect_abnormal_time(effector, effected, skill);
        let id = self.effects.next_info_id();
        BuffInfo::new(id, effector, effected, skill, time, self.tick)
    }

    /// `PVE` when a playable hits an attackable, `PVP` between playables, else none.
    fn pvp_or_pve_scope(&self, effector: ObjectId, effected: ObjectId) -> Option<EffectScope> {
        let playable = |id| self.stats(id).is_some_and(|v| v.is_playable());
        let attackable = |id| self.stats(id).is_some_and(|v| v.is_attackable());
        if playable(effector) && attackable(effected) {
            Some(EffectScope::Pve)
        } else if playable(effector) && playable(effected) {
            Some(EffectScope::Pvp)
        } else {
            None
        }
    }

    /// Java `Skill.hasEffectType(type, types...)`: an effect of any scope whose handler has one
    /// of the types (an effect without a handler has none).
    pub fn skill_has_effect_type(&mut self, skill: &SkillTemplate, types: &[EffectType]) -> bool {
        for scope in EffectScope::ALL.iter().copied() {
            for index in 0..skill.effects.get(scope).len() {
                if self
                    .effect_handler(skill, scope, index)
                    .is_some_and(|h| types.contains(&h.effect_type()))
                {
                    return true;
                }
            }
        }
        false
    }

    /// Whether `id` is a player, an NPC or a door in the world (Java's `Creature` argument:
    /// `Door extends Creature`, so `OpenDoor` and `DetectHiddenObjects` run on doors).
    fn is_creature(&self, id: ObjectId) -> bool {
        self.players.contains_key(&id) || self.npcs.contains_key(&id) || self.doors.contains_key(&id)
    }

    /// Java `Skill.checkCondition(creature, object, false)`: the skill's conditions for
    /// `creature` against `target`; a failed one tells the player why.
    pub fn check_skill_condition(&mut self, creature: ObjectId, target: ObjectId, key: SkillKey) -> bool {
        let Some(skills) = self.skill_table() else {
            return false;
        };
        let Some(skill) = skills.get(key.id, key.level) else {
            return false;
        };
        let failed = {
            let Some(view) = self.stats(creature) else {
                return false;
            };
            // Fake players are not ported.
            if view.is_gm() && !view.settings().gm_skill_restriction {
                return true;
            }
            // Mounts are not modelled: the mounted check of skills with a negative effect never
            // applies.
            if skill.pre_conditions.is_empty() {
                return true;
            }
            let ctx = calc_skill(skill);
            let target = self.is_creature(target).then_some(target);
            skill
                .pre_conditions
                .iter()
                .find(|c| {
                    !conditions::test(
                        &c.condition,
                        &CondEnv {
                            actor: &view,
                            target,
                            skill: Some(&ctx),
                        },
                    )
                })
                .map(|c| c.message.clone())
        };
        let Some(message) = failed else {
            return true;
        };
        if message.message_id != 0 {
            if let Some(id) = SystemMessageId::from_id(message.message_id) {
                let mut sm = SystemMessage::new(id);
                if message.add_name {
                    // NOTE(java-quirk): `addSkillName(_id)` names level 1.
                    sm = sm.with(SystemMessageParam::SkillName {
                        id: skill.id,
                        level: 1,
                    });
                }
                self.send(creature, &sm);
            }
        } else if let Some(text) = &message.message {
            self.send_message(creature, text);
        }
        false
    }

    /// Java `Formulas.calcEffectSuccess(attacker, target, skill)`.
    pub(crate) fn calc_effect_success(
        &mut self,
        attacker: ObjectId,
        target: ObjectId,
        skill: &SkillTemplate,
    ) -> bool {
        // StaticObjects can not receive continuous effects.
        let siege_flag = self
            .stats(target)
            .and_then(|v| v.npc_template())
            .is_some_and(|t| t.npc_type == "SiegeFlag");
        if self.doors.contains_key(&target) || siege_flag || self.static_objects.contains_key(&target) {
            return false;
        }
        let ctx = calc_skill(skill);
        if skill.is_debuff
            && self
                .stats(target)
                .is_some_and(|v| v.calc_stat(Stat::DebuffImmunity, 0.0, Some(attacker), Some(&ctx)) > 0.0)
        {
            self.send_resisted(attacker, target, skill);
            return false;
        }
        let activate_rate = skill.activate_rate;
        if activate_rate == -1 || activate_rate > 99 || skill.basic_property == SkillBaseStat::None {
            return true;
        }
        // The landing rate (magic level, the target's base stat, element, trait, m. atk and
        // vulnerability mods, `Rnd.get(100)`).
        let landed = self.with_rng(|world, rng| {
            let (Some(av), Some(tv)) = (world.stats(attacker), world.stats(target)) else {
                // A creature that left the world cannot be rolled against.
                return true;
            };
            let input = EffectSuccessInput {
                skill_is_debuff: skill.is_debuff,
                debuff_immunity: tv.calc_stat(Stat::DebuffImmunity, 0.0, Some(attacker), Some(&ctx)),
                activate_rate,
                target_base_stat: match skill.basic_property {
                    SkillBaseStat::Str => Some(tv.str()),
                    SkillBaseStat::Dex => Some(tv.dex()),
                    SkillBaseStat::Con => Some(tv.con()),
                    SkillBaseStat::Int => Some(tv.int()),
                    SkillBaseStat::Men => Some(tv.men()),
                    SkillBaseStat::Wit => Some(tv.wit()),
                    SkillBaseStat::None => None,
                },
                skill_magic_level: skill.magic_level,
                skill_lvl_bonus_rate: skill.lvl_bonus_rate,
                target_level: tv.level(),
                element_mod: skill_attribute_bonus(&av, &tv, skill),
                // M5: the trait tables (`AttackTrait`, `DefenceTrait`, trait invulnerability) are
                // not in the stat view yet, so no creature has a trait: the bonus is 1.
                trait_mod: 1.0,
                buff_debuff_vuln: tv.calc(
                    if skill.is_debuff {
                        Stat::DebuffVuln
                    } else {
                        Stat::BuffVuln
                    },
                    1.0,
                ),
                skill_is_magic: skill.is_magic(),
                attacker_m_atk: av.m_atk(None, None),
                attacker_blessed_spiritshot_charged: world
                    .is_charged_shot(attacker, ShotType::BlessedSpiritshots),
                target_m_def: tv.m_def(None, None),
                min_chance: skill.min_chance,
                max_chance: skill.max_chance,
            };
            calc_effect_success(&input, rng)
        });
        if !landed {
            self.send_resisted(attacker, target, skill);
        }
        landed
    }

    /// "$c1 has resisted your $s2." to the attacker.
    fn send_resisted(&self, attacker: ObjectId, target: ObjectId, skill: &SkillTemplate) {
        let sm = SystemMessage::new(SystemMessageId::C1HasResistedYourS2)
            .with(SystemMessageParam::Text(self.creature_name(target)))
            .with(skill_name_param(skill));
        self.send(attacker, &sm);
    }

    /// Java `Formulas.calcEffectAbnormalTime(caster, target, skill)`: the seconds a continuous
    /// effect lasts (`-1` for passives and toggles).
    pub(crate) fn calc_effect_abnormal_time(
        &mut self,
        caster: ObjectId,
        target: ObjectId,
        skill: &SkillTemplate,
    ) -> i32 {
        let passive_or_toggle = skill.is_passive() || skill.is_toggle();
        let uses_resists = self.settings.effects.debuff_duration_uses_resists;
        self.with_rng(|world, rng| {
            let (Some(cv), Some(tv)) = (world.stats(caster), world.stats(target)) else {
                // A creature that left the world: the plain time.
                return if passive_or_toggle {
                    -1
                } else {
                    skill.abnormal_time
                };
            };
            let resists = (skill.is_debuff && uses_resists).then(|| AbnormalTimeResists {
                stat_mod: match skill.basic_property {
                    SkillBaseStat::Str => tv.bonus(BaseStat::Str),
                    SkillBaseStat::Dex => tv.bonus(BaseStat::Dex),
                    SkillBaseStat::Con => tv.bonus(BaseStat::Con),
                    SkillBaseStat::Int => tv.bonus(BaseStat::Int),
                    SkillBaseStat::Men => tv.bonus(BaseStat::Men),
                    SkillBaseStat::Wit => tv.bonus(BaseStat::Wit),
                    SkillBaseStat::None => 1.0,
                },
                // M5: no trait tables in the stat view yet (see `calc_effect_success`).
                res_mod: 1.0,
                lvl_bonus_mod: calc_lvl_bonus_mod(
                    skill.magic_level,
                    skill.lvl_bonus_rate,
                    cv.level(),
                    tv.level(),
                ),
                element_mod: skill_attribute_bonus(&cv, &tv, skill),
            });
            let input = AbnormalTimeInput {
                skill_is_passive_or_toggle: passive_or_toggle,
                skill_abnormal_time: skill.abnormal_time,
                // The half time of an herb on a servitor needs summons (M6).
                halve_for_servitor: false,
                mastery: SkillMasteryInput {
                    actor_is_player: cv.is_player(),
                    skill_is_static: skill.is_static(),
                    skill_mastery: cv.calc(Stat::SkillMastery, 0.0),
                    str_bonus: cv.bonus(BaseStat::Str),
                    int_bonus: cv.bonus(BaseStat::Int),
                    skill_mastery_rate: cv.calc(Stat::SkillMasteryRate, 1.0),
                    // M5: `SKILL_MASTERY_CHANCE_MULTIPLIERS` is not in the class balance settings
                    // yet; the default is 1.
                    class_balance_multiplier: 1.0,
                },
                resists,
            };
            calc_effect_abnormal_time(&input, rng)
        })
    }

    /// Java `Formulas.calcBuffDebuffReflection(target, skill)`.
    pub(crate) fn calc_buff_debuff_reflection(&mut self, target: ObjectId, skill: &SkillTemplate) -> bool {
        if !skill.is_debuff || skill.activate_rate == -1 {
            return false;
        }
        let ctx = calc_skill(skill);
        let stat = if skill.is_magic() {
            Stat::ReflectSkillMagic
        } else {
            Stat::ReflectSkillPhysic
        };
        let reflect_chance = self
            .stats(target)
            .map_or(0.0, |v| v.calc_stat(stat, 0.0, None, Some(&ctx)));
        reflect_chance > f64::from(self.rng().get_int(100))
    }
}

/// Java `Formulas.calcAttributeBonus(attacker, target, skill)` for a skill.
fn skill_attribute_bonus(attacker: &StatView<'_>, target: &StatView<'_>, skill: &SkillTemplate) -> f64 {
    let element = attribute_element(Some(skill.element), attacker.attack_element());
    let power = (skill.element != -1).then_some(skill.element_power);
    calc_attribute_bonus_skill(
        power,
        attacker.attack_element_value(element),
        target.defense_element_value(element),
    )
}
