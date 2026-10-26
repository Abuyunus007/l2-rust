//! What must hold before a skill is cast: `Player.checkUseMagicConditions` (the player's click)
//! and `Creature.checkDoCastConditions` (every caster, again when the cast begins), plus the
//! skill numbers casting reads (MP cost, magic range) and the target list as the caster sees it.
// Java: entity/actor/Player.java (checkUseMagicConditions)
// Java: entity/actor/Creature.java (checkDoCastConditions)
// Java: entity/actor/stat/CreatureStat.java (getMpConsume, getMpInitialConsume,
//       getMagicalAttackRange)
// Java: mechanics/skill/Skill.java (isContinuous, getTargetList: the messages the handlers send)
//
// # Not modelled (each refuses a cast in Java; none of these states exists yet)
// Observer mode, duels, olympiad, siege friends and territory wars, event
// monsters, castle and fort sieges of doors (a door in a siege zone has no castle or fort yet),
// the access level's `allowPeaceAttack` (not loaded: no level has it), signets in peace zones
// (`checkEffectRangeInsidePeaceZone`, the channelizer is not ported).

use l2_data::SkillTemplate;
use l2_data::skill::{FlyType, TargetType};
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ActionFailed, SystemMessage, SystemMessageParam};
use l2_util::ObjectId;

use super::targets::{TargetContext, TargetMessage, TargetResult};
use crate::effects::EffectType;
use crate::effects::apply::has_negative_effect;
use crate::effects::buff_info::skill_name_param;
use crate::effects::effect_flag;
use crate::objects::ObjectKind;
use crate::stats::Stat;
use crate::stats::skills::calc_skill;
use crate::target::{distance_2d, distance_3d};
use crate::world::World;
use crate::zones::ZoneId;

/// `CommonSkill.PUMPING`.
const PUMPING: i32 = 1313;

/// Java `Skill.isContinuous()`: `A2`, `A4`, `DA2`, or the self-continuous `A3`.
pub(crate) fn is_continuous(skill: &SkillTemplate) -> bool {
    skill.operate_type.is_continuous() || skill.operate_type.is_self_continuous()
}

/// Java `Skill.isChanneling()`.
pub(crate) fn is_channeling(skill: &SkillTemplate) -> bool {
    skill.operate_type.is_channeling()
}

/// Java `LocationUtil.calculateHeadingFrom(obj1, obj2)`.
pub(crate) fn heading_between(from: (i32, i32, i32), to: (i32, i32, i32)) -> i32 {
    crate::movement::heading_from_points(from.0, from.1, to.0, to.1)
}

impl World {
    /// Java `Creature.getKnownSkill(skillId)`: the level the creature knows the skill at.
    pub fn known_skill(&self, id: ObjectId, skill_id: i32) -> Option<crate::effects::SkillKey> {
        let core = self.core_of(id)?;
        core.skills
            .iter()
            .find(|(s, _)| *s == skill_id)
            .map(|&(s, level)| crate::effects::SkillKey::new(s, level))
    }

    /// Java `CreatureStat.getMpConsume(skill)`.
    pub fn mp_consume(&self, id: ObjectId, skill: &SkillTemplate) -> i32 {
        let Some(view) = self.stats(id) else {
            return 0;
        };
        let ctx = calc_skill(skill);
        let base = f64::from(skill.mp_consume);
        let mut mp_consume = base;
        let next_dance_mp_cost = (base / 2.0).ceil();
        let dance_count = self.effect_list(id).map_or(0, |l| l.dance_count());
        if skill.is_dance() && self.settings.cast.dance_consume_additional_mp && dance_count > 0 {
            mp_consume += f64::from(dance_count) * next_dance_mp_cost;
        }
        mp_consume = view.calc_stat(Stat::MpConsume, mp_consume, None, Some(&ctx));
        if skill.is_dance() {
            return view.calc(Stat::DanceMpConsumeRate, mp_consume) as i32;
        }
        if skill.is_magic() {
            return view.calc(Stat::MagicalMpConsumeRate, mp_consume) as i32;
        }
        view.calc(Stat::PhysicalMpConsumeRate, mp_consume) as i32
    }

