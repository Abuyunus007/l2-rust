//! Server packets the status effects send: a creature turning around (`Bluff`), a charge
//! (`EnemyCharge`) and the soul emitter (`SoulEating`). Values only, like the other packet modules.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_opcode;
use crate::generated::opcodes::ServerOpcode;

// Java: gameserver/network/serverpackets/StartRotation.java
/// A creature starts to turn (`START_ROTATING`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartRotation {
    pub object_id: i32,
    pub degree: i32,
    pub side: i32,
    pub speed: i32,
}

impl StartRotation {
    pub const OPCODE: ServerOpcode = ServerOpcode::StartRotating;
}

impl Encode for StartRotation {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.object_id)
            .i32(self.degree)
            .i32(self.side)
            .i32(self.speed);
    }
}

impl Decode for StartRotation {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            object_id: r.i32()?,
            degree: r.i32()?,
            side: r.i32()?,
            speed: r.i32()?,
        })
    }
}

// Java: gameserver/network/serverpackets/StopRotation.java
/// A creature stops turning at `degree` (`FINISH_ROTATING`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StopRotation {
    pub object_id: i32,
    pub degree: i32,
    pub speed: i32,
}

impl StopRotation {
    pub const OPCODE: ServerOpcode = ServerOpcode::FinishRotating;
}

impl Encode for StopRotation {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        // Java ends with `writeByte(0); // ?`.
        w.i32(self.object_id).i32(self.degree).i32(self.speed).u8(0);
    }
}

impl Decode for StopRotation {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let packet = Self {
            object_id: r.i32()?,
            degree: r.i32()?,
            speed: r.i32()?,
        };
        r.skip(1)?;
        Ok(packet)
    }
}

// Java: gameserver/network/serverpackets/FlyToLocation.java
/// A creature flies to a point (`FLY_TO_LOCATION`): a charge, a throw or a blink.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlyToLocation {
    pub object_id: i32,
    pub dest_x: i32,
    pub dest_y: i32,
    pub dest_z: i32,
    pub org_x: i32,
    pub org_y: i32,
    pub org_z: i32,
    /// Java `FlyType.ordinal()`: `THROW_UP` 0, `THROW_HORIZONTAL` 1, `DUMMY` 2, `CHARGE` 3.
    pub fly_type: i32,
}

impl FlyToLocation {
    pub const OPCODE: ServerOpcode = ServerOpcode::FlyToLocation;
}

impl Encode for FlyToLocation {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.object_id)
            .i32(self.dest_x)
            .i32(self.dest_y)
            .i32(self.dest_z)
            .i32(self.org_x)
            .i32(self.org_y)
            .i32(self.org_z)
            .i32(self.fly_type);
    }
}

impl Decode for FlyToLocation {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            object_id: r.i32()?,
            dest_x: r.i32()?,
            dest_y: r.i32()?,
            dest_z: r.i32()?,
            org_x: r.i32()?,
            org_y: r.i32()?,
            org_z: r.i32()?,
            fly_type: r.i32()?,
        })
    }
}

// Java: gameserver/network/serverpackets/ExSpawnEmitter.java
/// The soul of a dead NPC flows to a player (`EX_SPAWN_EMITTER`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExSpawnEmitter {
    pub player_object_id: i32,
    pub npc_object_id: i32,
}

impl ExSpawnEmitter {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExSpawnEmitter;
}

impl Encode for ExSpawnEmitter {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        // Java writes the NPC first; the last int is unknown ("?").
        w.i32(self.npc_object_id).i32(self.player_object_id).i32(0);
    }
}

impl Decode for ExSpawnEmitter {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let npc_object_id = r.i32()?;
        let player_object_id = r.i32()?;
        r.skip(4)?;
        Ok(Self {
            player_object_id,
            npc_object_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body<T: Encode>(packet: &T) -> (Vec<u8>, usize) {
        let bytes = packet.to_bytes();
        let skip = if bytes.first() == Some(&0xFE) { 3 } else { 1 };
        (bytes, skip)
    }

    fn round_trip<T: Encode + Decode + PartialEq + std::fmt::Debug>(packet: T) {
        let (bytes, skip) = body(&packet);
        let mut r = PacketReader::new(&bytes[skip..]);
        assert_eq!(T::decode(&mut r).unwrap(), packet);
    }

    #[test]
    fn start_rotation_layout() {
        let p = StartRotation {
            object_id: 7,
            degree: 100,
            side: 1,
            speed: 65535,
        };
        let bytes = p.to_bytes();
        assert_eq!(bytes[0], 0x7A);
        assert_eq!(bytes.len(), 1 + 16);
        round_trip(p);
    }

    #[test]
    fn stop_rotation_ends_with_a_zero_byte() {
        let p = StopRotation {
            object_id: 7,
            degree: 200,
            speed: 65535,
        };
        let bytes = p.to_bytes();
        assert_eq!(bytes[0], 0x61);
        assert_eq!(bytes.len(), 1 + 12 + 1);
        assert_eq!(bytes[bytes.len() - 1], 0);
        round_trip(p);
    }

    #[test]
    fn fly_to_location_layout() {
        let p = FlyToLocation {
            object_id: 1,
            dest_x: 10,
            dest_y: 20,
            dest_z: 30,
            org_x: 1,
            org_y: 2,
            org_z: 3,
            fly_type: 3,
        };
        let bytes = p.to_bytes();
        assert_eq!(bytes[0], 0xD4);
        assert_eq!(bytes.len(), 1 + 32);
        round_trip(p);
    }

    #[test]
    fn spawn_emitter_writes_the_npc_first() {
        let p = ExSpawnEmitter {
            player_object_id: 2,
            npc_object_id: 9,
        };
        let bytes = p.to_bytes();
        assert_eq!(bytes[..3], [0xFE, 0x5D, 0x00]);
        assert_eq!(bytes[3..7], 9_i32.to_le_bytes());
        assert_eq!(bytes[7..11], 2_i32.to_le_bytes());
        round_trip(p);
    }
}
