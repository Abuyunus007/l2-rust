//! `Hellbound/AI/Zones`: the towers and workshops of Hellbound.

pub mod base_tower;
pub mod tower_of_infinitum;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(base_tower::BaseTower);
    r.add(tower_of_infinitum::TowerOfInfinitum);
}
