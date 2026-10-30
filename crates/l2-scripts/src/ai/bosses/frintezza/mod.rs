//! `data/scripts/ai/bosses/Frintezza`: the Last Imperial Tomb instance and Scarlet van Halisha.

pub mod last_imperial_tomb;
pub mod scarlet_van_halisha;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(last_imperial_tomb::LastImperialTomb);
    r.add(scarlet_van_halisha::ScarletVanHalisha);
}
