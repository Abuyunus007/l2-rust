//! The chat of `l2-ai`: the administrator talks to a local model, and the model runs the bridge's
//! tools ([`McpServer::call_tool`]) in the same process. A command that punishes, removes or stops
//! something runs only after the administrator said yes ([`Confirm`]); the model cannot approve
//! itself.

use std::future::Future;

use serde_json::{Value, json};

use crate::backend::Backend;
use crate::llm::Llm;
use crate::server::{INSTRUCTIONS, McpServer, RUN_DESTRUCTIVE, ToolAnswer};

/// How many rounds of tool calls one question may take before the chat stops it.
const MAX_STEPS: usize = 8;

/// What the chat adds to the bridge's instructions.
const CHAT_RULES: &str = "Answer in the administrator's language, in a few short sentences. When \
you are not sure how a command is written, look it up with command_help before you run it. Run one \
command at a time, read what the server answered, and say what happened. Never invent a result.";

/// Asks the administrator about a destructive command. `true`: run it.
pub trait Confirm: Send + Sync {
    fn approve(&self, command: &str, reason: &str) -> impl Future<Output = bool> + Send;
}

/// One tool the model ran while answering, for a window that shows the work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// The tool's name.
    pub tool: String,
    /// What it was asked: the command (with ` @target`), the query or the text.
    pub what: String,
    pub ok: bool,
    /// The first line of the answer.
    pub answer: String,
}

/// One conversation with the model.
pub struct Chat<B, L, C> {
    server: McpServer<B>,
    llm: L,
    confirm: C,
    model: String,
    messages: Vec<Value>,
    /// The destructive commands the administrator said no to in this question: the model is not
    /// allowed to ask again (a small model tends to try once more).
    declined: Vec<String>,
    /// The tools run for the last question.
    steps: Vec<Step>,
}

impl<B: Backend, L: Llm, C: Confirm> Chat<B, L, C> {
    /// `model`: the name the endpoint knows the model by (Ollama needs it; `llama-server` takes
    /// any).
    pub fn new(server: McpServer<B>, llm: L, confirm: C, model: String) -> Self {
        let mut chat = Self {
            server,
            llm,
            confirm,
            model,
            messages: Vec::new(),
            declined: Vec::new(),
            steps: Vec::new(),
        };
        chat.reset();
        chat
    }

    /// Forgets the conversation.
    pub fn reset(&mut self) {
        self.messages =
            vec![json!({ "role": "system", "content": format!("{INSTRUCTIONS}\n\n{CHAT_RULES}") })];
    }

    /// The tools in the form of the OpenAI chat API.
    fn tools(&self) -> Vec<Value> {
        self.server
            .tools()
            .into_iter()
            .map(|tool| {
                json!({
                    "type": "function",
                    "function": {
                        "name": tool["name"],
                        "description": tool["description"],
                        "parameters": tool["inputSchema"],
                    }
                })
            })
            .collect()
    }

    /// One question of the administrator: the model answers, running tools on the way. `Err`
    /// when the model cannot be reached or answers nonsense; the question stays in the
    /// conversation either way.
    pub async fn say(&mut self, text: &str) -> Result<String, String> {
        self.messages.push(json!({ "role": "user", "content": text }));
        self.declined.clear();
        self.steps.clear();
        let tools = self.tools();
        for _ in 0..MAX_STEPS {
            let request = json!({
                "model": self.model,
                "messages": self.messages,
                "tools": tools,
                "tool_choice": "auto",
                "temperature": 0.2,
            });
            let response = self.llm.complete(request).await?;
            let Some(message) = response.pointer("/choices/0/message").cloned() else {
                return Err(format!("the model answered without a message: {response}"));
            };
            let calls = message
                .get("tool_calls")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let content = message
                .get("content")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            let mut kept = json!({ "role": "assistant", "content": content });
            if !calls.is_empty() {
                kept["tool_calls"] = Value::Array(calls.clone());
            }
            self.messages.push(kept);
            if calls.is_empty() {
                return Ok(clean(&content));
            }
            for call in &calls {
                let id = call.get("id").cloned().unwrap_or(Value::Null);
                let answer = self.run_call(call).await;
                self.steps.push(step_of(call, &answer));
                let content = if answer.ok {
                    answer.text
                } else {
                    format!("Error: {}", answer.text)
                };
                self.messages
                    .push(json!({ "role": "tool", "tool_call_id": id, "content": content }));
            }
        }
        Ok(format!(
            "(Stopped after {MAX_STEPS} rounds of commands without a final answer. Ask again, more precisely.)"
        ))
    }

