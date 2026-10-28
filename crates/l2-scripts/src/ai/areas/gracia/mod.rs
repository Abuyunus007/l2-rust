//! `data/scripts/ai/areas/Gracia`: Gracia's seeds, its NPCs, monsters and instances.
//!
//! The air ship docks of `Gracia/vehicles` are built into the world (`l2_world::vehicle`).

pub mod ai;
pub mod instances;
pub mod seeds;
pub mod util;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    // Java makes the seed managers before the scripts load (`GameServer`).
    r.add(seeds::SeedOfDestructionManager);
    r.add(seeds::SeedOfInfinityManager);
    ai::register(r);
    instances::register(r);
}
