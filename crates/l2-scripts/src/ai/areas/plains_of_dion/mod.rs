//! `data/scripts/ai/areas/PlainsOfDion`: the Plains of Dion scripts.
//!
//! One module per script, sorted by name; one `add` line per script in [`register`].

// Java names the folder and its main script alike.
#[allow(clippy::module_inception)]
pub mod plains_of_dion;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(plains_of_dion::PlainsOfDion);
}
