//! Shape checks for the committed `java-vectors/out/login_packets.json` file (M1-JV-02).
//!
//! These tests prove that the file parses into the expected structure, that every packet has at
//! least two variants whose payload starts with the opcode, and a few length/prefix spot checks.
//! The packet codecs (M1-LPROTO-*) check the real field-by-field values.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use l2_testkit::vectors::load;
use l2_testkit::{from_hex, to_hex};
use serde::Deserialize;
use serde_json::{Map, Value};

#[derive(Deserialize)]
struct File {
    source: String,
    generated_by: String,
    packets: Vec<Packet>,
    skipped: Vec<Skipped>,
}

#[derive(Deserialize)]
struct Packet {
    name: String,
    java_class: String,
    direction: String,
    opcode: u8,
    variants: Vec<Variant>,
}

#[derive(Deserialize)]
struct Variant {
    what: String,
    fields: Map<String, Value>,
    payload: String,
    #[serde(default)]
    produced_by: Option<String>,
    #[serde(default)]
    clock_pinned: Option<bool>,
}

#[derive(Deserialize)]
struct Skipped {
    name: String,
    reason: String,
}

fn file() -> File {
    load("login_packets")
}

fn packet<'a>(f: &'a File, name: &str) -> &'a Packet {
    f.packets
        .iter()
        .find(|p| p.name == name)
        .unwrap_or_else(|| panic!("packet {name} missing"))
}

fn assert_lower_hex(s: &str) {
    assert!(
        s.len() % 2 == 0
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "not lowercase hex: {s:?}"
    );
}

/// Every packet class the card lists, in file order: login server to client, login server to
/// game server, game server to login server.
const EXPECTED: &[(&str, &str)] = &[
    ("Init", "login_server_to_client"),
    ("LoginFail", "login_server_to_client"),
    ("AccountKicked", "login_server_to_client"),
    ("LoginOk", "login_server_to_client"),
    ("ServerList", "login_server_to_client"),
    ("PlayFail", "login_server_to_client"),
    ("PlayOk", "login_server_to_client"),
    ("GGAuth", "login_server_to_client"),
    ("InitLS", "login_server_to_game_server"),
    ("LoginServerFail", "login_server_to_game_server"),
    ("AuthResponse", "login_server_to_game_server"),
    ("PlayerAuthResponse", "login_server_to_game_server"),
    ("KickPlayer", "login_server_to_game_server"),
    ("RequestCharacters", "login_server_to_game_server"),
    ("ChangePasswordResponse", "login_server_to_game_server"),
    ("BlowFishKey", "game_server_to_login_server"),
    ("AuthRequest", "game_server_to_login_server"),
    ("PlayerInGame", "game_server_to_login_server"),
    ("PlayerLogout", "game_server_to_login_server"),
    ("ChangeAccessLevel", "game_server_to_login_server"),
    ("PlayerAuthRequest", "game_server_to_login_server"),
    ("ServerStatus", "game_server_to_login_server"),
    ("PlayerTracert", "game_server_to_login_server"),
    ("ReplyCharacters", "game_server_to_login_server"),
    ("SendMail", "game_server_to_login_server"),
    ("TempBan", "game_server_to_login_server"),
    ("ChangePassword", "game_server_to_login_server"),
];

#[test]
fn login_packets_shape() {
    let f = file();
    assert_eq!(f.generated_by, "java-vectors");
    assert!(!f.source.is_empty());

    let actual: Vec<(&str, &str)> = f
        .packets
        .iter()
        .map(|p| (p.name.as_str(), p.direction.as_str()))
        .collect();
    assert_eq!(actual, EXPECTED);

    for p in &f.packets {
        assert!(p.java_class.starts_with("org.l2jmobius."), "{}", p.name);
        assert!(p.java_class.ends_with(&p.name), "{}", p.name);
        assert!(p.variants.len() >= 2, "{}: needs 2+ variants", p.name);
        for v in &p.variants {
            assert!(!v.what.is_empty(), "{}", p.name);
            assert!(!v.fields.is_empty(), "{}: {}", p.name, v.what);
            assert_lower_hex(&v.payload);
            let payload = from_hex(&v.payload);
            assert_eq!(
                payload.first().copied(),
                Some(p.opcode),
                "{}: {}: payload must start with the opcode",
                p.name,
                v.what
            );
            // Byte-array fields are hex strings.
            for (k, value) in &v.fields {
                if k.ends_with("_hex") && !value.is_null() {
                    assert_lower_hex(value.as_str().unwrap());
                }
            }
        }
    }

    // Only AuthResponse is replayed, only TempBan has a pinned clock.
    for p in &f.packets {
        for v in &p.variants {
            assert_eq!(
                v.produced_by.as_deref() == Some("replay"),
                p.name == "AuthResponse",
                "{}: {}",
                p.name,
                v.what
            );
            assert_eq!(v.clock_pinned == Some(true), p.name == "TempBan");
        }
    }

    assert!(!f.skipped.is_empty());
    for s in &f.skipped {
        assert!(!s.name.is_empty() && !s.reason.is_empty());
    }
}

