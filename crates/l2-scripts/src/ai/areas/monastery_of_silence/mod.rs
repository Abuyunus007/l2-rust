//! `data/scripts/ai/areas/MonasteryOfSilence`: the Monastery of Silence scripts.
//!
//! One module per script, sorted by name; one `add` line per script in [`register`].

pub mod etis_van_etina;
pub mod etis_van_etina_phantom;
pub mod monastery_of_silence;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(etis_van_etina::EtisVanEtina);
    r.add(etis_van_etina_phantom::EtisVanEtinaPhantom);
    r.add(monastery_of_silence::MonasteryOfSilence);
}
