//! Maps `(connection state, opcode)` to a client packet.
//!
//! Java: `gameserver/network/GamePacketHandler.java`. Which packet is accepted in which state
//! comes from the generated tables ([`ClientOpcode::connection_states`] and
//! [`ExClientOpcode::connection_states`]), so this file only lists the packets that already have
//! a struct. Adding a packet = one new [`ClientPacket`] variant + one match arm.

use l2_net::{Decode, PacketError, PacketReader};

use crate::client::{
    Action, Appearing, AttackRequest, AuthLogin, BypassUserCmd, CannotMoveAnymore, CharacterCreate,
    CharacterDelete, CharacterRestore, CharacterSelect, DlgAnswer, EnterWorld, Logout, MoveToLocation,
    NewCharacter, ProtocolVersion, RequestAcquireSkill, RequestAcquireSkillInfo, RequestActionUse,
    RequestAutoSoulShot, RequestBypassToServer, RequestDestroyItem, RequestDispel, RequestDropItem,
    RequestGotoLobby, RequestItemList, RequestKeyMapping, RequestLinkHtml, RequestMagicSkillUse,
    RequestNetPing, RequestRestart, RequestRestartPoint, RequestShortCutDel, RequestShortCutReg,
    RequestSkillCoolTime, RequestTargetCanceld, RequestUnEquipItem, Say2, SendBypassBuildCmd, UseItem,
    ValidatePosition,
};
use crate::generated::opcodes::{ClientOpcode, ConnectionState, ExClientOpcode};

/// A decoded client packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientPacket {
    ProtocolVersion(ProtocolVersion),
    AuthLogin(AuthLogin),
    NewCharacter(NewCharacter),
    CharacterCreate(CharacterCreate),
    CharacterDelete(CharacterDelete),
    CharacterRestore(CharacterRestore),
    CharacterSelect(CharacterSelect),
    RequestGotoLobby(RequestGotoLobby),
    EnterWorld(EnterWorld),
    RequestKeyMapping(RequestKeyMapping),
    Say2(Say2),
    Logout(Logout),
    RequestRestart(RequestRestart),
    RequestNetPing(RequestNetPing),
    RequestSkillCoolTime(RequestSkillCoolTime),
    Action(Action),
    RequestTargetCanceld(RequestTargetCanceld),
    RequestBypassToServer(RequestBypassToServer),
    RequestLinkHtml(RequestLinkHtml),
    SendBypassBuildCmd(SendBypassBuildCmd),
    BypassUserCmd(BypassUserCmd),
    MoveToLocation(MoveToLocation),
    ValidatePosition(ValidatePosition),
    CannotMoveAnymore(CannotMoveAnymore),
    Appearing(Appearing),
    /// `ATTACK` (0x01) and `ATTACK_REQUEST` (0x32): Java's `AttackRequest` for both.
    AttackRequest(AttackRequest),
    RequestActionUse(RequestActionUse),
    RequestRestartPoint(RequestRestartPoint),
    UseItem(UseItem),
    RequestUnEquipItem(RequestUnEquipItem),
    RequestDropItem(RequestDropItem),
    RequestDestroyItem(RequestDestroyItem),
    RequestItemList(RequestItemList),
    RequestMagicSkillUse(RequestMagicSkillUse),
    RequestAcquireSkill(RequestAcquireSkill),
    RequestAcquireSkillInfo(RequestAcquireSkillInfo),
    RequestShortCutReg(RequestShortCutReg),
    RequestShortCutDel(RequestShortCutDel),
    DlgAnswer(DlgAnswer),
    /// `EX_REQUEST_AUTO_SOULSHOT` (`0xD0` 0x000D), an ex packet.
    RequestAutoSoulShot(RequestAutoSoulShot),
    /// `EX_REQUEST_DISPEL` (`0xD0` 0x004B), an ex packet.
    RequestDispel(RequestDispel),
    /// Java would accept this plain packet in this state, but it has no struct here yet.
    /// The body is left unread.
    Unported(ClientOpcode),
    /// The same for ex packets (`0xD0` + `u16`). The ex id is consumed, the body is left unread.
    UnportedEx(ExClientOpcode),
}