#[test]
fn fixed_size_packets_have_the_documented_length() {
    let f = file();
    // opcode + 8 ints + 16 reserved bytes.
    for v in &packet(&f, "LoginOk").variants {
        assert_eq!(from_hex(&v.payload).len(), 1 + 32 + 16);
    }
    // opcode + 2 ints.
    for v in &packet(&f, "PlayOk").variants {
        assert_eq!(from_hex(&v.payload).len(), 9);
    }
    // opcode + 5 ints.
    for v in &packet(&f, "GGAuth").variants {
        assert_eq!(from_hex(&v.payload).len(), 21);
    }
    // opcode + 1 byte reason.
    for name in ["LoginFail", "PlayFail", "LoginServerFail"] {
        for v in &packet(&f, name).variants {
            assert_eq!(from_hex(&v.payload).len(), 2, "{name}: {}", v.what);
        }
    }
    // opcode + int reason.
    for v in &packet(&f, "AccountKicked").variants {
        assert_eq!(from_hex(&v.payload).len(), 5);
    }
}

#[test]
fn init_starts_with_session_id_and_protocol_revision() {
    let f = file();
    for v in &packet(&f, "Init").variants {
        let payload = from_hex(&v.payload);
        let session = i32::try_from(v.fields["session_id"].as_i64().unwrap()).unwrap();
        assert_eq!(payload[1..5], session.to_le_bytes(), "{}", v.what);
        assert_eq!(payload[5..9], 0xc621_i32.to_le_bytes(), "{}", v.what);
        let key = from_hex(v.fields["public_key_hex"].as_str().unwrap());
        assert_eq!(payload[9..9 + key.len()], key[..], "{}", v.what);
        // Four GG ints, Blowfish key, null terminator.
        let bf = from_hex(v.fields["blowfish_key_hex"].as_str().unwrap());
        assert_eq!(payload.len(), 9 + key.len() + 16 + bf.len() + 1, "{}", v.what);
        assert_eq!(payload.last(), Some(&0));
    }
}

/// UTF-16LE code units of `s` plus the 0x0000 terminator (what `writeString` puts on the wire).
fn utf16_string(s: Option<&str>) -> Vec<u8> {
    let mut out: Vec<u8> = s
        .unwrap_or("")
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    out.extend([0, 0]);
    out
}

#[test]
fn single_string_packets_are_opcode_plus_utf16_string() {
    let f = file();
    for name in ["KickPlayer", "RequestCharacters", "PlayerLogout"] {
        let p = packet(&f, name);
        for v in &p.variants {
            let account = v.fields["account"].as_str();
            let mut expected = vec![p.opcode];
            expected.extend(utf16_string(account));
            assert_eq!(to_hex(&expected), v.payload, "{name}: {}", v.what);
        }
    }
}

#[test]
fn variants_cover_non_ascii_and_extreme_values() {
    let f = file();
    // At least one variant of the string packets carries a non-ASCII string and a surrogate pair.
    let kick = packet(&f, "KickPlayer");
    let accounts: Vec<&str> = kick
        .variants
        .iter()
        .filter_map(|v| v.fields["account"].as_str())
        .collect();
    assert!(accounts.iter().any(|a| !a.is_ascii()));
    assert!(accounts.iter().any(|a| a.chars().any(|c| c.len_utf16() == 2)));

    // Negative and extreme ints survive as signed JSON numbers.
    let login_ok = packet(&f, "LoginOk");
    assert!(
        login_ok
            .variants
            .iter()
            .any(|v| v.fields["login_ok1"].as_i64().unwrap() < 0)
    );
    let change = packet(&f, "ChangeAccessLevel");
    assert!(
        change
            .variants
            .iter()
            .any(|v| v.fields["access"].as_i64() == Some(i64::from(i32::MIN)))
    );
}
