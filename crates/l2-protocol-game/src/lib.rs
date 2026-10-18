//! Game client/server protocol.
//!
//! For now this crate only holds the generated opcode tables (M1-GEN-01), which the packet proxy
//! uses to name packets. Packet structs and encode/decode code arrive in M2.

pub mod generated;

pub use generated::opcodes::{ClientOpcode, ConnectionState, ExClientOpcode, ServerOpcode};

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    // Expected counts: docs/analysis/02-network-crypto.md (checked against the Java tables).
    #[test]
    fn table_sizes_match_java() {
        assert_eq!(ClientOpcode::ALL.len(), 170);
        assert_eq!(ExClientOpcode::ALL.len(), 144);
        assert_eq!(ServerOpcode::ALL.len(), 485);
        assert_eq!(ConnectionState::ALL.len(), 6);
    }

    #[test]
    fn ids_round_trip_and_names_are_unique() {
        for op in ClientOpcode::ALL {
            assert_eq!(ClientOpcode::from_id(op.id()), Some(op));
        }
        for op in ExClientOpcode::ALL {
            assert_eq!(ExClientOpcode::from_id(op.id()), Some(op));
        }
        for op in ServerOpcode::ALL {
            assert_eq!(ServerOpcode::from_id(op.id(), op.ex_id()), Some(op));
        }
        let names: HashSet<_> = ServerOpcode::ALL.iter().map(|o| o.name()).collect();
        assert_eq!(names.len(), ServerOpcode::ALL.len());
        assert_eq!(
            ClientOpcode::from_id(0x02),
            None,
            "0x02 is unused in the Java table"
        );
        assert_eq!(ServerOpcode::from_id(0xFE, None), None);
    }

    // Spot checks against the Java source (ClientPackets.java etc.).
    #[test]
    fn client_spot_checks() {
        use ConnectionState::*;
        assert_eq!(ClientOpcode::from_id(0x00), Some(ClientOpcode::Logout));
        assert_eq!(ClientOpcode::Logout.name(), "LOGOUT");
        assert_eq!(ClientOpcode::Logout.connection_states(), [Authenticated, InGame]);
        assert_eq!(ClientOpcode::ProtocolVersion.id(), 0x0E);
        assert_eq!(ClientOpcode::ProtocolVersion.connection_states(), [Connected]);
        assert_eq!(ClientOpcode::EnterWorld.connection_states(), [Entering]);
        assert_eq!(ClientOpcode::Attack.connection_states(), [InGame]);
        // EX_PACKET allows every state (`ConnectionState.values()`).
        assert_eq!(ClientOpcode::ExPacket.id(), 0xD0);
        assert_eq!(ClientOpcode::ExPacket.connection_states(), ConnectionState::ALL);
        assert_eq!(ClientOpcode::RequestBuyProcure.id(), 0xCF);
    }

    #[test]
    fn ex_client_spot_checks() {
        use ConnectionState::*;
        assert_eq!(
            ExClientOpcode::from_id(0x36),
            Some(ExClientOpcode::RequestGotoLobby)
        );
        assert_eq!(
            ExClientOpcode::RequestGotoLobby.connection_states(),
            [Authenticated]
        );
        assert_eq!(ExClientOpcode::RequestHardwareInfo.id(), 0x96);
        assert_eq!(
            ExClientOpcode::RequestHardwareInfo.connection_states(),
            ConnectionState::ALL
        );
        assert_eq!(ExClientOpcode::ExMax.id(), 0x97);
        assert_eq!(ExClientOpcode::RequestManorList.name(), "REQUEST_MANOR_LIST");
    }

    #[test]
    fn server_spot_checks() {
        assert_eq!(ServerOpcode::from_id(0x00, None), Some(ServerOpcode::Die));
        assert_eq!(ServerOpcode::CharacterSelectionInfo.id(), 0x09);
        assert_eq!(ServerOpcode::CharacterSelectionInfo.ex_id(), None);
        assert_eq!(
            ServerOpcode::from_id(0xFE, Some(0xE8)),
            Some(ServerOpcode::ExSay2Fail)
        );
        assert_eq!(ServerOpcode::ExSay2Fail.name(), "EX_SAY2_FAIL");
        // Java quirk: the declared ex id 0x00 is kept in the table.
        assert_eq!(ServerOpcode::ExDummy.ex_id(), Some(0));
    }

    #[test]
    fn connection_state_names() {
        assert_eq!(ConnectionState::InGame.name(), "IN_GAME");
        assert_eq!(ConnectionState::Connected.name(), "CONNECTED");
    }
}
