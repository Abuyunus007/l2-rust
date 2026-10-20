//! The `Grow` effect: an NPC gets its "grown" collision radius while it lasts.
// Java: dist/game/data/scripts/handlers/skill/effects/Grow.java

use l2_data::skill::EffectTemplate;

use crate::effects::registry::{EffectCx, EffectHandler, EffectType};
use crate::world::World;

/// Java `Grow`. It reads no parameters.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Grow;

impl Grow {
    /// Java `Grow(attachCond, applyCond, set, params)`.
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Grow))
    }
}

impl EffectHandler for Grow {
    fn effect_type(&self) -> EffectType {
        EffectType::Buff
    }

    /// `npc.setCollisionRadius(npc.getTemplate().getFCollisionRadius())`.
    fn on_exit(&self, world: &mut World, cx: &EffectCx<'_>) {
        let radius = world
            .npc_template(cx.effected)
            .map(|t| f64::from(t.collision_radius));
        if let (Some(npc), Some(radius)) = (world.npcs.get_mut(&cx.effected), radius) {
            npc.collision_radius = radius;
        }
    }

    /// `npc.setCollisionRadius(npc.getTemplate().getCollisionRadiusGrown())`.
    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        let radius = world.npc_template(cx.effected).map(|t| t.collision_radius_grown);
        if let (Some(npc), Some(radius)) = (world.npcs.get_mut(&cx.effected), radius) {
            npc.collision_radius = radius;
        }
    }
}
