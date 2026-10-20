//! The force and soul handlers: `FocusEnergy`, `FocusMaxEnergy`, `FocusSouls` and `SoulEating`.
// Java: dist/game/data/scripts/handlers/skill/effects/FocusEnergy.java
// Java: dist/game/data/scripts/handlers/skill/effects/FocusMaxEnergy.java
// Java: dist/game/data/scripts/handlers/skill/effects/FocusSouls.java
// Java: dist/game/data/scripts/handlers/skill/effects/SoulEating.java

use l2_data::skill::EffectTemplate;

use super::status_params::int_or;
use crate::effects::registry::{EffectCx, EffectHandler};
use crate::world::World;

/// Java `Stat.MAX_SOULS` of the stat engine, truncated: `(int) player.calcStat(MAX_SOULS, 0)`.
fn max_souls(world: &World, player: l2_util::ObjectId) -> i32 {
    world
        .stats(player)
        .map_or(0, |v| v.calc(crate::stats::Stat::MaxSouls, 0.0) as i32)
}

/// Java `FocusEnergy`: one more force charge, up to `charge`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FocusEnergy {
    charge: i32,
}

impl FocusEnergy {
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(FocusEnergy {
            charge: int_or(template, "charge", 0)?,
        }))
    }
}

impl EffectHandler for FocusEnergy {
    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if !world.is_player(cx.effected) {
            return;
        }
        world.increase_charges(cx.effected, 1, self.charge);
    }
}

/// Java `FocusMaxEnergy`: the force charges go to the maximum of Sonic / Focus Mastery.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct FocusMaxEnergy;

impl FocusMaxEnergy {
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(FocusMaxEnergy))
    }
}

/// Sonic Mastery and Focus Mastery.
const SONIC_MASTERY: i32 = 992;
const FOCUS_MASTERY: i32 = 993;

impl EffectHandler for FocusMaxEnergy {
    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        let player = cx.effected;
        if !world.is_player(player) {
            return;
        }
        let (sonic, focus) = world.stats(player).map_or((0, 0), |v| {
            (v.skill_level(SONIC_MASTERY), v.skill_level(FOCUS_MASTERY))
        });
        let max_charge = if sonic > 0 {
            sonic
        } else if focus > 0 {
            focus
        } else {
            0
        };
        if max_charge != 0 {
            let count = max_charge - world.charges_of(player);
            world.increase_charges(player, count, max_charge);
        }
    }
}

/// Java `FocusSouls`: `charge` more souls, up to the player's maximum.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FocusSouls {
    charge: i32,
}

impl FocusSouls {
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(FocusSouls {
            charge: int_or(template, "charge", 0)?,
        }))
    }
}

impl EffectHandler for FocusSouls {
    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        let target = cx.effected;
        if !world.is_player(target) || world.is_alike_dead(target) {
            return;
        }
        let max = max_souls(world, target);
        if max > 0 {
            let amount = self.charge;
            let charged = world.charged_souls_of(target);
            if charged < max {
                let count = if charged + amount <= max {
                    amount
                } else {
                    max - charged
                };
                world.increase_souls(target, count);
            } else {
                world.send(
                    target,
                    &l2_protocol_game::server::SystemMessage::new(
                        l2_protocol_game::SystemMessageId::SoulCannotBeIncreasedAnymore,
                    ),
                );
            }
        }
    }
}

/// Java `SoulEating`: a soul for every big enough experience gain, while it lasts.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SoulEating {
    exp_needed: i32,
}

impl SoulEating {
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        // `params.getInt("expNeeded")`: required.
        template
            .param("expNeeded")
            .ok_or_else(|| "Integer value required, but not specified: expNeeded!".to_owned())?;
        Ok(Box::new(SoulEating {
            exp_needed: int_or(template, "expNeeded", 0)?,
        }))
    }
}

impl EffectHandler for SoulEating {
    fn on_exit(&self, world: &mut World, cx: &EffectCx<'_>) {
        // `removeListenerIf(ON_PLAYABLE_EXP_CHANGED, owner == this)`: this effect's listener.
        if world.is_player(cx.effected) {
            let skill = (cx.skill.id, cx.skill.level);
            if let Some(control) = world.control_mut(cx.effected) {
                control.soul_eating.retain(|(owner, _)| *owner != skill);
            }
        }
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if world.is_player(cx.effected) {
            let skill = (cx.skill.id, cx.skill.level);
            if let Some(control) = world.control_mut(cx.effected) {
                control.soul_eating.push((skill, self.exp_needed));
            }
        }
    }
}
