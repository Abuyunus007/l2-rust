#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The capture comparison end to end without a database: a fake login client, an in-process
//! `l2-proxy`, and a tiny fake login server that only knows the wire protocol. A session is
//! recorded, replayed against a second fake server with the same behaviour (shapes match) and
//! against one that behaves differently (the comparison reports it).
//!
//! `crates/login-server/tests/e2e.rs` does the same against the real login server.

use std::net::SocketAddr;
use std::sync::Arc;

use l2_crypto::{LoginCipher, RsaKeyPair};
use l2_net::{
    CancellationToken, CipherError, ConnectionConfig, Control, Encode, FrameCipher, FrameHandler,
    PacketReader, listen,
};
use l2_protocol_login::enums::{LoginFailReason, PlayFailReason};
use l2_protocol_login::server::{GGAuth, Init, LoginFail, LoginOk, PlayFail, ServerEntry, ServerList};
use l2_protocol_login::{
    ClientPacket, ConnectionState, RequestAuthLogin, credentials_from_decrypted, decode_client,
};
use l2_proxy::{CaptureWriter, Channel, Proxy, ProxyConfig};
use l2_testkit::capture_compare::{
    CompareOptions, ReplayAccount, assert_same_shape, compare, drain, load_capture, login_connections, replay,
};
use l2_testkit::fake_login_client::{FakeClient, login_fail, play_fail};
use rand::{Rng, RngExt};
use tokio::net::TcpListener;

struct LsCipher(LoginCipher);

impl FrameCipher for LsCipher {
    fn encrypt(&mut self, payload: Vec<u8>) -> Vec<u8> {
        self.0.encrypt(&payload, 0x1234_5678)
    }

    fn decrypt(&mut self, body: &mut Vec<u8>) -> Result<(), CipherError> {
        self.0.decrypt(body).map_err(|e| CipherError(e.to_string()))
    }
}

/// How a fake login server differs from another.
#[derive(Clone, Copy)]
struct Behaviour {
    brackets: bool,
}

struct FakeLs {
    keys: Arc<(RsaKeyPair, [u8; 128])>,
    behaviour: Behaviour,
    session_id: i32,
    blowfish_key: [u8; 16],
    login_ok: (i32, i32),
    state: ConnectionState,
}

impl FrameHandler for FakeLs {
    async fn on_connect(&mut self, ctl: &mut Control<'_>) {
        ctl.send(Init::new(self.session_id, self.keys.1, self.blowfish_key).to_bytes());
    }

    async fn on_frame(&mut self, body: Vec<u8>, ctl: &mut Control<'_>) {
        let mut reader = PacketReader::new(&body);
        let opcode = reader.u8().unwrap();
        let Some(packet) = decode_client(self.state, opcode, &mut reader).unwrap() else {
            return;
        };
        match packet {
            ClientPacket::AuthGameGuard(p) => {
                self.state = ConnectionState::AuthedGg;
                ctl.send(
                    GGAuth {
                        response: p.session_id,
                    }
                    .to_bytes(),
                );
            }
            ClientPacket::RequestAuthLogin(p) => {
                let blocks = match *p {
                    RequestAuthLogin::Rsa128 { block } => vec![block],
                    RequestAuthLogin::Rsa256 { block1, block2 } => vec![block1, block2],
                };
                let mut plain = Vec::new();
                for block in blocks {
                    plain.extend(self.keys.0.decrypt_raw(&block).unwrap());
                }
                let (user, password) = credentials_from_decrypted(&plain);
                if (user.as_str(), password.as_str()) == ("compat", "test") {
                    self.state = ConnectionState::AuthedLogin;
                    ctl.send(
                        LoginOk {
                            login_ok1: self.login_ok.0,
                            login_ok2: self.login_ok.1,
                        }
                        .to_bytes(),
                    );
                } else {
                    ctl.send_and_close(
                        LoginFail {
                            reason: LoginFailReason::UserOrPassWrong,
                        }
                        .to_bytes(),
                    );
                }
            }
            ClientPacket::RequestServerList(p) => {
                // The pair is random per session: a replay that sent the recorded one fails here.
                if (p.login_ok1, p.login_ok2) == self.login_ok {
                    ctl.send(
                        ServerList {
                            last_server: 0,
                            servers: vec![ServerEntry {
                                server_id: 1,
                                ip: [127, 0, 0, 1],
                                port: 7777,
                                age_limit: 0,
                                pvp: true,
                                current_players: 0,
                                max_players: 100,
                                up: false,
                                server_type: 1,
                                brackets: self.behaviour.brackets,
                            }],
                            char_info: None,
                        }
                        .to_bytes(),
                    );
                } else {
                    ctl.close();
                }
            }
            ClientPacket::RequestServerLogin(p) => {
                if (p.login_ok1, p.login_ok2) == self.login_ok && p.server_id == 1 {
                    ctl.send(
                        PlayFail {
                            reason: PlayFailReason::ServerOverloaded,
                        }
                        .to_bytes(),
                    );
                } else {
                    ctl.close();
                }
            }
        }
    }
}

