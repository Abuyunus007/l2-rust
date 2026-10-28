//! `data/scripts/ai/areas/Gracia/instances/SeedOfDestruction`: the Seed of Destruction instances.

pub mod mounted_troops;
// The script file mirrors Java's SeedOfDestruction/SeedOfDestruction.java.
#[allow(clippy::module_inception)]
pub mod seed_of_destruction;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    mounted_troops::register(r);
    r.add(seed_of_destruction::SeedOfDestruction);
}
