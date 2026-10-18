//! Maps `(opcode, connection state)` to a client packet.
//!
//! Java: `loginserver/network/{LoginClientPackets,ConnectionState,LoginPacketHandler}.java`.

use l2_net::{Decode, PacketError, PacketReader};

use crate::client::{AuthGameGuard, RequestAuthLogin, RequestServerList, RequestServerLogin};

/// Where a client connection is in the login flow (Java `ConnectionState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConnectionState {
    Connected,
    AuthedGg,
    AuthedLogin,
}

/// `REQUEST_PI_AGREEMENT_CHECK`: allowed in `AuthedLogin`, but Java has no packet class for it.
pub const REQUEST_PI_AGREEMENT_CHECK: u8 = 0x0E;
/// `REQUEST_PI_AGREEMENT`: same as above.
pub const REQUEST_PI_AGREEMENT: u8 = 0x0F;

/// A decoded client packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientPacket {
    AuthGameGuard(AuthGameGuard),
    // Boxed because the two RSA blocks make this variant much larger than the others.
    RequestAuthLogin(Box<RequestAuthLogin>),
    RequestServerList(RequestServerList),
    RequestServerLogin(RequestServerLogin),
}

/// Decodes the body of a client packet whose `opcode` byte was already read from `reader`.
///
/// Returns `Ok(None)` when Java would ignore the packet: unknown opcode, an opcode that is not
/// allowed in `state`, or the PI agreement opcodes, which are recognized but have no handler.
/// Returns `Err` when the opcode and state are fine but the body is too short.
pub fn decode_client(
    state: ConnectionState,
    opcode: u8,
    reader: &mut PacketReader,
) -> Result<Option<ClientPacket>, PacketError> {
    use ConnectionState::{AuthedGg, AuthedLogin, Connected};
    let packet = match (opcode, state) {
        (AuthGameGuard::OPCODE, Connected) => ClientPacket::AuthGameGuard(AuthGameGuard::decode(reader)?),
        (RequestAuthLogin::OPCODE, AuthedGg) => {
            ClientPacket::RequestAuthLogin(Box::new(RequestAuthLogin::decode(reader)?))
        }
        (RequestServerLogin::OPCODE, AuthedLogin) => {
            ClientPacket::RequestServerLogin(RequestServerLogin::decode(reader)?)
        }
        (RequestServerList::OPCODE, AuthedLogin) => {
            ClientPacket::RequestServerList(RequestServerList::decode(reader)?)
        }
        // Recognized, but Java ignores them: same result as an unknown opcode.
        (REQUEST_PI_AGREEMENT_CHECK | REQUEST_PI_AGREEMENT, AuthedLogin) => return Ok(None),
        _ => return Ok(None),
    };
    Ok(Some(packet))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ConnectionState::{AuthedGg, AuthedLogin, Connected};
    use l2_net::Encode;

    const STATES: [ConnectionState; 3] = [Connected, AuthedGg, AuthedLogin];

    /// Valid body for each decodable opcode (the bytes after the opcode).
    fn body(opcode: u8) -> Vec<u8> {
        let packet: Vec<u8> = match opcode {
            AuthGameGuard::OPCODE => AuthGameGuard { session_id: 1 }.to_bytes(),
            RequestAuthLogin::OPCODE => RequestAuthLogin::Rsa128 { block: [9; 128] }.to_bytes(),
            RequestServerLogin::OPCODE => RequestServerLogin {
                login_ok1: 1,
                login_ok2: 2,
                server_id: 3,
            }
            .to_bytes(),
            RequestServerList::OPCODE => RequestServerList {
                login_ok1: 1,
                login_ok2: 2,
            }
            .to_bytes(),
            _ => vec![opcode, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        };
        packet[1..].to_vec()
    }

    /// The state in which Java accepts the opcode, if it accepts it at all.
    fn allowed_state(opcode: u8) -> Option<ConnectionState> {
        match opcode {
            0x07 => Some(Connected),
            0x00 => Some(AuthedGg),
            0x02 | 0x05 | 0x0E | 0x0F => Some(AuthedLogin),
            _ => None,
        }
    }

    #[test]
    fn every_opcode_in_every_state() {
        for opcode in 0..=u8::MAX {
            for state in STATES {
                let body = body(opcode);
                let mut reader = PacketReader::new(&body);
                let result = decode_client(state, opcode, &mut reader).unwrap();
                let accepted = allowed_state(opcode) == Some(state);
                let decodes = matches!(opcode, 0x07 | 0x00 | 0x02 | 0x05);
                assert_eq!(
                    result.is_some(),
                    accepted && decodes,
                    "opcode {opcode:#04x} in {state:?}"
                );
                if result.is_none() {
                    // Ignored packets leave the body unread.
                    assert_eq!(reader.remaining(), body.len(), "opcode {opcode:#04x}");
                }
            }
        }
    }

    #[test]
    fn decodes_the_right_packet() {
        let mut r = PacketReader::new(&[7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(
            decode_client(Connected, 0x07, &mut r).unwrap(),
            Some(ClientPacket::AuthGameGuard(AuthGameGuard { session_id: 7 }))
        );
        let body = [1, 0, 0, 0, 2, 0, 0, 0, 5];
        assert_eq!(
            decode_client(AuthedLogin, 0x02, &mut PacketReader::new(&body)).unwrap(),
            Some(ClientPacket::RequestServerLogin(RequestServerLogin {
                login_ok1: 1,
                login_ok2: 2,
                server_id: 5
            }))
        );
        assert_eq!(
            decode_client(AuthedLogin, 0x05, &mut PacketReader::new(&body)).unwrap(),
            Some(ClientPacket::RequestServerList(RequestServerList {
                login_ok1: 1,
                login_ok2: 2
            }))
        );
        assert_eq!(
            decode_client(AuthedGg, 0x00, &mut PacketReader::new(&[4; 128])).unwrap(),
            Some(ClientPacket::RequestAuthLogin(Box::new(
                RequestAuthLogin::Rsa128 { block: [4; 128] }
            )))
        );
    }

    #[test]
    fn short_body_in_the_right_state_is_an_error() {
        let cases = [
            (Connected, 0x07, 19),
            (AuthedGg, 0x00, 127),
            (AuthedLogin, 0x02, 8),
            (AuthedLogin, 0x05, 7),
        ];
        for (state, opcode, len) in cases {
            let body = vec![0; len];
            let result = decode_client(state, opcode, &mut PacketReader::new(&body));
            assert!(result.is_err(), "opcode {opcode:#04x} with {len} bytes");
        }
    }

    #[test]
    fn short_body_in_the_wrong_state_is_ignored_not_an_error() {
        // Java checks the state before it reads anything.
        assert_eq!(
            decode_client(AuthedLogin, 0x07, &mut PacketReader::new(&[])).unwrap(),
            None
        );
    }
}
