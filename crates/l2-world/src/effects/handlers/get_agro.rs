//! The `GetAgro` effect: the NPC attacks the caster (the chance is `chance`), and the NPCs of its
//! clan that can move come to help with a little hate for the caster.
// Java: dist/game/data/scripts/handlers/skill/effects/GetAgro.java

use l2_data::skill::EffectTemplate;

use super::hate::is_clan;
use super::params;
use crate::effects::EffectType;
use crate::effects::registry::{EffectCx, EffectHandler};
use crate::formulas::Rnd;
use crate::world::World;

/// Java `GetAgro`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GetAgro {
    chance: i32,
}

impl GetAgro {
    /// Java `GetAgro(attachCond, applyCond, set, params)`: `chance` (default 100).
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self {
            chance: params::int(template, "chance", 100)?,
        }))
    }
}

impl EffectHandler for GetAgro {
    fn effect_type(&self) -> EffectType {
        EffectType::Aggression
    }

    fn is_instant(&self) -> bool {
        true
    }

    fn calc_success(&self, world: &mut World, _cx: &EffectCx<'_>) -> bool {
        world.rng().get_int(100) < self.chance
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if !world.is_attackable_npc(cx.effected) {
            return;
        }
        world.set_intention_attack(cx.effected, cx.effector);
        // NOTE: Java's clan set is `null` for an NPC without clans; an empty set stands for it.
        let Some((clans, help_range)) = world
            .stats(cx.effected)
            .and_then(|v| v.npc_template())
            .map(|t| (t.clans.clone(), t.clan_help_range))
        else {
            return;
        };
        if clans.is_empty() {
            return;
        }
        let generic_clan = world.npc_world.generic_clan_id();
        for nearby in world.visible_objects_in_range(cx.effected, help_range) {
            if !world.is_attackable_npc(nearby) || world.is_movement_disabled(nearby) {
                continue;
            }
            let Some(nearby_clans) = world
                .stats(nearby)
                .and_then(|v| v.npc_template())
                .map(|t| t.clans.clone())
            else {
                continue;
            };
            if !is_clan(&nearby_clans, &clans, generic_clan) {
                continue;
            }
            world.add_damage_hate(nearby, cx.effector, 1, 200);
            world.set_intention_attack(nearby, cx.effector);
            world.set_running(nearby, true);
        }
    }
}
