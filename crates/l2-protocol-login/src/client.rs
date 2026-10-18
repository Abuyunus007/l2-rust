//! Packets the game client sends to the login server.
//!
//! Same shape as the server packets. `Decode` is what the login server uses; `Encode` exists for
//! test clients and the proxy. Java's `readImpl` returns `false` when the packet is too short, so
//! the minimum lengths below are enforced by reading every field: a short packet gives
//! [`PacketError::Truncated`]. Bytes after the last field are ignored (the client pads packets).

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

// Java: loginserver/network/clientpackets/AuthGameGuard.java
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthGameGuard {
    pub session_id: i32,
}

impl AuthGameGuard {
    pub const OPCODE: u8 = 0x07;
}

impl Encode for AuthGameGuard {
    fn encode(&self, w: &mut PacketWriter) {
        w.u8(Self::OPCODE).i32(self.session_id);
        w.bytes(&[0; 16]); // four reserved ints, Java ignores them
    }
}

impl Decode for AuthGameGuard {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        // Java: `remaining() >= 20` (session id + 4 reserved ints).
        let session_id = r.i32()?;
        r.skip(16)?;
        Ok(Self { session_id })
    }
}

// Java: loginserver/network/clientpackets/RequestAuthLogin.java
/// The credentials are in RSA blocks of 128 bytes. Decrypting them and reading the account name
/// and password is the login server's job: see [`credentials_from_decrypted`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestAuthLogin {
    /// Old auth method: one block.
    Rsa128 { block: [u8; 128] },
    /// New auth method (H5 client): two blocks, each decrypted on its own.
    Rsa256 { block1: [u8; 128], block2: [u8; 128] },
}

impl RequestAuthLogin {
    pub const OPCODE: u8 = 0x00;
}

impl Encode for RequestAuthLogin {
    fn encode(&self, w: &mut PacketWriter) {
        w.u8(Self::OPCODE);
        match self {
            Self::Rsa128 { block } => {
                w.bytes(block);
            }
            Self::Rsa256 { block1, block2 } => {
                w.bytes(block1).bytes(block2);
            }
        }
    }
}

impl Decode for RequestAuthLogin {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        // Java: `remaining() >= 256` is the new method, `>= 128` the old one, else the read fails.
        if r.remaining() >= 256 {
            Ok(Self::Rsa256 {
                block1: r.byte_array()?,
                block2: r.byte_array()?,
            })
        } else {
            Ok(Self::Rsa128 {
                block: r.byte_array()?,
            })
        }
    }
}

/// Account name and password from the RSA-decrypted credentials (Java `RequestAuthLogin.run`).
///
/// `decrypted` is the concatenated plaintext of the blocks: 256 bytes for the new method, 128 for
/// the old one. A slice of 256 bytes or more is read as the new method, anything shorter as the
/// old one. Fields that reach past the end of the slice are cut short instead of panicking
/// (Java throws and drops the packet).
///
/// - new: user = `trim(dec[0x4E..+50]) + trim(dec[0xCE..+14])`, password = `trim(dec[0xDC..+16])`
/// - old: user = `trim(dec[0x5E..+14])`, password = `trim(dec[0x6C..+16])`
pub fn credentials_from_decrypted(decrypted: &[u8]) -> (String, String) {
    if decrypted.len() >= 256 {
        let user = field(decrypted, 0x4E, 50) + &field(decrypted, 0xCE, 14);
        (user, field(decrypted, 0xDC, 16))
    } else {
        (field(decrypted, 0x5E, 14), field(decrypted, 0x6C, 16))
    }
}

/// Java `new String(bytes, offset, len).trim()`.
///
/// NOTE(java-quirk): `String.trim()` strips every char `<= ' '` (control characters and the
/// RSA zero padding included), unlike Rust's `str::trim`, which only strips whitespace.
/// Bytes are read as UTF-8, Java's default charset on current JDKs; invalid bytes become U+FFFD.
fn field(buf: &[u8], offset: usize, len: usize) -> String {
    let end = buf.len().min(offset + len);
    let start = offset.min(end);
    String::from_utf8_lossy(&buf[start..end])
        .trim_matches(|c: char| c <= ' ')
        .to_owned()
}

