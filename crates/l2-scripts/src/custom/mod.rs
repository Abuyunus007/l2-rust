//! `data/scripts/custom`: the custom scripts (events, validators, ...).

pub mod delevel_manager;
pub mod echo_crystals;
pub mod events;
pub mod nobless_master;
pub mod raidboss_info;
pub mod shadow_weapons;
pub mod transmog;
pub mod validators;

use l2_world::ScriptRegistry;

/// Registers every custom script, folder by folder in Java's order.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(delevel_manager::DelevelManager);
    r.add(echo_crystals::EchoCrystals);
    r.add(nobless_master::NoblessMaster);
    r.add(raidboss_info::RaidbossInfo);
    r.add(shadow_weapons::ShadowWeapons);
    r.add(transmog::Transmog);
    events::register(r);
    validators::register(r);
}
