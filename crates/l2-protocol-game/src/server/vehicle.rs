//! Server packets of the boats (vehicles) and the air ships.
//!
//! Values only: the boat and ship positions, the passengers and the fuel are the world's and
//! arrive here as plain ints. The layouts are the Java `writeImpl` order.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::{read_count, write_opcode};
use crate::generated::opcodes::ServerOpcode;

// Java: gameserver/network/serverpackets/VehicleInfo.java
server_packet!(
    /// A boat's position and heading (`VEHICLE_INFO`).
    VehicleInfo,
    VehicleInfo {
        object_id: i32,
        x: i32,
        y: i32,
        z: i32,
        heading: i32,
    }
);

// Java: gameserver/network/serverpackets/VehicleDeparture.java
server_packet!(
    /// A boat starts to move to its destination (`VEHICLE_DEPARTURE`). Java writes the speeds
    /// before the destination.
    VehicleDeparture,
    VehicleDeparture {
        object_id: i32,
        move_speed: i32,
        rotation_speed: i32,
        /// The destination, not the current position.
        x: i32,
        y: i32,
        z: i32,
    }
);

// Java: gameserver/network/serverpackets/VehicleStarted.java
server_packet!(
    /// The boat's control state changed (`VEHICLE_START`).
    VehicleStarted,
    VehicleStart {
        object_id: i32,
        /// Java `_state`.
        state: i32,
    }
);

// Java: gameserver/network/serverpackets/VehicleCheckLocation.java
server_packet!(
    /// The boat's position, for the client to check (`VEHICLE_CHECK_LOCATION`).
    VehicleCheckLocation,
    VehicleCheckLocation {
        object_id: i32,
        x: i32,
        y: i32,
        z: i32,
        heading: i32,
    }
);

// Java: gameserver/network/serverpackets/GetOnVehicle.java
server_packet!(
    /// A character boards a boat (`GET_ON_VEHICLE`).
    GetOnVehicle,
    GetOnVehicle {
        object_id: i32,
        boat_object_id: i32,
        x: i32,
        y: i32,
        z: i32,
    }
);

// Java: gameserver/network/serverpackets/GetOffVehicle.java
server_packet!(
    /// A character leaves a boat (`GET_OFF_VEHICLE`).
    GetOffVehicle,
    GetOffVehicle {
        object_id: i32,
        boat_object_id: i32,
        x: i32,
        y: i32,
        z: i32,
    }
);

// Java: gameserver/network/serverpackets/MoveToLocationInVehicle.java
server_packet!(
    /// A character walks to a place inside a boat (`MOVE_TO_LOCATION_IN_VEHICLE`).
    MoveToLocationInVehicle,
    MoveToLocationInVehicle {
        object_id: i32,
        boat_id: i32,
        destination_x: i32,
        destination_y: i32,
        destination_z: i32,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
    }
);

// Java: gameserver/network/serverpackets/StopMoveInVehicle.java
server_packet!(
    /// A character stops inside a boat (`STOP_MOVE_IN_VEHICLE`).
    StopMoveInVehicle,
    StopMoveInVehicle {
        object_id: i32,
        boat_id: i32,
        x: i32,
        y: i32,
        z: i32,
        heading: i32,
    }
);

// Java: gameserver/network/serverpackets/ValidateLocationInVehicle.java
server_packet!(
    /// The position of a character inside a boat, to validate it (`VALIDATE_LOCATION_IN_VEHICLE`).
    ValidateLocationInVehicle,
    ValidateLocationInVehicle {
        object_id: i32,
        boat_object_id: i32,
        x: i32,
        y: i32,
        z: i32,
        heading: i32,
    }
);

// Java: gameserver/network/serverpackets/ExAirShipInfo.java
/// An air ship's state for the window (`EX_AIRSHIP_INFO`).
///
/// Java writes the helm and captain positions only when there is a helm (`helm_object_id != 0`):
/// [`ExAirShipInfo::HELM_POINTS`]; otherwise six zeros. The caller passes the six values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExAirShipInfo {
    pub ship_object_id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
    /// Java `getCaptainId()`.
    pub captain_id: i32,
    pub move_speed: i32,
    pub rotation_speed: i32,
    /// Java `getHelmObjectId()`.
    pub helm_object_id: i32,
    /// The controller and captain positions (x, y, z each), as Java writes them.
    pub helm_points: [i32; 6],
    pub fuel: i32,
    pub max_fuel: i32,
}

impl ExAirShipInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExAirshipInfo;
    /// The fixed controller and captain positions Java sends when the ship has a helm.
    pub const HELM_POINTS: [i32; 6] = [0x16E, 0, 0x6B, 0x15C, 0, 0x69];
}

impl Encode for ExAirShipInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.ship_object_id)
            .i32(self.x)
            .i32(self.y)
            .i32(self.z)
            .i32(self.heading)
            .i32(self.captain_id)
            .i32(self.move_speed)
            .i32(self.rotation_speed)
            .i32(self.helm_object_id);
        for point in self.helm_points {
            w.i32(point);
        }
        w.i32(self.fuel).i32(self.max_fuel);
    }
}

