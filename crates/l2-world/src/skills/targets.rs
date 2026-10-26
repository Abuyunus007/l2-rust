//! The skill target handlers: `Skill.getTargetList(creature, onlyFirst, target)` and the 34
//! `ITargetTypeHandler`s it dispatches to.
// Java: mechanics/skill/Skill.java (getTargetList, getFirstOfTargetList,
//       checkForAreaOffensiveSkills, addSummon, addCharacter)
// Java: handler/ITargetTypeHandler.java, handler/TargetHandler.java
// Java: dist/game/data/scripts/handlers/skill/targets/*.java (one `Targeting::<handler>` each)
// Java: entity/actor/Player.java (checkPvpSkill), util/LocationUtil.java (checkIfInRange)
//
// # Shape
// Targeting only reads the world. Java's handlers also *send* messages to the caster and, in
// `FrontArea`, turn it towards the origin; here both are returned in a [`TargetResult`] and the
// caster (M5-CAST-01) decides what to do with them. `skill.getAffectLimit()` draws from the
// RNG (`affectLimit` is `min-max`), so the entry points take an RNG; the draws happen where
// Java's handlers call `getAffectLimit()`, so a seeded world replays them.
//
// # What is not there yet (each marked `// M6:` or `// M5:` where it would act)
// - Summons, pets, servitors, command channels, clans, alliances, duels, olympiad, sieges,
//   events, auto play, observer mode: none exist. Java's code path for "none of these" is
//   followed: clan = only the caster, summon targets find nothing. Parties are real since
//   M6-PARTY-01 (`party/`).
// - PvP flags (`Player.getPvpFlag()`): the world has none, they read as 0.
// - NPCs have no zone flags in the world, so `isInsideZone` is false for them.
//
// # Quirks kept
// - `getTargetList` filters players out of a monster's non-negative skill only when the handler
//   returned a `LinkedList`; no handler does, so the filter never runs (and is not ported).
// - Handlers that throw (a null `player`, ...) end in `Skill.getTargetList`'s catch: no targets and
//   "Target type of skill is not currently handled." to the caster ([`NOT_HANDLED_TEXT`]).

use l2_data::skill::{SkillTemplate, TargetType};
use l2_protocol_game::SystemMessageId;
use l2_util::ObjectId;

use crate::combat::hit::position_from;
use crate::formulas::{Position, Rnd};
use crate::movement::heading_from_points;
use crate::objects::ObjectKind;
use crate::world::World;
use crate::zones::ZoneId;

/// What `Skill.getTargetList` sends when no handler is registered for the target type or the
/// handler throws (`creature.sendMessage(...)`).
pub const NOT_HANDLED_TEXT: &str = "Target type of skill is not currently handled.";

/// The target types that have a handler (`TargetHandler._datatable`, 34 entries). The other
/// target types (`AREA_UNDEAD`, `CORPSE`, `NONE`, `UNDEAD`) get [`NOT_HANDLED_TEXT`], as in Java.
pub const HANDLED_TARGET_TYPES: [TargetType; 34] = [
    TargetType::Area,
    TargetType::AreaCorpseMob,
    TargetType::AreaFriendly,
    TargetType::AreaSummon,
    TargetType::Aura,
    TargetType::AuraCorpseMob,
    TargetType::AuraFriendly,
    TargetType::BehindArea,
    TargetType::BehindAura,
    TargetType::Clan,
    TargetType::ClanMember,
    TargetType::CommandChannel,
    TargetType::CorpseClan,
    TargetType::CorpseMob,
    TargetType::EnemySummon,
    TargetType::Flagpole,
    TargetType::FrontArea,
    TargetType::FrontAura,
    TargetType::Ground,
    TargetType::Holy,
    TargetType::One,
    TargetType::OwnerPet,
    TargetType::Party,
    TargetType::PartyClan,
    TargetType::PartyMember,
    TargetType::PartyNotMe,
    TargetType::PartyOther,
    TargetType::PcBody,
    TargetType::Pet,
    TargetType::SelfTarget,
    TargetType::Servitor,
    TargetType::Summon,
    TargetType::TargetParty,
    TargetType::Unlockable,
];

/// Java `TargetHandler.getHandler(type) != null`.
pub fn is_target_type_handled(target_type: TargetType) -> bool {
    HANDLED_TARGET_TYPES.contains(&target_type)
}

/// A message a handler sends to the caster in Java. The caster's side decides when to send it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetMessage {
    /// `creature.sendPacket(SystemMessageId)`.
    System(SystemMessageId),
    /// `S1_CANNOT_BE_USED_DUE_TO_UNSUITABLE_TERMS` with `addSkillName(skill)` (`PartyMember`).
    UnsuitableTerms,
    /// `creature.sendMessage(text)`: a plain system chat line.
    Text(&'static str),
}

/// What a handler returns: the targets in Java's order, the messages it sent, and the heading it
/// set on the caster.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TargetResult {
    /// Java's `List<WorldObject>`.
    pub targets: Vec<ObjectId>,
    /// In the order Java sent them.
    pub messages: Vec<TargetMessage>,
    /// `FrontArea` turns the caster towards the origin (`creature.setHeading(...)`) before it
    /// looks who is in front. The handler used the new heading; the caster applies it.
    pub caster_heading: Option<i32>,
}

impl TargetResult {
    /// Java `getFirstOfTargetList`: the first target, if any.
    pub fn first(&self) -> Option<ObjectId> {
        self.targets.first().copied()
    }

    fn incorrect_target(&mut self) {
        self.messages
            .push(TargetMessage::System(SystemMessageId::ThatIsAnIncorrectTarget));
    }
}

/// Inputs a handler reads from the caster's cast in progress, which the world does not keep yet.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TargetContext {
    /// `_currentSkill.isCtrlPressed()` (`Player.checkPvpSkill`): the force-attack key.
    pub ctrl_pressed: bool,
    /// `Player.getCurrentSkillWorldPosition()`: the ground point of a `GROUND` skill.
    pub ground: Option<(i32, i32, i32)>,
}

/// The effect names whose Java classes return these `EffectType`s from `getEffectType()`
/// (`dist/game/data/scripts/handlers/skill/effects/*.java`). `SUMMON` has no effect class, so
/// `hasEffectType(SUMMON)` is always false.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EffectKind {
    /// `MAGICAL_ATTACK`, `HP_DRAIN`, `PHYSICAL_ATTACK`, `PHYSICAL_ATTACK_HP_LINK`: `Skill.isDamage()`.
    Damage,
    /// `HP_DRAIN` alone (`HpDrain`), not the other damage types.
    HpDrain,
    StealAbnormal,
    Resurrection,
    SummonNpc,
}

impl EffectKind {
    fn matches(self, effect_name: &str) -> bool {
        match self {
            Self::Damage => matches!(
                effect_name,
                "Backstab"
                    | "EnergyDamage"
                    | "FatalBlow"
                    | "HpDrain"
                    | "MagicalDamage"
                    | "MagicalDamageByAbnormal"
                    | "MagicalDamageMp"
                    | "MagicalSoulDamage"
                    | "PhysicalDamage"
                    | "PhysicalDamageHpLink"
                    | "PhysicalSoulDamage"
                    | "SoulBlow"
            ),
            Self::HpDrain => effect_name == "HpDrain",
            Self::StealAbnormal => effect_name == "StealAbnormal",
            Self::Resurrection => effect_name == "Resurrection",
            Self::SummonNpc => effect_name == "SummonNpc",
        }
    }
}

