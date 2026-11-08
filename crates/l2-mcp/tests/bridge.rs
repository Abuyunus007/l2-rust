#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The bridge against a real admin endpoint (on port 0) with a stand-in game server behind it.

use std::sync::{Arc, Mutex};

use l2_admin_api::{AdminHandler, GameAction, GameInfo, Request, Response, Session, TickStats, Token, serve};
use l2_mcp::audit::Audit;
use l2_mcp::backend::AdminBackend;
use l2_mcp::server::McpServer;
use serde_json::{Value, json};
use tokio::net::TcpListener;

const TOKEN: &str = "test-token";

/// The game server: runs nothing, remembers what it was asked.
#[derive(Default)]
struct FakeGame {
    seen: Mutex<Vec<GameAction>>,
}

impl AdminHandler for FakeGame {
    async fn handle(&self, request: Request, _session: &Session) -> Response {
        let Request::GameAdmin { server: None, action } = request else {
            return Response::Error {
                message: "not a game action".into(),
            };
        };
        self.seen.lock().unwrap().push(action.clone());
        match action {
            GameAction::Command { command } if command.starts_with("fail") => Response::Error {
                message: "no such player".into(),
            },
            GameAction::Command { command } => Response::GameReply {
                lines: vec![format!("ran {command}"), "second line".into()],
            },
            GameAction::CommandOn { target, command } => Response::GameReply {
                lines: vec![format!("ran {command} on {target}")],
            },
            GameAction::Announce { .. } => Response::GameReply { lines: vec![] },
            GameAction::ServerInfo => Response::GameInfo {
                info: GameInfo {
                    uptime_secs: 7_380,
                    online: 3,
                    memory_bytes: 512 * 1024 * 1024,
                    threads: 12,
                    ticks: 100,
                    tick_ms: 50,
                    tick_stats: TickStats::default(),
                    shutdown_in_secs: None,
                },
            },
            _ => Response::Error {
                message: "unexpected".into(),
            },
        }
    }
}

struct Setup {
    game: Arc<FakeGame>,
    url: String,
    audit: tempfile::TempDir,
}

async fn endpoint() -> Setup {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    let game = Arc::new(FakeGame::default());
    tokio::spawn(serve(
        listener,
        "game".into(),
        Token::new(TOKEN),
        Arc::clone(&game),
    ));
    Setup {
        game,
        url,
        audit: tempfile::tempdir().unwrap(),
    }
}

impl Setup {
    fn bridge(&self, token: &str, destructive: bool) -> McpServer<AdminBackend> {
        let backend = AdminBackend::new(self.url.clone(), token.into(), None);
        McpServer::new(backend, Audit::new(Some(self.audit_path())), destructive)
    }

    fn audit_path(&self) -> std::path::PathBuf {
        self.audit.path().join("mcp-audit.log")
    }

    fn seen(&self) -> Vec<GameAction> {
        self.game.seen.lock().unwrap().clone()
    }
}

async fn ask(bridge: &McpServer<AdminBackend>, id: i64, method: &str, params: Value) -> Value {
    let line = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }).to_string();
    let reply = bridge.handle_line(&line).await.expect("an answer");
    let reply: Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(reply["id"], id);
    reply
}

async fn call(bridge: &McpServer<AdminBackend>, tool: &str, arguments: Value) -> (bool, String) {
    let reply = ask(
        bridge,
        9,
        "tools/call",
        json!({ "name": tool, "arguments": arguments }),
    )
    .await;
    let result = &reply["result"];
    (
        !result["isError"].as_bool().unwrap(),
        result["content"][0]["text"].as_str().unwrap().to_owned(),
    )
}