// Java: loginserver/network/clientpackets/RequestServerList.java
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestServerList {
    pub login_ok1: i32,
    pub login_ok2: i32,
}

impl RequestServerList {
    pub const OPCODE: u8 = 0x05;
}

impl Encode for RequestServerList {
    fn encode(&self, w: &mut PacketWriter) {
        w.u8(Self::OPCODE).i32(self.login_ok1).i32(self.login_ok2);
    }
}

impl Decode for RequestServerList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        // Java: `remaining() >= 8`. The javadoc mentions a third byte, but Java never reads it.
        Ok(Self {
            login_ok1: r.i32()?,
            login_ok2: r.i32()?,
        })
    }
}

// Java: loginserver/network/clientpackets/RequestServerLogin.java
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestServerLogin {
    pub login_ok1: i32,
    pub login_ok2: i32,
    /// NOTE(java-quirk): Java reads this with the signed `readByte()`, so ids above 127 become
    /// negative there and match no game server. We keep the raw byte; callers that mirror the
    /// lookup should treat ids above 127 as unknown.
    pub server_id: u8,
}

impl RequestServerLogin {
    pub const OPCODE: u8 = 0x02;
}

impl Encode for RequestServerLogin {
    fn encode(&self, w: &mut PacketWriter) {
        w.u8(Self::OPCODE)
            .i32(self.login_ok1)
            .i32(self.login_ok2)
            .u8(self.server_id);
    }
}

