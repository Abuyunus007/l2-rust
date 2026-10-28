//! `data/scripts/custom`: the custom scripts (validators, ...).

pub mod validators;

use l2_world::ScriptRegistry;

/// Registers every custom script, folder by folder in Java's order.
pub(crate) fn register(r: &mut ScriptRegistry) {
    validators::register(r);
}
