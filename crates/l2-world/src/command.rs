//! What the rest of the server can ask the world to do.

use std::fmt;
use std::net::IpAddr;

use crate::outbox::PacketSink;
use crate::player::{OnlinePlayer, PlayerData};

/// One client connection, as numbered by the game server's listener.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SessionId(pub u64);

/// In-game client input, already decoded by the session task.
///
/// Later cards add variants as they port packets (movement, actions, …). Anything the world
/// doesn't handle yet arrives as [`ClientInput::Other`] and is ignored at debug level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientInput {
    /// Java `Logout`: store the character, `LeaveWorld`, close the connection.
    Logout,
    /// Java `RequestRestart`: store the character and go back to the character screen.
    Restart,
    /// Java `Say2`. `chat_type` is the number the client sent; the world checks it.
    Say {
        chat_type: i32,
        text: String,
        target: Option<String>,
    },
    /// Java `RequestNetPing`.
    NetPing,
    /// Java `RequestSkillCoolTime`.
    SkillCoolTime,
    /// Java `RequestKeyMapping` (answered with the stored UI key mapping).
    KeyMapping,
    /// Java `MoveToLocation` (the client wants to walk; `movement_mode` 1 = mouse, 0 = keys).
    MoveTo {
        target_x: i32,
        target_y: i32,
        target_z: i32,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
        movement_mode: i32,
    },
    /// Java `ValidatePosition`: where the client thinks the player is.
    ValidatePosition { x: i32, y: i32, z: i32, heading: i32 },
    /// Java `CannotMoveAnymore`.
    CannotMoveAnymore { x: i32, y: i32, z: i32, heading: i32 },
    /// Java `Appearing`: the client has loaded the target of a teleport.
    Appearing,
    /// Java `Action`: a click on an object (`action_id` 0 = click, 1 = shift click). The origin
    /// the client sends is read and never used by Java.
    Action { object_id: i32, action_id: i8 },
    /// Java `RequestTargetCanceld` (`unselect` 0 = Escape key).
    TargetCanceled { unselect: i16 },
    /// Java `RequestBypassToServer`: the raw command of an html link.
    Bypass { command: String },
    /// Java `RequestLinkHtml`: the raw path of a linked html page.
    LinkHtml { link: String },
    /// A packet the world doesn't handle yet.
    Other { opcode: u8, ex_opcode: Option<u16> },
}

/// Why a player left the world. The session task gets this through [`PacketSink::left_world`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaveReason {
    Logout,
    /// Back to the character screen; the connection stays.
    Restart,
    Disconnected,
    Kicked,
    ServerShutdown,
}

/// A question for the admin endpoint. The answer goes to the callback (called on the world
/// thread, so it must only hand the value over, e.g. into a channel).
pub enum AdminQuery {
    OnlinePlayers(Box<dyn FnOnce(Vec<OnlinePlayer>) + Send>),
}

impl fmt::Debug for AdminQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OnlinePlayers(_) => f.write_str("OnlinePlayers(..)"),
        }
    }
}

/// Everything that can enter the world.
pub enum WorldCommand {
    /// The session loaded the character from the database; put it into the world.
    EnterWorld {
        session: SessionId,
        player: Box<PlayerData>,
        sink: Box<dyn PacketSink>,
        /// The client's address, from its connection. `None` when the session doesn't know it.
        /// Only the admin status shows it.
        ip: Option<IpAddr>,
    },
    /// An in-game packet from the client of `session`.
    Input {
        session: SessionId,
        input: ClientInput,
    },
    /// The connection is gone (the character is stored, no packets are sent).
    Disconnected {
        session: SessionId,
    },
    /// The login server asked to kick this account.
    Kick {
        account: String,
    },
    Admin(AdminQuery),
    /// A path worker finished a search (M3-GEO-03). `requester` and `request_id` are the ones
    /// of the `l2_geo::PathRequest`; the movement code (M3-MOVE-01) matches them against the
    /// creature's current request and drops stale answers.
    PathFound {
        requester: u64,
        request_id: u64,
        result: l2_geo::PathResult,
    },
    /// Installs the path search service (the pool starts after the world thread, so it cannot be
    /// given to the world before it runs).
    SetPathRequests(Box<dyn crate::movement::PathRequests>),
    /// Store everyone, tell their sessions, then call `done` and stop the world thread.
    Shutdown {
        done: Box<dyn FnOnce() + Send>,
    },
}

impl fmt::Debug for WorldCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EnterWorld { session, player, .. } => {
                write!(f, "EnterWorld({session:?}, {})", player.name)
            }
            Self::Input { session, input } => write!(f, "Input({session:?}, {input:?})"),
            Self::Disconnected { session } => write!(f, "Disconnected({session:?})"),
            Self::Kick { account } => write!(f, "Kick({account})"),
            Self::Admin(query) => write!(f, "Admin({query:?})"),
            Self::PathFound {
                requester,
                request_id,
                result,
            } => write!(
                f,
                "PathFound({requester}, {request_id}, {})",
                if result.path.is_ok() { "path" } else { "no path" }
            ),
            Self::SetPathRequests(_) => f.write_str("SetPathRequests"),
            Self::Shutdown { .. } => f.write_str("Shutdown"),
        }
    }
}
