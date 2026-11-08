//! AI assistant view (D-63): chat with a local model (Ollama or llama.cpp's `llama-server`) that
//! runs admin commands on the game server.
//!
//! ```text
//!   UI thread (this view)                 "ai-chat" thread (tokio runtime)
//!   say / reset / configure  ──cmd──►     l2_mcp::chat::Chat  ──HTTP──►  the model
//!   entries, confirm dialog  ◄─event──    GuiBackend  ──Requester──►  bridge  ──►  supervisor
//! ```
//!
//! The commands go through the GUI's own connection to the supervisor: the worker sends them
//! with a [`Requester`] and the answers come back to the UI as [`BridgeEvent::Response`], which
//! [`View::on_event`] hands to the waiting worker. A command that punishes, removes or stops
//! something runs only after the administrator pressed "Run" in a dialog of this view.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc as std_mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use l2_admin_api::{GameAction, Response};
use l2_mcp::audit::Audit;
use l2_mcp::backend::Backend;
use l2_mcp::chat::{Chat, Confirm, Step};
use l2_mcp::llm::HttpLlm;
use l2_mcp::server::McpServer;
use tokio::sync::{mpsc, oneshot};

use crate::app::AppCtx;
use crate::bridge::{BridgeEvent, Requester};
use crate::views::View;
use crate::views::game_admin::game_request;

/// How long a command may take on the server before the assistant is told it failed.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(60);
/// The chat keeps this many entries.
const MAX_ENTRIES: usize = 400;

/// The answers the worker waits for, by request id.
type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Response, String>>>>>;

/// What the worker needs to build its chat. A change rebuilds it (and forgets the conversation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiConfig {
    pub llm_url: String,
    pub model: String,
    pub allow_destructive: bool,
    pub audit_log: Option<PathBuf>,
}

/// The game server through the GUI's connection to the supervisor.
struct GuiBackend {
    requester: Requester,
    pending: Pending,
}

impl Backend for GuiBackend {
    async fn game_action(&self, action: GameAction) -> Result<Response, String> {
        let (tx, rx) = oneshot::channel();
        let id = self.requester.next_id();
        self.pending
            .lock()
            .map_err(|_| "the GUI is closing".to_owned())?
            .insert(id, tx);
        self.requester.send(id, game_request(action));
        match tokio::time::timeout(COMMAND_TIMEOUT, rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err("the GUI is closing".into()),
            Err(_) => {
                if let Ok(mut pending) = self.pending.lock() {
                    pending.remove(&id);
                }
                Err(format!(
                    "the server did not answer within {} s",
                    COMMAND_TIMEOUT.as_secs()
                ))
            }
        }
    }
}

/// A destructive command waiting for the administrator.
pub struct Question {
    pub command: String,
    pub reason: String,
    reply: oneshot::Sender<bool>,
}

/// Asks through the view's dialog.
struct DialogConfirm {
    events: Events,
}

impl Confirm for DialogConfirm {
    async fn approve(&self, command: &str, reason: &str) -> bool {
        let (reply, answer) = oneshot::channel();
        self.events.send(WorkerEvent::Question(Question {
            command: command.to_owned(),
            reason: reason.to_owned(),
            reply,
        }));
        // A closed dialog (the GUI ends) is a no.
        answer.await.unwrap_or(false)
    }
}

enum WorkerCommand {
    Configure(AiConfig),
    Say(String),
    Reset,
    Models,
}

enum WorkerEvent {
    Models(Result<Vec<String>, String>),
    Answer {
        result: Result<String, String>,
        steps: Vec<Step>,
    },
    Question(Question),
}

/// The worker's way back to the UI: a channel, and a repaint so the UI looks at it.
#[derive(Clone)]
struct Events {
    tx: std_mpsc::Sender<WorkerEvent>,
    ctx: egui::Context,
}

impl Events {
    fn send(&self, event: WorkerEvent) {
        let _ = self.tx.send(event);
        self.ctx.request_repaint();
    }
}

type GuiChat = Chat<GuiBackend, HttpLlm, DialogConfirm>;

