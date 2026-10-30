//! `data/scripts/ai/areas/ForestOfTheDead`: the Forest of the Dead scripts.
//!
//! One module per script, sorted by name; one `add` line per script in [`register`].

pub mod eilhalder_von_hellmann;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(eilhalder_von_hellmann::EilhalderVonHellmann);
}
