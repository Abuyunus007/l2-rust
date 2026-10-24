//! The `Fishing`, `Pumping` and `Reeling` effects: casting the line and fighting the fish.
// Java: dist/game/data/scripts/handlers/skill/effects/Fishing.java
// Java: dist/game/data/scripts/handlers/skill/effects/Pumping.java
// Java: dist/game/data/scripts/handlers/skill/effects/Reeling.java
//
// The work is in `fishing/` (`cast.rs`, `combat.rs`); these only read the parameters and call it.

use l2_data::skill::EffectTemplate;

use super::params;
use crate::effects::registry::{EffectCx, EffectHandler, EffectType};
use crate::fishing::FishAction;
use crate::world::World;

/// Java `Fishing`: the skill 1312. Instant, `EffectType.FISHING_START`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Fishing;

impl Fishing {
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self))
    }
}

impl EffectHandler for Fishing {
    fn effect_type(&self) -> EffectType {
        EffectType::FishingStart
    }

    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        // `if (!effector.isPlayer()) return;`
        if world.is_player(cx.effector) {
            world.fishing_effect_start(cx.effector);
        }
    }
}

/// The `power` parameter of `Pumping` and `Reeling`, which has no default.
fn power(name: &str, template: &EffectTemplate) -> Result<f64, String> {
    // `params.getString("power", null) == null` throws `IllegalArgumentException`.
    if params::string(template, "power")?.is_none() {
        return Err(format!("{name}: effect without power!"));
    }
    params::double(template, "power", 0.0)
}

/// Java `Pumping`: skill 1313. Instant, `EffectType.FISHING`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Pumping {
    power: f64,
}

impl Pumping {
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self {
            power: power("Pumping", template)?,
        }))
    }
}

impl EffectHandler for Pumping {
    fn effect_type(&self) -> EffectType {
        EffectType::Fishing
    }

    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if world.is_player(cx.effector) {
            world.pumping_reeling_effect_start(cx.effector, FishAction::Pumping, self.power, cx.skill.level);
        }
    }
}

/// Java `Reeling`: skill 1314. Instant, `EffectType.FISHING`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Reeling {
    power: f64,
}

impl Reeling {
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self {
            power: power("Reeling", template)?,
        }))
    }
}

impl EffectHandler for Reeling {
    fn effect_type(&self) -> EffectType {
        EffectType::Fishing
    }

    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if world.is_player(cx.effector) {
            world.pumping_reeling_effect_start(cx.effector, FishAction::Reeling, self.power, cx.skill.level);
        }
    }
}