/// Java `Skill.hasEffectType(...)`: any effect of any scope has this type.
fn has_effect(skill: &SkillTemplate, kind: EffectKind) -> bool {
    skill.effects.iter().any(|(_, effect)| kind.matches(&effect.name))
}

/// Java `Skill.hasNegativeEffect()`.
fn has_negative_effect(skill: &SkillTemplate) -> bool {
    skill.effect_point < 0 && skill.target_type != TargetType::SelfTarget
}

impl World {
    /// Java `Skill.getTargetList(creature, onlyFirst, target)`: the targets of `skill` cast by
    /// `caster` at `target` (`None` = no creature targeted). Draws from the world RNG where
    /// Java calls `skill.getAffectLimit()`.
    pub fn skill_target_list(
        &mut self,
        skill: &SkillTemplate,
        caster: ObjectId,
        only_first: bool,
        target: Option<ObjectId>,
        ctx: &TargetContext,
    ) -> TargetResult {
        self.with_rng(|world, rng| world.skill_target_list_with(rng, skill, caster, only_first, target, ctx))
    }

    /// [`Self::skill_target_list`] with the RNG passed in (replays and tests).
    pub fn skill_target_list_with(
        &self,
        rng: &mut impl Rnd,
        skill: &SkillTemplate,
        caster: ObjectId,
        only_first: bool,
        target: Option<ObjectId>,
        ctx: &TargetContext,
    ) -> TargetResult {
        self.skill_target_list_with_type(rng, skill, skill.target_type, caster, only_first, target, ctx)
    }

    /// `TargetHandler.getInstance().getHandler(targetType).getTargetList(skill, caster, onlyFirst,
    /// target)`: the handler of `target_type` runs on `skill`, whatever target type the skill has
    /// (the trigger skill effects pick the handler from their own `targetType` parameter).
    pub fn skill_target_list_as(
        &mut self,
        skill: &SkillTemplate,
        target_type: TargetType,
        caster: ObjectId,
        only_first: bool,
        target: Option<ObjectId>,
        ctx: &TargetContext,
    ) -> TargetResult {
        self.with_rng(|world, rng| {
            world.skill_target_list_with_type(rng, skill, target_type, caster, only_first, target, ctx)
        })
    }

    /// [`Self::skill_target_list_as`] with the RNG passed in.
    #[allow(clippy::too_many_arguments)]
    pub fn skill_target_list_with_type(
        &self,
        rng: &mut impl Rnd,
        skill: &SkillTemplate,
        target_type: TargetType,
        caster: ObjectId,
        only_first: bool,
        target: Option<ObjectId>,
        ctx: &TargetContext,
    ) -> TargetResult {
        let mut t = Targeting {
            w: self,
            rng,
            skill,
            target_type,
            caster,
            only_first,
            target,
            ctx,
            out: TargetResult::default(),
        };
        t.run();
        t.out
    }

    /// Java `Skill.getTargetList(creature, onlyFirst)`: the target is what the caster has
    /// selected, if it is a creature.
    pub fn skill_target_list_current(
        &mut self,
        skill: &SkillTemplate,
        caster: ObjectId,
        only_first: bool,
        ctx: &TargetContext,
    ) -> TargetResult {
        let target = self.current_creature_target(caster);
        self.skill_target_list(skill, caster, only_first, target, ctx)
    }

    /// Java `Skill.getFirstOfTargetList(creature)`: `getTargetList(creature, true)`; the caller
    /// takes [`TargetResult::first`] (`null` in Java when the list is empty).
    pub fn skill_first_of_target_list(
        &mut self,
        skill: &SkillTemplate,
        caster: ObjectId,
        ctx: &TargetContext,
    ) -> TargetResult {
        self.skill_target_list_current(skill, caster, true, ctx)
    }

    /// `creature.getTarget()` when it is a `Creature`. NPCs keep no target of their own in the
    /// world yet (M5), so this is `None` for them; a summon's is the one its owner ordered.
    fn current_creature_target(&self, caster: ObjectId) -> Option<ObjectId> {
        let target = if self.is_summon(caster) {
            self.creature_target(caster)?
        } else {
            self.target_of(caster)?
        };
        self.objects
            .get(&target)
            .filter(|e| e.kind.is_creature())
            .map(|_| target)
    }

    /// Java `Skill.checkForAreaOffensiveSkills(caster, target, skill, sourceInArena)`: should
    /// `target` be in the target list of an area skill. Distance is not checked.
    pub fn check_for_area_offensive_skills(
        &self,
        caster: ObjectId,
        target: ObjectId,
        skill: &SkillTemplate,
        source_in_arena: bool,
        ctx: &TargetContext,
    ) -> bool {
        if self.is_dead_creature(target) || target == caster {
            return false;
        }
        if self.is_player(caster) {
            if self.is_player(target) {
                // M5/M6: auto play settings, observer mode, event teams. None exist.
                // The same side of the same siege (M7-SIEGE-01).
                let state = self.siege_state_of(caster);
                if has_negative_effect(skill)
                    && state > 0
                    && self.is_inside_zone(caster, ZoneId::Siege)
                    && state == self.siege_state_of(target)
                    && self.siege_side_of(caster) == self.siege_side_of(target)
                {
                    return false;
                }
                if has_negative_effect(skill) && self.is_inside_zone(target, ZoneId::Peace) {
                    return false;
                }
                // Same party (a command channel would be the same; there are none).
                if self.is_in_party_with(caster, target) {
                    return false;
                }
                if !source_in_arena
                    && !(self.is_inside_zone(target, ZoneId::Pvp)
                        && !self.is_inside_zone(target, ZoneId::Siege))
                {
                    // Same alliance, same clan (M6-CLAN-01).
                    let ally_id = self.ally_id_of(caster);
                    if ally_id != 0 && ally_id == self.ally_id_of(target) {
                        return false;
                    }
                    let clan_id = self.clan_id_of(caster);
                    if clan_id != 0 && clan_id == self.clan_id_of(target) {
                        return false;
                    }
                    if !self.check_pvp_skill(caster, target, skill, ctx) {
                        return false;
                    }
                }
            }
        } else if !self.is_player(target)
            && self.is_attackable_creature(target)
            && self.is_attackable_creature(caster)
        {
            // The caster is a mob: it does not hit other mobs.
            return false;
        }
        self.can_see_target(caster, target)
    }

