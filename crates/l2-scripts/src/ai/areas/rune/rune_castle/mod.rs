//! `data/scripts/ai/areas/Rune/RuneCastle`: the scripts of Rune Castle.
//!
//! One module per script, sorted by name; one `add` line per script in [`register`].

pub mod venom;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(venom::Venom);
}
