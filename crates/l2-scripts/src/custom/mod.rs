//! `data/scripts/custom`: the custom scripts (events, validators, ...).

pub mod banking_auto_convert;
pub mod class_buffer;
pub mod class_buffer_data;
pub mod companions;
pub mod delevel_manager;
pub mod echo_crystals;
pub mod events;
pub mod faction_system;
pub mod fake_players;
pub mod nobless_master;
pub mod raidboss_info;
pub mod rift;
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
    r.add(banking_auto_convert::BankingAutoConvert);
    r.add(class_buffer::ClassBuffer);
    r.add(companions::Companions);
    r.add(rift::RiftInvasion);
    events::register(r);
    faction_system::register(r);
    fake_players::register(r);
    validators::register(r);
}
