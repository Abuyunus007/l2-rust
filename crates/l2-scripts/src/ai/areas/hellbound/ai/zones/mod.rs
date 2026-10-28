//! `Hellbound/AI/Zones`: the towers and workshops of Hellbound.

pub mod anomic_foundry;
pub mod base_tower;
pub mod tower_of_infinitum;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(anomic_foundry::AnomicFoundry);
    r.add(base_tower::BaseTower);
    r.add(tower_of_infinitum::TowerOfInfinitum);
}
