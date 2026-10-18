//! Comparing JSONL captures of two login server sessions, and replaying a recorded client.
//!
//! The M1 acceptance asks for "the same logs/decisions as Java for the same inputs". The proxy
//! (`l2-proxy`) records every login packet of a session as a [`Record`]. This module compares two
//! such recordings by **packet type and fields**, ignoring what is random per session (session
//! id, Blowfish key, RSA modulus, the login/play key pairs) and what depends on the machine
//! (addresses, ports).
//!
//! - [`compare`] / [`assert_same_shape`]: the comparison.
//! - [`turns`] and [`replay`]: drive a [`FakeClient`] through the c2s packets of a recorded
//!   connection, so the same conversation can be recorded again against another login server.
//!
//! Typical use (see `crates/login-server/tests/e2e.rs`):
//! 1. a capture recorded with the Java login server (`fixtures/captures/`, credentials are never
//!    in it) is the **expected** side;
//! 2. [`replay`] drives the Rust login server (behind an in-process `l2_proxy::Proxy`) through the
//!    same conversation and the proxy's records are the **actual** side;
//! 3. [`assert_same_shape`] compares them.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::net::SocketAddr;
use std::path::Path;
use std::time::Duration;

use l2_net::{Decode, PacketReader};
use l2_protocol_login::server::LoginOk;
use l2_proxy::{CaptureReader, Channel, Dir, Record};
use serde_json::Value;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::fake_login_client::FakeClient;

/// What to ignore when comparing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompareOptions {
    /// Field names (at any depth) whose *values* are ignored. The field must still be present
    /// with the same JSON type.
    pub ignore_values: BTreeSet<String>,
    /// Also require the same payload length (`raw` hex length). Catches a wrong field layout
    /// that decodes to the same fields.
    pub compare_raw_length: bool,
}

impl Default for CompareOptions {
    /// Ignores everything that is random per session or depends on the machine.
    fn default() -> Self {
        Self {
            ignore_values: [
                // Random per connection (Init, AuthGameGuard, GGAuth).
                "session_id",
                "response",
                "scrambled_modulus",
                "blowfish_key",
                // Random per login (LoginOk, RequestServerList, RequestServerLogin, PlayOk).
                "login_ok1",
                "login_ok2",
                "play_ok1",
                "play_ok2",
                // Where the game server is reachable, and how many players are on it.
                "ip",
                "port",
                "proxy_ip",
                "proxy_port",
                "players",
                "max_players",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            compare_raw_length: true,
        }
    }
}

impl CompareOptions {
    /// Also ignores the values of these fields.
    pub fn ignoring(mut self, fields: &[&str]) -> Self {
        self.ignore_values.extend(fields.iter().map(|f| (*f).to_owned()));
        self
    }
}

/// One packet reduced to what is compared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shape {
    pub dir: Dir,
    pub opcode: u8,
    pub name: String,
    /// `fields` with the ignored values replaced by their JSON type (`"<number>"`).
    pub fields: Option<Value>,
    /// Length of the payload in bytes (`None` when not compared).
    pub raw_length: Option<usize>,
}

impl std::fmt::Display for Shape {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let dir = match self.dir {
            Dir::C2s => "c2s",
            Dir::S2c => "s2c",
        };
        write!(f, "{dir} 0x{:02X} {}", self.opcode, self.name)?;
        if let Some(fields) = &self.fields {
            write!(f, " {fields}")?;
        }
        if let Some(len) = self.raw_length {
            write!(f, " ({len} bytes)")?;
        }
        Ok(())
    }
}

fn type_tag(value: &Value) -> &'static str {
    match value {
        Value::Null => "<null>",
        Value::Bool(_) => "<bool>",
        Value::Number(_) => "<number>",
        Value::String(_) => "<string>",
        Value::Array(_) => "<array>",
        Value::Object(_) => "<object>",
    }
}

fn mask(value: &Value, ignore: &BTreeSet<String>) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, v)| {
                    let masked = if ignore.contains(key) {
                        Value::String(type_tag(v).to_owned())
                    } else {
                        mask(v, ignore)
                    };
                    (key.clone(), masked)
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(|v| mask(v, ignore)).collect()),
        other => other.clone(),
    }
}

