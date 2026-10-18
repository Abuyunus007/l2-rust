//! Link packets against the real Java output (`java-vectors/out/login_packets.json`).
//!
//! Two directions: `login_server_to_game_server` (LS writes, GS reads) and
//! `game_server_to_login_server` (GS writes, LS reads). For each variant the struct must encode
//! to the Java bytes, and decoding the Java bytes must give the struct back.
//!
//! Java's narrowing is applied to the expected values here: `writeByte` keeps the low byte,
//! `writeShort` the low 16 bits, and a null string or list becomes "" or empty.
//!
//! `AuthResponse` vectors are a replay (`produced_by: replay`): the Java class needs the game
//! server table, so the bytes come from the same format, not from the class itself.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fmt::Debug;

use l2_net::{Decode, Encode, PacketReader};
use l2_protocol_login::*;
use l2_testkit::vectors::load;
use l2_testkit::{from_hex, to_hex};
use serde::Deserialize;
use serde_json::{Map, Value};

const LS_TO_GS: &str = "login_server_to_game_server";
const GS_TO_LS: &str = "game_server_to_login_server";

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

fn variants(name: &str, direction: &str) -> (u8, Vec<Variant>) {
    let file: File = load("login_packets");
    let packet = file
        .packets
        .into_iter()
        .find(|p| p.name == name)
        .unwrap_or_else(|| panic!("packet {name} missing"));
    assert_eq!(packet.direction, direction, "{name}");
    (packet.opcode, packet.variants)
}

fn int(f: &Map<String, Value>, key: &str) -> i64 {
    f[key].as_i64().unwrap_or_else(|| panic!("{key} is not an int"))
}

/// A Java string. `null` becomes "" (Java `writeString(null)` writes only the terminator).
fn text(f: &Map<String, Value>, key: &str) -> String {
    f.get(key).and_then(Value::as_str).unwrap_or("").to_owned()
}

fn flag(f: &Map<String, Value>, key: &str) -> bool {
    f[key].as_bool().unwrap_or_else(|| panic!("{key} is not a bool"))
}

/// A byte array in hex. `null` becomes empty.
fn bytes(f: &Map<String, Value>, key: &str) -> Vec<u8> {
    f.get(key)
        .and_then(Value::as_str)
        .map(from_hex)
        .unwrap_or_default()
}

/// A list of strings. `null` list becomes empty, and `null` entries become "".
fn strings(f: &Map<String, Value>, key: &str) -> Vec<String> {
    f.get(key)
        .and_then(Value::as_array)
        .map(|list| list.iter().map(|v| v.as_str().unwrap_or("").to_owned()).collect())
        .unwrap_or_default()
}

/// Encoding `expected` gives the Java bytes, and decoding the Java bytes gives `expected`.
fn check<P: Encode + Decode + PartialEq + Debug>(v: &Variant, opcode: u8, expected: P) {
    let java = from_hex(&v.payload);
    assert_eq!(java[0], opcode, "{}", v.what);
    assert_eq!(to_hex(&expected.to_bytes()), v.payload, "encode: {}", v.what);
    let decoded = P::decode(&mut PacketReader::new(&java[1..])).unwrap();
    assert_eq!(decoded, expected, "decode: {}", v.what);
}

// ---------------------------------------------------------------------------------------------
// LS -> GS
// ---------------------------------------------------------------------------------------------

#[test]
fn init_ls() {
    let (opcode, all) = variants("InitLS", LS_TO_GS);
    for v in &all {
        let f = &v.fields;
        let expected = InitLS {
            protocol_revision: int(f, "protocol_revision") as i32,
            public_key: bytes(f, "public_key_hex"),
        };
        check(v, opcode, expected);
    }
    assert_eq!(all.len(), 4);
}

#[test]
fn login_server_fail() {
    let (opcode, all) = variants("LoginServerFail", LS_TO_GS);
    for v in &all {
        let expected = LoginServerFail {
            reason: int(&v.fields, "reason") as u8,
        };
        check(v, opcode, expected);
    }
    assert_eq!(all.len(), 8);
}

#[test]
fn auth_response() {
    let (opcode, all) = variants("AuthResponse", LS_TO_GS);
    for v in &all {
        let f = &v.fields;
        let expected = AuthResponse {
            server_id: int(f, "server_id") as u8,
            server_name: text(f, "server_name"),
        };
        check(v, opcode, expected);
    }
    assert_eq!(all.len(), 5);
}