    /// Java `Player.checkPvpSkill(target, skill)` for a player target (anything else is `true`).
    pub fn check_pvp_skill(
        &self,
        player: ObjectId,
        target: ObjectId,
        skill: &SkillTemplate,
        ctx: &TargetContext,
    ) -> bool {
        if !self.is_player(target) {
            return true;
        }
        if !(skill.is_debuff || has_effect(skill, EffectKind::StealAbnormal) || has_negative_effect(skill)) {
            return true;
        }
        if player == target {
            return false;
        }
        // Duel ("fight is on").
        if self.is_in_duel_with(player, target) {
            return true;
        }
        // M8: the same olympiad game (returns true as well).
        if self.is_inside_zone(target, ZoneId::Peace) {
            return false;
        }
        let target_flagged = self.pvp_flag_of(target) != 0 || self.karma_of(target) != 0;
        if skill.is_pvp_only
            && !target_flagged
            && !self.is_inside_zone(player, ZoneId::Pvp)
            && !self.is_inside_zone(player, ZoneId::Siege)
        {
            return false;
        }
        // Siege (M7-SIEGE-01).
        if self.is_siege_friend(player, target) {
            self.send(
                player,
                &l2_protocol_game::server::SystemMessage::new(
                    SystemMessageId::ForceAttackIsImpossibleAgainstATemporaryAlliedMemberDuringASiege,
                ),
            );
            return false;
        }
        // Party (and command channel): only a force attack with a damage skill hits a member.
        if self.is_in_party_with(player, target) {
            return skill.effect_range > 0
                && ctx.ctrl_pressed
                && self.target_of(player) == Some(target)
                && has_effect(skill, EffectKind::Damage);
        }
        // You can debuff anyone except party members while in an arena.
        if self.is_inside_zone(player, ZoneId::Pvp) && self.is_inside_zone(target, ZoneId::Pvp) {
            return true;
        }
        // Clan war, same clan or alliance (M6-CLAN-01).
        if let (Some(my_clan), Some(t_clan)) = (self.clan_of(player), self.clan_of(target)) {
            if my_clan.is_at_war_with(t_clan.id) && t_clan.is_at_war_with(my_clan.id) {
                // Always return true at war.
                return true;
            } else if my_clan.id == t_clan.id || (my_clan.ally_id > 0 && my_clan.ally_id == t_clan.ally_id) {
                // Check if skill can do dmg.
                return skill.effect_range > 0
                    && ctx.ctrl_pressed
                    && self.target_of(player) == Some(target)
                    && has_effect(skill, EffectKind::Damage);
            }
        }
        // On retail, it is impossible to debuff a "peaceful" player.
        if !target_flagged {
            return skill.effect_range > 0
                && ctx.ctrl_pressed
                && self.target_of(player) == Some(target)
                && has_effect(skill, EffectKind::Damage);
        }
        self.pvp_flag_of(target) > 0 || self.karma_of(target) > 0
    }

    /// Java `Skill.addCharacter(caster, target, radius, isDead)`.
    pub fn skill_add_character(
        &self,
        caster: ObjectId,
        target: ObjectId,
        radius: i32,
        is_dead: bool,
    ) -> bool {
        if is_dead != self.is_dead_creature(target) {
            return false;
        }
        if radius > 0 && !self.check_if_in_range(radius, caster, target, true) {
            return false;
        }
        true
    }

    /// Java `LocationUtil.checkIfInRange(range, obj1, obj2, includeZ)`: the same instance and the
    /// distance within `range` plus both collision radii.
    pub fn check_if_in_range(&self, range: i32, a: ObjectId, b: ObjectId, include_z: bool) -> bool {
        let (Some(pa), Some(pb)) = (self.position_of(a), self.position_of(b)) else {
            return false;
        };
        if self.skill_instance_of(a) != self.skill_instance_of(b) {
            return false;
        }
        let combined = self.skill_collision_radius(a) + self.skill_collision_radius(b);
        let dx = f64::from(pa.0) - f64::from(pb.0);
        let dy = f64::from(pa.1) - f64::from(pb.1);
        let dz = if include_z {
            f64::from(pa.2) - f64::from(pb.2)
        } else {
            0.0
        };
        (dx * dx + dy * dy + dz * dz).sqrt() <= f64::from(range.wrapping_add(combined))
    }

    fn skill_instance_of(&self, id: ObjectId) -> i32 {
        self.objects.get(&id).map_or(0, |e| e.instance_id)
    }

    /// `getTemplate().getCollisionRadius()` of a creature (`isCreature()` only, else 0).
    fn skill_collision_radius(&self, id: ObjectId) -> i32 {
        match self.objects.get(&id).map(|e| e.kind) {
            Some(ObjectKind::Door) => self.doors.get(&id).map_or(0, |d| d.template.collision_radius),
            _ => self.collision_radius_of(id),
        }
    }

    /// `isDead()` of a creature: doors keep their own flag.
    fn is_dead_creature(&self, id: ObjectId) -> bool {
        match self.objects.get(&id).map(|e| e.kind) {
            Some(ObjectKind::Door) => self.doors.get(&id).is_some_and(|d| d.dead),
            _ => self.is_dead(id),
        }
    }

    /// Java `isAttackable()`: `Attackable` NPCs (monsters, guards, ...).
    fn is_attackable_creature(&self, id: ObjectId) -> bool {
        self.is_attackable_npc(id)
    }

    fn is_door(&self, id: ObjectId) -> bool {
        self.objects.get(&id).is_some_and(|e| e.kind == ObjectKind::Door)
    }

    fn karma_of(&self, id: ObjectId) -> i32 {
        self.players.get(&id).map_or(0, |p| p.data.karma)
    }

    /// Every creature in the regions around `id`, same instance, without `id`
    /// (`World.forEachVisibleObject(id, Creature.class, ...)`).
    fn visible_creatures(&self, id: ObjectId) -> Vec<ObjectId> {
        self.visible_objects(id)
            .into_iter()
            .filter(|o| self.objects.get(o).is_some_and(|e| e.kind.is_creature()))
            .collect()
    }

    /// `World.getVisibleObjectsInRange(id, Creature.class, range)` (3D distance, no radius).
    fn visible_creatures_in_range(&self, id: ObjectId, range: i32) -> Vec<ObjectId> {
        self.visible_objects_in_range(id, range)
            .into_iter()
            .filter(|o| self.objects.get(o).is_some_and(|e| e.kind.is_creature()))
            .collect()
    }

    fn npc_template_for_skill(&self, id: ObjectId) -> Option<&l2_data::NpcTemplate> {
        let npc = self.npcs.get(&id)?;
        self.npc_world.template(npc.npc_id)
    }

    /// Java `npc.isInMyClan(other)` = `getTemplate().isClan(other.getTemplate().getClans())`.
    // NOTE: Java tells a template without clans (null) from one with an empty set; the template
    // here has one set, and empty is read as "no clans".
    fn npc_in_my_clan(&self, npc: ObjectId, other: ObjectId) -> bool {
        let (Some(mine), Some(theirs)) = (
            self.npc_template_for_skill(npc),
            self.npc_template_for_skill(other),
        ) else {
            return false;
        };
        if theirs.clans.is_empty() {
            return false;
        }
        if mine.clans.contains(&self.npc_world.generic_clan_id()) {
            return true;
        }
        theirs.clans.iter().any(|c| mine.clans.contains(c))
    }

    /// Java `Position.getPosition(obj, creature)` with the creature facing `heading`.
    fn position_of_towards(&self, obj: ObjectId, creature: ObjectId, heading: i32) -> Option<Position> {
        let from = self.position_of(obj)?;
        let to = self.position_of(creature)?;
        Some(position_from(from.0, from.1, to.0, to.1, heading))
    }
}

/// One `getTargetList` call: the inputs of the handler and what it collects.
struct Targeting<'a, R: Rnd> {
    w: &'a World,
    rng: &'a mut R,
    skill: &'a SkillTemplate,
    /// The handler to run: the skill's own target type, or the one a trigger effect names.
    target_type: TargetType,
    caster: ObjectId,
    only_first: bool,
    target: Option<ObjectId>,
    ctx: &'a TargetContext,
    out: TargetResult,
}