/// Decodes the body of a client packet whose `opcode` byte was already read from `reader`.
///
/// Returns `Ok(None)` when Java ignores the packet: unknown opcode, or an opcode that is not
/// allowed in `state` (Java checks the state before reading any field). Returns `Err` when the
/// opcode and state are fine but the body is too short. For `0xD0` the dispatcher reads the
/// `u16` ex id itself, as Java does.
pub fn decode_client(
    state: ConnectionState,
    opcode: u8,
    reader: &mut PacketReader,
) -> Result<Option<ClientPacket>, PacketError> {
    let Some(opcode) = ClientOpcode::from_id(opcode) else {
        return Ok(None);
    };
    if !opcode.connection_states().contains(&state) {
        return Ok(None);
    }
    if opcode == ClientOpcode::ExPacket {
        return decode_ex_client(state, reader);
    }
    let packet = match opcode {
        ProtocolVersion::OPCODE => ClientPacket::ProtocolVersion(ProtocolVersion::decode(reader)?),
        AuthLogin::OPCODE => ClientPacket::AuthLogin(AuthLogin::decode(reader)?),
        NewCharacter::OPCODE => ClientPacket::NewCharacter(NewCharacter::decode(reader)?),
        CharacterCreate::OPCODE => ClientPacket::CharacterCreate(CharacterCreate::decode(reader)?),
        CharacterDelete::OPCODE => ClientPacket::CharacterDelete(CharacterDelete::decode(reader)?),
        CharacterRestore::OPCODE => ClientPacket::CharacterRestore(CharacterRestore::decode(reader)?),
        CharacterSelect::OPCODE => ClientPacket::CharacterSelect(CharacterSelect::decode(reader)?),
        EnterWorld::OPCODE => ClientPacket::EnterWorld(EnterWorld::decode(reader)?),
        Say2::OPCODE => ClientPacket::Say2(Say2::decode(reader)?),
        Logout::OPCODE => ClientPacket::Logout(Logout::decode(reader)?),
        RequestRestart::OPCODE => ClientPacket::RequestRestart(RequestRestart::decode(reader)?),
        RequestNetPing::OPCODE => ClientPacket::RequestNetPing(RequestNetPing::decode(reader)?),
        RequestSkillCoolTime::OPCODE => {
            ClientPacket::RequestSkillCoolTime(RequestSkillCoolTime::decode(reader)?)
        }
        Action::OPCODE => ClientPacket::Action(Action::decode(reader)?),
        RequestTargetCanceld::OPCODE => {
            ClientPacket::RequestTargetCanceld(RequestTargetCanceld::decode(reader)?)
        }
        RequestBypassToServer::OPCODE => {
            ClientPacket::RequestBypassToServer(RequestBypassToServer::decode(reader)?)
        }
        RequestLinkHtml::OPCODE => ClientPacket::RequestLinkHtml(RequestLinkHtml::decode(reader)?),
        SendBypassBuildCmd::OPCODE => ClientPacket::SendBypassBuildCmd(SendBypassBuildCmd::decode(reader)?),
        BypassUserCmd::OPCODE => ClientPacket::BypassUserCmd(BypassUserCmd::decode(reader)?),
        MoveToLocation::OPCODE => ClientPacket::MoveToLocation(MoveToLocation::decode(reader)?),
        ValidatePosition::OPCODE => ClientPacket::ValidatePosition(ValidatePosition::decode(reader)?),
        CannotMoveAnymore::OPCODE => ClientPacket::CannotMoveAnymore(CannotMoveAnymore::decode(reader)?),
        Appearing::OPCODE => ClientPacket::Appearing(Appearing::decode(reader)?),
        AttackRequest::OPCODE | AttackRequest::OPCODE_REQUEST => {
            ClientPacket::AttackRequest(AttackRequest::decode(reader)?)
        }
        RequestActionUse::OPCODE => ClientPacket::RequestActionUse(RequestActionUse::decode(reader)?),
        RequestRestartPoint::OPCODE => {
            ClientPacket::RequestRestartPoint(RequestRestartPoint::decode(reader)?)
        }
        UseItem::OPCODE => ClientPacket::UseItem(UseItem::decode(reader)?),
        RequestUnEquipItem::OPCODE => ClientPacket::RequestUnEquipItem(RequestUnEquipItem::decode(reader)?),
        RequestDropItem::OPCODE => ClientPacket::RequestDropItem(RequestDropItem::decode(reader)?),
        RequestDestroyItem::OPCODE => ClientPacket::RequestDestroyItem(RequestDestroyItem::decode(reader)?),
        RequestItemList::OPCODE => ClientPacket::RequestItemList(RequestItemList::decode(reader)?),
        RequestMagicSkillUse::OPCODE => {
            ClientPacket::RequestMagicSkillUse(RequestMagicSkillUse::decode(reader)?)
        }
        RequestAcquireSkill::OPCODE => {
            ClientPacket::RequestAcquireSkill(RequestAcquireSkill::decode(reader)?)
        }
        RequestAcquireSkillInfo::OPCODE => {
            ClientPacket::RequestAcquireSkillInfo(RequestAcquireSkillInfo::decode(reader)?)
        }
        RequestShortCutReg::OPCODE => ClientPacket::RequestShortCutReg(RequestShortCutReg::decode(reader)?),
        RequestShortCutDel::OPCODE => ClientPacket::RequestShortCutDel(RequestShortCutDel::decode(reader)?),
        DlgAnswer::OPCODE => ClientPacket::DlgAnswer(DlgAnswer::decode(reader)?),
        other => ClientPacket::Unported(other),
    };
    Ok(Some(packet))
}

