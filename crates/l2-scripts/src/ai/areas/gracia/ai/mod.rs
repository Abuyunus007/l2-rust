//! `data/scripts/ai/areas/Gracia/AI`: the NPCs and monsters of Gracia.

pub mod npc;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    npc::register(r);
}
