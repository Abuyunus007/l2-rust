//! The `Transformation` effect: the buff turns a player into the form `id`; when the buff ends
//! (death, running out, `TransformDispel`), the player is itself again.
// Java: dist/game/data/scripts/handlers/skill/effects/Transformation.java

use l2_data::skill::{EffectTemplate, ParamValue};

use crate::effects::registry::{EffectCx, EffectHandler};
use crate::world::World;

/// Java `Transformation`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Transformation {
    id: i32,
}

impl Transformation {
    /// Java `Transformation(attachCond, applyCond, set, params)`: `id` (default 0).
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        let id = match template.params.get("id") {
            None => 0,
            Some(ParamValue::Text(text)) => text
                .parse()
                .map_err(|_| format!("Integer value required, but found: {text}"))?,
            Some(ParamValue::Items(_)) => return Err("parameter id is a list, not a value".to_string()),
        };
        Ok(Box::new(Self { id }))
    }
}

impl EffectHandler for Transformation {
    fn can_start(&self, world: &World, cx: &EffectCx<'_>) -> bool {
        world.is_player(cx.effected)
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        world.transform_player(self.id, cx.effected);
    }

    fn on_exit(&self, world: &mut World, cx: &EffectCx<'_>) {
        world.stop_transformation(cx.effected, false);
    }
}
