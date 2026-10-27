//! `RequestCursedWeaponList` and `RequestCursedWeaponLocation`.
// Java: network/clientpackets/RequestCursedWeaponList.java
// Java: network/clientpackets/RequestCursedWeaponLocation.java

use l2_protocol_game::server::{CursedWeaponLocation, ExCursedWeaponList, ExCursedWeaponLocation};
use l2_util::ObjectId;

use crate::world::World;

/// The cursed weapon requests of the client (`ClientInput::Cursed`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursedInput {
    /// `RequestCursedWeaponList` (`0xD0 0x2A`).
    List,
    /// `RequestCursedWeaponLocation` (`0xD0 0x2B`).
    Location,
}

impl World {
    pub(crate) fn on_cursed_input(&mut self, player: ObjectId, input: CursedInput) {
        if !self.players.contains_key(&player) {
            return;
        }
        match input {
            // The ids of every cursed weapon, active or not.
            CursedInput::List => {
                let ids = self.cursed_weapon_ids();
                self.send(player, &ExCursedWeaponList { ids });
            }
            CursedInput::Location => {
                let mut locations = Vec::new();
                for weapon in self.cursed.weapons() {
                    if !weapon.is_active() {
                        continue;
                    }
                    if let Some((x, y, z)) = self.cursed_world_position(weapon.item_id) {
                        locations.push(CursedWeaponLocation {
                            id: weapon.item_id,
                            activated: i32::from(weapon.is_activated),
                            x,
                            y,
                            z,
                        });
                    }
                }
                // Nothing is sent when no weapon is in the world.
                if !locations.is_empty() {
                    self.send(player, &ExCursedWeaponLocation { locations });
                }
            }
        }
    }
}
