//! `data/scripts/ai/areas/PrimevalIsle`: the Primeval Isle scripts.
//!
//! One module per script, sorted by name; one `add` line per script in [`register`].

pub mod elroki_teleporters;
#[allow(clippy::module_inception)]
pub mod primeval_isle;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(elroki_teleporters::ElrokiTeleporters);
    r.add(primeval_isle::PrimevalIsle);
}
