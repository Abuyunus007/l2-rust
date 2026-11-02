//! `data/scripts/custom/FactionSystem`: the faction manager.

// The Java folder and its class are both `FactionSystem`.
#[allow(clippy::module_inception)]
pub mod faction_system;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(faction_system::FactionSystem);
}
