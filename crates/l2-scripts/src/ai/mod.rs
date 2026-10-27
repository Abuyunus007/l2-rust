//! `data/scripts/ai`: the AI scripts (`areas`, `bosses`, `others`).

pub mod others;

use l2_world::ScriptRegistry;

/// Registers every AI script, folder by folder in Java's order.
pub(crate) fn register(r: &mut ScriptRegistry) {
    others::register(r);
}
