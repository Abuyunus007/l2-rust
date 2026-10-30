//! `data/scripts/ai/areas/SelMahumTrainingGrounds`: the Sel Mahum Training Grounds scripts.
//!
//! One module per script, sorted by name; one `add` line per script in [`register`].

pub mod sel_mahum_drill;
pub mod sel_mahum_squad;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(sel_mahum_drill::SelMahumDrill);
    r.add(sel_mahum_squad::SelMahumSquad);
}
