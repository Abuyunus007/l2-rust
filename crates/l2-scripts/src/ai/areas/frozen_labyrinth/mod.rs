//! `data/scripts/ai/areas/FrozenLabyrinth`: the Frozen Labyrinth scripts.
//!
//! One module per script, sorted by name; one `add` line per script in [`register`].

pub mod freyas_steward;
// Java names the folder and its main script alike.
#[allow(clippy::module_inception)]
pub mod frozen_labyrinth;
pub mod jinia;
pub mod rafforty;
pub mod sirra;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(freyas_steward::FreyasSteward);
    r.add(frozen_labyrinth::FrozenLabyrinth);
    r.add(jinia::Jinia);
    r.add(rafforty::Rafforty);
    r.add(sirra::Sirra);
}