impl Decode for RequestServerLogin {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        // Java: `remaining() >= 9`.
        Ok(Self {
            login_ok1: r.i32()?,
            login_ok2: r.i32()?,
            server_id: r.u8()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// decode(encode(x)) == x, and the opcode is the first byte.
    fn round_trip<P: Encode + Decode + PartialEq + std::fmt::Debug>(packet: P, opcode: u8) {
        let bytes = packet.to_bytes();
        assert_eq!(bytes[0], opcode);
        let decoded = P::decode(&mut PacketReader::new(&bytes[1..])).unwrap();
        assert_eq!(decoded, packet);
    }

    fn decode<P: Decode>(body: &[u8]) -> Result<P, PacketError> {
        P::decode(&mut PacketReader::new(body))
    }

    #[test]
    fn auth_game_guard() {
        round_trip(AuthGameGuard { session_id: -123 }, AuthGameGuard::OPCODE);
        let mut body = 0x0102_0304_i32.to_le_bytes().to_vec();
        body.extend([0xAA; 16]); // the reserved ints are ignored, whatever they hold
        assert_eq!(decode::<AuthGameGuard>(&body).unwrap().session_id, 0x0102_0304);
        body.extend([0xFF; 7]); // trailing padding is fine
        assert!(decode::<AuthGameGuard>(&body).is_ok());
        assert!(decode::<AuthGameGuard>(&body[..19]).is_err());
    }

    #[test]
    fn request_auth_login_variants() {
        let rsa128 = RequestAuthLogin::Rsa128 { block: [1; 128] };
        let rsa256 = RequestAuthLogin::Rsa256 {
            block1: [2; 128],
            block2: [3; 128],
        };
        round_trip(rsa128.clone(), RequestAuthLogin::OPCODE);
        round_trip(rsa256, RequestAuthLogin::OPCODE);

        // Padding after one block is ignored; 256+ bytes mean the new method.
        assert_eq!(decode::<RequestAuthLogin>(&[1; 255]).unwrap(), rsa128);
        assert!(matches!(
            decode::<RequestAuthLogin>(&[1; 300]).unwrap(),
            RequestAuthLogin::Rsa256 { .. }
        ));
        assert!(decode::<RequestAuthLogin>(&[1; 127]).is_err());
        assert!(decode::<RequestAuthLogin>(&[]).is_err());
    }

    /// 256 bytes of RSA plaintext, zero padded, with the fields Java reads.
    fn plaintext_256(user_a: &[u8], user_b: &[u8], password: &[u8]) -> Vec<u8> {
        let mut dec = vec![0; 256];
        dec[0x4E..0x4E + user_a.len()].copy_from_slice(user_a);
        dec[0xCE..0xCE + user_b.len()].copy_from_slice(user_b);
        dec[0xDC..0xDC + password.len()].copy_from_slice(password);
        dec
    }

    #[test]
    fn credentials_new_method() {
        let dec = plaintext_256(b"player1", b"", b"secret");
        assert_eq!(
            credentials_from_decrypted(&dec),
            ("player1".into(), "secret".into())
        );

        // A name longer than 50 bytes continues at 0xCE (up to 14 more).
        let long = [b'a'; 50];
        let dec = plaintext_256(&long, b"bcd", b"pw");
        let (user, password) = credentials_from_decrypted(&dec);
        assert_eq!(user, format!("{}bcd", "a".repeat(50)));
        assert_eq!(password, "pw");
    }

    #[test]
    fn credentials_old_method() {
        let mut dec = vec![0; 128];
        dec[0x5E..0x5E + 5].copy_from_slice(b"alice");
        dec[0x6C..0x6C + 3].copy_from_slice(b"pwd");
        assert_eq!(credentials_from_decrypted(&dec), ("alice".into(), "pwd".into()));
    }

    #[test]
    fn credentials_trim_like_java() {
        // Java trim() strips every char <= ' ', also NUL and other control characters.
        let dec = plaintext_256(b"\x01 bob \x02", b"", b"\t pass\x1f\0");
        assert_eq!(credentials_from_decrypted(&dec), ("bob".into(), "pass".into()));
        // Only the ends are trimmed; inner characters stay.
        let dec = plaintext_256(b"a b\0c", b"", b"");
        assert_eq!(credentials_from_decrypted(&dec).0, "a b\0c");
        // Non-ASCII UTF-8 survives.
        let dec = plaintext_256("Jörg".as_bytes(), b"", b"x");
        assert_eq!(credentials_from_decrypted(&dec).0, "Jörg");
    }

    #[test]
    fn credentials_never_panic_on_odd_lengths() {
        for len in [0, 1, 0x5E, 0x6D, 127, 129, 255] {
            let _ = credentials_from_decrypted(&vec![b'x'; len]);
        }
        assert_eq!(credentials_from_decrypted(&[b'x'; 0x60]).0, "xx");
    }

    #[test]
    fn request_server_list() {
        round_trip(
            RequestServerList {
                login_ok1: 1,
                login_ok2: -1,
            },
            RequestServerList::OPCODE,
        );
        assert!(decode::<RequestServerList>(&[0; 7]).is_err());
        // Java reads 8 bytes and ignores the rest.
        assert!(decode::<RequestServerList>(&[0; 8]).is_ok());
        assert!(decode::<RequestServerList>(&[0; 20]).is_ok());
    }

    #[test]
    fn request_server_login() {
        round_trip(
            RequestServerLogin {
                login_ok1: i32::MIN,
                login_ok2: 7,
                server_id: 255,
            },
            RequestServerLogin::OPCODE,
        );
        assert!(decode::<RequestServerLogin>(&[0; 8]).is_err());
        assert!(decode::<RequestServerLogin>(&[0; 9]).is_ok());
        let packet = decode::<RequestServerLogin>(&[1, 0, 0, 0, 2, 0, 0, 0, 9, 0xEE, 0xEE]).unwrap();
        assert_eq!(
            packet,
            RequestServerLogin {
                login_ok1: 1,
                login_ok2: 2,
                server_id: 9
            }
        );
    }
}