/// The worker thread: one chat at a time, rebuilt when the settings change.
async fn work(
    mut commands: mpsc::UnboundedReceiver<WorkerCommand>,
    events: Events,
    requester: Requester,
    pending: Pending,
) {
    let mut config: Option<AiConfig> = None;
    let mut chat: Option<GuiChat> = None;
    let build = |config: &AiConfig| -> Result<GuiChat, String> {
        let llm = HttpLlm::new(&config.llm_url)?;
        let backend = GuiBackend {
            requester: requester.clone(),
            pending: Arc::clone(&pending),
        };
        let server = McpServer::new(
            backend,
            Audit::new(config.audit_log.clone()),
            config.allow_destructive,
        );
        let confirm = DialogConfirm {
            events: events.clone(),
        };
        Ok(Chat::new(server, llm, confirm, config.model.clone()))
    };
    while let Some(command) = commands.recv().await {
        match command {
            WorkerCommand::Configure(new) => {
                chat = build(&new).ok();
                config = Some(new);
            }
            WorkerCommand::Reset => {
                if let Some(chat) = chat.as_mut() {
                    chat.reset();
                }
            }
            WorkerCommand::Models => {
                let result = match config.as_ref().map(|c| HttpLlm::new(&c.llm_url)) {
                    Some(Ok(llm)) => llm.models().await,
                    Some(Err(error)) => Err(error),
                    None => Err("not configured yet".into()),
                };
                events.send(WorkerEvent::Models(result));
            }
            WorkerCommand::Say(text) => {
                let (result, steps) = match chat.as_mut() {
                    Some(chat) => {
                        let result = chat.say(&text).await;
                        (result, chat.steps().to_vec())
                    }
                    None => {
                        let why = config
                            .as_ref()
                            .map(|c| HttpLlm::new(&c.llm_url).err().unwrap_or_default())
                            .unwrap_or_default();
                        (Err(format!("the assistant is not set up: {why}")), Vec::new())
                    }
                };
                events.send(WorkerEvent::Answer { result, steps });
            }
        }
    }
}

struct Worker {
    commands: mpsc::UnboundedSender<WorkerCommand>,
    events: std_mpsc::Receiver<WorkerEvent>,
}

impl Worker {
    fn spawn(ctx: egui::Context, requester: Requester, pending: Pending) -> std::io::Result<Self> {
        let (commands, rx) = mpsc::unbounded_channel();
        let (tx, events) = std_mpsc::channel();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let events_out = Events { tx, ctx };
        std::thread::Builder::new()
            .name("ai-chat".into())
            .spawn(move || runtime.block_on(work(rx, events_out, requester, pending)))?;
        Ok(Self { commands, events })
    }

    fn send(&self, command: WorkerCommand) {
        let _ = self.commands.send(command);
    }
}

/// One line of the chat window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    You(String),
    Assistant(String),
    /// A tool the assistant ran.
    Ran(Step),
    Error(String),
    Note(String),
}

/// The text of a step line: `set_level 60 @Amy -> Done`.
pub fn step_line(step: &Step) -> String {
    let what = if step.what.is_empty() {
        step.tool.clone()
    } else {
        step.what.clone()
    };
    format!(
        "{} {} -> {}",
        if step.ok { "ok" } else { "failed" },
        what,
        step.answer
    )
}

/// The audit log next to the supervisor (`<run folder>/log/mcp-audit.log`).
pub fn audit_path(run_folder: Option<&std::path::Path>) -> Option<PathBuf> {
    run_folder.map(|folder| folder.join("log").join("mcp-audit.log"))
}

#[derive(Default)]
pub struct AiChatView {
    worker: Option<Worker>,
    pending: Pending,
    configured: Option<AiConfig>,
    entries: Vec<Entry>,
    input: String,
    busy: bool,
    models: Vec<String>,
    model_status: Option<Result<usize, String>>,
    question: Option<Question>,
    start_error: Option<String>,
}

impl AiChatView {
    fn push(&mut self, entry: Entry) {
        self.entries.push(entry);
        if self.entries.len() > MAX_ENTRIES {
            self.entries.drain(..self.entries.len() - MAX_ENTRIES);
        }
    }

    /// Starts the worker on first use.
    fn ensure_worker(&mut self, ctx: &egui::Context, cx: &AppCtx<'_>) {
        if self.worker.is_some() || self.start_error.is_some() {
            return;
        }
        match Worker::spawn(ctx.clone(), cx.bridge.requester(), Arc::clone(&self.pending)) {
            Ok(worker) => {
                worker.send(WorkerCommand::Models);
                self.worker = Some(worker);
            }
            Err(error) => self.start_error = Some(format!("The assistant could not start: {error}")),
        }
    }