impl Decode for ExAirShipInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let ship_object_id = r.i32()?;
        let x = r.i32()?;
        let y = r.i32()?;
        let z = r.i32()?;
        let heading = r.i32()?;
        let captain_id = r.i32()?;
        let move_speed = r.i32()?;
        let rotation_speed = r.i32()?;
        let helm_object_id = r.i32()?;
        let mut helm_points = [0; 6];
        for point in &mut helm_points {
            *point = r.i32()?;
        }
        Ok(Self {
            ship_object_id,
            x,
            y,
            z,
            heading,
            captain_id,
            move_speed,
            rotation_speed,
            helm_object_id,
            helm_points,
            fuel: r.i32()?,
            max_fuel: r.i32()?,
        })
    }
}

// Java: gameserver/network/serverpackets/ExAirShipStopMove.java
server_packet!(
    /// A passenger stopped moving on the air ship (`EX_STOP_MOVE_AIRSHIP`, the player layout).
    ExAirShipStopMove,
    ExStopMoveAirship {
        air_ship_id: i32,
        player_id: i32,
        x: i32,
        y: i32,
        z: i32,
    }
);

// Java: gameserver/network/serverpackets/ExStopMoveAirShip.java
server_packet!(
    /// The air ship itself stopped moving (`EX_STOP_MOVE_AIRSHIP`, the ship layout).
    ExStopMoveAirShip,
    ExStopMoveAirship {
        object_id: i32,
        x: i32,
        y: i32,
        z: i32,
        heading: i32,
    }
);

/// One destination of the air ship teleport list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AirShipTeleport {
    /// Java `i - 1` for the entry `i` of the path list: `-1` is the first entry.
    pub index: i32,
    /// Java `_fuelConsumption[i]`.
    pub fuel: i32,
    /// The last point of the path (Java `path[path.length - 1]`).
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

// Java: gameserver/network/serverpackets/ExAirShipTeleportList.java
/// The destinations of an air ship at a dock (`EX_AIRSHIP_TELEPORT_LIST`). An empty list is
/// Java's `null` path list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExAirShipTeleportList {
    pub dock_id: i32,
    pub teleports: Vec<AirShipTeleport>,
}

impl ExAirShipTeleportList {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExAirshipTeleportList;
}

impl Encode for ExAirShipTeleportList {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.dock_id).i32(self.teleports.len() as i32);
        for teleport in &self.teleports {
            w.i32(teleport.index)
                .i32(teleport.fuel)
                .i32(teleport.x)
                .i32(teleport.y)
                .i32(teleport.z);
        }
    }
}

impl Decode for ExAirShipTeleportList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let dock_id = r.i32()?;
        let count = read_count(r, "teleport count")?;
        let mut teleports = Vec::new();
        for _ in 0..count {
            teleports.push(AirShipTeleport {
                index: r.i32()?,
                fuel: r.i32()?,
                x: r.i32()?,
                y: r.i32()?,
                z: r.i32()?,
            });
        }
        Ok(Self { dock_id, teleports })
    }
}

// Java: gameserver/network/serverpackets/ExStopMoveInAirShip.java
server_packet!(
    /// A passenger stopped inside the air ship (`EX_STOP_MOVE_IN_AIRSHIP`).
    ExStopMoveInAirShip,
    ExStopMoveInAirship {
        player_id: i32,
        ship_object_id: i32,
        x: i32,
        y: i32,
        z: i32,
        heading: i32,
    }
);

// Java: gameserver/network/serverpackets/ExValidateLocationInAirShip.java
server_packet!(
    /// The position of a passenger inside the air ship, to validate it
    /// (`EX_VALIDATE_LOCATION_IN_AIRSHIP`).
    ExValidateLocationInAirShip,
    ExValidateLocationInAirship {
        player_id: i32,
        ship_id: i32,
        x: i32,
        y: i32,
        z: i32,
        heading: i32,
    }
);

// Java: gameserver/network/serverpackets/ExGetOnAirShip.java
server_packet!(
    /// A passenger boards the air ship (`EX_GET_ON_AIRSHIP`).
    ExGetOnAirShip,
    ExGetOnAirship {
        player_id: i32,
        air_ship_id: i32,
        x: i32,
        y: i32,
        z: i32,
    }
);

// Java: gameserver/network/serverpackets/ExGetOffAirShip.java
server_packet!(
    /// A passenger leaves the air ship (`EX_GET_OFF_AIRSHIP`).
    ExGetOffAirShip,
    ExGetOffAirship {
        player_id: i32,
        air_ship_id: i32,
        x: i32,
        y: i32,
        z: i32,
    }
);

// Java: gameserver/network/serverpackets/ExMoveToLocationAirShip.java
server_packet!(
    /// The air ship moves to a destination (`EX_MOVE_TO_LOCATION_AIRSHIP`).
    ExMoveToLocationAirShip,
    ExMoveToLocationAirship {
        object_id: i32,
        destination_x: i32,
        destination_y: i32,
        destination_z: i32,
        x: i32,
        y: i32,
        z: i32,
    }
);

// Java: gameserver/network/serverpackets/ExMoveToLocationInAirShip.java
server_packet!(
    /// A passenger walks to a place inside the air ship (`EX_MOVE_TO_LOCATION_IN_AIRSHIP`).
    ExMoveToLocationInAirShip,
    ExMoveToLocationInAirship {
        object_id: i32,
        air_ship_id: i32,
        destination_x: i32,
        destination_y: i32,
        destination_z: i32,
        heading: i32,
    }
);
