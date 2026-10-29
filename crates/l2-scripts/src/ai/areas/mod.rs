//! `data/scripts/ai/areas`: the scripts of one hunting ground or town each, a folder per area.

pub mod fantasy_isle;
pub mod gracia;
pub mod hellbound;

use l2_world::ScriptRegistry;

/// Registers every area, folder by folder in Java's order.
pub(crate) fn register(r: &mut ScriptRegistry) {
    fantasy_isle::register(r);
    gracia::register(r);
    hellbound::register(r);
}