#[test]
fn player_auth_response() {
    let (opcode, all) = variants("PlayerAuthResponse", LS_TO_GS);
    for v in &all {
        let f = &v.fields;
        let expected = PlayerAuthResponse {
            account: text(f, "account"),
            authed: flag(f, "response"),
        };
        check(v, opcode, expected);
    }
    assert_eq!(all.len(), 4);
}

#[test]
fn kick_player() {
    let (opcode, all) = variants("KickPlayer", LS_TO_GS);
    for v in &all {
        let expected = KickPlayer {
            account: text(&v.fields, "account"),
        };
        check(v, opcode, expected);
    }
    assert_eq!(all.len(), 7);
}

#[test]
fn request_characters() {
    let (opcode, all) = variants("RequestCharacters", LS_TO_GS);
    for v in &all {
        let expected = RequestCharacters {
            account: text(&v.fields, "account"),
        };
        check(v, opcode, expected);
    }
    assert_eq!(all.len(), 7);
}

#[test]
fn change_password_response() {
    let (opcode, all) = variants("ChangePasswordResponse", LS_TO_GS);
    for v in &all {
        let f = &v.fields;
        let expected = ChangePasswordResponse {
            character_name: text(f, "character_name"),
            message: text(f, "message"),
        };
        check(v, opcode, expected);
    }
    assert_eq!(all.len(), 4);
}

// ---------------------------------------------------------------------------------------------
// GS -> LS
// ---------------------------------------------------------------------------------------------

#[test]
fn blow_fish_key() {
    let (opcode, all) = variants("BlowFishKey", GS_TO_LS);
    for v in &all {
        // The block is RSA output from Java, so it is compared as bytes. The RSA step itself is
        // not part of this packet (see `BlowFishKey` docs).
        let java = from_hex(&v.payload);
        let expected = BlowFishKey {
            encrypted_key: java[5..].to_vec(),
        };
        check(v, opcode, expected);
    }
    assert_eq!(all.len(), 4);
}

#[test]
fn game_server_auth() {
    let (opcode, all) = variants("AuthRequest", GS_TO_LS);
    for v in &all {
        let f = &v.fields;
        let mut subnets = strings(f, "subnets");
        let mut hosts = strings(f, "hosts");
        // Java writes only min(subnets, hosts) pairs, so the expected lists are cut to match.
        let pairs = subnets.len().min(hosts.len());
        subnets.truncate(pairs);
        hosts.truncate(pairs);
        let expected = GameServerAuth {
            desired_id: int(f, "id") as u8,
            accept_alternate: flag(f, "accept_alternate"),
            reserve_host: flag(f, "reserve_host"),
            port: int(f, "port") as u16,
            max_players: int(f, "max_players") as i32,
            hex_id: bytes(f, "hex_id_hex"),
            subnets,
            hosts,
        };
        check(v, opcode, expected);
    }
    assert_eq!(all.len(), 6);
}

#[test]
fn player_in_game() {
    let (opcode, all) = variants("PlayerInGame", GS_TO_LS);
    for v in &all {
        let expected = PlayerInGame {
            accounts: strings(&v.fields, "players"),
        };
        check(v, opcode, expected);
    }
    assert_eq!(all.len(), 6);
}

#[test]
fn player_logout() {
    let (opcode, all) = variants("PlayerLogout", GS_TO_LS);
    for v in &all {
        let expected = PlayerLogout {
            account: text(&v.fields, "account"),
        };
        check(v, opcode, expected);
    }
    assert_eq!(all.len(), 7);
}

#[test]
fn change_access_level() {
    let (opcode, all) = variants("ChangeAccessLevel", GS_TO_LS);
    for v in &all {
        let f = &v.fields;
        let expected = ChangeAccessLevel {
            account: text(f, "player"),
            level: int(f, "access") as i32,
        };
        check(v, opcode, expected);
    }
    assert_eq!(all.len(), 4);
}

#[test]
fn player_auth_request() {
    let (opcode, all) = variants("PlayerAuthRequest", GS_TO_LS);
    for v in &all {
        let f = &v.fields;
        // The null case has `session_key: null` instead of the four ints: Java writes zeros.
        let key_int = |key: &str| {
            if f.contains_key(key) {
                int(f, key) as i32
            } else {
                0
            }
        };
        let expected = PlayerAuthRequest {
            account: text(f, "account"),
            play_ok1: key_int("play_ok1"),
            play_ok2: key_int("play_ok2"),
            login_ok1: key_int("login_ok1"),
            login_ok2: key_int("login_ok2"),
        };
        check(v, opcode, expected);
    }
    assert_eq!(all.len(), 3);
}