    /// Hands the worker the settings when they changed (and the model list along).
    fn sync_config(&mut self, cx: &AppCtx<'_>) {
        let ai = &cx.settings.ai;
        let model = if ai.model.trim().is_empty() {
            self.models.first().cloned().unwrap_or_default()
        } else {
            ai.model.trim().to_owned()
        };
        let config = AiConfig {
            llm_url: ai.llm_url.trim().to_owned(),
            model,
            allow_destructive: ai.allow_destructive,
            audit_log: audit_path(cx.launch.run_folder.as_deref()),
        };
        if self.configured.as_ref() == Some(&config) {
            return;
        }
        let url_changed = self
            .configured
            .as_ref()
            .is_none_or(|c| c.llm_url != config.llm_url);
        if let Some(worker) = &self.worker {
            worker.send(WorkerCommand::Configure(config.clone()));
            if url_changed {
                worker.send(WorkerCommand::Models);
            }
        }
        if self.configured.is_some() && !self.entries.is_empty() {
            self.push(Entry::Note(
                "The settings changed: a new conversation starts.".into(),
            ));
        }
        self.configured = Some(config);
    }

    fn drain(&mut self) {
        let Some(worker) = &self.worker else {
            return;
        };
        let events: Vec<WorkerEvent> = worker.events.try_iter().collect();
        for event in events {
            match event {
                WorkerEvent::Models(result) => {
                    self.model_status = Some(result.as_ref().map(Vec::len).map_err(Clone::clone));
                    if let Ok(models) = result {
                        self.models = models;
                    }
                }
                WorkerEvent::Answer { result, steps } => {
                    self.busy = false;
                    for step in steps {
                        self.push(Entry::Ran(step));
                    }
                    match result {
                        Ok(text) => self.push(Entry::Assistant(text)),
                        Err(error) => self.push(Entry::Error(error)),
                    }
                }
                WorkerEvent::Question(question) => self.question = Some(question),
            }
        }
    }

    fn send_input(&mut self) {
        let text = self.input.trim().to_owned();
        if text.is_empty() || self.busy {
            return;
        }
        let Some(worker) = &self.worker else {
            return;
        };
        worker.send(WorkerCommand::Say(text.clone()));
        self.input.clear();
        self.busy = true;
        self.push(Entry::You(text));
    }

    fn settings_ui(&mut self, ui: &mut egui::Ui, cx: &mut AppCtx<'_>) {
        egui::Grid::new("ai_settings")
            .num_columns(2)
            .spacing([8.0, 4.0])
            .show(ui, |ui| {
                ui.label("Model server");
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut cx.settings.ai.llm_url).desired_width(240.0))
                        .on_hover_text(
                            "Ollama: http://127.0.0.1:11434   llama-server: http://127.0.0.1:8080",
                        );
                    if ui.button("Check").clicked()
                        && let Some(worker) = &self.worker
                    {
                        worker.send(WorkerCommand::Models);
                    }
                    match &self.model_status {
                        Some(Ok(n)) => {
                            ui.colored_label(
                                egui::Color32::from_rgb(60, 160, 60),
                                format!("● reachable, {n} model(s)"),
                            );
                        }
                        Some(Err(error)) => {
                            ui.colored_label(ui.visuals().error_fg_color, "● not reachable")
                                .on_hover_text(error);
                        }
                        None => {
                            ui.label("…");
                        }
                    }
                });
                ui.end_row();

                ui.label("Model");
                let shown = if cx.settings.ai.model.is_empty() {
                    self.models.first().cloned().unwrap_or_else(|| "(none)".into())
                } else {
                    cx.settings.ai.model.clone()
                };
                egui::ComboBox::from_id_salt("ai_model")
                    .selected_text(shown)
                    .width(320.0)
                    .show_ui(ui, |ui| {
                        for model in &self.models {
                            ui.selectable_value(&mut cx.settings.ai.model, model.clone(), model);
                        }
                    });
                ui.end_row();

                ui.label("Commands");
                ui.checkbox(
                    &mut cx.settings.ai.allow_destructive,
                    "Allow bans, kicks, kills and shutdowns (each one is asked about first)",
                );
                ui.end_row();
            });
        if !cx.conn.is_connected() {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                "Not connected to the supervisor: the assistant can talk but not run commands.",
            );
        }
        if matches!(&self.model_status, Some(Err(_))) {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                "The model server does not answer. Start Ollama (or llama-server) and press Check.",
            );
        }
    }

    fn chat_ui(&mut self, ui: &mut egui::Ui) {
        let input_height = 36.0;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .max_height((ui.available_height() - input_height).max(80.0))
            .show(ui, |ui| {
                if self.entries.is_empty() {
                    ui.weak("Write what you want done, e.g. \"How many players are online?\" or \"Set Amy to level 60\".");
                }
                for entry in &self.entries {
                    match entry {
                        Entry::You(text) => {
                            ui.label(egui::RichText::new(format!("You: {text}")).strong());
                        }
                        Entry::Assistant(text) => {
                            ui.label(format!("Assistant: {text}"));
                        }
                        Entry::Ran(step) => {
                            let color = if step.ok {
                                egui::Color32::from_rgb(60, 140, 60)
                            } else {
                                ui.visuals().error_fg_color
                            };
                            ui.colored_label(color, egui::RichText::new(step_line(step)).monospace().small());
                        }
                        Entry::Error(text) => {
                            ui.colored_label(ui.visuals().error_fg_color, format!("Error: {text}"));
                        }
                        Entry::Note(text) => {
                            ui.weak(text);
                        }
                    }
                    ui.add_space(2.0);
                }
                if self.busy {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.weak("The assistant is working…");
                    });
                }
            });
        ui.separator();
        ui.horizontal(|ui| {
            let edit = ui.add_enabled(
                !self.busy,
                egui::TextEdit::singleline(&mut self.input)
                    .hint_text("What should be done?")
                    .desired_width(ui.available_width() - 170.0),
            );
            let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ui.add_enabled(!self.busy, egui::Button::new("Send")).clicked() || enter {
                self.send_input();
                edit.request_focus();
            }
            if ui
                .add_enabled(!self.busy, egui::Button::new("New chat"))
                .clicked()
            {
                if let Some(worker) = &self.worker {
                    worker.send(WorkerCommand::Reset);
                }
                self.entries.clear();
            }
        });
    }

    fn question_ui(&mut self, ctx: &egui::Context) {
        let Some(question) = &self.question else {
            return;
        };
        let mut answer = None;
        egui::Window::new("Approve a command")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label("The assistant wants to run a command that punishes, removes or stops something:");
                ui.add_space(4.0);
                ui.label(egui::RichText::new(&question.command).monospace().strong());
                if !question.reason.is_empty() {
                    ui.label(format!("Reason: {}", question.reason));
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Run it").clicked() {
                        answer = Some(true);
                    }
                    if ui.button("Don't run").clicked() {
                        answer = Some(false);
                    }
                });
            });
        if let Some(yes) = answer
            && let Some(question) = self.question.take()
        {
            let _ = question.reply.send(yes);
        }
    }
}

