//! MCP bridge (D-63): an AI assistant (an MCP client such as Claude Desktop, or a local model in
//! an MCP-capable chat app) runs the game server's admin commands.
//!
//! The bridge is its own process. It speaks MCP (JSON-RPC 2.0, one message per line) on stdin and
//! stdout, and the admin protocol (D-30) to the game server's admin endpoint or the supervisor.
//! The server already runs any admin command for a virtual GM with the master access level
//! (`GameAction::Command`); the bridge adds nothing to the game server, and the world loop never
//! waits for a model.
//!
//! # Safety
//! - Commands that punish, remove or stop something ([`guard::is_destructive`]) are refused by
//!   `run_admin_command` and only run through `run_destructive_admin_command`. An MCP client asks
//!   its user before a tool call unless told to always allow that tool: set the destructive one
//!   to "ask every time", and a person approves each of them, not the model.
//! - `--no-destructive` leaves that tool out entirely.
//! - Every command and its result goes to the audit log ([`audit::Audit`]).

pub mod audit;
pub mod backend;
pub mod guard;
pub mod server;
