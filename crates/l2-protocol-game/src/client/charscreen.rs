//! Client packets of the character selection screen.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::{write_ex_opcode, write_opcode};
use crate::generated::opcodes::{ClientOpcode, ExClientOpcode};

// Java: gameserver/network/clientpackets/NewCharacter.java
/// "Create character" button. No fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewCharacter;

impl NewCharacter {
    pub const OPCODE: ClientOpcode = ClientOpcode::NewCharacter;
}

impl Encode for NewCharacter {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
    }
}

impl Decode for NewCharacter {
    fn decode(_r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self)
    }
}

// Java: gameserver/network/clientpackets/CharacterCreate.java
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterCreate {
    pub name: String,
    /// Java reads and ignores it (the class decides the race).
    pub race: i32,
    pub is_female: bool,
    pub class_id: i32,
    // The six base stats: Java reads and ignores them. Wire order is INT, STR, CON, MEN, DEX, WIT.
    pub int: i32,
    pub str: i32,
    pub con: i32,
    pub men: i32,
    pub dex: i32,
    pub wit: i32,
    // NOTE(java-quirk): Java reads these with `(byte) readInt()`, so only the low 8 bits count
    // and the range checks (hair style 0..=4/6, colour 0..=3, face 0..=2) see a signed byte.
    pub hair_style: i8,
    pub hair_color: i8,
    pub face: i8,
}

impl CharacterCreate {
    pub const OPCODE: ClientOpcode = ClientOpcode::CharacterCreate;
}

impl Encode for CharacterCreate {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.string(&self.name)
            .i32(self.race)
            .i32(i32::from(self.is_female))
            .i32(self.class_id)
            .i32(self.int)
            .i32(self.str)
            .i32(self.con)
            .i32(self.men)
            .i32(self.dex)
            .i32(self.wit)
            .i32(i32::from(self.hair_style))
            .i32(i32::from(self.hair_color))
            .i32(i32::from(self.face));
    }
}

impl Decode for CharacterCreate {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            name: r.string(),
            race: r.i32()?,
            // Java: `readInt() != 0`.
            is_female: r.i32()? != 0,
            class_id: r.i32()?,
            int: r.i32()?,
            str: r.i32()?,
            con: r.i32()?,
            men: r.i32()?,
            dex: r.i32()?,
            wit: r.i32()?,
            hair_style: r.i32()? as i8,
            hair_color: r.i32()? as i8,
            face: r.i32()? as i8,
        })
    }
}

// Java: gameserver/network/clientpackets/CharacterDelete.java
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharacterDelete {
    /// Index in the character list the server last sent.
    pub char_slot: i32,
}

impl CharacterDelete {
    pub const OPCODE: ClientOpcode = ClientOpcode::CharacterDelete;
}

impl Encode for CharacterDelete {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.char_slot);
    }
}

impl Decode for CharacterDelete {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { char_slot: r.i32()? })
    }
}

// Java: gameserver/network/clientpackets/CharacterRestore.java
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharacterRestore {
    pub char_slot: i32,
}

impl CharacterRestore {
    pub const OPCODE: ClientOpcode = ClientOpcode::CharacterRestore;
}

impl Encode for CharacterRestore {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.char_slot);
    }
}

impl Decode for CharacterRestore {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { char_slot: r.i32()? })
    }
}

// Java: gameserver/network/clientpackets/CharacterSelect.java
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharacterSelect {
    pub char_slot: i32,
    // "New in C4" values that Java reads and never uses. Kept so the packet encodes back to the
    // same bytes.
    pub unk1: i16,
    pub unk2: i32,
    pub unk3: i32,
    pub unk4: i32,
}

impl CharacterSelect {
    pub const OPCODE: ClientOpcode = ClientOpcode::CharacterSelect;
}

impl Encode for CharacterSelect {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.char_slot)
            .i16(self.unk1)
            .i32(self.unk2)
            .i32(self.unk3)
            .i32(self.unk4);
    }
}

