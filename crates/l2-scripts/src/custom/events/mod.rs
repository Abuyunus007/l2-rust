//! `data/scripts/custom/events`: event scripts.
//!
//! One module per script, sorted by name; one `add` line per script in [`register`].

pub mod elpies;
pub mod rabbits;
pub mod race;
mod schedule;
pub mod wedding;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(elpies::Elpies);
    r.add(rabbits::Rabbits);
    r.add(race::Race);
    r.add(wedding::Wedding);
}
