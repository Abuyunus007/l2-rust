//! `Hellbound/AI`: the monsters, NPCs and zones of Hellbound.

pub mod npc;

use l2_world::ScriptRegistry;

/// Registers every script of the folder in Java's `HellboundLoader` order.
pub(crate) fn register(r: &mut ScriptRegistry) {
    npc::register(r);
}
