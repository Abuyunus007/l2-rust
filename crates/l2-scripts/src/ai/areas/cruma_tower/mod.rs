//! `data/scripts/ai/areas/CrumaTower`: the Cruma Tower scripts.
//!
//! One module per script, sorted by name; one `add` line per script in [`register`].

pub mod alarm;
// Java names the folder and its main script alike.
#[allow(clippy::module_inception)]
pub mod cruma_tower;
pub mod summon_pc;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(alarm::Alarm);
    r.add(cruma_tower::CrumaTower);
    r.add(summon_pc::SummonPc);
}
