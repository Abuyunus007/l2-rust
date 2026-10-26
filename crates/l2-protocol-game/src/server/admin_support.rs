//! Server packets the GM tools and the petition system use: `Snoop`, `ShowBoard`,
//! `PetitionVotePacket` and the free-form `AdminForgePacket`.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::ChatType;
use super::write_opcode;
use crate::generated::opcodes::ServerOpcode;

// Java: gameserver/network/serverpackets/Snoop.java
/// A line of the chat of a snooped player (`SNOOP`), sent to the GM who snoops it after the
/// `CreatureSay` itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snoop {
    /// `_convoId`: the object id of the snooped player.
    pub convo_id: i32,
    /// `_name`: the snooped player's name.
    pub name: String,
    /// `_type`: the channel of the line.
    pub chat_type: ChatType,
    /// `_speaker`: who said it.
    pub speaker: String,
    pub message: String,
}

impl Snoop {
    pub const OPCODE: ServerOpcode = ServerOpcode::Snoop;
}

impl Encode for Snoop {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.convo_id).string(&self.name);
        // Java: `buffer.writeInt(0); // ??`
        w.i32(0);
        w.i32(self.chat_type.client_id())
            .string(&self.speaker)
            .string(&self.message);
    }
}

impl Decode for Snoop {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let convo_id = r.i32()?;
        let name = r.string();
        let _unknown = r.i32()?;
        let type_id = r.i32()?;
        let chat_type = ChatType::from_client_id(type_id)
            .ok_or_else(|| PacketError::invalid("chat_type", format!("unknown chat type {type_id}")))?;
        Ok(Self {
            convo_id,
            name,
            chat_type,
            speaker: r.string(),
            message: r.string(),
        })
    }
}

// Java: gameserver/network/serverpackets/ShowBoard.java
/// The community board window (`SHOW_BOARD`): the eight navigation bypasses are fixed, the
/// content is `"<id>\u{8}<html>"` (ids 101 to 103 are the three parts of a page, 1001 a page with
/// a multiedit, 1002 the multiedit values).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShowBoard {
    /// `_showBoard`: 1 shows the board, 0 hides it.
    pub show: bool,
    pub content: String,
}

impl ShowBoard {
    pub const OPCODE: ServerOpcode = ServerOpcode::ShowBoard;

    /// The eight bypasses Java writes between the flag and the content, in order.
    pub const BYPASSES: [&'static str; 8] = [
        "bypass _bbshome",
        "bypass _bbsgetfav",
        "bypass _bbsloc",
        "bypass _bbsclan",
        "bypass _bbsmemo",
        "bypass _bbsmail",
        "bypass _bbsfriends",
        "bypass bbs_add_fav",
    ];

    /// Java `ShowBoard(String htmlCode, String id)`. A `None` page is Java's `null`, which
    /// `id + "\u0008" + null` turns into the text `null`.
    pub fn page(html: Option<&str>, id: &str) -> Self {
        Self {
            show: true,
            content: format!("{id}\u{8}{}", html.unwrap_or("null")),
        }
    }

    /// Java `ShowBoard(List<String> arg)`: id 1002 and the values, each followed by `\u0008`.
    pub fn arguments(values: &[String]) -> Self {
        let mut content = String::from("1002\u{8}");
        for value in values {
            content.push_str(value);
            content.push('\u{8}');
        }
        Self { show: true, content }
    }

    /// Java `ShowBoard()`: hides the board.
    pub fn hide() -> Self {
        Self {
            show: false,
            content: String::new(),
        }
    }
}

impl Encode for ShowBoard {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.u8(u8::from(self.show));
        for bypass in Self::BYPASSES {
            w.string(bypass);
        }
        w.string(&self.content);
    }
}

impl Decode for ShowBoard {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let show = r.u8()? != 0;
        for _ in Self::BYPASSES {
            let _ = r.string();
        }
        Ok(Self {
            show,
            content: r.string(),
        })
    }
}

