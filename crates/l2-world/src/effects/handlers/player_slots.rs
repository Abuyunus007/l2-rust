//! The passive effects that open equipment slots or change the expertise penalty:
//! `TalismanSlot`, `EnableCloak` and `CrystalGradeModify`. Their values are read by the player's
//! `UserInfo`, the inventory and the expertise check through `CreatureControl`.
// Java: dist/game/data/scripts/handlers/skill/effects/TalismanSlot.java
// Java: dist/game/data/scripts/handlers/skill/effects/EnableCloak.java
// Java: dist/game/data/scripts/handlers/skill/effects/CrystalGradeModify.java

use l2_data::skill::EffectTemplate;

use super::status_params::int_or;
use crate::effects::registry::{EffectCx, EffectHandler};
use crate::world::World;

/// Java `TalismanSlot`: `slots` more talisman slots.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TalismanSlot {
    slots: i32,
}

impl TalismanSlot {
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(TalismanSlot {
            slots: int_or(template, "slots", 0)?,
        }))
    }
}

impl EffectHandler for TalismanSlot {
    fn can_start(&self, world: &World, cx: &EffectCx<'_>) -> bool {
        world.is_player(cx.effected)
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if let Some(c) = world.control_mut(cx.effected) {
            c.talisman_slots = c.talisman_slots.wrapping_add(self.slots);
        }
    }

    fn on_action_time(&self, _world: &mut World, cx: &EffectCx<'_>) -> bool {
        cx.skill.is_passive()
    }

    fn on_exit(&self, world: &mut World, cx: &EffectCx<'_>) {
        if let Some(c) = world.control_mut(cx.effected) {
            c.talisman_slots = c.talisman_slots.wrapping_sub(self.slots);
        }
    }
}

/// Java `EnableCloak`: the player may wear a cloak.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct EnableCloak;

impl EnableCloak {
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(EnableCloak))
    }
}

impl EffectHandler for EnableCloak {
    fn can_start(&self, world: &World, cx: &EffectCx<'_>) -> bool {
        world.is_player(cx.effected)
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if let Some(c) = world.control_mut(cx.effected) {
            c.cloak_slot = true;
        }
    }

    fn on_action_time(&self, _world: &mut World, cx: &EffectCx<'_>) -> bool {
        cx.skill.is_passive()
    }

    fn on_exit(&self, world: &mut World, cx: &EffectCx<'_>) {
        if let Some(c) = world.control_mut(cx.effected) {
            c.cloak_slot = false;
        }
    }
}

/// Java `CrystalGradeModify`: the expertise penalty counts `grade` grades fewer.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CrystalGradeModify {
    grade: i32,
}

impl CrystalGradeModify {
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(CrystalGradeModify {
            grade: int_or(template, "grade", 0)?,
        }))
    }
}

impl EffectHandler for CrystalGradeModify {
    fn can_start(&self, world: &World, cx: &EffectCx<'_>) -> bool {
        world.is_player(cx.effected)
    }

    fn on_exit(&self, world: &mut World, cx: &EffectCx<'_>) {
        if let Some(c) = world.control_mut(cx.effected) {
            c.expertise_penalty_bonus = 0;
        }
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if let Some(c) = world.control_mut(cx.effected) {
            c.expertise_penalty_bonus = self.grade;
        }
    }
}
