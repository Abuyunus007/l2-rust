//! The `RentPet` NPC bypass: a wyvern or a strider for adena.
// Java: dist/game/data/scripts/handlers/bypass/npc/RentPet.java
// Java: entity/actor/Player.java (mount, startRentPet): `mount.rs`

use l2_protocol_game::server::{NpcHtmlMessage, SetupGauge};
use l2_util::ObjectId;

use crate::bypass::BypassHandlers;
use crate::world::World;

/// Registers the handler of this module (called by `BypassHandlers::default`).
pub(crate) fn register(handlers: &mut BypassHandlers) {
    handlers.register("RentPet", rent_pet_handler);
}

/// The pet page of Java `RentPet.onCommand` (`%objectId%` is replaced by the NPC).
const RENT_PET_PAGE: &str = "<html><body>Pet Manager:<br>You can rent a wyvern or strider for adena.<br>My prices:<br1><table border=0><tr><td>Ride</td></tr><tr><td>Wyvern</td><td>Strider</td></tr><tr><td><a action=\"bypass -h npc_%objectId%_RentPet 1\">30 sec/1800 adena</a></td><td><a action=\"bypass -h npc_%objectId%_RentPet 11\">30 sec/900 adena</a></td></tr><tr><td><a action=\"bypass -h npc_%objectId%_RentPet 2\">1 min/7200 adena</a></td><td><a action=\"bypass -h npc_%objectId%_RentPet 12\">1 min/3600 adena</a></td></tr><tr><td><a action=\"bypass -h npc_%objectId%_RentPet 3\">10 min/720000 adena</a></td><td><a action=\"bypass -h npc_%objectId%_RentPet 13\">10 min/360000 adena</a></td></tr><tr><td><a action=\"bypass -h npc_%objectId%_RentPet 4\">30 min/6480000 adena</a></td><td><a action=\"bypass -h npc_%objectId%_RentPet 14\">30 min/3240000 adena</a></td></tr></table></body></html>";
/// `RentPet.tryRentPet`: the price of each ride in adena and its length in seconds.
const RENT_COST: [i64; 4] = [1800, 7200, 720_000, 6_480_000];
const RENT_TIME: [i32; 4] = [30, 60, 600, 1800];
/// The mounts: a wyvern for 1 to 4, a strider (half the price) for 11 to 14.
const WYVERN: i32 = 12621;
const STRIDER: i32 = 12526;

/// What `tryRentPet` charges and rides for a menu value: (npc id, price, seconds). `None` where
/// Java returns without renting.
pub(crate) fn rent_pet_offer(pet_value: i32) -> Option<(i32, i64, i32)> {
    let (pet_id, value, half) = if pet_value > 10 {
        (STRIDER, pet_value - 10, true)
    } else {
        (WYVERN, pet_value, false)
    };
    if !(1..=4).contains(&value) {
        return None;
    }
    let index = usize::try_from(value - 1).ok()?;
    let mut price = 1.0_f64;
    if half {
        price /= 2.0;
    }
    price *= RENT_COST[index] as f64;
    Some((pet_id, price as i64, RENT_TIME[index]))
}

/// Java `RentPet.onCommand`.
fn rent_pet_handler(world: &mut World, player: ObjectId, command: &str, target: Option<ObjectId>) -> bool {
    let Some(npc) = target.filter(|t| world.npc_is_merchant(*t)) else {
        return false;
    };
    if !world.script_handlers.settings.allow_rentpet {
        return false;
    }
    let npc_id = world.npcs.get(&npc).map_or(0, |n| n.npc_id);
    if !world.settings.talk.list_pet_rent_npc.contains(&npc_id) {
        return false;
    }
    let mut tokens = command.split(' ').filter(|t| !t.is_empty());
    tokens.next();
    match tokens.next() {
        None => {
            let html = RENT_PET_PAGE.replace("%objectId%", &npc.0.to_string());
            if let Some(message) = NpcHtmlMessage::new(npc.0, &html) {
                world.send_html(player, &message);
            }
            true
        }
        Some(value) => match value.parse::<i32>() {
            Ok(pet_value) => {
                world.try_rent_pet(player, pet_value);
                true
            }
            Err(_) => {
                tracing::info!("Exception in RentPet");
                false
            }
        },
    }
}

impl World {
    /// Java `RentPet.tryRentPet`.
    fn try_rent_pet(&mut self, player: ObjectId, pet_value: i32) {
        if !self.players.contains_key(&player)
            || self.summon_of(player).is_some()
            || self.is_mounted(player)
            || self.is_rented_pet(player)
            || self.is_transformed(player)
            || self.is_cursed_weapon_equipped(player)
        {
            return;
        }
        if !self.disarm_weapons(player) {
            return;
        }
        let Some((pet_id, price, time)) = rent_pet_offer(pet_value) else {
            return;
        };
        if !self.reduce_adena(player, price, true) {
            return;
        }
        self.mount_npc(player, pet_id, ObjectId(0), false);
        self.send(player, &SetupGauge::new(player.0, 3, time.wrapping_mul(1000)));
        self.start_rent_pet(player, time);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rent_menu_values() {
        assert_eq!(rent_pet_offer(1), Some((12621, 1800, 30)));
        assert_eq!(rent_pet_offer(4), Some((12621, 6_480_000, 1800)));
        assert_eq!(rent_pet_offer(11), Some((12526, 900, 30)));
        assert_eq!(rent_pet_offer(14), Some((12526, 3_240_000, 1800)));
        assert_eq!(rent_pet_offer(0), None);
        assert_eq!(rent_pet_offer(5), None);
        assert_eq!(rent_pet_offer(15), None);
    }
}
