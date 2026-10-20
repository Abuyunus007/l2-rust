//! The `ConsumeBody` effect: the corpse of a dead NPC is consumed. With a start effect in the
//! skill it decays after the skill's cast time, otherwise it decays now.
// Java: dist/game/data/scripts/handlers/skill/effects/ConsumeBody.java

use l2_data::skill::{EffectScope, EffectTemplate};

use crate::effects::registry::{EffectCx, EffectHandler};
use crate::formulas::calc_atk_spd;
use crate::timer::{TimerKind, TimerWheel};
use crate::world::World;

/// Java `ConsumeBody`. It reads no parameters.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ConsumeBody;

impl ConsumeBody {
    /// Java `ConsumeBody(attachCond, applyCond, set, params)`.
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(ConsumeBody))
    }
}

impl EffectHandler for ConsumeBody {
    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if !world.is_npc(cx.effected) || !world.is_dead(cx.effected) {
            return;
        }
        if cx.skill.effects.get(EffectScope::Start).is_empty() {
            // Java `asNpc().endDecayTask()`: the corpse goes now.
            world.cancel_decay_task(cx.effected);
            world.on_npc_decay(cx.effected);
            return;
        }
        world.cancel_decay_task(cx.effected);
        let Some(view) = world.stats(cx.effector) else {
            return;
        };
        // Java `ThreadPool.schedule(effected::onDecay, Formulas.calcAtkSpd(effector, skill,
        // hitTime + coolTime))`: a plain task, so it is not in the decay table.
        let delay_ms = calc_atk_spd(
            cx.skill.is_magic(),
            f64::from(cx.skill.hit_time + cx.skill.cool_time),
            view.m_atk_spd(),
            view.p_atk_spd(),
        );
        let delay_ms = u64::try_from(delay_ms).unwrap_or(0);
        let due = world.tick + TimerWheel::ms_to_ticks(delay_ms);
        world.timers.schedule_at(due, TimerKind::Decay(cx.effected));
    }
}
