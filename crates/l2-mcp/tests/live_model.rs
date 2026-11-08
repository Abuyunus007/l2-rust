#![allow(clippy::unwrap_used, clippy::expect_used)]
//! A real local model against a stand-in game server: does it pick the right commands?
//!
//! Skipped unless `L2_AI_LIVE_URL` (an OpenAI-compatible endpoint, e.g. `http://127.0.0.1:11434`
//! for Ollama) and `L2_AI_LIVE_MODEL` (the model's name there) are set. Run it with
//! `cargo test -p l2-mcp --test live_model -- --nocapture` to see the model's answers.

use std::sync::Mutex;
use std::time::Instant;

use l2_admin_api::{GameAction, GameInfo, Response, TickStats};
use l2_mcp::audit::Audit;
use l2_mcp::backend::Backend;
use l2_mcp::chat::{Chat, Confirm};
use l2_mcp::llm::HttpLlm;
use l2_mcp::server::McpServer;

#[derive(Default)]
struct FakeGame {
    seen: Mutex<Vec<GameAction>>,
}

impl Backend for &FakeGame {
    async fn game_action(&self, action: GameAction) -> Result<Response, String> {
        self.seen.lock().unwrap().push(action.clone());
        Ok(match action {
            GameAction::ServerInfo => Response::GameInfo {
                info: GameInfo {
                    uptime_secs: 3_600,
                    online: 17,
                    memory_bytes: 1 << 30,
                    threads: 20,
                    ticks: 72_000,
                    tick_ms: 50,
                    tick_stats: TickStats::default(),
                    shutdown_in_secs: None,
                },
            },
            GameAction::Command { command } if command == "online" => Response::GameReply {
                lines: vec!["Online players: 17".into()],
            },
            GameAction::Command { command } => Response::GameReply {
                lines: vec![format!("Done: {command}")],
            },
            GameAction::CommandOn { target, command } => Response::GameReply {
                lines: vec![format!("Done: {command} on {target}")],
            },
            GameAction::Announce { text, .. } => Response::GameReply {
                lines: vec![format!("Announced: {text}")],
            },
            _ => Response::Ok,
        })
    }
}

/// Says no, and remembers what it was asked.
#[derive(Default)]
struct No {
    asked: Mutex<Vec<String>>,
}

impl Confirm for &No {
    async fn approve(&self, command: &str, _reason: &str) -> bool {
        self.asked.lock().unwrap().push(command.to_owned());
        false
    }
}

/// What the stand-in server was asked to run, as text.
fn ran(game: &FakeGame) -> Vec<String> {
    game.seen
        .lock()
        .unwrap()
        .iter()
        .map(|action| match action {
            GameAction::Command { command } => command.clone(),
            GameAction::CommandOn { target, command } => format!("{command} @{target}"),
            GameAction::Announce { text, .. } => format!("announce: {text}"),
            GameAction::ServerInfo => "server_info".into(),
            other => format!("{other:?}"),
        })
        .collect()
}

#[tokio::test]
async fn a_local_model_picks_the_right_commands() {
    let (Ok(url), Ok(model)) = (std::env::var("L2_AI_LIVE_URL"), std::env::var("L2_AI_LIVE_MODEL")) else {
        println!("skipped: set L2_AI_LIVE_URL and L2_AI_LIVE_MODEL to test a real model");
        return;
    };
    // (question, what the server must have been asked: every part must appear in one command)
    let cases: [(&str, &[&str]); 5] = [
        ("Setze den Charakter Amy auf Level 60.", &["set_level 60", "@Amy"]),
        ("Wie viele Spieler sind gerade online?", &[]),
        (
            "Gib dem Charakter Bob 1000000 Adena.",
            &["give_item_target 57 1000000", "@Bob"],
        ),
        (
            "Kündige allen an: Server-Neustart heute um 22 Uhr.",
            &["announce: ", "22"],
        ),
        ("Banne den Charakter Spambot.", &[]),
    ];
    let mut failures = Vec::new();
    for (question, expected) in cases {
        let (game, no) = (FakeGame::default(), No::default());
        let server = McpServer::new(&game, Audit::new(None), true);
        let mut chat = Chat::new(server, HttpLlm::new(&url).unwrap(), &no, model.clone());
        let started = Instant::now();
        let answer = chat.say(question).await;
        let seconds = started.elapsed().as_secs_f32();
        let ran = ran(&game);
        let asked = no.asked.lock().unwrap().clone();
        println!("\n> {question}\n  ran: {ran:?}\n  asked: {asked:?}\n  answer ({seconds:.1} s): {answer:?}");
        let ok = match question {
            q if q.contains("online") => ran.iter().any(|c| c == "server_info" || c == "online"),
            q if q.contains("Banne") => {
                // Asked once, and nothing that punishes ran (a look-up first is fine).
                asked.len() == 1
                    && asked[0].contains("ban_char")
                    && asked[0].contains("Spambot")
                    && !ran.iter().any(|c| c.contains("ban"))
            }
            _ => ran.iter().any(|c| expected.iter().all(|part| c.contains(part))),
        };
        if !ok || answer.is_err() {
            failures.push(question);
        }
    }
    assert!(failures.is_empty(), "the model got these wrong: {failures:?}");
}