    /// Java `CreatureStat.getMpInitialConsume(skill)`.
    pub fn mp_initial_consume(&self, id: ObjectId, skill: &SkillTemplate) -> i32 {
        let ctx = calc_skill(skill);
        self.stats(id).map_or(0, |v| {
            v.calc_stat(
                Stat::MpConsume,
                f64::from(skill.mp_initial_consume),
                None,
                Some(&ctx),
            ) as i32
        })
    }

    /// Java `CreatureStat.getMagicalAttackRange(skill)`.
    pub fn magical_attack_range_for(&self, id: ObjectId, skill: &SkillTemplate) -> i32 {
        let ctx = calc_skill(skill);
        self.stats(id).map_or(0, |v| {
            v.calc_stat(
                Stat::MagicAttackRange,
                f64::from(skill.cast_range),
                None,
                Some(&ctx),
            ) as i32
        })
    }

    /// The handler inputs of the caster's cast in progress: the ctrl key of the current skill and
    /// the ground point (Java reads both from the player).
    pub(crate) fn cast_target_context(&self, caster: ObjectId) -> TargetContext {
        let state = self.cast.creatures.get(&caster);
        TargetContext {
            ctrl_pressed: state
                .and_then(|s| s.current_skill)
                .is_some_and(|s| s.ctrl_pressed),
            ground: state.and_then(|s| s.current_skill_world_position),
        }
    }

    /// Java `skill.getTargetList(caster, onlyFirst, target)` as the caster lives it: the handler
    /// turns the caster (`FrontArea`) and its messages reach the caster.
    pub(crate) fn cast_target_list(
        &mut self,
        skill: &SkillTemplate,
        caster: ObjectId,
        only_first: bool,
        target: Option<ObjectId>,
    ) -> Vec<ObjectId> {
        let ctx = self.cast_target_context(caster);
        let result = self.skill_target_list(skill, caster, only_first, target, &ctx);
        self.apply_target_result(caster, skill, &result);
        result.targets
    }

    /// The side effects of a target handler: the heading, then the messages in Java's order.
    pub(crate) fn apply_target_result(
        &mut self,
        caster: ObjectId,
        skill: &SkillTemplate,
        result: &TargetResult,
    ) {
        if let Some(heading) = result.caster_heading {
            self.set_heading(caster, heading);
        }
        for message in &result.messages {
            match message {
                TargetMessage::System(id) => {
                    self.send(caster, &SystemMessage::new(*id));
                }
                TargetMessage::UnsuitableTerms => {
                    self.send(
                        caster,
                        &SystemMessage::new(SystemMessageId::S1CannotBeUsedDueToUnsuitableTerms)
                            .with(skill_name_param(skill)),
                    );
                }
                TargetMessage::Text(text) => self.send_message(caster, text),
            }
        }
    }

    /// Java `_target.asCreature()`: [`World::creature_target`] (`effects/control.rs`) when it is a
    /// creature.
    pub(crate) fn cast_selected_target(&self, id: ObjectId) -> Option<ObjectId> {
        let target = self.creature_target(id)?;
        self.objects
            .get(&target)
            .filter(|e| e.kind.is_creature())
            .map(|_| target)
    }

    /// `isPlayable()`: players and summons.
    pub(crate) fn is_playable(&self, id: ObjectId) -> bool {
        self.is_player(id) || self.is_summon(id)
    }

    /// `GeoEngine.canSeeTarget(creature, worldPosition)`: doors, fences and the geodata between
    /// the creature and a point.
    pub(crate) fn can_see_position(&self, id: ObjectId, to: (i32, i32, i32)) -> bool {
        use l2_geo::DynamicObstacles;

        let Some(from) = self.position_of(id) else {
            return false;
        };
        let instance = self.objects.get(&id).map_or(0, |e| e.instance_id);
        let from = l2_geo::Location::new(from.0, from.1, from.2);
        let to = l2_geo::Location::new(to.0, to.1, to.2);
        let obstacles = self.obstacles();
        if obstacles.doors_between(from, to, instance, true) || obstacles.fence_between(from, to, instance) {
            return false;
        }
        self.geo.as_ref().is_none_or(|geo| geo.can_see_target(from, to))
    }

