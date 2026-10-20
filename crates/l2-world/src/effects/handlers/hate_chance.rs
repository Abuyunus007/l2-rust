//! The hate effects with a chance: `DeleteHate`, `DeleteHateOfMe`, `RandomizeHate`,
//! `TransferHate` and `TargetMeProbability`. Their landing roll is `Formulas.calcProbability`.
// Java: dist/game/data/scripts/handlers/skill/effects/DeleteHate.java
// Java: dist/game/data/scripts/handlers/skill/effects/DeleteHateOfMe.java
// Java: dist/game/data/scripts/handlers/skill/effects/RandomizeHate.java
// Java: dist/game/data/scripts/handlers/skill/effects/TransferHate.java
// Java: dist/game/data/scripts/handlers/skill/effects/TargetMeProbability.java

use l2_data::skill::EffectTemplate;
use l2_util::ObjectId;

use super::hate::reduce_hate;
use super::params;
use crate::effects::registry::{EffectCx, EffectHandler, EffectType};
use crate::formulas::Rnd;
use crate::skills::TargetContext;
use crate::world::World;

/// Java `Formulas.calcProbability(_chance, effector, effected, skill)`.
fn calc_success(world: &mut World, cx: &EffectCx<'_>, chance: i32) -> bool {
    world.calc_probability_for_effect(f64::from(chance), cx.effector, cx.effected, cx.skill)
}

/// `target.setWalking(); target.getAI().setIntentionActive();`
fn walk_and_go_active(world: &mut World, npc: ObjectId) {
    // Java `setWalking()` (fake players are not ported).
    world.set_running(npc, false);
    world.set_intention_active(npc);
}

/// Java `DeleteHate`: the NPC forgets everybody it hates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DeleteHate {
    chance: i32,
}

impl DeleteHate {
    /// Java `DeleteHate(attachCond, applyCond, set, params)`: `chance` (default 100).
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self {
            chance: params::int(template, "chance", 100)?,
        }))
    }
}

impl EffectHandler for DeleteHate {
    fn calc_success(&self, world: &mut World, cx: &EffectCx<'_>) -> bool {
        calc_success(world, cx, self.chance)
    }

    fn effect_type(&self) -> EffectType {
        EffectType::Hate
    }

    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if !world.is_attackable_npc(cx.effected) {
            return;
        }
        world.aggro_clear(cx.effected);
        walk_and_go_active(world, cx.effected);
    }
}

/// Java `DeleteHateOfMe`: the NPC forgets the hate it has for the caster.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DeleteHateOfMe {
    chance: i32,
}

impl DeleteHateOfMe {
    /// Java `DeleteHateOfMe(attachCond, applyCond, set, params)`: `chance` (default 100).
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self {
            chance: params::int(template, "chance", 100)?,
        }))
    }
}

impl EffectHandler for DeleteHateOfMe {
    fn calc_success(&self, world: &mut World, cx: &EffectCx<'_>) -> bool {
        calc_success(world, cx, self.chance)
    }

    fn effect_type(&self) -> EffectType {
        EffectType::Hate
    }

    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if !world.is_attackable_npc(cx.effected) {
            return;
        }
        world.aggro_stop_hating(cx.effected, cx.effector);
        walk_and_go_active(world, cx.effected);
    }
}

/// Java `RandomizeHate`: the NPC moves the hate it has for the caster to somebody else on its
/// aggro list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RandomizeHate {
    chance: i32,
}

impl RandomizeHate {
    /// Java `RandomizeHate(attachCond, applyCond, set, params)`: `chance` (default 100).
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self {
            chance: params::int(template, "chance", 100)?,
        }))
    }
}

impl EffectHandler for RandomizeHate {
    fn calc_success(&self, world: &mut World, cx: &EffectCx<'_>) -> bool {
        calc_success(world, cx, self.chance)
    }

    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        let (effector, effected) = (cx.effector, cx.effected);
        if effected == effector || !world.is_attackable_npc(effected) {
            return;
        }
        // NOTE(java-quirk): Java walks a `ConcurrentHashMap` keyed by creature, whose order is
        // unrelated to the order the attackers were added; the aggro list order stands in.
        let others: Vec<ObjectId> = world
            .aggro_list(effected)
            .iter()
            .map(|info| info.attacker)
            .filter(|attacker| *attacker != effector)
            .collect();
        if others.is_empty() {
            return;
        }
        // Choosing randomly a new target.
        let size = i32::try_from(others.len()).unwrap_or(i32::MAX);
        let index = usize::try_from(world.rng().get_int(size)).unwrap_or(0);
        let Some(&target) = others.get(index) else {
            return;
        };
        let hate = world.aggro_hating(effected, effector);
        world.aggro_stop_hating(effected, effector);
        world.add_damage_hate(effected, target, 0, hate);
    }
}

/// Java `TransferHate`: the hate every NPC around has for the caster goes to the effected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TransferHate {
    chance: i32,
}

impl TransferHate {
    /// Java `TransferHate(attachCond, applyCond, set, params)`: `chance` (default 100).
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self {
            chance: params::int(template, "chance", 100)?,
        }))
    }
}

impl EffectHandler for TransferHate {
    fn calc_success(&self, world: &mut World, cx: &EffectCx<'_>) -> bool {
        calc_success(world, cx, self.chance)
    }

    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        let (effector, effected) = (cx.effector, cx.effected);
        for hater in world.visible_objects_in_range(effector, cx.skill.affect_range) {
            if !world.is_attackable_npc(hater) || world.is_dead(hater) {
                continue;
            }
            let hate = world.aggro_hating(hater, effector);
            if hate <= 0 {
                continue;
            }
            reduce_hate(world, hater, effector, -hate);
            world.add_damage_hate(hater, effected, 0, hate);
        }
    }
}

/// Java `TargetMeProbability`: a playable is made to target the caster.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TargetMeProbability {
    chance: i32,
}

impl TargetMeProbability {
    /// Java `TargetMeProbability(attachCond, applyCond, set, params)`: `chance` (default 100).
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self {
            chance: params::int(template, "chance", 100)?,
        }))
    }
}

impl EffectHandler for TargetMeProbability {
    fn calc_success(&self, world: &mut World, cx: &EffectCx<'_>) -> bool {
        calc_success(world, cx, self.chance)
    }

    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        let (effector, effected) = (cx.effector, cx.effected);
        // `effected.isPlayable()`: a player (summons are M6) that does not target the caster yet.
        if !world.is_player(effected) || world.target_of(effected) == Some(effector) {
            return;
        }
        // If effector is null, then its not a player, but NPC. If it is not null, then it should
        // check if the skill is pvp skill.
        let player = world.is_player(effector).then_some(effector);
        let allowed = match player {
            None => true,
            // M5-CAST-01: `_currentSkill.isCtrlPressed()` is not known here.
            Some(player) => world.check_pvp_skill(player, effected, cx.skill, &TargetContext::default()),
        };
        if allowed {
            // Target is different.
            // NOTE(java-quirk): an NPC caster passes `null`, which clears the target.
            world.set_target(effected, player);
        }
    }
}
