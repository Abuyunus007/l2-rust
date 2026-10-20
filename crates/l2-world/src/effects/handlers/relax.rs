//! `Relax` and `ChameleonRest`: sit down and recover, paying MP for it.
// Java: dist/game/data/scripts/handlers/skill/effects/Relax.java
// Java: dist/game/data/scripts/handlers/skill/effects/ChameleonRest.java

use l2_data::skill::EffectTemplate;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;

use super::status_params::double_or;
use crate::effects::registry::{EffectCx, EffectHandler, EffectType, effect_flag};
use crate::world::World;

/// What both handlers do when they start: a player sits down, anybody else rests.
fn sit_or_rest(world: &mut World, cx: &EffectCx<'_>) {
    if world.is_player(cx.effected) {
        world.sit_down(cx.effected, false);
    } else {
        // `getAI().setIntentionRest()` of a `CreatureAI`: stop following and go idle.
        world.stop_follow(cx.effected);
        world.set_intention_idle(cx.effected);
    }
}

/// Java `Relax`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Relax {
    power: f64,
}

impl Relax {
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Relax {
            power: double_or(template, "power", 0.0)?,
        }))
    }
}

impl EffectHandler for Relax {
    fn effect_flags(&self) -> u32 {
        effect_flag::RELAXING
    }

    fn effect_type(&self) -> EffectType {
        EffectType::Relaxing
    }

    fn on_action_time(&self, world: &mut World, cx: &EffectCx<'_>) -> bool {
        let effected = cx.effected;
        if world.is_dead(effected) {
            return false;
        }
        if world.is_player(effected) && !world.is_sitting(effected) {
            return false;
        }
        let (hp, mp) = world
            .core_of(effected)
            .map_or((0.0, 0.0), |c| (c.cur_hp, c.cur_mp));
        let max_hp = world.stats(effected).map_or(0, |v| v.max_recoverable_hp());
        if (hp + 1.0) > f64::from(max_hp) && cx.skill.is_toggle() {
            world.send(
                effected,
                &SystemMessage::new(SystemMessageId::ThatSkillHasBeenDeActivatedAsHpWasFullyRecovered),
            );
            return false;
        }
        let mana_dam = self.power * cx.ticks_multiplier(world);
        if mana_dam > mp && cx.skill.is_toggle() {
            world.send_lack_of_mp(effected);
            return false;
        }
        world.reduce_mp(effected, mana_dam);
        cx.skill.is_toggle()
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        sit_or_rest(world, cx);
    }
}

/// Java `ChameleonRest`: `Relax` that also hides the sitter from monsters (silent move).
#[derive(Debug, Clone, Copy)]
pub(crate) struct ChameleonRest {
    power: f64,
}

impl ChameleonRest {
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(ChameleonRest {
            power: double_or(template, "power", 0.0)?,
        }))
    }
}

impl EffectHandler for ChameleonRest {
    fn effect_flags(&self) -> u32 {
        effect_flag::SILENT_MOVE | effect_flag::RELAXING
    }

    fn effect_type(&self) -> EffectType {
        EffectType::Relaxing
    }

    fn on_action_time(&self, world: &mut World, cx: &EffectCx<'_>) -> bool {
        let effected = cx.effected;
        if world.is_dead(effected) {
            return false;
        }
        if world.is_player(effected) && !world.is_sitting(effected) {
            return false;
        }
        let mp = world.core_of(effected).map_or(0.0, |c| c.cur_mp);
        let mana_dam = self.power * cx.ticks_multiplier(world);
        // NOTE(java-quirk): unlike `Relax`, the MP check does not look at the toggle flag.
        if mana_dam > mp {
            world.send_lack_of_mp(effected);
            return false;
        }
        world.reduce_mp(effected, mana_dam);
        cx.skill.is_toggle()
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        sit_or_rest(world, cx);
    }
}
