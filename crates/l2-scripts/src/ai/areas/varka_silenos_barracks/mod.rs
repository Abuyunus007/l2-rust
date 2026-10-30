//! `data/scripts/ai/areas/VarkaSilenosBarracks`: the Varka Silenos Barracks scripts.
//!
//! One module per script, sorted by name; one `add` line per script in [`register`].

pub mod varka_silenos_support;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(varka_silenos_support::VarkaSilenosSupport);
}
