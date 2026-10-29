//! `Hellbound/AI`: the monsters, NPCs and zones of Hellbound.

pub mod amaskari;
pub mod chimeras;
pub mod demon_prince;
pub mod hellbound_core;
pub mod keltas;
pub mod naia_lock;
pub mod npc;
pub mod outpost_captain;
pub mod ranku;
pub mod slaves;
pub mod typhoon;
pub mod zones;

use l2_world::ScriptRegistry;

/// Registers every script of the folder in Java's `HellboundLoader` order.
pub(crate) fn register(r: &mut ScriptRegistry) {
    // AIs
    r.add(amaskari::Amaskari);
    r.add(chimeras::Chimeras);
    r.add(demon_prince::DemonPrince);
    r.add(hellbound_core::HellboundCore);
    r.add(keltas::Keltas);
    r.add(naia_lock::NaiaLock);
    r.add(outpost_captain::OutpostCaptain);
    r.add(ranku::Ranku);
    r.add(slaves::Slaves);
    r.add(typhoon::Typhoon);
    // NPCs
    npc::register(r);
    // Zones
    zones::register(r);
}
