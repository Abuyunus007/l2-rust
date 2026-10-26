//! Server packets the GM tools and the petition system use: `Snoop`, `PetitionVotePacket` and
//! the free-form `AdminForgePacket`. (`ShowBoard` and `ExServerPrimitive`, which the admin pages
//! also use, are in [`super::debug_draw`].)

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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn snoop_with_an_unknown_chat_type_does_not_decode() {
        let mut w = PacketWriter::new();
        w.i32(1).string("a").i32(0).i32(99).string("b").string("c");
        assert!(Snoop::decode(&mut PacketReader::new(w.as_bytes())).is_err());
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
        packet.add_part(ForgePart::Double(1.0));
        assert_eq!(
            packet.to_bytes(),
            [
                0x49, 0xFE, 0xFF, 0xFF, 0xFF, 0x02, 0x01, b'H', 0, b'i', 0, 0, 0, 7, 8, 1, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0xF0, 0x3F
            ]
        );
    }
}
