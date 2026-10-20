//! The resurrection effects: `Resurrection` (a spell that brings a dead creature back) and
//! `ResurrectionSpecial` (a buff that asks its owner to come back when it ends, at death).
// Java: dist/game/data/scripts/handlers/skill/effects/Resurrection.java
// Java: dist/game/data/scripts/handlers/skill/effects/ResurrectionSpecial.java

use l2_data::skill::EffectTemplate;
use tracing::debug;

use super::params;
use crate::effects::registry::{EffectCx, EffectHandler, EffectType, effect_flag};
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
        if world.is_player(cx.effector) {
            // `effected.asPlayer()` is null for an NPC; a pet (`isPet()`) is M6.
            if world.is_player(cx.effected) {
                world.revive_request(cx.effected, cx.effector, false, self.power);
            }
        } else {
            world.cancel_decay_task(cx.effected);
            let percent = world.skill_resurrect_restore_percent(f64::from(self.power), cx.effector);
            world.do_revive_with_power(cx.effected, percent);
        }
    }
}

/// Java `ResurrectionSpecial`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ResurrectionSpecial {
    power: i32,
}

impl ResurrectionSpecial {
    /// Java `ResurrectionSpecial(attachCond, applyCond, set, params)`: `power` (default 0).
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self {
            power: params::int(template, "power", 0)?,
        }))
    }
}

impl EffectHandler for ResurrectionSpecial {
    fn effect_type(&self) -> EffectType {
        EffectType::ResurrectionSpecial
    }

    fn effect_flags(&self) -> u32 {
        effect_flag::RESURRECTION_SPECIAL
    }

    fn on_exit(&self, world: &mut World, cx: &EffectCx<'_>) {
        // `!effected.isPlayer() && !effected.isPet()`: pets are M6.
        if !world.is_player(cx.effected) {
            return;
        }
        // `player.isInOlympiadMode()`: the olympiad is not modelled.
        // NOTE(java-quirk): `effector.asPlayer()` is null when the buff came from an NPC and
        // `reviveRequest` then fails with a NullPointerException; the request is dropped here.
        if !world.is_player(cx.effector) {
            debug!("ResurrectionSpecial: the caster is not a player, no resurrection request");
            return;
        }
        world.revive_request(cx.effected, cx.effector, false, self.power);
    }
}
