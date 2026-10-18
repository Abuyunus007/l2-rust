#![allow(clippy::unwrap_used, clippy::expect_used)] // test helpers may panic

//! The admin API of a whole supervisor over a loopback WebSocket.

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use common::*;
use l2_admin_api::{
    AdminClient, AdminError, AdminHandler, Event, GameServerStatus, LogLevel, LoginStatus, Request, Response,
    ServerState, Session, Token, serve,
};
use l2_supervisor::Running;
use tokio::net::TcpListener;
use tokio::sync::mpsc::UnboundedReceiver;

async fn connect(running: &Running) -> (AdminClient, UnboundedReceiver<Event>) {
    AdminClient::connect(&format!("ws://{}", running.local_addr()), "secret")
        .await
        .unwrap()
}

/// Next event for which `pick` returns a value.
async fn next_event<T>(events: &mut UnboundedReceiver<Event>, mut pick: impl FnMut(Event) -> Option<T>) -> T {
    tokio::time::timeout(WAIT, async {
        loop {
            let event = events.recv().await.expect("event stream ended");
            if let Some(found) = pick(event) {
                return found;
            }
        }
    })
    .await
    .expect("timed out waiting for an event")
}

fn status_of(servers: Vec<l2_admin_api::ServerStatus>, name: &str) -> l2_admin_api::ServerStatus {
    servers.into_iter().find(|s| s.name == name).unwrap()
}

async fn list(client: &AdminClient) -> Vec<l2_admin_api::ServerStatus> {
    match client.request(Request::ListServers).await.unwrap() {
        Response::Servers { servers } => servers,
        other => panic!("unexpected {other:?}"),
    }
}

#[tokio::test]
async fn wrong_token_is_rejected() {
    let running = Running::start(config(vec![])).await.unwrap();
    let err = AdminClient::connect(&format!("ws://{}", running.local_addr()), "wrong")
        .await
        .unwrap_err();
    assert!(matches!(err, AdminError::Rejected(_)), "{err:?}");
}

#[tokio::test]
async fn list_start_stop() {
    let running = Running::start(config(vec![
        child_server("a", &["sleep"]),
        child_server("b", &["sleep"]),
    ]))
    .await
    .unwrap();
    let (client, mut events) = connect(&running).await;
    assert_eq!(client.endpoint_name(), "supervisor");

    // Configuration order, all stopped.
    let servers = list(&client).await;
    assert_eq!(
        servers.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
        ["a", "b"]
    );
    assert!(
        servers
            .iter()
            .all(|s| s.state == ServerState::Stopped && s.pid.is_none())
    );

    assert_eq!(
        client
            .request(Request::StartServer { name: "a".into() })
            .await
            .unwrap(),
        Response::Ok
    );
    let a = status_of(list(&client).await, "a");
    assert_eq!(a.state, ServerState::Running);
    assert!(a.pid.is_some());
    assert_eq!(status_of(list(&client).await, "b").state, ServerState::Stopped);

    // Every registered client gets the state changes (this one made requests, so it is registered).
    let running_event = next_event(&mut events, |e| match e {
        Event::ServerStateChanged { status }
            if status.name == "a" && status.state == ServerState::Running =>
        {
            Some(status)
        }
        _ => None,
    })
    .await;
    assert_eq!(running_event.pid, a.pid);

    // Errors.
    assert_eq!(
        client
            .request(Request::StartServer { name: "a".into() })
            .await
            .unwrap(),
        Response::Error {
            message: "already running".into()
        }
    );
    assert_eq!(
        client
            .request(Request::StopServer { name: "nope".into() })
            .await
            .unwrap(),
        Response::Error {
            message: "unknown server 'nope'".into()
        }
    );

    assert_eq!(
        client
            .request(Request::RestartServer { name: "a".into() })
            .await
            .unwrap(),
        Response::Ok
    );
    assert_ne!(status_of(list(&client).await, "a").pid, a.pid);

    assert_eq!(
        client
            .request(Request::StopServer { name: "a".into() })
            .await
            .unwrap(),
        Response::Ok
    );
    assert_eq!(status_of(list(&client).await, "a").state, ServerState::Stopped);
    next_event(&mut events, |e| match e {
        Event::ServerStateChanged { status }
            if status.name == "a" && status.state == ServerState::Stopped =>
        {
            Some(())
        }
        _ => None,
    })
    .await;
    running.shutdown().await;
}

#[tokio::test]
async fn autostart_runs_marked_servers_only() {
    let mut auto = child_server("auto", &["sleep"]);
    auto.autostart = true;
    let running = Running::start(config(vec![auto, child_server("manual", &["sleep"])]))
        .await
        .unwrap();
    running.autostart().await;
    assert_eq!(
        running.supervisor.get("auto").unwrap().status().state,
        ServerState::Running
    );
    assert_eq!(
        running.supervisor.get("manual").unwrap().status().state,
        ServerState::Stopped
    );
    running.shutdown().await;
}

