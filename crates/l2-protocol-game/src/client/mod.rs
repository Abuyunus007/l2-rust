//! Packets the game client sends to the game server.
//!
//! One struct per packet, copied from the login crate's pattern (see `l2_net::packet`):
//! - a `// Java: path/To/Class.java` line,
//! - `OPCODE` (and `EX_OPCODE` for `0xD0` packets) taken from the generated tables, never typed
//!   by hand,
//! - `impl Decode`: the fields **after** the opcode(s), exactly what Java `readImpl` reads. A
//!   short packet gives [`PacketError::Truncated`](l2_net::PacketError); trailing bytes are
//!   ignored (the client pads packets),
//! - `impl Encode`: opcode(s) + fields, for test clients and the proxy.
//!
//! Which packets are accepted in which connection state is not decided here but by
//! [`crate::dispatch`], from the generated tables.

use l2_net::PacketWriter;

use crate::generated::opcodes::{ClientOpcode, ExClientOpcode};

pub mod charscreen;
pub mod enterworld;
pub mod handshake;
pub mod ingame;
pub mod interaction;

pub use charscreen::{
    CharacterCreate, CharacterDelete, CharacterRestore, CharacterSelect, NewCharacter, RequestGotoLobby,
};
pub use enterworld::{EnterWorld, RequestKeyMapping};
pub use handshake::{AuthLogin, ProtocolVersion};
pub use ingame::{Logout, RequestNetPing, RequestRestart, RequestSkillCoolTime, Say2};
pub use interaction::{Action, RequestBypassToServer, RequestLinkHtml, RequestTargetCanceld};

/// Writes the id byte of a plain client packet.
pub(crate) fn write_opcode(w: &mut PacketWriter, opcode: ClientOpcode) {
    w.u8(opcode.id());
}

/// Writes `0xD0` followed by the `u16` ex id of an ex client packet.
pub(crate) fn write_ex_opcode(w: &mut PacketWriter, ex: ExClientOpcode) {
    w.u8(ClientOpcode::ExPacket.id()).u16(ex.id());
}
