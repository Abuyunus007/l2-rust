//! The `Escape` effect: the creature is sent to the restart point of its `escapeType` (a town, a
//! castle, a clan hall, a siege flag or a fortress). A guard goes back to its spawn instead.
// Java: dist/game/data/scripts/handlers/skill/effects/Escape.java

use l2_data::skill::EffectTemplate;

use super::params;
use crate::effects::EffectType;
use crate::effects::registry::{EffectCx, EffectHandler};
use crate::teleport::{TeleportTarget, TeleportWhereType};
use crate::world::World;

/// Java `Escape`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Escape {
    escape_type: Option<TeleportWhereType>,
}

impl Escape {
    /// Java `Escape(attachCond, applyCond, set, params)`: `escapeType` (a `TeleportWhereType` name,
    /// none when missing or unknown, as Java's `getEnum` default).
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        let escape_type = params::string(template, "escapeType")?
            .as_deref()
            .and_then(where_from_name);
        Ok(Box::new(Self { escape_type }))
    }
}

/// Java `Enum.valueOf(TeleportWhereType.class, name)`.
fn where_from_name(name: &str) -> Option<TeleportWhereType> {
    match name {
        "CASTLE" => Some(TeleportWhereType::Castle),
        "CLANHALL" => Some(TeleportWhereType::ClanHall),
        "SIEGEFLAG" => Some(TeleportWhereType::SiegeFlag),
        "TOWN" => Some(TeleportWhereType::Town),
        "FORTRESS" => Some(TeleportWhereType::Fortress),
        _ => None,
    }
}

impl EffectHandler for Escape {
    fn effect_type(&self) -> EffectType {
        EffectType::Teleport
    }

    fn is_instant(&self) -> bool {
        true
    }

    // NOTE: the Krateis arena and UC state checks of `Teleport` apply here too (not in the world).
    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        let Some(where_type) = self.escape_type else {
            return;
        };
        let is_guard = world
            .stats(cx.effected)
            .and_then(|v| v.npc_template())
            .is_some_and(|t| matches!(t.npc_type.as_str(), "Guard" | "QuestGuard"));
        if is_guard {
            // Java `Guard`: back to the spawn point, with the spawn heading.
            let spawn = world
                .npcs
                .get(&cx.effected)
                .and_then(|n| n.spawn)
                .and_then(|index| world.npc_world.spawn_data()?.spawns().get(index));
            let Some((x, y, z, heading)) =
                spawn.map(|s| (s.location.x, s.location.y, s.location.z, s.heading))
            else {
                return;
            };
            world.tele_to_location(cx.effected, TeleportTarget::at(x, y, z));
            world.set_heading(cx.effected, heading);
        } else {
            world.tele_to_location_where(cx.effected, where_type);
        }
        // NOTE: Java also calls `setIn7sDungeon(false)` on a player; the Seven Signs dungeons are
        // not in the world yet.
    }
}
