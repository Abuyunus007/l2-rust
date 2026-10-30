//! `data/scripts/ai/areas/ImperialTomb`: the Imperial Tomb scripts.
//!
//! One module per script, sorted by name; one `add` line per script in [`register`].

pub mod four_sepulchers;
pub mod four_sepulchers_data;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(four_sepulchers::FourSepulchers);
}
