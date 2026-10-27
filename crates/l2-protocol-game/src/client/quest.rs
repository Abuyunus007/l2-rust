//! Client packets of the quest journal and the tutorial window (S-API-01).
//!
//! This only reads the packets. What they do is the world's script engine
//! (`l2_world::script`).

// Java: gameserver/network/clientpackets/RequestQuestList.java
client_unit_packet!(
    /// The player opens the quest journal (`0x62`). Java answers with `QuestList`.
    RequestQuestList,
    plain RequestQuestList
);

// Java: gameserver/network/clientpackets/RequestQuestAbort.java
client_packet!(
    /// The player aborts a quest from the journal (`0x63`).
    RequestQuestAbort,
    plain RequestQuestAbort {
        /// Java `_questId` (`readInt()`).
        quest_id: i32,
    }
);

// Java: gameserver/network/clientpackets/RequestTutorialLinkHtml.java
client_packet!(
    /// A link of the tutorial window (`0x85`): a bypass handler, or `TL:<bypass>` for the
    /// tutorial quest.
    RequestTutorialLinkHtml,
    plain RequestTutorialLinkHtml {
        /// Java `_bypass` (`readString()`).
        bypass: String,
    }
);

// Java: gameserver/network/clientpackets/RequestTutorialPassCmdToServer.java
client_packet!(
    /// A command of the tutorial window (`0x86`): a bypass handler, or `TC:<bypass>` for the
    /// tutorial quest.
    RequestTutorialPassCmdToServer,
    plain RequestTutorialPassCmdToServer {
        /// Java `_bypass` (`readString()`).
        bypass: String,
    }
);

// Java: gameserver/network/clientpackets/RequestTutorialQuestionMark.java
client_packet!(
    /// The player clicked the blinking question mark (`0x87`): `QM:<number>` for the tutorial
    /// quest.
    RequestTutorialQuestionMark,
    plain RequestTutorialQuestionMark {
        /// Java `_number` (`readInt()`).
        number: i32,
    }
);

// Java: gameserver/network/clientpackets/RequestTutorialClientEvent.java
client_packet!(
    /// A client event the tutorial enabled happened (`0x88`): `CE:<eventId>` for the tutorial
    /// quest.
    RequestTutorialClientEvent,
    plain RequestTutorialClientEvent {
        /// Java `_eventId` (`readInt()`).
        event_id: i32,
    }
);

#[cfg(test)]
mod tests {
    use l2_net::{Decode, Encode, PacketReader};

    use super::*;

    fn round_trip<P: Encode + Decode + PartialEq + std::fmt::Debug>(packet: P) -> Vec<u8> {
        let bytes = packet.to_bytes();
        let decoded = P::decode(&mut PacketReader::new(&bytes[1..])).unwrap();
        assert_eq!(decoded, packet);
        bytes
    }

    #[test]
    fn quest_list_is_the_opcode_alone() {
        assert_eq!(round_trip(RequestQuestList), [0x62]);
    }

    #[test]
    fn quest_abort_carries_the_quest_id() {
        assert_eq!(
            round_trip(RequestQuestAbort { quest_id: 1 }),
            [0x63, 1, 0, 0, 0]
        );
    }

    #[test]
    fn tutorial_links_carry_a_utf16_string() {
        let bytes = round_trip(RequestTutorialLinkHtml {
            bypass: "close".to_owned(),
        });
        assert_eq!(bytes[0], 0x85);
        assert_eq!(&bytes[1..3], [b'c', 0]);
        assert_eq!(&bytes[bytes.len() - 2..], [0, 0]);
        assert_eq!(
            round_trip(RequestTutorialPassCmdToServer {
                bypass: "admin_x".to_owned()
            })[0],
            0x86
        );
    }

    #[test]
    fn question_mark_and_client_event_carry_an_int() {
        assert_eq!(
            round_trip(RequestTutorialQuestionMark { number: -2 }),
            [0x87, 0xFE, 0xFF, 0xFF, 0xFF]
        );
        assert_eq!(
            round_trip(RequestTutorialClientEvent { event_id: 8 }),
            [0x88, 8, 0, 0, 0]
        );
    }

    #[test]
    fn a_short_body_is_an_error() {
        assert!(RequestQuestAbort::decode(&mut PacketReader::new(&[1, 2])).is_err());
    }
}
