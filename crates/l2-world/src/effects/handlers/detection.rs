//! The `Detection` effect: a player who casts it on an invisible player (one that is neither in its
//! party, clan nor alliance) makes that player's `Hide` effect go.
// Java: dist/game/data/scripts/handlers/skill/effects/Detection.java

use l2_data::skill::EffectTemplate;
use l2_data::stats::AbnormalType;

use crate::effects::SkillFinishType;
use crate::effects::registry::{EffectCx, EffectHandler};
use crate::world::World;

/// Java `Detection`. It reads no parameters.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Detection;

impl Detection {
    /// Java `Detection(attachCond, applyCond, set, params)`.
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Detection))
    }
}

impl EffectHandler for Detection {
    fn is_instant(&self) -> bool {
        true
    }

    // NOTE: Java returns early when the two players are in one party, clan or alliance. The world
    // has no parties, clans or alliances yet, so every pair counts as not together (what Java
    // answers for players without them). Revisit this with the party and clan milestones.
    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if !world.is_player(cx.effector) || !world.is_player(cx.effected) {
            return;
        }
        let invisible = world
            .objects
            .get(&cx.effected)
            .is_some_and(|entry| entry.invisible);
        if invisible {
            world.stop_skill_effects_by_abnormal(cx.effected, SkillFinishType::Removed, AbnormalType::Hide);
        }
    }
}
