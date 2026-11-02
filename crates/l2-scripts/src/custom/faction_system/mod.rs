//! `data/scripts/custom/FactionSystem`: the faction manager.

pub mod faction_system;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(faction_system::FactionSystem);
}