/// Reduces a record to its [`Shape`].
pub fn shape_of(record: &Record, options: &CompareOptions) -> Shape {
    Shape {
        dir: record.dir,
        opcode: record.opcode,
        name: record.name.clone(),
        fields: record.fields.as_ref().map(|f| mask(f, &options.ignore_values)),
        raw_length: options.compare_raw_length.then_some(record.raw.len() / 2),
    }
}

/// The records of `channel` grouped by connection, connections in the order they first appear.
pub fn connections(records: &[Record], channel: Channel) -> Vec<Vec<&Record>> {
    let mut order: Vec<u64> = Vec::new();
    for record in records.iter().filter(|r| r.channel == channel) {
        if !order.contains(&record.conn) {
            order.push(record.conn);
        }
    }
    order
        .into_iter()
        .map(|conn| {
            records
                .iter()
                .filter(|r| r.channel == channel && r.conn == conn)
                .collect()
        })
        .collect()
}

/// Compares the `channel` packets of two recordings. The result lists every difference; empty
/// means the same packet types and fields in the same order on every connection.
pub fn compare(
    expected: &[Record],
    actual: &[Record],
    channel: Channel,
    options: &CompareOptions,
) -> Vec<String> {
    let expected = connections(expected, channel);
    let actual = connections(actual, channel);
    let mut differences = Vec::new();
    if expected.len() != actual.len() {
        differences.push(format!(
            "expected {} connection(s), got {}",
            expected.len(),
            actual.len()
        ));
    }
    for (index, (exp, act)) in expected.iter().zip(&actual).enumerate() {
        let exp: Vec<Shape> = exp.iter().map(|r| shape_of(r, options)).collect();
        let act: Vec<Shape> = act.iter().map(|r| shape_of(r, options)).collect();
        for position in 0..exp.len().max(act.len()) {
            match (exp.get(position), act.get(position)) {
                (Some(e), Some(a)) if e == a => {}
                (Some(e), Some(a)) => differences.push(format!(
                    "connection {index}, packet {position}: expected `{e}`, got `{a}`"
                )),
                (Some(e), None) => differences.push(format!(
                    "connection {index}, packet {position}: expected `{e}`, but the session ended"
                )),
                (None, Some(a)) => differences.push(format!(
                    "connection {index}, packet {position}: unexpected extra packet `{a}`"
                )),
                (None, None) => {}
            }
        }
    }
    differences
}

/// Panics with every difference when the two recordings do not have the same shape.
pub fn assert_same_shape(expected: &[Record], actual: &[Record], channel: Channel, options: &CompareOptions) {
    let differences = compare(expected, actual, channel, options);
    if !differences.is_empty() {
        let mut message = String::from("the captures differ:\n");
        for difference in &differences {
            let _ = writeln!(message, "  - {difference}");
        }
        panic!("{message}");
    }
}

/// Reads a JSONL capture file.
pub fn load_capture(path: impl AsRef<Path>) -> Vec<Record> {
    let path = path.as_ref();
    CaptureReader::open(path)
        .unwrap_or_else(|e| panic!("cannot open capture {}: {e}", path.display()))
        .read_all()
        .unwrap_or_else(|e| panic!("cannot read capture {}: {e}", path.display()))
}

/// Takes every record the proxy has sent so far. The proxy records a packet *before* it forwards
/// it, so once a client has read a reply, the record of that reply is already here.
pub fn drain(records: &mut UnboundedReceiver<Record>) -> Vec<Record> {
    let mut collected = Vec::new();
    while let Ok(record) = records.try_recv() {
        collected.push(record);
    }
    collected
}

// ---------------------------------------------------------------------------------------------
// Replay
// ---------------------------------------------------------------------------------------------

/// One thing a client does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientStep {
    /// `AuthGameGuard` with the session id of this session's `Init`.
    GameGuard,
    /// `RequestAuthLogin`. The credentials are never in a capture; see [`ReplayAccount`].
    Login,
    /// `RequestServerList` with this session's `LoginOk` pair.
    ServerList,
    /// `RequestServerLogin` for this server id.
    ServerLogin(u8),
    /// Any other client packet, sent from its recorded plaintext.
    Raw(Vec<u8>),
}

