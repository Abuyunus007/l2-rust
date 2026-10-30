//! The trap effects: `SummonTrap`, `TrapDetect` and `TrapRemove`.
// Java: dist/game/data/scripts/handlers/skill/effects/{SummonTrap,TrapDetect,TrapRemove}.java

use l2_data::skill::EffectTemplate;
use tracing::warn;

use super::params;
use crate::effects::registry::{EffectCx, EffectHandler};
use crate::world::World;

/// Java `SummonTrap`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SummonTrap {
    despawn_time: i32,
    npc_id: i32,
}

impl SummonTrap {
    /// Java `SummonTrap(..)`: `despawnTime` (0), `npcId` (0).
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self {
            despawn_time: params::int(template, "despawnTime", 0)?,
            npc_id: params::int(template, "npcId", 0)?,
        }))
    }
}

impl EffectHandler for SummonTrap {
    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if !world.is_player(cx.effected) || world.is_alike_dead(cx.effected) {
            return;
        }
        if self.npc_id <= 0 {
            warn!("SummonTrap: Invalid NPC ID:{} in skill ID: {}", self.npc_id, cx.skill.id);
            return;
        }
        let player = cx.effected;
        if world.in_observer_mode(player) || world.is_mounted(player) {
            return;
        }
        if world.npc_world.template(self.npc_id).is_none() {
            // Java unsummons the previous trap before it looks the template up.
            if let Some(old) = world.trap_of(player) {
                world.trap_unsummon(old);
            }
            warn!(
                "SummonTrap: Spawn of the non-existing Trap ID: {} in skill ID:{}",
                self.npc_id, cx.skill.id
            );
            return;
        }
        world.summon_trap(player, self.npc_id, self.despawn_time);
    }
}

/// The `power` of `TrapDetect` and `TrapRemove`: "effect without power!" when the effect has no
/// parameters, and `params.getInt("power")` throws when it is missing.
fn power_of(template: &EffectTemplate, name: &str) -> Result<i32, String> {
    if template.params.is_empty() {
        return Err(format!("{name}: effect without power!"));
    }
    if template.param("power").is_none() {
        return Err("Integer value required, but not specified: power!".to_owned());
    }
    params::int(template, "power", 0)
}

/// Java `TrapDetect`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TrapDetect {
    power: i32,
}

impl TrapDetect {
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self {
            power: power_of(template, "TrapDetect")?,
        }))
    }
}

impl EffectHandler for TrapDetect {
    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        world.trap_detect_effect(cx.effector, cx.effected, self.power);
    }
}

/// Java `TrapRemove`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TrapRemove {
    power: i32,
}

impl TrapRemove {
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self {
            power: power_of(template, "TrapRemove")?,
        }))
    }
}

impl EffectHandler for TrapRemove {
    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        world.trap_remove_effect(cx.effector, cx.effected, self.power);
    }
}
