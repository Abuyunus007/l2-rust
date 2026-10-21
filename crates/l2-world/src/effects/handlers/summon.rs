//! The `Summon` effect: the caster calls its servitor.
// Java: dist/game/data/scripts/handlers/skill/effects/Summon.java

use l2_data::npc::NpcRace;
use l2_data::skill::EffectTemplate;

use super::params;
use crate::effects::registry::{EffectCx, EffectHandler};
use crate::summon::spawn::ServitorParams;
use crate::world::World;

/// Java `Summon`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Summon {
    npc_id: i32,
    exp_multiplier: f32,
    consume_item: (i32, i32),
    /// `lifeTime` seconds * 1000.
    life_time: i32,
    /// `consumeItemInterval`, seconds (0 = the default of the servitor's race).
    consume_item_interval: i32,
}

impl Summon {
    /// Java `Summon(attachCond, applyCond, set, params)`: `npcId` (required), `expMultiplier`
    /// (1), `consumeItemId` (0), `consumeItemCount` (1), `consumeItemInterval` (0), `lifeTime`
    /// (3600 s).
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        if template.params.is_empty() {
            return Err("Summon effect without parameters!".to_owned());
        }
        if template.param("npcId").is_none() {
            return Err("Integer value required, but not specified: npcId!".to_owned());
        }
        Ok(Box::new(Self {
            npc_id: params::int(template, "npcId", 0)?,
            exp_multiplier: params::float(template, "expMultiplier", 1.0)?,
            consume_item: (
                params::int(template, "consumeItemId", 0)?,
                params::int(template, "consumeItemCount", 1)?,
            ),
            consume_item_interval: params::int(template, "consumeItemInterval", 0)?,
            life_time: params::int(template, "lifeTime", 3600)?.wrapping_mul(1000),
        }))
    }
}

impl EffectHandler for Summon {
    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if !world.is_player(cx.effector) || world.summon_of(cx.effector).is_some() {
            return;
        }
        // `effected.asPlayer()`: the owner (the skill targets the caster itself).
        let Some(player) = world.acting_player(cx.effected) else {
            return;
        };
        let race = world.npc_world.template(self.npc_id).map(|t| t.race);
        let interval = if self.consume_item_interval > 0 {
            self.consume_item_interval
        } else if race != Some(NpcRace::SiegeWeapon) {
            240
        } else {
            60
        };
        world.summon_servitor(
            player,
            ServitorParams {
                npc_id: self.npc_id,
                exp_multiplier: self.exp_multiplier,
                consume_item: self.consume_item,
                life_time_ms: self.life_time,
                consume_interval_ms: interval.wrapping_mul(1000),
                reference_skill: cx.skill.id,
            },
        );
    }
}
