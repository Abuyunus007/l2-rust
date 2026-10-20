//! `TargetMe`: the playable has to target the caster (`TargetMeProbability` is in `hate_chance.rs`).
// Java: dist/game/data/scripts/handlers/skill/effects/TargetMe.java

use l2_data::skill::EffectTemplate;

use super::params;
use crate::effects::registry::{EffectCx, EffectHandler};
use crate::world::World;

/// The target switch both effects start with: a playable that does not target the effector yet
/// targets it, if the skill may be used on it.
///
/// NOTE(java-quirk): `effected.setTarget(player)` runs with `player = effector.asPlayer()`, which
/// is `null` for an NPC caster: an NPC's `TargetMe` clears the target of the playable instead of
/// setting it.
fn switch_target(world: &mut World, cx: &EffectCx<'_>) {
    let effected = cx.effected;
    if world.target_of(effected) == Some(cx.effector) {
        return;
    }
    let player = world.is_player(cx.effector).then_some(cx.effector);
    let allowed = match player {
        // "If it is not null, then it should check if the skill is pvp skill."
        Some(player) => {
            let ctx = world.cast_target_context(player);
            world.check_pvp_skill(player, effected, cx.skill, &ctx)
        }
        None => true,
    };
    if allowed {
        world.set_target(effected, player);
    }
}

/// Java `TargetMe`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TargetMe {
    chance: i32,
}

impl TargetMe {
    /// Java `TargetMe(attachCond, applyCond, set, params)`: `chance` (default 100).
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self {
            chance: params::int(template, "chance", 100)?,
        }))
    }
}

impl EffectHandler for TargetMe {
    // NOTE(java-quirk): `TargetMe` is not instant, and the engine only asks an instant effect
    // whether it lands (`Skill.applyEffectScope`), so this roll never happens.
    fn calc_success(&self, world: &mut World, _cx: &EffectCx<'_>) -> bool {
        world.rng().below(100) < self.chance
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if world.is_playable(cx.effected) {
            switch_target(world, cx);
            world.set_locked_target(cx.effected, Some(cx.effector));
        }
    }

    fn on_exit(&self, world: &mut World, cx: &EffectCx<'_>) {
        if world.is_playable(cx.effected) {
            world.set_locked_target(cx.effected, None);
        }
    }
}
