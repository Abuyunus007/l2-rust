//! `data/scripts/ai/areas/CryptsOfDisgrace`: the Crypts of Disgrace scripts.
//!
//! One module per script, sorted by name; one `add` line per script in [`register`].

pub mod legend_orc_buff;
pub mod turka_commander_ghost;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(legend_orc_buff::LegendOrcBuff);
    r.add(turka_commander_ghost::TurkaCommanderGhost);
}