    /// `isInsideRadius2D(target, radius)`: `calculateDistance2D(target) < radius`.
    pub(crate) fn inside_radius_2d(&self, id: ObjectId, to: (i32, i32, i32), radius: i32) -> bool {
        self.position_of(id)
            .is_some_and(|me| distance_2d(me, to) < f64::from(radius))
    }

    /// `isInsideRadius3D(target, radius)`: `calculateDistance3D(target) < radius`.
    pub(crate) fn inside_radius_3d(&self, id: ObjectId, to: (i32, i32, i32), radius: i32) -> bool {
        self.position_of(id)
            .is_some_and(|me| distance_3d(me, to) < f64::from(radius))
    }

    /// Java `Creature.checkDoCastConditions(skill)`.
    pub(crate) fn check_do_cast_conditions(&mut self, id: ObjectId, skill: &SkillTemplate) -> bool {
        if self.is_skill_disabled(id, skill)
            || (skill.fly_type == Some(FlyType::Charge) && self.is_movement_disabled(id))
        {
            // Send a Server->Client packet ActionFailed to the Player.
            self.send(id, &ActionFailed);
            return false;
        }

        // Check if the caster has enough MP.
        let (cur_hp, cur_mp) = self.core_of(id).map_or((0.0, 0.0), |c| (c.cur_hp, c.cur_mp));
        let needed = self.mp_consume(id, skill) + self.mp_initial_consume(id, skill);
        if cur_mp < f64::from(needed) {
            self.send(id, &SystemMessage::new(SystemMessageId::NotEnoughMp));
            self.send(id, &ActionFailed);
            return false;
        }

        // Check if the caster has enough HP.
        if cur_hp <= f64::from(skill.hp_consume) {
            self.send(id, &SystemMessage::new(SystemMessageId::NotEnoughHp));
            self.send(id, &ActionFailed);
            return false;
        }

        // Skill mute checks.
        if !skill.is_static() {
            // `isMuted()` for spells, `isPhysicalMuted()` for the rest.
            let flag = if skill.is_magic() {
                effect_flag::MUTED
            } else {
                effect_flag::PSYCHICAL_MUTED
            };
            if self.is_affected(id, flag) {
                self.send(id, &ActionFailed);
                return false;
            }
        }

        // Prevent casting signets to peace zone.
        // M5: `ZoneRegion.checkEffectRangeInsidePeaceZone` for channeling skills (signets); the
        // skill channelizer is not ported, so no signet is cast.

        // Check if the caster's weapon is limited to use only its own skills.
        let weapon_refuses = self.stats(id).is_some_and(|v| {
            v.active_weapon_item().is_some_and(|w| {
                let only_own = w.weapon().is_some_and(|info| info.use_weapon_skills_only);
                match &w.skills.item_skills {
                    Some(skills) if only_own && !v.is_gm() => !skills.iter().any(|s| s.id == skill.id),
                    _ => false,
                }
            })
        });
        if weapon_refuses {
            if self.is_player(id) {
                self.send(
                    id,
                    &SystemMessage::new(
                        SystemMessageId::ThatWeaponCannotUseAnyOtherSkillExceptTheWeaponSSkill,
                    ),
                );
            }
            return false;
        }

        // Check if the spell consumes an Item. Only players have an inventory.
        if skill.item_consume_id > 0 && self.is_player(id) {
            let count = self.players.get(&id).map_or(0, |p| {
                p.data
                    .items
                    .iter()
                    .find(|i| {
                        (i.loc == "INVENTORY" || i.loc == "PAPERDOLL") && i.item_id == skill.item_consume_id
                    })
                    .map_or(-1, |i| i.count)
            });
            if count < i64::from(skill.item_consume_count) {
                // Checked: when a summon skill failed, server show required consume item count.
                if self.skill_has_effect_type(skill, &[EffectType::Summon]) {
                    let sm = SystemMessage::new(SystemMessageId::SummoningAServitorCostsS2S1)
                        .with(SystemMessageParam::ItemName(skill.item_consume_id))
                        .with(SystemMessageParam::IntNumber(skill.item_consume_count));
                    self.send(id, &sm);
                } else {
                    self.send(
                        id,
                        &SystemMessage::new(SystemMessageId::ThereAreNotEnoughNecessaryItemsToUseTheSkill),
                    );
                }
                return false;
            }
        }
        true
    }

