//! The `RollingDice` item handler.
// Java: dist/game/data/scripts/handlers/items/RollingDice.java

use l2_geo::Location;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{Dice, SystemMessage, SystemMessageParam};
use l2_util::ObjectId;

use crate::flood::FloodKind;
use crate::movement::heading_to_degree;
use crate::player::ItemData;
use crate::world::World;
use crate::zones::ZoneId;

impl World {
    /// Java `RollingDice.onItemUse`.
    pub(crate) fn rolling_dice_on_use(&mut self, id: ObjectId, item: &ItemData) -> bool {
        if !self.players.contains_key(&id) {
            // A pet's `sendPacket` goes to its owner.
            if let Some(owner) = self.acting_player(id) {
                self.send(
                    owner,
                    &SystemMessage::new(SystemMessageId::YourPetCannotCarryThisItem),
                );
            }
            return false;
        }
        if self.is_in_olympiad_mode(id) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouCannotUseThatItemInAGrandOlympiadMatch),
            );
            return false;
        }

        let number = self.roll_dice(id);
        if number == 0 {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouMayNotThrowTheDiceAtThisTimeTryAgainLater),
            );
            return false;
        }

        // Mobius: retail dice position land calculation.
        let (Some((px, py, pz)), heading) = (self.position_of(id), self.heading_of(id)) else {
            return false;
        };
        let angle = heading_to_degree(heading);
        let radian = angle.to_radians();
        let course = 180.0_f64.to_radians();
        let x1 = ((std::f64::consts::PI + radian + course).cos() * 40.0) as i32;
        let y1 = ((std::f64::consts::PI + radian + course).sin() * 40.0) as i32;
        let instance = self.instance_id_of(id);
        let destination = self.geo_valid_location(
            Location { x: px, y: py, z: pz },
            Location {
                x: px.wrapping_add(x1),
                y: py.wrapping_add(y1),
                z: pz,
            },
            instance,
        );
        // `World.broadcastToSelfAndVisiblePlayers`.
        let dice = Dice {
            object_id: id.0,
            item_id: item.item_id,
            number,
            x: destination.x,
            y: destination.y,
            z: destination.z,
        };
        self.send(id, &dice);
        for viewer in self.visible_players(id) {
            self.send(viewer, &dice);
        }

        let name = self
            .players
            .get(&id)
            .map(|p| p.data.name.clone())
            .unwrap_or_default();
        let message = SystemMessage::new(SystemMessageId::C1HasRolledAS2)
            .with(SystemMessageParam::Text(name))
            .with(SystemMessageParam::IntNumber(number));
        self.send(id, &message);
        if self.is_inside_zone(id, ZoneId::Peace) {
            for viewer in self.visible_players(id) {
                self.send(viewer, &message);
            }
        } else if self.party_of(id).is_some() {
            // TODO(java): "Verify this!"
            for member in self.party_members_of(id) {
                if member != id {
                    self.send(member, &message);
                }
            }
        }
        true
    }

    /// Java `RollingDice.rollDice`: 0 while the flood protector refuses, else `Rnd.get(1, 6)`.
    fn roll_dice(&mut self, id: ObjectId) -> i32 {
        if !self.flood_allows(id, FloodKind::RollDice) {
            return 0;
        }
        self.rng().get(1, 6)
    }
}