server_unit_packet!(
    /// The petition evaluation button (`PETITION_VOTE`), sent to the petitioner when the
    /// consultation ends.
    // Java: gameserver/network/serverpackets/PetitionVotePacket.java
    PetitionVotePacket,
    PetitionVote
);

// Java: gameserver/network/serverpackets/AdminForgePacket.java
/// A packet of any layout, built by `//forge_send`. Unlike every other server packet it writes
/// no opcode of its own: the first part is the opcode.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AdminForgePacket {
    pub parts: Vec<ForgePart>,
}

/// One part of an [`AdminForgePacket`], already read from its text value.
#[derive(Debug, Clone, PartialEq)]
pub enum ForgePart {
    /// `c`: one byte.
    Byte(u8),
    /// `h`: two bytes.
    Short(i16),
    /// `d`: four bytes.
    Int(i32),
    /// `q`: eight bytes.
    Long(i64),
    /// `f`: a double.
    Double(f64),
    /// `s`: a string.
    Text(String),
    /// `b`, `x`: raw bytes (Java `BigInteger.toByteArray()`).
    Bytes(Vec<u8>),
}

impl AdminForgePacket {
    /// Java `addPart`.
    pub fn add_part(&mut self, part: ForgePart) {
        self.parts.push(part);
    }
}

impl Encode for AdminForgePacket {
    fn encode(&self, w: &mut PacketWriter) {
        for part in &self.parts {
            match part {
                ForgePart::Byte(v) => {
                    w.u8(*v);
                }
                ForgePart::Short(v) => {
                    w.i16(*v);
                }
                ForgePart::Int(v) => {
                    w.i32(*v);
                }
                ForgePart::Long(v) => {
                    w.i64(*v);
                }
                ForgePart::Double(v) => {
                    w.f64(*v);
                }
                ForgePart::Text(v) => {
                    w.string(v);
                }
                ForgePart::Bytes(v) => {
                    w.bytes(v);
                }
            }
        }
    }
}

// Java: gameserver/network/serverpackets/ExServerPrimitive.java
/// A point drawn on the client (`ExServerPrimitive`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrimitivePoint {
    /// The name shown over the point.
    pub name: String,
    /// `0xRRGGBB` (Java `Color.getRGB()`, whose alpha byte is ignored on the wire).
    pub color: i32,
    pub name_colored: bool,
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// A line drawn on the client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrimitiveLine {
    pub name: String,
    pub color: i32,
    pub name_colored: bool,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub x2: i32,
    pub y2: i32,
    pub z2: i32,
}

/// Points and lines the client draws (`EX_SERVER_PRIMITIVE`). A second packet with the same
/// `name` replaces the first one's drawing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExServerPrimitive {
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub points: Vec<PrimitivePoint>,
    pub lines: Vec<PrimitiveLine>,
}

impl ExServerPrimitive {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExServerPrimitive;

    /// Java `Color.GREEN.getRGB()`.
    pub const GREEN: i32 = 0xFF00_FF00_u32 as i32;
    /// Java `Color.BLUE.getRGB()`.
    pub const BLUE: i32 = 0xFF00_00FF_u32 as i32;
    /// Java `Color.BLACK.getRGB()`.
    pub const BLACK: i32 = 0xFF00_0000_u32 as i32;

    /// Java `new ExServerPrimitive(name, x, y, z)`.
    pub fn new(name: &str, x: i32, y: i32, z: i32) -> Self {
        Self {
            name: name.to_owned(),
            x,
            y,
            z,
            points: Vec::new(),
            lines: Vec::new(),
        }
    }

    /// Java `addLine(color, x, y, z, x2, y2, z2)`: no name.
    pub fn add_line(&mut self, color: i32, from: (i32, i32, i32), to: (i32, i32, i32)) {
        self.lines.push(PrimitiveLine {
            name: String::new(),
            color,
            name_colored: false,
            x: from.0,
            y: from.1,
            z: from.2,
            x2: to.0,
            y2: to.1,
            z2: to.2,
        });
    }
}