impl View for AiChatView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut AppCtx<'_>) {
        let ctx = ui.ctx().clone();
        self.ensure_worker(&ctx, cx);
        self.drain();
        self.sync_config(cx);
        ui.heading("AI assistant");
        if let Some(error) = &self.start_error {
            ui.colored_label(ui.visuals().error_fg_color, error);
            return;
        }
        self.settings_ui(ui, cx);
        ui.separator();
        self.chat_ui(ui);
        self.question_ui(&ctx);
    }

    fn on_event(&mut self, event: &BridgeEvent) {
        if let BridgeEvent::Response { id, result } = event {
            let waiting = self.pending.lock().ok().and_then(|mut p| p.remove(id));
            if let Some(waiting) = waiting {
                let _ = waiting.send(result.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_step_reads_as_one_line() {
        let step = Step {
            tool: "run_admin_command".into(),
            what: "set_level 60 @Amy".into(),
            ok: true,
            answer: "Done".into(),
        };
        assert_eq!(step_line(&step), "ok set_level 60 @Amy -> Done");
        let empty = Step {
            tool: "server_info".into(),
            what: String::new(),
            ok: false,
            answer: "no connection".into(),
        };
        assert_eq!(step_line(&empty), "failed server_info -> no connection");
    }

    #[test]
    fn the_audit_log_lies_next_to_the_supervisor() {
        assert_eq!(audit_path(None), None);
        assert_eq!(
            audit_path(Some(std::path::Path::new("run"))),
            Some(PathBuf::from("run").join("log").join("mcp-audit.log"))
        );
    }

    /// The answer to a command the worker sent comes back to it through the view.
    #[tokio::test]
    async fn the_answer_of_the_server_reaches_the_waiting_worker() {
        let bridge = crate::bridge::Bridge::spawn(None).unwrap();
        let mut view = AiChatView::default();
        let backend = GuiBackend {
            requester: bridge.requester(),
            pending: Arc::clone(&view.pending),
        };
        let waiting = tokio::spawn(async move { backend.game_action(GameAction::ServerInfo).await });
        // The bridge is not connected: it answers "not connected" at once; the view routes it.
        let mut answered = false;
        for _ in 0..200 {
            while let Some(event) = bridge.try_recv() {
                view.on_event(&event);
                answered |= matches!(event, BridgeEvent::Response { .. });
            }
            if answered {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(answered, "the bridge never answered");
        let result = waiting.await.unwrap();
        assert_eq!(result, Err("not connected".to_owned()));
        assert!(view.pending.lock().unwrap().is_empty(), "nothing left waiting");
    }
}
