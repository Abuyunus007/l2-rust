//! `data/scripts/ai/areas/BeastFarm`: the Beast Farm scripts.
//!
//! One module per script, sorted by name; one `add` line per script in [`register`].

#[allow(clippy::module_inception)]
pub mod beast_farm;
pub mod feedable_beasts;
pub mod tunatun;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(beast_farm::BeastFarm);
    r.add(feedable_beasts::FeedableBeasts);
    r.add(tunatun::Tunatun);
}
