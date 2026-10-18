//! `GameState`: everything the game server's tasks share (network, LS link, persistence,
//! admin), plus the exit codes and the shutdown request channel.
// Java: org/l2jmobius/gameserver/GameServer.java (EXIT codes are in Shutdown.java)
// Java: org/l2jmobius/gameserver/Shutdown.java (`System.exit(0)` / `System.exit(2)`)
//
// Same pattern as `login-server/src/state.rs`: one struct behind an `Arc`, built once at
// startup. This is the server process, not the game world: the world's own state lives in the
// world thread and is only reachable through [`WorldHandle`] (D-10).

use std::sync::Arc;
use std::sync::mpsc::Sender;

use l2_config::GameConfig;
use l2_data::Datapack;
use l2_db::Pool;
use l2_world::{IdFactory, WorldCommand};
use tokio::sync::mpsc;

use crate::Paths;
use crate::hexid::HexId;

/// Why and how the process should end. The exit code is what the start scripts and the
/// supervisor look at.
// Java: Shutdown.java (`System.exit(0)` shutdown, `System.exit(2)` restart); a fatal startup
// error ends the Java process through an uncaught exception, here it is code 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitReason {
    /// Normal shutdown, exit code 0.
    Shutdown,
    /// Fatal startup error, exit code 1.
    Fatal,
    /// Restart requested, exit code 2 (the wrapper starts the server again).
    Restart,
}

impl ExitReason {
    pub fn code(self) -> u8 {
        match self {
            Self::Shutdown => 0,
            Self::Fatal => 1,
            Self::Restart => 2,
        }
    }
}

/// Cloneable way to ask the running server to stop: the admin endpoint, the scheduled restart
/// (later) and the GM shutdown command (later) hold one.
#[derive(Debug, Clone)]
pub struct ShutdownHandle {
    sender: mpsc::UnboundedSender<ExitReason>,
}

impl ShutdownHandle {
    /// A handle and the receiver `Server::run_until` waits on.
    pub fn channel() -> (Self, mpsc::UnboundedReceiver<ExitReason>) {
        let (sender, receiver) = mpsc::unbounded_channel();
        (Self { sender }, receiver)
    }

    /// Asks for a shutdown, or a restart (exit code 2). A request after the server already
    /// stopped is ignored.
    pub fn request(&self, restart: bool) {
        let reason = if restart {
            ExitReason::Restart
        } else {
            ExitReason::Shutdown
        };
        let _ = self.sender.send(reason);
    }
}

/// How network tasks reach the world thread: a cloneable, non-blocking sender of
/// [`WorldCommand`]s (design: `docs/design/world-loop.md`).
#[derive(Clone)]
pub struct WorldHandle {
    commands: Sender<WorldCommand>,
}

impl WorldHandle {
    pub(crate) fn new(commands: Sender<WorldCommand>) -> Self {
        Self { commands }
    }

    /// Queues a command. `false` when the world thread is gone (shutting down).
    pub fn send(&self, command: WorldCommand) -> bool {
        self.commands.send(command).is_ok()
    }
}

/// Everything the tasks of the game server share. Read-only after startup.
pub struct GameState {
    pub config: Arc<GameConfig>,
    pub paths: Paths,
    /// The game database (same schema as Java, D-21). Persistence (M2-PERSIST-01) and the
    /// character screen (M2-GS-03) use it; the world never does (D-15).
    pub pool: Pool,
    /// Object ids from the Rust range (D-39).
    pub ids: Arc<IdFactory>,
    pub datapack: Arc<Datapack>,
    pub world: WorldHandle,
    /// The registration found in `config/hexid.txt`, if any (M2-GS-02 uses it for `AuthRequest`
    /// and writes the file after a new registration with [`crate::hexid::save`]).
    pub hexid: Option<HexId>,
    pub(crate) shutdown: ShutdownHandle,
}

impl GameState {
    /// Asks the server to stop (`restart`: exit code 2).
    pub fn request_shutdown(&self, restart: bool) {
        self.shutdown.request(restart);
    }

    /// A cloneable handle for tasks that request shutdowns on their own.
    pub fn shutdown_handle(&self) -> ShutdownHandle {
        self.shutdown.clone()
    }
}
