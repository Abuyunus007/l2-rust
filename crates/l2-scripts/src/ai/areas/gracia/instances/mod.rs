//! `data/scripts/ai/areas/Gracia/instances`: the instances of Gracia's seeds.

pub mod secret_area;
pub mod seed_of_destruction;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(secret_area::SecretArea);
    seed_of_destruction::register(r);
}
