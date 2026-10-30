//! `data/scripts/ai/areas`: the scripts of one hunting ground or town each, a folder per area.

pub mod cruma_tower;
pub mod crypts_of_disgrace;
pub mod den_of_evil;
pub mod fantasy_isle;
pub mod forest_of_the_dead;
pub mod frozen_labyrinth;
pub mod giants_cave;
pub mod giran;
pub mod gracia;
pub mod hellbound;
pub mod hot_springs;
pub mod isle_of_prayer;
pub mod ketra_orc_outpust;
pub mod mithril_mines;
pub mod pavel_archaic;
pub mod plains_of_dion;
pub mod plains_of_lizardman;
pub mod rune;
pub mod silent_valley;
pub mod stakato_nest;
pub mod tower_of_insolence;
pub mod varka_silenos_barracks;

use l2_world::ScriptRegistry;

/// Registers every area, folder by folder in Java's order.
pub(crate) fn register(r: &mut ScriptRegistry) {
    cruma_tower::register(r);
    crypts_of_disgrace::register(r);
    den_of_evil::register(r);
    fantasy_isle::register(r);
    forest_of_the_dead::register(r);
    frozen_labyrinth::register(r);
    giants_cave::register(r);
    giran::register(r);
    gracia::register(r);
    hellbound::register(r);
    hot_springs::register(r);
    isle_of_prayer::register(r);
    ketra_orc_outpust::register(r);
    mithril_mines::register(r);
    pavel_archaic::register(r);
    plains_of_dion::register(r);
    plains_of_lizardman::register(r);
    rune::register(r);
    silent_valley::register(r);
    stakato_nest::register(r);
    tower_of_insolence::register(r);
    varka_silenos_barracks::register(r);
}
