//! The trigger skill effects: a skill is cast on its own when something happens to the owner of
//! the effect. `TriggerSkillByAvoid` (an attack missed the owner), `TriggerSkillByDamageDealt`
//! (the owner hurt somebody), `TriggerSkillByDamageReceived` (the owner was hurt) and
//! `TriggerSkillBySkill` (the owner started a skill).
// Java: dist/game/data/scripts/handlers/skill/effects/TriggerSkillByAvoid.java
// Java: dist/game/data/scripts/handlers/skill/effects/TriggerSkillByDamageDealt.java
// Java: dist/game/data/scripts/handlers/skill/effects/TriggerSkillByDamageReceived.java
// Java: dist/game/data/scripts/handlers/skill/effects/TriggerSkillBySkill.java
//
// The effect registers a listener with the creature when it starts and removes it when it ends
// (`effects/triggers.rs`). The listener carries the parameters below; the checks of the events
// are in the same order as Java's, because the dice (`Rnd.get(100)`) are only rolled once the
// checks before them pass.

use l2_data::item::WeaponType;
use l2_data::skill::{EffectTemplate, TargetType};
use l2_data::stats::InstanceType;
use l2_util::ObjectId;

use super::params;
use crate::effects::SkillKey;
use crate::effects::registry::{EffectCx, EffectHandler};
use crate::effects::triggers::{DamageEvent, TriggerKind, owner_of, warn_no_handler};
use crate::skills::targets::is_target_type_handled;
use crate::world::World;

/// `params.getInt(key)`: required, Java's `StatSet.getInt(key)` throws when it is missing.
fn required_int(template: &EffectTemplate, key: &str) -> Result<i32, String> {
    if template.param(key).is_none() {
        return Err(format!("Integer value required, but not specified: {key}!"));
    }
    params::int(template, key, 0)
}

/// `params.getEnum("targetType", TargetType.class, default)`.
fn target_type(template: &EffectTemplate, default: TargetType) -> Result<TargetType, String> {
    match params::string(template, "targetType")? {
        None => Ok(default),
        Some(name) => TargetType::from_name(&name).ok_or_else(|| {
            format!("No enum constant org.l2jmobius.gameserver.mechanics.skill.targets.TargetType.{name}")
        }),
    }
}

/// `params.getEnum("attackerType", InstanceType.class, InstanceType.Creature)`.
fn attacker_type(template: &EffectTemplate) -> Result<InstanceType, String> {
    match params::string(template, "attackerType")? {
        None => Ok(InstanceType::Creature),
        Some(name) => InstanceType::from_name(&name).ok_or_else(|| {
            format!(
                "No enum constant org.l2jmobius.gameserver.entity.actor.enums.creature.InstanceType.{name}"
            )
        }),
    }
}

/// Java `InstanceType._parent`.
fn parent_type(t: InstanceType) -> Option<InstanceType> {
    use InstanceType as T;
    Some(match t {
        T::WorldObject => return None,
        T::Item | T::Creature => T::WorldObject,
        T::Npc | T::Playable | T::StaticObject | T::Door | T::Decoy | T::Vehicle => T::Creature,
        T::Summon | T::Player => T::Playable,
        T::Folk
        | T::TerrainObject
        | T::EffectPoint
        | T::Trap
        | T::Attackable
        | T::FlyTerrainObject
        | T::FestivalGuide
        | T::ControlTower
        | T::FlameTower
        | T::SiegeFlag
        | T::SignsPriest
        | T::DungeonGatekeeper
        | T::Auctioneer
        | T::BroadcastingTower
        | T::OlympiadManager
        | T::RaceManager
        | T::Teleporter
        | T::SchemeBuffer
        | T::EventMob => T::Npc,
        T::Merchant
        | T::Warehouse
        | T::FlyNpc
        | T::Artefact
        | T::Adventurer
        | T::Trainer
        | T::VillageMaster
        | T::Doorman
        | T::ClassMaster
        | T::UCManagerInstance => T::Folk,
        T::Servitor | T::Pet => T::Summon,
        T::BabyPet => T::Pet,
        T::Guard | T::Monster | T::FriendlyMob | T::Defender => T::Attackable,
        T::QuestGuard => T::Guard,
        T::Chest | T::FeedableBeast | T::RiftInvader | T::RaidBoss | T::FlyMonster | T::FestivalMonster => {
            T::Monster
        }
        T::TamedBeast => T::FeedableBeast,
        T::GrandBoss | T::FlyRaidBoss => T::RaidBoss,
        T::Boat | T::AirShip => T::Vehicle,
        T::ControllableAirShip => T::AirShip,
        T::FortCommander => T::Defender,
        T::FortLogistics | T::FortManager | T::ClanHallManager | T::Fisherman | T::PetManager => T::Merchant,
        T::DawnPriest | T::DuskPriest => T::SignsPriest,
        T::CastleDoorman | T::FortDoorman | T::ClanHallDoorman => T::Doorman,
    })
}

