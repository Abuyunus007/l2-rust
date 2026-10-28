//! `Hellbound/AI/NPC`: the town NPCs of Hellbound.

pub mod bernarde;
pub mod budenka;
pub mod buron;
pub mod deltuva;
pub mod falk;
pub mod hude;
pub mod jude;
pub mod kanaf;
pub mod kief;
pub mod natives;
pub mod quarry;
pub mod shadai;
pub mod solomon;
pub mod warpgate;

use l2_world::ScriptRegistry;

/// Registers every script of the folder in Java's `HellboundLoader` order.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(bernarde::Bernarde);
    r.add(budenka::Budenka);
    r.add(buron::Buron);
    r.add(deltuva::Deltuva);
    r.add(falk::Falk);
    r.add(hude::Hude);
    r.add(jude::Jude);
    r.add(kanaf::Kanaf);
    r.add(kief::Kief);
    r.add(natives::Natives);
    r.add(quarry::Quarry);
    r.add(shadai::Shadai);
    r.add(solomon::Solomon);
    r.add(warpgate::Warpgate);
}