/// The part after `0xD0`: the `u16` ex id and the body.
fn decode_ex_client(
    state: ConnectionState,
    reader: &mut PacketReader,
) -> Result<Option<ClientPacket>, PacketError> {
    let Some(ex) = ExClientOpcode::from_id(reader.u16()?) else {
        return Ok(None);
    };
    if !ex.connection_states().contains(&state) {
        return Ok(None);
    }
    // TODO(M2-GPROTO-*): Java reads a `u32` sub id right after the ex id of 0x51 (bookmarks) to
    // pick the packet. Whoever ports the bookmark packets handles it here.
    let packet = match ex {
        RequestGotoLobby::EX_OPCODE => ClientPacket::RequestGotoLobby(RequestGotoLobby::decode(reader)?),
        RequestKeyMapping::EX_OPCODE => ClientPacket::RequestKeyMapping(RequestKeyMapping::decode(reader)?),
        RequestAutoSoulShot::EX_OPCODE => {
            ClientPacket::RequestAutoSoulShot(RequestAutoSoulShot::decode(reader)?)
        }
        RequestDispel::EX_OPCODE => ClientPacket::RequestDispel(RequestDispel::decode(reader)?),
        other => ClientPacket::UnportedEx(other),
    };
    Ok(Some(packet))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::skills::AcquireSkillType;
    use ConnectionState::{Authenticated, Connected, Entering, InGame};
    use l2_net::Encode;

    /// A body long enough for every ported packet (all fields zero; strings are empty).
    /// EnterWorld needs 104 bytes.
    const ZEROS: [u8; 128] = [0; 128];
    /// The id byte of ex packets.
    const EX_PACKET: u8 = 0xD0;

    fn decode(state: ConnectionState, opcode: u8, body: &[u8]) -> Result<Option<ClientPacket>, PacketError> {
        decode_client(state, opcode, &mut PacketReader::new(body))
    }

    #[test]
    fn every_opcode_in_every_state_follows_the_generated_table() {
        for opcode in 0..=u8::MAX {
            for &state in ConnectionState::ALL {
                let known = ClientOpcode::from_id(opcode);
                let accepted = known.is_some_and(|op| op.connection_states().contains(&state));
                // 0xD0 reads its ex id from the body: 0xFFFF is no ex packet, so it is ignored.
                let body = [0xFF; 2].iter().chain(&ZEROS).copied().collect::<Vec<u8>>();
                let result = decode(state, opcode, &body).unwrap();
                if opcode == EX_PACKET {
                    assert!(result.is_none(), "{state:?}");
                } else {
                    assert_eq!(result.is_some(), accepted, "opcode {opcode:#04x} in {state:?}");
                }
            }
        }
    }

    #[test]
    fn every_ex_opcode_in_every_state_follows_the_generated_table() {
        for ex in ExClientOpcode::ALL {
            for &state in ConnectionState::ALL {
                let mut body = ex.id().to_le_bytes().to_vec();
                body.extend(ZEROS);
                let result = decode(state, EX_PACKET, &body).unwrap();
                assert_eq!(
                    result.is_some(),
                    ex.connection_states().contains(&state),
                    "{} in {state:?}",
                    ex.name()
                );
            }
        }
    }

    #[test]
    fn states_match_java_for_the_ported_packets() {
        // GamePacketHandler + ClientPackets.java: handshake in CONNECTED, character screen in
        // AUTHENTICATED.
        let ported = [
            (0x0E, Connected),
            (0x2B, Connected),
            (0x0C, Authenticated),
            (0x0D, Authenticated),
            (0x12, Authenticated),
            (0x13, Authenticated),
            (0x7B, Authenticated),
            (0x11, Entering),
        ];
        for (opcode, home) in ported {
            for &state in ConnectionState::ALL {
                let result = decode(state, opcode, &ZEROS).unwrap();
                assert_eq!(
                    result.is_some(),
                    state == home,
                    "opcode {opcode:#04x} in {state:?}"
                );
            }
        }
        // RequestGotoLobby is ex id 0x0036, AUTHENTICATED only.
        for &state in ConnectionState::ALL {
            assert_eq!(
                decode(state, EX_PACKET, &[0x36, 0x00]).unwrap().is_some(),
                state == Authenticated,
                "{state:?}"
            );
        }
        // RequestKeyMapping is ex id 0x0021, ENTERING and IN_GAME.
        for &state in ConnectionState::ALL {
            assert_eq!(
                decode(state, EX_PACKET, &[0x21, 0x00]).unwrap().is_some(),
                matches!(state, Entering | InGame),
                "{state:?}"
            );
        }
        // Logout is accepted in AUTHENTICATED and IN_GAME; chat, restart and net ping in IN_GAME.
        for &state in ConnectionState::ALL {
            assert_eq!(
                decode(state, 0x00, &ZEROS).unwrap().is_some(),
                matches!(state, Authenticated | InGame),
                "{state:?}"
            );
            for opcode in [0x49, 0x57, 0xB1] {
                assert_eq!(
                    decode(state, opcode, &ZEROS).unwrap().is_some(),
                    state == InGame,
                    "opcode {opcode:#04x} in {state:?}"
                );
            }
        }
    }

    #[test]
    fn decodes_the_right_packet() {
        assert_eq!(
            decode(Connected, 0x0E, &[0x11, 0x01, 0, 0]).unwrap(),
            Some(ClientPacket::ProtocolVersion(ProtocolVersion { version: 273 }))
        );
        assert_eq!(
            decode(Authenticated, 0x0D, &[1, 0, 0, 0]).unwrap(),
            Some(ClientPacket::CharacterDelete(CharacterDelete { char_slot: 1 }))
        );
        assert_eq!(
            decode(Authenticated, 0x7B, &[2, 0, 0, 0]).unwrap(),
            Some(ClientPacket::CharacterRestore(CharacterRestore { char_slot: 2 }))
        );
        assert_eq!(
            decode(Authenticated, 0x13, &[]).unwrap(),
            Some(ClientPacket::NewCharacter(NewCharacter))
        );
        // Ex packet: the dispatcher consumes the u16 ex id.
        assert_eq!(
            decode(Authenticated, 0xD0, &[0x36, 0x00]).unwrap(),
            Some(ClientPacket::RequestGotoLobby(RequestGotoLobby))
        );
        assert_eq!(
            decode(InGame, 0x49, &[0x41, 0, 0, 0, 2, 0, 0, 0, 0x42, 0, 0, 0]).unwrap(),
            Some(ClientPacket::Say2(Say2 {
                text: "A".into(),
                chat_type: 2,
                target: Some("B".into()),
            }))
        );
        assert_eq!(
            decode(InGame, 0x57, &[]).unwrap(),
            Some(ClientPacket::RequestRestart(RequestRestart))
        );
        // What the encoders write decodes to the same packet (opcode bytes stripped).
        let select = CharacterSelect {
            char_slot: 3,
            unk1: 1,
            unk2: 2,
            unk3: 3,
            unk4: 4,
        };
        let bytes = select.to_bytes();
        assert_eq!(
            decode(Authenticated, bytes[0], &bytes[1..]).unwrap(),
            Some(ClientPacket::CharacterSelect(select))
        );
    }

    #[test]
    fn known_but_unported_packets_are_reported() {
        // RequestGetItemFromPet (0x2C, a pet packet) is accepted in IN_GAME, but has no struct yet.
        assert_eq!(
            decode(InGame, 0x2C, &[]).unwrap(),
            Some(ClientPacket::Unported(ClientOpcode::RequestGetItemFromPet))
        );
        // RequestManorList (ex id 0x0001): the id is consumed, the body is left alone.
        let mut r = PacketReader::new(&[0x01, 0x00, 0xAA]);
        let result = decode_client(InGame, 0xD0, &mut r).unwrap();
        assert_eq!(
            result,
            Some(ClientPacket::UnportedEx(ExClientOpcode::RequestManorList))
        );
        assert_eq!(r.remaining(), 1);
    }

    #[test]
    fn attack_request_has_both_java_ids() {
        // Java ClientPackets: ATTACK (0x01) and ATTACK_REQUEST (0x32) both use AttackRequest.
        // object id, three zero origin ints, then the attack id byte (1 = shift-click).
        let mut body = 0x4433_2211_i32.to_le_bytes().to_vec();
        body.extend([0; 12]);
        body.push(1);
        let expected = Some(ClientPacket::AttackRequest(AttackRequest {
            object_id: 0x4433_2211,
            origin_x: 0,
            origin_y: 0,
            origin_z: 0,
            attack_id: 1,
        }));
        for id in [0x01, 0x32] {
            assert_eq!(decode(InGame, id, &body).unwrap(), expected, "opcode {id:#04x}");
        }
        // Not in the other states, like the rest of the table.
        assert_eq!(decode(Authenticated, 0x01, &body).unwrap(), None);
    }

    #[test]
    fn combat_and_item_packets_decode_in_game() {
        assert_eq!(
            decode(InGame, 0x14, &[]).unwrap(),
            Some(ClientPacket::RequestItemList(RequestItemList))
        );
        assert_eq!(
            decode(InGame, 0x19, &[7, 0, 0, 0, 1, 0, 0, 0]).unwrap(),
            Some(ClientPacket::UseItem(UseItem {
                object_id: 7,
                ctrl_pressed: true,
            }))
        );
        assert_eq!(
            decode(InGame, 0x7D, &[5, 0, 0, 0]).unwrap(),
            Some(ClientPacket::RequestRestartPoint(RequestRestartPoint {
                requested_point_type: 5,
            }))
        );
        // The state check comes first: RequestRestartPoint is IN_GAME only.
        assert_eq!(decode(Authenticated, 0x7D, &[5, 0, 0, 0]).unwrap(), None);
    }

    #[test]
    fn skill_and_soulshot_packets_decode_in_game() {
        assert_eq!(
            decode(InGame, 0x39, &[0x99, 0x04, 0, 0, 2, 0, 0, 0, 1]).unwrap(),
            Some(ClientPacket::RequestMagicSkillUse(RequestMagicSkillUse {
                magic_id: 0x499,
                // Java: any non-zero int is "pressed" for ctrl and any non-zero byte for shift.
                ctrl_pressed: true,
                shift_pressed: true,
            }))
        );
        assert_eq!(
            decode(InGame, 0x7C, &[1, 0, 0, 0, 2, 0, 0, 0, 2, 0, 0, 0]).unwrap(),
            Some(ClientPacket::RequestAcquireSkill(RequestAcquireSkill {
                id: 1,
                level: 2,
                skill_type: AcquireSkillType::Pledge,
                sub_type: 0,
            }))
        );
        // The squad id is only read for SUBPLEDGE (type 3).
        assert_eq!(
            decode(InGame, 0x7C, &[1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0, 5, 0, 0, 0]).unwrap(),
            Some(ClientPacket::RequestAcquireSkill(RequestAcquireSkill {
                id: 1,
                level: 2,
                skill_type: AcquireSkillType::Subpledge,
                sub_type: 5,
            }))
        );
        // An unknown learn type is a bad packet, not a panic.
        assert!(decode(InGame, 0x7C, &[1, 0, 0, 0, 2, 0, 0, 0, 9, 0, 0, 0]).is_err());
        assert_eq!(
            decode(InGame, 0x73, &[4, 0, 0, 0, 1, 0, 0, 0, 4, 0, 0, 0]).unwrap(),
            Some(ClientPacket::RequestAcquireSkillInfo(RequestAcquireSkillInfo {
                id: 4,
                level: 1,
                skill_type: AcquireSkillType::Transform,
            }))
        );
        assert_eq!(
            decode(
                InGame,
                0x3D,
                &[2, 0, 0, 0, 14, 0, 0, 0, 0xB4, 0x04, 0, 0, 7, 0, 0, 0, 1, 0, 0, 0]
            )
            .unwrap(),
            Some(ClientPacket::RequestShortCutReg(RequestShortCutReg {
                type_id: 2,
                slot: 14,
                id: 1204,
                level: 7,
                character_type: 1,
            }))
        );
        assert_eq!(
            decode(InGame, 0x3F, &[13, 0, 0, 0]).unwrap(),
            Some(ClientPacket::RequestShortCutDel(RequestShortCutDel { slot: 13 }))
        );
        // The shortcut bar only exists in game.
        assert_eq!(decode(Authenticated, 0x3F, &[13, 0, 0, 0]).unwrap(), None);
        // Ex packets: the dispatcher consumes the ex id (0x000D soulshot, 0x004B dispel).
        assert_eq!(
            decode(InGame, 0xD0, &[0x0D, 0x00, 7, 0, 0, 0, 1, 0, 0, 0]).unwrap(),
            Some(ClientPacket::RequestAutoSoulShot(RequestAutoSoulShot {
                item_id: 7,
                shot_type: RequestAutoSoulShot::ON,
            }))
        );
        assert_eq!(
            decode(InGame, 0xD0, &[0x4B, 0x00, 9, 0, 0, 0, 3, 0, 0, 0, 2, 0, 0, 0]).unwrap(),
            Some(ClientPacket::RequestDispel(RequestDispel {
                object_id: 9,
                skill_id: 3,
                skill_level: 2,
            }))
        );
        // The state check comes first: these are IN_GAME only.
        assert_eq!(decode(Authenticated, 0x39, &ZEROS).unwrap(), None);
        assert_eq!(decode(Entering, 0xD0, &[0x4B, 0x00]).unwrap(), None);
    }

    #[test]
    fn unknown_opcodes_are_ignored() {
        assert_eq!(decode(Connected, 0x02, &ZEROS).unwrap(), None); // unused in the Java table
        assert_eq!(decode(Authenticated, 0xFF, &ZEROS).unwrap(), None);
        assert_eq!(decode(Authenticated, 0xD0, &[0xFF, 0xFF]).unwrap(), None);
    }

    #[test]
    fn short_bodies() {
        // Right state, body too short: an error.
        for (state, opcode, len) in [
            (Connected, 0x2B, 3),
            (Authenticated, 0x0C, 4),
            (Authenticated, 0x0D, 3),
            (Authenticated, 0x12, 17),
            (Authenticated, 0x7B, 0),
            (Entering, 0x11, 103),
            (Authenticated, 0xD0, 1),
            (InGame, 0x49, 2),
            (InGame, 0x3D, 19),
            (InGame, 0x3F, 3),
        ] {
            assert!(
                decode(state, opcode, &ZEROS[..len]).is_err(),
                "opcode {opcode:#04x}"
            );
        }
        // Wrong state: the body is never read, so no error (Java checks the state first).
        assert_eq!(decode(InGame, 0x0D, &[]).unwrap(), None);
        assert_eq!(decode(Connected, 0x12, &[]).unwrap(), None);
        // ProtocolVersion tolerates any length (Java quirk), even an empty one.
        assert_eq!(
            decode(Connected, 0x0E, &[]).unwrap(),
            Some(ClientPacket::ProtocolVersion(ProtocolVersion { version: 0 }))
        );
    }
}