    /// Runs one tool call of the model, asking the administrator first where needed.
    async fn run_call(&mut self, call: &Value) -> ToolAnswer {
        let name = call
            .pointer("/function/name")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let args = match call.pointer("/function/arguments") {
            // The OpenAI API sends the arguments as a JSON text; some servers send the object.
            Some(Value::String(text)) if text.trim().is_empty() => json!({}),
            Some(Value::String(text)) => match serde_json::from_str::<Value>(text) {
                Ok(args) => args,
                Err(_) => return failed(&format!("the arguments of {name} are not JSON: {text}")),
            },
            Some(args) => args.clone(),
            None => json!({}),
        };
        if name == RUN_DESTRUCTIVE {
            let command = args.get("command").and_then(Value::as_str).unwrap_or_default();
            let reason = args.get("reason").and_then(Value::as_str).unwrap_or_default();
            if command.trim().is_empty() {
                return failed("The command is empty.");
            }
            let key = command.trim().to_lowercase();
            let refusal = format!(
                "The administrator answered NO to '{command}'. It did not run. Do not try it again;                  tell the administrator that it was not run because they declined, and ask what to do instead."
            );
            if self.declined.contains(&key) {
                return failed(&refusal);
            }
            if !self.confirm.approve(command, reason).await {
                self.server.note_declined(name, command);
                self.declined.push(key);
                return failed(&refusal);
            }
        }
        match self.server.call_tool(name, &args).await {
            Some(answer) => answer,
            None => failed(&format!("there is no tool {name}")),
        }
    }
}

impl<B, L, C> Chat<B, L, C> {
    /// The tools the model ran for the last question, in order.
    pub fn steps(&self) -> &[Step] {
        &self.steps
    }
}

/// What a tool call was, for [`Step`].
fn step_of(call: &Value, answer: &ToolAnswer) -> Step {
    let tool = call
        .pointer("/function/name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let args = match call.pointer("/function/arguments") {
        Some(Value::String(text)) => serde_json::from_str::<Value>(text).unwrap_or(Value::Null),
        Some(args) => args.clone(),
        None => Value::Null,
    };
    let field = |key: &str| args.get(key).and_then(Value::as_str).unwrap_or_default().trim();
    let what = match (field("command"), field("target")) {
        ("", _) => [field("query"), field("text")].concat(),
        (command, "") => command.to_owned(),
        (command, target) => format!("{command} @{target}"),
    };
    Step {
        tool,
        what,
        ok: answer.ok,
        answer: answer.text.lines().next().unwrap_or_default().to_owned(),
    }
}

fn failed(text: &str) -> ToolAnswer {
    ToolAnswer {
        ok: false,
        text: text.to_owned(),
    }
}

/// The answer without a model's thinking (`<think>...</think>`).
fn clean(content: &str) -> String {
    let mut text = content.to_owned();
    while let (Some(start), Some(end)) = (text.find("<think>"), text.find("</think>")) {
        if end < start {
            break;
        }
        text.replace_range(start..end + "</think>".len(), "");
    }
    text.trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_thinking_is_left_out() {
        assert_eq!(clean("<think>hmm</think>\nAmy is level 60."), "Amy is level 60.");
        assert_eq!(clean("  plain  "), "plain");
        assert_eq!(clean("</think> odd <think>"), "</think> odd <think>");
    }
}
