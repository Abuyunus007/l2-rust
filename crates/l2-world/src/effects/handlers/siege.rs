//! The castle siege effects: `HeadquarterCreate` (Build Headquarters) and `TakeCastle` (Seal of
//! Ruler), see `siege/npcs.rs` (M7-SIEGE-01).
// Java: dist/game/data/scripts/handlers/skill/effects/HeadquarterCreate.java
// Java: dist/game/data/scripts/handlers/skill/effects/TakeCastle.java

use l2_data::skill::EffectTemplate;

use super::statset::get_boolean;
use crate::effects::registry::{EffectCx, EffectHandler};
use crate::world::World;

/// Java `HeadquarterCreate`: the clan leader's headquarters (advanced with `isAdvanced`).
#[derive(Debug, Clone, Copy)]
pub(crate) struct HeadquarterCreate {
    /// `isAdvanced`.
    advanced: bool,
}

impl HeadquarterCreate {
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self {
            advanced: get_boolean(template, "isAdvanced", false),
        }))
    }
}

impl EffectHandler for HeadquarterCreate {
    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        // `effector.asPlayer()`: a summon's owner.
        let Some(player) = world.acting_player(cx.effector) else {
            return;
        };
        world.siege_create_headquarters(player, self.advanced);
    }
}

/// Java `TakeCastle`: the caster engraves the artefact.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct TakeCastle;

impl TakeCastle {
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(TakeCastle))
    }
}

impl EffectHandler for TakeCastle {
    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if !world.is_player(cx.effector) {
            return;
        }
        world.castle_engrave(cx.effector, cx.effected);
    }
}
