//! `data/scripts/custom`: the custom scripts (events, validators, ...).

pub mod events;
pub mod validators;

use l2_world::ScriptRegistry;

/// Registers every custom script, folder by folder in Java's order.
pub(crate) fn register(r: &mut ScriptRegistry) {
    events::register(r);
    validators::register(r);
}
