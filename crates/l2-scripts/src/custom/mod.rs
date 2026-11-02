//! `data/scripts/custom`: the custom scripts (events, validators, ...).

pub mod banking_auto_convert;
pub mod events;
pub mod faction_system;
pub mod fake_players;
pub mod validators;

use l2_world::ScriptRegistry;

/// Registers every custom script, folder by folder in Java's order.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(banking_auto_convert::BankingAutoConvert);
    events::register(r);
    faction_system::register(r);
    fake_players::register(r);
    validators::register(r);
}
