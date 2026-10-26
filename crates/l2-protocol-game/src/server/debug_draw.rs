//! The GM drawing packets: `ExServerPrimitive` (points and lines drawn in the world) and
//! `ShowBoard` (the community board window the admin pages use for wide layouts).
//!
//! Values only: the world passes in the coordinates and the colours.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_opcode;
use crate::generated::opcodes::ServerOpcode;

// Java: java.awt.Color constants (`getRGB()`, alpha in the top byte).
/// A colour as `java.awt.Color.getRGB()` gives it (`0xAARRGGBB`); the packet sends R, G and B.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrimitiveColor(pub i32);

impl PrimitiveColor {
    /// `Color.BLACK`.
    pub const BLACK: Self = Self(0xFF00_0000_u32 as i32);
    /// `Color.RED`.
    pub const RED: Self = Self(0xFFFF_0000_u32 as i32);
    /// `Color.GREEN`.
    pub const GREEN: Self = Self(0xFF00_FF00_u32 as i32);
    /// `Color.ORANGE` (255, 200, 0).
    pub const ORANGE: Self = Self(0xFFFF_C800_u32 as i32);

    fn red(self) -> i32 {
        (self.0 >> 16) & 0xFF
    }

    fn green(self) -> i32 {
        (self.0 >> 8) & 0xFF
    }

    fn blue(self) -> i32 {
        self.0 & 0xFF
    }
}

// Java: gameserver/network/serverpackets/ExServerPrimitive.java
/// One point (`end` is `None`) or line of an [`ExServerPrimitive`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Primitive {
    /// The name drawn over the point or the middle of the line. The client shows it even when you
    /// do not look at it (Java: "Names in points and lines are bugged").
    pub name: String,
    pub color: PrimitiveColor,
    pub name_colored: bool,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// The end of a line.
    pub end: Option<(i32, i32, i32)>,
}

/// Draws points and lines on the client (`EX_SERVER_PRIMITIVE`). A later packet with the same
/// `name` replaces the drawing of the earlier one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExServerPrimitive {
    pub name: String,
    /// The middle of the drawing area.
    pub x: i32,
    pub y: i32,
    pub z: i32,
    points: Vec<Primitive>,
    lines: Vec<Primitive>,
}

impl ExServerPrimitive {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExServerPrimitive;

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

    /// Java `addPoint(name, color, isNameColored, x, y, z)`.
    pub fn add_named_point(
        &mut self,
        name: &str,
        color: PrimitiveColor,
        name_colored: bool,
        x: i32,
        y: i32,
        z: i32,
    ) {
        self.points.push(Primitive {
            name: name.to_owned(),
            color,
            name_colored,
            x,
            y,
            z,
            end: None,
        });
    }

    /// Java `addPoint(color, x, y, z)`.
    pub fn add_point(&mut self, color: PrimitiveColor, x: i32, y: i32, z: i32) {
        self.add_named_point("", color, false, x, y, z);
    }

    /// Java `addLine(color, x, y, z, x2, y2, z2)`.
    pub fn add_line(&mut self, color: PrimitiveColor, from: (i32, i32, i32), to: (i32, i32, i32)) {
        self.lines.push(Primitive {
            name: String::new(),
            color,
            name_colored: false,
            x: from.0,
            y: from.1,
            z: from.2,
            end: Some(to),
        });
    }

    pub fn points(&self) -> &[Primitive] {
        &self.points
    }

    pub fn lines(&self) -> &[Primitive] {
        &self.lines
    }
}

fn write_primitive(w: &mut PacketWriter, kind: u8, p: &Primitive) {
    w.u8(kind).string(&p.name);
    w.i32(p.color.red())
        .i32(p.color.green())
        .i32(p.color.blue())
        .i32(i32::from(p.name_colored))
        .i32(p.x)
        .i32(p.y)
        .i32(p.z);
    if let Some((x2, y2, z2)) = p.end {
        w.i32(x2).i32(y2).i32(z2);
    }
}

impl Encode for ExServerPrimitive {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.string(&self.name).i32(self.x).i32(self.y).i32(self.z);
        // "Has to do something with display range and angle."
        w.i32(65535).i32(65535);
        w.i32((self.points.len() + self.lines.len()) as i32);
        for point in &self.points {
            write_primitive(w, 1, point);
        }
        for line in &self.lines {
            write_primitive(w, 2, line);
        }
    }
}

impl Decode for ExServerPrimitive {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let name = r.string();
        let (x, y, z) = (r.i32()?, r.i32()?, r.i32()?);
        r.i32()?;
        r.i32()?;
        let count = super::read_count(r, "count")?;
        let mut packet = Self::new(&name, x, y, z);
        for _ in 0..count {
            let kind = r.u8()?;
            let entry_name = r.string();
            let (red, green, blue) = (r.i32()?, r.i32()?, r.i32()?);
            let name_colored = r.i32()? != 0;
            let (ex, ey, ez) = (r.i32()?, r.i32()?, r.i32()?);
            // `Color.getRGB()` has the alpha byte set; the wire has no alpha.
            let color = PrimitiveColor(
                (0xFF00_0000_u32
                    | ((red as u32 & 0xFF) << 16)
                    | ((green as u32 & 0xFF) << 8)
                    | (blue as u32 & 0xFF)) as i32,
            );
            match kind {
                1 => packet.add_named_point(&entry_name, color, name_colored, ex, ey, ez),
                2 => {
                    let end = (r.i32()?, r.i32()?, r.i32()?);
                    packet.lines.push(Primitive {
                        name: entry_name,
                        color,
                        name_colored,
                        x: ex,
                        y: ey,
                        z: ez,
                        end: Some(end),
                    });
                }
                other => return Err(PacketError::invalid("kind", format!("primitive type {other}"))),
            }
        }
        Ok(packet)
    }
}

