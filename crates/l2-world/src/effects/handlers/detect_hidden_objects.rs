//! The `DetectHiddenObjects` effect: a stealth door shows its other mesh and becomes targetable when
//! its open type is not zero.
// Java: dist/game/data/scripts/handlers/skill/effects/DetectHiddenObjects.java

use l2_data::skill::EffectTemplate;

use crate::effects::registry::{EffectCx, EffectHandler};
use crate::world::World;

/// Java `DetectHiddenObjects`. It reads no parameters.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct DetectHiddenObjects;

impl DetectHiddenObjects {
    /// Java `DetectHiddenObjects(attachCond, applyCond, set, params)`.
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(DetectHiddenObjects))
    }
}

impl EffectHandler for DetectHiddenObjects {
    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        let Some(door) = world.doors.get_mut(&cx.effected) else {
            return;
        };
        if !door.template.stealth {
            return;
        }
        let targetable = door.template.open_type != 0;
        // Java `setMeshIndex(1)`, then `setTargetable(...)`, which sends the door's status.
        door.mesh_index = 1;
        world.set_door_targetable(cx.effected, targetable);
    }
}
