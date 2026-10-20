//! The `Resurrection` effect, the branch of an NPC caster. A player caster only proposes the
//! resurrection to the target (`Player.reviveRequest`, a dialog): M5-EFX-05.
// Java: dist/game/data/scripts/handlers/skill/effects/Resurrection.java

use l2_data::skill::EffectTemplate;

use super::params;
use crate::effects::registry::{EffectCx, EffectHandler, EffectType};
use crate::formulas::calculate_skill_resurrect_restore_percent;
use crate::stats::BaseStat;
use crate::world::World;

/// Java `Resurrection`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Resurrection {
    power: i32,
}

impl Resurrection {
    /// Java `Resurrection(attachCond, applyCond, set, params)`: `power` (default 0).
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self {
            power: params::int(template, "power", 0)?,
        }))
    }
}

impl EffectHandler for Resurrection {
    fn effect_type(&self) -> EffectType {
        EffectType::Resurrection
    }

    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        let (effector, effected) = (cx.effector, cx.effected);
        if world.is_player(effector) {
            // M5-EFX-05: `effected.asPlayer().reviveRequest(effector, effected.isPet(), power)`
            // (the `ConfirmDlg` and its answer).
            return;
        }
        let wit_bonus = world.stats(effector).map_or(1.0, |v| v.bonus(BaseStat::Wit));
        let percent = calculate_skill_resurrect_restore_percent(f64::from(self.power), wit_bonus);
        world.cancel_decay_task(effected);
        // `Creature.doRevive(revivePower)` ignores the power; `Player.doRevive(revivePower)` gives
        // back that share of the experience lost at death.
        if world.is_player(effected) {
            world.do_revive_with_power(effected, percent);
        } else {
            world.do_revive(effected);
        }
    }
}
