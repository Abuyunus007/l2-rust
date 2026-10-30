//! The `SummonNpc` effect: a decoy, or an NPC that despawns after a while.
// Java: dist/game/data/scripts/handlers/skill/effects/SummonNpc.java

use l2_data::skill::EffectTemplate;
use tracing::warn;

use super::params;
use crate::effects::registry::{EffectCx, EffectHandler, EffectType};
use crate::world::World;

/// Java `SummonNpc`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SummonNpc {
    despawn_delay: i32,
    npc_id: i32,
    npc_count: i32,
    random_offset: bool,
    #[allow(dead_code)]
    is_summon_spawn: bool,
}

impl SummonNpc {
    /// Java `SummonNpc(attachCond, applyCond, set, params)`: `despawnDelay` (20000), `npcId` (0),
    /// `npcCount` (1), `randomOffset` (false), `isSummonSpawn` (false).
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self {
            despawn_delay: params::int(template, "despawnDelay", 20000)?,
            npc_id: params::int(template, "npcId", 0)?,
            npc_count: params::int(template, "npcCount", 1)?,
            random_offset: params::boolean(template, "randomOffset", false)?,
            is_summon_spawn: params::boolean(template, "isSummonSpawn", false)?,
        }))
    }
}

impl EffectHandler for SummonNpc {
    fn effect_type(&self) -> EffectType {
        EffectType::SummonNpc
    }

    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if !world.is_player(cx.effected) || world.is_alike_dead(cx.effected) {
            return;
        }
        if self.npc_id <= 0 || self.npc_count <= 0 {
            warn!("SummonNpc: Invalid NPC ID or count skill ID: {}", cx.skill.id);
            return;
        }
        let player = cx.effected;
        if world.in_observer_mode(player) || world.is_mounted(player) {
            return;
        }
        let Some(npc_type) = world.npc_world.template(self.npc_id).map(|t| t.npc_type.clone()) else {
            warn!(
                "SummonNpc: Spawn of the nonexisting NPC ID: {}, skill ID:{}",
                self.npc_id, cx.skill.id
            );
            return;
        };
        match npc_type.as_str() {
            "Decoy" => {
                world.summon_decoy(player, self.npc_id, self.despawn_delay);
            }
            // TODO in Java: "Implement proper signet skills." The `EffectPoint` creature is not in
            // the world, so a signet's effect point is not made.
            "EffectPoint" => {}
            _ => world.summon_npc_effect_spawn(player, self.npc_id, self.despawn_delay, self.random_offset),
        }
    }
}
