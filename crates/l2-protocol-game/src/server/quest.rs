//! Server packets of the script engine (S-API-01): the quest window, the quest mark, the kill
//! log, the radar and the tutorial window. `QuestList` is in `enterworld.rs`,
//! `TutorialCloseHtml` and `ExShowQuestInfo` in `handler_windows.rs`.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::interaction::prepare_html;
use super::write_opcode;
use crate::generated::opcodes::ServerOpcode;

// Java: gameserver/network/serverpackets/NpcQuestHtmlMessage.java
/// The quest dialog (`EX_NPC_QUEST_HTML_MESSAGE`): an NPC page with the quest id, so the client
/// shows the quest title. Build it with [`NpcQuestHtmlMessage::new`] to get Java's `setHtml`
/// rules (the same as `NpcHtmlMessage`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NpcQuestHtmlMessage {
    pub npc_obj_id: i32,
    pub html: String,
    pub quest_id: i32,
}

impl NpcQuestHtmlMessage {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExNpcQuestHtmlMessage;

    /// Java `new NpcQuestHtmlMessage(npcObjId, questId)` + `setHtml(html)`. `None` where Java
    /// throws (a negative object id).
    pub fn new(npc_obj_id: i32, quest_id: i32, html: &str) -> Option<Self> {
        if npc_obj_id < 0 {
            return None;
        }
        Some(Self {
            npc_obj_id,
            html: prepare_html(html),
            quest_id,
        })
    }
}

impl Encode for NpcQuestHtmlMessage {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.npc_obj_id).string(&self.html).i32(self.quest_id);
    }
}

impl Decode for NpcQuestHtmlMessage {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let npc_obj_id = r.i32()?;
        let html = r.string();
        let quest_id = r.i32()?;
        Ok(Self {
            npc_obj_id,
            html,
            quest_id,
        })
    }
}

// Java: gameserver/network/serverpackets/TutorialShowHtml.java
/// The tutorial window (`TUTORIAL_SHOW_HTML`). Build it with [`TutorialShowHtml::new`] to get
/// Java's `setHtml` rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TutorialShowHtml {
    pub html: String,
}

impl TutorialShowHtml {
    pub const OPCODE: ServerOpcode = ServerOpcode::TutorialShowHtml;

    /// Java `new TutorialShowHtml(html)`.
    pub fn new(html: &str) -> Self {
        Self {
            html: prepare_html(html),
        }
    }
}

impl Encode for TutorialShowHtml {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.string(&self.html);
    }
}

impl Decode for TutorialShowHtml {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { html: r.string() })
    }
}

// Java: gameserver/network/serverpackets/TutorialShowQuestionMark.java
server_packet!(
    /// The blinking question mark of the tutorial (`SHOW_TUTORIAL_MARK`).
    TutorialShowQuestionMark,
    ShowTutorialMark {
        /// Java `_markId`.
        mark_id: i32,
    }
);

// Java: gameserver/network/serverpackets/TutorialEnableClientEvent.java
server_packet!(
    /// Asks the client to report a client event (`TUTORIAL_ENABLE_CLIENT_EVENT`).
    TutorialEnableClientEvent,
    TutorialEnableClientEvent {
        /// Java `_eventId`.
        event_id: i32,
    }
);

// Java: gameserver/network/serverpackets/ExShowQuestMark.java
server_packet!(
    /// The "quest progressed" mark of a quest (`EX_SHOW_QUEST_MARK`).
    ExShowQuestMark,
    ExShowQuestMark {
        quest_id: i32,
    }
);

// Java: gameserver/network/serverpackets/RadarControl.java
server_packet!(
    /// A radar marker (`RADAR_CONTROL`). Java's `Radar` sends `(2, 2)` + `(0, 1)` to add one,
    /// `(1, 1)` to remove one and `(2, 2)` per marker to clear them.
    RadarControl,
    RadarControl {
        /// Java `_showRadar`: "0 = showradar; 1 = delete radar".
        show_radar: i32,
        /// Java `_type`: "radar type??".
        radar_type: i32,
        x: i32,
        y: i32,
        z: i32,
    }
);

// Java: gameserver/network/serverpackets/ExQuestNpcLogList.java
/// One NPC line of [`ExQuestNpcLogList`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestNpcLogEntry {
    /// The template id; `encode` adds 1000000 like Java's `writeImpl`.
    pub npc_id: i32,
    /// Java `NpcHolder.getUnknown()` (`writeByte`): 0 unless a script passes one.
    pub unknown: u8,
    pub count: i32,
}

// Java: gameserver/network/serverpackets/ExQuestNpcLogList.java
/// The kill counts of a quest shown in the journal (`EX_QUEST_NPC_LOG_LIST`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExQuestNpcLogList {
    pub quest_id: i32,
    pub npcs: Vec<QuestNpcLogEntry>,
}

impl ExQuestNpcLogList {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExQuestNpcLogList;
    const NPC_ID_OFFSET: i32 = 1_000_000;

