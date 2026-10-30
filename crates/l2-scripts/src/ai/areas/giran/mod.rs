//! `data/scripts/ai/areas/Giran`: the Giran scripts.
//!
//! One module per script, sorted by name; one `add` line per script in [`register`].

pub mod alexandria;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(alexandria::Alexandria);
}