// Java: gameserver/network/serverpackets/ShowBoard.java
/// Opens, fills or closes the community board window (`SHOW_BOARD`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShowBoard {
    /// Java `_showBoard`: 1 shows the board, 0 hides it.
    pub show: bool,
    /// Java `_content`: the board id, a `\u{8}` and the page (or the arguments).
    pub content: String,
}

/// The eight fixed bypasses the board packet carries (top, favorites, region, clan, memo, mail,
/// friends, add favorite).
const BOARD_BYPASSES: [&str; 8] = [
    "bypass _bbshome",
    "bypass _bbsgetfav",
    "bypass _bbsloc",
    "bypass _bbsclan",
    "bypass _bbsmemo",
    "bypass _bbsmail",
    "bypass _bbsfriends",
    "bypass bbs_add_fav",
];

impl ShowBoard {
    pub const OPCODE: ServerOpcode = ServerOpcode::ShowBoard;

    /// Java `new ShowBoard(htmlCode, id)`.
    ///
    /// NOTE(java-quirk): Java concatenates `id + "\b" + htmlCode`, so a `null` page is the text
    /// "null" (the part packets "102" and "103" of a short page carry it).
    pub fn page(html: Option<&str>, id: &str) -> Self {
        Self {
            show: true,
            content: format!("{id}\u{8}{}", html.unwrap_or("null")),
        }
    }

    /// Java `new ShowBoard(List<String>)`: the "1002" arguments packet.
    pub fn arguments(args: &[String]) -> Self {
        let mut content = String::from("1002\u{8}");
        for arg in args {
            content.push_str(arg);
            content.push('\u{8}');
        }
        Self { show: true, content }
    }

    /// Java `new ShowBoard()`: hides the board.
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
        for bypass in BOARD_BYPASSES {
            w.string(bypass);
        }
        w.string(&self.content);
    }
}

impl Decode for ShowBoard {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let show = r.u8()? != 0;
        for _ in BOARD_BYPASSES {
            r.string();
        }
        Ok(Self {
            show,
            content: r.string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes(packet: &impl Encode) -> Vec<u8> {
        let mut w = PacketWriter::new();
        packet.encode(&mut w);
        w.into_bytes()
    }

    #[test]
    fn primitive_layout_and_round_trip() {
        let mut p = ExServerPrimitive::new("DebugRegion", 10, -20, -16000);
        p.add_named_point("0", PrimitiveColor::RED, true, 1, 2, 3);
        p.add_line(PrimitiveColor::ORANGE, (1, 2, 3), (4, 5, 6));
        let b = bytes(&p);
        // 0xFE, ex id 0x0011.
        assert_eq!(&b[..3], &[0xFE, 0x11, 0x00]);
        // The name is UTF-16LE with a terminator, then x, y, z, 65535 twice and the count.
        let name_len = ("DebugRegion".len() + 1) * 2;
        let rest = &b[3 + name_len..];
        assert_eq!(&rest[0..4], &10i32.to_le_bytes());
        assert_eq!(&rest[4..8], &(-20i32).to_le_bytes());
        assert_eq!(&rest[8..12], &(-16000i32).to_le_bytes());
        assert_eq!(&rest[12..16], &65535i32.to_le_bytes());
        assert_eq!(&rest[16..20], &65535i32.to_le_bytes());
        assert_eq!(&rest[20..24], &2i32.to_le_bytes());
        // The point comes first: type 1, name "0", R G B 255 0 0, name colored 1.
        assert_eq!(rest[24], 1);
        assert_eq!(&rest[25..29], &[b'0', 0, 0, 0]);
        assert_eq!(&rest[29..33], &255i32.to_le_bytes());
        assert_eq!(&rest[33..37], &0i32.to_le_bytes());
        assert_eq!(&rest[37..41], &0i32.to_le_bytes());
        assert_eq!(&rest[41..45], &1i32.to_le_bytes());
        let mut r = PacketReader::new(&b[3..]);
        let back = ExServerPrimitive::decode(&mut r).unwrap();
        assert_eq!(back, p);
        assert_eq!(back.lines()[0].color, PrimitiveColor::ORANGE);
    }

    #[test]
    fn primitive_without_entries() {
        let p = ExServerPrimitive::new("x", 1, 2, 3);
        let b = bytes(&p);
        let mut r = PacketReader::new(&b[3..]);
        assert_eq!(ExServerPrimitive::decode(&mut r).unwrap(), p);
    }

    #[test]
    fn colors_are_java_rgb() {
        assert_eq!(PrimitiveColor::GREEN.green(), 255);
        assert_eq!(PrimitiveColor::ORANGE.green(), 200);
        assert_eq!(PrimitiveColor::BLACK.red(), 0);
    }

    #[test]
    fn show_board_round_trip_and_null_page() {
        let page = ShowBoard::page(Some("<html/>"), "101");
        assert_eq!(page.content, "101\u{8}<html/>");
        let b = bytes(&page);
        assert_eq!(b[0], 0x7B);
        assert_eq!(b[1], 1);
        let mut r = PacketReader::new(&b[1..]);
        assert_eq!(ShowBoard::decode(&mut r).unwrap(), page);
        // Java's string concatenation of a null page.
        assert_eq!(ShowBoard::page(None, "102").content, "102\u{8}null");
        let hidden = ShowBoard::hide();
        let b = bytes(&hidden);
        let mut r = PacketReader::new(&b[1..]);
        assert_eq!(ShowBoard::decode(&mut r).unwrap(), hidden);
        assert_eq!(
            ShowBoard::arguments(&["a".into(), "b".into()]).content,
            "1002\u{8}a\u{8}b\u{8}"
        );
    }
}