#[test]
fn server_status() {
    let (opcode, all) = variants("ServerStatus", GS_TO_LS);
    for v in &all {
        let attributes = v.fields["attributes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| {
                let a = a.as_object().unwrap();
                (int(a, "id") as i32, int(a, "value") as i32)
            })
            .collect();
        check(v, opcode, ServerStatus { attributes });
    }
    assert_eq!(all.len(), 5);
}

#[test]
fn player_tracert() {
    let (opcode, all) = variants("PlayerTracert", GS_TO_LS);
    for v in &all {
        let f = &v.fields;
        let expected = PlayerTracert {
            account: text(f, "account"),
            pc_ip: text(f, "pc_ip"),
            hop1: text(f, "hop1"),
            hop2: text(f, "hop2"),
            hop3: text(f, "hop3"),
            hop4: text(f, "hop4"),
        };
        check(v, opcode, expected);
    }
    assert_eq!(all.len(), 4);
}

#[test]
fn reply_characters() {
    let (opcode, all) = variants("ReplyCharacters", GS_TO_LS);
    for v in &all {
        let f = &v.fields;
        // Java writes a null entry as 0. A null list is written as count 0.
        let delete_times = f
            .get("times")
            .and_then(Value::as_array)
            .map(|list| list.iter().map(|t| t.as_i64().unwrap_or(0)).collect())
            .unwrap_or_default();
        let expected = ReplyCharacters {
            account: text(f, "account"),
            chars: int(f, "chars") as u8,
            delete_times,
        };
        check(v, opcode, expected);
    }
    assert_eq!(all.len(), 4);
}

#[test]
fn send_mail() {
    let (opcode, all) = variants("SendMail", GS_TO_LS);
    for v in &all {
        let f = &v.fields;
        let expected = SendMail {
            account: text(f, "account"),
            mail_id: text(f, "mail_id"),
            args: strings(f, "args"),
        };
        check(v, opcode, expected);
    }
    assert_eq!(all.len(), 4);
}

#[test]
fn request_temp_ban() {
    let (opcode, all) = variants("TempBan", GS_TO_LS);
    for v in &all {
        let f = &v.fields;
        // `expire_millis` is the value on the wire. The clock is pinned in the vectors.
        let expected = RequestTempBan {
            account: text(f, "account"),
            ip: text(f, "ip"),
            ban_time: int(f, "expire_millis"),
            reason: None,
        };
        check(v, opcode, expected);
    }
    assert_eq!(all.len(), 6);
}

#[test]
fn change_password() {
    let (opcode, all) = variants("ChangePassword", GS_TO_LS);
    for v in &all {
        let f = &v.fields;
        let expected = ChangePassword {
            account: text(f, "account"),
            character_name: text(f, "character"),
            old_pass: text(f, "old_pass"),
            new_pass: text(f, "new_pass"),
        };
        check(v, opcode, expected);
    }
    assert_eq!(all.len(), 4);
}

/// Every link packet in the file is covered by a test above. A new Java packet added to the
/// vectors fails here until it gets a struct and a test.
#[test]
fn all_link_packets_are_covered() {
    let file: File = load("login_packets");
    let names = |direction: &str| -> Vec<String> {
        file.packets
            .iter()
            .filter(|p| p.direction == direction)
            .map(|p| p.name.clone())
            .collect()
    };
    assert_eq!(
        names(LS_TO_GS),
        [
            "InitLS",
            "LoginServerFail",
            "AuthResponse",
            "PlayerAuthResponse",
            "KickPlayer",
            "RequestCharacters",
            "ChangePasswordResponse"
        ]
    );
    assert_eq!(
        names(GS_TO_LS),
        [
            "BlowFishKey",
            "AuthRequest",
            "PlayerInGame",
            "PlayerLogout",
            "ChangeAccessLevel",
            "PlayerAuthRequest",
            "ServerStatus",
            "PlayerTracert",
            "ReplyCharacters",
            "SendMail",
            "TempBan",
            "ChangePassword"
        ]
    );
}
