//! `data/scripts/ai/areas/Gracia/instances`: the instances of Gracia's seeds.

pub mod common;
pub mod hall_of_erosion_attack;
pub mod hall_of_suffering;
pub mod hall_of_suffering_attack;
pub mod hall_of_suffering_defence;
pub mod secret_area;
pub mod seed_of_destruction;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(hall_of_erosion_attack::HallOfErosionAttack);
    r.add(hall_of_suffering_attack::HallOfSufferingAttack);
    r.add(hall_of_suffering_defence::HallOfSufferingDefence);
    r.add(secret_area::SecretArea);
    seed_of_destruction::register(r);
}
