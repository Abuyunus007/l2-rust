//! `data/scripts/ai/areas/PaganTemple`: the Pagan Temple scripts.
//!
//! One module per script, sorted by name; one `add` line per script in [`register`].

pub mod altar_gatekeeper;
pub mod andreas_van_halter;
pub mod pagan_keys;
pub mod pagan_teleporters;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(altar_gatekeeper::AltarGatekeeper);
    r.add(andreas_van_halter::AndreasVanHalter);
    r.add(pagan_keys::PaganKeys);
    r.add(pagan_teleporters::PaganTeleporters);
}
