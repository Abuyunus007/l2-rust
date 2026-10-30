//! `data/scripts/ai/areas/PavelArchaic`: the Pavel Archaic scripts.
//!
//! One module per script, sorted by name; one `add` line per script in [`register`].

// Java names the folder and its main script alike.
#[allow(clippy::module_inception)]
pub mod pavel_archaic;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(pavel_archaic::PavelArchaic);
}
