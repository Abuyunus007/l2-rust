//! Server packets against the real `writeImpl` output of the Java classes
//! (`java-vectors/out/login_packets.json`, direction `login_server_to_client`).
//!
//! The file has no client packets: Java only reads those, so they are tested with hand-written
//! bytes next to the code (`src/client.rs`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fmt::Debug;

use l2_net::{Decode, Encode, PacketReader};
use l2_protocol_login::*;
use l2_testkit::vectors::load;
use l2_testkit::{from_hex, to_hex};
use serde::Deserialize;
use serde_json::{Map, Value};

#[derive(Deserialize)]
struct File {
    packets: Vec<Packet>,
}

#[derive(Deserialize)]
struct Packet {
    name: String,
    direction: String,
    opcode: u8,
    variants: Vec<Variant>,
}

#[derive(Deserialize)]
struct Variant {
    what: String,
    fields: Map<String, Value>,
    payload: String,
}

fn variants(name: &str) -> (u8, Vec<Variant>) {
    let file: File = load("login_packets");
    let packet = file
        .packets
        .into_iter()
        .find(|p| p.name == name)
        .unwrap_or_else(|| panic!("packet {name} missing"));
    assert_eq!(packet.direction, "login_server_to_client");
    (packet.opcode, packet.variants)
}

fn int(fields: &Map<String, Value>, key: &str) -> i64 {
    fields[key]
        .as_i64()
        .unwrap_or_else(|| panic!("{key} is not an int"))
}

fn bytes<const N: usize>(fields: &Map<String, Value>, key: &str) -> [u8; N] {
    from_hex(fields[key].as_str().unwrap()).try_into().unwrap()
}

/// Encoding `expected` gives the Java bytes, and decoding the Java bytes gives `expected`.
fn check<P: Encode + Decode + PartialEq + Debug>(v: &Variant, opcode: u8, expected: P) {
    let java = from_hex(&v.payload);
    assert_eq!(java[0], opcode, "{}", v.what);
    assert_eq!(to_hex(&expected.to_bytes()), v.payload, "encode: {}", v.what);
    let decoded = P::decode(&mut PacketReader::new(&java[1..])).unwrap();
    assert_eq!(decoded, expected, "decode: {}", v.what);
}

#[test]
fn init() {
    let (opcode, all) = variants("Init");
    for v in &all {
        let f = &v.fields;
        let key = from_hex(f["blowfish_key_hex"].as_str().unwrap());
        if key.len() != 16 {
            // Java does not check the key length; the struct only holds the real 16-byte key.
            let java = from_hex(&v.payload);
            assert!(
                Init::decode(&mut PacketReader::new(&java[1..])).is_err(),
                "{}",
                v.what
            );
            continue;
        }
        let expected = Init {
            session_id: int(f, "session_id") as i32,
            protocol: int(f, "protocol_revision") as u32,
            scrambled_modulus: bytes(f, "public_key_hex"),
            gg: Init::GG,
            blowfish_key: key.try_into().unwrap(),
        };
        check(v, opcode, expected);
    }
    assert_eq!(all.len(), 4);
}

#[test]
fn login_fail() {
    let (opcode, all) = variants("LoginFail");
    for v in &all {
        let code = int(&v.fields, "reason") as u8;
        let reason = LoginFailReason::try_from(code).unwrap();
        assert_eq!(reason.name(), v.what);
        check(v, opcode, LoginFail { reason });
    }
    assert_eq!(all.len(), LoginFailReason::ALL.len());
}

#[test]
fn play_fail() {
    let (opcode, all) = variants("PlayFail");
    for v in &all {
        let code = int(&v.fields, "reason") as u8;
        let reason = PlayFailReason::try_from(code).unwrap();
        assert_eq!(reason.name(), v.what);
        check(v, opcode, PlayFail { reason });
    }
    assert_eq!(all.len(), PlayFailReason::ALL.len());
}