/// Java `getInstanceType().isType(wanted)`: the creature's class is `wanted` or inherits from it.
fn is_instance_type(world: &World, id: ObjectId, wanted: InstanceType) -> bool {
    let mut current = if world.players.contains_key(&id) {
        Some(InstanceType::Player)
    } else if let Some(npc_type) = world
        .stats(id)
        .and_then(|v| v.npc_template().map(|t| t.npc_type.clone()))
    {
        // The class Java instantiated for the template type; an unknown name is a plain `Npc`.
        Some(InstanceType::from_name(&npc_type).unwrap_or(InstanceType::Npc))
    } else if world.objects.contains_key(&id) {
        Some(InstanceType::Creature)
    } else {
        None
    };
    while let Some(t) = current {
        if t == wanted {
            return true;
        }
        current = parent_type(t);
    }
    false
}

/// `(skillId, skillLevel)` of a `SkillHolder`.
fn holder(id: i32, level: i32) -> SkillKey {
    SkillKey::new(id, level)
}

// ---------------------------------------------------------------------------- ByAvoid

/// Java `TriggerSkillByAvoid`: its parameters, and the listener it registers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ByAvoid {
    chance: i32,
    skill: SkillKey,
    target_type: TargetType,
}

impl ByAvoid {
    /// Java `TriggerSkillByAvoid(attachCond, applyCond, set, params)`.
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self {
            chance: params::int(template, "chance", 100)?,
            skill: holder(
                params::int(template, "skillId", 0)?,
                params::int(template, "skillLevel", 0)?,
            ),
            target_type: target_type(template, TargetType::One)?,
        }))
    }

    /// Java `onAvoidEvent(event)`: `target` is the owner (it avoided the attack of `attacker`).
    pub(crate) fn on_avoid(&self, world: &mut World, attacker: ObjectId, target: ObjectId, is_dot: bool) {
        if is_dot || self.chance == 0 || self.skill.id == 0 || self.skill.level == 0 {
            return;
        }
        if !is_target_type_handled(self.target_type) {
            warn_no_handler(self.target_type);
            return;
        }
        // NOTE(java-quirk): `Rnd.get(100) > _chance`, so a chance of 50 passes 51 times in 100.
        if world.rng().below(100) > self.chance {
            return;
        }
        world.trigger_skill_on_targets(self.skill, self.target_type, target, Some(attacker));
    }
}

impl EffectHandler for ByAvoid {
    fn on_exit(&self, world: &mut World, cx: &EffectCx<'_>) {
        world.remove_trigger_listeners(cx.effected, owner_of(cx), |k| matches!(k, TriggerKind::Avoid(_)));
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        world.add_trigger_listener(cx.effected, owner_of(cx), TriggerKind::Avoid(*self));
    }
}

// ---------------------------------------------------------------------------- ByDamageDealt

/// Java `TriggerSkillByDamageDealt`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ByDamageDealt {
    min_attacker_level: i32,
    max_attacker_level: i32,
    min_damage: i32,
    chance: i32,
    skill: SkillKey,
    target_type: TargetType,
    attacker_type: InstanceType,
    /// `_allowWeapons`: the `WeaponType` masks, 0 for "ALL".
    allow_weapons: i32,
    is_critical: bool,
    allow_normal_attack: bool,
    allow_skill_attack: bool,
}