impl Decode for CharacterSelect {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            char_slot: r.i32()?,
            unk1: r.i16()?,
            unk2: r.i32()?,
            unk3: r.i32()?,
            unk4: r.i32()?,
        })
    }
}

// Java: gameserver/network/clientpackets/RequestGotoLobby.java
/// Ex packet `0xD0 0x0036`: back to the character list. No fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestGotoLobby;

impl RequestGotoLobby {
    pub const EX_OPCODE: ExClientOpcode = ExClientOpcode::RequestGotoLobby;
}

impl Encode for RequestGotoLobby {
    fn encode(&self, w: &mut PacketWriter) {
        write_ex_opcode(w, Self::EX_OPCODE);
    }
}

impl Decode for RequestGotoLobby {
    fn decode(_r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// decode(encode(x)) == x, given the number of opcode bytes at the front. Returns the bytes.
    fn round_trip<P: Encode + Decode + PartialEq + std::fmt::Debug>(packet: P, opcode_len: usize) -> Vec<u8> {
        let bytes = packet.to_bytes();
        let decoded = P::decode(&mut PacketReader::new(&bytes[opcode_len..])).unwrap();
        assert_eq!(decoded, packet);
        bytes
    }

    #[test]
    fn empty_packets() {
        assert_eq!(round_trip(NewCharacter, 1), [0x13]);
        // Ex packets: 0xD0 and the u16 ex id.
        assert_eq!(round_trip(RequestGotoLobby, 3), [0xD0, 0x36, 0x00]);
    }

    #[test]
    fn slot_packets() {
        assert_eq!(
            round_trip(CharacterDelete { char_slot: 2 }, 1),
            [0x0D, 2, 0, 0, 0]
        );
        assert_eq!(
            round_trip(CharacterRestore { char_slot: -1 }, 1),
            [0x7B, 0xFF, 0xFF, 0xFF, 0xFF]
        );
        assert!(CharacterDelete::decode(&mut PacketReader::new(&[1, 0, 0])).is_err());
    }

    #[test]
    fn character_select_layout() {
        let packet = CharacterSelect {
            char_slot: 1,
            unk1: 0x0102,
            unk2: 0,
            unk3: 0,
            unk4: 0,
        };
        let bytes = round_trip(packet, 1);
        // opcode, int slot, short unk1, three ints: 1 + 4 + 2 + 12.
        assert_eq!(bytes.len(), 19);
        assert_eq!(bytes[..7], [0x12, 1, 0, 0, 0, 0x02, 0x01]);
        // Java needs all of it.
        assert!(CharacterSelect::decode(&mut PacketReader::new(&bytes[1..18])).is_err());
    }

    #[test]
    fn character_create_layout() {
        let packet = CharacterCreate {
            name: "Ab".into(),
            race: 1,
            is_female: true,
            class_id: 18,
            int: 21,
            str: 41,
            con: 43,
            men: 25,
            dex: 30,
            wit: 11,
            hair_style: 5,
            hair_color: 2,
            face: 1,
        };
        let bytes = round_trip(packet, 1);
        let mut expected = vec![0x0C, 0x41, 0, 0x62, 0, 0, 0];
        for value in [1_i32, 1, 18, 21, 41, 43, 25, 30, 11, 5, 2, 1] {
            expected.extend(value.to_le_bytes());
        }
        assert_eq!(bytes, expected);
    }

    #[test]
    fn character_create_reads_appearance_as_signed_bytes() {
        let mut body = vec![0, 0]; // empty name
        for value in [0_i32, 0, 0, 0, 0, 0, 0, 0, 0, 0x105, 0xFF, 0x80] {
            body.extend(value.to_le_bytes());
        }
        let decoded = CharacterCreate::decode(&mut PacketReader::new(&body)).unwrap();
        assert_eq!(
            (decoded.hair_style, decoded.hair_color, decoded.face),
            (5, -1, -128)
        );
        assert!(!decoded.is_female);
        assert!(CharacterCreate::decode(&mut PacketReader::new(&body[..body.len() - 1])).is_err());
    }
}
