//! User commands (`BypassUserCmd`): `/loc` and `/partyinfo`.
// Java: network/clientpackets/BypassUserCmd.java
// Java: dist/game/data/scripts/handlers/chat/commands/user/Loc.java
// Java: dist/game/data/scripts/handlers/chat/commands/user/PartyInfo.java
//
// Java's `//loc` does not exist; the client's `/loc` is user command 0. The other user commands
// (clan penalty, channels, time ...) belong to their systems; the card of each ports its handler.

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use tracing::{debug, warn};

use crate::world::World;
use crate::zones::race_name;

/// The "RespawnZone" zone type of `zones.xml`.
const RESPAWN_ZONE: &str = "RespawnZone";

impl World {
    /// Java `BypassUserCmd.runImpl`.
    pub(crate) fn on_user_command(&mut self, id: ObjectId, command: i32) {
        if !self.players.contains_key(&id) {
            return;
        }
        if command == 0 {
            self.user_command_loc(id);
        } else if command == crate::party::PARTY_INFO_COMMAND {
            self.user_command_party_info(id);
        } else {
            // Java tells a GM "User commandID N not implemented yet." for an id without handler.
            // Handlers that exist in Java but are not ported yet would make that message a lie,
            // so it stays a debug line until the user commands are ported.
            debug!("{id}: user command {command} is not ported yet");
        }
    }

    /// Java `Loc.onCommand`: the name of the area (a system message of the client) with the
    /// coordinates, or the plain coordinates.
    fn user_command_loc(&mut self, id: ObjectId) {
        let Some((x, y, z)) = self.position_of(id) else {
            return;
        };
        let race = self
            .players
            .get(&id)
            .and_then(|p| race_name(p.data.race))
            .unwrap_or("");
        let zones = self.zoning.zones.clone();
        let regions = self.zoning.map_regions.clone();

        // `ZoneManager.getZone(player, RespawnZone.class)`: the restart region of that zone, else
        // the map region the player stands in.
        let respawn_zone = zones
            .as_ref()
            .and_then(|zones| zones.zone_at(x, y, z, RESPAWN_ZONE));
        let region = match (&regions, respawn_zone) {
            (Some(regions), Some(zone)) => regions.restart_region_loc_id(race, zone.respawn_point("HUMAN")),
            (Some(regions), None) => regions.map_region_loc_id(x, y),
            (None, _) => 0,
        };

        let named = (region > 0).then(|| SystemMessageId::from_id(region)).flatten();
        let message = match named {
            Some(message_id) => {
                let mut message = SystemMessage::new(message_id);
                if message_id.param_count() == 3 {
                    message = message
                        .with(SystemMessageParam::IntNumber(x))
                        .with(SystemMessageParam::IntNumber(y))
                        .with(SystemMessageParam::IntNumber(z));
                }
                message
            }
            None => {
                if region > 0 {
                    warn!("{id}: map region message {region} is not a known system message");
                }
                SystemMessage::new(SystemMessageId::CurrentLocationS1)
                    .with(SystemMessageParam::Text(format!("{x}, {y}, {z}")))
            }
        };
        self.send(id, &message);
    }
}
