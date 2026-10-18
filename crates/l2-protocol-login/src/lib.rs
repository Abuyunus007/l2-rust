//! Login protocol: the packets between the game client and the login server.
//!
//! - [`client`]: client to login server (`AuthGameGuard`, `RequestAuthLogin`, ...)
//! - [`server`]: login server to client (`Init`, `LoginFail`, `ServerList`, ...)
//! - [`enums`]: the reason codes (`LoginFailReason`, `PlayFailReason`, `AccountKickedReason`)
//! - [`dispatch`]: `(opcode, connection state)` to a decoded client packet
//!
//! Plaintext packets only: no framing, no encryption, no login logic. Link packets between the
//! login and game server (port 9014) are added to this crate by M1-LPROTO-02.
//!
//! ## The packet pattern
//! One struct per packet with public fields, an `OPCODE` const, `impl Encode` and `impl Decode`,
//! and a `// Java: path/To/Class.java` line. Both traits are implemented for every login packet
//! (the server, the proxy and test clients each use a different half). `Decode` starts after the
//! opcode and tolerates trailing bytes. Wire constants from Java are named consts with a comment.
//! Parts of a packet that are not packets themselves (list entries) use plain `write` / `read`
//! methods instead of the traits.

pub mod client;
pub mod dispatch;
pub mod enums;
pub mod server;

pub use client::{
    AuthGameGuard, RequestAuthLogin, RequestServerList, RequestServerLogin, credentials_from_decrypted,
};
pub use dispatch::{ClientPacket, ConnectionState, decode_client};
pub use enums::{AccountKickedReason, LoginFailReason, PlayFailReason};
pub use server::{
    AccountKicked, GGAuth, Init, LoginFail, LoginOk, PlayFail, PlayOk, ServerCharInfo, ServerEntry,
    ServerList,
};
