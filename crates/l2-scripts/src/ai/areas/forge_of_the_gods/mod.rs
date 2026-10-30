//! `data/scripts/ai/areas/ForgeOfTheGods`: the Forge of the Gods scripts.
//!
//! One module per script, sorted by name; one `add` line per script in [`register`].

pub mod forge_of_the_gods;
pub mod rooney;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(forge_of_the_gods::ForgeOfTheGods);
    r.add(rooney::Rooney);
}