impl<R: Rnd> Targeting<'_, R> {
    /// `TargetHandler.getInstance().getHandler(getTargetType())` and the call.
    fn run(&mut self) {
        match self.target_type {
            TargetType::Area => self.area(),
            TargetType::AreaCorpseMob => self.area_corpse_mob(),
            TargetType::AreaFriendly => self.area_friendly(),
            TargetType::AreaSummon => self.area_summon(),
            TargetType::Aura => self.aura(),
            TargetType::AuraCorpseMob => self.aura_corpse_mob(),
            TargetType::AuraFriendly => self.aura_friendly(),
            TargetType::BehindArea => self.behind_area(),
            TargetType::BehindAura => self.behind_aura(),
            TargetType::Clan => self.clan(),
            TargetType::ClanMember => self.clan_member(),
            TargetType::CommandChannel => self.command_channel(),
            TargetType::CorpseClan => self.corpse_clan(),
            TargetType::CorpseMob => self.corpse_mob(),
            TargetType::EnemySummon => self.enemy_summon(),
            TargetType::Flagpole => self.flag_pole(),
            TargetType::FrontArea => self.front_area(),
            TargetType::FrontAura => self.front_aura(),
            TargetType::Ground => self.ground(),
            TargetType::Holy => self.holy(),
            TargetType::One => self.one(),
            TargetType::OwnerPet => self.owner_pet(),
            TargetType::Party => self.party(),
            TargetType::PartyClan => self.party_clan(),
            TargetType::PartyMember => self.party_member(),
            TargetType::PartyNotMe => self.party_not_me(),
            TargetType::PartyOther => self.party_other(),
            TargetType::PcBody => self.pc_body(),
            TargetType::Pet => self.pet(),
            TargetType::SelfTarget => self.self_target(),
            TargetType::Servitor => self.servitor(),
            TargetType::Summon => self.summon(),
            TargetType::TargetParty => self.target_party(),
            TargetType::Unlockable => self.unlockable(),
            // No handler is registered for these: Java sends the text and returns an empty list.
            TargetType::AreaUndead | TargetType::Corpse | TargetType::Undead | TargetType::None => {
                self.not_handled()
            }
        }
    }

    // ------------------------------------------------------------------ helpers

    /// A handler threw (null `player`, ...): `Skill.getTargetList` catches it, drops the list and
    /// sends the "not handled" text.
    fn not_handled(&mut self) {
        self.out.targets.clear();
        self.out.messages.push(TargetMessage::Text(NOT_HANDLED_TEXT));
    }

    /// Java `skill.getAffectLimit()`: `_affectLimit[0] + Rnd.get(_affectLimit[1])`.
    fn affect_limit(&mut self) -> i32 {
        let [base, random] = self.skill.affect_limit;
        base.wrapping_add(self.rng.get_int(random))
    }

    /// `targetList.size() >= n`.
    fn size_at_least(&self, n: i32) -> bool {
        i64::try_from(self.out.targets.len()).unwrap_or(i64::MAX) >= i64::from(n)
    }

    /// The `maxTargets > 0 && size >= maxTargets` test of the area handlers.
    fn limit_reached(&self, max_targets: i32) -> bool {
        max_targets > 0 && self.size_at_least(max_targets)
    }

    fn push(&mut self, id: ObjectId) {
        self.out.targets.push(id);
    }

    /// `creature.isInsideZone(PVP) && !creature.isInsideZone(SIEGE)`.
    fn src_in_arena(&self) -> bool {
        self.w.is_inside_zone(self.caster, ZoneId::Pvp) && !self.w.is_inside_zone(self.caster, ZoneId::Siege)
    }

    fn check_area(&self, obj: ObjectId, src_in_arena: bool) -> bool {
        self.w
            .check_for_area_offensive_skills(self.caster, obj, self.skill, src_in_arena, self.ctx)
    }

    fn in_range(&self, range: i32, a: ObjectId, b: ObjectId) -> bool {
        self.w.check_if_in_range(range, a, b, true)
    }

    fn attackable(&self, id: ObjectId) -> bool {
        self.w.is_attackable_creature(id)
    }

    fn playable(&self, id: ObjectId) -> bool {
        self.w.is_playable(id)
    }

    /// `creature.isInParty() && target.isInParty()` with the same leader: a summon is in its
    /// owner's party.
    fn same_party(&self, a: ObjectId, b: ObjectId) -> bool {
        match (self.w.acting_player(a), self.w.acting_player(b)) {
            (Some(pa), Some(pb)) => self.w.is_in_party_with(pa, pb),
            _ => false,
        }
    }

    /// Java `Skill.addCharacter(caster, target, radius, isDead)` from the caster.
    fn add_character(&self, target: ObjectId, radius: i32, is_dead: bool) -> bool {
        self.w.skill_add_character(self.caster, target, radius, is_dead)
    }

    /// Java `Skill.addSummon(caster, owner, radius, isDead)`: the owner's summon, when it fits.
    fn add_summon(&self, owner: ObjectId, radius: i32, is_dead: bool) -> Option<ObjectId> {
        let summon = self.w.summon_of(owner)?;
        self.add_character(summon, radius, is_dead).then_some(summon)
    }

    fn attackable_or_playable(&self, id: ObjectId) -> bool {
        self.attackable(id) || self.playable(id)
    }

    fn dead(&self, id: ObjectId) -> bool {
        self.w.is_dead_creature(id)
    }

    /// `isAlikeDead()`: dead (fake death is an effect, M5).
    fn alike_dead(&self, id: ObjectId) -> bool {
        self.dead(id)
    }

    fn caster_is_player(&self) -> bool {
        self.w.is_player(self.caster)
    }

    /// `clan.getMembers()` of the caster's clan whose `getPlayer()` is set, in the clan's map
    /// order (M6-CLAN-01).
    fn online_clan_members(&self) -> Vec<ObjectId> {
        // `creature.asPlayer().getClan()`: a summon's owner's clan.
        let player = self.w.acting_player(self.caster).unwrap_or(self.caster);
        let clan_id = self.w.clan_id_of(player);
        self.w.clan_online_members(clan_id, 0)
    }

    /// The first test of `Area`, `BehindArea` and `FrontArea`.
    fn bad_area_target(&self) -> bool {
        let Some(target) = self.target else {
            return true;
        };
        ((target == self.caster || self.alike_dead(target)) && self.skill.cast_range >= 0)
            || !self.attackable_or_playable(target)
    }

    // ------------------------------------------------------------------ area / aura

    // Java: Area.java
    fn area(&mut self) {
        if self.bad_area_target() {
            self.out.incorrect_target();
            return;
        }
        let Some(target) = self.target else { return };
        let src_in_arena = self.src_in_arena();
        let origin;
        if self.skill.cast_range >= 0 {
            if !self.check_area(target, src_in_arena) {
                return;
            }
            if self.only_first {
                self.push(target);
                return;
            }
            origin = target;
            self.push(origin);
        } else {
            origin = self.caster;
        }

        let max_targets = self.affect_limit();
        for obj in self.w.visible_creatures(self.caster) {
            if !self.attackable_or_playable(obj) || obj == origin {
                continue;
            }
            if self.in_range(self.skill.affect_range, origin, obj) {
                if !self.check_area(obj, src_in_arena) {
                    continue;
                }
                if self.limit_reached(max_targets) {
                    continue;
                }
                self.push(obj);
            }
        }
    }

    // Java: AreaCorpseMob.java
    fn area_corpse_mob(&mut self) {
        let Some(target) = self.target.filter(|t| self.attackable(*t) && self.dead(*t)) else {
            self.out.incorrect_target();
            return;
        };
        self.push(target);
        if self.only_first {
            return;
        }
        let src_in_arena = self.src_in_arena();
        for obj in self.w.visible_creatures(self.caster) {
            if !self.attackable_or_playable(obj) || !self.in_range(self.skill.affect_range, target, obj) {
                continue;
            }
            if !self.check_area(obj, src_in_arena) {
                continue;
            }
            self.push(obj);
        }
    }

    // Java: AreaFriendly.java
    fn area_friendly(&mut self) {
        // NOTE(java-quirk): `creature.asPlayer()` is null for an NPC caster and `checkTarget`
        // (`canSeeTarget(null, ...)`) or `player.sendPacket` throws: the catch in
        // `Skill.getTargetList` ends it.
        if !self.caster_is_player() {
            self.not_handled();
            return;
        }
        let player = self.caster;
        if !self.target.is_some_and(|t| self.friendly_area_target(player, t)) && self.skill.cast_range >= 0 {
            self.out.incorrect_target();
            return;
        }
        if self.only_first {
            // NOTE(java-quirk): a null target is added to the list as is; here it is left out.
            if let Some(target) = self.target {
                self.push(target);
            }
            return;
        }
        // M6: `player.isInOlympiadMode()` returns just the player.
        if let Some(target) = self.target {
            self.push(target);
            let max_targets = self.affect_limit();
            for obj in self.w.visible_creatures_in_range(target, self.skill.affect_range) {
                if !self.friendly_area_target(player, obj) || obj == self.caster {
                    continue;
                }
                if self.limit_reached(max_targets) {
                    continue;
                }
                self.push(obj);
            }
        }
    }

    /// `AreaFriendly.checkTarget(player, target)`.
    fn friendly_area_target(&self, player: ObjectId, target: ObjectId) -> bool {
        // `target.isDoor()`, `SiegeFlag` (M6) and `target.isNpc()`.
        if self.alike_dead(target) || self.w.is_door(target) || self.w.is_npc(target) {
            return false;
        }
        if !self.w.can_see_target(player, target) {
            return false;
        }
        if self.playable(target) {
            if player == target {
                return true;
            }
            // M6: observer mode, olympiad.
            if self.w.is_in_duel_with(player, target) || self.w.is_at_war_with(player, target) {
                return false;
            }
            if self.w.is_in_party_with(player, target) {
                return true;
            }
            if self.w.is_inside_zone(target, ZoneId::Pvp) {
                return false;
            }
            // Same clan / ally (true); M6: a command channel member would be true as well.
            if self.w.is_in_clan_with(player, target) || self.w.is_in_ally_with(player, target) {
                return true;
            }
            if self.w.pvp_flag_of(target) > 0 || self.w.karma_of(target) > 0 {
                return false;
            }
        }
        true
    }

    // Java: AreaSummon.java
    fn area_summon(&mut self) {
        // `creature.isPlayer() ? creature.asPlayer().getSummon() : null`: a live servitor.
        let summon = if self.caster_is_player() {
            self.w.summon_of(self.caster)
        } else {
            None
        };
        let Some(summon) = summon.filter(|s| self.w.is_servitor(*s) && !self.dead(*s)) else {
            return;
        };
        if self.only_first {
            self.push(summon);
            return;
        }
        let src_in_arena = self.src_in_arena();
        let max_targets = self.affect_limit();
        for obj in self.w.visible_creatures_in_range(summon, self.skill.affect_range) {
            if obj == self.caster {
                continue;
            }
            if !self.attackable_or_playable(obj) {
                continue;
            }
            if !self.check_area(obj, src_in_arena) {
                continue;
            }
            if self.limit_reached(max_targets) {
                continue;
            }
            self.push(obj);
        }
    }

    // Java: Aura.java
    fn aura(&mut self) {
        let src_in_arena = self.src_in_arena();
        for obj in self
            .w
            .visible_creatures_in_range(self.caster, self.skill.affect_range)
        {
            if !(self.w.is_door(obj) || self.attackable(obj) || self.playable(obj)) {
                continue;
            }
            // Stealth door targeting.
            if self.w.is_door(obj) {
                let stealth = self.w.doors.get(&obj).is_some_and(|d| d.template.stealth);
                if !stealth {
                    continue;
                }
            }
            if !self.check_area(obj, src_in_arena) {
                continue;
            }
            if self.playable(self.caster) && self.attackable(obj) && !has_negative_effect(self.skill) {
                continue;
            }
            self.push(obj);
            if self.only_first {
                return;
            }
        }
    }

    // Java: AuraCorpseMob.java
    fn aura_corpse_mob(&mut self) {
        let max_targets = self.affect_limit();
        for obj in self
            .w
            .visible_creatures_in_range(self.caster, self.skill.affect_range)
        {
            if !self.attackable(obj) {
                continue;
            }
            if self.dead(obj) {
                self.push(obj);
                if self.only_first {
                    return;
                }
                if self.limit_reached(max_targets) {
                    break;
                }
            }
        }
    }

    // Java: AuraFriendly.java
    fn aura_friendly(&mut self) {
        let max_targets = self.affect_limit();
        // `World.forEachVisibleObject(player, ...)` returns at once for a null player (NPC caster).
        if !self.caster_is_player() {
            return;
        }
        let player = self.caster;
        // NOTE(java-quirk): no range test: everything in the surrounding regions is a candidate.
        for obj in self.w.visible_creatures(player) {
            if obj == self.caster || !self.friendly_aura_target(player, obj) {
                continue;
            }
            if self.limit_reached(max_targets) {
                continue;
            }
            self.push(obj);
        }
    }

    /// `AuraFriendly.checkTarget(player, target)`.
    fn friendly_aura_target(&self, player: ObjectId, target: ObjectId) -> bool {
        if !self.w.can_see_target(player, target) {
            return false;
        }
        // `isDoor()`, `SiegeFlag` (M6), `isMonster()`.
        if self.alike_dead(target) || self.w.is_door(target) || self.w.is_monster(target) {
            return false;
        }
        if self.playable(target) {
            if self.w.is_in_duel_with(player, target) {
                return false;
            }
            if self.w.is_in_party_with(player, target) {
                return true;
            }
            if self.w.is_inside_zone(target, ZoneId::Pvp) {
                return false;
            }
            // Same clan / ally (true); M6: a command channel member would be true as well.
            if self.w.is_in_clan_with(player, target) || self.w.is_in_ally_with(player, target) {
                return true;
            }
            if self.w.pvp_flag_of(target) > 0 || self.w.karma_of(target) > 0 {
                return false;
            }
        }
        true
    }

    // Java: BehindArea.java
    fn behind_area(&mut self) {
        self.directional_area(false);
    }

    // Java: FrontArea.java
    fn front_area(&mut self) {
        self.directional_area(true);
    }

    /// `BehindArea` and `FrontArea` are the same handler but for the position test and the
    /// heading `FrontArea` sets on the caster.
    fn directional_area(&mut self, front: bool) {
        if self.bad_area_target() {
            self.out.incorrect_target();
            return;
        }
        let Some(target) = self.target else { return };
        let src_in_arena = self.src_in_arena();
        let origin;
        if self.skill.cast_range >= 0 {
            if !self.check_area(target, src_in_arena) {
                return;
            }
            origin = target;
            self.push(origin);
            if self.only_first {
                return;
            }
        } else {
            origin = self.caster;
        }

        let mut caster_heading = self.w.heading_of(self.caster);
        if front && self.caster != origin {
            // Update heading towards the origin. `isInFrontOf(creature)` below uses it.
            if let (Some(from), Some(to)) = (self.w.position_of(self.caster), self.w.position_of(origin)) {
                caster_heading = heading_from_points(from.0, from.1, to.0, to.1);
                self.out.caster_heading = Some(caster_heading);
            }
        }

        let wanted = if front { Position::Front } else { Position::Back };
        let max_targets = self.affect_limit();
        for obj in self.w.visible_creatures(self.caster) {
            if !self.attackable_or_playable(obj) {
                continue;
            }
            if obj == origin {
                continue;
            }
            if self.in_range(self.skill.affect_range, origin, obj) {
                if self.w.position_of_towards(obj, self.caster, caster_heading) != Some(wanted) {
                    continue;
                }
                if !self.check_area(obj, src_in_arena) {
                    continue;
                }
                if self.limit_reached(max_targets) {
                    continue;
                }
                self.push(obj);
            }
        }
    }

    // Java: BehindAura.java
    fn behind_aura(&mut self) {
        self.directional_aura(false);
    }

    // Java: FrontAura.java
    fn front_aura(&mut self) {
        self.directional_aura(true);
    }

    fn directional_aura(&mut self, front: bool) {
        let src_in_arena = self.src_in_arena();
        let max_targets = self.affect_limit();
        let wanted = if front { Position::Front } else { Position::Back };
        let heading = self.w.heading_of(self.caster);
        for obj in self
            .w
            .visible_creatures_in_range(self.caster, self.skill.affect_range)
        {
            if !self.attackable_or_playable(obj) {
                continue;
            }
            if self.w.position_of_towards(obj, self.caster, heading) != Some(wanted) {
                continue;
            }
            if !self.check_area(obj, src_in_arena) {
                continue;
            }
            self.push(obj);
            if self.only_first {
                return;
            }
            if self.limit_reached(max_targets) {
                break;
            }
        }
    }

    // ------------------------------------------------------------------ clan

    /// The duel test of `Clan`, `CorpseClan` and `PartyClan`: a duelist skips whoever is in
    /// another duel, or in a party with another leader when both are in a party.
    fn duel_excludes(&self, player: ObjectId, obj: ObjectId) -> bool {
        if !self.w.is_in_duel(player) {
            return false;
        }
        if self.w.duel_id_of(player) != self.w.duel_id_of(obj) {
            return true;
        }
        match (self.w.party_of(player), self.w.party_of(obj)) {
            (Some(a), Some(b)) => self.w.party_leader(a) != self.w.party_leader(b),
            _ => false,
        }
    }

    // Java: Clan.java
    fn clan(&mut self) {
        if self.playable(self.caster) {
            // `creature.asPlayer()`: a summon casts for its owner.
            let Some(player) = self.w.acting_player(self.caster) else {
                return;
            };
            // M6: `isInOlympiadMode()` returns just the player.
            self.push(player);
            if self.only_first {
                return;
            }
            let radius = self.skill.affect_range;
            if let Some(summon) = self.add_summon(player, radius, false) {
                self.push(summon);
            }
            // The online members of its clan within the affect range (M6-CLAN-01).
            for obj in self.online_clan_members() {
                if obj == player {
                    continue;
                }
                if self.duel_excludes(player, obj) {
                    continue;
                }
                // Do not add this target if this is a Pc->Pc pvp casting and pvp condition not met
                if !self.w.check_pvp_skill(player, obj, self.skill, self.ctx) {
                    continue;
                }
                // Events (`isOnEvent()` teams): not modelled.
                // The member's summon (`Skill.addSummon(creature, obj, radius, false)`).
                if let Some(summon) = self.add_summon(obj, radius, false) {
                    self.push(summon);
                }
                if !self.w.skill_add_character(self.caster, obj, radius, false) {
                    continue;
                }
                self.push(obj);
            }
        } else if self.w.is_npc(self.caster) {
            // For buff purposes, returns friendly mobs nearby and mob itself.
            self.push(self.caster);
            if self
                .w
                .npc_template_for_skill(self.caster)
                .is_none_or(|t| t.clans.is_empty())
            {
                return;
            }
            for other in self
                .w
                .visible_creatures_in_range(self.caster, self.skill.cast_range)
            {
                if self.w.is_npc(other) && self.w.npc_in_my_clan(self.caster, other) {
                    let max_targets = self.affect_limit();
                    if self.limit_reached(max_targets) {
                        break;
                    }
                    self.push(other);
                }
            }
        }
    }

    // Java: ClanMember.java
    fn clan_member(&mut self) {
        if !self.w.is_npc(self.caster) {
            return;
        }
        // For buff purposes, returns friendly mobs nearby and mob itself.
        if self
            .w
            .npc_template_for_skill(self.caster)
            .is_none_or(|t| t.clans.is_empty())
        {
            self.push(self.caster);
            return;
        }
        // `forFirstVisibleObjectInRange`: the first match in region order, not the nearest.
        let first = self
            .w
            .visible_creatures_in_range(self.caster, self.skill.cast_range)
            .into_iter()
            .find(|o| self.w.is_npc(*o) && self.w.npc_in_my_clan(self.caster, *o));
        match first {
            Some(other) => self.push(other),
            None => self.push(self.caster),
        }
    }

    // Java: CommandChannel.java
    fn command_channel(&mut self) {
        // `creature.asPlayer()`.
        let Some(player) = self.w.acting_player(self.caster) else {
            return;
        };
        self.push(player);
        if let Some(summon) = self.add_summon(player, self.skill.affect_range, false) {
            self.push(summon);
        }
        // `party = player.getParty()` (a summon caster's is its owner's).
        let Some(party) = self.w.party_of(player) else {
            // If the player is not in a party, it gets only itself (and its summon).
            return;
        };
        let radius = self.skill.affect_range;
        let max_targets = self.affect_limit();
        // M6: the members of the command channel, when the party is in one (there are none).
        for member in self.w.party_members(party) {
            if member == self.caster {
                continue;
            }
            if self.w.skill_add_character(self.caster, member, radius, false) {
                self.push(member);
                // NOTE(java-quirk): no `maxTargets > 0` test, so an affect limit of 0 stops the
                // list after the first member.
                if self.size_at_least(max_targets) {
                    break;
                }
            }
        }
    }

    // Java: CorpseClan.java
    fn corpse_clan(&mut self) {
        if self.caster_is_player() {
            // M6: `isInOlympiadMode()` returns just the player.
            // Dead clan members in range, not the caster (M6-CLAN-01). Without a clan the list
            // stays empty (the caster is not added).
            if self.w.clan_of(self.caster).is_none() {
                return;
            }
            let radius = self.skill.affect_range;
            let max_targets = self.affect_limit();
            for obj in self.online_clan_members() {
                if obj == self.caster {
                    continue;
                }
                if self.duel_excludes(self.caster, obj) {
                    continue;
                }
                // Do not add this target if this is a Pc->Pc pvp casting and pvp condition not met
                if !self.w.check_pvp_skill(self.caster, obj, self.skill, self.ctx) {
                    continue;
                }
                // Events (`isOnEvent()` teams): not modelled.
                if !self.w.skill_add_character(self.caster, obj, radius, true) {
                    continue;
                }
                // Check target is not in a active siege zone (M7-SIEGE-01).
                if self.w.is_inside_zone(obj, ZoneId::Siege) && !self.w.is_in_siege(obj) {
                    continue;
                }
                self.push(obj);
                if self.only_first {
                    return;
                }
                if self.limit_reached(max_targets) {
                    break;
                }
            }
        } else if self.w.is_npc(self.caster) {
            // For buff purposes, returns friendly mobs nearby and mob itself.
            self.push(self.caster);
            if self
                .w
                .npc_template_for_skill(self.caster)
                .is_none_or(|t| t.clans.is_empty())
            {
                return;
            }
            for other in self
                .w
                .visible_creatures_in_range(self.caster, self.skill.cast_range)
            {
                if self.w.is_npc(other) && self.w.npc_in_my_clan(self.caster, other) {
                    // NOTE(java-quirk): unlike `Clan`, no `maxTargets > 0` test: an affect limit
                    // of 0 stops after the caster.
                    let max_targets = self.affect_limit();
                    if self.size_at_least(max_targets) {
                        break;
                    }
                    self.push(other);
                }
            }
        }
    }

    // ------------------------------------------------------------------ single targets

    // Java: CorpseMob.java
    fn corpse_mob(&mut self) {
        let Some(target) = self.target.filter(|t| self.attackable(*t) && self.dead(*t)) else {
            self.out.incorrect_target();
            return;
        };
        // `hasEffectType(SUMMON) && target.isServitor()`: no effect class has the SUMMON type.
        // `hasEffectType(HP_DRAIN) && attackable.isOldCorpse(player, CorpseConsumeSkillAllowed
        // TimeBeforeDecay, true)`: a corpse about to decay cannot be drained. The list is empty
        // and a player caster is told (`attacker != null` in `isOldCorpse`).
        if has_effect(self.skill, EffectKind::HpDrain) && self.w.is_old_corpse(target) {
            if self.caster_is_player() {
                self.out.messages.push(TargetMessage::System(
                    SystemMessageId::TheCorpseIsTooOldTheSkillCannotBeUsed,
                ));
            }
            return;
        }
        self.push(target);
    }

    // Java: EnemySummon.java
    fn enemy_summon(&mut self) {
        let Some(target) = self.target.filter(|t| self.w.is_summon(*t)) else {
            return;
        };
        if !self.caster_is_player() {
            return;
        }
        if self.w.summon_of(self.caster) == Some(target) || self.dead(target) {
            return;
        }
        let Some(owner) = self.w.summon_owner(target) else {
            return;
        };
        if self.w.pvp_flag_of(owner) != 0 || self.w.karma_of(owner) > 0 {
            self.push(target);
            return;
        }
        if self.w.is_inside_zone(owner, ZoneId::Pvp) && self.w.is_inside_zone(self.caster, ZoneId::Pvp) {
            self.push(target);
        }
        // M6 (DUEL-01): both in the same duel.
    }

    // Java: FlagPole.java
    fn flag_pole(&mut self) {
        if !self.caster_is_player() {
            return;
        }
        // NOTE(java-quirk): `singletonList(target)` even when target is null; here it is left out.
        if let Some(target) = self.target {
            self.push(target);
        }
    }

    // Java: Ground.java
    fn ground(&mut self) {
        let max_targets = self.affect_limit();
        let src_in_arena = self.src_in_arena();
        let candidates = self.w.visible_creatures(self.caster);
        // NOTE(java-quirk): `player.getCurrentSkillWorldPosition()` is dereferenced for every
        // candidate; an NPC caster (no player) or a missing ground point throws on the first one.
        if !candidates.is_empty() && (!self.caster_is_player() || self.ctx.ground.is_none()) {
            self.not_handled();
            return;
        }
        if let Some(ground) = self.ctx.ground {
            for character in candidates {
                let inside = self.w.position_of(character).is_some_and(|p| {
                    // `isInsideRadius2D`: `calculateDistance2D(...) < radius`.
                    let dx = f64::from(p.0) - f64::from(ground.0);
                    let dy = f64::from(p.1) - f64::from(ground.1);
                    (dx * dx + dy * dy).sqrt() < f64::from(self.skill.affect_range)
                });
                if !inside {
                    continue;
                }
                if !self.check_area(character, src_in_arena) {
                    continue;
                }
                if self.w.is_door(character) {
                    continue;
                }
                self.push(character);
                // NOTE(java-quirk): the limit test sits after the add and only `return`s from the
                // lambda, so it never limits anything.
                let _ = self.limit_reached(max_targets);
            }
        }
        if self.out.targets.is_empty() && has_effect(self.skill, EffectKind::SummonNpc) {
            self.push(self.caster);
        }
    }

    // Java: Holy.java
    fn holy(&mut self) {
        // `target.isArtefact()`: the castle artefact NPCs.
        let artefact = self.target.filter(|t| {
            self.w
                .npc_template_for_skill(*t)
                .is_some_and(|n| n.is_type("Artefact"))
        });
        if let Some(target) = artefact {
            self.push(target);
        }
    }

    // Java: One.java
    fn one(&mut self) {
        // Check for null target or any other invalid target.
        let Some(target) = self
            .target
            .filter(|t| !self.dead(*t) && !(*t == self.caster && has_negative_effect(self.skill)))
        else {
            self.out.incorrect_target();
            return;
        };
        self.push(target);
    }

    // Java: OwnerPet.java
    fn owner_pet(&mut self) {
        // When the caster is a summon, its live owner.
        if let Some(owner) = self.w.summon_owner(self.caster)
            && self.w.players.contains_key(&owner)
            && !self.dead(owner)
        {
            self.push(owner);
        }
    }

    // Java: Party.java
    fn party(&mut self) {
        self.push(self.caster);
        if self.only_first {
            return;
        }
        let radius = self.skill.affect_range;
        // `creature.asPlayer()`: a summon's owner.
        let player = self.w.acting_player(self.caster);
        if let Some(owner) = self.w.summon_owner(self.caster) {
            // A summon caster adds its owner.
            if self.w.players.contains_key(&owner) && self.add_character(owner, radius, false) {
                self.push(owner);
            }
        } else if self.caster_is_player()
            && let Some(summon) = self.add_summon(self.caster, radius, false)
        {
            // A player caster adds its summon.
            self.push(summon);
        }
        // Get a list of party members (`creature.getParty()`: a summon's is its owner's).
        if let Some(party) = player.and_then(|p| self.w.party_of(p)) {
            for member in self.w.party_members(party) {
                if Some(member) == player {
                    continue;
                }
                if self.w.skill_add_character(self.caster, member, radius, false) {
                    self.push(member);
                }
                if let Some(summon) = self.add_summon(member, radius, false) {
                    self.push(summon);
                }
            }
        }
    }

    // Java: PartyClan.java
    fn party_clan(&mut self) {
        if !self.caster_is_player() {
            return;
        }
        self.push(self.caster);
        if self.only_first {
            return;
        }
        let radius = self.skill.affect_range;
        let clan_id = self.w.clan_id_of(self.caster);
        let has_clan = self.w.clan(clan_id).is_some();
        let party = self.w.party_of(self.caster);
        let has_party = party.is_some();
        if let Some(summon) = self.add_summon(self.caster, radius, false) {
            self.push(summon);
        }
        // If the player has neither a clan nor a party Java returns here.
        if !(has_clan || has_party) {
            return;
        }
        let max_targets = self.affect_limit();
        // `World.getVisibleObjectsInRange(creature, Player.class, radius)`.
        for obj in self.w.visible_creatures_in_range(self.caster, radius) {
            if !self.w.is_player(obj) {
                continue;
            }
            // M8: olympiad side checks.
            // A duelist only gets the members of its own duel and party.
            if self.duel_excludes(self.caster, obj) {
                continue;
            }
            // Same clan, or same party.
            let same_clan = has_clan && self.w.clan_id_of(obj) == clan_id;
            let same_party = has_party && self.w.party_of(obj) == party;
            if !(same_clan || same_party) {
                continue;
            }
            // Don't add this target if this is a Pc->Pc pvp casting and pvp condition not met.
            if !self.w.check_pvp_skill(self.caster, obj, self.skill, self.ctx) {
                continue;
            }
            // M6: event teams.
            if !self.only_first
                && let Some(summon) = self.add_summon(obj, radius, false)
            {
                self.push(summon);
            }
            if !self.w.skill_add_character(self.caster, obj, radius, false) {
                continue;
            }
            self.push(obj);
            if self.limit_reached(max_targets) {
                break;
            }
        }
    }

    // Java: PartyMember.java
    fn party_member(&mut self) {
        let Some(target) = self.target else {
            self.out.incorrect_target();
            return;
        };
        // Check if target is a valid party member.
        if !self.dead(target) {
            let own_summon = self.caster_is_player() && self.w.summon_of(self.caster) == Some(target);
            let own_owner = self.w.summon_owner(self.caster) == Some(target) && self.w.is_player(target);
            if target == self.caster || self.same_party(self.caster, target) || own_summon || own_owner {
                self.push(target);
                return;
            }
        }
        if self.caster_is_player() && self.w.is_player(target) {
            self.out.messages.push(TargetMessage::UnsuitableTerms);
        }
    }

    // Java: PartyNotMe.java
    fn party_not_me(&mut self) {
        let Some(party) = self.w.party_of(self.caster) else {
            return;
        };
        let party_range = self.w.settings.party.alt_party_range;
        let affect_range = self.skill.affect_range;
        for member in self.w.party_members(party) {
            if !self.dead(member)
                && member != self.caster
                && self.in_range(party_range, self.caster, member)
                && (affect_range <= 0 || self.in_range(affect_range, self.caster, member))
            {
                self.push(member);
                // The member's living summon.
                if let Some(summon) = self.w.summon_of(member).filter(|s| !self.dead(*s)) {
                    self.push(summon);
                }
            }
        }
    }

    // Java: PartyOther.java
    fn party_other(&mut self) {
        if let Some(target) = self
            .target
            .filter(|t| *t != self.caster && self.w.is_in_party_with(self.caster, *t))
        {
            if self.dead(target) {
                return;
            }
            if let Some(class_id) = self.w.players.get(&target).map(|p| p.data.class_id) {
                let mage = u16::try_from(class_id).is_ok_and(|id| l2_data::ClassId(id).is_mage());
                // FORCE BUFFS may cancel here but there should be a proper condition.
                match self.skill.id {
                    426 => {
                        if !mage {
                            self.push(target);
                        }
                        return;
                    }
                    427 => {
                        if mage {
                            self.push(target);
                        }
                        return;
                    }
                    _ => {}
                }
            }
            self.push(target);
            return;
        }
        self.out.incorrect_target();
    }

    // Java: PcBody.java
    fn pc_body(&mut self) {
        if let Some(target) = self.target.filter(|t| self.dead(*t)) {
            // A dead pet (`targetPet`) is a valid body too.
            let target_player = self.w.is_player(target);
            if self.caster_is_player() && (target_player || self.w.is_pet(target)) {
                let mut cond_good = true;
                if has_effect(self.skill, EffectKind::Resurrection) && target_player {
                    // Check target is not in an active siege zone (M7-SIEGE-01).
                    if self.w.is_inside_zone(target, ZoneId::Siege) && !self.w.is_in_siege(target) {
                        cond_good = false;
                        self.out.messages.push(TargetMessage::System(
                            SystemMessageId::ItIsNotPossibleToResurrectInBattlefieldsWhereASiegeWarIsTakingPlace,
                        ));
                    }
                    // M6: `isFestivalParticipant()` -> sendMessage("You may not resurrect
                    // participants in a festival.") and no target.
                }
                if cond_good {
                    self.push(target);
                    return;
                }
            }
        }
        self.out.incorrect_target();
    }

    // Java: Pet.java
    fn pet(&mut self) {
        if self.caster_is_player()
            && let Some(summon) = self.w.summon_of(self.caster).filter(|s| self.w.is_pet(*s))
        {
            self.push(summon);
        }
    }

    // Java: Self.java
    fn self_target(&mut self) {
        self.push(self.caster);
    }

    // Java: Servitor.java
    fn servitor(&mut self) {
        if self.caster_is_player()
            && let Some(summon) = self.w.summon_of(self.caster).filter(|s| self.w.is_servitor(*s))
        {
            self.push(summon);
        }
    }

    // Java: Summon.java
    fn summon(&mut self) {
        if self.caster_is_player()
            && let Some(summon) = self.w.summon_of(self.caster)
        {
            self.push(summon);
        }
    }

    // Java: TargetParty.java
    fn target_party(&mut self) {
        // Check for null target or any other invalid target.
        let Some(target) = self.target.filter(|t| !self.dead(*t) && *t != self.caster) else {
            self.out.incorrect_target();
            return;
        };
        // NOTE(java-quirk): Java reads `creature.getTarget().asPlayer()` (the selected object,
        // not the `target` argument); a non-player there is a NullPointerException.
        let selected = if self.caster_is_player() {
            self.w.target_of(self.caster)
        } else {
            // NPCs keep no target in the world yet: the argument stands in for it.
            Some(target)
        };
        if !selected.is_some_and(|s| self.w.is_player(s)) {
            self.not_handled();
            return;
        }
        let selected = selected.unwrap_or(target);
        let radius = self.skill.affect_range;
        if let Some(party) = self.w.party_of(selected) {
            // Every member within the affect range of the selected player, and its summon.
            for member in self.w.party_members(party) {
                if self.w.skill_add_character(selected, member, radius, false) {
                    self.push(member);
                }
                if let Some(summon) = self.w.summon_of(member)
                    && self.w.skill_add_character(selected, summon, radius, false)
                {
                    self.push(summon);
                }
            }
        } else {
            self.push(target);
        }
    }

    // Java: Unlockable.java
    fn unlockable(&mut self) {
        let valid = self.target.filter(|t| {
            self.w.is_door(*t)
                || self
                    .w
                    .npc_template_for_skill(*t)
                    .is_some_and(|n| n.is_type("Chest"))
        });
        match valid {
            Some(target) => self.push(target),
            None => self
                .out
                .messages
                .push(TargetMessage::System(SystemMessageId::InvalidTarget)),
        }
    }
}
