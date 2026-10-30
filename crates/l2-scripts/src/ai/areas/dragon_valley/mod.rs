//! `data/scripts/ai/areas/DragonValley`: the Dragon Valley scripts.
//!
//! One module per script, sorted by name; one `add` line per script in [`register`].

pub mod dragon_valley;
pub mod dragon_vortex;
pub mod lair_of_antharas;
pub mod separated_soul;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(dragon_valley::DragonValley);
    r.add(dragon_vortex::DragonVortex);
    r.add(lair_of_antharas::LairOfAntharas);
    r.add(separated_soul::SeparatedSoul);
}