impl ByDamageDealt {
    /// Java `TriggerSkillByDamageDealt(attachCond, applyCond, set, params)`.
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        let mut allow_weapons = 0;
        let weapons = params::string(template, "allowWeapons")?.unwrap_or_else(|| "ALL".to_owned());
        if !weapons.eq_ignore_ascii_case("ALL") {
            for name in weapons.split(',') {
                // `WeaponType.valueOf(s)`: exact names, throws on an unknown one.
                let weapon = WeaponType::from_name(name).ok_or_else(|| {
                    format!("No enum constant org.l2jmobius.gameserver.entity.item.type.WeaponType.{name}")
                })?;
                allow_weapons |= weapon.mask();
            }
        }
        Ok(Box::new(Self {
            min_attacker_level: params::int(template, "minAttackerLevel", 1)?,
            max_attacker_level: params::int(template, "maxAttackerLevel", 127)?,
            min_damage: params::int(template, "minDamage", 1)?,
            chance: params::int(template, "chance", 100)?,
            skill: holder(
                required_int(template, "skillId")?,
                params::int(template, "skillLevel", 1)?,
            ),
            target_type: target_type(template, TargetType::SelfTarget)?,
            attacker_type: attacker_type(template)?,
            is_critical: params::boolean(template, "isCritical", false)?,
            allow_normal_attack: params::boolean(template, "allowNormalAttack", true)?,
            allow_skill_attack: params::boolean(template, "allowSkillAttack", false)?,
            allow_weapons,
        }))
    }

    /// Java `onAttackEvent(event)`: the owner is `event.attacker`.
    pub(crate) fn on_damage_dealt(&self, world: &mut World, event: &DamageEvent) {
        let Some(attacker) = event.attacker else {
            return;
        };
        if event.damage_over_time
            || self.chance == 0
            || self.skill.id == 0
            || self.skill.level == 0
            || (!self.allow_normal_attack && !self.allow_skill_attack)
        {
            return;
        }
        // Check if there is dependancy on critical.
        if self.is_critical != event.critical {
            return;
        }
        if !is_target_type_handled(self.target_type) {
            warn_no_handler(self.target_type);
            return;
        }
        // When no skill attacks are allowed.
        if !self.allow_skill_attack && event.skill.is_some() {
            return;
        }
        // When no normal attacks are allowed.
        if !self.allow_normal_attack && event.skill.is_none() {
            return;
        }
        if attacker == event.target {
            return;
        }
        let level = world.stats(attacker).map_or(0, |v| v.level());
        if level < self.min_attacker_level || level > self.max_attacker_level {
            return;
        }
        if event.damage < f64::from(self.min_damage)
            || world.rng().below(100) > self.chance
            || !is_instance_type(world, attacker, self.attacker_type)
        {
            return;
        }
        if self.allow_weapons > 0 {
            let weapon_mask = world.stats(attacker).and_then(|v| {
                v.active_weapon_item()
                    .and_then(|t| t.weapon())
                    .map(|w| w.weapon_type.mask())
            });
            if weapon_mask.is_none_or(|mask| mask & self.allow_weapons == 0) {
                return;
            }
        }
        world.trigger_skill_on_targets(self.skill, self.target_type, attacker, Some(event.target));
    }
}

impl EffectHandler for ByDamageDealt {
    fn on_exit(&self, world: &mut World, cx: &EffectCx<'_>) {
        world.remove_trigger_listeners(cx.effected, owner_of(cx), |k| {
            matches!(k, TriggerKind::DamageDealt(_))
        });
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        world.add_trigger_listener(cx.effected, owner_of(cx), TriggerKind::DamageDealt(*self));
    }
}

// ---------------------------------------------------------------------------- ByDamageReceived

/// Java `TriggerSkillByDamageReceived`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ByDamageReceived {
    min_attacker_level: i32,
    max_attacker_level: i32,
    min_damage: i32,
    chance: i32,
    skill: SkillKey,
    target_type: TargetType,
    attacker_type: InstanceType,
}

