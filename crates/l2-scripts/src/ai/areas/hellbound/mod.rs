//! `data/scripts/ai/areas/Hellbound`: the Hellbound engine (level and trust), its NPCs, monsters,
//! zones and instances. Java's `HellboundLoader` loads the engine first, then the scripts.

pub mod data;
pub mod engine;

use l2_world::ScriptRegistry;

/// Registers every script of the folder, the engine first (Java's `HellboundLoader` order).
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(engine::HellboundEngine);
}
