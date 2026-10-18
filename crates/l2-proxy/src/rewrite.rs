//! Rewriting the `ServerList` so the client's game connection goes through the proxy.
//!
//! Java: loginserver/network/serverpackets/ServerList.java (layout).
//!
//! The rewrite patches the IP and port bytes **in place** instead of decoding and re-encoding
//! the packet. Everything else (flags, player counts, character info, anything a newer server
//! adds) reaches the client byte for byte as the real login server sent it.

use std::net::Ipv4Addr;

use l2_protocol_login::server::ServerList;
use thiserror::Error;

/// Bytes before the first server entry: opcode, server count, last server.
const HEADER_LEN: usize = 3;
/// Size of one server entry. Layout: `u8 id, 4B ip, i32 port, u8 age, u8 pvp, u16 cur, u16 max,
/// u8 up, i32 type, u8 brackets` = 21 bytes.
pub(crate) const ENTRY_LEN: usize = 21;
const IP_OFFSET: usize = 1;
const PORT_OFFSET: usize = 5;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RewriteError {
    #[error("not a ServerList packet (opcode {0:#04x})")]
    NotAServerList(u8),

    #[error("ServerList has {actual} entries but {expected} replacements were given")]
    CountMismatch { expected: usize, actual: usize },

    #[error("ServerList is truncated: {len} bytes cannot hold {count} entries")]
    Truncated { len: usize, count: usize },
}

/// Returns a copy of `payload` (a plaintext `ServerList`, opcode first) where entry `i` has its
/// address replaced by `replacements[i]`. `None` leaves that entry untouched.
pub fn patch_server_list(
    payload: &[u8],
    replacements: &[Option<(Ipv4Addr, u16)>],
) -> Result<Vec<u8>, RewriteError> {
    let opcode = payload.first().copied().unwrap_or(0xff);
    if opcode != ServerList::OPCODE {
        return Err(RewriteError::NotAServerList(opcode));
    }
    let count = usize::from(payload.get(1).copied().unwrap_or(0));
    if count != replacements.len() {
        return Err(RewriteError::CountMismatch {
            expected: replacements.len(),
            actual: count,
        });
    }
    if payload.len() < HEADER_LEN + count * ENTRY_LEN {
        return Err(RewriteError::Truncated {
            len: payload.len(),
            count,
        });
    }
    let mut out = payload.to_vec();
    for (i, replacement) in replacements.iter().enumerate() {
        let Some((ip, port)) = replacement else { continue };
        let entry = HEADER_LEN + i * ENTRY_LEN;
        out[entry + IP_OFFSET..entry + IP_OFFSET + 4].copy_from_slice(&ip.octets());
        out[entry + PORT_OFFSET..entry + PORT_OFFSET + 4].copy_from_slice(&i32::from(*port).to_le_bytes());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use l2_net::{Decode, Encode, PacketReader};
    use l2_protocol_login::server::{ServerCharInfo, ServerEntry};

    fn entry(id: u8) -> ServerEntry {
        ServerEntry {
            server_id: id,
            ip: [10, 0, 0, id],
            port: 7777,
            age_limit: 15,
            pvp: true,
            current_players: 123,
            max_players: 3000,
            up: true,
            server_type: 0x41,
            brackets: true,
        }
    }

    fn list(n: u8) -> ServerList {
        ServerList {
            last_server: 2,
            servers: (1..=n).map(entry).collect(),
            char_info: Some(vec![ServerCharInfo {
                server_id: 1,
                chars: 3,
                seconds_until_delete: vec![60, 120],
            }]),
        }
    }

    fn decode(bytes: &[u8]) -> ServerList {
        ServerList::decode(&mut PacketReader::new(&bytes[1..])).unwrap()
    }

    #[test]
    fn entry_len_matches_the_real_encoding() {
        let one = list(1).to_bytes().len();
        let three = list(3).to_bytes().len();
        assert_eq!(three - one, 2 * ENTRY_LEN);
    }

    #[test]
    fn only_ip_and_port_change() {
        let original = list(3);
        let bytes = original.to_bytes();
        let patched = patch_server_list(
            &bytes,
            &[
                Some((Ipv4Addr::new(127, 0, 0, 1), 40000)),
                None,
                Some((Ipv4Addr::new(192, 168, 1, 9), 65535)),
            ],
        )
        .unwrap();
        assert_eq!(patched.len(), bytes.len());

        let mut expected = original;
        expected.servers[0].ip = [127, 0, 0, 1];
        expected.servers[0].port = 40000;
        expected.servers[2].ip = [192, 168, 1, 9];
        expected.servers[2].port = 65535;
        assert_eq!(decode(&patched), expected);
    }

    #[test]
    fn nothing_to_do_is_identity() {
        let bytes = list(2).to_bytes();
        assert_eq!(patch_server_list(&bytes, &[None, None]).unwrap(), bytes);
    }

    #[test]
    fn trailing_bytes_survive() {
        let mut bytes = list(1).to_bytes();
        bytes.extend_from_slice(&[0xaa, 0xbb]);
        let patched = patch_server_list(&bytes, &[Some((Ipv4Addr::LOCALHOST, 1))]).unwrap();
        assert_eq!(&patched[patched.len() - 2..], &[0xaa, 0xbb]);
    }

    #[test]
    fn bad_input_is_an_error() {
        let bytes = list(2).to_bytes();
        assert_eq!(
            patch_server_list(&bytes, &[None]),
            Err(RewriteError::CountMismatch {
                expected: 1,
                actual: 2
            })
        );
        assert_eq!(
            patch_server_list(&[0x03, 0, 0], &[]),
            Err(RewriteError::NotAServerList(3))
        );
        assert_eq!(
            patch_server_list(&[], &[]),
            Err(RewriteError::NotAServerList(0xff))
        );
        assert!(matches!(
            patch_server_list(&bytes[..10], &[None, None]),
            Err(RewriteError::Truncated { .. })
        ));
    }
}
