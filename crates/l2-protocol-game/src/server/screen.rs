//! The screen packets of the admin effect commands: the earthquake (`//earthquake`), the sun
//! rise and set and the red sky (`//atmosphere`), and the special camera of `//cam`, `//camex`
//! and `//cam3`.
//!
//! Java: `gameserver/network/serverpackets/{Earthquake,SunRise,SunSet,ExRedSky,SpecialCamera}.java`.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_opcode;
use crate::generated::opcodes::ServerOpcode;

// Java: gameserver/network/serverpackets/Earthquake.java
/// Shakes the screen of the players around `(x, y, z)` (`EARTHQUAKE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Earthquake {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub intensity: i32,
    pub duration: i32,
}

impl Earthquake {
    pub const OPCODE: ServerOpcode = ServerOpcode::Earthquake;
}

impl Encode for Earthquake {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.x)
            .i32(self.y)
            .i32(self.z)
            .i32(self.intensity)
            .i32(self.duration)
            // Java: `writeInt(0); // Unknown`.
            .i32(0);
    }
}

impl Decode for Earthquake {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let packet = Self {
            x: r.i32()?,
            y: r.i32()?,
            z: r.i32()?,
            intensity: r.i32()?,
            duration: r.i32()?,
        };
        // The unknown int is always 0 on the wire; it is read and dropped.
        r.i32()?;
        Ok(packet)
    }
}

// Java: gameserver/network/serverpackets/SunRise.java
/// The day comes (`SUNRISE`). A header only packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SunRise;

impl SunRise {
    pub const OPCODE: ServerOpcode = ServerOpcode::Sunrise;
}

impl Encode for SunRise {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
    }
}

impl Decode for SunRise {
    fn decode(_r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self)
    }
}

// Java: gameserver/network/serverpackets/SunSet.java
/// The night comes (`SUNSET`). A header only packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SunSet;

impl SunSet {
    pub const OPCODE: ServerOpcode = ServerOpcode::Sunset;
}

impl Encode for SunSet {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
    }
}

impl Decode for SunSet {
    fn decode(_r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self)
    }
}

// Java: gameserver/network/serverpackets/ExRedSky.java
/// The red sky for `duration` seconds (`EX_RED_SKY`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExRedSky {
    pub duration: i32,
}

impl ExRedSky {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExRedSky;
}

impl Encode for ExRedSky {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.duration);
    }
}

impl Decode for ExRedSky {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { duration: r.i32()? })
    }
}

// Java: gameserver/network/serverpackets/SpecialCamera.java
/// The camera moves around the creature `id` on the receiving client (`SPECIAL_CAMERA`). The
/// fields are the wire fields; [`SpecialCamera::from_range_duration`] and the other constructors
/// give Java's three constructors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpecialCamera {
    /// `_id`: the object the camera turns around.
    pub id: i32,
    /// `_force`.
    pub force: i32,
    /// `_angle1`.
    pub angle1: i32,
    /// `_angle2`.
    pub angle2: i32,
    /// `_time`.
    pub time: i32,
    /// `_duration`.
    pub duration: i32,
    /// `_relYaw`.
    pub rel_yaw: i32,
    /// `_relPitch`.
    pub rel_pitch: i32,
    /// `_isWide`.
    pub is_wide: i32,
    /// `_relAngle`.
    pub rel_angle: i32,
    /// `_unk`: the post-C4 parameter (0 except for `cam3`).
    pub unk: i32,
}

impl SpecialCamera {
    pub const OPCODE: ServerOpcode = ServerOpcode::SpecialCamera;

