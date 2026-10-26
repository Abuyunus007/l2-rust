//! The debug primitives the GM sees with `//showdoors`: named points and lines drawn by the client.
// Java: gameserver/network/serverpackets/ExServerPrimitive.java
//
// Values only: the admin door view passes in the corners and colours it computed.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_opcode;
use crate::generated::opcodes::ServerOpcode;

/// `Point`: a named point in colour `color` (`0xRRGGBB`, the alpha byte of Java's `getRGB` is
/// not written).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrimitivePoint {
    pub name: String,
    pub color: i32,
    pub name_colored: bool,
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// `Line`: a [`PrimitivePoint`] (its position is the first end) and the second end.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrimitiveLine {
    pub point: PrimitivePoint,
    pub x2: i32,
    pub y2: i32,
    pub z2: i32,
}

/// `ExServerPrimitive`: a set of points and lines drawn under one name. A second packet with the
/// same name replaces the first (the door view uses this to draw and to erase).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExServerPrimitive {
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub points: Vec<PrimitivePoint>,
    pub lines: Vec<PrimitiveLine>,
}

/// Java `writeImpl`: the display range and the angle are written as the constant 65535.
const DISPLAY_RANGE: i32 = 65535;
const TYPE_POINT: u8 = 1;
const TYPE_LINE: u8 = 2;

impl ExServerPrimitive {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExServerPrimitive;

    /// Java `new ExServerPrimitive(name, x, y, z)`.
    pub fn new(name: &str, x: i32, y: i32, z: i32) -> Self {
        Self {
            name: name.to_owned(),
            x,
            y,
            z,
            ..Self::default()
        }
    }
}

fn encode_color(w: &mut PacketWriter, color: i32, name_colored: bool) {
    // Java writes the red, green and blue bytes as ints.
    w.i32((color >> 16) & 0xFF)
        .i32((color >> 8) & 0xFF)
        .i32(color & 0xFF)
        .i32(i32::from(name_colored));
}

impl Encode for ExServerPrimitive {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.string(&self.name)
            .i32(self.x)
            .i32(self.y)
            .i32(self.z)
            .i32(DISPLAY_RANGE)
            .i32(DISPLAY_RANGE);
        let count = self.points.len() + self.lines.len();
        w.i32(i32::try_from(count).unwrap_or(i32::MAX));
        for point in &self.points {
            w.u8(TYPE_POINT).string(&point.name);
            encode_color(w, point.color, point.name_colored);
            w.i32(point.x).i32(point.y).i32(point.z);
        }
        for line in &self.lines {
            w.u8(TYPE_LINE).string(&line.point.name);
            encode_color(w, line.point.color, line.point.name_colored);
            w.i32(line.point.x)
                .i32(line.point.y)
                .i32(line.point.z)
                .i32(line.x2)
                .i32(line.y2)
                .i32(line.z2);
        }
    }
}

impl Decode for ExServerPrimitive {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let name = r.string();
        let (x, y, z) = (r.i32()?, r.i32()?, r.i32()?);
        // The display range and the angle: always 65535.
        let _ = (r.i32()?, r.i32()?);
        let count = r.i32()?;
        let count = usize::try_from(count).map_err(|_| PacketError::Invalid {
            field: "primitive count",
            reason: format!("{count} is negative"),
        })?;
        let mut packet = Self::new(&name, x, y, z);
        for _ in 0..count {
            let kind = r.u8()?;
            let point_name = r.string();
            let color = decode_color(r)?;
            let name_colored = r.i32()? != 0;
            let point = PrimitivePoint {
                name: point_name,
                color,
                name_colored,
                x: r.i32()?,
                y: r.i32()?,
                z: r.i32()?,
            };
            match kind {
                TYPE_POINT => packet.points.push(point),
                TYPE_LINE => packet.lines.push(PrimitiveLine {
                    point,
                    x2: r.i32()?,
                    y2: r.i32()?,
                    z2: r.i32()?,
                }),
                other => {
                    return Err(PacketError::Invalid {
                        field: "primitive type",
                        reason: format!("{other} is neither a point nor a line"),
                    });
                }
            }
        }
        Ok(packet)
    }
}

fn decode_color(r: &mut PacketReader) -> Result<i32, PacketError> {
    let (red, green, blue) = (r.i32()?, r.i32()?, r.i32()?);
    Ok((red << 16) | (green << 8) | blue)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(name: &str, color: i32, x: i32) -> PrimitivePoint {
        PrimitivePoint {
            name: name.to_owned(),
            color,
            name_colored: false,
            x,
            y: 2,
            z: 3,
        }
    }

    #[test]
    fn primitive_layout_and_round_trip() {
        let packet = ExServerPrimitive {
            name: "Door24190001".to_owned(),
            x: -10,
            y: 20,
            z: -16_000,
            points: vec![point("p", 0x00FF_0000, 7)],
            lines: vec![PrimitiveLine {
                point: point("", 0x0000_FF00, 4),
                x2: 5,
                y2: 6,
                z2: 7,
            }],
        };
        let bytes = packet.to_bytes();
        // 0xFE, ex id 0x0011, the name, the centre, the two 65535 values, the count.
        assert_eq!(&bytes[..3], &[0xFE, 0x11, 0x00]);
        let back = ExServerPrimitive::decode(&mut PacketReader::new(&bytes[3..])).unwrap();
        assert_eq!(back, packet);
    }

    #[test]
    fn an_empty_primitive_round_trips() {
        let packet = ExServerPrimitive::new("Door1", 1, 2, 3);
        let bytes = packet.to_bytes();
        let back = ExServerPrimitive::decode(&mut PacketReader::new(&bytes[3..])).unwrap();
        assert_eq!(back, packet);
    }

    #[test]
    fn an_unknown_primitive_type_is_refused() {
        // One complete element of type 9: its name (an empty string), colour, flag and point.
        let mut bytes = ExServerPrimitive::new("", 0, 0, 0).to_bytes();
        let len = bytes.len();
        bytes[len - 4..].copy_from_slice(&1_i32.to_le_bytes());
        bytes.push(9);
        bytes.extend([0, 0]);
        bytes.extend([0_u8; 4 * 3 + 4 + 4 * 3]);
        let err = ExServerPrimitive::decode(&mut PacketReader::new(&bytes[3..])).unwrap_err();
        assert!(matches!(err, PacketError::Invalid { field: "primitive type", .. }));
    }
}
