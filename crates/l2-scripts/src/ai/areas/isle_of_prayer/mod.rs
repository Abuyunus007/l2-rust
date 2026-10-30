//! `data/scripts/ai/areas/IsleOfPrayer`: the Isle of Prayer scripts.
//!
//! One module per script, sorted by name; one `add` line per script in [`register`].

pub mod dark_water_dragon;
pub mod evas_gift_box;
// Java names the folder and its main script alike.
#[allow(clippy::module_inception)]
pub mod isle_of_prayer;
pub mod rignos;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(dark_water_dragon::DarkWaterDragon);
    r.add(evas_gift_box::EvasGiftBox);
    r.add(isle_of_prayer::IsleOfPrayer);
    r.add(rignos::Rignos);
}
