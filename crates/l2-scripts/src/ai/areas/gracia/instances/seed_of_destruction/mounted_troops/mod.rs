//! `data/scripts/ai/areas/Gracia/instances/SeedOfDestruction/MountedTroops`: the four troops.

pub mod chamblains_mounted_troop;
mod common;
pub mod great_warriors_mounted_troop;
pub mod soldiers_mounted_troop;
pub mod warriors_mounted_troop;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(chamblains_mounted_troop::ChamblainsMountedTroop);
    r.add(great_warriors_mounted_troop::GreatWarriorsMountedTroop);
    r.add(soldiers_mounted_troop::SoldiersMountedTroop);
    r.add(warriors_mounted_troop::WarriorsMountedTroop);
}
