//! Confusion and betrayal: the creature turns on somebody else.
// Java: dist/game/data/scripts/handlers/skill/effects/Confuse.java
// Java: dist/game/data/scripts/handlers/skill/effects/Betray.java

use l2_data::skill::EffectTemplate;

use super::status_params::int_or;
use crate::effects::registry::{EffectCx, EffectHandler, EffectType, effect_flag};
use crate::world::World;

/// Java `Confuse`.
// NOTE(java-quirk): the effect is instant (`isInstant()` is true), so it never joins a buff info:
// its `CONFUSED` flag and its `onExit` are never used and nothing is ever "confused" through it.
// The one thing it does is `onStart`: the creature attacks a random creature it can see.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Confuse {
    chance: i32,
}

impl Confuse {
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Confuse {
            chance: int_or(template, "chance", 100)?,
        }))
    }
}

impl EffectHandler for Confuse {
    fn calc_success(&self, world: &mut World, cx: &EffectCx<'_>) -> bool {
        world.calc_probability_for_effect(f64::from(self.chance), cx.effector, cx.effected, cx.skill)
    }

    fn effect_flags(&self) -> u32 {
        effect_flag::CONFUSED
    }

    fn is_instant(&self) -> bool {
        true
    }

    fn on_exit(&self, world: &mut World, cx: &EffectCx<'_>) {
        if !world.is_player(cx.effected) {
            world.notify_action_think(cx.effected);
        }
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        world.notify_action_confused(cx.effected);
        // Choosing a uniformly-random visible Creature as the new target.
        if let Some(target) = world.random_visible_creature(cx.effected) {
            // Attacking the target.
            world.set_creature_target(cx.effected, Some(target));
            world.set_intention_attack(cx.effected, target);
        }
    }
}

/// Java `Betray`: a player turns somebody else's summon against its master.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Betray;

impl Betray {
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Betray))
    }
}

impl EffectHandler for Betray {
    /// `effector.isPlayer() && effected.isSummon()`.
    fn can_start(&self, world: &World, cx: &EffectCx<'_>) -> bool {
        world.is_player(cx.effector) && world.is_summon(cx.effected)
    }

    fn effect_flags(&self) -> u32 {
        effect_flag::BETRAYED
    }

    fn effect_type(&self) -> EffectType {
        EffectType::Debuff
    }

    fn on_exit(&self, world: &mut World, cx: &EffectCx<'_>) {
        world.set_intention_idle(cx.effected);
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        // `effected.getAI().setIntentionAttack(effected.asPlayer())`: the summon attacks its own
        // master.
        if let Some(owner) = world.acting_player(cx.effected) {
            world.set_intention_attack(cx.effected, owner);
        }
    }
}