/// The byte-per-channel colour the packet writes: the three ints R, G, B.
fn write_color(w: &mut PacketWriter, color: i32) {
    w.i32((color >> 16) & 0xFF)
        .i32((color >> 8) & 0xFF)
        .i32(color & 0xFF);
}

fn read_color(r: &mut PacketReader) -> Result<i32, PacketError> {
    let (red, green, blue) = (r.i32()?, r.i32()?, r.i32()?);
    Ok(((red & 0xFF) << 16) | ((green & 0xFF) << 8) | (blue & 0xFF))
}

impl Encode for ExServerPrimitive {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.string(&self.name).i32(self.x).i32(self.y).i32(self.z);
        // "Has to do something with display range and angle."
        w.i32(65535).i32(65535);
        w.i32((self.points.len() + self.lines.len()) as i32);
        for point in &self.points {
            // The type: a point.
            w.u8(1).string(&point.name);
            write_color(w, point.color);
            w.i32(i32::from(point.name_colored))
                .i32(point.x)
                .i32(point.y)
                .i32(point.z);
        }
        for line in &self.lines {
            // The type: a line.
            w.u8(2).string(&line.name);
            write_color(w, line.color);
            w.i32(i32::from(line.name_colored))
                .i32(line.x)
                .i32(line.y)
                .i32(line.z)
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
        let _range = (r.i32()?, r.i32()?);
        let count = super::read_count(r, "primitive count")?;
        let mut points = Vec::new();
        let mut lines = Vec::new();
        for _ in 0..count {
            let kind = r.u8()?;
            let item_name = r.string();
            let color = read_color(r)?;
            let name_colored = r.i32()? != 0;
            let (px, py, pz) = (r.i32()?, r.i32()?, r.i32()?);
            match kind {
                1 => points.push(PrimitivePoint {
                    name: item_name,
                    color,
                    name_colored,
                    x: px,
                    y: py,
                    z: pz,
                }),
                2 => lines.push(PrimitiveLine {
                    name: item_name,
                    color,
                    name_colored,
                    x: px,
                    y: py,
                    z: pz,
                    x2: r.i32()?,
                    y2: r.i32()?,
                    z2: r.i32()?,
                }),
                other => {
                    return Err(PacketError::invalid(
                        "primitive type",
                        format!("unknown type {other}"),
                    ));
                }
            }
        }
        Ok(Self {
            name,
            x,
            y,
            z,
            points,
            lines,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_primitive_round_trips_and_writes_rgb_bytes() {
        let mut packet = ExServerPrimitive::new("DebugMove", 10, 20, -16000);
        packet.add_line(ExServerPrimitive::GREEN, (1, 2, 3), (4, 5, 6));
        packet.points.push(PrimitivePoint {
            name: "p".to_owned(),
            color: 0x0012_3456,
            name_colored: true,
            x: 7,
            y: 8,
            z: 9,
        });
        let bytes = packet.to_bytes();
        let decoded = ExServerPrimitive::decode(&mut PacketReader::new(&bytes[3..])).unwrap();
        // The alpha byte of Java's `Color.GREEN.getRGB()` does not travel.
        assert_eq!(decoded.lines[0].color, 0x0000_FF00);
        assert_eq!(decoded.points, packet.points);
        assert_eq!(decoded.name, "DebugMove");
    }

    #[test]
    fn a_line_has_the_layout_of_java() {
        let le = |v: i32| v.to_le_bytes();
        let mut packet = ExServerPrimitive::new("A", 1, 2, 3);
        packet.add_line(ExServerPrimitive::GREEN, (4, 5, 6), (7, 8, 9));
        let mut expected = vec![0xFE];
        expected.extend_from_slice(&ServerOpcode::ExServerPrimitive.ex_id().unwrap().to_le_bytes());
        expected.extend_from_slice(&[b'A', 0, 0, 0]);
        for v in [1, 2, 3, 65535, 65535, 1] {
            expected.extend_from_slice(&le(v));
        }
        // The type, the empty name, R = 0, G = 255, B = 0, not name coloured, the six coordinates.
        expected.extend_from_slice(&[2, 0, 0]);
        for v in [0, 255, 0, 0, 4, 5, 6, 7, 8, 9] {
            expected.extend_from_slice(&le(v));
        }
        assert_eq!(packet.to_bytes(), expected);
    }

    #[test]
    fn snoop_round_trips() {
        let packet = Snoop {
            convo_id: 268_435_457,
            name: "Target".to_owned(),
            chat_type: ChatType::Whisper,
            speaker: "Friend".to_owned(),
            message: "hi".to_owned(),
        };
        let bytes = packet.to_bytes();
        assert_eq!(bytes[0], Snoop::OPCODE.id());
        assert_eq!(
            Snoop::decode(&mut PacketReader::new(&bytes[1..])).unwrap(),
            packet
        );
        // The unknown int between the name and the type is 0.
        let mut w = PacketWriter::new();
        w.i32(1).string("a").i32(0).i32(2).string("b").string("c");
        assert_eq!(&bytes[..1], &[Snoop::OPCODE.id()]);
        assert_eq!(
            Snoop {
                convo_id: 1,
                name: "a".to_owned(),
                chat_type: ChatType::Whisper,
                speaker: "b".to_owned(),
                message: "c".to_owned(),
            }
            .to_bytes()[1..],
            w.into_bytes()[..]
        );
    }

    #[test]
    fn show_board_writes_the_eight_bypasses_and_the_content() {
        let packet = ShowBoard::page(Some("<html></html>"), "101");
        assert_eq!(packet.content, "101\u{8}<html></html>");
        let bytes = packet.to_bytes();
        assert_eq!(bytes[1], 1);
        assert_eq!(
            ShowBoard::decode(&mut PacketReader::new(&bytes[1..])).unwrap(),
            packet
        );
        // A missing page is the text "null", like Java's string concatenation.
        assert_eq!(ShowBoard::page(None, "102").content, "102\u{8}null");
        // The values form: id 1002 and a terminator after every value.
        let values = ShowBoard::arguments(&["0".to_owned(), "name".to_owned()]);
        assert_eq!(values.content, "1002\u{8}0\u{8}name\u{8}");
        // Hiding writes the flag 0 and an empty content.
        let hide = ShowBoard::hide();
        let bytes = hide.to_bytes();
        assert_eq!(bytes[1], 0);
        assert_eq!(
            ShowBoard::decode(&mut PacketReader::new(&bytes[1..])).unwrap(),
            hide
        );
    }

    #[test]
    fn petition_vote_is_the_opcode_alone() {
        assert_eq!(PetitionVotePacket.to_bytes(), [PetitionVotePacket::OPCODE.id()]);
    }

    #[test]
    fn a_forged_packet_has_no_opcode_of_its_own() {
        let mut packet = AdminForgePacket::default();
        packet.add_part(ForgePart::Byte(0x49));
        packet.add_part(ForgePart::Int(-2));
        packet.add_part(ForgePart::Short(0x0102));
        packet.add_part(ForgePart::Text("Hi".to_owned()));
        packet.add_part(ForgePart::Bytes(vec![7, 8]));
        packet.add_part(ForgePart::Long(1));
        assert_eq!(
            packet.to_bytes(),
            [
                0x49, 0xFE, 0xFF, 0xFF, 0xFF, 0x02, 0x01, b'H', 0, b'i', 0, 0, 0, 7, 8, 1, 0, 0, 0, 0, 0, 0,
                0
            ]
        );
    }
}
