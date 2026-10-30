//! `data/scripts/ai/areas`: the scripts of one hunting ground or town each, a folder per area.

pub mod beast_farm;
pub mod dragon_valley;
pub mod fantasy_isle;
pub mod forge_of_the_gods;
pub mod gracia;
pub mod hellbound;
pub mod monastery_of_silence;
pub mod pagan_temple;
pub mod primeval_isle;
pub mod sel_mahum_training_grounds;

use l2_world::ScriptRegistry;

/// Registers every area, folder by folder in Java's order.
pub(crate) fn register(r: &mut ScriptRegistry) {
    beast_farm::register(r);
    dragon_valley::register(r);
    fantasy_isle::register(r);
    forge_of_the_gods::register(r);
    gracia::register(r);
    hellbound::register(r);
    monastery_of_silence::register(r);
    pagan_temple::register(r);
    primeval_isle::register(r);
    sel_mahum_training_grounds::register(r);
}