#[tokio::test]
async fn subscribe_logs_backlog_since_and_live() {
    let running = Running::start(config(vec![
        child_server("a", &["sleep"]),
        child_server("b", &["print"]),
    ]))
    .await
    .unwrap();
    let logs = Arc::clone(running.supervisor.logs());
    let (client, mut events) = connect(&running).await;

    // Live: subscribe first, then start.
    assert_eq!(
        client
            .request(Request::SubscribeLogs {
                server: Some("a".into()),
                since: None
            })
            .await
            .unwrap(),
        Response::Ok
    );
    client
        .request(Request::StartServer { name: "a".into() })
        .await
        .unwrap();
    let started = next_event(&mut events, |e| match e {
        Event::LogLine { line } if line.message.starts_with("started") => Some(line),
        _ => None,
    })
    .await;
    assert_eq!(started.server, "a");
    assert_eq!(started.level, Some(LogLevel::Info));
    assert_eq!(started.target.as_deref(), Some("supervisor"));
    let ready = next_event(&mut events, |e| match e {
        Event::LogLine { line } if line.message == "ready" => Some(line),
        _ => None,
    })
    .await;
    assert!(ready.seq > started.seq);

    // Backlog: a second client asks for everything after `started`: it gets `ready`, not `started`.
    let (second, mut second_events) = connect(&running).await;
    second
        .request(Request::SubscribeLogs {
            server: Some("a".into()),
            since: Some(started.seq),
        })
        .await
        .unwrap();
    let first_backlog = next_event(&mut second_events, |e| match e {
        Event::LogLine { line } => Some(line),
        _ => None,
    })
    .await;
    assert_eq!(first_backlog, ready);

    // All servers: a line from b arrives too, and a filter on a does not show it.
    second
        .request(Request::SubscribeLogs {
            server: None,
            since: None,
        })
        .await
        .unwrap();
    client
        .request(Request::StartServer { name: "b".into() })
        .await
        .unwrap();
    wait_log(&logs, "b", "stderr line").await;
    next_event(&mut second_events, |e| match e {
        Event::LogLine { line } if line.server == "b" && line.message == "stderr line" => Some(()),
        _ => None,
    })
    .await;
    // The first client subscribed to "a" only.
    while let Ok(event) = events.try_recv() {
        if let Event::LogLine { line } = event {
            assert_eq!(line.server, "a");
        }
    }

    // Unknown server, unsubscribe.
    assert_eq!(
        client
            .request(Request::SubscribeLogs {
                server: Some("zzz".into()),
                since: None
            })
            .await
            .unwrap(),
        Response::Error {
            message: "unknown server 'zzz'".into()
        }
    );
    assert_eq!(
        client.request(Request::UnsubscribeLogs).await.unwrap(),
        Response::Ok
    );
    running.shutdown().await;
}

/// A stand-in for a login server's admin endpoint.
struct FakeLoginServer {
    sessions: Mutex<Vec<Session>>,
}

fn login_status(online: u32) -> LoginStatus {
    LoginStatus {
        game_servers: vec![GameServerStatus {
            id: 1,
            name: "Bartz".into(),
            authed: true,
            online,
            max_players: 100,
            status: "Normal".into(),
        }],
        authed_clients: 0,
    }
}

impl AdminHandler for FakeLoginServer {
    async fn handle(&self, request: Request, session: &Session) -> Response {
        self.sessions.lock().unwrap().push(session.clone());
        match request {
            Request::GetLoginStatus { server: None } => Response::LoginStatus {
                status: login_status(3),
            },
            _ => Response::Error {
                message: "unsupported".into(),
            },
        }
    }
}

#[tokio::test]
async fn login_status_is_relayed() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let fake_addr = listener.local_addr().unwrap();
    let fake = Arc::new(FakeLoginServer {
        sessions: Mutex::default(),
    });
    tokio::spawn(serve(
        listener,
        "login".into(),
        Token::new("ls-secret"),
        Arc::clone(&fake),
    ));

    let mut login = child_server("ls", &["sleep"]);
    login.admin_endpoint = Some(fake_addr.to_string());
    login.admin_token = "ls-secret".into();
    let running = Running::start(config(vec![login, child_server("plain", &["sleep"])]))
        .await
        .unwrap();
    let (client, mut events) = connect(&running).await;

    // The link connects in the background; ask until it answers.
    let mut answer = None;
    for _ in 0..200 {
        let response = client
            .request(Request::GetLoginStatus {
                server: Some("ls".into()),
            })
            .await
            .unwrap();
        if let Response::LoginStatus { status } = response {
            answer = Some(status);
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(answer.unwrap(), login_status(3));

    // A status change pushed by the login server reaches the GUI, labelled with our server name.
    let pushed = fake.sessions.lock().unwrap().first().unwrap().clone();
    assert!(pushed.send_event(Event::LoginStatusChanged {
        server: "whatever-the-server-calls-itself".into(),
        status: login_status(7),
    }));
    let relayed = next_event(&mut events, |e| match e {
        Event::LoginStatusChanged { server, status } if status.game_servers[0].online == 7 => Some(server),
        _ => None,
    })
    .await;
    assert_eq!(relayed, "ls");

    // Errors: no endpoint, unknown server, missing name.
    for (server, expected) in [
        (Some("plain"), "no admin connection to 'plain'"),
        (Some("zzz"), "unknown server 'zzz'"),
        (None, "the supervisor needs a server name"),
    ] {
        let response = client
            .request(Request::GetLoginStatus {
                server: server.map(Into::into),
            })
            .await
            .unwrap();
        assert_eq!(
            response,
            Response::Error {
                message: expected.into()
            }
        );
    }
    running.shutdown().await;
}
