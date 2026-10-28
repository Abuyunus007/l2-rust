//! `data/scripts/ai/areas/Gracia/instances/SeedOfDestruction`: the Seed of Destruction instances.

pub mod mounted_troops;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    mounted_troops::register(r);
}