    /// Java `new ExQuestNpcLogList(questId)`.
    pub fn new(quest_id: i32) -> Self {
        Self {
            quest_id,
            npcs: Vec::new(),
        }
    }

    /// Java `addNpc(npcId, count)`.
    pub fn add_npc(&mut self, npc_id: i32, count: i32) {
        self.add_npc_with(npc_id, 0, count);
    }

    /// Java `addNpc(npcId, unknown, count)`.
    pub fn add_npc_with(&mut self, npc_id: i32, unknown: u8, count: i32) {
        self.npcs.push(QuestNpcLogEntry {
            npc_id,
            unknown,
            count,
        });
    }
}

impl Encode for ExQuestNpcLogList {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        // Java `writeByte(_npcs.size())`: the count keeps its low byte.
        w.i32(self.quest_id).u8(self.npcs.len() as u8);
        for npc in &self.npcs {
            w.i32(npc.npc_id.wrapping_add(Self::NPC_ID_OFFSET))
                .u8(npc.unknown)
                .i32(npc.count);
        }
    }
}

impl Decode for ExQuestNpcLogList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let quest_id = r.i32()?;
        let count = r.u8()?;
        let mut npcs = Vec::with_capacity(usize::from(count));
        for _ in 0..count {
            let npc_id = r.i32()?.wrapping_sub(Self::NPC_ID_OFFSET);
            let unknown = r.u8()?;
            let count = r.i32()?;
            npcs.push(QuestNpcLogEntry {
                npc_id,
                unknown,
                count,
            });
        }
        Ok(Self { quest_id, npcs })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The body after the opcode (one byte, or `0xFE` + two for an ex packet).
    fn body(bytes: &[u8]) -> &[u8] {
        if bytes[0] == 0xFE {
            &bytes[3..]
        } else {
            &bytes[1..]
        }
    }

    fn round_trip<P: Encode + Decode + PartialEq + std::fmt::Debug>(packet: &P) -> Vec<u8> {
        let bytes = packet.to_bytes();
        let decoded = P::decode(&mut PacketReader::new(body(&bytes))).unwrap();
        assert_eq!(&decoded, packet);
        bytes
    }

    fn opcode_of(bytes: &[u8]) -> Option<ServerOpcode> {
        let ex = (bytes[0] == 0xFE).then(|| u16::from_le_bytes([bytes[1], bytes[2]]));
        ServerOpcode::from_id(bytes[0], ex)
    }

    #[test]
    fn quest_html_is_object_html_quest() {
        let packet = NpcQuestHtmlMessage::new(7, 1, "<html>a</html>").unwrap();
        let bytes = round_trip(&packet);
        assert_eq!(opcode_of(&bytes), Some(ServerOpcode::ExNpcQuestHtmlMessage));
        assert_eq!(&body(&bytes)[..4], [7, 0, 0, 0]);
        assert_eq!(&bytes[bytes.len() - 4..], [1, 0, 0, 0]);
        // `setHtml` wraps a fragment like `NpcHtmlMessage`.
        assert_eq!(
            NpcQuestHtmlMessage::new(0, 1, "x").unwrap().html,
            "<html><body>x</body></html>"
        );
        assert!(NpcQuestHtmlMessage::new(-1, 1, "x").is_none());
    }

    #[test]
    fn tutorial_packets_round_trip() {
        let bytes = round_trip(&TutorialShowHtml::new("<html>t</html>"));
        assert_eq!(opcode_of(&bytes), Some(ServerOpcode::TutorialShowHtml));
        let mark = round_trip(&TutorialShowQuestionMark { mark_id: 3 });
        assert_eq!(opcode_of(&mark), Some(ServerOpcode::ShowTutorialMark));
        assert_eq!(body(&mark), [3, 0, 0, 0]);
        let event = round_trip(&TutorialEnableClientEvent { event_id: 8 });
        assert_eq!(opcode_of(&event), Some(ServerOpcode::TutorialEnableClientEvent));
    }

    #[test]
    fn quest_mark_and_radar_round_trip() {
        let mark = round_trip(&ExShowQuestMark { quest_id: 1 });
        assert_eq!(opcode_of(&mark), Some(ServerOpcode::ExShowQuestMark));
        assert_eq!(body(&mark), [1, 0, 0, 0]);
        let radar = round_trip(&RadarControl {
            show_radar: 2,
            radar_type: 2,
            x: -1,
            y: 2,
            z: 3,
        });
        assert_eq!(body(&radar).len(), 20);
    }

    #[test]
    fn npc_log_adds_the_npc_offset_and_writes_byte_counts() {
        let mut packet = ExQuestNpcLogList::new(5);
        packet.add_npc(20_001, 3);
        packet.add_npc_with(20_002, 1, 4);
        let bytes = round_trip(&packet);
        let body = body(&bytes);
        assert_eq!(&body[..5], [5, 0, 0, 0, 2]);
        assert_eq!(
            i32::from_le_bytes([body[5], body[6], body[7], body[8]]),
            1_020_001
        );
        assert_eq!(body[9], 0);
        assert_eq!(body.len(), 5 + 2 * 9);
    }
}
