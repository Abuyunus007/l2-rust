//! `data/scripts/ai/areas/Gracia/AI`: the NPCs and monsters of Gracia.

pub mod lindvior;
pub mod maguen;
pub mod npc;
pub mod seed_of_annihilation;
pub mod star_stones;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(lindvior::Lindvior);
    r.add(maguen::Maguen);
    r.add(seed_of_annihilation::SeedOfAnnihilation);
    r.add(star_stones::StarStones);
    npc::register(r);
}
