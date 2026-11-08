#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The chat of `l2-ai` with a scripted model and a stand-in game server, and the HTTP client
//! against a stand-in model endpoint.

use std::collections::VecDeque;
use std::sync::Mutex;

use l2_admin_api::{GameAction, Response};
use l2_mcp::audit::Audit;
use l2_mcp::backend::Backend;
use l2_mcp::chat::{Chat, Confirm};
use l2_mcp::llm::{HttpLlm, Llm};
use l2_mcp::server::McpServer;
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// The game server: answers every command with "ran <command>" and remembers it.
#[derive(Default)]
struct FakeGame {
    seen: Mutex<Vec<String>>,
}

impl Backend for &FakeGame {
    async fn game_action(&self, action: GameAction) -> Result<Response, String> {
        let GameAction::Command { command } = action else {
            return Ok(Response::Ok);
        };
        self.seen.lock().unwrap().push(command.clone());
        Ok(Response::GameReply {
            lines: vec![format!("ran {command}")],
        })
    }
}

/// The model: plays its answers in order and keeps every request.
#[derive(Default)]
struct Script {
    answers: Mutex<VecDeque<Value>>,
    requests: Mutex<Vec<Value>>,
}

impl Script {
    fn new(answers: Vec<Value>) -> Self {
        Self {
            answers: Mutex::new(answers.into()),
            requests: Mutex::default(),
        }
    }
}

impl Llm for &Script {
    async fn complete(&self, request: Value) -> Result<Value, String> {
        self.requests.lock().unwrap().push(request);
        self.answers
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| "the script is over".to_owned())
    }
}

/// The administrator: says yes or no, and remembers what he was asked.
struct Admin {
    yes: bool,
    asked: Mutex<Vec<(String, String)>>,
}

impl Admin {
    fn saying(yes: bool) -> Self {
        Self {
            yes,
            asked: Mutex::default(),
        }
    }
}

impl Confirm for &Admin {
    async fn approve(&self, command: &str, reason: &str) -> bool {
        self.asked.lock().unwrap().push((command.into(), reason.into()));
        self.yes
    }
}

/// A model answer that calls one tool (arguments as the JSON text of the OpenAI API).
fn calls(tool: &str, arguments: Value) -> Value {
    json!({ "choices": [{ "message": { "role": "assistant", "content": null, "tool_calls": [{
        "id": format!("call-{tool}"), "type": "function",
        "function": { "name": tool, "arguments": arguments.to_string() }
    }]}}]})
}

fn says(text: &str) -> Value {
    json!({ "choices": [{ "message": { "role": "assistant", "content": text } }] })
}

fn chat<'a>(
    game: &'a FakeGame,
    script: &'a Script,
    admin: &'a Admin,
    audit: Option<std::path::PathBuf>,
) -> Chat<&'a FakeGame, &'a Script, &'a Admin> {
    let server = McpServer::new(game, Audit::new(audit), true);
    Chat::new(server, script, admin, "local".into())
}

