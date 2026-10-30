//! `data/scripts/ai/areas/ForgeOfTheGods`: the Forge of the Gods scripts.
//!
//! One module per script, sorted by name; one `add` line per script in [`register`].

#[allow(clippy::module_inception)]
pub mod forge_of_the_gods;
pub mod rooney;
pub mod tar_beetle;
pub mod tar_beetle_data;
pub mod tar_beetle_spawn;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(forge_of_the_gods::ForgeOfTheGods);
    r.add(rooney::Rooney);
    r.add(tar_beetle::TarBeetle);
}
