//! `data/scripts/ai/areas/BeastFarm`: the Beast Farm scripts.
//!
//! One module per script, sorted by name; one `add` line per script in [`register`].

pub mod tunatun;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(tunatun::Tunatun);
}