/// The tool messages the model was sent back, in order.
fn tool_replies(script: &Script) -> Vec<String> {
    let requests = script.requests.lock().unwrap();
    let last = requests.last().unwrap();
    last["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["role"] == "tool")
        .map(|m| m["content"].as_str().unwrap().to_owned())
        .collect()
}

#[tokio::test]
async fn the_model_runs_a_command_and_says_what_happened() {
    let (game, admin) = (FakeGame::default(), Admin::saying(true));
    let script = Script::new(vec![
        calls("run_admin_command", json!({ "command": "setlevel 60" })),
        says("<think>done</think>Amy is level 60 now."),
    ]);
    let mut chat = chat(&game, &script, &admin, None);
    let answer = chat.say("Setz Amy auf Level 60").await.unwrap();
    assert_eq!(answer, "Amy is level 60 now.");
    assert_eq!(*game.seen.lock().unwrap(), vec!["setlevel 60"]);
    assert_eq!(tool_replies(&script), vec!["ran setlevel 60"]);
    assert!(
        admin.asked.lock().unwrap().is_empty(),
        "nothing destructive, nobody asked"
    );

    // The request is an OpenAI chat request with the tools.
    let requests = script.requests.lock().unwrap();
    let first = &requests[0];
    assert_eq!(first["model"], "local");
    assert_eq!(first["messages"][0]["role"], "system");
    assert_eq!(first["messages"][1]["content"], "Setz Amy auf Level 60");
    let tools: Vec<&str> = first["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["function"]["name"].as_str().unwrap())
        .collect();
    assert!(tools.contains(&"run_admin_command") && tools.contains(&"run_destructive_admin_command"));
    assert_eq!(first["tools"][0]["type"], "function");
}

#[tokio::test]
async fn a_destructive_command_waits_for_the_administrator() {
    let dir = tempfile::tempdir().unwrap();
    let audit = dir.path().join("audit.log");

    // He says no: nothing runs, the model is told, the log says so.
    let (game, admin) = (FakeGame::default(), Admin::saying(false));
    let script = Script::new(vec![
        calls(
            "run_destructive_admin_command",
            json!({ "command": "ban_char Spambot", "reason": "spam" }),
        ),
        says("Okay, I did not ban him."),
    ]);
    let mut no = chat(&game, &script, &admin, Some(audit.clone()));
    assert_eq!(no.say("Bann Spambot").await.unwrap(), "Okay, I did not ban him.");
    assert!(game.seen.lock().unwrap().is_empty());
    assert_eq!(
        *admin.asked.lock().unwrap(),
        vec![("ban_char Spambot".to_owned(), "spam".to_owned())]
    );
    assert!(tool_replies(&script)[0].contains("said no"));
    let log = std::fs::read_to_string(&audit).unwrap();
    assert!(
        log.contains(r#""ban_char Spambot" -> error: declined by the administrator"#),
        "{log}"
    );

    // He says yes: it runs.
    let (game, admin) = (FakeGame::default(), Admin::saying(true));
    let script = Script::new(vec![
        calls(
            "run_destructive_admin_command",
            json!({ "command": "ban_char Spambot", "reason": "spam" }),
        ),
        says("Banned."),
    ]);
    let mut yes = chat(&game, &script, &admin, None);
    assert_eq!(yes.say("Bann Spambot").await.unwrap(), "Banned.");
    assert_eq!(*game.seen.lock().unwrap(), vec!["ban_char Spambot"]);
}

#[tokio::test]
async fn the_model_cannot_slip_a_destructive_command_past_the_question() {
    let (game, admin) = (FakeGame::default(), Admin::saying(true));
    let script = Script::new(vec![
        calls("run_admin_command", json!({ "command": "kill" })),
        says("That needs your approval."),
    ]);
    let mut chat = chat(&game, &script, &admin, None);
    chat.say("Töte das Ziel").await.unwrap();
    assert!(game.seen.lock().unwrap().is_empty());
    assert!(admin.asked.lock().unwrap().is_empty());
    assert!(tool_replies(&script)[0].contains("run_destructive_admin_command"));
}

#[tokio::test]
async fn broken_calls_are_told_to_the_model_and_endless_ones_are_stopped() {
    let (game, admin) = (FakeGame::default(), Admin::saying(true));
    let broken = json!({ "choices": [{ "message": { "role": "assistant", "tool_calls": [
        { "id": "a", "type": "function", "function": { "name": "run_admin_command", "arguments": "{not json" } },
        { "id": "b", "type": "function", "function": { "name": "format_disk", "arguments": "{}" } },
        { "id": "c", "type": "function", "function": { "name": "run_admin_command", "arguments": { "command": "gmliston" } } }
    ]}}]});
    let script = Script::new(vec![broken, says("Sorry.")]);
    let mut chat_a = chat(&game, &script, &admin, None);
    assert_eq!(chat_a.say("x").await.unwrap(), "Sorry.");
    let replies = tool_replies(&script);
    assert!(
        replies[0].starts_with("Error:") && replies[0].contains("not JSON"),
        "{replies:?}"
    );
    assert!(replies[1].contains("no tool format_disk"), "{replies:?}");
    assert_eq!(replies[2], "ran gmliston", "the arguments as an object work too");

    // A model that calls tools forever.
    let script = Script::new(
        (0..20)
            .map(|_| calls("run_admin_command", json!({ "command": "gmliston" })))
            .collect(),
    );
    let mut chat_b = chat(&game, &script, &admin, None);
    let answer = chat_b.say("loop").await.unwrap();
    assert!(answer.contains("Stopped after 8 rounds"), "{answer}");

    // A model that answers nonsense, or not at all.
    let script = Script::new(vec![json!({ "error": "model not loaded" })]);
    let mut chat_c = chat(&game, &script, &admin, None);
    assert!(chat_c.say("x").await.unwrap_err().contains("without a message"));
    let script = Script::new(vec![]);
    let mut chat_d = chat(&game, &script, &admin, None);
    assert!(chat_d.say("x").await.is_err());
}

#[tokio::test]
async fn reset_forgets_the_conversation() {
    let (game, admin) = (FakeGame::default(), Admin::saying(true));
    let script = Script::new(vec![says("one"), says("two")]);
    let mut chat = chat(&game, &script, &admin, None);
    chat.say("first").await.unwrap();
    chat.reset();
    chat.say("second").await.unwrap();
    let requests = script.requests.lock().unwrap();
    let messages = requests[1]["messages"].as_array().unwrap();
    assert_eq!(
        messages.len(),
        2,
        "system and the new question only: {messages:?}"
    );
    assert_eq!(messages[1]["content"], "second");
}

/// A stand-in model endpoint: answers one request with `reply` and hands back what it got.
async fn one_shot_endpoint(reply: &'static [u8]) -> (String, tokio::task::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut got = Vec::new();
        let mut buf = [0u8; 4096];
        // Read the head and the announced body.
        loop {
            let n = socket.read(&mut buf).await.unwrap();
            got.extend_from_slice(&buf[..n]);
            let text = String::from_utf8_lossy(&got).to_string();
            if let Some(split) = text.find("\r\n\r\n") {
                let length: usize = text
                    .lines()
                    .find_map(|l| l.strip_prefix("Content-Length: "))
                    .and_then(|v| v.trim().parse().ok())
                    .unwrap_or(0);
                if got.len() >= split + 4 + length {
                    break;
                }
            }
            if n == 0 {
                break;
            }
        }
        socket.write_all(reply).await.unwrap();
        socket.shutdown().await.unwrap();
        String::from_utf8_lossy(&got).to_string()
    });
    (url, task)
}

#[tokio::test]
async fn the_http_client_talks_to_an_openai_endpoint() {
    let (url, task) = one_shot_endpoint(
        b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\n\
          f\r\n{\"choices\":[{\"m\r\n1b\r\nessage\":{\"content\":\"hi\"}}]}\r\n0\r\n\r\n",
    )
    .await;
    let llm = HttpLlm::new(&url).unwrap();
    let answer = llm
        .complete(json!({ "model": "local", "messages": [] }))
        .await
        .unwrap();
    assert_eq!(answer["choices"][0]["message"]["content"], "hi");
    let request = task.await.unwrap();
    assert!(
        request.starts_with("POST /v1/chat/completions HTTP/1.1\r\n"),
        "{request}"
    );
    assert!(
        request.ends_with(r#"{"messages":[],"model":"local"}"#),
        "{request}"
    );

    let (url, _task) =
        one_shot_endpoint(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 7\r\n\r\nloading").await;
    let error = HttpLlm::new(&url).unwrap().complete(json!({})).await.unwrap_err();
    assert!(error.contains("503") && error.contains("loading"), "{error}");

    let error = HttpLlm::new("http://127.0.0.1:1")
        .unwrap()
        .complete(json!({}))
        .await
        .unwrap_err();
    assert!(error.contains("cannot reach the model"), "{error}");
}
