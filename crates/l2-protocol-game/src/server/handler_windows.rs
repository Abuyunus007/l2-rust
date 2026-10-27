//! Header-only packets the NPC bypass handlers send: `TutorialCloseHtml` and `ExShowQuestInfo`.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_opcode;
use crate::generated::opcodes::ServerOpcode;

// Java: gameserver/network/serverpackets/TutorialCloseHtml.java
/// Closes the tutorial window (`TUTORIAL_CLOSE_HTML`, 0xA9). No fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TutorialCloseHtml;

impl TutorialCloseHtml {
    pub const OPCODE: ServerOpcode = ServerOpcode::TutorialCloseHtml;
}

impl Encode for TutorialCloseHtml {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
    }
}

impl Decode for TutorialCloseHtml {
    fn decode(_r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self)
    }
}

// Java: gameserver/network/serverpackets/ExShowQuestInfo.java
/// Opens the quest list window (`EX_SHOW_QUEST_INFO`, 0xFE 0x0020). No fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExShowQuestInfo;

impl ExShowQuestInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExShowQuestInfo;
}

impl Encode for ExShowQuestInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
    }
}

impl Decode for ExShowQuestInfo {
    fn decode(_r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self)
    }
}

// Java: gameserver/network/serverpackets/ShowXMasSeal.java
/// Shows the Christmas seal window of an item (`SHOW_XMAS_SEAL`, 0xF8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShowXMasSeal {
    /// Java `_item`: the item template id.
    pub item_id: i32,
}

impl ShowXMasSeal {
    pub const OPCODE: ServerOpcode = ServerOpcode::ShowXmasSeal;
}

impl Encode for ShowXMasSeal {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.item_id);
    }
}

impl Decode for ShowXMasSeal {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { item_id: r.i32()? })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tutorial_close_is_one_byte() {
        assert_eq!(TutorialCloseHtml.to_bytes(), vec![0xA9]);
        assert_eq!(
            TutorialCloseHtml::decode(&mut PacketReader::new(&[])).unwrap(),
            TutorialCloseHtml
        );
    }

    #[test]
    fn xmas_seal_is_the_item_id() {
        let packet = ShowXMasSeal { item_id: 5555 };
        let bytes = packet.to_bytes();
        assert_eq!(bytes, vec![0xF8, 0xB3, 0x15, 0, 0]);
        assert_eq!(
            ShowXMasSeal::decode(&mut PacketReader::new(&bytes[1..])).unwrap(),
            packet
        );
    }

    #[test]
    fn show_quest_info_is_the_ex_header() {
        assert_eq!(ExShowQuestInfo.to_bytes(), vec![0xFE, 0x20, 0x00]);
        assert_eq!(
            ExShowQuestInfo::decode(&mut PacketReader::new(&[])).unwrap(),
            ExShowQuestInfo
        );
    }
}