#[tokio::test]
async fn the_handshake_and_the_tools() {
    let setup = endpoint().await;
    let bridge = setup.bridge(TOKEN, true);
    let reply = ask(
        &bridge,
        1,
        "initialize",
        json!({ "protocolVersion": "2025-03-26", "capabilities": {} }),
    )
    .await;
    assert_eq!(reply["result"]["protocolVersion"], "2025-03-26");
    assert_eq!(reply["result"]["serverInfo"]["name"], "l2-mcp");
    assert!(reply["result"]["capabilities"]["tools"].is_object());
    // A version it does not know: its own newest.
    let reply = ask(
        &bridge,
        2,
        "initialize",
        json!({ "protocolVersion": "1999-01-01" }),
    )
    .await;
    assert_eq!(reply["result"]["protocolVersion"], "2025-06-18");
    // A notification is not answered.
    let note = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }).to_string();
    assert_eq!(bridge.handle_line(&note).await, None);
    assert_eq!(ask(&bridge, 3, "ping", json!({})).await["result"], json!({}));

    let reply = ask(&bridge, 4, "tools/list", json!({})).await;
    let names: Vec<&str> = reply["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "server_info",
            "command_help",
            "run_admin_command",
            "announce",
            "run_destructive_admin_command"
        ]
    );

    // Without destructive commands, that tool is not there and cannot be called.
    let safe = setup.bridge(TOKEN, false);
    let reply = ask(&safe, 5, "tools/list", json!({})).await;
    assert_eq!(reply["result"]["tools"].as_array().unwrap().len(), 4);
    let reply = ask(
        &safe,
        6,
        "tools/call",
        json!({ "name": "run_destructive_admin_command", "arguments": { "command": "kill", "reason": "x" } }),
    )
    .await;
    assert_eq!(reply["error"]["code"], -32602);
    assert!(setup.seen().is_empty());
}

#[tokio::test]
async fn a_command_runs_on_the_server_and_its_answer_comes_back() {
    let setup = endpoint().await;
    let bridge = setup.bridge(TOKEN, true);
    let (ok, text) = call(&bridge, "run_admin_command", json!({ "command": "setlevel 80" })).await;
    assert!(ok);
    assert_eq!(text, "ran setlevel 80\nsecond line");
    let (ok, text) = call(&bridge, "run_admin_command", json!({ "command": "fail Amy" })).await;
    assert!(!ok);
    assert_eq!(text, "no such player");
    let (ok, text) = call(&bridge, "server_info", json!({})).await;
    assert!(ok);
    assert!(
        text.contains("Players online: 3") && text.contains("Uptime: 2 h 3 min"),
        "{text}"
    );
    let (ok, _) = call(&bridge, "announce", json!({ "text": "Hello", "critical": true })).await;
    assert!(ok);
    assert_eq!(
        setup.seen(),
        vec![
            GameAction::Command {
                command: "setlevel 80".into()
            },
            GameAction::Command {
                command: "fail Amy".into()
            },
            GameAction::ServerInfo,
            GameAction::Announce {
                text: "Hello".into(),
                critical: true
            },
        ]
    );
}

