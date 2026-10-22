//! The answer to a couple social action (`AnswerCoupleAction`).
//!
//! This only reads the packet. Every check (who asked, how far apart they are) is the world's job
//! (`l2_world::actions`). Java also ignores the client opcodes `SOCIAL_ACTION` (0x34),
//! `CHANGE_MOVE_TYPE` (0x35) and `CHANGE_WAIT_TYPE` (0x36): their handlers are `null`, the sit
//! and walk/run buttons arrive as `RequestActionUse`.

// Java: gameserver/network/clientpackets/AnswerCoupleAction.java
client_packet!(
    /// The asked player answers a couple action request (`0xD0` 0x007A, `ANSWER_COUPLE_ACTION`).
    /// Java reads the action id, the answer, then the object id of the player who asked.
    AnswerCoupleAction,
    ex AnswerCoupleAction {
        /// The couple social action id (`RequestActionUse` id minus 55).
        action_id: i32,
        /// `1` approve, `0` cancel, `-1` the client refuses couple actions.
        answer: i32,
        /// The object id of the player who asked.
        object_id: i32,
    }
);

#[cfg(test)]
mod tests {
    use l2_net::{Decode, Encode, PacketReader};

    use super::*;

    #[test]
    fn writes_0xd0_the_ex_id_and_the_fields_in_java_order() {
        let packet = AnswerCoupleAction {
            action_id: 16,
            answer: -1,
            object_id: 0x1000_0001,
        };
        assert_eq!(
            packet.to_bytes(),
            [
                0xD0, 0x7A, 0x00, 16, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF, 0x01, 0x00, 0x00, 0x10
            ]
        );
    }

    #[test]
    fn round_trips() {
        let packet = AnswerCoupleAction {
            action_id: 17,
            answer: 1,
            object_id: 42,
        };
        let bytes = packet.to_bytes();
        let decoded = AnswerCoupleAction::decode(&mut PacketReader::new(&bytes[3..])).unwrap();
        assert_eq!(decoded, packet);
    }

    #[test]
    fn a_short_body_is_an_error() {
        assert!(AnswerCoupleAction::decode(&mut PacketReader::new(&[1, 0, 0, 0, 1, 0, 0, 0])).is_err());
    }
}
