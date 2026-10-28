//! `data/scripts/ai`: the AI scripts (`areas`, `bosses`, `others`).

pub mod bosses;
pub mod others;

use l2_world::ScriptRegistry;

/// Registers every AI script, folder by folder in Java's order.
pub(crate) fn register(r: &mut ScriptRegistry) {
    bosses::register(r);
    others::register(r);
}
