//! Client packets of the boats, the air ships and the vehicles in general.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_ex_opcode;
use crate::generated::opcodes::ExClientOpcode;

// Java: gameserver/network/clientpackets/RequestGetOnVehicle.java
client_packet!(
    /// The player asks to board a boat at a position (`0x53`).
    RequestGetOnVehicle,
    plain RequestGetOnVehicle {
        /// Java `_boatId`: the object id of the boat.
        boat_id: i32,
        /// Java `_pos`: the boarding position.
        x: i32,
        y: i32,
        z: i32,
    }
);

// Java: gameserver/network/clientpackets/RequestGetOffVehicle.java
client_packet!(
    /// The player asks to leave a boat at a position (`0x54`).
    RequestGetOffVehicle,
    plain RequestGetOffVehicle {
        /// Java `_boatId`.
        boat_id: i32,
        x: i32,
        y: i32,
        z: i32,
    }
);

// Java: gameserver/network/clientpackets/RequestMoveToLocationInVehicle.java
client_packet!(
    /// The player clicks a place to walk to inside a boat (`0x75`).
    RequestMoveToLocationInVehicle,
    plain RequestMoveToLocationInVehicle {
        /// Java `_boatId`: the object id of the boat.
        boat_id: i32,
        target_x: i32,
        target_y: i32,
        target_z: i32,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
    }
);

// Java: gameserver/network/clientpackets/CannotMoveAnymoreInVehicle.java
client_packet!(
    /// The client stopped moving inside a boat at a position (`0x76`).
    CannotMoveAnymoreInVehicle,
    plain CannotMoveAnymoreInVehicle {
        /// Java `_boatId`.
        boat_id: i32,
        x: i32,
        y: i32,
        z: i32,
        heading: i32,
    }
);

// Java: gameserver/network/clientpackets/MoveToLocationAirShip.java
/// The air ship control panel commands (`0xD0` 0x0038).
///
/// Java reads `command` and `param1` and the optional `param2`, which only comes when bytes are
/// left in the packet; `param2` is `0` when it is missing. Encoding always writes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MoveToLocationAirShip {
    /// Java `_command`.
    pub command: i32,
    /// Java `_param1`.
    pub param1: i32,
    /// Java `_param2` (`0` when the client did not send it).
    pub param2: i32,
}

impl MoveToLocationAirShip {
    pub const EX_OPCODE: ExClientOpcode = ExClientOpcode::MoveToLocationAirShip;
}

impl Encode for MoveToLocationAirShip {
    fn encode(&self, w: &mut PacketWriter) {
        write_ex_opcode(w, Self::EX_OPCODE);
        w.i32(self.command).i32(self.param1).i32(self.param2);
    }
}

impl Decode for MoveToLocationAirShip {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let command = r.i32()?;
        let param1 = r.i32()?;
        let param2 = if r.remaining() > 0 { r.i32()? } else { 0 };
        Ok(Self {
            command,
            param1,
            param2,
        })
    }
}

// Java: gameserver/network/clientpackets/MoveToLocationInAirShip.java
client_packet!(
    /// The player clicks a place to walk to inside an air ship (`0xD0` 0x0020).
    MoveToLocationInAirShip,
    ex MoveToLocationInAirShip {
        /// Java `_shipId`.
        ship_id: i32,
        target_x: i32,
        target_y: i32,
        target_z: i32,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
    }
);