#[tokio::test]
async fn a_command_on_a_character_goes_with_its_name_and_the_help_teaches_the_syntax() {
    let setup = endpoint().await;
    let bridge = setup.bridge(TOKEN, true);
    let (ok, text) = call(
        &bridge,
        "run_admin_command",
        json!({ "command": "set_level 80", "target": "Amy" }),
    )
    .await;
    assert!(ok);
    assert_eq!(text, "ran set_level 80 on Amy");
    let (ok, _) = call(
        &bridge,
        "run_destructive_admin_command",
        json!({ "command": "setkarma 0", "target": "Amy", "reason": "asked" }),
    )
    .await;
    assert!(ok);
    assert_eq!(
        setup.seen(),
        vec![
            GameAction::CommandOn {
                target: "Amy".into(),
                command: "set_level 80".into()
            },
            GameAction::CommandOn {
                target: "Amy".into(),
                command: "setkarma 0".into()
            },
        ]
    );
    let log = std::fs::read_to_string(setup.audit_path()).unwrap();
    assert!(log.contains(r#""set_level 80 (target: Amy)" -> ok"#), "{log}");

    let (ok, text) = call(&bridge, "command_help", json!({ "query": "level" })).await;
    assert!(ok);
    assert!(
        text.contains("set_level <level>") && text.contains("Needs target"),
        "{text}"
    );
    let (_, all) = call(&bridge, "command_help", json!({})).await;
    assert!(all.lines().count() > 40, "{all}");
    let (ok, text) = call(&bridge, "command_help", json!({ "query": "teleportto" })).await;
    assert!(ok && text.starts_with("Nothing found"), "{text}");
    assert_eq!(setup.seen().len(), 2, "the help asks the server nothing");
}

#[tokio::test]
async fn a_destructive_command_only_runs_through_its_own_tool_and_everything_is_logged() {
    let setup = endpoint().await;
    let bridge = setup.bridge(TOKEN, true);
    let (ok, text) = call(&bridge, "run_admin_command", json!({ "command": "ban_char Amy" })).await;
    assert!(!ok);
    assert!(text.contains("run_destructive_admin_command"), "{text}");
    assert!(setup.seen().is_empty(), "the server was never asked");

    let (ok, text) = call(
        &bridge,
        "run_destructive_admin_command",
        json!({ "command": "ban_char Amy", "reason": "the admin asked: spam bot" }),
    )
    .await;
    assert!(ok, "{text}");
    assert_eq!(
        setup.seen(),
        vec![GameAction::Command {
            command: "ban_char Amy".into()
        }]
    );
    let log = std::fs::read_to_string(setup.audit_path()).unwrap();
    let lines: Vec<&str> = log.lines().collect();
    assert_eq!(lines.len(), 2, "{log}");
    assert!(
        lines[0].contains(r#"run_admin_command "ban_char Amy" -> error: refused"#),
        "{log}"
    );
    assert!(
        lines[1].contains(r#""ban_char Amy (reason: the admin asked: spam bot)" -> ok: ran ban_char Amy"#),
        "{log}"
    );
}

#[tokio::test]
async fn a_wrong_token_or_no_server_is_an_error_not_a_crash() {
    let setup = endpoint().await;
    let (ok, text) = call(&setup.bridge("wrong", true), "server_info", json!({})).await;
    assert!(!ok);
    assert!(text.contains("cannot reach the admin endpoint"), "{text}");

    let nobody = McpServer::new(
        AdminBackend::new("ws://127.0.0.1:1".into(), TOKEN.into(), None),
        Audit::new(None),
        true,
    );
    let (ok, _) = call(&nobody, "run_admin_command", json!({ "command": "setlevel 80" })).await;
    assert!(!ok);
    assert!(setup.seen().is_empty());
}

#[tokio::test]
async fn bad_messages_get_json_rpc_errors() {
    let setup = endpoint().await;
    let bridge = setup.bridge(TOKEN, true);
    let reply: Value = serde_json::from_str(&bridge.handle_line("{not json").await.unwrap()).unwrap();
    assert_eq!(reply["error"]["code"], -32700);
    assert_eq!(
        ask(&bridge, 1, "resources/list", json!({})).await["error"]["code"],
        -32601
    );
    assert_eq!(
        ask(&bridge, 2, "tools/call", json!({ "name": "format_disk" })).await["error"]["code"],
        -32602
    );
    let (ok, text) = call(&bridge, "run_admin_command", json!({ "command": "  " })).await;
    assert!(!ok && text.contains("empty"));
    assert_eq!(bridge.handle_line("").await, None);
}

#[tokio::test]
async fn the_program_speaks_only_mcp_on_stdout() {
    use std::process::Stdio;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let setup = endpoint().await;
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_l2-mcp"))
        .args(["--audit-log", "none"])
        .env("L2_MCP_URL", &setup.url)
        .env("L2_ADMIN_TOKEN", TOKEN)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let requests = [
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-06-18" } }),
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call",
                "params": { "name": "run_admin_command", "arguments": { "command": "setlevel 80" } } }),
    ];
    for request in requests {
        stdin.write_all(format!("{request}\n").as_bytes()).await.unwrap();
    }
    stdin.flush().await.unwrap();
    drop(stdin);
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let mut replies = Vec::new();
    while let Some(line) = lines.next_line().await.unwrap() {
        let reply: Value =
            serde_json::from_str(&line).unwrap_or_else(|_| panic!("not MCP on stdout: {line}"));
        replies.push(reply);
    }
    assert_eq!(replies.len(), 2, "{replies:?}");
    assert_eq!(replies[0]["id"], 1);
    assert_eq!(
        replies[1]["result"]["content"][0]["text"],
        "ran setlevel 80\nsecond line"
    );
    assert!(child.wait().await.unwrap().success());
}
