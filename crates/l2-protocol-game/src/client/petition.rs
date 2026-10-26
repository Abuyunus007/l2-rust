//! Client packets of the GM petitions and of the GM chat snooping.
//!
//! These only read the packet. The rules (the limits, the queue, who answers) are the world's job
//! (`l2-world/src/petition.rs`).

// Java: gameserver/network/clientpackets/RequestPetition.java
client_packet!(
    /// The player submits a petition (`REQUEST_PETITION`): the text and the type (1 to 9, the
    /// position in Java's `PetitionType`).
    RequestPetition,
    plain RequestPetition {
        content: String,
        kind: i32,
    }
);

// Java: gameserver/network/clientpackets/RequestPetitionCancel.java
client_unit_packet!(
    /// The player cancels its petition, or a GM ends the consultation (`REQUEST_PETITION_CANCEL`).
    /// Java's `readImpl` reads nothing.
    RequestPetitionCancel,
    plain RequestPetitionCancel
);

// Java: gameserver/network/clientpackets/RequestPetitionFeedback.java
client_packet!(
    /// The player rates the consultation (`REQUEST_PETITION_FEEDBACK`): an unknown int, the rate
    /// (0 very poor to 4 very good) and a message.
    RequestPetitionFeedback,
    plain RequestPetitionFeedback {
        unknown: i32,
        rate: i32,
        message: String,
    }
);

// Java: gameserver/network/clientpackets/SnoopQuit.java
client_packet!(
    /// A GM stops snooping a player (`SNOOP_QUIT`): the object id of the snooped player.
    SnoopQuit,
    plain SnoopQuit {
        snoop_id: i32,
    }
);

#[cfg(test)]
mod tests {
    use l2_net::{Decode, Encode, PacketReader};

    use super::*;

    #[test]
    fn petition_round_trips() {
        let packet = RequestPetition {
            content: "stuck".to_owned(),
            kind: 3,
        };
        let bytes = packet.to_bytes();
        assert_eq!(bytes[0], 0x89);
        assert_eq!(
            RequestPetition::decode(&mut PacketReader::new(&bytes[1..])).unwrap(),
            packet
        );
    }

    #[test]
    fn cancel_has_no_fields() {
        assert_eq!(RequestPetitionCancel.to_bytes(), [0x8A]);
        assert!(RequestPetitionCancel::decode(&mut PacketReader::new(&[])).is_ok());
    }

    #[test]
    fn feedback_round_trips() {
        let packet = RequestPetitionFeedback {
            unknown: 1,
            rate: 4,
            message: "thanks".to_owned(),
        };
        let bytes = packet.to_bytes();
        assert_eq!(bytes[0], 0xC9);
        assert_eq!(
            RequestPetitionFeedback::decode(&mut PacketReader::new(&bytes[1..])).unwrap(),
            packet
        );
    }

    #[test]
    fn snoop_quit_round_trips_and_a_short_body_fails() {
        let packet = SnoopQuit {
            snoop_id: 0x1000_0001,
        };
        let bytes = packet.to_bytes();
        assert_eq!(bytes, [0xB4, 0x01, 0x00, 0x00, 0x10]);
        assert_eq!(
            SnoopQuit::decode(&mut PacketReader::new(&bytes[1..])).unwrap(),
            packet
        );
        assert!(SnoopQuit::decode(&mut PacketReader::new(&[1, 2])).is_err());
    }
}