#[test]
fn account_kicked() {
    let (opcode, all) = variants("AccountKicked");
    for v in &all {
        let code = int(&v.fields, "reason") as u32;
        let reason = AccountKickedReason::try_from(code).unwrap();
        assert_eq!(reason.name(), v.what);
        check(v, opcode, AccountKicked { reason });
    }
    assert_eq!(all.len(), AccountKickedReason::ALL.len());
}

#[test]
fn login_ok() {
    let (opcode, all) = variants("LoginOk");
    for v in &all {
        let login_ok1 = int(&v.fields, "login_ok1") as i32;
        let login_ok2 = int(&v.fields, "login_ok2") as i32;
        check(v, opcode, LoginOk { login_ok1, login_ok2 });
    }
}

#[test]
fn play_ok() {
    let (opcode, all) = variants("PlayOk");
    for v in &all {
        let play_ok1 = int(&v.fields, "play_ok1") as i32;
        let play_ok2 = int(&v.fields, "play_ok2") as i32;
        check(v, opcode, PlayOk { play_ok1, play_ok2 });
    }
}

#[test]
fn gg_auth() {
    let (opcode, all) = variants("GGAuth");
    for v in &all {
        let response = int(&v.fields, "response") as i32;
        check(v, opcode, GGAuth { response });
    }
}

/// Java `ServerStatus.STATUS_DOWN`: the only status value that is written as "down".
const STATUS_DOWN: i64 = 4;

#[test]
fn server_list() {
    let (opcode, all) = variants("ServerList");
    for v in &all {
        let f = &v.fields;
        let servers = f["servers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| {
                let s = s.as_object().unwrap();
                ServerEntry {
                    server_id: int(s, "server_id") as u8,
                    ip: s["ip"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|b| b.as_i64().unwrap() as u8)
                        .collect::<Vec<_>>()
                        .try_into()
                        .unwrap(),
                    port: int(s, "port") as i32,
                    age_limit: int(s, "age_limit") as u8,
                    pvp: s["pvp"].as_bool().unwrap(),
                    current_players: int(s, "current_players") as u16,
                    max_players: int(s, "max_players") as u16,
                    up: int(s, "status") != STATUS_DOWN,
                    server_type: int(s, "server_type") as i32,
                    brackets: s["brackets"].as_bool().unwrap(),
                }
            })
            .collect();
        // The vectors have no delete timers (they depend on the clock), only server and count.
        let char_info = f["chars_on_servers"].as_array().map(|list| {
            list.iter()
                .map(|c| {
                    let c = c.as_object().unwrap();
                    ServerCharInfo {
                        server_id: int(c, "server_id") as u8,
                        chars: int(c, "chars") as u8,
                        seconds_until_delete: vec![],
                    }
                })
                .collect::<Vec<_>>()
        });
        let expected = ServerList {
            last_server: int(f, "last_server") as u8,
            servers,
            char_info,
        };

        if matches!(&expected.char_info, Some(list) if list.is_empty()) {
            // Java writes the same bytes for a null map and an empty map, so decoding gives `None`.
            let java = from_hex(&v.payload);
            assert_eq!(to_hex(&expected.to_bytes()), v.payload, "encode: {}", v.what);
            let decoded = ServerList::decode(&mut PacketReader::new(&java[1..])).unwrap();
            assert_eq!(
                decoded,
                ServerList {
                    char_info: None,
                    ..expected
                },
                "{}",
                v.what
            );
        } else {
            check(v, opcode, expected);
        }
    }
    assert_eq!(all.len(), 6);
}

/// Every server-to-client packet in the file is covered by a test above.
#[test]
fn all_client_facing_packets_are_covered() {
    let file: File = load("login_packets");
    let names: Vec<_> = file
        .packets
        .iter()
        .filter(|p| p.direction == "login_server_to_client")
        .map(|p| p.name.as_str())
        .collect();
    assert_eq!(
        names,
        [
            "Init",
            "LoginFail",
            "AccountKicked",
            "LoginOk",
            "ServerList",
            "PlayFail",
            "PlayOk",
            "GGAuth"
        ]
    );
}
