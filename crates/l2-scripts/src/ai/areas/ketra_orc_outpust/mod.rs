//! `data/scripts/ai/areas/KetraOrcOutpust` (sic): the Ketra Orc Outpost scripts.
//!
//! One module per script, sorted by name; one `add` line per script in [`register`].

pub mod ketra_orc_support;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(ketra_orc_support::KetraOrcSupport);
}