async fn start_fake_ls(behaviour: Behaviour) -> SocketAddr {
    let pair = RsaKeyPair::generate(1024, &mut rand::rng()).unwrap();
    let scrambled = pair.scrambled_modulus().unwrap();
    let keys = Arc::new((pair, scrambled));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(listen(
        listener,
        Arc::new(ConnectionConfig::default()),
        CancellationToken::new(),
        move |_peer| {
            let mut key = [0u8; 16];
            rand::rng().fill_bytes(&mut key);
            let session_id: i32 = rand::rng().random();
            Some((
                FakeLs {
                    keys: Arc::clone(&keys),
                    behaviour,
                    session_id,
                    blowfish_key: key,
                    login_ok: (rand::rng().random(), rand::rng().random()),
                    state: ConnectionState::Connected,
                },
                Box::new(LsCipher(LoginCipher::new(&key))) as Box<dyn FrameCipher>,
            ))
        },
    ));
    addr
}

async fn start_proxy(
    upstream: SocketAddr,
) -> (Proxy, tokio::sync::mpsc::UnboundedReceiver<l2_proxy::Record>) {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let mut config = ProxyConfig::new("127.0.0.1:0".parse().unwrap(), upstream);
    config.rewrite_server_list = false;
    config.rsa_pool = 1;
    (Proxy::start(config, tx).await.unwrap(), rx)
}

fn account() -> ReplayAccount {
    ReplayAccount {
        user: "compat".to_owned(),
        password: "test".to_owned(),
        wrong_password: "nope".to_owned(),
    }
}

#[tokio::test]
async fn a_recorded_session_is_replayed_and_compared() {
    // Record: a failed login, then a login with the server list and a refused server login.
    let (proxy, mut records) = start_proxy(start_fake_ls(Behaviour { brackets: false }).await).await;
    let addr = proxy.local_addr();
    let mut wrong = FakeClient::connect(addr).await;
    assert_eq!(
        login_fail(&wrong.login("compat", "nope").await),
        LoginFailReason::UserOrPassWrong
    );
    let mut right = FakeClient::connect(addr).await;
    let ok = right.login_ok("compat", "test").await;
    let list = right.server_list(&ok).await;
    assert_eq!(list.servers[0].server_id, 1);
    assert_eq!(
        play_fail(&right.server_login(&ok, 1).await),
        PlayFailReason::ServerOverloaded
    );
    let recorded = drain(&mut records);
    proxy.stop().await;

    // Through a file, as the Java capture will be.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("session.jsonl");
    let mut writer = CaptureWriter::create(&path).unwrap();
    for record in &recorded {
        writer.write_record(record).unwrap();
    }
    drop(writer);
    let expected = load_capture(&path);
    let connections = login_connections(&expected);
    assert_eq!(connections.len(), 2);

    // The same behaviour: same shapes, although every session id, key and login pair differs.
    let (proxy, mut records) = start_proxy(start_fake_ls(Behaviour { brackets: false }).await).await;
    for conn in &connections {
        replay(proxy.local_addr(), &expected, *conn, &account()).await;
    }
    let actual = drain(&mut records);
    proxy.stop().await;
    assert_ne!(
        expected[0].fields.as_ref().unwrap()["session_id"],
        actual[0].fields.as_ref().unwrap()["session_id"],
        "the sessions really are different"
    );
    assert_same_shape(&expected, &actual, Channel::Login, &CompareOptions::default());

    // Another behaviour: the comparison names the packet and the field.
    let (proxy, mut records) = start_proxy(start_fake_ls(Behaviour { brackets: true }).await).await;
    for conn in &connections {
        replay(proxy.local_addr(), &expected, *conn, &account()).await;
    }
    let different = drain(&mut records);
    proxy.stop().await;
    let differences = compare(&expected, &different, Channel::Login, &CompareOptions::default());
    assert_eq!(differences.len(), 1, "{differences:?}");
    assert!(
        differences[0].contains("ServerList") && differences[0].contains("brackets"),
        "{differences:?}"
    );
}