    /// Java `SpecialCamera(creature, force, angle1, angle2, time, range, duration, relYaw,
    /// relPitch, isWide, relAngle, unk)`: the constructor writes `duration` into the packet.
    // NOTE(java-quirk): Java's 11-argument constructor (`//cam`) passes its `duration` and
    // `range` to the 12-argument one in the swapped order, so the packet's duration field is the
    // `range` argument; `range` itself is never written. Java's `SpecialCamera(creature, force,
    // ..., relAngle)` is [`SpecialCamera::from_range_duration`].
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: i32,
        force: i32,
        angle1: i32,
        angle2: i32,
        time: i32,
        duration: i32,
        rel_yaw: i32,
        rel_pitch: i32,
        is_wide: i32,
        rel_angle: i32,
        unk: i32,
    ) -> Self {
        Self {
            id,
            force,
            angle1,
            angle2,
            time,
            duration,
            rel_yaw,
            rel_pitch,
            is_wide,
            rel_angle,
            unk,
        }
    }

    /// Java `SpecialCamera(creature, force, angle1, angle2, time, range, duration, relYaw,
    /// relPitch, isWide, relAngle)` (`//cam`): the packet carries `range` where Java passes it on.
    #[allow(clippy::too_many_arguments)]
    pub fn from_range_duration(
        id: i32,
        force: i32,
        angle1: i32,
        angle2: i32,
        time: i32,
        range: i32,
        _duration: i32,
        rel_yaw: i32,
        rel_pitch: i32,
        is_wide: i32,
        rel_angle: i32,
    ) -> Self {
        // Java: this(creature, force, angle1, angle2, time, duration, range, ...): the 12-argument
        // constructor's `range` gets `duration` and its `duration` gets `range`.
        Self::new(
            id, force, angle1, angle2, time, range, rel_yaw, rel_pitch, is_wide, rel_angle, 0,
        )
    }

    /// Java `SpecialCamera(creature, talker, force, angle1, angle2, time, duration, relYaw,
    /// relPitch, isWide, relAngle)` (`//camex`): the packet's duration is always 0.
    // NOTE(java-quirk): the Ex constructor passes `0` as the 12-argument constructor's
    // `duration`, so `//camex` never sends its duration argument.
    #[allow(clippy::too_many_arguments)]
    pub fn ex(
        id: i32,
        force: i32,
        angle1: i32,
        angle2: i32,
        time: i32,
        rel_yaw: i32,
        rel_pitch: i32,
        is_wide: i32,
        rel_angle: i32,
    ) -> Self {
        Self::new(
            id, force, angle1, angle2, time, 0, rel_yaw, rel_pitch, is_wide, rel_angle, 0,
        )
    }
}

impl Encode for SpecialCamera {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.id)
            .i32(self.force)
            .i32(self.angle1)
            .i32(self.angle2)
            .i32(self.time)
            .i32(self.duration)
            .i32(self.rel_yaw)
            .i32(self.rel_pitch)
            .i32(self.is_wide)
            .i32(self.rel_angle)
            .i32(self.unk);
    }
}

impl Decode for SpecialCamera {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            id: r.i32()?,
            force: r.i32()?,
            angle1: r.i32()?,
            angle2: r.i32()?,
            time: r.i32()?,
            duration: r.i32()?,
            rel_yaw: r.i32()?,
            rel_pitch: r.i32()?,
            is_wide: r.i32()?,
            rel_angle: r.i32()?,
            unk: r.i32()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip<T: Encode + Decode + PartialEq + std::fmt::Debug>(value: &T) -> Vec<u8> {
        let bytes = value.to_bytes();
        // The first byte (or the 0xFE and the u16 of an EX packet) is the opcode.
        let skip = if bytes[0] == 0xFE { 3 } else { 1 };
        let decoded = T::decode(&mut PacketReader::new(&bytes[skip..])).unwrap();
        assert_eq!(&decoded, value);
        bytes
    }

    #[test]
    fn earthquake_round_trips_with_its_unknown_zero() {
        let packet = Earthquake {
            x: -71_338,
            y: 258_271,
            z: -3_104,
            intensity: 4,
            duration: 10,
        };
        let bytes = round_trip(&packet);
        assert_eq!(bytes[0], 0xD3);
        assert_eq!(bytes.len(), 1 + 6 * 4);
        assert_eq!(&bytes[21..25], &[0, 0, 0, 0]);
    }

    #[test]
    fn sun_packets_are_header_only() {
        assert_eq!(round_trip(&SunRise), vec![0x12]);
        assert_eq!(round_trip(&SunSet), vec![0x13]);
    }

    #[test]
    fn red_sky_is_an_ex_packet() {
        let bytes = round_trip(&ExRedSky { duration: 10 });
        assert_eq!(bytes[0], 0xFE);
        assert_eq!(&bytes[1..3], &0x0041u16.to_le_bytes());
        assert_eq!(&bytes[3..], &10i32.to_le_bytes());
    }

    #[test]
    fn special_camera_round_trips_and_keeps_java_order() {
        let cam = SpecialCamera::from_range_duration(7, 1, 2, 3, 4, 55, 66, 7, 8, 9, 10);
        // `range` (55) is where Java's packet has its duration; the duration argument (66) is gone.
        assert_eq!(cam.duration, 55);
        assert_eq!(cam.unk, 0);
        round_trip(&cam);
        let ex = SpecialCamera::ex(7, 1, 2, 3, 4, 5, 6, 7, 8);
        assert_eq!(ex.duration, 0);
        round_trip(&ex);
        let cam3 = SpecialCamera::new(7, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10);
        assert_eq!(cam3.unk, 10);
        round_trip(&cam3);
    }
}