/// A client step and how many packets the server answered with in the recording.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Turn {
    /// `None` for the opening turn: the server's first packets (`Init`) before the client says
    /// anything.
    pub step: Option<ClientStep>,
    /// Names of the server packets that followed.
    pub replies: Vec<String>,
}

fn step_of(record: &Record) -> ClientStep {
    let field = |name: &str| {
        record
            .fields
            .as_ref()
            .and_then(|f| f.get(name))
            .and_then(Value::as_u64)
    };
    match record.name.as_str() {
        "AuthGameGuard" => ClientStep::GameGuard,
        "RequestAuthLogin" => ClientStep::Login,
        "RequestServerList" => ClientStep::ServerList,
        "RequestServerLogin" => match field("server_id").and_then(|id| u8::try_from(id).ok()) {
            Some(id) => ClientStep::ServerLogin(id),
            None => ClientStep::Raw(record.raw_bytes().unwrap_or_default()),
        },
        _ => ClientStep::Raw(record.raw_bytes().unwrap_or_default()),
    }
}

/// Splits connection number `conn` of a recording (`channel` login) into [`Turn`]s.
pub fn turns(records: &[Record], conn: u64) -> Vec<Turn> {
    let mut result = vec![Turn {
        step: None,
        replies: Vec::new(),
    }];
    for record in records
        .iter()
        .filter(|r| r.channel == Channel::Login && r.conn == conn)
    {
        match record.dir {
            Dir::C2s => result.push(Turn {
                step: Some(step_of(record)),
                replies: Vec::new(),
            }),
            Dir::S2c => {
                if let Some(turn) = result.last_mut() {
                    turn.replies.push(record.name.clone());
                }
            }
        }
    }
    result
}

/// The connection numbers of the login channel, in order of appearance.
pub fn login_connections(records: &[Record]) -> Vec<u64> {
    let mut order = Vec::new();
    for record in records.iter().filter(|r| r.channel == Channel::Login) {
        if !order.contains(&record.conn) {
            order.push(record.conn);
        }
    }
    order
}

/// The account a replay logs in with. A capture holds no credentials, so the replay decides:
/// when the recorded reply to `RequestAuthLogin` was `LoginOk` it sends the right password,
/// otherwise the wrong one.
#[derive(Debug, Clone)]
pub struct ReplayAccount {
    pub user: String,
    pub password: String,
    pub wrong_password: String,
}

/// How long a replay waits for each reply the recording had. A server that does not answer ends
/// the replay quickly; the comparison then reports the missing packets.
const REPLY_WAIT: Duration = Duration::from_secs(3);