impl ByDamageReceived {
    /// Java `TriggerSkillByDamageReceived(attachCond, applyCond, set, params)`.
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self {
            min_attacker_level: params::int(template, "minAttackerLevel", 1)?,
            // NOTE: 100 here, 127 in `TriggerSkillByDamageDealt`.
            max_attacker_level: params::int(template, "maxAttackerLevel", 100)?,
            min_damage: params::int(template, "minDamage", 1)?,
            chance: params::int(template, "chance", 100)?,
            skill: holder(
                required_int(template, "skillId")?,
                params::int(template, "skillLevel", 1)?,
            ),
            target_type: target_type(template, TargetType::SelfTarget)?,
            attacker_type: attacker_type(template)?,
        }))
    }

    /// Java `onDamageReceivedEvent(event)`: the owner is `event.target`.
    pub(crate) fn on_damage_received(&self, world: &mut World, event: &DamageEvent) {
        if event.damage_over_time || self.chance == 0 || self.skill.level == 0 {
            return;
        }
        // NOTE(java-quirk): Java dereferences the attacker without a null check below (the
        // listener runs asynchronously, so the exception is swallowed): no attacker, no trigger.
        let Some(attacker) = event.attacker else {
            return;
        };
        if self.target_type == TargetType::One && world.creature_target(attacker) != Some(event.target) {
            return;
        }
        if !is_target_type_handled(self.target_type) {
            warn_no_handler(self.target_type);
            return;
        }
        if attacker == event.target {
            return;
        }
        let level = world.stats(attacker).map_or(0, |v| v.level());
        if level < self.min_attacker_level || level > self.max_attacker_level {
            return;
        }
        if event.damage < f64::from(self.min_damage)
            || world.rng().below(100) > self.chance
            || !is_instance_type(world, attacker, self.attacker_type)
        {
            return;
        }
        world.trigger_skill_on_targets(self.skill, self.target_type, event.target, Some(attacker));
    }
}

impl EffectHandler for ByDamageReceived {
    fn on_exit(&self, world: &mut World, cx: &EffectCx<'_>) {
        world.remove_trigger_listeners(cx.effected, owner_of(cx), |k| {
            matches!(k, TriggerKind::DamageReceived(_))
        });
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        world.add_trigger_listener(cx.effected, owner_of(cx), TriggerKind::DamageReceived(*self));
    }
}

// ---------------------------------------------------------------------------- BySkill

/// Java `TriggerSkillBySkill`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BySkill {
    cast_skill_id: i32,
    chance: i32,
    skill: SkillKey,
    target_type: TargetType,
}

impl BySkill {
    /// Java `TriggerSkillBySkill(attachCond, applyCond, set, params)`.
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self {
            cast_skill_id: params::int(template, "castSkillId", 0)?,
            chance: params::int(template, "chance", 100)?,
            skill: holder(
                params::int(template, "skillId", 0)?,
                params::int(template, "skillLevel", 0)?,
            ),
            target_type: target_type(template, TargetType::One)?,
        }))
    }

    /// Java `onSkillUseEvent(event)`: the owner is the caster.
    pub(crate) fn on_skill_use(&self, world: &mut World, caster: ObjectId, used: SkillKey, target: ObjectId) {
        if self.chance == 0 || self.skill.id == 0 || self.skill.level == 0 || self.cast_skill_id == 0 {
            return;
        }
        if self.cast_skill_id != used.id {
            return;
        }
        if !is_target_type_handled(self.target_type) {
            warn_no_handler(self.target_type);
            return;
        }
        if world.rng().below(100) > self.chance {
            return;
        }
        world.trigger_skill_on_targets(self.skill, self.target_type, caster, Some(target));
    }
}

impl EffectHandler for BySkill {
    fn on_exit(&self, world: &mut World, cx: &EffectCx<'_>) {
        world.remove_trigger_listeners(cx.effected, owner_of(cx), |k| {
            matches!(k, TriggerKind::SkillUse(_))
        });
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        world.add_trigger_listener(cx.effected, owner_of(cx), TriggerKind::SkillUse(*self));
    }
}
