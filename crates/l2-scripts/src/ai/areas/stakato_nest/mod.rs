//! `data/scripts/ai/areas/StakatoNest`: the Stakato Nest scripts.
//!
//! One module per script, sorted by name; one `add` line per script in [`register`].

// Java names the folder and its main script alike.
#[allow(clippy::module_inception)]
pub mod stakato_nest;
pub mod stakato_nest_teleporter;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(stakato_nest::StakatoNest);
    r.add(stakato_nest_teleporter::StakatoNestTeleporter);
}
