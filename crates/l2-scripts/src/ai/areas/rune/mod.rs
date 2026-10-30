//! `data/scripts/ai/areas/Rune`: the scripts of the Rune Township area.
//!
//! One module per Java folder; the leaf folders of a single script are flattened.

pub mod rune_castle;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    rune_castle::register(r);
}
