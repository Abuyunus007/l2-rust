//! The `OpenDwarfRecipeBook` and `OpenCommonRecipeBook` effects: the skill opens the recipe book
//! window of the caster.
// Java: dist/game/data/scripts/handlers/skill/effects/OpenDwarfRecipeBook.java
// Java: dist/game/data/scripts/handlers/skill/effects/OpenCommonRecipeBook.java

use l2_data::skill::EffectTemplate;

use crate::effects::registry::{EffectCx, EffectHandler};
use crate::world::World;

/// Java `OpenDwarfRecipeBook`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OpenDwarfRecipeBook;

impl OpenDwarfRecipeBook {
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self))
    }
}

impl EffectHandler for OpenDwarfRecipeBook {
    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        world.open_recipe_book_effect(cx.effector, true);
    }
}

/// Java `OpenCommonRecipeBook`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OpenCommonRecipeBook;

impl OpenCommonRecipeBook {
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self))
    }
}

impl EffectHandler for OpenCommonRecipeBook {
    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        world.open_recipe_book_effect(cx.effector, false);
    }
}