/// Connects to `addr` (normally an in-process proxy in front of the login server under test)
/// and plays the client side of connection `conn` of `recording`. Returns the replies received
/// for each turn after the opening one.
pub async fn replay(
    addr: SocketAddr,
    recording: &[Record],
    conn: u64,
    account: &ReplayAccount,
) -> Vec<Vec<Vec<u8>>> {
    let turns = turns(recording, conn);
    let mut client = FakeClient::connect(addr).await;
    let mut login_ok: Option<LoginOk> = None;
    let mut all = Vec::new();
    for turn in turns.into_iter().skip(1) {
        let Some(step) = turn.step else { continue };
        match &step {
            ClientStep::GameGuard => {
                let session_id = client.init.session_id;
                client
                    .send_packet(&l2_protocol_login::client::AuthGameGuard { session_id })
                    .await;
            }
            ClientStep::Login => {
                let good = turn.replies.first().is_some_and(|name| name == "LoginOk");
                let password = if good {
                    &account.password
                } else {
                    &account.wrong_password
                };
                client.send_login_request(&account.user, password).await;
            }
            ClientStep::ServerList => {
                let (login_ok1, login_ok2) = login_ok
                    .as_ref()
                    .map_or((0, 0), |ok| (ok.login_ok1, ok.login_ok2));
                client
                    .send_packet(&l2_protocol_login::client::RequestServerList { login_ok1, login_ok2 })
                    .await;
            }
            ClientStep::ServerLogin(server_id) => {
                let (login_ok1, login_ok2) = login_ok
                    .as_ref()
                    .map_or((0, 0), |ok| (ok.login_ok1, ok.login_ok2));
                client
                    .send_packet(&l2_protocol_login::client::RequestServerLogin {
                        login_ok1,
                        login_ok2,
                        server_id: *server_id,
                    })
                    .await;
            }
            ClientStep::Raw(payload) => client.send(payload).await,
        }
        let mut replies = Vec::new();
        for _ in 0..turn.replies.len() {
            let Some(reply) = client.recv_within(REPLY_WAIT).await else {
                break;
            };
            if step == ClientStep::Login && reply.first() == Some(&LoginOk::OPCODE) {
                login_ok = LoginOk::decode(&mut PacketReader::new(&reply[1..])).ok();
            }
            replies.push(reply);
        }
        all.push(replies);
    }
    all
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn record(conn: u64, dir: Dir, name: &str, opcode: u8, fields: Option<Value>, raw: &[u8]) -> Record {
        Record::new(conn, dir, Channel::Login, opcode, name, fields, raw)
    }

    /// A short session: Init, AuthGameGuard, GGAuth, then a login that gives `login_ok1`.
    fn session(conn: u64, session_id: i64, login_ok1: i64, ok: bool) -> Vec<Record> {
        let mut records = vec![
            record(
                conn,
                Dir::S2c,
                "Init",
                0,
                Some(json!({ "session_id": session_id, "protocol": 50721, "blowfish_key": "ab" })),
                &[0; 4],
            ),
            record(
                conn,
                Dir::C2s,
                "AuthGameGuard",
                7,
                Some(json!({ "session_id": session_id })),
                &[7; 21],
            ),
            record(
                conn,
                Dir::S2c,
                "GGAuth",
                0x0B,
                Some(json!({ "response": session_id })),
                &[0x0B; 21],
            ),
            record(
                conn,
                Dir::C2s,
                "RequestAuthLogin",
                0,
                Some(json!({ "credentials": "<redacted>" })),
                &[0],
            ),
        ];
        records.push(if ok {
            record(
                conn,
                Dir::S2c,
                "LoginOk",
                3,
                Some(json!({ "login_ok1": login_ok1, "login_ok2": 5 })),
                &[3; 49],
            )
        } else {
            record(
                conn,
                Dir::S2c,
                "LoginFail",
                1,
                Some(json!({ "reason": 3, "reason_name": "REASON_USER_OR_PASS_WRONG" })),
                &[1, 3],
            )
        });
        records
    }

    #[test]
    fn random_values_are_ignored() {
        let a = session(1, 111, 1000, true);
        let b = session(9, -222, 2000, true);
        assert!(compare(&a, &b, Channel::Login, &CompareOptions::default()).is_empty());
        assert_same_shape(&a, &b, Channel::Login, &CompareOptions::default());
    }

    #[test]
    fn other_packet_types_and_values_are_found() {
        let a = session(1, 1, 1, true);
        let b = session(1, 1, 1, false);
        let differences = compare(&a, &b, Channel::Login, &CompareOptions::default());
        assert_eq!(differences.len(), 1, "{differences:?}");
        assert!(differences[0].contains("LoginOk") && differences[0].contains("LoginFail"));

        // The same type with another (not ignored) value.
        let mut c = session(1, 1, 1, true);
        c[0].fields = Some(json!({ "session_id": 1, "protocol": 50722, "blowfish_key": "ab" }));
        let differences = compare(&a, &c, Channel::Login, &CompareOptions::default());
        assert_eq!(differences.len(), 1, "{differences:?}");
        assert!(differences[0].contains("packet 0") && differences[0].contains("50722"));
    }

    #[test]
    fn missing_and_extra_packets_and_connections_are_found() {
        let a = session(1, 1, 1, true);
        let mut short = session(1, 1, 1, true);
        short.pop();
        let differences = compare(&a, &short, Channel::Login, &CompareOptions::default());
        assert!(differences[0].contains("the session ended"), "{differences:?}");
        let differences = compare(&short, &a, Channel::Login, &CompareOptions::default());
        assert!(differences[0].contains("extra packet"), "{differences:?}");

        let mut two = session(1, 1, 1, true);
        two.extend(session(2, 2, 2, false));
        let differences = compare(&a, &two, Channel::Login, &CompareOptions::default());
        assert!(differences[0].contains("expected 1 connection(s), got 2"));
    }

    #[test]
    fn an_ignored_field_must_keep_its_type() {
        let a = session(1, 1, 1, true);
        let mut b = session(1, 1, 1, true);
        b[4].fields = Some(json!({ "login_ok1": "text", "login_ok2": 5 }));
        let differences = compare(&a, &b, Channel::Login, &CompareOptions::default());
        assert_eq!(differences.len(), 1, "{differences:?}");
        // And an ignored field that is missing is a difference too.
        b[4].fields = Some(json!({ "login_ok2": 5 }));
        assert_eq!(
            compare(&a, &b, Channel::Login, &CompareOptions::default()).len(),
            1
        );
    }

    #[test]
    fn the_payload_length_is_compared_unless_switched_off() {
        let a = session(1, 1, 1, true);
        let mut b = session(1, 1, 1, true);
        b[4].raw = hex::encode([3u8; 50]);
        assert_eq!(
            compare(&a, &b, Channel::Login, &CompareOptions::default()).len(),
            1
        );
        let lenient = CompareOptions {
            compare_raw_length: false,
            ..CompareOptions::default()
        };
        assert!(compare(&a, &b, Channel::Login, &lenient).is_empty());
    }

    #[test]
    fn more_ignored_fields_can_be_added() {
        let a = session(1, 1, 1, true);
        let mut b = session(1, 1, 1, true);
        b[0].fields = Some(json!({ "session_id": 1, "protocol": 1, "blowfish_key": "ab" }));
        assert_eq!(
            compare(&a, &b, Channel::Login, &CompareOptions::default()).len(),
            1
        );
        let options = CompareOptions::default().ignoring(&["protocol"]);
        assert!(compare(&a, &b, Channel::Login, &options).is_empty());
    }

    #[test]
    fn other_channels_are_not_compared() {
        let a = session(1, 1, 1, true);
        let mut b = session(1, 1, 1, true);
        b.push(Record::new(2, Dir::C2s, Channel::Game, 1, "x", None, &[1]));
        assert!(compare(&a, &b, Channel::Login, &CompareOptions::default()).is_empty());
        // The game channel only exists in `b`: one connection too many.
        let differences = compare(&a, &b, Channel::Game, &CompareOptions::default());
        assert_eq!(differences, ["expected 0 connection(s), got 1"]);
    }

    #[test]
    fn a_recording_is_split_into_turns() {
        let mut records = session(4, 1, 1, true);
        records.push(record(
            4,
            Dir::C2s,
            "RequestServerList",
            5,
            Some(json!({ "login_ok1": 1, "login_ok2": 5 })),
            &[5; 9],
        ));
        records.push(record(
            4,
            Dir::C2s,
            "RequestServerLogin",
            2,
            Some(json!({ "login_ok1": 1, "login_ok2": 5, "server_id": 2 })),
            &[2; 10],
        ));
        records.push(record(
            4,
            Dir::C2s,
            "RequestPIAgreementCheck",
            0x0E,
            None,
            &[0x0E, 0, 0],
        ));
        let t = turns(&records, 4);
        let steps: Vec<_> = t.iter().map(|t| t.step.clone()).collect();
        assert_eq!(
            steps,
            [
                None,
                Some(ClientStep::GameGuard),
                Some(ClientStep::Login),
                Some(ClientStep::ServerList),
                Some(ClientStep::ServerLogin(2)),
                Some(ClientStep::Raw(vec![0x0E, 0, 0])),
            ]
        );
        assert_eq!(t[0].replies, ["Init"]);
        assert_eq!(t[1].replies, ["GGAuth"]);
        assert_eq!(t[2].replies, ["LoginOk"]);
        assert!(t[3].replies.is_empty());
        assert_eq!(login_connections(&records), [4]);
    }
}