    /// Java `Player.checkUseMagicConditions(skill, forceUse, dontMove)`.
    pub(crate) fn check_use_magic_conditions(
        &mut self,
        id: ObjectId,
        skill: &SkillTemplate,
        force_use: bool,
        dont_move: bool,
    ) -> bool {
        // ---- Check Player State
        // Abnormal effects (ex: Stun, Sleep...) are checked in Creature useMagic().
        // `isOutOfControl()` (confused or afraid), `isParalyzed()`, `isStunned()`, `isSleeping()`.
        if self.is_affected(
            id,
            effect_flag::CONFUSED
                | effect_flag::FEAR
                | effect_flag::PARALYZED
                | effect_flag::STUNNED
                | effect_flag::SLEEP,
        ) {
            self.send(id, &ActionFailed);
            return false;
        }

        // Check if the player is dead.
        if self.is_dead(id) {
            self.send(id, &ActionFailed);
            return false;
        }

        if self.is_fishing(id)
            && !self.skill_has_effect_type(skill, &[EffectType::Fishing, EffectType::FishingStart])
        {
            // Only fishing skills are available.
            // NOTE(java-quirk): this refusal sends no `ActionFailed`.
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::OnlyFishingSkillsMayBeUsedAtThisTime),
            );
            return false;
        }

        // Pumping and reeling are refused here, so that no reuse delay is applied when there is
        // no fish to fight (`_fishCombat == null`).
        if !self.has_fish_combat(id) && self.skill_has_effect_type(skill, &[EffectType::Fishing]) {
            let message = if skill.id == PUMPING {
                SystemMessageId::YouMayOnlyUseThePumpingSkillWhileYouAreFishing
            } else {
                SystemMessageId::YouMayOnlyUseTheReelingSkillWhileYouAreFishing
            };
            self.send(id, &SystemMessage::new(message));
            self.send(id, &ActionFailed);
            return false;
        }

        // Observer mode: not modelled.

        // Check if the caster is sitting.
        if self.is_sitting(id) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouCannotMoveWhileSitting),
            );
            self.send(id, &ActionFailed);
            return false;
        }

        // Check if the skill type is toggle.
        if skill.is_toggle() && self.is_affected_by_skill(id, skill.id) {
            self.stop_skill_effects(id, crate::effects::SkillFinishType::Removed, skill.id);
            self.send(id, &ActionFailed);
            return false;
        }

        // Check if the player uses "Fake Death" skill (not before the toggle reset).
        if self.is_fake_death(id) {
            self.send(id, &ActionFailed);
            return false;
        }

        // ---- Check Target
        let target_type = skill.target_type;
        let ground = self
            .cast
            .creatures
            .get(&id)
            .and_then(|s| s.current_skill_world_position);
        if target_type == TargetType::Ground && ground.is_none() {
            self.send(id, &ActionFailed);
            return false;
        }

        let target = match target_type {
            // Target the player if skill type is AURA, PARTY, CLAN or SELF.
            TargetType::Aura
            | TargetType::FrontAura
            | TargetType::BehindAura
            | TargetType::Party
            | TargetType::Clan
            | TargetType::PartyClan
            | TargetType::Ground
            | TargetType::SelfTarget
            | TargetType::AreaSummon
            | TargetType::AuraCorpseMob
            | TargetType::CommandChannel
            | TargetType::AuraFriendly => Some(id),
            // `_summon`: the player's summon.
            TargetType::Pet | TargetType::Servitor | TargetType::Summon => self.summon_of(id),
            _ => self.target_of(id),
        };

        // Check the validity of the target.
        let Some(target) = target else {
            self.send(id, &ActionFailed);
            return false;
        };
        let target_kind = self.objects.get(&target).map(|e| e.kind);

        // Skills can be used on walls and doors only during siege.
        if target_kind == Some(ObjectKind::Door) {
            if self.is_inside_zone(target, ZoneId::Siege) {
                // Check if the door belongs to a castle and if the siege is in progress
                // (M7-SIEGE-01).
                let castle = self
                    .door(target)
                    .and_then(|d| self.castle_at(d.x, d.y, d.z))
                    .filter(|c| c.0 > 0);
                if castle.is_some_and(|c| !self.is_castle_siege_in_progress(c)) {
                    self.send(id, &SystemMessage::new(SystemMessageId::InvalidTarget));
                    return false;
                }
                // Check if the door belongs to a fort and if the siege is in progress.
                let door = self.door(target).map(|d| (d.x, d.y, d.z, d.template.show_hp));
                if let Some((x, y, z, show_hp)) = door
                    && self
                        .fort_at(x, y, z)
                        .filter(|f| f.0 > 0)
                        .is_some_and(|f| !self.fort_siege_in_progress(f) || !show_hp)
                {
                    self.send(id, &SystemMessage::new(SystemMessageId::InvalidTarget));
                    return false;
                }
            } else if target_type != TargetType::Unlockable {
                self.send(id, &SystemMessage::new(SystemMessageId::InvalidTarget));
                return false;
            }
        }

        // Are the target and the player in the same duel?
        // `target.isPlayable()` / `asPlayer()`: a summon target counts as its owner.
        if self.is_in_duel(id)
            && let Some(other) = self.acting_player(target)
            && self.duel_id_of(other) != self.duel_id_of(id)
        {
            self.send_message(id, "You cannot do this while duelling.");
            self.send(id, &ActionFailed);
            return false;
        }

        // ---- Check skill availability
        // Check if this skill is enabled (ex : reuse time).
        if self.is_skill_disabled(id, skill) {
            let hash = skill.hash_code();
            let sm = if self.has_skill_reuse(id, hash) {
                let remaining_time = (self.skill_remaining_reuse_time(id, hash) / 1000) as i32;
                let hours = remaining_time / 3600;
                let minutes = (remaining_time % 3600) / 60;
                let seconds = remaining_time % 60;
                let sm = if hours > 0 {
                    SystemMessage::new(
                        SystemMessageId::ThereAreS2HourSS3MinuteSAndS4SecondSRemainingInS1SReUseTime,
                    )
                    .with(skill_name_param(skill))
                    .with(SystemMessageParam::IntNumber(hours))
                    .with(SystemMessageParam::IntNumber(minutes))
                } else if minutes > 0 {
                    SystemMessage::new(SystemMessageId::ThereAreS2MinuteSS3SecondSRemainingInS1SReUseTime)
                        .with(skill_name_param(skill))
                        .with(SystemMessageParam::IntNumber(minutes))
                } else {
                    SystemMessage::new(SystemMessageId::ThereAreS2SecondSRemainingInS1SReUseTime)
                        .with(skill_name_param(skill))
                };
                sm.with(SystemMessageParam::IntNumber(seconds))
            } else {
                SystemMessage::new(SystemMessageId::S1IsNotAvailableAtThisTimeBeingPreparedForReuse)
                    .with(skill_name_param(skill))
            };
            self.send(id, &sm);
            return false;
        }

        // ---- Check casting conditions
        if !self.check_skill_condition(id, target, crate::effects::SkillKey::of(skill)) {
            self.send(id, &ActionFailed);
            return false;
        }

        // ---- Check Skill Type
        // Check if this is bad magic skill.
        if has_negative_effect(skill) {
            if self.is_inside_peace_zone(id, id, target) {
                // If Creature or target is in a peace zone, send a system message TARGET_IN_PEACEZONE
                // a Server->Client packet ActionFailed.
                self.send(
                    id,
                    &SystemMessage::new(SystemMessageId::YouMayNotAttackThisTargetInAPeacefulZone),
                );
                self.send(id, &ActionFailed);
                return false;
            }
            // Olympiad: M8.
            if self.is_siege_friend(id, target) {
                // `YOU_CANNOT_FORCE_ATTACK_A_MEMBER_OF_THE_SAME_TERRITORY` in Territory War
                // (M7-TW-01).
                let message = if self.is_tw_in_progress() {
                    SystemMessageId::YouCannotForceAttackAMemberOfTheSameTerritory
                } else {
                    SystemMessageId::ForceAttackIsImpossibleAgainstATemporaryAlliedMemberDuringASiege
                };
                self.send(id, &SystemMessage::new(message));
                self.send(id, &ActionFailed);
                return false;
            }
            if !self.can_be_attacked(target) && target_kind != Some(ObjectKind::Door) {
                // If target is not attackable, send a Server->Client packet ActionFailed.
                self.send(id, &ActionFailed);
                return false;
            }
            // Event monsters: none.
            // Check if a Forced ATTACK is in progress on non-attackable target.
            if !self.is_auto_attackable_by(target, id) && !force_use {
                let allowed = matches!(
                    target_type,
                    TargetType::Aura
                        | TargetType::FrontAura
                        | TargetType::BehindAura
                        | TargetType::AuraCorpseMob
                        | TargetType::Clan
                        | TargetType::Party
                        | TargetType::SelfTarget
                        | TargetType::Ground
                        | TargetType::AreaSummon
                        | TargetType::Unlockable
                        | TargetType::AuraFriendly
                );
                if !allowed {
                    self.send(id, &ActionFailed);
                    return false;
                }
            }
            // Check if the target is in the skill cast range.
            if dont_move {
                let radius = self.collision_radius_of(id);
                if target_type == TargetType::Ground {
                    let point = ground.unwrap_or_default();
                    if !self.inside_radius_2d(id, point, skill.cast_range.wrapping_add(radius)) {
                        self.send(id, &SystemMessage::new(SystemMessageId::YourTargetIsOutOfRange));
                        self.send(id, &ActionFailed);
                        return false;
                    }
                } else if skill.cast_range > 0 {
                    let in_range = self
                        .position_of(target)
                        .is_some_and(|t| self.inside_radius_2d(id, t, skill.cast_range.wrapping_add(radius)));
                    if !in_range {
                        self.send(id, &SystemMessage::new(SystemMessageId::YourTargetIsOutOfRange));
                        self.send(id, &ActionFailed);
                        return false;
                    }
                }
            }
        }

        // Check if the skill is a good magic, target is a monster and if force attack is set, if
        // not then we don't want to cast.
        if skill.effect_point > 0 && self.is_monster(target) && !force_use {
            self.send(id, &ActionFailed);
            return false;
        }

        // Check if this is a Pvp skill and target isn't a non-flagged/non-karma player.
        let pvp_checked_elsewhere = matches!(
            target_type,
            TargetType::Party
                | TargetType::Clan
                | TargetType::PartyClan
                | TargetType::Aura
                | TargetType::FrontAura
                | TargetType::BehindAura
                | TargetType::AreaSummon
                | TargetType::Ground
                | TargetType::SelfTarget
        );
        if !pvp_checked_elsewhere && self.is_playable(target) {
            let ctx = self.cast_target_context(id);
            if !self.check_pvp_skill(id, target, skill, &ctx) {
                self.send(id, &SystemMessage::new(SystemMessageId::ThatIsAnIncorrectTarget));
                self.send(id, &ActionFailed);
                return false;
            }
        }

        // GeoData Los Check here.
        if skill.cast_range > 0 {
            let sees = if target_type == TargetType::Ground {
                ground.is_some_and(|point| self.can_see_position(id, point))
            } else {
                self.can_see_target(id, target)
            };
            if !sees {
                self.send(id, &SystemMessage::new(SystemMessageId::CannotSeeTarget));
                self.send(id, &ActionFailed);
                return false;
            }
        }

        if skill.fly_type == Some(FlyType::Charge) {
            let (Some(from), Some(to)) = (self.position_of(id), self.position_of(target)) else {
                return false;
            };
            let instance = self.objects.get(&id).map_or(0, |e| e.instance_id);
            let can_move = self.geo_can_move_to_target(
                l2_geo::Location::new(from.0, from.1, from.2),
                l2_geo::Location::new(to.0, to.1, to.2),
                instance,
            );
            if !can_move {
                self.send(
                    id,
                    &SystemMessage::new(SystemMessageId::TheTargetIsLocatedWhereYouCannotCharge),
                );
                return false;
            }
        }

        // Finally, after passing all conditions.
        true
    }
}
