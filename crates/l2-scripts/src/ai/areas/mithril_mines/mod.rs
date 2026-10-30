//! `data/scripts/ai/areas/MithrilMines`: the Mithril Mines scripts.
//!
//! One module per script, sorted by name; one `add` line per script in [`register`].

pub mod grave_robbers;
pub mod mithril_mines_teleporter;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(grave_robbers::GraveRobbers);
    r.add(mithril_mines_teleporter::MithrilMinesTeleporter);
}
