#![allow(clippy::unwrap_used, clippy::expect_used)] // test helpers may panic

//! Loopback tests over 127.0.0.1: handshake, auth, request/response matching, events.

use std::sync::Arc;
use std::time::Duration;

use l2_admin_api::*;
use tokio::net::TcpListener;

struct TestHandler;

impl AdminHandler for TestHandler {
    async fn handle(&self, request: Request, session: &Session) -> Response {
        match request {
            Request::StartServer { name } if name == "slow" => {
                tokio::time::sleep(Duration::from_millis(200)).await;
                Response::Ok
            }
            Request::StartServer { name } => Response::Error {
                message: format!("unknown server {name}"),
            },
            Request::SubscribeLogs { .. } => {
                session.send_event(Event::LogLine {
                    line: LogLine {
                        server: "login".into(),
                        seq: 1,
                        ts_ms: 0,
                        level: Some(LogLevel::Info),
                        target: None,
                        message: "hello log".into(),
                    },
                });
                Response::Ok
            }
            _ => Response::Servers { servers: vec![] },
        }
    }
}

async fn start() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(serve(
        listener,
        "test".into(),
        Token::new("secret"),
        Arc::new(TestHandler),
    ));
    format!("ws://{addr}")
}

#[tokio::test]
async fn auth_ok_and_request_response() {
    let url = start().await;
    let (client, _events) = AdminClient::connect(&url, "secret").await.unwrap();
    assert_eq!(client.endpoint_name(), "test");
    let resp = client.request(Request::ListServers).await.unwrap();
    assert_eq!(resp, Response::Servers { servers: vec![] });
}

#[tokio::test]
async fn wrong_token_is_rejected() {
    let url = start().await;
    let err = AdminClient::connect(&url, "nope").await.unwrap_err();
    assert!(
        matches!(err, AdminError::Rejected(ref r) if r == "invalid token"),
        "{err:?}"
    );
}

#[tokio::test]
async fn responses_match_request_ids() {
    let url = start().await;
    let (client, _events) = AdminClient::connect(&url, "secret").await.unwrap();
    // The slow request is sent first but answered last; each caller must get its own answer.
    let (slow, fast) = tokio::join!(
        client.request(Request::StartServer { name: "slow".into() }),
        client.request(Request::StartServer { name: "x".into() }),
    );
    assert_eq!(slow.unwrap(), Response::Ok);
    assert_eq!(
        fast.unwrap(),
        Response::Error {
            message: "unknown server x".into()
        }
    );
}

#[tokio::test]
async fn events_reach_the_client() {
    let url = start().await;
    let (client, mut events) = AdminClient::connect(&url, "secret").await.unwrap();
    let resp = client
        .request(Request::SubscribeLogs {
            server: None,
            since: None,
        })
        .await
        .unwrap();
    assert_eq!(resp, Response::Ok);
    let event = tokio::time::timeout(Duration::from_secs(2), events.recv())
        .await
        .unwrap()
        .unwrap();
    match event {
        Event::LogLine { line } => assert_eq!(line.message, "hello log"),
        other => panic!("unexpected event {other:?}"),
    }
}
